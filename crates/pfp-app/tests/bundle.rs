//! The real `pfp` binary serving the REAL front-end bundle (`cargo xtask build-web`
//! output, embedded by `pfp-server`'s build script): the shell and every asset it
//! names, fetched over TLS from a spawned process — no browser, no trust store.
//!
//! What this proves, from outside the process:
//!
//! * `index.html` carries **no inline script, no inline style and no inline event
//!   handler** — the three things `script-src 'self'; style-src 'self'` would
//!   block, silently, in a browser;
//! * every `src` / `href` it names is a **same-origin, content-hashed** path under
//!   `/assets/`, and every embedded asset is reachable from the shell (directly,
//!   or named by another asset);
//! * each of those answers 200 with the exact policy header set (asserted by the
//!   client on every response), `nosniff`, the right content type, `immutable`
//!   caching, a strong `ETag` that revalidates to 304 — and the bytes on the wire
//!   are the bytes that were embedded;
//! * no asset names a further origin, a source map or an `@import`;
//! * the export route exists in the binary and is closed without a session. With
//!   `--no-open` nobody holds the launch token (`SECURITY.md` §7.1), so the
//!   authenticated export is exercised in `pfp-server/tests/export.rs` and
//!   `tests/e2e.rs`' in-process launch, never from a spawned process.
//!
//! What it cannot prove: that a browser *executes* the bundle without a
//! `securitypolicyviolation`. That is the Playwright step's job.
//!
//! Without `web/dist` (a checkout with no Node) the embedded manifest is the
//! placeholder shell and the bundle assertions have nothing to look at; the test
//! then checks the placeholder is what is served and says so. It cannot fall back
//! silently where a bundle exists: if `web/dist/index.html` is on disk and the
//! placeholder was embedded anyway, the test fails. (No environment switch: this
//! crate reads no environment variable outside its allowlist, `SECURITY.md` §3.7.)

// Not an engine crate: spawning the built binary, sockets and temp files are the point.
#![forbid(unsafe_code)]
#![allow(
    clippy::disallowed_types,
    clippy::disallowed_methods,
    clippy::disallowed_macros
)]

mod support;

use std::collections::BTreeSet;

use pfp_server::AssetManifest;

const EXPORT: &str = "/api/v1/tax/rate-schedule/export";

/// One start tag of the document: lower-case name, attributes in order.
#[derive(Debug)]
struct Tag {
    name: String,
    attrs: Vec<(String, String)>,
    /// Byte offset just past the tag's `>`.
    end: usize,
}

/// The start tags of a small, well-formed HTML document (Vite's output). Crude on
/// purpose and fail-closed: anything it cannot read is a test failure, not a skip.
fn start_tags(html: &str) -> Vec<Tag> {
    let mut tags = Vec::new();
    let mut at = 0;
    while let Some(open) = html[at..].find('<').map(|i| at + i) {
        let rest = &html[open + 1..];
        if rest.starts_with("!--") {
            at = open + rest.find("-->").expect("comment is closed") + 4;
            continue;
        }
        let close = open + 1 + rest.find('>').expect("tag is closed");
        let inner = html[open + 1..close].trim_end_matches('/').trim();
        at = close + 1;
        if inner.starts_with('/') || inner.starts_with('!') {
            continue;
        }
        let (name, mut tail) = inner.split_once(char::is_whitespace).unwrap_or((inner, ""));
        let mut attrs = Vec::new();
        loop {
            tail = tail.trim_start();
            if tail.is_empty() {
                break;
            }
            let name_end = tail
                .find(|c: char| c == '=' || c.is_whitespace())
                .unwrap_or(tail.len());
            let attr = tail[..name_end].to_ascii_lowercase();
            tail = tail[name_end..].trim_start();
            let value = if let Some(after) = tail.strip_prefix('=') {
                let after = after.trim_start();
                let quote = after.chars().next().expect("attribute value");
                assert!(
                    quote == '"' || quote == '\'',
                    "unquoted attribute value in <{inner}>"
                );
                let body = &after[1..];
                let len = body.find(quote).expect("attribute value is closed");
                tail = &body[len + 1..];
                body[..len].to_owned()
            } else {
                String::new()
            };
            attrs.push((attr, value));
        }
        tags.push(Tag {
            name: name.to_ascii_lowercase(),
            attrs,
            end: close + 1,
        });
    }
    tags
}

