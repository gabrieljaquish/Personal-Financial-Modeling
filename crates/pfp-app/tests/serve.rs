//! The real `pfp` binary, started only as
//! `pfp serve --port 0 --no-open --no-trust --state-dir <temp>` by the one spawn
//! helper: ready line, loopback-only TLS-only listener, the exact headers, refusals
//! without a session, the single-instance lock, graceful SIGTERM, cold start, and
//! no token-shaped string on either stream.
//!
//! A `--no-open` process **cannot be given a session from outside**: the launch
//! token is never printed, never in argv, the environment or a file
//! (`SECURITY.md` §7.1). That is asserted here; the full handshake against the
//! launch sequence runs in-process in `tests/e2e.rs`, where the test is the
//! browser opener.

// Not an engine crate: spawning the built binary, sockets and temp files are the point.
#![forbid(unsafe_code)]
#![allow(
    clippy::disallowed_types,
    clippy::disallowed_methods,
    clippy::disallowed_macros
)]

mod support;

use std::net::{Ipv4Addr, TcpStream};
use std::os::unix::fs::PermissionsExt;
use std::time::{Duration, Instant};

#[test]
fn ready_line_headers_refusals_and_graceful_sigterm() {
    let mut pfp = support::spawn_pfp();
    let ready = pfp.wait_ready().expect("ready");
    assert!(
        ready.line.ends_with("trust=declined open=skipped"),
        "{}",
        ready.line
    );
    assert!(ready.fingerprint.starts_with("SHA256:"));
    assert_eq!(
        ready
            .fingerprint
            .trim_start_matches("SHA256:")
            .split(':')
            .count(),
        32
    );

    let client = pfp.client();
    // The shell, over TLS validated against the one CA, with the exact header set
    // (asserted inside the client on every response).
    let document = client.navigate("/");
    assert_eq!(document.status, 200);
    assert_eq!(
        document.header("content-type"),
        Some("text/html; charset=utf-8")
    );
    let html = String::from_utf8(document.body).unwrap();
    assert!(html.contains("Personal Financial Modeling"));

    // Every asset the shell names is served immutable with a strong ETag.
    for path in html.split('"').filter(|s| s.starts_with("/assets/")) {
        let asset = client.subresource(path);
        assert_eq!(asset.status, 200, "{path}");
        assert!(asset.header("etag").is_some());
    }

    // Admission, from the real binary.
    assert_eq!(
        client
            .bare("GET", "/", &[("Sec-Fetch-Site", "cross-site".to_owned())])
            .status,
        403
    );
    assert_eq!(client.bare("GET", "/nope", &[]).status, 404);
    assert_eq!(
        client.bare("POST", "/api/v1/session/status", &[]).status,
        403
    );

    // No session can be obtained from outside the process.
    for path in [
        "/api/v1/session/status",
        "/api/v1/assumptions/list",
        "/api/v1/session/relaunch",
    ] {
        assert_eq!(client.api(path, None).status, 401, "{path}");
    }
    let guess = "0".repeat(63) + "1";
    let refused = client
        .bootstrap(&guess)
        .err()
        .expect("a guessed token is refused");
    assert_eq!(refused.status, 401);
    assert_eq!(refused.json()["code"], "launch_token_invalid");

    let status = pfp.terminate();
    assert_eq!(status.code(), Some(0), "SIGTERM is a graceful exit");
    assert!(pfp.stdout().contains("Stopped."));
}

#[test]
fn binary_refuses_plaintext() {
    let mut pfp = support::spawn_pfp();
    pfp.wait_ready().expect("ready");
    let bytes = pfp.client().plaintext_get();
    assert!(
        !bytes.starts_with(b"HTTP/"),
        "no HTTP response is ever written to a non-TLS socket"
    );
    assert!(bytes.len() <= 16, "at most a TLS alert record");
}

#[test]
fn listener_is_loopback_only() {
    let mut pfp = support::spawn_pfp();
    let ready = pfp.wait_ready().expect("ready");
    assert!(ready.origin.starts_with("https://127.0.0.1:"));
    // Reachable on 127.0.0.1 ...
    assert!(TcpStream::connect((Ipv4Addr::LOCALHOST, ready.port)).is_ok());
    // ... and not on the IPv6 loopback (`::1` is in the SANs but is not bound).
    assert!(TcpStream::connect_timeout(
        &(std::net::Ipv6Addr::LOCALHOST, ready.port).into(),
        Duration::from_secs(1)
    )
    .is_err());
}

