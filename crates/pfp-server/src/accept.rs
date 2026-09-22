//! The accept loop: the only place a socket is accepted, and TLS-only by type.
//!
//! The per-connection handler receives a
//! [`TlsStream<TcpStream>`](tokio_rustls::server::TlsStream) by concrete type and
//! nothing else, so no code downstream of this module can be handed a plaintext
//! socket: there is no plaintext acceptor in **any** build profile (`SECURITY.md`
//! §6.1, test id S-02). A peer that does not complete a TLS 1.3 handshake — a
//! plaintext HTTP client included — gets a TLS alert or a closed socket, never an
//! HTTP response.

use std::fmt;
use std::future::Future;
use std::io;
use std::net::SocketAddrV4;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use rustls::ServerConfig;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use tokio_rustls::server::TlsStream;
use tokio_rustls::TlsAcceptor;

use crate::bind::{is_canonical_loopback, BindOutcome};
use crate::limits::{
    ACCEPT_ERROR_BACKOFF, HEADER_READ_TIMEOUT, MAX_CONNECTIONS, MAX_HEADER_BYTES, MAX_HEADER_COUNT,
    TLS_HANDSHAKE_TIMEOUT,
};
use crate::linger::{self, EarlyResponse, LingerLimits};

/// The limits hyper enforces on one connection. `Default` is the production value
/// of each.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HttpLimits {
    /// See [`HEADER_READ_TIMEOUT`]; also the keep-alive idle timeout.
    pub header_read_timeout: Duration,
    /// See [`MAX_HEADER_BYTES`].
    pub max_header_bytes: usize,
    /// See [`MAX_HEADER_COUNT`].
    pub max_header_count: usize,
    /// The bounds of the graceful close after an early response.
    pub linger: LingerLimits,
}

impl Default for HttpLimits {
    fn default() -> Self {
        Self {
            header_read_timeout: HEADER_READ_TIMEOUT,
            max_header_bytes: MAX_HEADER_BYTES,
            max_header_count: MAX_HEADER_COUNT,
            linger: LingerLimits::default(),
        }
    }
}

/// Speaks HTTP/1.1 on an established TLS stream. This is the **only** place in the
/// crate that drives hyper, and it takes the TLS stream by concrete type: there is
/// no way to hand it a plaintext socket, in any build profile (S-02).
///
/// Input that is not HTTP is answered by hyper itself with a bare 400 or 431 and
/// an empty body, below every layer of this crate (`tests/protocol_errors.rs`).
///
/// hyper is run **without** its own shutdown so that the close is ours: when the
/// service marked `early` — it answered while a request body was still unread —
/// the peer may still be uploading, and the close drains it within fixed bounds
/// rather than resetting it (`linger.rs`). A connection hyper ends with an error
/// (a reset, garbage, the header timer) is dropped as it always was.
pub(crate) async fn serve_http<S>(
    stream: TlsStream<TcpStream>,
    service: S,
    limits: HttpLimits,
    early: EarlyResponse,
) where
    S: hyper::service::HttpService<hyper::body::Incoming, ResBody = axum::body::Body>,
    S::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
{
    let mut http = hyper::server::conn::http1::Builder::new();
    http.timer(hyper_util::rt::TokioTimer::new())
        .header_read_timeout(limits.header_read_timeout)
        .max_buf_size(limits.max_header_bytes)
        .max_headers(limits.max_header_count)
        .keep_alive(true);
    // A connection error is the peer's business (reset, garbage, timeout); there
    // is nothing to report and nothing of it may be logged.
    let served = http
        .serve_connection(hyper_util::rt::TokioIo::new(stream), service)
        .without_shutdown()
        .await;
    if let Ok(parts) = served {
        let read_ahead = parts.read_buf.len();
        let stream = parts.io.into_inner();
        let _ = linger::close(stream, early.happened(), read_ahead, limits.linger).await;
    }
}

/// The limits the accept loop enforces. `Default` is the production value of each.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AcceptLimits {
    /// See [`TLS_HANDSHAKE_TIMEOUT`].
    pub handshake_timeout: Duration,
    /// See [`MAX_CONNECTIONS`].
    pub max_connections: usize,
}

impl Default for AcceptLimits {
    fn default() -> Self {
        Self {
            handshake_timeout: TLS_HANDSHAKE_TIMEOUT,
            max_connections: MAX_CONNECTIONS,
        }
    }
}

/// Counters of what the accept loop did. Numbers only: no address, no bytes.
#[derive(Debug, Default)]
pub struct AcceptStats {
    handshakes_completed: AtomicU64,
    handshakes_failed: AtomicU64,
    handshakes_timed_out: AtomicU64,
    refused_at_cap: AtomicU64,
    refused_peer: AtomicU64,
}

impl AcceptStats {
    /// Connections that completed a TLS 1.3 handshake and reached the handler.
    #[must_use]
    pub fn handshakes_completed(&self) -> u64 {
        self.handshakes_completed.load(Ordering::Relaxed)
    }

    /// Connections dropped because the handshake failed (plaintext bytes, TLS 1.2,
    /// a client that does not trust the certificate and says so, a reset).
    #[must_use]
    pub fn handshakes_failed(&self) -> u64 {
        self.handshakes_failed.load(Ordering::Relaxed)
    }

