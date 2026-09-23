//! Request admission over real TLS on loopback (`SECURITY.md` §7.2, test id S-01).
//!
//! The pure rule table is unit-tested in `src/admission.rs`; these tests prove the
//! server applies it, with the exact status codes, and that every refusal still
//! carries the full policy-header set (asserted inside the client's `send`).

// Not an engine crate: tests here legitimately touch sockets, the clock and files.
#![allow(
    clippy::disallowed_types,
    clippy::disallowed_methods,
    clippy::disallowed_macros
)]

mod support;

use support::{HttpServer, Req};

const STATUS: &str = "/api/v1/session/status";

#[tokio::test]
async fn foreign_host_is_421() {
    let server = HttpServer::start();
    let port = server.addr.port();
    let variants: Vec<(&str, Req)> = vec![
        ("missing", server.navigate("/").without("Host")),
        (
            "duplicate",
            server.navigate("/").header("Host", &server.host),
        ),
        (
            "duplicate foreign",
            server.navigate("/").header("Host", "evil.example"),
        ),
        (
            "localhost",
            server
                .navigate("/")
                .set("Host", &format!("localhost:{port}")),
        ),
        ("no port", server.navigate("/").set("Host", "127.0.0.1")),
        (
            "other port",
            server
                .navigate("/")
                .set("Host", &format!("127.0.0.1:{}", port.wrapping_add(1))),
        ),
        (
            "trailing dot",
            server
                .navigate("/")
                .set("Host", &format!("127.0.0.1.:{port}")),
        ),
        (
            "rebound name",
            server
                .navigate("/")
                .set("Host", &format!("rebind.example:{port}")),
        ),
        (
            "absolute-form target naming another authority",
            server
                .navigate("/")
                .target(&format!("https://rebind.example:{port}/")),
        ),
        (
            "foreign host on the API, otherwise perfect",
            server.api(STATUS).set("Host", "evil.example"),
        ),
    ];
    for (what, request) in variants {
        let response = server.send(request).await;
        assert_eq!(response.status, 421, "{what}");
        assert_eq!(response.code(), "misdirected_host", "{what}");
    }
    server.stop().await;
}

#[tokio::test]
async fn absolute_form_target_with_the_canonical_authority_is_served() {
    let server = HttpServer::start();
    let target = format!("{}/", server.origin);
    let response = server.send(server.navigate("/").target(&target)).await;
    assert_eq!(response.status, 200);
    server.stop().await;
}

#[tokio::test]
async fn foreign_or_missing_origin_on_api_is_403() {
    let server = HttpServer::start();
    let port = server.addr.port();
    let bad = [
        "null".to_owned(),
        "https://evil.example".to_owned(),
        format!("http://127.0.0.1:{port}"),
        format!("https://localhost:{port}"),
        format!("https://127.0.0.1:{}", port.wrapping_add(1)),
        format!("https://127.0.0.1:{port}/"),
    ];
    for origin in &bad {
        let response = server.send(server.api(STATUS).set("Origin", origin)).await;
        assert_eq!(response.status, 403, "{origin}");
        assert_eq!(response.code(), "origin_forbidden", "{origin}");
    }
    let missing = server.send(server.api(STATUS).without("Origin")).await;
    assert_eq!(
        (missing.status, missing.code().as_str()),
        (403, "origin_forbidden")
    );
    let twice = server
        .send(server.api(STATUS).header("Origin", &server.origin))
        .await;
    assert_eq!(twice.status, 403);
    // A same-origin GET carries no Origin in a browser; to the letter of §7.2 it
    // is refused, which is why every API operation is POST.
    let get = server
        .send(server.api(STATUS).method("GET").without("Origin"))
        .await;
    assert_eq!(get.status, 403);
    server.stop().await;
}

#[tokio::test]
async fn api_fetch_site_absent_none_same_site_cross_site_is_403() {
    let server = HttpServer::start();
    for value in ["none", "same-site", "cross-site", "Same-Origin", ""] {
        let response = server
            .send(
                server
                    .api(STATUS)
                    .set("Sec-Fetch-Site", value)
                    .set("Sec-Fetch-Mode", "navigate")
                    .set("Sec-Fetch-Dest", "document"),
            )
            .await;
        assert_eq!(response.status, 403, "{value:?}");
        assert_eq!(response.code(), "fetch_site_forbidden", "{value:?}");
    }
    let absent = server
        .send(server.api(STATUS).without("Sec-Fetch-Site"))
        .await;
    assert_eq!(
        (absent.status, absent.code().as_str()),
        (403, "fetch_site_forbidden")
    );
    server.stop().await;
}

