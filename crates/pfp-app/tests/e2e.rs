//! End to end through the launcher: the launch sequence (`pfp_app::launch::serve`,
//! the function the binary runs) → TLS → the session handshake → the API → the
//! headers → graceful shutdown.
//!
//! It runs **in-process**, with the test standing in for the browser opener. That
//! is not a shortcut: by design the launch token reaches a browser only through an
//! in-process API call and is never printed, passed in argv or the environment, or
//! written to a file (`SECURITY.md` §7.1), so a separately spawned `pfp` cannot be
//! handed a session by a test — `tests/serve.rs` asserts exactly that against the
//! real binary, along with everything that does not need a session. The opener
//! here is an in-memory recorder; no browser, no trust store, `127.0.0.1` on an
//! OS-assigned port, a temporary state directory.

// Not an engine crate: spawning the built binary, sockets and temp files are the point.
#![forbid(unsafe_code)]
#![allow(
    clippy::disallowed_types,
    clippy::disallowed_methods,
    clippy::disallowed_macros
)]

mod support;

use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use pfp_app::cli::{PortChoice, ServeArgs};
use pfp_app::launch::{serve, Shutdown};
use pfp_app::platform::{BrowserOpener, Platform, PlatformError, UnsupportedPlatform};
use pfp_server::Redacted;

/// The test's "browser": remembers the URL it was asked to open, in memory.
#[derive(Default)]
struct CapturingOpener(Mutex<Vec<String>>);

impl BrowserOpener for CapturingOpener {
    fn available(&self) -> bool {
        true
    }
    fn open(&self, url: &Redacted<String>) -> Result<(), PlatformError> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(url.expose().clone());
        Ok(())
    }
}

#[derive(Clone, Default)]
struct Shared(Arc<Mutex<Vec<u8>>>);

impl Write for Shared {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl Shared {
    fn text(&self) -> String {
        String::from_utf8_lossy(&self.0.lock().unwrap_or_else(PoisonError::into_inner)).into_owned()
    }
}

struct Launched {
    out: Shared,
    err: Shared,
    opener: Arc<CapturingOpener>,
    stop: Option<tokio::sync::oneshot::Sender<()>>,
    thread: Option<std::thread::JoinHandle<u8>>,
    state_dir: PathBuf,
    port: u16,
}

impl Launched {
    fn start() -> Self {
        let state_dir = support::fresh_temp_dir("e2e");
        let opener = Arc::new(CapturingOpener::default());
        let platform = Platform {
            trust: Box::new(UnsupportedPlatform),
            alerter: Box::new(UnsupportedPlatform),
            opener: Arc::clone(&opener) as Arc<dyn BrowserOpener>,
        };
        let args = ServeArgs {
            no_trust: true,
            port: PortChoice::OsAssigned,
            state_dir: Some(state_dir.clone()),
            ..ServeArgs::default()
        };
        let (out, err) = (Shared::default(), Shared::default());
        let (stop, stopped) = tokio::sync::oneshot::channel();
        let thread = {
            let (mut out, mut err) = (out.clone(), err.clone());
            std::thread::spawn(move || {
                serve(
                    &args,
                    &platform,
                    None,
                    &mut out,
                    &mut err,
                    Shutdown::Channel(stopped),
                )
            })
        };
        let deadline = Instant::now() + support::SOON;
        while !out.text().contains("Press Ctrl-C") {
            assert!(!thread.is_finished(), "serve ended early: {}", err.text());
            assert!(Instant::now() < deadline, "not ready");
            std::thread::sleep(Duration::from_millis(5));
        }
        let port = out
            .text()
            .split("origin=https://127.0.0.1:")
            .nth(1)
            .and_then(|rest| rest.split(' ').next().map(str::to_owned))
            .and_then(|port| port.parse().ok())
            .expect("port in the ready line");
        Self {
            out,
            err,
            opener,
            stop: Some(stop),
            thread: Some(thread),
            state_dir,
            port,
        }
    }

    fn client(&self) -> support::Client {
        let ca = std::fs::read(self.state_dir.join("tls/ca-cert.der")).expect("CA certificate");
        support::Client::new(self.port, &ca.into())
    }

    /// The token from the URL the "browser" was handed.
    fn launch_token(&self) -> String {
        let opened = self.opener.0.lock().unwrap_or_else(PoisonError::into_inner);
        let url = opened.last().expect("the opener was called");
        let (origin, token) = url.split_once("/#t=").expect("token in the fragment");
        assert_eq!(origin, format!("https://127.0.0.1:{}", self.port));
        assert!(!url.contains('?'), "never the query string");
        token.to_owned()
    }

    fn shutdown(mut self) -> u8 {
        let _ = self.stop.take().expect("stop").send(());
        self.thread
            .take()
            .expect("thread")
            .join()
            .expect("serve thread")
    }
}

impl Drop for Launched {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        let _ = std::fs::remove_dir_all(&self.state_dir);
    }
}

