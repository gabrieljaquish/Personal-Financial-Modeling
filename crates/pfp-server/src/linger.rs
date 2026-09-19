//! The bounded graceful close for a response sent before the request body was
//! consumed (RFC 9112 §9.6).
//!
//! A refusal is often decided from the request head alone — 413 on a declared
//! length, 403/421 from admission, 401 from the session gate — or part-way through
//! a body (413 on a chunked body, 408 at the deadline). The peer may still be
//! writing that body. If the socket were closed at once, the bytes still arriving
//! would be answered with a TCP reset: the peer's write fails with `EPIPE` or
//! `ECONNRESET`, and a client that treats a failed upload as a network error never
//! looks at the refusal it was sent.
//!
//! So after such a response the connection is closed in three steps: the response
//! is flushed, the write half is shut down (TLS `close_notify`, then FIN), and what
//! the peer still sends is read and **discarded** until it closes its side.
//!
//! That read is a denial-of-service surface — it holds one of
//! [`MAX_CONNECTIONS`](crate::limits::MAX_CONNECTIONS) permits and spends CPU on
//! TLS records nobody wants — so it is bounded twice, by
//! [`LINGER_MAX_BYTES`](crate::limits::LINGER_MAX_BYTES) and by
//! [`LINGER_TIMEOUT`](crate::limits::LINGER_TIMEOUT) for the whole close, never per
//! read. A peer that exceeds either bound gets the reset it would have got anyway.
//! Nothing that is drained is parsed, logged or kept.

use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;

use axum::body::{Bytes, HttpBody};
use hyper::body::{Frame, SizeHint};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::limits::{LINGER_MAX_BYTES, LINGER_TIMEOUT};

/// The size of the discard buffer: one TLS record's worth of plaintext.
const SINK_BYTES: usize = 16 * 1024;

/// The bounds of one graceful close. `Default` is the production value of each.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LingerLimits {
    /// See [`LINGER_TIMEOUT`]. Covers the shutdown **and** the drain together.
    pub timeout: Duration,
    /// See [`LINGER_MAX_BYTES`].
    pub max_bytes: usize,
}

impl Default for LingerLimits {
    fn default() -> Self {
        Self {
            timeout: LINGER_TIMEOUT,
            max_bytes: LINGER_MAX_BYTES,
        }
    }
}

/// Set, per connection, when a response was produced while its request body was
/// still unconsumed — the one case in which the peer may still be writing.
#[derive(Debug, Clone, Default)]
pub(crate) struct EarlyResponse(Arc<AtomicBool>);

impl EarlyResponse {
    pub(crate) fn mark(&self) {
        self.0.store(true, Ordering::Relaxed);
    }

