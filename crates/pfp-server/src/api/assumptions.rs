//! The Assumptions Registry: every parameter with its source, as-of date,
//! vintage, verification status, projection and rounding rule.

use std::sync::Arc;

use axum::extract::State;
use axum::Json;

use super::dto::{AssumptionsResponse, ErrorBody, VintageDto};
use crate::service::AppState;

/// Lists every vintage compiled into this build, with every table in it.
#[utoipa::path(
    post,
    path = "/api/v1/assumptions/list",
    tag = "assumptions",
    security(("sessionCookie" = [], "sessionProof" = [])),
    responses(
        (status = 200, description = "The registry.", body = AssumptionsResponse),
        (status = 401, description = "`session_required`.", body = ErrorBody),
        (status = 409, description = "`session_cookie_displaced`.", body = ErrorBody),
    )
)]
pub(crate) async fn list(State(state): State<Arc<AppState>>) -> Json<AssumptionsResponse> {
    Json(AssumptionsResponse {
        vintages: vec![VintageDto::from(state.vintage.as_ref())],
    })
}