#[tokio::test]
async fn an_admitted_api_request_reaches_the_session_gate() {
    let server = HttpServer::start();
    let response = server.send(server.api(STATUS)).await;
    assert_eq!(
        (response.status, response.code().as_str()),
        (401, "session_required")
    );
    server.stop().await;
}

#[tokio::test]
async fn top_level_navigation_to_root_is_served() {
    let server = HttpServer::start();
    let response = server.send(server.navigate("/")).await;
    assert_eq!(response.status, 200);
    assert_eq!(
        response.header("content-type"),
        Some("text/html; charset=utf-8")
    );
    assert!(response.body.starts_with(b"<!doctype html>"));
    // With a query string too (the launch token is in the fragment, never here).
    let with_query = server.send(server.navigate("/?utm=x")).await;
    assert_eq!(with_query.status, 200);
    server.stop().await;
}

#[tokio::test]
async fn fetch_site_none_without_navigate_or_off_root_is_403() {
    let server = HttpServer::start();
    let cases = [
        (
            "mode cors",
            server.navigate("/").set("Sec-Fetch-Mode", "cors"),
        ),
        ("no mode", server.navigate("/").without("Sec-Fetch-Mode")),
        ("no dest", server.navigate("/").without("Sec-Fetch-Dest")),
        (
            "dest iframe",
            server.navigate("/").set("Sec-Fetch-Dest", "iframe"),
        ),
        ("off root", server.navigate("/index.html")),
        ("asset", server.navigate("/assets/index-AbCd1234.js")),
    ];
    for (what, request) in cases {
        let response = server.send(request).await;
        assert_eq!(response.status, 403, "{what}");
        assert_eq!(response.code(), "fetch_site_forbidden", "{what}");
    }
    server.stop().await;
}

#[tokio::test]
async fn cross_site_and_same_site_navigation_is_403() {
    let server = HttpServer::start();
    for site in ["cross-site", "same-site", "bogus"] {
        for path in ["/", "/assets/index-AbCd1234.js"] {
            let response = server
                .send(server.navigate(path).set("Sec-Fetch-Site", site))
                .await;
            assert_eq!(response.status, 403, "{site} {path}");
        }
    }
    server.stop().await;
}

#[tokio::test]
async fn absent_fetch_site_is_served_off_the_api_with_corp_same_origin() {
    // Decision D4: the design refuses absence on /api/** only. The compensating
    // control for a client without Fetch Metadata is CORP, asserted here by name.
    let server = HttpServer::start_with(support::HttpOptions {
        assets: support::vite_like_bundle(),
        ..Default::default()
    });
    for path in ["/", "/assets/index-AbCd1234.js"] {
        let response = server
            .send(Req::new("GET", path).header("Host", &server.host))
            .await;
        assert_eq!(response.status, 200, "{path}");
        assert_eq!(
            response.header("cross-origin-resource-policy"),
            Some("same-origin")
        );
    }
    server.stop().await;
}

#[tokio::test]
async fn a_present_foreign_origin_is_403_on_every_route() {
    let server = HttpServer::start();
    for path in ["/", "/assets/x-AbCd1234.js"] {
        let response = server
            .send(
                server
                    .subresource(path)
                    .header("Origin", "https://evil.example"),
            )
            .await;
        assert_eq!(
            (response.status, response.code().as_str()),
            (403, "origin_forbidden")
        );
    }
    server.stop().await;
}

#[tokio::test]
async fn state_changing_method_on_static_route() {
    let server = HttpServer::start();
    for method in ["POST", "PUT", "DELETE", "PATCH"] {
        // Without an Origin: 403 before it is 405.
        let bare = server.send(server.subresource("/").method(method)).await;
        assert_eq!(bare.status, 403, "{method}");
        let with_origin = server
            .send(
                server
                    .subresource("/")
                    .method(method)
                    .header("Origin", &server.origin),
            )
            .await;
        assert_eq!(with_origin.status, 405, "{method}");
        assert_eq!(with_origin.code(), "method_not_allowed");
        assert_eq!(with_origin.header("allow"), Some("GET, HEAD"));
    }
    server.stop().await;
}

