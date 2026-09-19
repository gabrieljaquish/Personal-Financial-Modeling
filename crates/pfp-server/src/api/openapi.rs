//! The `OpenAPI` document, generated from the DTOs and handler annotations
//! (ADR-017). Snapshot-tested in `tests/openapi.rs`; a diff there is an API change.

use utoipa::openapi::security::{ApiKey, ApiKeyValue, SecurityScheme};
use utoipa::{Modify, OpenApi};

use super::dto;
use crate::session::COOKIE_NAME;

struct SessionSchemes;

impl Modify for SessionSchemes {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        let components = openapi.components.get_or_insert_with(Default::default);
        components.add_security_scheme(
            "sessionCookie",
            SecurityScheme::ApiKey(ApiKey::Cookie(ApiKeyValue::with_description(
                COOKIE_NAME,
                "Set by the bootstrap; HttpOnly. Useless without the proof.",
            ))),
        );
        components.add_security_scheme(
            "sessionProof",
            SecurityScheme::ApiKey(ApiKey::Header(ApiKeyValue::with_description(
                "X-PFP-Proof",
                "Returned by the bootstrap; kept in sessionStorage. Useless without the cookie.",
            ))),
        );
    }
}

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Personal Financial Modeling local API",
        description = "Served only on https://127.0.0.1:<port> to the application's own front end. \
Every operation is POST and requires exact Host, Origin and Sec-Fetch-Site headers. \
All money is integer cents.",
        version = "v1"
    ),
    paths(
        super::session::bootstrap,
        super::session::status,
        super::session::relaunch,
        super::tax::rate_schedule,
        super::tax::rate_schedule_export,
        super::assumptions::list,
    ),
    components(schemas(dto::CentsDto, dto::ErrorBody)),
    modifiers(&SessionSchemes)
)]
struct ApiDoc;

/// The document as JSON: keys sorted, pretty-printed, one trailing newline.
/// Deterministic, so two builds of the same source produce the same bytes.
#[must_use]
pub fn openapi_json() -> String {
    // Through `Value`, whose maps are `BTreeMap`s: every object's keys are sorted.
    let value = serde_json::to_value(ApiDoc::openapi()).unwrap_or_default();
    let mut text = serde_json::to_string_pretty(&value).unwrap_or_default();
    text.push('\n');
    text
}