    pub(crate) fn happened(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}

/// Whether a request body was read to its end. Shared between the body handed to
/// the pipeline and the connection's service, which looks at it once the response
/// exists (the request, and the body with it, is gone by then).
#[derive(Debug, Clone)]
pub(crate) struct Consumed(Arc<AtomicBool>);

impl Consumed {
    pub(crate) fn get(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}

/// A request body that records reaching its end. It changes nothing else: frames,
/// errors and the size hint pass through.
#[derive(Debug)]
pub(crate) struct WatchedBody<B> {
    inner: B,
    consumed: Consumed,
}

impl<B: HttpBody + Unpin> WatchedBody<B> {
    /// A body that is already at its end (no body at all, or `Content-Length: 0`)
    /// starts out consumed.
    pub(crate) fn new(inner: B) -> (Self, Consumed) {
        let consumed = Consumed(Arc::new(AtomicBool::new(inner.is_end_stream())));
        (
            Self {
                inner,
                consumed: consumed.clone(),
            },
            consumed,
        )
    }
}

impl<B> HttpBody for WatchedBody<B>
where
    B: HttpBody<Data = Bytes> + Unpin,
{
    type Data = Bytes;
    type Error = B::Error;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Bytes>, B::Error>>> {
        let polled = Pin::new(&mut self.inner).poll_frame(cx);
        let ended = match &polled {
            Poll::Ready(None) => true,
            Poll::Ready(Some(Ok(_))) => self.inner.is_end_stream(),
            Poll::Ready(Some(Err(_))) | Poll::Pending => false,
        };
        if ended {
            self.consumed.0.store(true, Ordering::Relaxed);
        }
        polled
    }

    fn is_end_stream(&self) -> bool {
        self.inner.is_end_stream()
    }

    fn size_hint(&self) -> SizeHint {
        self.inner.size_hint()
    }
}

/// How a graceful close ended. Reported to tests; production ignores it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LingerOutcome {
    /// The write half was shut down and no drain was asked for.
    ShutDown,
    /// The peer closed its side: nothing was reset.
    PeerClosed,
    /// The byte budget ran out with the peer still sending.
    ByteBudget,
    /// The time budget ran out.
    TimeBudget,
    /// The stream failed (a reset, a bad TLS record).
    Failed,
}

/// Shuts down the write half of `stream` and, when `drain` is set, discards what
/// the peer still sends — at most `limits.max_bytes` (of which `already_buffered`
/// were read ahead by the HTTP layer) and for at most `limits.timeout` in all.
/// The stream is dropped, and so closed, on return.
pub(crate) async fn close<S>(
    mut stream: S,
    drain: bool,
    already_buffered: usize,
    limits: LingerLimits,
) -> LingerOutcome
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let deadline = tokio::time::Instant::now() + limits.timeout;
    match tokio::time::timeout_at(deadline, stream.shutdown()).await {
        Ok(Ok(())) => {}
        Ok(Err(_)) => return LingerOutcome::Failed,
        Err(_) => return LingerOutcome::TimeBudget,
    }
    if !drain {
        return LingerOutcome::ShutDown;
    }
    let mut remaining = limits.max_bytes.saturating_sub(already_buffered);
    let mut sink = vec![0_u8; SINK_BYTES];
    while remaining > 0 {
        let want = remaining.min(sink.len());
        match tokio::time::timeout_at(deadline, stream.read(&mut sink[..want])).await {
            Ok(Ok(0)) => return LingerOutcome::PeerClosed,
            Ok(Ok(read)) => remaining = remaining.saturating_sub(read),
            Ok(Err(_)) => return LingerOutcome::Failed,
            Err(_) => return LingerOutcome::TimeBudget,
        }
    }
    LingerOutcome::ByteBudget
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::duplex;

    const LIMITS: LingerLimits = LingerLimits {
        timeout: Duration::from_millis(200),
        max_bytes: 4096,
    };

    #[tokio::test]
    async fn a_peer_that_finishes_within_the_budget_is_not_cut_off() {
        let (server, mut peer) = duplex(64);
        let writer = tokio::spawn(async move {
            // Still writing after the server has shut its side down.
            let result = peer.write_all(&[b'x'; 3000]).await;
            drop(peer);
            result
        });
        assert_eq!(
            close(server, true, 0, LIMITS).await,
            LingerOutcome::PeerClosed
        );
        assert!(writer.await.expect("join").is_ok(), "every write succeeded");
    }

    #[tokio::test]
    async fn the_byte_budget_is_a_hard_stop() {
        let (server, mut peer) = duplex(64);
        let writer = tokio::spawn(async move {
            let mut written = 0_usize;
            while peer.write_all(&[b'x'; 512]).await.is_ok() {
                written += 512;
            }
            written
        });
        assert_eq!(
            close(server, true, 0, LIMITS).await,
            LingerOutcome::ByteBudget
        );
        // The duplex pipe holds 64 bytes more than were read.
        let written = writer.await.expect("join");
        assert!(written <= LIMITS.max_bytes + 512 + 64, "drained {written}");
    }