    /// Connections dropped because the handshake did not finish in time.
    #[must_use]
    pub fn handshakes_timed_out(&self) -> u64 {
        self.handshakes_timed_out.load(Ordering::Relaxed)
    }

    /// Connections dropped, before any TLS byte, because the cap was reached.
    #[must_use]
    pub fn refused_at_cap(&self) -> u64 {
        self.refused_at_cap.load(Ordering::Relaxed)
    }

    /// Connections dropped because the peer address was not `127.0.0.1`. Cannot
    /// happen on a socket bound to `127.0.0.1`; counted rather than assumed.
    #[must_use]
    pub fn refused_peer(&self) -> u64 {
        self.refused_peer.load(Ordering::Relaxed)
    }
}

fn bump(counter: &AtomicU64) {
    counter.fetch_add(1, Ordering::Relaxed);
}

/// The bound socket could not be handed to the runtime.
#[derive(Debug)]
pub struct ListenError(io::Error);

impl fmt::Display for ListenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "cannot start the listener: {}", self.0)
    }
}

impl std::error::Error for ListenError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.0)
    }
}

/// A bound loopback socket paired with a TLS acceptor. It has no way to yield a
/// plaintext stream.
pub struct TlsListener {
    listener: TcpListener,
    local_addr: SocketAddrV4,
    acceptor: TlsAcceptor,
    limits: AcceptLimits,
    stats: Arc<AcceptStats>,
}

impl fmt::Debug for TlsListener {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TlsListener")
            .field("local_addr", &self.local_addr)
            .field("limits", &self.limits)
            .finish_non_exhaustive()
    }
}

impl TlsListener {
    /// Wraps a socket bound by [`bind_loopback`](crate::bind::bind_loopback) — the
    /// only source of a [`BindOutcome`], which has already passed the loopback
    /// self-check, so a listener cannot be built around any other address. Must be
    /// called inside a Tokio runtime.
    ///
    /// # Errors
    /// [`ListenError`] when the socket cannot be registered with the runtime.
    pub fn new(
        bound: BindOutcome,
        tls: Arc<ServerConfig>,
        limits: AcceptLimits,
    ) -> Result<Self, ListenError> {
        let (listener, local_addr) = bound.into_parts();
        listener.set_nonblocking(true).map_err(ListenError)?;
        let listener = TcpListener::from_std(listener).map_err(ListenError)?;
        Ok(Self {
            listener,
            local_addr,
            acceptor: TlsAcceptor::from(tls),
            limits,
            stats: Arc::new(AcceptStats::default()),
        })
    }

    /// The checked local address: always `127.0.0.1:<port>`.
    #[must_use]
    pub fn local_addr(&self) -> SocketAddrV4 {
        self.local_addr
    }

    /// The counters, shareable with whoever reports them.
    #[must_use]
    pub fn stats(&self) -> Arc<AcceptStats> {
        Arc::clone(&self.stats)
    }

    /// Accepts connections until `shutdown` completes. Each accepted socket takes
    /// one of `max_connections` permits **before** the handshake (none left: the
    /// socket is dropped at once), must finish the TLS handshake within
    /// `handshake_timeout`, and only then reaches `handler`. The permit is held
    /// until the handler's future ends.
    ///
    /// When `shutdown` completes the listener closes and connection tasks still
    /// running are cancelled.
    pub async fn serve<H, Fut, S>(self, handler: H, shutdown: S)
    where
        H: Fn(TlsStream<TcpStream>) -> Fut + Clone + Send + 'static,
        Fut: Future<Output = ()> + Send + 'static,
        S: Future<Output = ()>,
    {
        let permits = Arc::new(Semaphore::new(self.limits.max_connections));
        let mut connections = JoinSet::new();
        tokio::pin!(shutdown);

        loop {
            let accepted = tokio::select! {
                biased;
                () = &mut shutdown => break,
                // Reap finished tasks so the set does not grow with every connection.
                Some(_) = connections.join_next(), if !connections.is_empty() => continue,
                accepted = self.listener.accept() => accepted,
            };
            let Ok((socket, peer)) = accepted else {
                // For example `EMFILE`; pause so a persistent error cannot spin the loop.
                tokio::time::sleep(ACCEPT_ERROR_BACKOFF).await;
                continue;
            };
            if !is_canonical_loopback(peer.ip()) {
                bump(&self.stats.refused_peer);
                continue;
            }
            let Ok(permit) = Arc::clone(&permits).try_acquire_owned() else {
                bump(&self.stats.refused_at_cap);
                continue;
            };

            let acceptor = self.acceptor.clone();
            let handler = handler.clone();
            let stats = Arc::clone(&self.stats);
            let handshake_timeout = self.limits.handshake_timeout;
            connections.spawn(async move {
                let _permit = permit;
                match tokio::time::timeout(handshake_timeout, acceptor.accept(socket)).await {
                    Ok(Ok(stream)) => {
                        bump(&stats.handshakes_completed);
                        handler(stream).await;
                    }
                    Ok(Err(_)) => bump(&stats.handshakes_failed),
                    Err(_) => bump(&stats.handshakes_timed_out),
                }
            });
        }
        // Dropping the set aborts what is still running; the listener closes with `self`.
        drop(connections);
    }
}
