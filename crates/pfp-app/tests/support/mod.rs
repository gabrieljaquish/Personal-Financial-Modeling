//! Test support for the launcher: **the only place a test starts a process**, and
//! the raw TLS/HTTP-1.1 client the binary-level tests talk through.
//!
//! Machine-safety contract, enforced here rather than remembered by each test:
//!
//! * the server is only ever started as
//!   `pfp serve --port 0 --no-open --no-trust --state-dir <fresh temp dir>` — the
//!   flag list is the constant [`SERVE_FLAGS`]; no test can add `--install-trust`
//!   or drop `--no-open` / `--no-trust`;
//! * the child gets an **empty environment** (no `HOME`, so even a bug in the flag
//!   handling could not reach a real state directory) and its **own process
//!   group**; teardown signals the whole group (SIGTERM, then SIGKILL after 2 s),
//!   reaps the child, then **asserts** the group is empty and the port refuses
//!   connections, and removes the temporary state directory;
//! * the other entry points run `pfp` only with a command that starts nothing
//!   (`--version`, `--help`, `openapi`), or re-execute the *test* executable;
//! * the client trusts exactly one anchor — the CA certificate the child wrote to
//!   its state directory — and connects only to `127.0.0.1:<the child's port>`.
//!   There is no HTTP client crate in this workspace (ADR-020).

// Each integration test compiles this module and uses a different part of it.
#![allow(dead_code)]

use std::fmt::Write as _;
use std::net::{Ipv4Addr, SocketAddrV4, TcpStream as StdTcpStream};
use std::os::unix::process::CommandExt as _;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Output, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use rustix::process::{kill_process_group, test_kill_process_group, Pid, Signal};
use rustls::pki_types::{CertificateDer, ServerName};
use rustls::{ClientConfig, RootCertStore};
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
use tokio::net::TcpStream;
use tokio_rustls::TlsConnector;

/// The safety flags of every server start. `--state-dir <dir>` is appended.
pub(crate) const SERVE_FLAGS: [&str; 5] = ["serve", "--port", "0", "--no-open", "--no-trust"];

/// Commands that start nothing, the only other way a test may run `pfp`.
const INERT_COMMANDS: [&str; 3] = ["--version", "--help", "openapi"];

/// `sandbox-exec`'s profile for S-07: everything allowed except outbound network.
const DENY_OUTBOUND_PROFILE: &str = "(version 1)(allow default)(deny network-outbound)";
const SANDBOX_EXEC: &str = "/usr/bin/sandbox-exec";

/// Scheduling slack on top of a server-side timer. Larger than in the in-process
/// server tests: these waits include spawning a process on a loaded runner.
pub(crate) const MARGIN: Duration = Duration::from_secs(10);

/// For waits on a child process (start-up, exit) and on a plaintext peer being
/// dropped, which the server does at `TLS_HANDSHAKE_TIMEOUT` at the latest. A
/// harness wait must outlast the server timer it observes, or a stall shows up as
/// a harness timeout instead of as what the server did.
pub(crate) const SOON: Duration =
    Duration::from_secs(pfp_server::limits::TLS_HANDSHAKE_TIMEOUT.as_secs() + MARGIN.as_secs());

/// For a complete HTTPS exchange: the slowest legitimate answer is the 408 at
/// `REQUEST_DEADLINE`, followed by at most `LINGER_TIMEOUT` of graceful close.
pub(crate) const PAST_REQUEST_DEADLINE: Duration = Duration::from_secs(
    pfp_server::limits::REQUEST_DEADLINE.as_secs()
        + pfp_server::limits::LINGER_TIMEOUT.as_secs()
        + MARGIN.as_secs(),
);

static COUNTER: AtomicU32 = AtomicU32::new(0);

/// A fresh directory under the system temporary directory — never the repository.
pub(crate) fn fresh_temp_dir(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "pfp-app-test-{label}-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn collect(mut stream: impl std::io::Read + Send + 'static) -> Arc<Mutex<Vec<u8>>> {
    let sink = Arc::new(Mutex::new(Vec::new()));
    let writer = Arc::clone(&sink);
    std::thread::spawn(move || {
        let mut chunk = [0_u8; 1024];
        while let Ok(n @ 1..) = stream.read(&mut chunk) {
            writer
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .extend_from_slice(&chunk[..n]);
        }
    });
    sink
}

/// What the `PFP-READY` line said.
#[derive(Debug, Clone)]
pub(crate) struct Ready {
    pub(crate) port: u16,
    pub(crate) origin: String,
    pub(crate) fingerprint: String,
    pub(crate) line: String,
}