    #[tokio::test]
    async fn bytes_the_http_layer_read_ahead_count_against_the_budget() {
        let (server, mut peer) = duplex(64);
        let writer = tokio::spawn(async move {
            let mut written = 0_usize;
            while peer.write_all(&[b'x'; 64]).await.is_ok() {
                written += 64;
            }
            written
        });
        assert_eq!(
            close(server, true, LIMITS.max_bytes - 128, LIMITS).await,
            LingerOutcome::ByteBudget
        );
        assert!(writer.await.expect("join") <= 128 + 64 + 64);
    }

    #[tokio::test(start_paused = true)]
    async fn the_time_budget_covers_the_whole_close_not_each_read() {
        let (server, mut peer) = duplex(64);
        // One byte every 50 ms: each read is prompt, the close as a whole is not.
        let dripper = tokio::spawn(async move {
            loop {
                if peer.write_all(b"x").await.is_err() {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        });
        let started = tokio::time::Instant::now();
        assert_eq!(
            close(server, true, 0, LIMITS).await,
            LingerOutcome::TimeBudget
        );
        assert_eq!(started.elapsed(), LIMITS.timeout);
        dripper.await.expect("join");
    }

    #[tokio::test(start_paused = true)]
    async fn a_silent_peer_is_released_at_the_time_budget() {
        let (server, _peer) = duplex(64);
        let started = tokio::time::Instant::now();
        assert_eq!(
            close(server, true, 0, LIMITS).await,
            LingerOutcome::TimeBudget
        );
        assert_eq!(started.elapsed(), LIMITS.timeout);
    }

    #[tokio::test]
    async fn without_an_early_response_nothing_is_read() {
        let (server, mut peer) = duplex(64);
        peer.write_all(b"unread").await.expect("write");
        assert_eq!(
            close(server, false, 0, LIMITS).await,
            LingerOutcome::ShutDown
        );
    }

    /// A body of several frames, so that "cut part-way" is expressible.
    struct Frames(std::collections::VecDeque<&'static [u8]>);

    impl HttpBody for Frames {
        type Data = Bytes;
        type Error = std::convert::Infallible;

        fn poll_frame(
            mut self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
        ) -> Poll<Option<Result<Frame<Bytes>, Self::Error>>> {
            Poll::Ready(
                self.0
                    .pop_front()
                    .map(|data| Ok(Frame::data(Bytes::from_static(data)))),
            )
        }

        fn is_end_stream(&self) -> bool {
            self.0.is_empty()
        }
    }

    async fn next_frame<B: HttpBody + Unpin>(
        body: &mut B,
    ) -> Option<Result<Frame<B::Data>, B::Error>> {
        std::future::poll_fn(|cx| Pin::new(&mut *body).poll_frame(cx)).await
    }

    #[tokio::test]
    async fn a_body_read_to_its_end_is_consumed() {
        let (mut body, consumed) = WatchedBody::new(Frames([&b"{"[..], &b"}"[..]].into()));
        assert!(!consumed.get());
        assert!(next_frame(&mut body).await.is_some());
        assert!(!consumed.get(), "one frame is still to come");
        assert!(next_frame(&mut body).await.is_some());
        assert!(consumed.get(), "the last frame ends the stream");
    }

    #[tokio::test]
    async fn an_absent_body_is_consumed_from_the_start() {
        let (_body, consumed) = WatchedBody::new(Frames([].into()));
        assert!(consumed.get());
    }

    #[tokio::test]
    async fn a_body_dropped_part_way_is_not_consumed() {
        let (body, consumed) = WatchedBody::new(Frames([&b"01"[..], &b"23"[..]].into()));
        drop(body);
        assert!(!consumed.get());

        // What the body cap does: it reads until it has seen too much, then stops.
        let (mut body, consumed) = WatchedBody::new(Frames([&b"01"[..], &b"23"[..]].into()));
        assert!(next_frame(&mut body).await.is_some());
        drop(body);
        assert!(!consumed.get());
    }
}
