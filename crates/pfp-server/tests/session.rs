//! Session establishment over real TLS (`SECURITY.md` §7.1, test ids S-08, S-28).
//!
//! Secrets of a test run are never printed: comparisons use `assert!(a == b)`
//! with a fixed message, never `assert_eq!`, which would format both sides.

// Not an engine crate: tests here legitimately touch sockets, the clock and files.
#![allow(
    clippy::disallowed_types,
    clippy::disallowed_methods,
    clippy::disallowed_macros
)]

mod support;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use pfp_server::redact::Redacted;
use pfp_server::{Clock, EventCode, RelaunchHook, ReopenUnavailable, TestKnobs};
use support::{HttpOptions, HttpServer, BOOTSTRAP};

const STATUS: &str = "/api/v1/session/status";
const RELAUNCH: &str = "/api/v1/session/relaunch";

struct SteppedClock(Mutex<Instant>);

impl SteppedClock {
    fn new() -> Arc<Self> {
        Arc::new(Self(Mutex::new(Instant::now())))
    }
    fn advance(&self, by: Duration) {
        *self.0.lock().unwrap() += by;
    }
}

impl Clock for SteppedClock {
    fn now(&self) -> Instant {
        *self.0.lock().unwrap()
    }
}

fn with_clock(clock: Arc<SteppedClock>, relaunch: Option<Arc<dyn RelaunchHook>>) -> HttpServer {
    HttpServer::start_with(HttpOptions {
        relaunch,
        knobs: TestKnobs {
            clock,
            echo_events_to_stderr: false,
            ..TestKnobs::default()
        },
        ..Default::default()
    })
}

