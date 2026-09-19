//! The exact response-header set (`SECURITY.md` §7.3, test id S-04).
//!
//! Two lines of defence. The client's `send` asserts the policy set — constants
//! typed from the design, independently of `src/headers.rs` — on **every**
//! response. This file then records the complete header list of each response
//! class as a snapshot, so that an *added* header is caught too: nothing may be
//! added without updating the snapshot and `SECURITY.md` §7.3 together.

// Not an engine crate: tests here legitimately touch sockets, the clock and files.
#![allow(
    clippy::disallowed_types,
    clippy::disallowed_methods,
    clippy::disallowed_macros
)]

mod support;

use std::fmt::Write as _;

use pfp_server::TestKnobs;
use support::{HttpOptions, HttpServer, Resp, IMMUTABLE, NO_STORE};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const STATUS: &str = "/api/v1/session/status";
const JS: &str = "/assets/index-AbCd1234.js";

fn bundle_server() -> HttpServer {
    HttpServer::start_with(HttpOptions {
        assets: support::vite_like_bundle(),
        ..Default::default()
    })
}

// One response per class, in one server, so the snapshot reads as one table.
#[allow(clippy::too_many_lines)]
#[tokio::test]
async fn header_set_per_response_class() {
    let server = bundle_server();
    let session = server.establish().await;
    let mut classes: Vec<(&str, Resp)> = Vec::new();

    classes.push(("document 200", server.send(server.navigate("/")).await));
    classes.push((
        "document HEAD 200",
        server.send(server.navigate("/").method("HEAD")).await,
    ));
    let asset = server.send(server.subresource(JS)).await;
    let etag = asset.header("etag").unwrap().to_owned();
    classes.push(("asset 200", asset));
    classes.push((
        "asset 304",
        server
            .send(server.subresource(JS).header("If-None-Match", &etag))
            .await,
    ));
    classes.push((
        "static 404",
        server.send(server.subresource("/nope.js")).await,
    ));
    let token = server.sessions.mint_launch_token().unwrap();
    classes.push((
        "bootstrap 200",
        server
            .send(
                server
                    .api(support::BOOTSTRAP)
                    .json(&format!(r#"{{"token":"{}"}}"#, token.expose())),
            )
            .await,
    ));
    // That bootstrap replaced the session.
    let _ = session;
    let session = server.establish().await;
    classes.push((
        "api 200",
        server.send(server.authed(STATUS, &session)).await,
    ));
    classes.push(("api 401", server.send(server.api(STATUS)).await));
    classes.push((
        "api 409",
        server
            .send(server.authed(STATUS, &session).without("Cookie"))
            .await,
    ));
    classes.push((
        "api 400",
        server
            .send(
                server
                    .authed("/api/v1/tax/rate-schedule", &session)
                    .json("{"),
            )
            .await,
    ));
    classes.push((
        "api 422",
        server
            .send(
                server
                    .authed("/api/v1/tax/rate-schedule", &session)
                    .json("{}"),
            )
            .await,
    ));
    classes.push((
        "api 404",
        server.send(server.authed("/api/v1/nope", &session)).await,
    ));
    classes.push((
        "api 405",
        server
            .send(server.authed(STATUS, &session).method("GET"))
            .await,
    ));
    classes.push((
        "api 503",
        server
            .send(server.authed("/api/v1/session/relaunch", &session))
            .await,
    ));
    classes.push((
        "host 421",
        server
            .send(server.navigate("/").set("Host", "evil.example"))
            .await,
    ));
    classes.push((
        "origin 403",
        server
            .send(server.api(STATUS).set("Origin", "https://evil.example"))
            .await,
    ));
    classes.push((
        "static 405",
        server
            .send(
                server
                    .subresource("/")
                    .method("POST")
                    .header("Origin", &server.origin),
            )
            .await,
    ));
    classes.push((
        "body 413",
        server
            .send(
                server
                    .authed(STATUS, &session)
                    .header("Content-Type", "application/json")
                    .header("Content-Length", "1048577"),
            )
            .await,
    ));
    classes.push((
        "body 415",
        server
            .send(
                server
                    .authed(STATUS, &session)
                    .header("Content-Type", "text/plain")
                    .body(b"x"),
            )
            .await,
    ));

    let mut snapshot = String::new();
    for (class, response) in &classes {
        let expected: u16 = class.rsplit(' ').next().unwrap().parse().unwrap();
        assert_eq!(response.status, expected, "{class}");
        let cache = response.header("cache-control").unwrap();
        if class.starts_with("asset") {
            assert_eq!(cache, IMMUTABLE, "{class}");
        } else {
            assert_eq!(cache, NO_STORE, "{class}");
        }
        let _ = writeln!(snapshot, "## {class}\n{}", response.header_snapshot());
    }
    insta::assert_snapshot!("header_set_per_response_class", snapshot);
    server.stop().await;
}

#[tokio::test]
async fn request_timeout_is_408_with_the_full_set_and_closes() {
    let server = HttpServer::start_with(HttpOptions {
        knobs: TestKnobs {
            request_deadline: support::SHORT_TIMER,
            echo_events_to_stderr: false,
            ..TestKnobs::default()
        },
        ..Default::default()
    });
    let session = server.establish().await;
    // Declares a body and never sends it: the handler waits, the deadline fires.
    let head = server
        .authed("/api/v1/tax/rate-schedule", &session)
        .header("Content-Type", "application/json")
        .header("Content-Length", "64")
        .to_bytes();
    let head = String::from_utf8(head)
        .unwrap()
        .replace("Connection: close\r\n", "");
    let mut stream = server.connect().await;
    stream.write_all(head.as_bytes()).await.unwrap();
    let mut received = Vec::new();
    tokio::time::timeout(support::SOON, stream.read_to_end(&mut received))
        .await
        .expect("the server answers and closes by itself")
        .unwrap();
    let response = Resp::parse(&received);
    assert_eq!(response.status, 408);
    assert_eq!(response.code(), "request_timeout");
    assert_eq!(response.header("connection"), Some("close"));
    for (name, value) in support::POLICY_HEADERS {
        assert_eq!(response.header(name), Some(value));
    }
    assert_eq!(response.header("cache-control"), Some(NO_STORE));
    insta::assert_snapshot!("request_timeout_408", response.header_snapshot());
    assert_eq!(
        server.events.count(pfp_server::EventCode::RequestDeadline),
        1
    );
    server.stop().await;
}

#[tokio::test]
async fn relaunch_throttle_429_carries_the_full_set() {
    struct Opener;
    impl pfp_server::RelaunchHook for Opener {
        fn reopen(
            &self,
            _: &pfp_server::redact::Redacted<String>,
        ) -> Result<(), pfp_server::ReopenUnavailable> {
            Ok(())
        }
    }
    let server = HttpServer::start_with(HttpOptions {
        relaunch: Some(std::sync::Arc::new(Opener)),
        ..Default::default()
    });
    let session = server.establish().await;
    let path = "/api/v1/session/relaunch";
    assert_eq!(server.send(server.authed(path, &session)).await.status, 202);
    let throttled = server.send(server.authed(path, &session)).await;
    assert_eq!(throttled.status, 429);
    insta::assert_snapshot!("relaunch_throttled_429", throttled.header_snapshot());
    server.stop().await;
}
