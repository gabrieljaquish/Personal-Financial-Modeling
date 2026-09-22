//! The public surface: [`ServerConfig`] → [`Server::bind`] → [`BoundServer`].
//!
//! The configuration has **no bind-address field**: the listener is `127.0.0.1`
//! by construction (`bind.rs`), and there is no plaintext mode to select.

use std::fmt;
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use axum::http::Request;
use hyper::body::Incoming;
use hyper::service::service_fn;

use crate::accept::{serve_http, AcceptLimits, AcceptStats, HttpLimits, ListenError, TlsListener};
use crate::api::dto::TrustModeDto;
use crate::assets::AssetManifest;
use crate::bind::{bind_loopback, BindError};
use crate::csp::{self, CspError};
use crate::events::{EventCode, EventLog};
use crate::limits::{EVENT_RING_CAPACITY, IDLE_CHECK_INTERVAL, REQUEST_DEADLINE};
use crate::linger::{EarlyResponse, WatchedBody};
use crate::origin::CanonicalOrigin;
use crate::redact::Redacted;
use crate::service::{App, AppState};
use crate::session::{Clock, EntropyError, MonotonicClock, NoHooks, SessionHooks, SessionManager};
use crate::tls::{leaf_fingerprint, server_config, TlsConfigError, TlsMaterial};

/// Whether the local CA is trusted by the user's browsers; reported by the status
/// endpoint so the front end can explain a certificate warning.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrustMode {
    /// The CA was installed into the user's trust settings.
    Installed,
    /// Trust was declined, skipped or is unavailable on this platform.
    Declined,
}

impl From<TrustMode> for TrustModeDto {
    fn from(mode: TrustMode) -> Self {
        match mode {
            TrustMode::Installed => Self::Installed,
            TrustMode::Declined => Self::Declined,
        }
    }
}

/// The opener could not re-open the application.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReopenUnavailable;

/// The launcher's browser opener, as the server sees it: the only path from an
/// HTTP request to opening a window. The server throttles calls to it
/// (`limits::RELAUNCH_*`). A build without an opener passes `None` in
/// [`ServerConfig::relaunch`] and `relaunch` answers 503.
pub trait RelaunchHook: Send + Sync {
    /// Opens `launch_url` in the user's browser from inside the process. The URL
    /// carries a launch token in its fragment: it must never reach argv, the
    /// environment, a file, the clipboard or a log.
    ///
    /// # Errors
    /// [`ReopenUnavailable`] when nothing could be opened.
    fn reopen(&self, launch_url: &Redacted<String>) -> Result<(), ReopenUnavailable>;
}

/// Overrides for tests. Production code uses [`TestKnobs::default`]; none of these
/// can widen what the server accepts, only shorten a timer, lower a cap, or
/// substitute the session clock.
#[doc(hidden)]
#[derive(Clone)]
pub struct TestKnobs {
    pub accept: AcceptLimits,
    pub http: HttpLimits,
    pub request_deadline: Duration,
    pub clock: Arc<dyn Clock>,
    pub echo_events_to_stderr: bool,
}

impl Default for TestKnobs {
    fn default() -> Self {
        Self {
            accept: AcceptLimits::default(),
            http: HttpLimits::default(),
            request_deadline: REQUEST_DEADLINE,
            clock: Arc::new(MonotonicClock),
            echo_events_to_stderr: true,
        }
    }
}

/// What the launcher decides. Note what is absent: a bind address, a plaintext
/// switch, a CORS allowlist, a token.
pub struct ServerConfig {
    /// Try this port first, then an OS-assigned one. `None`: OS-assigned.
    pub preferred_port: Option<u16>,
    /// The CA certificate, the leaf and the leaf's key.
    pub material: TlsMaterial,
    /// The web bundle; [`AssetManifest::placeholder`] when none was embedded.
    pub assets: AssetManifest,
    /// Reported by the status endpoint.
    pub trust_mode: TrustMode,
    /// The browser opener, when this build has one.
    pub relaunch: Option<Arc<dyn RelaunchHook>>,
    /// What the launcher does when the session is replaced or goes idle.
    pub hooks: Arc<dyn SessionHooks>,
    #[doc(hidden)]
    pub test: TestKnobs,
}

impl ServerConfig {
    /// A configuration with no preferred port, the placeholder shell, declined
    /// trust, no opener and no hooks.
    #[must_use]
    pub fn new(material: TlsMaterial) -> Self {
        Self {
            preferred_port: None,
            material,
            assets: AssetManifest::placeholder(),
            trust_mode: TrustMode::Declined,
            relaunch: None,
            hooks: Arc::new(NoHooks),
            test: TestKnobs::default(),
        }
    }
}

impl fmt::Debug for ServerConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ServerConfig")
            .field("preferred_port", &self.preferred_port)
            .field("trust_mode", &self.trust_mode)
            .field("has_opener", &self.relaunch.is_some())
            .finish_non_exhaustive()
    }
}

/// Why the server did not start.
#[derive(Debug)]
pub enum ServerError {
    /// No loopback socket could be bound, or the bound address failed the self-check.
    Bind(BindError),
    /// The TLS material was refused by rustls.
    Tls(TlsConfigError),
    /// The socket could not be handed to the runtime.
    Listen(ListenError),
    /// The fixed CSP does not fit the embedded bundle.
    Csp(CspError),
    /// The embedded parameter vintage does not validate.
    Params(pfp_params::ParamError),
    /// The embedded validation report is not the shape this build reads.
    Report(crate::validation::ReportError),
}

