//! The exact response-header set (`SECURITY.md` §7.3, test id S-04).
//!
//! [`apply`] runs outermost, on **every response the service produces** — served,
//! refused or failed. (The bare 400/431 hyper writes by itself for input that is
//! not HTTP never reaches any layer; `tests/protocol_errors.rs` pins that class.)
//!
//! Beyond setting the policy headers it enforces the universal negatives: no
//! `Access-Control-*` header (CORS and Private Network Access are never approved,
//! under any condition), no `Location` (there is no redirect of any kind), no
//! `Server`, no `Vary`, and `Set-Cookie` only where the caller says the route is
//! the session bootstrap.

use axum::body::Body;
use axum::http::header::{
    HeaderName, CACHE_CONTROL, CONTENT_SECURITY_POLICY, LOCATION, REFERRER_POLICY, SERVER,
    SET_COOKIE, VARY, X_CONTENT_TYPE_OPTIONS,
};
use axum::http::{HeaderValue, Response, StatusCode};

const CROSS_ORIGIN_OPENER_POLICY: HeaderName =
    HeaderName::from_static("cross-origin-opener-policy");
const CROSS_ORIGIN_RESOURCE_POLICY: HeaderName =
    HeaderName::from_static("cross-origin-resource-policy");

const NO_STORE: HeaderValue = HeaderValue::from_static("no-store");
const IMMUTABLE: HeaderValue = HeaderValue::from_static("public, max-age=31536000, immutable");

/// What [`apply`] needs to know about the request the response answers.
#[derive(Debug, Clone, Copy)]
pub struct ResponseContext<'a> {
    /// The request path.
    pub path: &'a str,
    /// Whether the request was admitted. A refusal is never cacheable.
    pub admitted: bool,
    /// Whether this route may set the session cookie (the bootstrap, on a 200).
    pub may_set_cookie: bool,
}

/// Stamps the policy headers and strips what must never be sent.
pub fn apply(response: &mut Response<Body>, context: ResponseContext<'_>, csp: &HeaderValue) {
    let status = response.status();
    let headers = response.headers_mut();

    let forbidden: Vec<HeaderName> = headers
        .keys()
        .filter(|name| name.as_str().starts_with("access-control-"))
        .cloned()
        .collect();
    debug_assert!(forbidden.is_empty(), "a handler set a CORS header");
    for name in forbidden {
        headers.remove(name);
    }
    debug_assert!(!headers.contains_key(LOCATION), "a handler set Location");
    headers.remove(LOCATION);
    headers.remove(SERVER);
    headers.remove(VARY);
    if !(context.may_set_cookie && status == StatusCode::OK) {
        debug_assert!(
            !headers.contains_key(SET_COOKIE),
            "Set-Cookie off the bootstrap"
        );
        headers.remove(SET_COOKIE);
    }

    let immutable = context.admitted
        && context.path.starts_with("/assets/")
        && (status == StatusCode::OK || status == StatusCode::NOT_MODIFIED);

    headers.insert(CONTENT_SECURITY_POLICY, csp.clone());
    headers.insert(
        CROSS_ORIGIN_OPENER_POLICY,
        HeaderValue::from_static("same-origin"),
    );
    headers.insert(
        CROSS_ORIGIN_RESOURCE_POLICY,
        HeaderValue::from_static("same-origin"),
    );
    headers.insert(X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
    headers.insert(REFERRER_POLICY, HeaderValue::from_static("no-referrer"));
    headers.insert(CACHE_CONTROL, if immutable { IMMUTABLE } else { NO_STORE });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::csp::POLICY;

    fn run(status: StatusCode, path: &str, admitted: bool) -> Response<Body> {
        let mut response = Response::new(Body::empty());
        *response.status_mut() = status;
        apply(
            &mut response,
            ResponseContext {
                path,
                admitted,
                may_set_cookie: false,
            },
            &HeaderValue::from_static(POLICY),
        );
        response
    }

    #[test]
    fn only_a_served_hashed_asset_is_immutable() {
        let cache = |r: &Response<Body>| r.headers()[CACHE_CONTROL].clone();
        let js = "/assets/index-AbCd1234.js";
        assert_eq!(cache(&run(StatusCode::OK, js, true)), IMMUTABLE);
        assert_eq!(cache(&run(StatusCode::NOT_MODIFIED, js, true)), IMMUTABLE);
        assert_eq!(cache(&run(StatusCode::NOT_FOUND, js, true)), NO_STORE);
        assert_eq!(cache(&run(StatusCode::OK, js, false)), NO_STORE);
        assert_eq!(cache(&run(StatusCode::OK, "/", true)), NO_STORE);
        assert_eq!(cache(&run(StatusCode::OK, "/api/v1/x", true)), NO_STORE);
    }

    #[test]
    fn exactly_six_policy_headers_are_added() {
        let response = run(StatusCode::OK, "/", true);
        let mut names: Vec<&str> = response.headers().keys().map(HeaderName::as_str).collect();
        names.sort_unstable();
        assert_eq!(
            names,
            [
                "cache-control",
                "content-security-policy",
                "cross-origin-opener-policy",
                "cross-origin-resource-policy",
                "referrer-policy",
                "x-content-type-options",
            ]
        );
    }
}
