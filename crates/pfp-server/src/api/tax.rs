//! The rate schedule over HTTP: `pfp_tax::schedule_tax_with` and its `Line` tree.

use std::sync::Arc;

use axum::body::Body;
use axum::extract::State;
use axum::http::header::{CONTENT_DISPOSITION, CONTENT_LENGTH, CONTENT_TYPE};
use axum::http::{HeaderValue, Response};
use axum::Json;
use pfp_money::Cents;
use pfp_params::ParamView;
use pfp_tax::{line_id, schedule_tax_with, ScheduleError};

use super::dto::{
    ErrorBody, LineDto, RateScheduleExportRequest, RateScheduleRequest, RateScheduleResponse,
    VintageSummaryDto,
};
use super::export::{rate_schedule_attachment, Attachment, ExportError};
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
    compute(&state, &request).map(Json)
}

/// The one computation behind both the worksheet and its export, so the two
/// cannot disagree.
fn compute(
    state: &AppState,
    request: &RateScheduleRequest,
) -> Result<RateScheduleResponse, ApiError> {
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
    Ok(RateScheduleResponse {
        year: request.year,
        filing_status: request.filing_status,
        taxable_income: request.taxable_income,
        tax: tax.into(),
        root_line_id: line_id::TAX.as_str().to_owned(),
        lines: lines.iter().map(LineDto::from).collect(),
        vintage: VintageSummaryDto::from(state.vintage.as_ref()),
        verified: state.vintage.is_verified(),
    })
}

/// Exports the rate-schedule worksheet as a CSV or JSON attachment.
///
/// The worksheet is recomputed from the inputs; the client never posts lines
/// back. CSV text cells carry spreadsheet formula-injection escaping and amounts
/// are bare numbers; the JSON is the worksheet's values verbatim.
#[utoipa::path(
    post,
    path = "/api/v1/tax/rate-schedule/export",
    tag = "tax",
    security(("sessionCookie" = [], "sessionProof" = [])),
    request_body = RateScheduleExportRequest,
    responses(
        (status = 200, description = "The worksheet as an attachment, in the requested format.", content(
            (String = "text/csv"),
            (String = "application/json"),
        )),
        (status = 400, description = "`invalid_json`.", body = ErrorBody),
        (status = 401, description = "`session_required`.", body = ErrorBody),
        (status = 409, description = "`session_cookie_displaced`.", body = ErrorBody),
        (status = 422, description = "`request_invalid` or `schedule_input_invalid`.", body = ErrorBody),
    )
)]
pub(crate) async fn rate_schedule_export(
    State(state): State<Arc<AppState>>,
    ApiJson(request): ApiJson<RateScheduleExportRequest>,
) -> Result<Response<Body>, ApiError> {
    let worksheet = compute(
        &state,
        &RateScheduleRequest {
            year: request.year,
            filing_status: request.filing_status,
            taxable_income: request.taxable_income,
        },
    )?;
    let built = rate_schedule_attachment(request.format, &worksheet);
    if built.is_err() {
        state.events.record(EventCode::InternalError);
    }
    attachment_response(built)
}

/// The response for an export. It FAILS CLOSED: a body that could not be built is
/// the API's standard `500` error body, never `200` with an empty attachment. The
/// success headers live inside the [`Attachment`], so they cannot be sent without one.
fn attachment_response(built: Result<Attachment, ExportError>) -> Result<Response<Body>, ApiError> {
    let attachment = built.map_err(|ExportError| ApiError::INTERNAL)?;
    // Every header value is a literal: nothing from the request reaches a header.
    let length = attachment.body.len();
    let mut response = Response::new(Body::from(attachment.body));
    let headers = response.headers_mut();
    headers.insert(
        CONTENT_TYPE,
        HeaderValue::from_static(attachment.content_type),
    );
    headers.insert(
        CONTENT_DISPOSITION,
        HeaderValue::from_static(attachment.disposition),
    );
    headers.insert(CONTENT_LENGTH, HeaderValue::from(length));
    Ok(response)
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;

    use super::*;
    use crate::api::export::{json_attachment, JSON_DISPOSITION};

    #[test]
    fn an_export_that_cannot_be_built_is_the_standard_500_body_and_never_an_attachment() {
        let error = attachment_response(Err(ExportError)).expect_err("fails closed");
        assert_eq!(error, ApiError::INTERNAL);
        let response = error.response(true);
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(
            response.headers().get(CONTENT_TYPE).expect("content type"),
            "application/json"
        );
        assert!(response.headers().get(CONTENT_DISPOSITION).is_none());
        assert_ne!(
            response.headers().get(CONTENT_LENGTH).expect("length"),
            "0",
            "the error body is the API's standard one, not an empty file"
        );
    }

    #[test]
    fn a_built_export_is_a_200_attachment_whose_length_is_its_body() {
        let built = json_attachment(&serde_json::json!({ "tax": 1 }));
        let length = built.as_ref().expect("serializes").body.len();
        let response = attachment_response(built).expect("succeeds");
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get(CONTENT_DISPOSITION)
                .expect("disposition"),
            JSON_DISPOSITION
        );
        assert_eq!(
            response.headers().get(CONTENT_LENGTH).expect("length"),
            &HeaderValue::from(length)
        );
    }
}
