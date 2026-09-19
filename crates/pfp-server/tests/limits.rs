//! Resource limits (`src/limits.rs`): the connection cap, the header/idle timer and
//! the whole-request deadline. They bound memory, tasks and file descriptors; they
//! are not an availability guarantee against a same-user local process.

// Not an engine crate: tests here legitimately touch sockets, the clock and files.
#![allow(
    clippy::disallowed_types,
    clippy::disallowed_methods,
    clippy::disallowed_macros
)]

mod support;

use std::time::{Duration, Instant};

use pfp_server::{AcceptLimits, HttpLimits, TestKnobs};
use support::{
    eventually, HttpOptions, HttpServer, Resp, POLL_INTERVAL, SHORT_TIMER, SHORT_TIMER_FLOOR, SOON,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// The slow-body drip: one byte every `DRIP_EVERY`, `DRIP_COUNT` times — five
/// seconds in all, many times the shortened request deadline.
const DRIP_EVERY: Duration = Duration::from_millis(25);
const DRIP_COUNT: u32 = 200;
/// Well past the shortened deadline, well short of the whole drip: an answer
/// before this was cut by the deadline, not by the body ending.
const CUT_BY: Duration = Duration::from_millis(2_500);
const _: () = {
    assert!(SHORT_TIMER.as_millis() * 4 < CUT_BY.as_millis());
    assert!(CUT_BY.as_millis() * 2 <= DRIP_EVERY.as_millis() * DRIP_COUNT as u128);
    assert!(CUT_BY.as_millis() < SOON.as_millis());
};

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

#[test]
fn production_limits_are_the_documented_values() {
    use pfp_server::limits::*;
    assert_eq!(TLS_HANDSHAKE_TIMEOUT, Duration::from_secs(10));
    assert_eq!(HEADER_READ_TIMEOUT, Duration::from_secs(10));
    assert_eq!(KEEPALIVE_IDLE, HEADER_READ_TIMEOUT);
    assert_eq!(MAX_HEADER_BYTES, 64 * 1024);
    assert_eq!(MAX_HEADER_COUNT, 64);
    assert_eq!(MAX_BODY_BYTES, 1024 * 1024);
    assert_eq!(REQUEST_DEADLINE, Duration::from_secs(30));
    assert_eq!(MAX_CONNECTIONS, 64);
    assert_eq!(LAUNCH_TOKEN_TTL, Duration::from_secs(60));
    assert_eq!(IDLE_LOCK_AFTER, Duration::from_secs(900));
    assert_eq!(RELAUNCH_MIN_INTERVAL, Duration::from_secs(10));
    assert_eq!(RELAUNCH_MAX_PER_SESSION, 20);
    let defaults = TestKnobs::default();
    assert_eq!(defaults.accept, AcceptLimits::default());
    assert_eq!(defaults.http, HttpLimits::default());
    assert_eq!(defaults.request_deadline, REQUEST_DEADLINE);
}

#[tokio::test]
async fn cap_releases_and_service_resumes() {
    let server = HttpServer::start_with(knobs(|k| k.accept.max_connections = 3));
    // Three idle TLS connections hold every permit.
    let mut held = Vec::new();
    for _ in 0..3 {
        held.push(server.connect().await);
    }
    // The next socket is dropped before any TLS byte.
    let refused = tokio::time::timeout(
        SOON,
        support::tls_connect(server.addr, support::client_trusting_nothing(), "127.0.0.1"),
    )
    .await
    .expect("refusal is immediate");
    assert!(refused.is_err());
    eventually("the refusal is counted", || {
        server.stats.refused_at_cap() >= 1
    })
    .await;

    drop(held);
    // Capacity returns by itself.
    let deadline = Instant::now() + SOON;
    loop {
        if let Ok(mut stream) =
            support::tls_connect(server.addr, server.client(), "127.0.0.1").await
        {
            let request = server.navigate("/").to_bytes();
            stream.write_all(&request).await.unwrap();
            let mut received = Vec::new();
            let _ = stream.read_to_end(&mut received).await;
            assert_eq!(Resp::parse(&received).status, 200);
            break;
        }
        assert!(Instant::now() < deadline, "service did not resume");
        tokio::time::sleep(POLL_INTERVAL).await;
    }
    server.stop().await;
}

#[tokio::test]
async fn idle_keepalive_connection_is_closed() {
    let server = HttpServer::start_with(knobs(|k| {
        k.http.header_read_timeout = SHORT_TIMER;
    }));
    let mut stream = server.connect().await;
    // One keep-alive request…
    let request = String::from_utf8(server.navigate("/").to_bytes())
        .unwrap()
        .replace("Connection: close\r\n", "");
    stream.write_all(request.as_bytes()).await.unwrap();
    // …then silence. The server must close the connection by itself.
    let started = Instant::now();
    let mut received = Vec::new();
    tokio::time::timeout(SOON, stream.read_to_end(&mut received))
        .await
        .expect("an idle kept-alive connection is closed by the server")
        .ok();
    assert_eq!(Resp::parse(&received).status, 200);
    assert!(started.elapsed() >= SHORT_TIMER_FLOOR);
    server.stop().await;
}

#[tokio::test]
async fn a_connection_that_never_sends_a_request_is_closed() {
    let server = HttpServer::start_with(knobs(|k| {
        k.http.header_read_timeout = SHORT_TIMER;
    }));
    let mut stream = server.connect().await;
    stream.write_all(b"GET / HTTP/1.1\r\nHost: ").await.unwrap();
    let mut received = Vec::new();
    tokio::time::timeout(SOON, stream.read_to_end(&mut received))
        .await
        .expect("a stalled request head is dropped")
        .ok();
    // hyper closes without writing a response.
    assert!(!received.starts_with(b"HTTP/1.1 2"));
    server.stop().await;
}

#[tokio::test]
async fn slow_body_is_cut_at_the_request_deadline() {
    let server = HttpServer::start_with(knobs(|k| {
        k.request_deadline = SHORT_TIMER;
    }));
    let session = server.establish().await;
    let head = String::from_utf8(
        server
            .authed("/api/v1/tax/rate-schedule", &session)
            .header("Content-Type", "application/json")
            .header("Content-Length", "1000")
            .to_bytes(),
    )
    .unwrap();
    let mut stream = server.connect().await;
    stream.write_all(head.as_bytes()).await.unwrap();

    // Drip the body a byte at a time, far slower than the deadline allows.
    let (mut reader, mut writer) = tokio::io::split(stream);
    let drip = tokio::spawn(async move {
        for _ in 0..DRIP_COUNT {
            if writer.write_all(b" ").await.is_err() {
                break;
            }
            let _ = writer.flush().await;
            tokio::time::sleep(DRIP_EVERY).await;
        }
    });
    let started = Instant::now();
    let mut received = Vec::new();
    tokio::time::timeout(SOON, reader.read_to_end(&mut received))
        .await
        .expect("the server answers and closes")
        .ok();
    drip.abort();
    let response = Resp::parse(&received);
    assert_eq!(
        (response.status, response.code().as_str()),
        (408, "request_timeout")
    );
    assert!(
        started.elapsed() < CUT_BY,
        "cut at the deadline, not at the end of the drip"
    );
    server.stop().await;
}

#[tokio::test]
async fn stalled_handshake_is_dropped() {
    let server = HttpServer::start_with(knobs(|k| {
        k.accept.handshake_timeout = SHORT_TIMER;
    }));
    let mut tcp = tokio::net::TcpStream::connect(server.addr).await.unwrap();
    let mut received = Vec::new();
    tokio::time::timeout(SOON, tcp.read_to_end(&mut received))
        .await
        .expect("a silent peer is dropped")
        .ok();
    assert!(
        received.is_empty(),
        "no byte is ever written to a non-TLS peer"
    );
    eventually("counted", || server.stats.handshakes_timed_out() == 1).await;
    server.stop().await;
}