fn grid_points() -> Vec<(i64, i64)> {
    // Expected values come from the synthetic fixture, not from the engine.
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/pending/schedule/mfj-2026-plan-acceptance.json");
    let fixture: serde_json::Value =
        serde_json::from_slice(&std::fs::read(path).expect("fixture")).expect("JSON");
    let cases = fixture["expect"]["cases"].as_array().expect("cases");
    let points: Vec<(i64, i64)> = cases
        .iter()
        .map(|case| {
            assert_eq!(case["inputs"]["filingStatus"], "mfj");
            assert_eq!(case["inputs"]["year"], 2026);
            (
                case["inputs"]["taxableIncomeCents"]
                    .as_i64()
                    .expect("income"),
                case["expectTaxCents"].as_i64().expect("tax"),
            )
        })
        .collect();
    assert_eq!(points.len(), 3);
    points
}

#[test]
fn handshake_rate_schedule_registry_headers_shutdown() {
    let launched = Launched::start();
    assert!(launched
        .out
        .text()
        .contains("trust=declined open=requested"));
    let client = launched.client();

    // The shell first, as a browser would.
    assert_eq!(client.navigate("/").status, 200);
    // Before the handshake the API is closed.
    assert_eq!(client.api("/api/v1/session/status", None).status, 401);

    // The handshake: launch token -> cookie + proof. Single use.
    let token = launched.launch_token();
    let session = client.bootstrap(&token).expect("bootstrap");
    assert_eq!(
        client.bootstrap(&token).err().expect("single use").status,
        401
    );

    let status = client.authed(&session, "/api/v1/session/status", None);
    assert_eq!(status.status, 200);
    assert_eq!(status.json()["trustMode"], "declined");
    assert_eq!(status.json()["appVersion"], env!("CARGO_PKG_VERSION"));

    // The rate schedule for a synthetic input, against the fixture grid points.
    for (income, expected_tax) in grid_points() {
        let body = format!(r#"{{"year":2026,"filingStatus":"mfj","taxableIncome":{income}}}"#);
        let response = client.authed(&session, "/api/v1/tax/rate-schedule", Some(&body));
        assert_eq!(response.status, 200);
        assert_eq!(response.header("cache-control"), Some(support::NO_STORE));
        let json = response.json();
        assert_eq!(json["tax"], expected_tax, "income {income}");
        // The Line tree: the root is in the list and every input resolves.
        let lines = json["lines"].as_array().expect("lines");
        let ids: Vec<&str> = lines
            .iter()
            .map(|l| l["id"].as_str().expect("id"))
            .collect();
        let root = json["rootLineId"].as_str().expect("root");
        assert!(ids.contains(&root));
        let root_line = lines.iter().find(|l| l["id"] == root).unwrap();
        assert_eq!(root_line["value"], expected_tax);
        assert!(
            root_line["inputs"]
                .as_array()
                .is_some_and(|i| !i.is_empty()),
            "the root has inputs"
        );
        for line in lines {
            // Leaves omit `inputs`.
            for input in line["inputs"].as_array().into_iter().flatten() {
                assert!(
                    ids.contains(&input.as_str().unwrap()),
                    "dangling input {input}"
                );
            }
        }
    }

    // The Assumptions Registry: every parameter with source, as-of, vintage,
    // projection and rounding.
    let registry = client.authed(&session, "/api/v1/assumptions/list", None);
    assert_eq!(registry.status, 200);
    let vintages = registry.json()["vintages"]
        .as_array()
        .expect("vintages")
        .clone();
    assert!(!vintages.is_empty());
    let mut tables = 0;
    for vintage in &vintages {
        for table in vintage["tables"].as_array().expect("tables") {
            tables += 1;
            assert!(table["id"].is_string());
            assert!(table["asOf"].is_string(), "as-of date");
            assert_eq!(table["vintageId"], vintage["contentId"], "vintage id");
            assert_registry_spelling(table);
            assert!(table["projection"]["rule"].is_string(), "projection rule");
            let sources = table["sources"].as_array().expect("sources");
            assert!(!sources.is_empty(), "at least one source");
            assert!(sources[0]["url"].is_string() && sources[0]["sha256"].is_string());
        }
    }
    assert!(tables >= 2, "both shipped tables are listed");
    assert!(
        vintages
            .iter()
            .flat_map(|v| v["tables"].as_array().unwrap())
            .any(|t| t["rounding"].is_object()),
        "a rounding rule is listed"
    );

    // A cookie alone is useless; a proof alone is the distinguishable 409.
    // (Header exactness and the universal negatives were asserted by the client on
    // every response above.)
    let relaunch = client.authed(&session, "/api/v1/session/relaunch", None);
    assert_eq!(relaunch.status, 202, "the opener exists in this run");
    let second_token = launched.launch_token();
    assert_ne!(
        second_token, token,
        "a fresh token, delivered through the opener only"
    );
    assert!(!String::from_utf8_lossy(&relaunch.body).contains(&second_token));

    // Nothing token-shaped reached either stream.
    let (out, err) = (launched.out.clone(), launched.err.clone());
    assert_eq!(launched.shutdown(), 0);
    assert!(out.text().contains("Stopped."));
    assert!(!support::has_token_shaped_run(&out.text()));
    assert!(!support::has_token_shaped_run(&err.text()));
}

/// A registry entry spells its verification status as the parameter files do, and
/// says which unit it is served in.
fn assert_registry_spelling(table: &serde_json::Value) {
    assert_eq!(table["servedUnit"], "cents");
    assert!(
        matches!(
            table["verification"].as_str(),
            Some(
                "pending-hand-verification"
                    | "primary-source-confirmed"
                    | "hand-worked-reviewed"
                    | "unstated"
            )
        ),
        "{table}"
    );
}
