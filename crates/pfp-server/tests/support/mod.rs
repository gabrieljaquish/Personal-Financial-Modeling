//! In-process harness for the TLS listener tests.
//!
//! Everything here is generated at run time and lives in memory: the certificates
//! and keys are never written anywhere. The server binds `127.0.0.1` on an
//! OS-assigned port and is shut down (and awaited) by [`TestServer::stop`]. The
//! client is `tokio-rustls` with a root store holding exactly one CA; there is no
//! HTTP client and no outbound connection to anything but the harness's own port.

// Each integration test compiles this module and uses a different part of it.
#![allow(dead_code)]

use std::net::SocketAddrV4;
use std::sync::Arc;
use std::time::Duration;

use pfp_server::{
    bind_loopback, issue, server_config, AcceptLimits, AcceptStats, TlsListener, TlsMaterial,
};
use rustls::pki_types::{CertificateDer, ServerName};
use rustls::{ClientConfig, RootCertStore};
use time::OffsetDateTime;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;
use tokio_rustls::client::TlsStream;
use tokio_rustls::TlsConnector;

/// What the test handler writes once the handshake is done.
pub(crate) const GREETING: &[u8] = b"pfp-test: tls established\n";

// ---------------------------------------------------------------------------
// Harness timeouts.
//
// The rule: a harness wait that observes something the server does on a timer must
// be STRICTLY LONGER than that timer. Otherwise a stall surfaces as "the harness
// gave up" and hides what the server would have answered — a 408, say — which is
// the difference between "slow" and "wrong". One constant per class of wait; no
// bare duration at a call site.
// ---------------------------------------------------------------------------

/// Scheduling slack on top of a server-side timer, for a loaded CI runner.
pub(crate) const MARGIN: Duration = Duration::from_secs(5);

/// For waits on which **no production server timer bears**: in-process state
/// changes, an accept loop stopping, a refusal decided at `accept(2)` — or a timer
/// the test itself shortened to [`SHORT_TIMER`]. Never use it to await a response
/// from a server running production limits: use [`PAST_REQUEST_DEADLINE`].
pub(crate) const SOON: Duration = MARGIN;

/// What a test sets a server timer to (through `TestKnobs`) when it wants to see
/// the timer fire and waits [`SOON`] for it.
pub(crate) const SHORT_TIMER: Duration = Duration::from_millis(300);

/// How often a polling wait looks again.
pub(crate) const POLL_INTERVAL: Duration = Duration::from_millis(10);

/// "It took at least the timer": [`SHORT_TIMER`] less a tolerance, because the
/// test's own clock starts a little after the server's timer does.
pub(crate) const SHORT_TIMER_FLOOR: Duration = Duration::from_millis(250);

/// For a complete HTTP exchange against production limits. The slowest thing the
/// server can legitimately do is answer 408 at `REQUEST_DEADLINE` and then take
/// `LINGER_TIMEOUT` to close; a wait longer than both always ends with an observed
/// server response, never with a harness timeout.
pub(crate) const PAST_REQUEST_DEADLINE: Duration = Duration::from_secs(
    pfp_server::limits::REQUEST_DEADLINE.as_secs()
        + pfp_server::limits::LINGER_TIMEOUT.as_secs()
        + MARGIN.as_secs(),
);

/// For a plaintext peer of the TLS listener: the slowest legitimate outcome is
/// being dropped at `TLS_HANDSHAKE_TIMEOUT`.
pub(crate) const PAST_HANDSHAKE_TIMEOUT: Duration =
    Duration::from_secs(pfp_server::limits::TLS_HANDSHAKE_TIMEOUT.as_secs() + MARGIN.as_secs());

// The relations the constants above exist to keep, checked at compile time.
const _: () = {
    use pfp_server::limits::{
        HEADER_READ_TIMEOUT, LINGER_TIMEOUT, REQUEST_DEADLINE, TLS_HANDSHAKE_TIMEOUT,
    };
    assert!(
        PAST_REQUEST_DEADLINE.as_millis()
            > (REQUEST_DEADLINE.as_millis() + LINGER_TIMEOUT.as_millis())
    );
    assert!(PAST_REQUEST_DEADLINE.as_millis() > HEADER_READ_TIMEOUT.as_millis());
    assert!(PAST_HANDSHAKE_TIMEOUT.as_millis() > TLS_HANDSHAKE_TIMEOUT.as_millis());
    // A shortened timer, and the linger that may follow it, fit well inside SOON.
    assert!(SHORT_TIMER.as_millis() + LINGER_TIMEOUT.as_millis() < SOON.as_millis());
    assert!(SHORT_TIMER_FLOOR.as_millis() < SHORT_TIMER.as_millis());
};