/// A running `pfp serve`, in its own process group, torn down on drop.
pub(crate) struct PfpProcess {
    child: Child,
    group: Pid,
    stdout: Arc<Mutex<Vec<u8>>>,
    stderr: Arc<Mutex<Vec<u8>>>,
    state_dir: PathBuf,
    owns_state_dir: bool,
    port: Option<u16>,
    started: Instant,
}

fn spawn(mut command: Command, state_dir: PathBuf, owns_state_dir: bool) -> PfpProcess {
    let started = Instant::now();
    let mut child = command
        .args(SERVE_FLAGS)
        .arg("--state-dir")
        .arg(&state_dir)
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0)
        .spawn()
        .expect("the pfp binary starts");
    let group = Pid::from_child(&child);
    let stdout = collect(child.stdout.take().expect("stdout"));
    let stderr = collect(child.stderr.take().expect("stderr"));
    PfpProcess {
        child,
        group,
        stdout,
        stderr,
        state_dir,
        owns_state_dir,
        port: None,
        started,
    }
}

/// Entry point 1: `pfp` with the constant flags and a fresh state directory.
pub(crate) fn spawn_pfp() -> PfpProcess {
    spawn(
        Command::new(env!("CARGO_BIN_EXE_pfp")),
        fresh_temp_dir("state"),
        true,
    )
}

/// Entry point 1b: the same, on a state directory another instance may hold (the
/// single-instance test). The caller's first process owns and removes it.
pub(crate) fn spawn_pfp_sharing(state_dir: &Path) -> PfpProcess {
    spawn(
        Command::new(env!("CARGO_BIN_EXE_pfp")),
        state_dir.to_path_buf(),
        false,
    )
}

/// Entry point 2 (S-07): the same command line under a deny-outbound sandbox
/// profile passed inline. Constrains only the child; changes no setting.
/// `None` when this machine has no `sandbox-exec`.
pub(crate) fn spawn_pfp_sandboxed() -> Option<PfpProcess> {
    if !Path::new(SANDBOX_EXEC).is_file() {
        return None;
    }
    let mut command = Command::new(SANDBOX_EXEC);
    command
        .arg("-p")
        .arg(DENY_OUTBOUND_PROFILE)
        .arg(env!("CARGO_BIN_EXE_pfp"));
    Some(spawn(command, fresh_temp_dir("sandbox"), true))
}

