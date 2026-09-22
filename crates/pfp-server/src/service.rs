//! The request pipeline, in one function so the order cannot be rearranged by a
//! layering accident (`SECURITY.md` §7.2–§7.3). First failing rule wins:
//!
//! ```text
//! [L0] response headers — outermost: every response produced here gets them
//! [Lt] whole-request deadline                         -> 408 + Connection: close
//! [L1] Host  [L2] route class  [L3] Origin  [L4] Sec-Fetch-Site
//! [L5] method gate  [L6] body rules                   -> 421 / 403 / 405 / 413 / 415
//! [L7] session (API only; bootstrap exempt; relaunch needs the proof only)
//!                                                     -> 401 / 409
//! [L8] handler: the API router, or the embedded assets
//! ```
//!
//! Logging records the method name, the route **template** and the status — never
//! the path as sent, a header, a cookie, a token or a body.

use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::http::header::COOKIE;
use axum::http::{HeaderMap, HeaderValue, Method, Request, Response};
use axum::Router;
use pfp_params::Vintage;
use tower::ServiceExt;

use crate::admission::{admit, HeaderFact, RequestFacts, RouteClass};
use crate::api::report_dto::ValidationReport;
use crate::api::{self, BOOTSTRAP_PATH, RELAUNCH_PATH};
use crate::assets::AssetManifest;
use crate::error::ApiError;
use crate::events::{EventCode, EventLog};
use crate::headers::{self, ResponseContext};
use crate::limits::MAX_HEADER_BYTES;
use crate::origin::CanonicalOrigin;
use crate::redact::Redacted;
use crate::session::{session_cookies, Presented, SessionCheck, SessionManager, PROOF_HEADER};
use crate::{RelaunchHook, TrustMode};

/// Everything a request needs. Shared, immutable but for the session manager and
/// the event log, which synchronize themselves.
pub(crate) struct AppState {
    pub(crate) origin: CanonicalOrigin,
    pub(crate) sessions: Arc<SessionManager>,
    pub(crate) events: Arc<EventLog>,
    pub(crate) vintage: Arc<Vintage>,
    /// The validation report the build embedded; `None` when none was generated.
    pub(crate) validation: Option<Arc<ValidationReport>>,
    pub(crate) trust_mode: TrustMode,
    pub(crate) relaunch: Option<Arc<dyn RelaunchHook>>,
    pub(crate) assets: AssetManifest,
    pub(crate) csp: HeaderValue,
    pub(crate) request_deadline: Duration,
}

impl AppState {
    /// `https://127.0.0.1:<port>/#t=<token>`: the token rides in the **fragment**,
    /// which is never sent to a server, logged by one, or put in a `Referer`.
    pub(crate) fn launch_url(&self, token: &Redacted<String>) -> Redacted<String> {
        Redacted::new(format!("{}/#t={}", self.origin.origin(), token.expose()))
    }
}

/// The application: the state plus the API router built over it.
#[derive(Clone)]
pub(crate) struct App {
    state: Arc<AppState>,
    router: Router,
}

fn method_name(method: &Method) -> &'static str {
    match *method {
        Method::GET => "GET",
        Method::HEAD => "HEAD",
        Method::POST => "POST",
        Method::PUT => "PUT",
        Method::PATCH => "PATCH",
        Method::DELETE => "DELETE",
        Method::OPTIONS => "OPTIONS",
        _ => "OTHER",
    }
}

fn route_template(class: RouteClass, path: &str) -> &'static str {
    match class {
        RouteClass::Api => api::route_template(path),
        RouteClass::Document => "/",
        RouteClass::Static if path.starts_with("/assets/") => "/assets/*",
        RouteClass::Static => "/*",
    }
}

/// The size of the request head as this layer sees it: the target plus every
/// header name and value, with the four bytes of `: ` and CRLF each line costs.
fn head_bytes<B>(request: &Request<B>) -> usize {
    let target = request
        .uri()
        .path_and_query()
        .map_or(0, |target| target.as_str().len());
    request
        .headers()
        .iter()
        .fold(target, |total, (name, value)| {
            total + name.as_str().len() + value.len() + 4
        })
}

