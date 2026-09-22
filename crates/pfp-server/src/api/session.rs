//! Session endpoints: bootstrap, status, relaunch (`SECURITY.md` §7.1).

use std::sync::Arc;

use axum::extract::State;
use axum::http::header::SET_COOKIE;
use axum::http::{HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;

use super::dto::{
    Accepted, BootstrapRequest, BootstrapResponse, ErrorBody, SessionStatus, VintageSummaryDto,
};
use super::ApiJson;
use crate::error::ApiError;
use crate::events::EventCode;
use crate::redact::Redacted;
use crate::service::AppState;
use crate::session::{set_cookie_value, BootstrapError, RelaunchError};

/// Exchanges the launch token for the session pair.
#[utoipa::path(
    post,
    path = "/api/v1/session/bootstrap",
    tag = "session",
    request_body = BootstrapRequest,
    responses(
        (status = 200, description = "Session established. Sets the `__Host-pfp` cookie \
            (`Secure; HttpOnly; SameSite=Strict; Path=/`).", body = BootstrapResponse),
        (status = 401, description = "`launch_token_invalid`: wrong, expired or already used.", body = ErrorBody),
    )
)]
pub(crate) async fn bootstrap(
    State(state): State<Arc<AppState>>,
    ApiJson(request): ApiJson<BootstrapRequest>,
) -> Result<Response, ApiError> {
    let established = state
        .sessions
        .bootstrap(&request.token)
        .map_err(|error| match error {
            BootstrapError::Invalid => ApiError::LAUNCH_TOKEN_INVALID,
            BootstrapError::Entropy => ApiError::INTERNAL,
        })?;
    let mut cookie = HeaderValue::from_str(set_cookie_value(&established.cookie).expose())
        .map_err(|_| ApiError::INTERNAL)?;
    cookie.set_sensitive(true);
    let mut response = Json(BootstrapResponse {
        proof: established.proof.expose().clone(),
    })
    .into_response();
    response.headers_mut().insert(SET_COOKIE, cookie);
    Ok(response)
}

/// Health, versions and trust mode.
#[utoipa::path(
    post,
    path = "/api/v1/session/status",
    tag = "session",
    security(("sessionCookie" = [], "sessionProof" = [])),
    responses(
        (status = 200, description = "The session is live.", body = SessionStatus),
        (status = 401, description = "`session_required`.", body = ErrorBody),
        (status = 409, description = "`session_cookie_displaced`.", body = ErrorBody),
    )
)]
pub(crate) async fn status(State(state): State<Arc<AppState>>) -> Json<SessionStatus> {
    Json(SessionStatus {
        app_version: env!("CARGO_PKG_VERSION").to_owned(),
        licence: env!("CARGO_PKG_LICENSE").to_owned(),
        api_version: "v1".to_owned(),
        trust_mode: state.trust_mode.into(),
        vintages: vec![VintageSummaryDto::from(state.vintage.as_ref())],
    })
}

/// Asks the running application to re-open itself in the browser: the recovery
/// from a displaced cookie. Needs the proof only. The fresh launch token goes to
/// the opener inside the process and is never in this response.
#[utoipa::path(
    post,
    path = "/api/v1/session/relaunch",
    tag = "session",
    security(("sessionProof" = [])),
    responses(
        (status = 202, description = "A fresh launch URL was handed to the opener.", body = Accepted),
        (status = 401, description = "`session_required`.", body = ErrorBody),
        (status = 429, description = "`relaunch_throttled`: inside the minimum interval, try again later. `relaunch_exhausted`: the per-session cap is used up and waiting does not help.", body = ErrorBody),
        (status = 503, description = "`open_unavailable`: this build has no opener.", body = ErrorBody),
    )
)]
pub(crate) async fn relaunch(State(state): State<Arc<AppState>>) -> Result<Response, ApiError> {
    let Some(hook) = state.relaunch.as_ref() else {
        state.events.record(EventCode::RelaunchUnavailable);
        return Err(ApiError::OPEN_UNAVAILABLE);
    };
    let token = state.sessions.relaunch().map_err(|error| match error {
        RelaunchError::Throttled => ApiError::RELAUNCH_THROTTLED,
        RelaunchError::Exhausted => ApiError::RELAUNCH_EXHAUSTED,
        RelaunchError::Entropy => ApiError::INTERNAL,
    })?;
    let url: Redacted<String> = state.launch_url(&token);
    if hook.reopen(&url).is_err() {
        state.events.record(EventCode::RelaunchUnavailable);
        return Err(ApiError::OPEN_UNAVAILABLE);
    }
    state.events.record(EventCode::RelaunchAccepted);
    Ok((StatusCode::ACCEPTED, Json(Accepted {})).into_response())
}