/// Entry point 3 (S-20): re-executes the **current test executable** with one
/// marker variable set. Never `pfp`, never a server.
pub(crate) fn spawn_self(marker: &str) -> Output {
    let child = Command::new(std::env::current_exe().expect("current test executable"))
        .env_clear()
        .env(marker, "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0)
        .spawn()
        .expect("the test executable starts");
    let group = Pid::from_child(&child);
    let output = child.wait_with_output().expect("child output");
    assert!(
        test_kill_process_group(group).is_err(),
        "the child's group is empty"
    );
    output
}

/// Runs `pfp` with a command that starts nothing and waits for it to exit.
pub(crate) fn run_pfp_inert(command: &str) -> Output {
    assert!(
        INERT_COMMANDS.contains(&command),
        "{command} is not an inert command"
    );
    Command::new(env!("CARGO_BIN_EXE_pfp"))
        .arg(command)
        .env_clear()
        .stdin(Stdio::null())
        .output()
        .expect("the pfp binary runs")
}

impl PfpProcess {
    fn text(buffer: &Arc<Mutex<Vec<u8>>>) -> String {
        String::from_utf8_lossy(&buffer.lock().unwrap_or_else(PoisonError::into_inner)).into_owned()
    }

    pub(crate) fn stdout(&self) -> String {
        Self::text(&self.stdout)
    }

    pub(crate) fn stderr(&self) -> String {
        Self::text(&self.stderr)
    }

    pub(crate) fn state_dir(&self) -> &Path {
        &self.state_dir
    }

    pub(crate) fn started(&self) -> Instant {
        self.started
    }

    /// Waits for the ready line; `Err(exit status)` if the process ended first.
    pub(crate) fn wait_ready(&mut self) -> Result<Ready, ExitStatus> {
        let deadline = Instant::now() + SOON;
        loop {
            let stdout = self.stdout();
            if let Some(line) = stdout.lines().find(|l| l.starts_with("PFP-READY ")) {
                if stdout.contains(&format!("{line}\n")) {
                    let field = |name: &str| {
                        line.split(' ')
                            .find_map(|part| part.strip_prefix(name))
                            .unwrap_or_else(|| panic!("ready line has {name}"))
                            .to_owned()
                    };
                    let origin = field("origin=");
                    let port: u16 = origin
                        .strip_prefix("https://127.0.0.1:")
                        .expect("canonical origin")
                        .parse()
                        .expect("port");
                    self.port = Some(port);
                    return Ok(Ready {
                        port,
                        origin,
                        fingerprint: field("fingerprint="),
                        line: line.to_owned(),
                    });
                }
            }
            if let Some(status) = self.child.try_wait().expect("try_wait") {
                return Err(status);
            }
            assert!(
                Instant::now() < deadline,
                "pfp did not become ready: {}",
                self.stderr()
            );
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    /// Waits for the process to exit by itself (the second-instance test).
    pub(crate) fn wait_exit(&mut self) -> ExitStatus {
        let deadline = Instant::now() + SOON;
        loop {
            if let Some(status) = self.child.try_wait().expect("try_wait") {
                return status;
            }
            assert!(Instant::now() < deadline, "pfp did not exit");
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    /// SIGTERM to the group, then waits for a graceful exit.
    pub(crate) fn terminate(&mut self) -> ExitStatus {
        kill_process_group(self.group, Signal::TERM).expect("SIGTERM");
        let status = self.wait_exit();
        // Let the reader threads drain the pipes.
        std::thread::sleep(Duration::from_millis(50));
        status
    }

    /// The CA certificate the child wrote to its state directory (public).
    pub(crate) fn ca_cert(&self) -> CertificateDer<'static> {
        let der =
            std::fs::read(self.state_dir.join("tls").join("ca-cert.der")).expect("CA certificate");
        CertificateDer::from(der)
    }

    pub(crate) fn client(&self) -> Client {
        Client::new(self.port.expect("wait_ready first"), &self.ca_cert())
    }
}

impl Drop for PfpProcess {
    fn drop(&mut self) {
        let _ = kill_process_group(self.group, Signal::TERM);
        let deadline = Instant::now() + Duration::from_secs(2);
        let mut reaped = false;
        while Instant::now() < deadline {
            if matches!(self.child.try_wait(), Ok(Some(_))) {
                reaped = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let _ = kill_process_group(self.group, Signal::KILL);
        if !reaped {
            let _ = self.child.wait();
        }
        // A forked grandchild may need a moment to die after SIGKILL.
        let gone = (0..200).any(|_| {
            let empty = test_kill_process_group(self.group).is_err();
            if !empty {
                std::thread::sleep(Duration::from_millis(10));
            }
            empty
        });
        if self.owns_state_dir {
            let _ = std::fs::remove_dir_all(&self.state_dir);
        }
        if std::thread::panicking() {
            return;
        }
        assert!(gone, "the child's process group is empty after teardown");
        if let Some(port) = self.port {
            let addr = SocketAddrV4::new(Ipv4Addr::LOCALHOST, port);
            assert!(
                StdTcpStream::connect_timeout(&addr.into(), Duration::from_secs(1)).is_err(),
                "the child's port refuses connections after teardown"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// The raw client
// ---------------------------------------------------------------------------

/// The policy headers, typed from `SECURITY.md` §7.3 independently of the server.
pub(crate) const POLICY_HEADERS: [(&str, &str); 5] = [
    (
        "content-security-policy",
        "default-src 'none'; script-src 'self'; style-src 'self'; img-src 'self' data:; connect-src 'self'; font-src 'self'; frame-ancestors 'none'; base-uri 'none'; form-action 'none'",
    ),
    ("cross-origin-opener-policy", "same-origin"),
    ("cross-origin-resource-policy", "same-origin"),
    ("x-content-type-options", "nosniff"),
    ("referrer-policy", "no-referrer"),
];
pub(crate) const NO_STORE: &str = "no-store";
pub(crate) const IMMUTABLE: &str = "public, max-age=31536000, immutable";
pub(crate) const BOOTSTRAP: &str = "/api/v1/session/bootstrap";

/// A parsed response. Header names are lower-cased; order and duplicates are kept.
#[derive(Debug, Clone)]
pub(crate) struct Resp {
    pub(crate) status: u16,
    pub(crate) headers: Vec<(String, String)>,
    pub(crate) body: Vec<u8>,
}

impl Resp {
    fn parse(bytes: &[u8]) -> Self {
        let split = bytes
            .windows(4)
            .position(|w| w == b"\r\n\r\n")
            .expect("a complete response head");
        let head = String::from_utf8_lossy(&bytes[..split]).into_owned();
        let mut lines = head.split("\r\n");
        let status = lines
            .next()
            .and_then(|l| l.split(' ').nth(1))
            .and_then(|s| s.parse().ok())
            .expect("status line");
        let headers = lines
            .filter_map(|l| l.split_once(':'))
            .map(|(n, v)| (n.trim().to_ascii_lowercase(), v.trim().to_owned()))
            .collect();
        Self {
            status,
            headers,
            body: bytes[split + 4..].to_vec(),
        }
    }

    pub(crate) fn all(&self, name: &str) -> Vec<&str> {
        self.headers
            .iter()
            .filter(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
            .collect()
    }

    pub(crate) fn header(&self, name: &str) -> Option<&str> {
        self.all(name).first().copied()
    }

    pub(crate) fn json(&self) -> serde_json::Value {
        serde_json::from_slice(&self.body).expect("JSON body")
    }
}

/// The session pair. `Debug` prints nothing of it.
pub(crate) struct Session {
    cookie: String,
    proof: String,
}

/// A TLS client for one `127.0.0.1:<port>` that trusts exactly one CA.
pub(crate) struct Client {
    port: u16,
    config: Arc<ClientConfig>,
    runtime: tokio::runtime::Runtime,
}

impl Client {
    pub(crate) fn new(port: u16, ca: &CertificateDer<'static>) -> Self {
        let mut roots = RootCertStore::empty();
        roots.add(ca.clone()).expect("CA is a usable trust anchor");
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let mut config = ClientConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()
            .expect("protocol versions")
            .with_root_certificates(roots)
            .with_no_client_auth();
        config.alpn_protocols = vec![b"http/1.1".to_vec()];
        Self {
            port,
            config: Arc::new(config),
            runtime: tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("runtime"),
        }
    }

    fn origin(&self) -> String {
        format!("https://127.0.0.1:{}", self.port)
    }

    /// One request on one connection (`Connection: close`), full chain and name
    /// validation against the single trusted CA. Every response passes the
    /// universal negatives before the test sees it.
    fn exchange(
        &self,
        method: &str,
        path: &str,
        headers: &[(&str, String)],
        body: Option<&str>,
    ) -> Resp {
        let mut request = format!(
            "{method} {path} HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nConnection: close\r\n",
            self.port
        );
        for (name, value) in headers {
            let _ = write!(request, "{name}: {value}\r\n");
        }
        if let Some(body) = body {
            let _ = write!(
                request,
                "Content-Type: application/json
Content-Length: {}
",
                body.len()
            );
        }
        request.push_str("\r\n");
        request.push_str(body.unwrap_or_default());

        let addr = SocketAddrV4::new(Ipv4Addr::LOCALHOST, self.port);
        let config = Arc::clone(&self.config);
        let bytes = self.runtime.block_on(async move {
            tokio::time::timeout(PAST_REQUEST_DEADLINE, async move {
                let tcp = TcpStream::connect(addr).await.expect("connect");
                let name = ServerName::try_from("127.0.0.1").expect("server name");
                let mut tls = TlsConnector::from(config)
                    .connect(name, tcp)
                    .await
                    .expect("TLS handshake");
                tls.write_all(request.as_bytes()).await.expect("write");
                // `write_all` only hands the bytes to rustls; `flush` puts them on
                // the socket. Without it the server can be left waiting for them.
                tls.flush().await.expect("flush");
                let mut response = Vec::new();
                // The server may close without close_notify after `Connection: close`.
                let _ = tls.read_to_end(&mut response).await;
                response
            })
            .await
            .expect("response in time")
        });
        let response = Resp::parse(&bytes);
        assert_universal_invariants(method, path, &response);
        response
    }

    /// A top-level navigation, as a browser sends it.
    pub(crate) fn navigate(&self, path: &str) -> Resp {
        self.exchange(
            "GET",
            path,
            &[
                ("Sec-Fetch-Site", "none".to_owned()),
                ("Sec-Fetch-Mode", "navigate".to_owned()),
                ("Sec-Fetch-Dest", "document".to_owned()),
            ],
            None,
        )
    }

    /// A same-origin subresource load.
    pub(crate) fn subresource(&self, path: &str) -> Resp {
        self.exchange(
            "GET",
            path,
            &[("Sec-Fetch-Site", "same-origin".to_owned())],
            None,
        )
    }

    /// A same-origin API call without a session.
    pub(crate) fn api(&self, path: &str, body: Option<&str>) -> Resp {
        self.exchange(
            "POST",
            path,
            &[
                ("Origin", self.origin()),
                ("Sec-Fetch-Site", "same-origin".to_owned()),
            ],
            body,
        )
    }

    /// A request with arbitrary extra headers and no browser defaults.
    pub(crate) fn bare(&self, method: &str, path: &str, headers: &[(&str, String)]) -> Resp {
        self.exchange(method, path, headers, None)
    }

    /// Exchanges a launch token for the session pair.
    pub(crate) fn bootstrap(&self, token: &str) -> Result<Session, Resp> {
        let response = self.api(BOOTSTRAP, Some(&format!(r#"{{"token":"{token}"}}"#)));
        if response.status != 200 {
            return Err(response);
        }
        let set_cookie = response.header("set-cookie").expect("Set-Cookie");
        assert!(
            set_cookie.ends_with("; Secure; HttpOnly; SameSite=Strict; Path=/"),
            "cookie attributes"
        );
        let cookie = set_cookie
            .strip_prefix("__Host-pfp=")
            .and_then(|v| v.split(';').next())
            .expect("session cookie")
            .to_owned();
        let proof = response.json()["proof"].as_str().expect("proof").to_owned();
        Ok(Session { cookie, proof })
    }

    /// An authenticated API call.
    pub(crate) fn authed(&self, session: &Session, path: &str, body: Option<&str>) -> Resp {
        self.exchange(
            "POST",
            path,
            &[
                ("Origin", self.origin()),
                ("Sec-Fetch-Site", "same-origin".to_owned()),
                ("Cookie", format!("__Host-pfp={}", session.cookie)),
                ("X-PFP-Proof", session.proof.clone()),
            ],
            body,
        )
    }

    /// Plaintext HTTP to the TLS port: whatever comes back, as raw bytes.
    pub(crate) fn plaintext_get(&self) -> Vec<u8> {
        let addr = SocketAddrV4::new(Ipv4Addr::LOCALHOST, self.port);
        let port = self.port;
        self.runtime.block_on(async move {
            let mut tcp = TcpStream::connect(addr).await.expect("connect");
            let request =
                format!("GET / HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n");
            tcp.write_all(request.as_bytes()).await.expect("write");
            let mut response = Vec::new();
            let _ = tokio::time::timeout(SOON, tcp.read_to_end(&mut response)).await;
            response
        })
    }
}

/// `SECURITY.md` §7.2–§7.3 as universal negatives plus the complete policy set, on
/// **every** response this client receives.
fn assert_universal_invariants(method: &str, path: &str, response: &Resp) {
    let names: Vec<&str> = response.headers.iter().map(|(n, _)| n.as_str()).collect();
    assert!(
        !names.iter().any(|n| n.starts_with("access-control-")),
        "{method} {path}: CORS header"
    );
    assert!(!names.contains(&"location"), "{method} {path}: Location");
    let cookies = names.iter().filter(|n| **n == "set-cookie").count();
    let bootstrap_ok = method == "POST" && path == BOOTSTRAP && response.status == 200;
    assert_eq!(
        cookies,
        usize::from(bootstrap_ok),
        "{method} {path}: Set-Cookie count"
    );
    for forbidden in [
        "server",
        "vary",
        "strict-transport-security",
        "cross-origin-embedder-policy",
    ] {
        assert!(!names.contains(&forbidden), "{method} {path}: {forbidden}");
    }
    for (name, value) in POLICY_HEADERS {
        assert_eq!(response.all(name), [value], "{method} {path}: {name}");
    }
    let immutable = path.starts_with("/assets/") && matches!(response.status, 200 | 304);
    assert_eq!(
        response.all("cache-control"),
        [if immutable { IMMUTABLE } else { NO_STORE }],
        "{method} {path} {}",
        response.status
    );
}

/// Whether `text` holds a run of 64 or more hex digits of either case: the shape
/// of every token this application mints. (The certificate fingerprint is
/// colon-separated on purpose, so it cannot match.)
pub(crate) fn has_token_shaped_run(text: &str) -> bool {
    let mut run = 0_usize;
    for byte in text.bytes() {
        run = if byte.is_ascii_hexdigit() { run + 1 } else { 0 };
        if run >= 64 {
            return true;
        }
    }
    false
}