/// `/assets/<stem>-<8 URL-safe hash characters>.<ext>`: what Vite emits and what
/// makes `immutable` caching sound.
fn is_hashed_asset_path(path: &str) -> bool {
    let Some(file) = path.strip_prefix("/assets/") else {
        return false;
    };
    let Some((stem, extension)) = file.rsplit_once('.') else {
        return false;
    };
    // The hash is base64url, so it may itself contain `-`: take it by length.
    if !stem.is_ascii() || stem.len() < 10 {
        return false;
    }
    let (name, hash) = stem.split_at(stem.len() - 8);
    let Some(name) = name.strip_suffix('-') else {
        return false;
    };
    let url_safe = |s: &str| {
        s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    };
    !name.is_empty()
        && url_safe(name)
        && url_safe(hash)
        && matches!(extension, "js" | "css" | "svg" | "png" | "woff2")
}

#[test]
fn hashed_asset_paths_are_recognised() {
    assert!(is_hashed_asset_path("/assets/index-CmlKDCQS.js"));
    assert!(is_hashed_asset_path("/assets/style-CWwnV_hf.css"));
    assert!(is_hashed_asset_path("/assets/a-b-c-AbCd12_-.js"));
    for refused in [
        "/assets/index.js",
        "/assets/index-short.js",
        "/assets/-AbCd1234.js",
        "/assets/index-AbCd1234.map",
        "/assets/../index-AbCd1234.js",
        "/index-AbCd1234.js",
        "//cdn.example/assets/index-AbCd1234.js",
        "https://cdn.example/assets/index-AbCd1234.js",
        "assets/index-AbCd1234.js",
    ] {
        assert!(!is_hashed_asset_path(refused), "{refused}");
    }
}

#[test]
fn the_tag_reader_reads_what_vite_writes() {
    let tags = start_tags(
        "<!doctype html><!-- c --><html lang=\"en\"><script type=\"module\" crossorigin \
         src=\"/assets/a-AbCd1234.js\"></script><link rel='stylesheet' href='/x'><br/></html>",
    );
    let names: Vec<&str> = tags.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(names, ["html", "script", "link", "br"]);
    assert_eq!(
        tags[1].attrs,
        [
            ("type".to_owned(), "module".to_owned()),
            ("crossorigin".to_owned(), String::new()),
            ("src".to_owned(), "/assets/a-AbCd1234.js".to_owned()),
        ]
    );
    assert_eq!(tags[2].attrs[1], ("href".to_owned(), "/x".to_owned()));
}

/// The document half: nothing inline, nothing foreign. Returns the asset paths the
/// shell names.
fn assert_document_is_csp_clean(html: &str) -> BTreeSet<String> {
    let lower = html.to_ascii_lowercase();
    for forbidden in [
        "://",
        "javascript:",
        "data:",
        "srcdoc",
        "<base",
        "<meta http-equiv",
    ] {
        assert!(
            !lower.contains(forbidden),
            "the shell contains {forbidden:?}"
        );
    }
    let mut named = BTreeSet::new();
    for tag in start_tags(html) {
        assert!(
            !matches!(
                tag.name.as_str(),
                "style" | "iframe" | "object" | "embed" | "base" | "form"
            ),
            "<{}> in the shell",
            tag.name
        );
        for (attr, value) in &tag.attrs {
            assert_ne!(attr, "style", "inline style attribute on <{}>", tag.name);
            assert!(
                !attr.starts_with("on"),
                "inline event handler {attr} on <{}>",
                tag.name
            );
            assert!(
                !matches!(attr.as_str(), "nonce" | "integrity"),
                "{attr}: the policy has no nonce or hash source to match"
            );
            if matches!(attr.as_str(), "src" | "href") {
                assert!(
                    is_hashed_asset_path(value),
                    "<{} {attr}={value:?}> is not a same-origin hashed asset",
                    tag.name
                );
                named.insert(value.clone());
            }
        }
        if tag.name == "script" {
            assert!(
                tag.attrs.iter().any(|(a, _)| a == "src"),
                "a <script> without src is an inline script"
            );
            let body = &html[tag.end..];
            let close = body
                .to_ascii_lowercase()
                .find("</script")
                .expect("script is closed");
            assert!(
                body[..close].trim().is_empty(),
                "a <script src> with inline content"
            );
        }
        if tag.name == "link" {
            let rel = tag
                .attrs
                .iter()
                .find(|(a, _)| a == "rel")
                .map(|(_, v)| v.as_str());
            assert!(
                matches!(rel, Some("stylesheet" | "modulepreload")),
                "<link rel={rel:?}>"
            );
        }
    }
    named
}