#[tokio::test]
async fn preflight_is_never_approved() {
    let server = HttpServer::start();
    // What a browser sends before a cross-origin request with a custom header.
    let cross = Req::new("OPTIONS", STATUS)
        .header("Host", &server.host)
        .header("Origin", "https://evil.example")
        .header("Sec-Fetch-Site", "cross-site")
        .header("Sec-Fetch-Mode", "cors")
        .header("Access-Control-Request-Method", "POST")
        .header("Access-Control-Request-Headers", "x-pfp-proof")
        .header("Access-Control-Request-Private-Network", "true");
    let response = server.send(cross).await;
    assert_eq!(response.status, 403);

    // Even a same-origin OPTIONS with a live session is only ever a 405.
    let session = server.establish().await;
    let same = server
        .send(server.authed(STATUS, &session).method("OPTIONS"))
        .await;
    assert_eq!(same.status, 405);
    assert_eq!(same.header("allow"), Some("POST"));
    let off_api = server
        .send(
            server
                .subresource("/")
                .method("OPTIONS")
                .header("Origin", &server.origin),
        )
        .await;
    assert_eq!(off_api.status, 405);
    server.stop().await;
}

#[tokio::test]
async fn body_rules_are_413_and_415() {
    let server = HttpServer::start();
    let session = server.establish().await;
    let too_long = server
        .send(
            server
                .authed(STATUS, &session)
                .header("Content-Type", "application/json")
                .header("Content-Length", "1048577"),
        )
        .await;
    assert_eq!(
        (too_long.status, too_long.code().as_str()),
        (413, "payload_too_large")
    );

    for content_type in [
        "text/plain",
        "application/x-www-form-urlencoded",
        "multipart/form-data",
    ] {
        let response = server
            .send(
                server
                    .authed("/api/v1/tax/rate-schedule", &session)
                    .header("Content-Type", content_type)
                    .body(b"{}"),
            )
            .await;
        assert_eq!(response.status, 415, "{content_type}");
        assert_eq!(response.code(), "unsupported_media_type");
    }
    let untyped = server
        .send(
            server
                .authed("/api/v1/tax/rate-schedule", &session)
                .body(b"{}"),
        )
        .await;
    assert_eq!(untyped.status, 415);
    // A form post — the classic CSRF vector — dies before the session is looked at.
    let csrf = server
        .send(
            server
                .api(STATUS)
                .header("Content-Type", "application/x-www-form-urlencoded")
                .body(b"a=1"),
        )
        .await;
    assert_eq!(csrf.status, 415);
    server.stop().await;
}

#[tokio::test]
async fn chunked_body_over_the_cap_is_413() {
    let server = HttpServer::start();
    let session = server.establish().await;
    let chunk = vec![b' '; 64 * 1024];
    let mut body = Vec::new();
    for _ in 0..17 {
        body.extend_from_slice(format!("{:x}\r\n", chunk.len()).as_bytes());
        body.extend_from_slice(&chunk);
        body.extend_from_slice(b"\r\n");
    }
    body.extend_from_slice(b"0\r\n\r\n");
    let response = server
        .send(
            server
                .authed("/api/v1/tax/rate-schedule", &session)
                .header("Content-Type", "application/json")
                .header("Transfer-Encoding", "chunked")
                .body(&body),
        )
        .await;
    assert_eq!(
        (response.status, response.code().as_str()),
        (413, "payload_too_large")
    );
    server.stop().await;
}

#[tokio::test]
async fn unknown_paths_are_404_and_never_a_redirect() {
    let server = HttpServer::start();
    let session = server.establish().await;
    let api = server.send(server.authed("/api/v1/nope", &session)).await;
    assert_eq!((api.status, api.code().as_str()), (404, "not_found"));
    // Unauthenticated, an unknown API path is indistinguishable from a known one.
    let anonymous = server.send(server.api("/api/v1/nope")).await;
    assert_eq!(anonymous.status, 401);
    for path in [
        "/index.html",
        "/assets",
        "/assets/",
        "/favicon.ico",
        "/API/v1/x",
        "//",
        "/%2e%2e/",
    ] {
        let response = server.send(server.subresource(path)).await;
        assert_eq!(response.status, 404, "{path}");
    }
    server.stop().await;
}

#[tokio::test]
async fn refusals_are_logged_by_code_and_template_only() {
    let server = HttpServer::start();
    let _ = server
        .send(
            server
                .api("/api/v1/plan/a-secret-name")
                .set("Host", "evil.example"),
        )
        .await;
    let rendered: Vec<String> = server
        .events
        .snapshot()
        .iter()
        .map(ToString::to_string)
        .collect();
    // The refusal's stable code is named; the path as sent ("a-secret-name") never is.
    assert_eq!(
        rendered,
        ["#1 request_refused POST /api/** 421 misdirected_host"]
    );
    assert!(!rendered[0].contains("secret"));
    server.stop().await;
}