/// [L7]. The values are borrowed from the request and go nowhere else.
fn session_gate(state: &AppState, path: &str, headers: &HeaderMap) -> Result<(), ApiError> {
    if path == BOOTSTRAP_PATH {
        return Ok(());
    }
    let proof = match HeaderFact::of(headers, &axum::http::HeaderName::from_static(PROOF_HEADER)) {
        HeaderFact::One(bytes) => std::str::from_utf8(bytes).ok(),
        HeaderFact::Absent | HeaderFact::Many => None,
    };
    if path == RELAUNCH_PATH {
        return if state.sessions.check_proof(proof) {
            Ok(())
        } else {
            state.events.record(EventCode::SessionRequired);
            Err(ApiError::SESSION_REQUIRED)
        };
    }
    let cookies = session_cookies(
        headers
            .get_all(COOKIE)
            .iter()
            .filter_map(|value| value.to_str().ok()),
    );
    match state.sessions.check(Presented {
        proof,
        cookies: &cookies,
    }) {
        SessionCheck::Ok => Ok(()),
        SessionCheck::Required => {
            state.events.record(EventCode::SessionRequired);
            Err(ApiError::SESSION_REQUIRED)
        }
        SessionCheck::CookieDisplaced => {
            state.events.record(EventCode::SessionCookieDisplaced);
            Err(ApiError::SESSION_COOKIE_DISPLACED)
        }
    }
}

impl App {
    pub(crate) fn new(state: Arc<AppState>) -> Self {
        let router = api::router(Arc::clone(&state));
        Self { state, router }
    }

    /// Answers one request. Generic over the body so that the unit tests can
    /// drive it without a socket; the server passes hyper's `Incoming`.
    pub(crate) async fn handle<B>(&self, request: Request<B>) -> Response<Body>
    where
        B: axum::body::HttpBody<Data = axum::body::Bytes> + Send + 'static,
        B::Error: Into<axum::BoxError>,
    {
        let state = &self.state;
        let path = request.uri().path().to_owned();
        let method = method_name(request.method());
        let with_body = request.method() != Method::HEAD;
        let may_set_cookie = request.method() == Method::POST && path == BOOTSTRAP_PATH;

        let admission = admit(&RequestFacts::of(&request), &state.origin);
        let class = admission.unwrap_or_else(|_| RouteClass::of(&path));
        let template = route_template(class, &path);

        let (mut response, code) = match admission {
            // hyper's own cap is on its read buffer and is checked between reads,
            // so a head somewhat over the cap can get through it. This check is
            // exact, and comes first: nothing about an oversized head is trusted.
            _ if head_bytes(&request) > MAX_HEADER_BYTES => (
                ApiError::HEADERS_TOO_LARGE.response(with_body),
                EventCode::RequestRefused,
            ),
            Err(refusal) => (
                refusal.error().response(with_body),
                EventCode::RequestRefused,
            ),
            Ok(class) => {
                let answer = tokio::time::timeout(
                    state.request_deadline,
                    self.dispatch(class, &path, with_body, request),
                );
                match answer.await {
                    Ok(response) => (response, EventCode::RequestServed),
                    Err(_) => (
                        ApiError::REQUEST_TIMEOUT.response(with_body),
                        EventCode::RequestDeadline,
                    ),
                }
            }
        };

        headers::apply(
            &mut response,
            ResponseContext {
                path: &path,
                admitted: admission.is_ok() && code != EventCode::RequestRefused,
                may_set_cookie,
            },
            &state.csp,
        );
        state
            .events
            .record_request(code, method, template, response.status().as_u16());
        response
    }

    async fn dispatch<B>(
        &self,
        class: RouteClass,
        path: &str,
        with_body: bool,
        request: Request<B>,
    ) -> Response<Body>
    where
        B: axum::body::HttpBody<Data = axum::body::Bytes> + Send + 'static,
        B::Error: Into<axum::BoxError>,
    {
        match class {
            RouteClass::Document | RouteClass::Static => {
                self.state
                    .assets
                    .respond(path, !with_body, request.headers())
            }
            RouteClass::Api => {
                if let Err(error) = session_gate(&self.state, path, request.headers()) {
                    return error.response(with_body);
                }
                match self.router.clone().oneshot(request.map(Body::new)).await {
                    Ok(response) => response,
                    Err(never) => match never {},
                }
            }
        }
    }
}
