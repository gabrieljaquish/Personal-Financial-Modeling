//! The JSON API under `/api/v1` (`ARCHITECTURE.md` §5).
//!
//! Every M0 operation is `POST`, as a settled contract (README, "API methods"):
//! browsers omit `Origin` on same-origin `GET`, and `SECURITY.md` §7.2 requires
//! `Origin` on every `/api/**` request. How the `GET` reads that `ARCHITECTURE.md`
//! §5 draws for M2 are admitted is [`crate::admission::ApiReadRule`]'s subject;
//! it adds operations and does not move these.
//!
//! | Route | Session | Success |
//! |---|---|---|
//! | `/session/bootstrap` | none | 200 proof + `Set-Cookie` |
//! | `/session/status` | cookie + proof | 200 health and versions |
//! | `/session/relaunch` | proof only | 202; the token is never in the response |
//! | `/tax/rate-schedule` | cookie + proof | 200 the worksheet |
//! | `/assumptions/list` | cookie + proof | 200 the Assumptions Registry |
//!
//! Admission and the session gate run before this router (`service.rs`); handlers
//! see only admitted, authenticated requests.

pub mod assumptions;
pub mod dto;
pub mod openapi;
pub mod session;
pub mod tax;

use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, FromRequest, Request};
use axum::routing::post;
use axum::Router;
use serde::de::DeserializeOwned;

use crate::error::ApiError;
use crate::limits::MAX_BODY_BYTES;
use crate::service::AppState;

/// `POST /api/v1/session/bootstrap`.
pub const BOOTSTRAP_PATH: &str = "/api/v1/session/bootstrap";
/// `POST /api/v1/session/status`.
pub const STATUS_PATH: &str = "/api/v1/session/status";
/// `POST /api/v1/session/relaunch`.
pub const RELAUNCH_PATH: &str = "/api/v1/session/relaunch";
/// `POST /api/v1/tax/rate-schedule`.
pub const RATE_SCHEDULE_PATH: &str = "/api/v1/tax/rate-schedule";
/// `POST /api/v1/assumptions/list`.
pub const ASSUMPTIONS_PATH: &str = "/api/v1/assumptions/list";

/// Every API path, for route-template logging and the `OpenAPI` assertions.
pub const PATHS: [&str; 5] = [
    BOOTSTRAP_PATH,
    STATUS_PATH,
    RELAUNCH_PATH,
    RATE_SCHEDULE_PATH,
    ASSUMPTIONS_PATH,
];

/// The route **template** of a path, for the event log: a known path, or a
/// constant. The path as sent is never logged.
#[must_use]
pub fn route_template(path: &str) -> &'static str {
    PATHS
        .into_iter()
        .find(|known| *known == path)
        .unwrap_or("/api/**")
}

async fn method_not_allowed() -> ApiError {
    ApiError::API_METHOD_NOT_ALLOWED
}

async fn not_found() -> ApiError {
    ApiError::NOT_FOUND
}

pub(crate) fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route(
            BOOTSTRAP_PATH,
            post(session::bootstrap).fallback(method_not_allowed),
        )
        .route(
            STATUS_PATH,
            post(session::status).fallback(method_not_allowed),
        )
        .route(
            RELAUNCH_PATH,
            post(session::relaunch).fallback(method_not_allowed),
        )
        .route(
            RATE_SCHEDULE_PATH,
            post(tax::rate_schedule).fallback(method_not_allowed),
        )
        .route(
            ASSUMPTIONS_PATH,
            post(assumptions::list).fallback(method_not_allowed),
        )
        .fallback(not_found)
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
        .with_state(state)
}

/// A JSON request body whose every failure is an [`ApiError`] with a fixed
/// message: nothing of the body, and nothing of the parser's complaint, is echoed.
pub(crate) struct ApiJson<T>(pub(crate) T);

impl<S, T> FromRequest<S> for ApiJson<T>
where
    S: Send + Sync,
    T: DeserializeOwned,
{
    type Rejection = ApiError;

    async fn from_request(request: Request, state: &S) -> Result<Self, Self::Rejection> {
        // The only way reading fails on an admitted request is the body limit
        // (a chunked body longer than the cap); a vanished client sees nothing.
        let bytes = Bytes::from_request(request, state)
            .await
            .map_err(|_| ApiError::PAYLOAD_TOO_LARGE)?;
        serde_json::from_slice(&bytes).map(Self).map_err(|error| {
            if error.is_data() {
                ApiError::REQUEST_INVALID
            } else {
                ApiError::INVALID_JSON
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn templates_never_echo_an_unknown_path() {
        assert_eq!(route_template(STATUS_PATH), STATUS_PATH);
        assert_eq!(route_template("/api/v1/plan/secret-name"), "/api/**");
        assert!(PATHS.iter().all(|p| p.starts_with("/api/v1/")));
    }
}