pub(crate) struct TestServer {
    pub(crate) addr: SocketAddrV4,
    pub(crate) material: TlsMaterial,
    pub(crate) stats: Arc<AcceptStats>,
    shutdown: oneshot::Sender<()>,
    task: JoinHandle<()>,
}

impl TestServer {
    /// Must be called inside a Tokio runtime: the accept loop is spawned onto it.
    pub(crate) fn start() -> Self {
        Self::start_with(AcceptLimits::default())
    }

    pub(crate) fn start_with(limits: AcceptLimits) -> Self {
        let material = issue(OffsetDateTime::now_utc()).expect("issue");
        let tls = server_config(&material).expect("server config");
        let bound = bind_loopback(None).expect("bind");
        let listener = TlsListener::new(bound, tls, limits).expect("listener");
        let addr = listener.local_addr();
        let stats = listener.stats();
        let (shutdown, stopped) = oneshot::channel::<()>();
        let task = tokio::spawn(listener.serve(
            |mut stream| async move {
                // Greet, then hold the connection until the client closes it.
                if stream.write_all(GREETING).await.is_ok() {
                    let mut sink = [0_u8; 256];
                    while matches!(stream.read(&mut sink).await, Ok(n) if n > 0) {}
                }
                let _ = stream.shutdown().await;
            },
            async move {
                let _ = stopped.await;
            },
        ));
        Self {
            addr,
            material,
            stats,
            shutdown,
            task,
        }
    }

    /// Stops the accept loop and waits for it; afterwards the port is closed.
    pub(crate) async fn stop(self) -> SocketAddrV4 {
        let _ = self.shutdown.send(());
        tokio::time::timeout(SOON, self.task)
            .await
            .expect("accept loop stops promptly")
            .expect("accept loop does not panic");
        self.addr
    }
}

/// A client configuration whose root store holds exactly `ca` — no platform roots,
/// no other anchor.
pub(crate) fn client_trusting(ca: &CertificateDer<'static>) -> Arc<ClientConfig> {
    let mut roots = RootCertStore::empty();
    roots.add(ca.clone()).expect("CA is a usable trust anchor");
    assert_eq!(roots.len(), 1);
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let mut config = ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .expect("protocol versions")
        .with_root_certificates(roots)
        .with_no_client_auth();
    config.alpn_protocols = vec![b"http/1.1".to_vec()];
    Arc::new(config)
}

/// A client configuration with an empty root store: it can complete no handshake,
/// which is all the connection-cap test needs of it.
pub(crate) fn client_trusting_nothing() -> Arc<ClientConfig> {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    Arc::new(
        ClientConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()
            .expect("protocol versions")
            .with_root_certificates(RootCertStore::empty())
            .with_no_client_auth(),
    )
}

/// TCP-connects to `addr` and attempts a TLS handshake for `server_name`.
pub(crate) async fn tls_connect(
    addr: SocketAddrV4,
    config: Arc<ClientConfig>,
    server_name: &str,
) -> std::io::Result<TlsStream<TcpStream>> {
    let tcp = TcpStream::connect(addr).await?;
    let name = ServerName::try_from(server_name.to_owned()).expect("server name");
    TlsConnector::from(config).connect(name, tcp).await
}

/// Reads the handler's greeting from an established connection.
pub(crate) async fn read_greeting(stream: &mut TlsStream<TcpStream>) -> Vec<u8> {
    let mut buffer = vec![0_u8; GREETING.len()];
    tokio::time::timeout(SOON, stream.read_exact(&mut buffer))
        .await
        .expect("greeting arrives promptly")
        .expect("greeting is readable");
    buffer
}

/// Writes `request` to a fresh plaintext TCP connection and returns every byte the
/// server sends until it closes the connection.
pub(crate) async fn raw_exchange(addr: SocketAddrV4, request: &[u8]) -> Vec<u8> {
    let mut tcp = TcpStream::connect(addr).await.expect("tcp connect");
    if !request.is_empty() {
        tcp.write_all(request).await.expect("write");
    }
    let mut received = Vec::new();
    // A reset counts as "closed": whatever arrived before it is what we inspect.
    let _ = tokio::time::timeout(PAST_HANDSHAKE_TIMEOUT, tcp.read_to_end(&mut received))
        .await
        .expect("the server closes the connection promptly");
    received
}

