//! Refusals and failures as JSON: `{"code": "...", "message": "..."}`.
//!
//! Every code and every message is a compile-time string. A body never echoes a
//! request value, an engine error's text, a path or a header: whoever reads it
//! learns which rule refused the request and nothing about the request itself or
//! the server's state.

use axum::body::Body;
use axum::http::header::{ALLOW, CONNECTION, CONTENT_LENGTH, CONTENT_TYPE};
use axum::http::{HeaderValue, Response, StatusCode};
use axum::response::IntoResponse;

/// A refusal or failure with a stable code and a fixed message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ApiError {
    status: StatusCode,
    code: &'static str,
    message: &'static str,
    allow: Option<&'static str>,
    close: bool,
}

impl ApiError {
    const fn new(status: StatusCode, code: &'static str, message: &'static str) -> Self {
        Self {
            status,
            code,
            message,
            allow: None,
            close: false,
        }
    }

    /// 421: the `Host` header is not the canonical one (DNS-rebinding defence).
    pub const MISDIRECTED_HOST: Self = Self::new(
        StatusCode::MISDIRECTED_REQUEST,
        "misdirected_host",
        "This server answers only to its own loopback address.",
    );
    /// 403: the `Origin` header is missing where required, or foreign.
    pub const ORIGIN_FORBIDDEN: Self = Self::new(
        StatusCode::FORBIDDEN,
        "origin_forbidden",
        "Requests from another origin are refused.",
    );
    /// 403: `Sec-Fetch-Site` does not allow this request on this route.
    pub const FETCH_SITE_FORBIDDEN: Self = Self::new(
        StatusCode::FORBIDDEN,
        "fetch_site_forbidden",
        "Cross-site requests are refused.",
    );
    /// 405 on a document or static route.
    pub const METHOD_NOT_ALLOWED: Self = Self {
        allow: Some("GET, HEAD"),
        ..Self::new(
            StatusCode::METHOD_NOT_ALLOWED,
            "method_not_allowed",
            "This method is not allowed here.",
        )
    };
    /// 405 on an API route.
    pub const API_METHOD_NOT_ALLOWED: Self = Self {
        allow: Some("POST"),
        ..Self::METHOD_NOT_ALLOWED
    };
    /// 431: the request head is larger than the cap.
    pub const HEADERS_TOO_LARGE: Self = Self {
        close: true,
        ..Self::new(
            StatusCode::REQUEST_HEADER_FIELDS_TOO_LARGE,
            "headers_too_large",
            "The request headers are too large.",
        )
    };
    /// 413: the body is larger than the cap.
    pub const PAYLOAD_TOO_LARGE: Self = Self::new(
        StatusCode::PAYLOAD_TOO_LARGE,
        "payload_too_large",
        "The request body is too large.",
    );
    /// 415: a body that is not `application/json`.
    pub const UNSUPPORTED_MEDIA_TYPE: Self = Self::new(
        StatusCode::UNSUPPORTED_MEDIA_TYPE,
        "unsupported_media_type",
        "Request bodies must be application/json.",
    );
    /// 408: the whole-request deadline passed. Closes the connection.
    pub const REQUEST_TIMEOUT: Self = Self {
        close: true,
        ..Self::new(
            StatusCode::REQUEST_TIMEOUT,
            "request_timeout",
            "The request took too long.",
        )
    };
    /// 401: no live session, or the proof is missing or wrong.
    pub const SESSION_REQUIRED: Self = Self::new(
        StatusCode::UNAUTHORIZED,
        "session_required",
        "A session is required. Open the application again from its launcher.",
    );
    /// 409: the proof is right but the cookie is missing, wrong or duplicated.
    pub const SESSION_COOKIE_DISPLACED: Self = Self::new(
        StatusCode::CONFLICT,
        "session_cookie_displaced",
        "This browser's session cookie was displaced by another local site.",
    );
    /// 401: the launch token is wrong, expired or already used.
    pub const LAUNCH_TOKEN_INVALID: Self = Self::new(
        StatusCode::UNAUTHORIZED,
        "launch_token_invalid",
        "The launch token is not valid. Open the application again from its launcher.",
    );
    /// 429: the relaunch interval refused the call; a later call can succeed.
    pub const RELAUNCH_THROTTLED: Self = Self::new(
        StatusCode::TOO_MANY_REQUESTS,
        "relaunch_throttled",
        "Re-opening was requested too often. Wait a moment and try again.",
    );
    /// 429: the session has used up its relaunches. Distinct from
    /// [`Self::RELAUNCH_THROTTLED`] because waiting does not help.
    pub const RELAUNCH_EXHAUSTED: Self = Self::new(
        StatusCode::TOO_MANY_REQUESTS,
        "relaunch_exhausted",
        "This session cannot re-open the application again. Quit it and start it again from its launcher.",
    );
    /// 503: this build cannot open a browser window.
    pub const OPEN_UNAVAILABLE: Self = Self::new(
        StatusCode::SERVICE_UNAVAILABLE,
        "open_unavailable",
        "This build cannot re-open the application. Start it again from its launcher.",
    );
    /// 400: the body is not JSON.
    pub const INVALID_JSON: Self = Self::new(
        StatusCode::BAD_REQUEST,
        "invalid_json",
        "The request body is not valid JSON.",
    );
    /// 422: the body is JSON of the wrong shape.
    pub const REQUEST_INVALID: Self = Self::new(
        StatusCode::UNPROCESSABLE_ENTITY,
        "request_invalid",
        "The request body does not have the expected fields.",
    );
    /// 422: the rate-schedule inputs are outside what the schedule defines.
    pub const SCHEDULE_INPUT_INVALID: Self = Self::new(
        StatusCode::UNPROCESSABLE_ENTITY,
        "schedule_input_invalid",
        "The year, filing status or taxable income is outside the supported range.",
    );
    /// 404, JSON.
    pub const NOT_FOUND: Self = Self::new(
        StatusCode::NOT_FOUND,
        "not_found",
        "There is nothing at this address.",
    );
    /// 500: a defect; the detail is an event code, never in the body.
    pub const INTERNAL: Self = Self::new(
        StatusCode::INTERNAL_SERVER_ERROR,
        "internal_error",
        "The server could not complete the request.",
    );