#[test]
fn the_document_check_refuses_what_the_policy_would_block() {
    let clean = "<!doctype html><html><head><script type=\"module\" \
                 src=\"/assets/index-AbCd1234.js\"></script><link rel=\"stylesheet\" \
                 href=\"/assets/style-AbCd1234.css\"></head><body><div id=\"root\"></div></body></html>";
    assert_eq!(assert_document_is_csp_clean(clean).len(), 2);
    for dirty in [
        "<script>alert(1)</script>",
        "<script src=\"/assets/index-AbCd1234.js\">alert(1)</script>",
        "<style>p{color:red}</style>",
        "<p style=\"color:red\">x</p>",
        "<body onload=\"x()\"></body>",
        "<script src=\"/assets/index.js\"></script>",
        "<script src=\"//cdn.example/assets/index-AbCd1234.js\"></script>",
        "<link rel=\"stylesheet\" href=\"https://fonts.example/f-AbCd1234.css\">",
        "<link rel=\"icon\" href=\"/assets/icon-AbCd1234.svg\">",
        "<iframe></iframe>",
    ] {
        let caught = std::panic::catch_unwind(|| assert_document_is_csp_clean(dirty));
        assert!(caught.is_err(), "not refused: {dirty}");
    }
}

/// The extension of a path the hashed-name check has already vetted (ASCII, exact case).
fn extension(path: &str) -> &str {
    path.rsplit_once('.').map_or("", |(_, e)| e)
}

/// What an asset's text may not contain. `csp::derive` already refused URL schemes
/// at build time and again at startup; this is the view from outside the process.
fn assert_asset_text_is_self_contained(path: &str, text: &str) {
    assert!(
        !text.contains("sourceMappingURL"),
        "{path}: a source map reference"
    );
    if extension(path) == "css" {
        assert!(!text.contains("@import"), "{path}: @import");
        for (at, _) in text.match_indices("url(") {
            let target = text[at + 4..].trim_start_matches(['"', '\'']);
            assert!(
                target.starts_with("/assets/") || target.starts_with("data:"),
                "{path}: url() names something other than a same-origin asset"
            );
        }
    }
}

/// Every embedded asset, over TLS, with the policy intact (the client asserts the
/// complete header set, no CORS, no Location, no Server on every answer); the shell
/// names only what exists, and nothing embedded is orphaned.
fn assert_every_embedded_asset_is_served(
    client: &support::Client,
    embedded: &AssetManifest,
    named: &BTreeSet<String>,
) {
    let embedded_assets: Vec<(String, &pfp_server::assets::Asset)> = embedded
        .iter()
        .filter(|(path, _)| path.starts_with("assets/"))
        .map(|(path, asset)| (format!("/{path}"), asset))
        .collect();
    let mut texts = Vec::new();
    for (path, asset) in &embedded_assets {
        assert!(is_hashed_asset_path(path), "{path} is not content-hashed");
        let response = client.subresource(path);
        assert_eq!(response.status, 200, "{path}");
        assert_eq!(
            response.header("content-type"),
            Some(asset.content_type()),
            "{path}"
        );
        let expected_type = match extension(path) {
            "js" => "text/javascript; charset=utf-8",
            "css" => "text/css; charset=utf-8",
            _ => asset.content_type(),
        };
        assert_eq!(response.header("content-type"), Some(expected_type));
        assert_eq!(
            response.header("cache-control"),
            Some(support::IMMUTABLE),
            "{path}"
        );
        assert_eq!(
            response.header("content-length"),
            Some(asset.bytes().len().to_string().as_str()),
            "{path}"
        );
        assert_eq!(response.body, asset.bytes(), "{path}: bytes on the wire");
        let etag = response.header("etag").expect("etag").to_owned();
        assert_eq!(etag, format!("\"{}\"", asset.sha256_hex()), "{path}");
        let revalidated = client.bare(
            "GET",
            path,
            &[
                ("Sec-Fetch-Site", "same-origin".to_owned()),
                ("If-None-Match", etag),
            ],
        );
        assert_eq!(revalidated.status, 304, "{path}");
        assert!(revalidated.body.is_empty());
        if let Some(text) = asset.text() {
            assert_asset_text_is_self_contained(path, text);
            texts.push(text);
        }
    }

    for path in named {
        assert!(
            embedded_assets.iter().any(|(p, _)| p == path),
            "the shell names {path}, which is not embedded"
        );
    }
    for (path, _) in &embedded_assets {
        let file = path.trim_start_matches("/assets/");
        assert!(
            named.contains(path) || texts.iter().any(|t| t.contains(file)),
            "{path} is embedded but nothing names it"
        );
    }
}

