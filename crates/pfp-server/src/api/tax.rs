//! The rate schedule over HTTP: `pfp_tax::schedule_tax_with` and its `Line` tree.

use std::sync::Arc;

use axum::extract::State;
use axum::Json;
use pfp_money::Cents;
use pfp_params::ParamView;
use pfp_tax::{line_id, schedule_tax_with, ScheduleError};

use super::dto::{
    ErrorBody, LineDto, RateScheduleRequest, RateScheduleResponse, VintageSummaryDto,
};
use super::ApiJson;
use crate::error::ApiError;
use crate::events::EventCode;
use crate::service::AppState;

/// The largest taxable income accepted: 2^53 − 1 cents, the largest integer a
/// JavaScript number holds exactly.
pub const MAX_TAXABLE_INCOME_CENTS: i64 = (1 << 53) - 1;

/// Computes the ordinary-income rate schedule and returns the whole worksheet.
#[utoipa::path(
    post,
    path = "/api/v1/tax/rate-schedule",
    tag = "tax",
    security(("sessionCookie" = [], "sessionProof" = [])),
    request_body = RateScheduleRequest,
    responses(
        (status = 200, description = "The worksheet.", body = RateScheduleResponse),
        (status = 400, description = "`invalid_json`.", body = ErrorBody),
        (status = 401, description = "`session_required`.", body = ErrorBody),
        (status = 409, description = "`session_cookie_displaced`.", body = ErrorBody),
        (status = 422, description = "`request_invalid` or `schedule_input_invalid`.", body = ErrorBody),
    )
)]
pub(crate) async fn rate_schedule(
    State(state): State<Arc<AppState>>,
    ApiJson(request): ApiJson<RateScheduleRequest>,
) -> Result<Json<RateScheduleResponse>, ApiError> {
    let income = request.taxable_income.0;
    if !(0..=MAX_TAXABLE_INCOME_CENTS).contains(&income) {
        return Err(ApiError::SCHEDULE_INPUT_INVALID);
    }
    let lines = schedule_tax_with(
        request.year,
        request.filing_status.into(),
        Cents(income),
        &ParamView::new(&state.vintage),
    )
    .map_err(|error| match error {
        // The caller's inputs: a year the vintage cannot serve, an amount the
        // arithmetic cannot hold.
        ScheduleError::NegativeTaxableIncome(_)
        | ScheduleError::Params(_)
        | ScheduleError::Money(_) => ApiError::SCHEDULE_INPUT_INVALID,
        // Defects of the build, not of the request. The detail stays out of the body.
        ScheduleError::Vintage(_)
        | ScheduleError::MalformedTable { .. }
        | ScheduleError::Id(_)
        | ScheduleError::Trace(_) => {
            state.events.record(EventCode::InternalError);
            ApiError::INTERNAL
        }
    })?;
    let Some(tax) = lines.value(&line_id::TAX) else {
        state.events.record(EventCode::InternalError);
        return Err(ApiError::INTERNAL);
    };
    Ok(Json(RateScheduleResponse {
        year: request.year,
        filing_status: request.filing_status,
        taxable_income: request.taxable_income,
        tax: tax.into(),
        root_line_id: line_id::TAX.as_str().to_owned(),
        lines: lines.iter().map(LineDto::from).collect(),
        vintage: VintageSummaryDto::from(state.vintage.as_ref()),
        verified: state.vintage.is_verified(),
    }))
}