/// Polls `condition` until it holds or [`SOON`] elapses.
pub(crate) async fn eventually(what: &str, mut condition: impl FnMut() -> bool) {
    let deadline = tokio::time::Instant::now() + SOON;
    while !condition() {
        assert!(tokio::time::Instant::now() < deadline, "timed out: {what}");
        tokio::time::sleep(POLL_INTERVAL).await;
    }
}

// ---------------------------------------------------------------------------
// The HTTP harness: the whole server in-process, and a raw HTTP/1.1 client.
//
// There is deliberately no HTTP client crate (ADR-020). The admission matrix needs
// byte-level control — a missing `Host`, two `Host` headers, an absent
// `Sec-Fetch-Site` — which a well-behaved client refuses to send, and every
// successful request is also a full webpki validation of the certificate scheme.
// ---------------------------------------------------------------------------

use pfp_server::{
    AssetManifest, EventLog, RelaunchHook, Server, ServerConfig, SessionManager, TestKnobs,
};

/// The policy headers, typed from `SECURITY.md` §7.3 independently of the crate's
/// own constants. Every response the service produces must carry exactly these.
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

pub(crate) struct HttpOptions {
    pub(crate) assets: AssetManifest,
    pub(crate) relaunch: Option<Arc<dyn RelaunchHook>>,
    pub(crate) knobs: TestKnobs,
}

impl Default for HttpOptions {
    fn default() -> Self {
        Self {
            assets: AssetManifest::placeholder(),
            relaunch: None,
            knobs: TestKnobs {
                echo_events_to_stderr: false,
                ..TestKnobs::default()
            },
        }
    }
}

pub(crate) struct HttpServer {
    pub(crate) addr: SocketAddrV4,
    /// `127.0.0.1:<port>`
    pub(crate) host: String,
    /// `https://127.0.0.1:<port>`
    pub(crate) origin: String,
    pub(crate) events: Arc<EventLog>,
    pub(crate) sessions: Arc<SessionManager>,
    pub(crate) stats: Arc<AcceptStats>,
    client: Arc<ClientConfig>,
    shutdown: oneshot::Sender<()>,
    task: JoinHandle<()>,
}

impl HttpServer {
    pub(crate) fn start() -> Self {
        Self::start_with(HttpOptions::default())
    }

    pub(crate) fn start_with(options: HttpOptions) -> Self {
        let material = issue(OffsetDateTime::now_utc()).expect("issue");
        let client = client_trusting(&material.ca_cert_der);
        let mut config = ServerConfig::new(material);
        config.assets = options.assets;
        config.relaunch = options.relaunch;
        config.test = options.knobs;
        let bound = Server::bind(config).expect("bind");
        let host = bound.origin().host_header().to_owned();
        let origin = bound.origin().origin().to_owned();
        let port = bound.origin().port();
        let (events, sessions, stats) = (bound.events(), bound.sessions(), bound.stats());
        let (shutdown, stopped) = oneshot::channel::<()>();
        let task = tokio::spawn(bound.serve(async move {
            let _ = stopped.await;
        }));
        Self {
            addr: SocketAddrV4::new(std::net::Ipv4Addr::LOCALHOST, port),
            host,
            origin,
            events,
            sessions,
            stats,
            client,
            shutdown,
            task,
        }
    }

    pub(crate) async fn stop(self) {
        let _ = self.shutdown.send(());
        tokio::time::timeout(SOON, self.task)
            .await
            .expect("server stops promptly")
            .expect("server does not panic");
        assert!(
            TcpStream::connect(self.addr).await.is_err(),
            "the port is closed after stop"
        );
    }

    /// The client configuration that trusts only this server's CA.
    pub(crate) fn client(&self) -> Arc<ClientConfig> {
        self.client.clone()
    }

    /// A TLS connection that trusts only this server's CA.
    pub(crate) async fn connect(&self) -> TlsStream<TcpStream> {
        tls_connect(self.addr, self.client.clone(), "127.0.0.1")
            .await
            .expect("tls connect")
    }

