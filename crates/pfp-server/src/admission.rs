//! Request admission: `Host`, `Origin`, `Sec-Fetch-Site`, method and body rules
//! (`SECURITY.md` §7.2, ADR-015, test id S-01).
//!
//! [`admit`] is a pure function of [`RequestFacts`] and the [`CanonicalOrigin`];
//! the first failing rule wins, in this order:
//!
//! 1. `Host` — exactly one header, bytes equal to `127.0.0.1:<port>` → else **421**;
//! 2. route class — `/api` and `/api/**` are [`RouteClass::Api`], exactly `/` is
//!    [`RouteClass::Document`], everything else [`RouteClass::Static`];
//! 3. `Origin` — exactly `https://127.0.0.1:<port>`; required on every API request,
//!    elsewhere required on state-changing methods and checked where present →
//!    else **403**;
//! 4. `Sec-Fetch-Site` — API: exactly `same-origin` (absent is refused). Elsewhere:
//!    `same-origin` passes; `none` passes only as a top-level navigation to `/`
//!    (`Sec-Fetch-Mode: navigate` and `Sec-Fetch-Dest: document`); absent passes;
//!    everything else → **403**;
//! 5. method — non-API routes are `GET`/`HEAD` only → **405**;
//! 6. body — a declared length above the cap → **413**; a body that is not
//!    `application/json` → **415**.
//!
//! These headers are browser-supplied, so this is a control against a hostile
//! **web page** (threat T2) only; against a local process the control is the
//! session pair (`session.rs`).
//!
//! An absent `Sec-Fetch-Site` on a document or static route is accepted: the design
//! refuses absence on `/api/**` only. The residual — a client that predates Fetch
//! Metadata — is covered by `Cross-Origin-Resource-Policy: same-origin`, the total
//! absence of CORS headers, `frame-ancestors 'none'`, the `GET`/`HEAD` gate, and by
//! the shell and its assets being public and identical for every install.

use axum::http::header::{
    HeaderName, CONTENT_LENGTH, CONTENT_TYPE, HOST, ORIGIN, TRANSFER_ENCODING,
};
use axum::http::{HeaderMap, Method, Request};

use crate::error::ApiError;
use crate::limits::MAX_BODY_BYTES;
use crate::origin::CanonicalOrigin;

const SEC_FETCH_SITE: HeaderName = HeaderName::from_static("sec-fetch-site");
const SEC_FETCH_MODE: HeaderName = HeaderName::from_static("sec-fetch-mode");
const SEC_FETCH_DEST: HeaderName = HeaderName::from_static("sec-fetch-dest");

/// How many times a header occurs, and its bytes when it occurs once. A header
/// that occurs twice is never "the first one": it is its own, refused, case.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeaderFact<'a> {
    /// Not sent.
    Absent,
    /// Sent exactly once.
    One(&'a [u8]),
    /// Sent more than once.
    Many,
}

impl<'a> HeaderFact<'a> {
    /// Reads `name` from `headers`.
    #[must_use]
    pub fn of(headers: &'a HeaderMap, name: &HeaderName) -> Self {
        let mut values = headers.get_all(name).iter();
        match (values.next(), values.next()) {
            (None, _) => Self::Absent,
            (Some(one), None) => Self::One(one.as_bytes()),
            (Some(_), Some(_)) => Self::Many,
        }
    }

    fn is(self, expected: &str) -> bool {
        matches!(self, Self::One(bytes) if bytes == expected.as_bytes())
    }
}

/// Everything admission looks at. Nothing else about the request matters to it.
#[derive(Debug, Clone, Copy)]
pub struct RequestFacts<'a> {
    /// The method.
    pub method: &'a Method,
    /// The path of the request target.
    pub path: &'a str,
    /// The authority of an absolute-form request target, if one was sent.
    pub target_authority: Option<&'a str>,
    /// `Host`.
    pub host: HeaderFact<'a>,
    /// `Origin`.
    pub origin: HeaderFact<'a>,
    /// `Sec-Fetch-Site`.
    pub fetch_site: HeaderFact<'a>,
    /// `Sec-Fetch-Mode`.
    pub fetch_mode: HeaderFact<'a>,
    /// `Sec-Fetch-Dest`.
    pub fetch_dest: HeaderFact<'a>,
    /// `Content-Type`.
    pub content_type: HeaderFact<'a>,
    /// `Content-Length`.
    pub content_length: HeaderFact<'a>,
    /// Whether a `Transfer-Encoding` header is present.
    pub has_transfer_encoding: bool,
}

