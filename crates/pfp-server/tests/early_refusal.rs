//! The graceful close after an early refusal (`src/linger.rs`, RFC 9112 §9.6).
//!
//! A refusal decided before the request body was consumed — 413, 403, 401, 408 —
//! reaches a peer that may still be uploading. Closing at once answers the rest of
//! the upload with a TCP reset: the peer's write fails with `EPIPE`/`ECONNRESET`,
//! and a client that treats that as a network error never shows the refusal.
//!
//! These tests **force** that ordering instead of hoping for it: the client reads
//! the complete response, to end of stream, and only *then* writes the rest of its
//! body. Without the drain the socket is already closed by then and the write is
//! reset every time; with it, every byte is accepted. The bounds are tested the
//! same way, because an unbounded drain would be a denial-of-service surface.

// Not an engine crate: tests here legitimately touch sockets, the clock and files.
#![allow(
    clippy::disallowed_types,
    clippy::disallowed_methods,
    clippy::disallowed_macros
)]

mod support;

use std::time::{Duration, Instant};

use pfp_server::limits::{LINGER_MAX_BYTES, MAX_BODY_BYTES};
use pfp_server::TestKnobs;
use support::{
    HttpOptions, HttpServer, Req, Resp, PAST_REQUEST_DEADLINE, POLL_INTERVAL, SHORT_TIMER,
    SHORT_TIMER_FLOOR, SOON,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio_rustls::client::TlsStream;

const RATE_SCHEDULE: &str = "/api/v1/tax/rate-schedule";

/// What the client writes between flushes.
const PIECE: usize = 16 * 1024;

/// An upload half as large again as the body cap: refused, and — all of it, even
/// if none was read before the refusal — within the production drain budget.
const UPLOAD: usize = MAX_BODY_BYTES + MAX_BODY_BYTES / 2;
const _: () = assert!(UPLOAD > MAX_BODY_BYTES && UPLOAD <= LINGER_MAX_BYTES);

/// Far beyond any drain budget.
const HUGE_UPLOAD: usize = 8 * LINGER_MAX_BYTES;

/// A drain budget small enough that the kernel's socket buffers cannot hide it.
const TINY_DRAIN: usize = 64 * 1024;

fn knobs(change: impl FnOnce(&mut TestKnobs)) -> HttpOptions {
    let mut knobs = TestKnobs {
        echo_events_to_stderr: false,
        ..TestKnobs::default()
    };
    change(&mut knobs);
    HttpOptions {
        knobs,
        ..Default::default()
    }
}

/// These tests are about the byte path, so the drain's clock is taken out of the
/// picture: production's two seconds are ample on loopback, but a wall-clock bound
/// in a test of something else is how a suite becomes flaky.
fn unhurried() -> HttpOptions {
    knobs(|k| k.http.linger.timeout = PAST_REQUEST_DEADLINE)
}

/// Writes `total` bytes of body in flushed pieces — chunk-framed when `chunked` —
/// and reports how many body bytes were accepted before the first failed write.
async fn upload(
    stream: &mut TlsStream<TcpStream>,
    total: usize,
    chunked: bool,
) -> (usize, std::io::Result<()>) {
    let piece = vec![b' '; PIECE];
    let mut sent = 0;
    while sent < total {
        let size = piece.len().min(total - sent);
        let mut framed = Vec::with_capacity(size + 16);
        if chunked {
            framed.extend_from_slice(format!("{size:x}\r\n").as_bytes());
        }
        framed.extend_from_slice(&piece[..size]);
        if chunked {
            framed.extend_from_slice(b"\r\n");
        }
        let mut written = stream.write_all(&framed).await;
        if written.is_ok() {
            written = stream.flush().await;
        }
        if let Err(error) = written {
            return (sent, Err(error));
        }
        sent += size;
    }
    (sent, Ok(()))
}

/// Reads to end of stream: the complete response, and proof that the server has
/// finished writing and shut its side down.
async fn read_response(stream: &mut TlsStream<TcpStream>) -> Resp {
    let mut received = Vec::new();
    tokio::time::timeout(PAST_REQUEST_DEADLINE, stream.read_to_end(&mut received))
        .await
        .expect("the server answers and shuts its side down")
        .expect("with a clean end of stream (close_notify), not a reset");
    Resp::parse(&received)
}

/// The forced race: `before` bytes of body, then the whole response, then the rest.
async fn refused_mid_upload(
    server: &HttpServer,
    head: Req,
    chunked: bool,
    before: usize,
    total: usize,
) -> (Resp, usize, std::io::Result<()>) {
    // `to_bytes` ends the head; the body is written by hand below.
    let mut stream = server.connect().await;
    stream.write_all(&head.to_bytes()).await.expect("head");
    stream.flush().await.expect("head");
    let (sent_before, written) = upload(&mut stream, before, chunked).await;
    written.expect("nothing is refused yet");

    let response = read_response(&mut stream).await;

    // The server has answered and shut down. The client — like a browser in the
    // middle of an upload — has not noticed, and carries on.
    let (sent_after, written) = upload(&mut stream, total - before, chunked).await;
    (response, sent_before + sent_after, written)
}

fn declared(server: &HttpServer, session: &(String, String), length: usize) -> Req {
    server
        .authed(RATE_SCHEDULE, session)
        .header("Content-Type", "application/json")
        .header("Content-Length", &length.to_string())
}

#[tokio::test]
async fn an_upload_in_flight_when_it_is_refused_is_never_reset() {
    let server = HttpServer::start_with(unhurried());
    let session = server.establish().await;
    let nobody = (String::from("not-a-cookie"), String::from("not-a-proof"));

    let chunked = server
        .authed(RATE_SCHEDULE, &session)
        .header("Content-Type", "application/json")
        .header("Transfer-Encoding", "chunked");
    // (name, head, chunked, body bytes sent before the response, body bytes in
    // all, status, code)
    let cases: [(&str, Req, bool, usize, usize, u16, &str); 4] = [
        // Refused part-way: the cap is only known to be exceeded once it is.
        (
            "413, chunked",
            chunked,
            true,
            MAX_BODY_BYTES + PIECE,
            UPLOAD,
            413,
            "payload_too_large",
        ),
        // Refused from the head alone; not one body byte has been read.
        (
            "413, declared length",
            declared(&server, &session, UPLOAD),
            false,
            0,
            UPLOAD,
            413,
            "payload_too_large",
        ),
        (
            "403, admission",
            declared(&server, &session, UPLOAD).set("Origin", "https://evil.example"),
            false,
            0,
            UPLOAD,
            403,
            "origin_forbidden",
        ),
        (
            "401, session gate",
            // A length over the cap would be a 413 before the session is looked at.
            declared(&server, &nobody, MAX_BODY_BYTES),
            false,
            0,
            MAX_BODY_BYTES,
            401,
            "session_required",
        ),
    ];
    for (name, head, is_chunked, before, total, status, code) in cases {
        let (response, sent, written) =
            refused_mid_upload(&server, head, is_chunked, before, total).await;
        assert_eq!(response.status, status, "{name}");
        assert_eq!(response.code(), code, "{name}");
        assert_eq!(response.header("connection"), Some("close"), "{name}");
        assert!(
            written.is_ok(),
            "{name}: the upload was reset after {sent} of {total} bytes: {written:?}"
        );
        assert_eq!(sent, total, "{name}");
    }
    server.stop().await;
}

#[tokio::test]
async fn an_upload_cut_by_the_request_deadline_is_not_reset_either() {
    let server = HttpServer::start_with(knobs(|k| {
        k.request_deadline = SHORT_TIMER;
        k.http.linger.timeout = PAST_REQUEST_DEADLINE;
    }));
    let session = server.establish().await;
    // Part of the body, then silence until the 408 arrives, then the rest.
    let (response, sent, written) = refused_mid_upload(
        &server,
        declared(&server, &session, MAX_BODY_BYTES),
        false,
        PIECE,
        MAX_BODY_BYTES,
    )
    .await;
    assert_eq!(
        (response.status, response.code().as_str()),
        (408, "request_timeout")
    );
    assert!(written.is_ok(), "reset after {sent} bytes: {written:?}");
    server.stop().await;
}

#[tokio::test]
async fn the_drain_stops_at_its_byte_budget() {
    let server = HttpServer::start_with(knobs(|k| {
        k.http.linger.max_bytes = TINY_DRAIN;
        k.http.linger.timeout = PAST_REQUEST_DEADLINE;
    }));
    let session = server.establish().await;
    let (response, sent, written) = refused_mid_upload(
        &server,
        declared(&server, &session, HUGE_UPLOAD),
        false,
        0,
        HUGE_UPLOAD,
    )
    .await;
    // The refusal was delivered first, in full…
    assert_eq!(
        (response.status, response.code().as_str()),
        (413, "payload_too_large")
    );
    // …and the server did not go on to swallow the upload.
    assert!(
        written.is_err(),
        "all {sent} bytes were accepted: the drain is unbounded"
    );
    assert!(sent < HUGE_UPLOAD);
    server.stop().await;
}

/// Connects until the server has a free permit again, and reports how long that
/// took. With `max_connections = 1` this measures how long the previous
/// connection was held.
async fn until_a_permit_is_free(server: &HttpServer, since: Instant) -> Duration {
    loop {
        if let Ok(mut stream) =
            support::tls_connect(server.addr, server.client(), "127.0.0.1").await
        {
            let request = server.navigate("/").to_bytes();
            if stream.write_all(&request).await.is_ok() && stream.flush().await.is_ok() {
                let mut received = Vec::new();
                let _ = stream.read_to_end(&mut received).await;
                if received.starts_with(b"HTTP/1.1 200") {
                    return since.elapsed();
                }
            }
        }
        assert!(since.elapsed() < SOON, "the connection was never released");
        tokio::time::sleep(POLL_INTERVAL).await;
    }
}

#[tokio::test]
async fn the_drain_stops_at_its_time_budget() {
    let server = HttpServer::start_with(knobs(|k| {
        k.accept.max_connections = 1;
        k.http.linger.timeout = SHORT_TIMER;
    }));
    // Refused from the head; the peer then holds the connection open, silent.
    let nobody = (String::from("not-a-cookie"), String::from("not-a-proof"));
    let mut silent = server.connect().await;
    let head = declared(&server, &nobody, MAX_BODY_BYTES).to_bytes();
    let started = Instant::now();
    silent.write_all(&head).await.expect("head");
    silent.flush().await.expect("head");
    assert_eq!(read_response(&mut silent).await.status, 401);

    let held_for = until_a_permit_is_free(&server, started).await;
    // It lingered (the permit was held for the budget)…
    assert!(held_for >= SHORT_TIMER_FLOOR, "{held_for:?}");
    // …and `until_a_permit_is_free` asserted that it did not linger for ever.
    drop(silent);
    server.stop().await;
}

#[tokio::test]
async fn a_request_whose_body_was_consumed_is_closed_without_lingering() {
    let server = HttpServer::start_with(knobs(|k| {
        k.accept.max_connections = 1;
        // Were this connection drained, it would hold the only permit for longer
        // than `until_a_permit_is_free` is prepared to wait.
        k.http.linger.timeout = PAST_REQUEST_DEADLINE;
    }));
    let token = server.sessions.mint_launch_token().expect("mint");
    let bootstrap = server
        .api(support::BOOTSTRAP)
        .json(&format!(r#"{{"token":"{}"}}"#, token.expose()));
    let mut open = server.connect().await;
    let started = Instant::now();
    open.write_all(&bootstrap.to_bytes()).await.expect("write");
    open.flush().await.expect("flush");
    assert_eq!(read_response(&mut open).await.status, 200);

    // `open` is still held by the client; the server is done with it regardless.
    until_a_permit_is_free(&server, started).await;
    drop(open);
    server.stop().await;
}