    /// The HTTP status.
    #[must_use]
    pub const fn status(&self) -> StatusCode {
        self.status
    }

    /// The stable code.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        self.code
    }

    /// The fixed message.
    #[must_use]
    pub const fn message(&self) -> &'static str {
        self.message
    }

    /// The response, with or without its body (`HEAD`).
    #[must_use]
    pub fn response(&self, with_body: bool) -> Response<Body> {
        // Both strings are compile-time constants without quotes or control
        // characters, so `serde_json` cannot fail and needs no fallback of substance.
        let body = serde_json::to_vec(&serde_json::json!({
            "code": self.code,
            "message": self.message,
        }))
        .unwrap_or_default();
        let mut response = Response::new(if with_body {
            Body::from(body.clone())
        } else {
            Body::empty()
        });
        *response.status_mut() = self.status;
        let headers = response.headers_mut();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert(CONTENT_LENGTH, HeaderValue::from(body.len()));
        if let Some(allow) = self.allow {
            headers.insert(ALLOW, HeaderValue::from_static(allow));
        }
        if self.close {
            headers.insert(CONNECTION, HeaderValue::from_static("close"));
        }
        response
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response<Body> {
        self.response(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_405s_name_their_methods_and_the_408_closes() {
        let r = ApiError::METHOD_NOT_ALLOWED.response(true);
        assert_eq!(r.headers()[ALLOW], "GET, HEAD");
        let r = ApiError::API_METHOD_NOT_ALLOWED.response(true);
        assert_eq!(r.headers()[ALLOW], "POST");
        let r = ApiError::REQUEST_TIMEOUT.response(true);
        assert_eq!(r.headers()[CONNECTION], "close");
        assert_eq!(r.status(), StatusCode::REQUEST_TIMEOUT);
    }
}