impl<'a> RequestFacts<'a> {
    /// Reads the facts from a request head.
    #[must_use]
    pub fn of<B>(request: &'a Request<B>) -> Self {
        let headers = request.headers();
        Self {
            method: request.method(),
            path: request.uri().path(),
            target_authority: request
                .uri()
                .authority()
                .map(axum::http::uri::Authority::as_str),
            host: HeaderFact::of(headers, &HOST),
            origin: HeaderFact::of(headers, &ORIGIN),
            fetch_site: HeaderFact::of(headers, &SEC_FETCH_SITE),
            fetch_mode: HeaderFact::of(headers, &SEC_FETCH_MODE),
            fetch_dest: HeaderFact::of(headers, &SEC_FETCH_DEST),
            content_type: HeaderFact::of(headers, &CONTENT_TYPE),
            content_length: HeaderFact::of(headers, &CONTENT_LENGTH),
            has_transfer_encoding: headers.contains_key(TRANSFER_ENCODING),
        }
    }
}

/// Which family of routes a path belongs to; decides which rules apply.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteClass {
    /// `/api` and everything under `/api/`.
    Api,
    /// Exactly `/`: the shell document.
    Document,
    /// Everything else: static assets and unknown paths.
    Static,
}

impl RouteClass {
    /// The class of `path`.
    #[must_use]
    pub fn of(path: &str) -> Self {
        if path == "/api" || path.starts_with("/api/") {
            Self::Api
        } else if path == "/" {
            Self::Document
        } else {
            Self::Static
        }
    }
}

/// Why a request was not admitted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// 421.
    MisdirectedHost,
    /// 403.
    OriginForbidden,
    /// 403.
    FetchSiteForbidden,
    /// 405.
    MethodNotAllowed,
    /// 413.
    PayloadTooLarge,
    /// 415.
    UnsupportedMediaType,
}

impl Refusal {
    /// The response this refusal becomes.
    #[must_use]
    pub const fn error(self) -> ApiError {
        match self {
            Self::MisdirectedHost => ApiError::MISDIRECTED_HOST,
            Self::OriginForbidden => ApiError::ORIGIN_FORBIDDEN,
            Self::FetchSiteForbidden => ApiError::FETCH_SITE_FORBIDDEN,
            Self::MethodNotAllowed => ApiError::METHOD_NOT_ALLOWED,
            Self::PayloadTooLarge => ApiError::PAYLOAD_TOO_LARGE,
            Self::UnsupportedMediaType => ApiError::UNSUPPORTED_MEDIA_TYPE,
        }
    }
}

/// Whether an API read (`GET`/`HEAD`) may omit `Origin`.
///
/// Browsers do not send `Origin` on same-origin `GET`/`HEAD`, and `SECURITY.md`
/// §7.2 requires it on **every** `/api/**` request, so under the shipped
/// [`ApiReadRule::PostOnly`] an API `GET` from the real front end is refused.
///
/// **What this step decided (README, "API methods").** The five M0 operations are
/// `POST` and that is their contract, not a placeholder: none of them appears in
/// `ARCHITECTURE.md` §5 under another method, each is an RPC-shaped operation
/// (`…/list`, `…/rate-schedule`, `…/status`), and the one that takes financial
/// input must carry it in a body because URLs carry opaque ids only. So nothing
/// M0 ships disagrees with either document, and `PostOnly` is simply §7.2.
///
/// **What is left, and how it resolves.** `ARCHITECTURE.md` §5 draws the M2 run
/// reads as `GET` (`GET /runs/{id}/ledger`). `PLAN.md` ("Reading
/// order") says the spine wins over a specification, so the erratum is against
/// `SECURITY.md` §7.2's `Origin` row, and the pull request that adds the first
/// `GET` flips [`API_READ_RULE`] to [`ApiReadRule::SafeMethodsWithFetchMetadata`]
/// — written and tested below — together with that amendment. It adds `get`
/// operations to the `OpenAPI` document; it does not move the five that exist.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApiReadRule {
    /// `Origin` is required on every API request, whatever the method.
    PostOnly,
    /// An API `GET`/`HEAD` may omit `Origin` (a present one must still be exact),
    /// provided `Sec-Fetch-Site: same-origin` holds.
    SafeMethodsWithFetchMetadata,
}