    /// What the front end's `fetch` sends (`web/src/session/api.ts`): `mode:
    /// 'same-origin'` and `referrerPolicy: 'strict-origin'`, so the `Referer` is
    /// the origin alone and the `Origin` is real. Recorded from Firefox and Chrome.
    pub(crate) fn api(&self, path: &str) -> Req {
        Req::new("POST", path)
            .header("Host", &self.host)
            .header("Origin", &self.origin)
            .header("Referer", &format!("{}/", self.origin))
            .header("Sec-Fetch-Site", "same-origin")
            .header("Sec-Fetch-Mode", "same-origin")
            .header("Sec-Fetch-Dest", "empty")
    }

    /// The same request as the front end sent it before `referrerPolicy:
    /// 'strict-origin'`: a same-origin POST under the document's `no-referrer`
    /// policy, which Fetch serialises with `Origin: null` and Firefox and Safari
    /// send exactly so (`SECURITY.md` §7.2). Recorded from Firefox 155.
    pub(crate) fn api_under_no_referrer(&self, path: &str) -> Req {
        Req::new("POST", path)
            .header("Host", &self.host)
            .header("Origin", "null")
            .header("Sec-Fetch-Site", "same-origin")
            .header("Sec-Fetch-Mode", "same-origin")
            .header("Sec-Fetch-Dest", "empty")
    }

    /// What a browser sends when the user opens the URL: a top-level navigation.
    pub(crate) fn navigate(&self, path: &str) -> Req {
        Req::new("GET", path)
            .header("Host", &self.host)
            .header("Sec-Fetch-Site", "none")
            .header("Sec-Fetch-Mode", "navigate")
            .header("Sec-Fetch-Dest", "document")
    }

    /// What the loaded document sends for a subresource.
    pub(crate) fn subresource(&self, path: &str) -> Req {
        Req::new("GET", path)
            .header("Host", &self.host)
            .header("Sec-Fetch-Site", "same-origin")
            .header("Sec-Fetch-Mode", "no-cors")
            .header("Sec-Fetch-Dest", "script")
    }

    pub(crate) async fn send(&self, request: Req) -> Resp {
        let bytes = request.to_bytes();
        let response = self.exchange(&bytes).await;
        assert_universal_invariants(&request.method, &request.path(), &response, true);
        response
    }

    /// For input that is not well-formed HTTP: hyper answers it below every layer,
    /// so the policy set is not expected — every other universal negative is.
    /// `None`: the server closed the connection without writing a response.
    pub(crate) async fn send_protocol_level(&self, bytes: &[u8]) -> Option<Resp> {
        let received = self.exchange_raw(bytes).await;
        if received.is_empty() {
            return None;
        }
        let response = Resp::parse(&received);
        assert_universal_invariants("?", "?", &response, false);
        Some(response)
    }

    async fn exchange(&self, bytes: &[u8]) -> Resp {
        Resp::parse(&self.exchange_raw(bytes).await)
    }

    /// A client that writes its whole request and only then reads, as simple HTTP
    /// clients do.
    async fn exchange_raw(&self, bytes: &[u8]) -> Vec<u8> {
        let mut stream = self.connect().await;
        // `write_all` on a TLS stream returns once rustls has *buffered* the
        // plaintext; up to one buffer (64 KiB) of it may not have reached the
        // socket, and a read does not push it out. Without the `flush` the server
        // can be left waiting — correctly — for the tail of a body that never
        // leaves this process, and answers 408 at its deadline. (That was the
        // intermittent failure of `chunked_body_over_the_cap_is_413`.)
        //
        // Both results are ignored on purpose: the server may refuse an oversized
        // request and stop reading before it is fully written. What it sent is
        // still what we inspect.
        let _ = stream.write_all(bytes).await;
        let _ = stream.flush().await;
        let mut received = Vec::new();
        let finished =
            tokio::time::timeout(PAST_REQUEST_DEADLINE, stream.read_to_end(&mut received)).await;
        assert!(
            finished.is_ok(),
            "the server did not close the connection; it had sent {} bytes: {:?}",
            received.len(),
            String::from_utf8_lossy(&received[..received.len().min(40)])
        );
        received
    }

