//! Responses hyper writes by itself, below every layer of this crate.
//!
//! Input that is not well-formed HTTP never reaches the service, so these
//! responses carry **none** of the `SECURITY.md` §7.3 policy headers. That is a
//! recorded deviation, not an oversight: answering them ourselves would mean a
//! hand-written HTTP parser in front of hyper, in the one crate that parses
//! untrusted bytes, for responses with no body to sniff, frame, cache or read
//! cross-origin. This file pins the class — status, the exact reduced header list,
//! an empty body, the connection closed — so that a hyper upgrade that changes any
//! of it moves a snapshot and a human looks. The universal negatives (no CORS, no
//! `Set-Cookie`, no `Location`, no `Server`) are still asserted, inside
//! `send_protocol_level`.

// Not an engine crate: tests here legitimately touch sockets, the clock and files.
#![allow(
    clippy::disallowed_types,
    clippy::disallowed_methods,
    clippy::disallowed_macros
)]

mod support;

use std::fmt::Write as _;

use support::HttpServer;

#[tokio::test]
async fn hyper_written_responses_are_bare_and_pinned() {
    let server = HttpServer::start();
    let host = format!("Host: {}\r\n", server.host);
    let many_headers = "X-Pad: 1\r\n".repeat(80);

    let inputs: Vec<(&str, String, u16)> = vec![
        (
            "malformed request line",
            format!("GARBAGE\r\n{host}\r\n"),
            400,
        ),
        (
            "bad header bytes",
            format!("GET / HTTP/1.1\r\n{host}Bad\x01Name: x\r\n\r\n"),
            400,
        ),
        ("http/0.9-style request", "GET /\r\n\r\n".to_owned(), 400),
        (
            "conflicting content-length",
            format!("POST / HTTP/1.1\r\n{host}Content-Length: 1\r\nContent-Length: 2\r\n\r\nab"),
            400,
        ),
        (
            "too many headers",
            format!("GET / HTTP/1.1\r\n{host}{many_headers}\r\n"),
            431,
        ),
    ];

    let mut snapshot = String::new();
    for (what, bytes, expected) in inputs {
        let response = server
            .send_protocol_level(bytes.as_bytes())
            .await
            .unwrap_or_else(|| panic!("{what}: closed without a response"));
        assert_eq!(response.status, expected, "{what}");
        assert!(response.body.is_empty(), "{what}: the body is empty");
        assert!(
            response.header("content-security-policy").is_none(),
            "{what}: below the service"
        );
        let _ = writeln!(snapshot, "## {what}\n{}", response.header_snapshot());
    }
    insta::assert_snapshot!("hyper_written_responses", snapshot);
    server.stop().await;
}

#[tokio::test]
async fn an_http2_preface_gets_no_response_at_all() {
    // ALPN offers http/1.1 only; a client that speaks HTTP/2 anyway is dropped.
    let server = HttpServer::start();
    let response = server
        .send_protocol_level(b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n")
        .await;
    assert!(response.is_none());
    server.stop().await;
}

#[tokio::test]
async fn an_oversized_request_head_is_never_served() {
    // Two caps. hyper's is on its read buffer and is checked between reads, so it
    // is approximate: a head somewhat over 64 KiB can pass it. The service's own
    // check is exact (431 `headers_too_large`, with the full policy set). Whichever
    // fires, and even if the server closes before the client has finished writing,
    // the request is never served.
    let server = HttpServer::start();
    for kib in [65, 70, 100, 300] {
        let pad = "a".repeat(kib * 1024);
        let inputs = [
            format!(
                "GET / HTTP/1.1\r\nHost: {}\r\nX-Pad: {pad}\r\nConnection: close\r\n\r\n",
                server.host
            ),
            format!(
                "GET /?{pad} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
                server.host
            ),
        ];
        for bytes in inputs {
            if let Some(response) = server.send_protocol_level(bytes.as_bytes()).await {
                // 414 is hyper's answer to a request target over its own limit.
                assert!(
                    [431, 414].contains(&response.status),
                    "{kib} KiB: {}",
                    response.status
                );
                if !response.body.is_empty() {
                    assert_eq!(response.code(), "headers_too_large");
                    assert!(response.header("content-security-policy").is_some());
                }
            }
        }
    }
    // Just under the cap is served.
    let pad = "a".repeat(60 * 1024);
    let ok = server
        .send(server.navigate("/").header("X-Pad", &pad))
        .await;
    assert_eq!(ok.status, 200);
    server.stop().await;
}

#[tokio::test]
async fn duplicate_host_reaches_our_layer_and_is_421_with_the_full_set() {
    // hyper does not reject a duplicated Host: both values reach the header map,
    // and admission counts them rather than taking the first.
    let server = HttpServer::start();
    let response = server
        .send(server.navigate("/").header("Host", "evil.example"))
        .await;
    assert_eq!(
        (response.status, response.code().as_str()),
        (421, "misdirected_host")
    );
    server.stop().await;
}