/// The rule this build ships.
pub const API_READ_RULE: ApiReadRule = ApiReadRule::PostOnly;

/// [`admit_with`] under the shipped [`API_READ_RULE`].
///
/// # Errors
/// The first [`Refusal`] in the module's rule order.
pub fn admit(facts: &RequestFacts<'_>, origin: &CanonicalOrigin) -> Result<RouteClass, Refusal> {
    admit_with(facts, origin, API_READ_RULE)
}

/// Decides whether a request is admitted, and as which route class.
///
/// # Errors
/// The first [`Refusal`] in the module's rule order.
pub fn admit_with(
    facts: &RequestFacts<'_>,
    origin: &CanonicalOrigin,
    api_read_rule: ApiReadRule,
) -> Result<RouteClass, Refusal> {
    // L1 Host: an allowlist of one string. Missing, duplicated, `localhost`, no
    // port, another port, a trailing dot: all are simply "not that string".
    if !facts.host.is(origin.host_header()) {
        return Err(Refusal::MisdirectedHost);
    }
    if facts
        .target_authority
        .is_some_and(|authority| authority != origin.host_header())
    {
        return Err(Refusal::MisdirectedHost);
    }

    // L2 route class.
    let class = RouteClass::of(facts.path);
    let safe_method = facts.method == Method::GET || facts.method == Method::HEAD;

    // L3 Origin.
    let origin_ok = match (class, facts.origin) {
        (_, HeaderFact::One(_)) => facts.origin.is(origin.origin()),
        (_, HeaderFact::Many) => false,
        // THE ONE BRANCH `ApiReadRule` CONTROLS (see its documentation).
        (RouteClass::Api, HeaderFact::Absent) => {
            api_read_rule == ApiReadRule::SafeMethodsWithFetchMetadata && safe_method
        }
        (RouteClass::Document | RouteClass::Static, HeaderFact::Absent) => safe_method,
    };
    if !origin_ok {
        return Err(Refusal::OriginForbidden);
    }

    // L4 Sec-Fetch-Site.
    let fetch_site_ok = match class {
        RouteClass::Api => facts.fetch_site.is("same-origin"),
        RouteClass::Document | RouteClass::Static => match facts.fetch_site {
            HeaderFact::Absent => true,
            HeaderFact::One(_) if facts.fetch_site.is("same-origin") => true,
            HeaderFact::One(_) if facts.fetch_site.is("none") => {
                class == RouteClass::Document
                    && facts.fetch_mode.is("navigate")
                    && facts.fetch_dest.is("document")
            }
            // `same-site`, `cross-site`, anything unknown, and a repeated header.
            HeaderFact::One(_) | HeaderFact::Many => false,
        },
    };
    if !fetch_site_ok {
        return Err(Refusal::FetchSiteForbidden);
    }

    // L5 method gate. API methods are the router's business.
    if class != RouteClass::Api && !safe_method {
        return Err(Refusal::MethodNotAllowed);
    }

    // L6 body rules.
    let declared_length = match facts.content_length {
        HeaderFact::Absent => None,
        HeaderFact::One(bytes) => Some(
            std::str::from_utf8(bytes)
                .ok()
                .and_then(|s| s.parse::<u64>().ok()),
        ),
        HeaderFact::Many => Some(None),
    };
    let cap = u64::try_from(MAX_BODY_BYTES).unwrap_or(u64::MAX);
    match declared_length {
        // Unreadable or conflicting lengths are hyper's to reject; if one ever
        // reaches this layer it is treated as too large, never as zero.
        Some(None) => return Err(Refusal::PayloadTooLarge),
        Some(Some(n)) if n > cap => return Err(Refusal::PayloadTooLarge),
        _ => {}
    }
    let has_body = facts.has_transfer_encoding || matches!(declared_length, Some(Some(n)) if n > 0);
    if has_body && !is_json(facts.content_type) {
        return Err(Refusal::UnsupportedMediaType);
    }

    Ok(class)
}