    /// Bootstraps a session and returns `(cookie, proof)`. The values are secrets
    /// of this test run: never print them.
    pub(crate) async fn establish(&self) -> (String, String) {
        let token = self.sessions.mint_launch_token().expect("mint");
        let response = self
            .send(
                self.api(BOOTSTRAP)
                    .json(&format!(r#"{{"token":"{}"}}"#, token.expose())),
            )
            .await;
        assert_eq!(response.status, 200);
        let cookie = response
            .header("set-cookie")
            .and_then(|v| v.strip_prefix("__Host-pfp="))
            .and_then(|v| v.split(';').next())
            .expect("session cookie")
            .to_owned();
        let proof = response.json()["proof"].as_str().expect("proof").to_owned();
        (cookie, proof)
    }

    /// An authenticated API request.
    pub(crate) fn authed(&self, path: &str, session: &(String, String)) -> Req {
        self.api(path)
            .header("Cookie", &format!("__Host-pfp={}", session.0))
            .header("X-PFP-Proof", &session.1)
    }
}

#[derive(Clone)]
pub(crate) struct Req {
    pub(crate) method: String,
    target: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl Req {
    pub(crate) fn new(method: &str, target: &str) -> Self {
        Self {
            method: method.to_owned(),
            target: target.to_owned(),
            headers: Vec::new(),
            body: Vec::new(),
        }
    }

    /// Appends a header; a second call with the same name sends it twice.
    pub(crate) fn header(mut self, name: &str, value: &str) -> Self {
        self.headers.push((name.to_owned(), value.to_owned()));
        self
    }

    /// Removes every header of that name.
    pub(crate) fn without(mut self, name: &str) -> Self {
        self.headers.retain(|(n, _)| !n.eq_ignore_ascii_case(name));
        self
    }

    /// Replaces every header of that name with one value.
    pub(crate) fn set(self, name: &str, value: &str) -> Self {
        self.without(name).header(name, value)
    }

    pub(crate) fn method(mut self, method: &str) -> Self {
        method.clone_into(&mut self.method);
        self
    }

    pub(crate) fn target(mut self, target: &str) -> Self {
        target.clone_into(&mut self.target);
        self
    }

    pub(crate) fn json(self, body: &str) -> Self {
        self.body(body.as_bytes())
            .header("Content-Type", "application/json")
    }

    pub(crate) fn body(mut self, body: &[u8]) -> Self {
        self.body = body.to_vec();
        self
    }

    fn path(&self) -> String {
        let after_authority = self
            .target
            .strip_prefix("https://")
            .map_or(self.target.as_str(), |rest| {
                rest.find('/').map_or("/", |at| &rest[at..])
            });
        after_authority
            .split('?')
            .next()
            .unwrap_or_default()
            .to_owned()
    }

    pub(crate) fn to_bytes(&self) -> Vec<u8> {
        let mut out = format!("{} {} HTTP/1.1\r\n", self.method, self.target).into_bytes();
        for (name, value) in &self.headers {
            out.extend_from_slice(format!("{name}: {value}\r\n").as_bytes());
        }
        let explicit_length = self.headers.iter().any(|(n, _)| {
            n.eq_ignore_ascii_case("content-length") || n.eq_ignore_ascii_case("transfer-encoding")
        });
        if !explicit_length && (!self.body.is_empty() || self.method == "POST") {
            out.extend_from_slice(format!("Content-Length: {}\r\n", self.body.len()).as_bytes());
        }
        out.extend_from_slice(b"Connection: close\r\n\r\n");
        out.extend_from_slice(&self.body);
        out
    }
}

pub(crate) struct Resp {
    pub(crate) status: u16,
    /// Lower-cased names, in the order sent, duplicates preserved.
    pub(crate) headers: Vec<(String, String)>,
    pub(crate) body: Vec<u8>,
    /// Everything the server sent, for the rare test that needs the bytes.
    pub(crate) raw_len: usize,
}

impl Resp {
    pub(crate) fn parse(bytes: &[u8]) -> Self {
        let split = bytes
            .windows(4)
            .position(|w| w == b"\r\n\r\n")
            .unwrap_or_else(|| panic!("no HTTP response head in {} bytes", bytes.len()));
        let head = std::str::from_utf8(&bytes[..split]).expect("ascii head");
        let mut lines = head.split("\r\n");
        let status_line = lines.next().expect("status line");
        assert!(status_line.starts_with("HTTP/1.1 "), "status line");
        let status = status_line[9..12].parse().expect("status code");
        let headers = lines
            .map(|line| {
                let (name, value) = line.split_once(':').expect("header line");
                (name.to_ascii_lowercase(), value.trim().to_owned())
            })
            .collect();
        Self {
            status,
            headers,
            body: bytes[split + 4..].to_vec(),
            raw_len: bytes.len(),
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
        let all = self.all(name);
        assert!(all.len() <= 1, "{name} sent {} times", all.len());
        all.first().copied()
    }

    pub(crate) fn json(&self) -> serde_json::Value {
        serde_json::from_slice(&self.body).expect("JSON body")
    }

    /// The `code` of a refusal body.
    pub(crate) fn code(&self) -> String {
        self.json()["code"].as_str().expect("code").to_owned()
    }

    /// The header list with run-specific values normalised, for snapshots. The
    /// cookie value is a secret and the date is the clock.
    pub(crate) fn header_snapshot(&self) -> String {
        use std::fmt::Write as _;
        let mut out = format!("status: {}\n", self.status);
        for (name, value) in &self.headers {
            let value = match name.as_str() {
                "date" => "<date>".to_owned(),
                "etag" => "\"<sha256>\"".to_owned(),
                "content-length" if self.status == 200 => "<length>".to_owned(),
                "set-cookie" => {
                    let (pair, attributes) = value.split_once(';').unwrap_or((value, ""));
                    let (cookie_name, secret) = pair.split_once('=').unwrap_or((pair, ""));
                    format!("{cookie_name}=<{} chars>;{attributes}", secret.len())
                }
                _ => value.clone(),
            };
            let _ = writeln!(out, "{name}: {value}");
        }
        out
    }
}

/// `SECURITY.md` §7.2–§7.3 as universal negatives, run on **every** response the
/// client receives: a snapshot per response class cannot express "never".
fn assert_universal_invariants(method: &str, path: &str, response: &Resp, service_level: bool) {
    let names: Vec<&str> = response.headers.iter().map(|(n, _)| n.as_str()).collect();
    // 1. No CORS, no Private Network Access — under any condition.
    assert!(
        !names.iter().any(|n| n.starts_with("access-control-")),
        "{method} {path}: CORS header in {names:?}"
    );
    // 2. No redirect of any kind.
    assert!(!names.contains(&"location"), "{method} {path}: Location");
    // 3. Set-Cookie iff this is a successful bootstrap, and then exactly one.
    let cookies = names.iter().filter(|n| **n == "set-cookie").count();
    let bootstrap_ok = method == "POST" && path == BOOTSTRAP && response.status == 200;
    assert_eq!(
        cookies,
        usize::from(bootstrap_ok),
        "{method} {path} {}: Set-Cookie count",
        response.status
    );
    // 4. Headers the design excludes.
    for forbidden in [
        "server",
        "vary",
        "strict-transport-security",
        "cross-origin-embedder-policy",
    ] {
        assert!(!names.contains(&forbidden), "{method} {path}: {forbidden}");
    }
    if !service_level {
        return;
    }
    // 5. The complete policy set, exact values, each exactly once.
    for (name, value) in POLICY_HEADERS {
        assert_eq!(response.all(name), [value], "{method} {path}: {name}");
    }
    let cache = response.all("cache-control");
    let immutable_allowed =
        path.starts_with("/assets/") && (response.status == 200 || response.status == 304);
    if immutable_allowed {
        assert_eq!(cache, [IMMUTABLE], "{method} {path}");
    } else {
        assert_eq!(cache, [NO_STORE], "{method} {path} {}", response.status);
    }
    // A refusal is JSON with a code and a fixed message, and nothing else.
    if response.status >= 400 && method != "HEAD" {
        let body = response.json();
        let object = body.as_object().expect("refusal body is an object");
        let mut keys: Vec<&str> = object.keys().map(String::as_str).collect();
        keys.sort_unstable();
        assert_eq!(keys, ["code", "message"], "{method} {path}");
    }
}

/// A web bundle shaped like Vite's output, built in memory.
pub(crate) fn vite_like_bundle() -> AssetManifest {
    let index = "<!doctype html>\n<html lang=\"en\">\n  <head>\n    <meta charset=\"UTF-8\" />\n    <title>Personal Financial Modeling</title>\n    <script type=\"module\" crossorigin src=\"/assets/index-AbCd1234.js\"></script>\n    <link rel=\"stylesheet\" crossorigin href=\"/assets/style-WxYz5678.css\">\n  </head>\n  <body>\n    <div id=\"root\"></div>\n  </body>\n</html>\n";
    AssetManifest::from_files([
        ("index.html".to_owned(), index.as_bytes().to_vec()),
        (
            "assets/index-AbCd1234.js".to_owned(),
            b"const ns='http://www.w3.org/2000/svg';export{ns};".to_vec(),
        ),
        (
            "assets/style-WxYz5678.css".to_owned(),
            b"body{margin:0}".to_vec(),
        ),
    ])
    .expect("bundle")
}