fn token_body(token: &str) -> String {
    format!(r#"{{"token":"{token}"}}"#)
}

fn is_lower_hex_64(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

#[tokio::test]
async fn launch_token_is_single_use() {
    let server = HttpServer::start();
    let token = server.sessions.mint_launch_token().unwrap();
    let first = server
        .send(server.api(BOOTSTRAP).json(&token_body(token.expose())))
        .await;
    assert_eq!(first.status, 200);
    let second = server
        .send(server.api(BOOTSTRAP).json(&token_body(token.expose())))
        .await;
    assert_eq!(
        (second.status, second.code().as_str()),
        (401, "launch_token_invalid")
    );
    server.stop().await;
}

#[tokio::test]
async fn launch_token_expires_after_60s() {
    let clock = SteppedClock::new();
    let server = with_clock(clock.clone(), None);

    let token = server.sessions.mint_launch_token().unwrap();
    clock.advance(Duration::from_secs(59));
    let live = server
        .send(server.api(BOOTSTRAP).json(&token_body(token.expose())))
        .await;
    assert_eq!(live.status, 200);

    let token = server.sessions.mint_launch_token().unwrap();
    clock.advance(Duration::from_secs(61));
    let expired = server
        .send(server.api(BOOTSTRAP).json(&token_body(token.expose())))
        .await;
    assert_eq!(
        (expired.status, expired.code().as_str()),
        (401, "launch_token_invalid")
    );
    server.stop().await;
}

#[tokio::test]
async fn wrong_token_does_not_consume_the_real_one() {
    let server = HttpServer::start();
    let token = server.sessions.mint_launch_token().unwrap();
    for wrong in [
        "0".repeat(64),
        "f".repeat(63),
        String::new(),
        "zz".repeat(32),
    ] {
        let response = server
            .send(server.api(BOOTSTRAP).json(&token_body(&wrong)))
            .await;
        assert_eq!(response.status, 401);
    }
    let real = server
        .send(server.api(BOOTSTRAP).json(&token_body(token.expose())))
        .await;
    assert_eq!(real.status, 200);
    assert_eq!(server.events.count(EventCode::BootstrapRejected), 4);
    server.stop().await;
}

#[tokio::test]
async fn failed_bootstraps_are_counted_not_refused() {
    let server = HttpServer::start();
    let token = server.sessions.mint_launch_token().unwrap();
    for _ in 0..12 {
        let response = server
            .send(server.api(BOOTSTRAP).json(&token_body(&"0".repeat(64))))
            .await;
        assert_eq!(
            response.status, 401,
            "never 429: a limiter here would lock the user out"
        );
    }
    assert_eq!(server.events.count(EventCode::BootstrapFailuresHigh), 1);
    let real = server
        .send(server.api(BOOTSTRAP).json(&token_body(token.expose())))
        .await;
    assert_eq!(real.status, 200);
    server.stop().await;
}

#[tokio::test]
async fn token_in_query_string_is_ignored() {
    let server = HttpServer::start();
    let token = server.sessions.mint_launch_token().unwrap();
    // No body: the query string is not a transport for the token.
    let in_query = server
        .send(server.api(&format!("{BOOTSTRAP}?token={}", token.expose())))
        .await;
    assert_eq!(
        (in_query.status, in_query.code().as_str()),
        (400, "invalid_json")
    );
    // …and it was not consumed by that attempt.
    let real = server
        .send(server.api(BOOTSTRAP).json(&token_body(token.expose())))
        .await;
    assert_eq!(real.status, 200);
    server.stop().await;
}

#[tokio::test]
async fn set_cookie_is_exactly_the_documented_string() {
    let server = HttpServer::start();
    let token = server.sessions.mint_launch_token().unwrap();
    let response = server
        .send(server.api(BOOTSTRAP).json(&token_body(token.expose())))
        .await;
    let set_cookie = response.header("set-cookie").unwrap();
    let (pair, attributes) = set_cookie.split_once(';').unwrap();
    assert_eq!(attributes, " Secure; HttpOnly; SameSite=Strict; Path=/");
    let (name, value) = pair.split_once('=').unwrap();
    assert_eq!(name, "__Host-pfp");
    assert!(
        is_lower_hex_64(value),
        "cookie is 64 lower-case hex characters"
    );

    let body = response.json();
    let keys: Vec<&String> = body.as_object().unwrap().keys().collect();
    assert_eq!(keys, ["proof"]);
    let proof = body["proof"].as_str().unwrap();
    assert!(
        is_lower_hex_64(proof),
        "proof is 64 lower-case hex characters"
    );
    assert!(proof != value, "cookie and proof are independent");
    assert!(proof != token.expose() && value != token.expose());
    server.stop().await;
}

#[tokio::test]
async fn both_factors_are_required() {
    let server = HttpServer::start();
    let session = server.establish().await;

    let both = server.send(server.authed(STATUS, &session)).await;
    assert_eq!(both.status, 200);

    // cookie_only_is_401
    let cookie_only = server
        .send(server.authed(STATUS, &session).without("X-PFP-Proof"))
        .await;
    assert_eq!(
        (cookie_only.status, cookie_only.code().as_str()),
        (401, "session_required")
    );
    // neither_is_401
    let neither = server.send(server.api(STATUS)).await;
    assert_eq!(neither.status, 401);
    // The cookie value presented as the proof is still a wrong proof.
    let swapped = server
        .send(
            server
                .authed(STATUS, &session)
                .set("X-PFP-Proof", &session.0),
        )
        .await;
    assert_eq!(swapped.status, 401);
    // Two proof headers are not "the first one".
    let doubled = server
        .send(
            server
                .authed(STATUS, &session)
                .header("X-PFP-Proof", &session.1),
        )
        .await;
    assert_eq!(doubled.status, 401);
    server.stop().await;
}

#[tokio::test]
async fn displaced_cookie_is_409_with_stable_code() {
    let server = HttpServer::start();
    let session = server.establish().await;
    // proof_only_is_409: the cookie was evicted.
    let evicted = server
        .send(server.authed(STATUS, &session).without("Cookie"))
        .await;
    assert_eq!(
        (evicted.status, evicted.code().as_str()),
        (409, "session_cookie_displaced")
    );
    // Overwritten by another loopback origin.
    let overwritten = server
        .send(
            server
                .authed(STATUS, &session)
                .set("Cookie", &format!("__Host-pfp={}", "0".repeat(64))),
        )
        .await;
    assert_eq!(overwritten.status, 409);
    // Other cookies in the shared 127.0.0.1 jar do not matter.
    let crowded = server
        .send(server.authed(STATUS, &session).set(
            "Cookie",
            &format!("other=1; __Host-pfp={}; sid=abc", session.0),
        ))
        .await;
    assert_eq!(crowded.status, 200);
    assert_eq!(server.events.count(EventCode::SessionCookieDisplaced), 2);
    server.stop().await;
}

#[tokio::test]
async fn duplicate_host_cookie_is_409() {
    let server = HttpServer::start();
    let session = server.establish().await;
    let same_header = server
        .send(server.authed(STATUS, &session).set(
            "Cookie",
            &format!("__Host-pfp={}; __Host-pfp={}", "0".repeat(64), session.0),
        ))
        .await;
    assert_eq!(same_header.status, 409);
    let two_headers = server
        .send(
            server
                .authed(STATUS, &session)
                .header("Cookie", &format!("__Host-pfp={}", session.0)),
        )
        .await;
    assert_eq!(two_headers.status, 409);
    server.stop().await;
}

#[tokio::test]
async fn second_bootstrap_replaces_the_session() {
    let server = HttpServer::start();
    let old = server.establish().await;
    let new = server.establish().await;
    let with_old = server.send(server.authed(STATUS, &old)).await;
    assert_eq!(with_old.status, 401, "the old pair is dead");
    let with_new = server.send(server.authed(STATUS, &new)).await;
    assert_eq!(with_new.status, 200);
    assert_eq!(server.events.count(EventCode::SessionReplaced), 1);
    server.stop().await;
}

#[derive(Default)]
struct RecordingOpener {
    calls: AtomicUsize,
    last: Mutex<Option<String>>,
}

impl RelaunchHook for RecordingOpener {
    fn reopen(&self, launch_url: &Redacted<String>) -> Result<(), ReopenUnavailable> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        *self.last.lock().unwrap() = Some(launch_url.expose().clone());
        Ok(())
    }
}

#[tokio::test]
async fn relaunch_without_opener_is_503() {
    // The default, and the only configuration any binary in this change has.
    let server = HttpServer::start();
    let session = server.establish().await;
    let response = server.send(server.authed(RELAUNCH, &session)).await;
    assert_eq!(
        (response.status, response.code().as_str()),
        (503, "open_unavailable")
    );
    assert_eq!(
        server.events.count(EventCode::LaunchTokenMinted),
        1,
        "nothing minted"
    );
    server.stop().await;
}

#[tokio::test]
async fn relaunch_with_proof_mints_a_fresh_token_and_calls_the_hook() {
    let opener = Arc::new(RecordingOpener::default());
    let clock = SteppedClock::new();
    let server = with_clock(clock.clone(), Some(opener.clone()));
    let session = server.establish().await;

    // Needs the proof…
    let anonymous = server.send(server.api(RELAUNCH)).await;
    assert_eq!(anonymous.status, 401);
    let cookie_only = server
        .send(server.authed(RELAUNCH, &session).without("X-PFP-Proof"))
        .await;
    assert_eq!(cookie_only.status, 401);
    assert_eq!(opener.calls.load(Ordering::SeqCst), 0);

    // …and only the proof: this is the recovery from a displaced cookie.
    let response = server
        .send(server.authed(RELAUNCH, &session).without("Cookie"))
        .await;
    assert_eq!(response.status, 202);
    assert_eq!(opener.calls.load(Ordering::SeqCst), 1);

    // relaunch_response_never_contains_the_token
    let url = opener.last.lock().unwrap().clone().unwrap();
    let (base, token) = url.split_once("/#t=").expect("token in the fragment");
    assert!(
        base == server.origin,
        "the canonical origin, no hostname form"
    );
    assert!(is_lower_hex_64(token));
    assert!(!url.contains('?'), "never a query string");
    assert_eq!(response.body, b"{}");
    let raw = format!("{:?}", response.headers);
    assert!(
        !raw.contains(token),
        "the token is not in any response header"
    );

    // The URL the opener got really does open a session.
    let reopened = server
        .send(server.api(BOOTSTRAP).json(&token_body(token)))
        .await;
    assert_eq!(reopened.status, 200);
    server.stop().await;
}

#[tokio::test]
async fn relaunch_is_throttled_and_replaces_the_outstanding_token() {
    let opener = Arc::new(RecordingOpener::default());
    let clock = SteppedClock::new();
    let server = with_clock(clock.clone(), Some(opener.clone()));
    let session = server.establish().await;

    assert_eq!(
        server.send(server.authed(RELAUNCH, &session)).await.status,
        202
    );
    let first_url = opener.last.lock().unwrap().clone().unwrap();

    // relaunch_inside_min_interval_is_429_and_calls_nothing
    clock.advance(Duration::from_secs(9));
    let throttled = server.send(server.authed(RELAUNCH, &session)).await;
    assert_eq!(
        (throttled.status, throttled.code().as_str()),
        (429, "relaunch_throttled")
    );
    assert_eq!(opener.calls.load(Ordering::SeqCst), 1);
    assert_eq!(server.events.count(EventCode::RelaunchThrottled), 1);

    // relaunch_replaces_the_outstanding_launch_token
    clock.advance(Duration::from_secs(1));
    assert_eq!(
        server.send(server.authed(RELAUNCH, &session)).await.status,
        202
    );
    let stale = first_url.split_once("/#t=").unwrap().1.to_owned();
    let response = server
        .send(server.api(BOOTSTRAP).json(&token_body(&stale)))
        .await;
    assert_eq!(response.status, 401);

    // relaunch_cap_per_session_is_429
    for _ in 2..20 {
        clock.advance(Duration::from_secs(10));
        assert_eq!(
            server.send(server.authed(RELAUNCH, &session)).await.status,
            202
        );
    }
    // The cap has its own code and message: "wait a moment" would be false, since
    // no amount of waiting mints this session another token.
    clock.advance(Duration::from_secs(10));
    let exhausted = server.send(server.authed(RELAUNCH, &session)).await;
    assert_eq!(
        (exhausted.status, exhausted.code().as_str()),
        (429, "relaunch_exhausted")
    );
    let message = exhausted.json()["message"].as_str().unwrap().to_owned();
    assert!(message.contains("start it again") && !message.contains("Wait"));
    assert_eq!(server.events.count(EventCode::RelaunchExhausted), 1);
    assert_eq!(server.events.count(EventCode::RelaunchThrottled), 1);
    assert_eq!(opener.calls.load(Ordering::SeqCst), 20);
    server.stop().await;
}

#[tokio::test]
async fn tokens_never_reach_events_or_debug() {
    let server = HttpServer::start();
    let token = server.sessions.mint_launch_token().unwrap();
    let session = server.establish().await;
    let _ = server.send(server.authed(STATUS, &session)).await;
    let _ = server
        .send(server.authed(STATUS, &session).without("Cookie"))
        .await;
    let _ = server
        .send(server.api(BOOTSTRAP).json(&token_body(token.expose())))
        .await;

    let rendered = format!(
        "{:?}\n{}\n{:?} {:?}",
        server.events.snapshot(),
        server
            .events
            .snapshot()
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n"),
        server.sessions,
        token,
    );
    for secret in [token.expose(), &session.0, &session.1] {
        assert!(
            !rendered.contains(secret.as_str()),
            "a secret reached the log"
        );
    }
    let longest_hex_run = rendered
        .split(|c: char| !c.is_ascii_hexdigit())
        .map(str::len)
        .max()
        .unwrap_or(0);
    assert!(longest_hex_run < 64, "no token-shaped string in the log");
    // And the log does say what happened, by code.
    assert!(rendered.contains("session_established"));
    assert!(rendered.contains("session_cookie_displaced"));
    assert!(rendered.contains("request_served POST /api/v1/session/status 200"));
    server.stop().await;
}