impl fmt::Display for ServerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bind(e) => write!(f, "{e}"),
            Self::Tls(e) => write!(f, "{e}"),
            Self::Listen(e) => write!(f, "{e}"),
            Self::Csp(e) => write!(f, "the embedded web bundle was refused: {e}"),
            Self::Params(e) => write!(f, "the embedded parameters were refused: {e}"),
            Self::Report(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for ServerError {}

/// Entry point.
#[derive(Debug, Clone, Copy)]
pub struct Server;

impl Server {
    /// Binds `127.0.0.1`, builds the TLS acceptor and the application. Nothing is
    /// accepted until [`BoundServer::serve`]. Must be called inside a Tokio runtime.
    ///
    /// # Errors
    /// [`ServerError`]; startup refuses rather than degrade.
    pub fn bind(config: ServerConfig) -> Result<BoundServer, ServerError> {
        let csp = csp::derive(&config.assets).map_err(ServerError::Csp)?;
        let vintage = pfp_params::shipped::federal_2026().map_err(ServerError::Params)?;
        let validation = crate::validation::embedded().map_err(ServerError::Report)?;
        let tls = server_config(&config.material).map_err(ServerError::Tls)?;
        let fingerprint = leaf_fingerprint(&config.material.leaf_cert_der);

        let bound = bind_loopback(config.preferred_port).map_err(ServerError::Bind)?;
        let fell_back = bound.fell_back();
        let listener =
            TlsListener::new(bound, tls, config.test.accept).map_err(ServerError::Listen)?;
        let origin = CanonicalOrigin::of(listener.local_addr());

        let events = Arc::new(EventLog::with_capacity(
            EVENT_RING_CAPACITY,
            config.test.echo_events_to_stderr,
        ));
        let sessions = Arc::new(SessionManager::new(
            Arc::clone(&config.test.clock),
            Arc::clone(&config.hooks),
            Arc::clone(&events),
        ));
        let state = Arc::new(AppState {
            origin: origin.clone(),
            sessions: Arc::clone(&sessions),
            events: Arc::clone(&events),
            vintage: Arc::new(vintage),
            validation: validation.map(Arc::new),
            trust_mode: config.trust_mode,
            relaunch: config.relaunch,
            assets: config.assets,
            csp,
            request_deadline: config.test.request_deadline,
        });
        Ok(BoundServer {
            listener,
            app: App::new(Arc::clone(&state)),
            state,
            fingerprint,
            fell_back,
            http: config.test.http,
        })
    }
}

/// A bound, not yet serving, server.
pub struct BoundServer {
    listener: TlsListener,
    app: App,
    state: Arc<AppState>,
    fingerprint: String,
    fell_back: bool,
    http: HttpLimits,
}

impl fmt::Debug for BoundServer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BoundServer")
            .field("origin", &self.state.origin.origin())
            .finish_non_exhaustive()
    }
}

impl BoundServer {
    /// `https://127.0.0.1:<port>`.
    #[must_use]
    pub fn origin(&self) -> &CanonicalOrigin {
        &self.state.origin
    }

    /// SHA-256 of the leaf, for the status console and the decline-mode comparison.
    #[must_use]
    pub fn leaf_fingerprint(&self) -> &str {
        &self.fingerprint
    }

    /// Whether the preferred port was occupied and an OS-assigned one is in use.
    /// The launcher must warn the user **before** opening anything (S-29).
    #[must_use]
    pub const fn fell_back(&self) -> bool {
        self.fell_back
    }

    /// Mints a launch token (replacing any outstanding one) and returns the URL
    /// that carries it in its fragment. For the in-process browser opener only.
    ///
    /// # Errors
    /// [`EntropyError`].
    pub fn launch_url(&self) -> Result<Redacted<String>, EntropyError> {
        let token = self.state.sessions.mint_launch_token()?;
        Ok(self.state.launch_url(&token))
    }

    /// The event ring buffer: the real log.
    #[must_use]
    pub fn events(&self) -> Arc<EventLog> {
        Arc::clone(&self.state.events)
    }

    /// The session manager, for the launcher's interaction heartbeat.
    #[must_use]
    pub fn sessions(&self) -> Arc<SessionManager> {
        Arc::clone(&self.state.sessions)
    }

    /// The accept loop's counters.
    #[must_use]
    pub fn stats(&self) -> Arc<AcceptStats> {
        self.listener.stats()
    }

    /// Serves until `shutdown` completes, then closes the listener and cancels
    /// what is still running.
    pub async fn serve<S>(self, shutdown: S)
    where
        S: Future<Output = ()>,
    {
        let sessions = Arc::clone(&self.state.sessions);
        let idle = tokio::spawn(async move {
            let mut tick = tokio::time::interval(IDLE_CHECK_INTERVAL);
            loop {
                tick.tick().await;
                sessions.check_idle();
            }
        });

        let app = self.app;
        let http = self.http;
        self.listener
            .serve(
                move |stream| {
                    let app = app.clone();
                    async move {
                        let early = EarlyResponse::default();
                        let mark = early.clone();
                        let service = service_fn(move |request: Request<Incoming>| {
                            let app = app.clone();
                            let mark = mark.clone();
                            async move {
                                let (parts, body) = request.into_parts();
                                let (body, consumed) = WatchedBody::new(body);
                                let response = app.handle(Request::from_parts(parts, body)).await;
                                // The request is gone. A body it never finished
                                // reading is one the peer may still be sending.
                                if !consumed.get() {
                                    mark.mark();
                                }
                                Ok::<_, std::convert::Infallible>(response)
                            }
                        });
                        serve_http(stream, service, http, early).await;
                    }
                },
                shutdown,
            )
            .await;
        idle.abort();
        let _ = idle.await;
        self.state.events.record(EventCode::ServerStopped);
    }
}
