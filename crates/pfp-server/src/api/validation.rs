//! The validation report over HTTP: what the build embedded, or the explicit
//! statement that nothing was generated.

use std::sync::Arc;

use axum::extract::State;
use axum::Json;

use super::dto::{ErrorBody, ReportStateDto, ValidationReportResponse};
use crate::service::AppState;

/// The validation report compiled into this build, or the `not-generated` state.
#[utoipa::path(
    post,
    path = "/api/v1/validation/report",
    tag = "validation",
    security(("sessionCookie" = [], "sessionProof" = [])),
    responses(
        (status = 200, description = "The report, or `state: not-generated` with no report (a \
            debug build on a checkout where `cargo xtask validation-report` was not run; a \
            release build always carries one).", body = ValidationReportResponse),
        (status = 401, description = "`session_required`.", body = ErrorBody),
        (status = 409, description = "`session_cookie_displaced`.", body = ErrorBody),
    )
)]
pub(crate) async fn report(State(state): State<Arc<AppState>>) -> Json<ValidationReportResponse> {
    Json(match state.validation.as_deref() {
        Some(report) => ValidationReportResponse {
            state: ReportStateDto::Generated,
            report: Some(report.clone()),
        },
        None => ValidationReportResponse {
            state: ReportStateDto::NotGenerated,
            report: None,
        },
    })
}
