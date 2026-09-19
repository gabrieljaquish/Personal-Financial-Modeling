//! Every numeric limit of the transport, as a named constant.
//!
//! The limits bound memory, tasks and file descriptors; they are not an
//! availability guarantee against a same-user local process (threat T3), which
//! could equally signal the server.

use std::time::Duration;

/// How long a peer may take to complete the TLS handshake before the socket is
/// dropped. A plaintext client that sends nothing, or a stalled handshake, is
/// released after this long.
pub const TLS_HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);

/// Connections that may exist at once. The permit is taken **before** the TLS
/// handshake and held for the life of the connection, so half-open handshakes
/// count. A browser uses at most six HTTP/1.1 connections per origin.
pub const MAX_CONNECTIONS: usize = 64;

/// The pause after a failed `accept(2)` (for example `EMFILE`), so a persistent
/// error cannot spin the accept loop.
pub const ACCEPT_ERROR_BACKOFF: Duration = Duration::from_millis(50);

/// How long hyper waits for a complete request head. The timer is armed whenever
/// a connection waits for a request, so it is also the keep-alive idle timeout.
pub const HEADER_READ_TIMEOUT: Duration = Duration::from_secs(10);

/// How long a kept-alive connection may sit idle between requests. Enforced by
/// the same hyper timer as [`HEADER_READ_TIMEOUT`].
pub const KEEPALIVE_IDLE: Duration = HEADER_READ_TIMEOUT;

/// The largest request head hyper buffers. Set explicitly: hyper's inherited
/// default is several times larger. Above it hyper answers a bare 431.
pub const MAX_HEADER_BYTES: usize = 64 * 1024;

/// The most header fields one request may carry.
pub const MAX_HEADER_COUNT: usize = 64;

/// The largest request body any route reads (`SECURITY.md` §7.2).
pub const MAX_BODY_BYTES: usize = 1024 * 1024;

/// The whole-request deadline, body included: a body dripped a byte at a time is
/// cut here with a 408 and `Connection: close`.
pub const REQUEST_DEADLINE: Duration = Duration::from_secs(30);

/// The most bytes read **and discarded** after a response that was sent before its
/// request body had been consumed (an early 413, 408, 431 or admission refusal),
/// so that a peer still uploading is not answered with a TCP reset before it has
/// read the refusal (`linger.rs`, RFC 9112 §9.6). Twice the body cap: an upload up
/// to three times the cap is refused cleanly; anything larger gets the reset.
pub const LINGER_MAX_BYTES: usize = 2 * MAX_BODY_BYTES;

/// The most time such a close may take, shutdown and drain together — not per
/// read, so a dripping peer cannot extend it. The connection keeps its permit for
/// this long, so it is far below [`REQUEST_DEADLINE`].
pub const LINGER_TIMEOUT: Duration = Duration::from_secs(2);

/// Lifetime of a launch token (`SECURITY.md` §7.1), measured on the monotonic
/// session clock.
pub const LAUNCH_TOKEN_TTL: Duration = Duration::from_secs(60);

/// Idle time after which the auto-lock hook fires (`SECURITY.md` §5, M0 plumbing).
pub const IDLE_LOCK_AFTER: Duration = Duration::from_secs(15 * 60);

/// How often the server evaluates the idle condition.
pub const IDLE_CHECK_INTERVAL: Duration = Duration::from_secs(60);

/// The least time between two accepted `relaunch` calls.
pub const RELAUNCH_MIN_INTERVAL: Duration = Duration::from_secs(10);

/// The most `relaunch` calls one session may have accepted.
pub const RELAUNCH_MAX_PER_SESSION: u32 = 20;

/// Failed bootstraps after which one warning is emitted. They are counted, never
/// refused: a refusing limiter on an unauthenticated endpoint would let any local
/// process lock the user's own browser out of its one launch token.
pub const BOOTSTRAP_FAILURE_WARN_AT: u32 = 10;

/// Events the in-memory ring buffer keeps (`SECURITY.md` §8).
pub const EVENT_RING_CAPACITY: usize = 512;