#[test]
fn single_instance_lock() {
    let mut first = support::spawn_pfp();
    first.wait_ready().expect("ready");
    let mut second = support::spawn_pfp_sharing(first.state_dir());
    let status = second.wait_exit();
    assert_eq!(status.code(), Some(3));
    assert!(second.stderr().contains("already running"));
    assert!(!second.stdout().contains("PFP-READY"), "no second listener");

    // The lock goes with the process: after the first exits, the directory is free.
    assert_eq!(first.terminate().code(), Some(0));
    let mut third = support::spawn_pfp_sharing(first.state_dir());
    third
        .wait_ready()
        .expect("ready after the first instance exited");
}

#[test]
fn state_directory_is_private_and_holds_no_ca_key() {
    let mut pfp = support::spawn_pfp();
    pfp.wait_ready().expect("ready");
    let mode = |p: &std::path::Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode(pfp.state_dir()), 0o700);
    assert_eq!(mode(&pfp.state_dir().join("tls")), 0o700);
    assert_eq!(mode(&pfp.state_dir().join("tls/leaf-key.p8")), 0o600);
    let mut names: Vec<String> = std::fs::read_dir(pfp.state_dir().join("tls"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(names, ["ca-cert.der", "leaf-cert.der", "leaf-key.p8"]);
}

#[test]
fn leaf_is_reissued_when_the_key_file_was_loosened() {
    let mut first = support::spawn_pfp();
    let before = first.wait_ready().expect("ready").fingerprint;
    assert_eq!(first.terminate().code(), Some(0));

    let key = first.state_dir().join("tls/leaf-key.p8");
    std::fs::set_permissions(&key, std::fs::Permissions::from_mode(0o644)).unwrap();
    let mut second = support::spawn_pfp_sharing(first.state_dir());
    let after = second.wait_ready().expect("ready").fingerprint;
    assert_ne!(before, after, "a loosened key is never served again");
    assert_eq!(
        std::fs::metadata(&key).unwrap().permissions().mode() & 0o777,
        0o600
    );
}

#[test]
fn no_token_shaped_string_on_stdout_or_stderr() {
    let mut pfp = support::spawn_pfp();
    pfp.wait_ready().expect("ready");
    let client = pfp.client();
    // Served and refused requests, including ones that carry token-shaped values:
    // none may be echoed to either stream.
    let _ = client.navigate("/");
    let _ = client.api("/api/v1/session/status", None);
    let _ = client.bootstrap(&"ab".repeat(32));
    let _ = client.bare(
        "POST",
        "/api/v1/session/status",
        &[
            ("X-PFP-Proof", "cd".repeat(32)),
            ("Cookie", format!("__Host-pfp={}", "ef".repeat(32))),
        ],
    );
    let _ = client.bare("GET", &format!("/?t={}", "12".repeat(32)), &[]);
    for _ in 0..12 {
        let _ = client.bootstrap(&"ab".repeat(32));
    }
    assert_eq!(pfp.terminate().code(), Some(0));

    let (stdout, stderr) = (pfp.stdout(), pfp.stderr());
    assert!(stdout.contains("PFP-READY"));
    assert!(!support::has_token_shaped_run(&stdout), "stdout");
    assert!(!support::has_token_shaped_run(&stderr), "stderr");
    for needle in ["#t=", "abab", "cdcd", "efef", "1212"] {
        assert!(
            !stdout.contains(needle) && !stderr.contains(needle),
            "{needle}"
        );
    }
    // The matcher itself is not vacuous.
    assert!(support::has_token_shaped_run(&format!(
        "x {} y",
        "aB".repeat(32)
    )));
    assert!(!support::has_token_shaped_run(&"AB:".repeat(32)));
}

/// `PLAN.md` §4.1: cold start under two seconds. Measured here from process start
/// — fresh state directory, so key generation and certificate issuance are
/// included — to the first successful HTTPS response. Best of three, so a loaded
/// test machine does not make it flaky; the measurement is printed.
#[test]
fn cold_start_to_first_tls_response_under_2s() {
    let mut best = Duration::MAX;
    for _ in 0..3 {
        let mut pfp = support::spawn_pfp();
        pfp.wait_ready().expect("ready");
        let response = pfp.client().navigate("/");
        let elapsed = Instant::now().duration_since(pfp.started());
        assert_eq!(response.status, 200);
        best = best.min(elapsed);
        if best < Duration::from_secs(2) {
            break;
        }
    }
    eprintln!("cold start to first HTTPS 200: {} ms", best.as_millis());
    assert!(best < Duration::from_secs(2), "cold start took {best:?}");
}