/// The export route, from the real binary: present, POST-only, and closed.
fn assert_the_export_route_is_closed(client: &support::Client) {
    let body = r#"{"year":2026,"filingStatus":"mfj","taxableIncome":10000000,"format":"csv"}"#;
    for format_body in [body.to_owned(), body.replace("csv", "json")] {
        let refused = client.api(EXPORT, Some(&format_body));
        assert_eq!(refused.status, 401);
        assert_eq!(refused.json()["code"], "session_required");
        assert_eq!(
            refused.header("content-type"),
            Some("application/json"),
            "a refusal is JSON, never a file"
        );
        assert!(refused.header("content-disposition").is_none());
    }
    // A navigation to the export path (what a link or an address bar would send)
    // never yields a file either.
    let navigated = client.navigate(EXPORT);
    assert!(
        (403..=405).contains(&navigated.status),
        "{}",
        navigated.status
    );
    assert!(navigated.header("content-disposition").is_none());
}

#[test]
fn the_real_binary_serves_the_real_bundle_under_the_policy() {
    let embedded = AssetManifest::embedded().expect("what was embedded is a bundle");
    let mut pfp = support::spawn_pfp();
    pfp.wait_ready().expect("ready");
    let client = pfp.client();

    let document = client.navigate("/");
    assert_eq!(document.status, 200);
    assert_eq!(
        document.header("content-type"),
        Some("text/html; charset=utf-8")
    );
    assert!(
        document.header("etag").is_none(),
        "the shell is never cached"
    );
    let html = String::from_utf8(document.body.clone()).expect("UTF-8 shell");

    if embedded.is_placeholder() {
        let built =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../web/dist/index.html");
        assert!(
            !built.exists(),
            "web/dist exists but the placeholder shell was embedded: the build script \
             did not pick the bundle up"
        );
        assert!(html.contains("cargo xtask build-web"));
        assert!(assert_document_is_csp_clean(&html).is_empty());
        eprintln!("bundle.rs: web/dist was not built; only the placeholder shell was checked");
        assert_eq!(pfp.terminate().code(), Some(0));
        return;
    }

    assert!(
        !html.contains("cargo xtask build-web"),
        "not the placeholder"
    );
    assert_eq!(
        document.body,
        embedded.get("index.html").expect("index").bytes(),
        "the shell on the wire is the shell that was embedded"
    );
    assert!(html.contains("<div id=\"root\"></div>"));

    // 1. Nothing inline; every reference same-origin and hashed.
    let named = assert_document_is_csp_clean(&html);
    assert!(named.iter().any(|p| extension(p) == "js"), "a script");
    assert!(named.iter().any(|p| extension(p) == "css"), "a stylesheet");
    let stylesheets = start_tags(&html)
        .iter()
        .filter(|t| t.name == "link" && t.attrs.iter().any(|(_, v)| v == "stylesheet"))
        .count();
    assert_eq!(stylesheets, 1, "one stylesheet");

    assert_every_embedded_asset_is_served(&client, &embedded, &named);

    // 4. A hashed name that was not embedded is a 404, not a fallback to the shell.
    assert_eq!(client.subresource("/assets/index-00000000.js").status, 404);
    assert_eq!(client.subresource("/.dist-stamp").status, 404);
    assert_eq!(client.subresource("/assets/").status, 404);

    assert_the_export_route_is_closed(&client);

    assert_eq!(pfp.terminate().code(), Some(0), "graceful exit");
    assert!(!support::has_token_shaped_run(&pfp.stdout()));
    assert!(!support::has_token_shaped_run(&pfp.stderr()));
}