/// `application/json`, optionally followed by parameters (`; charset=utf-8`).
fn is_json(content_type: HeaderFact<'_>) -> bool {
    let HeaderFact::One(bytes) = content_type else {
        return false;
    };
    let Ok(text) = std::str::from_utf8(bytes) else {
        return false;
    };
    let essence = text.split(';').next().unwrap_or_default().trim();
    essence.eq_ignore_ascii_case("application/json")
}

#[cfg(test)]
mod tests {
    use super::*;

    const PORT: u16 = 8443;
    const HOST_OK: &[u8] = b"127.0.0.1:8443";
    const ORIGIN_OK: &[u8] = b"https://127.0.0.1:8443";

    fn api_post(method: &Method) -> RequestFacts<'_> {
        RequestFacts {
            method,
            path: "/api/v1/session/status",
            target_authority: None,
            host: HeaderFact::One(HOST_OK),
            origin: HeaderFact::One(ORIGIN_OK),
            fetch_site: HeaderFact::One(b"same-origin"),
            fetch_mode: HeaderFact::One(b"cors"),
            fetch_dest: HeaderFact::One(b"empty"),
            content_type: HeaderFact::Absent,
            content_length: HeaderFact::Absent,
            has_transfer_encoding: false,
        }
    }

    fn navigation(method: &Method) -> RequestFacts<'_> {
        RequestFacts {
            method,
            path: "/",
            target_authority: None,
            host: HeaderFact::One(HOST_OK),
            origin: HeaderFact::Absent,
            fetch_site: HeaderFact::One(b"none"),
            fetch_mode: HeaderFact::One(b"navigate"),
            fetch_dest: HeaderFact::One(b"document"),
            content_type: HeaderFact::Absent,
            content_length: HeaderFact::Absent,
            has_transfer_encoding: false,
        }
    }

    fn run(facts: &RequestFacts<'_>) -> Result<RouteClass, Refusal> {
        admit_with(facts, &CanonicalOrigin::new(PORT), ApiReadRule::PostOnly)
    }

    #[test]
    fn the_shipped_rule_is_post_only() {
        assert_eq!(API_READ_RULE, ApiReadRule::PostOnly);
    }

    #[test]
    fn host_is_an_allowlist_of_one_string() {
        let post = Method::POST;
        assert_eq!(run(&api_post(&post)), Ok(RouteClass::Api));
        let bad: [HeaderFact<'_>; 9] = [
            HeaderFact::Absent,
            HeaderFact::Many,
            HeaderFact::One(b"localhost:8443"),
            HeaderFact::One(b"127.0.0.1"),
            HeaderFact::One(b"127.0.0.1:8444"),
            HeaderFact::One(b"127.0.0.1.:8443"),
            HeaderFact::One(b"evil.example:8443"),
            HeaderFact::One(b"[::1]:8443"),
            HeaderFact::One(b"127.0.0.1:8443 "),
        ];
        for host in bad {
            let facts = RequestFacts {
                host,
                ..api_post(&post)
            };
            assert_eq!(run(&facts), Err(Refusal::MisdirectedHost), "{host:?}");
        }
        let absolute = RequestFacts {
            target_authority: Some("evil.example:8443"),
            ..api_post(&post)
        };
        assert_eq!(run(&absolute), Err(Refusal::MisdirectedHost));
        let absolute_ok = RequestFacts {
            target_authority: Some("127.0.0.1:8443"),
            ..api_post(&post)
        };
        assert_eq!(run(&absolute_ok), Ok(RouteClass::Api));
    }

    #[test]
    fn host_is_checked_before_everything_else() {
        let post = Method::POST;
        let facts = RequestFacts {
            host: HeaderFact::One(b"evil.example"),
            origin: HeaderFact::One(b"https://evil.example"),
            fetch_site: HeaderFact::One(b"cross-site"),
            ..api_post(&post)
        };
        assert_eq!(run(&facts), Err(Refusal::MisdirectedHost));
    }

    #[test]
    fn api_origin_is_required_and_exact() {
        let post = Method::POST;
        let get = Method::GET;
        let bad: [HeaderFact<'_>; 8] = [
            HeaderFact::Absent,
            HeaderFact::Many,
            HeaderFact::One(b"null"),
            HeaderFact::One(b"http://127.0.0.1:8443"),
            HeaderFact::One(b"https://127.0.0.1:8444"),
            HeaderFact::One(b"https://localhost:8443"),
            HeaderFact::One(b"https://127.0.0.1:8443/"),
            HeaderFact::One(b"https://evil.example"),
        ];
        for origin in bad {
            for method in [&post, &get] {
                let facts = RequestFacts {
                    origin,
                    ..api_post(method)
                };
                assert_eq!(run(&facts), Err(Refusal::OriginForbidden), "{origin:?}");
            }
        }
    }

    #[test]
    fn the_alternative_read_rule_relaxes_exactly_one_case() {
        let origin = CanonicalOrigin::new(PORT);
        let rule = ApiReadRule::SafeMethodsWithFetchMetadata;
        let (get, head, post) = (Method::GET, Method::HEAD, Method::POST);
        for method in [&get, &head] {
            let absent = RequestFacts {
                origin: HeaderFact::Absent,
                ..api_post(method)
            };
            assert_eq!(admit_with(&absent, &origin, rule), Ok(RouteClass::Api));
            // …but only with same-origin fetch metadata,
            let no_metadata = RequestFacts {
                fetch_site: HeaderFact::Absent,
                ..absent
            };
            assert_eq!(
                admit_with(&no_metadata, &origin, rule),
                Err(Refusal::FetchSiteForbidden)
            );
            // …and a present Origin must still be exact.
            let foreign = RequestFacts {
                origin: HeaderFact::One(b"https://evil.example"),
                ..api_post(method)
            };
            assert_eq!(
                admit_with(&foreign, &origin, rule),
                Err(Refusal::OriginForbidden)
            );
        }
        let post_without = RequestFacts {
            origin: HeaderFact::Absent,
            ..api_post(&post)
        };
        assert_eq!(
            admit_with(&post_without, &origin, rule),
            Err(Refusal::OriginForbidden)
        );
    }

    #[test]
    fn api_fetch_site_must_be_same_origin() {
        let post = Method::POST;
        let bad: [HeaderFact<'_>; 6] = [
            HeaderFact::Absent,
            HeaderFact::Many,
            HeaderFact::One(b"none"),
            HeaderFact::One(b"same-site"),
            HeaderFact::One(b"cross-site"),
            HeaderFact::One(b"Same-Origin"),
        ];
        for fetch_site in bad {
            let facts = RequestFacts {
                fetch_site,
                fetch_mode: HeaderFact::One(b"navigate"),
                fetch_dest: HeaderFact::One(b"document"),
                ..api_post(&post)
            };
            assert_eq!(
                run(&facts),
                Err(Refusal::FetchSiteForbidden),
                "{fetch_site:?}"
            );
        }
    }

    #[test]
    fn top_level_navigation_to_root_is_admitted() {
        let get = Method::GET;
        assert_eq!(run(&navigation(&get)), Ok(RouteClass::Document));
    }

    #[test]
    fn none_is_admitted_only_as_a_document_navigation_to_root() {
        let get = Method::GET;
        let cases = [
            RequestFacts {
                fetch_mode: HeaderFact::One(b"cors"),
                ..navigation(&get)
            },
            RequestFacts {
                fetch_mode: HeaderFact::Absent,
                ..navigation(&get)
            },
            RequestFacts {
                fetch_dest: HeaderFact::Absent,
                ..navigation(&get)
            },
            RequestFacts {
                fetch_dest: HeaderFact::One(b"iframe"),
                ..navigation(&get)
            },
            RequestFacts {
                path: "/assets/index-abc.js",
                ..navigation(&get)
            },
            RequestFacts {
                path: "/index.html",
                ..navigation(&get)
            },
        ];
        for facts in cases {
            assert_eq!(run(&facts), Err(Refusal::FetchSiteForbidden), "{facts:?}");
        }
    }

    #[test]
    fn cross_site_and_same_site_navigations_are_refused() {
        let get = Method::GET;
        for value in [&b"cross-site"[..], b"same-site", b"bogus", b""] {
            for path in ["/", "/assets/x.js"] {
                let facts = RequestFacts {
                    path,
                    fetch_site: HeaderFact::One(value),
                    ..navigation(&get)
                };
                assert_eq!(run(&facts), Err(Refusal::FetchSiteForbidden));
            }
        }
    }

    #[test]
    fn absent_fetch_site_passes_off_the_api_only() {
        let get = Method::GET;
        for (path, class) in [
            ("/", RouteClass::Document),
            ("/assets/x.js", RouteClass::Static),
        ] {
            let facts = RequestFacts {
                path,
                fetch_site: HeaderFact::Absent,
                fetch_mode: HeaderFact::Absent,
                fetch_dest: HeaderFact::Absent,
                ..navigation(&get)
            };
            assert_eq!(run(&facts), Ok(class));
        }
    }

    #[test]
    fn a_present_origin_is_checked_on_every_route() {
        let get = Method::GET;
        let facts = RequestFacts {
            origin: HeaderFact::One(b"https://evil.example"),
            ..navigation(&get)
        };
        assert_eq!(run(&facts), Err(Refusal::OriginForbidden));
    }

    #[test]
    fn state_changing_method_off_the_api_is_403_without_origin_else_405() {
        let post = Method::POST;
        let without = RequestFacts {
            fetch_site: HeaderFact::One(b"same-origin"),
            ..navigation(&post)
        };
        assert_eq!(run(&without), Err(Refusal::OriginForbidden));
        let with = RequestFacts {
            origin: HeaderFact::One(ORIGIN_OK),
            ..without
        };
        assert_eq!(run(&with), Err(Refusal::MethodNotAllowed));
    }

    #[test]
    fn a_preflight_is_never_admitted_off_the_api() {
        let options = Method::OPTIONS;
        let facts = RequestFacts {
            origin: HeaderFact::One(ORIGIN_OK),
            fetch_site: HeaderFact::One(b"same-origin"),
            ..navigation(&options)
        };
        assert_eq!(run(&facts), Err(Refusal::MethodNotAllowed));
        let cross = RequestFacts {
            origin: HeaderFact::One(b"https://evil.example"),
            fetch_site: HeaderFact::One(b"cross-site"),
            ..api_post(&options)
        };
        assert_eq!(run(&cross), Err(Refusal::OriginForbidden));
    }

    #[test]
    fn body_rules() {
        let post = Method::POST;
        let json = RequestFacts {
            content_type: HeaderFact::One(b"application/json; charset=utf-8"),
            content_length: HeaderFact::One(b"12"),
            ..api_post(&post)
        };
        assert_eq!(run(&json), Ok(RouteClass::Api));

        let at_cap = RequestFacts {
            content_length: HeaderFact::One(b"1048576"),
            ..json
        };
        assert_eq!(run(&at_cap), Ok(RouteClass::Api));
        let over = RequestFacts {
            content_length: HeaderFact::One(b"1048577"),
            ..json
        };
        assert_eq!(run(&over), Err(Refusal::PayloadTooLarge));
        let unreadable = RequestFacts {
            content_length: HeaderFact::One(b"twelve"),
            ..json
        };
        assert_eq!(run(&unreadable), Err(Refusal::PayloadTooLarge));

        for content_type in [
            HeaderFact::Absent,
            HeaderFact::Many,
            HeaderFact::One(b"text/plain"),
            HeaderFact::One(b"application/x-www-form-urlencoded"),
            HeaderFact::One(b"application/jsonp"),
        ] {
            let facts = RequestFacts {
                content_type,
                ..json
            };
            assert_eq!(run(&facts), Err(Refusal::UnsupportedMediaType));
            let chunked = RequestFacts {
                content_type,
                content_length: HeaderFact::Absent,
                has_transfer_encoding: true,
                ..json
            };
            assert_eq!(run(&chunked), Err(Refusal::UnsupportedMediaType));
        }

        // No body: no content type needed.
        let empty = RequestFacts {
            content_length: HeaderFact::One(b"0"),
            ..api_post(&post)
        };
        assert_eq!(run(&empty), Ok(RouteClass::Api));
    }

    #[test]
    fn route_classes() {
        assert_eq!(RouteClass::of("/api"), RouteClass::Api);
        assert_eq!(RouteClass::of("/api/"), RouteClass::Api);
        assert_eq!(RouteClass::of("/api/v1/x"), RouteClass::Api);
        assert_eq!(RouteClass::of("/apiary"), RouteClass::Static);
        assert_eq!(RouteClass::of("/"), RouteClass::Document);
        assert_eq!(RouteClass::of("/assets/a.js"), RouteClass::Static);
    }
}
