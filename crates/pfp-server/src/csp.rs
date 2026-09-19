//! The Content-Security-Policy, derived from the asset manifest **with refusal**.
//!
//! `SECURITY.md` §7.3 fixes the policy string and snapshot-tests it, and every
//! source in it is `'self'`, so no per-asset token can appear in it. "Generated
//! from the manifest so it cannot drift" is therefore implemented as: [`derive`]
//! returns the fixed policy only when the manifest proves that policy sufficient
//! and safe for what is embedded, and otherwise an error that stops startup:
//!
//! * `index.html` has no inline script, no `<style>`, no `style=` or `on*=`
//!   attribute, no `<base>` and no `<meta http-equiv>`;
//! * every `src`/`href` in it is root-relative and names a manifest entry;
//! * every file under `assets/` has a hash-shaped name (so `immutable` is safe);
//! * no embedded text names an absolute `http(s)://` URL outside a short
//!   allowlist of inert strings (XML namespace URIs, React's error-decoder URL)
//!   — the static half of "zero third-party requests" (test id S-06).

use std::fmt;

use axum::http::HeaderValue;

use crate::assets::{AssetKind, AssetManifest, ASSETS_PREFIX, INDEX};

/// The policy, exactly as `SECURITY.md` §7.3 gives it, on one line.
pub const POLICY: &str = "default-src 'none'; script-src 'self'; style-src 'self'; \
img-src 'self' data:; connect-src 'self'; font-src 'self'; \
frame-ancestors 'none'; base-uri 'none'; form-action 'none'";

/// Absolute URLs that may appear in embedded text because nothing fetches them.
const INERT_URL_PREFIXES: [&str; 2] = ["http://www.w3.org/", "https://react.dev/errors/"];

/// Why the fixed policy does not fit the embedded bundle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CspError {
    /// The manifest has no `index.html`, or it is not UTF-8.
    NoDocument,
    /// `index.html` contains something the policy would block or that would
    /// weaken it. The string names the construct, not its content.
    DocumentViolation(&'static str),
    /// `index.html` references something that is not a root-relative manifest entry.
    UnknownReference(String),
    /// A file under `assets/` does not have a hash-shaped name.
    UnhashedAsset(String),
    /// An embedded text file names an external origin.
    ExternalUrl(String),
}

impl fmt::Display for CspError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoDocument => f.write_str("the web bundle has no readable index.html"),
            Self::DocumentViolation(what) => write!(f, "index.html contains {what}"),
            Self::UnknownReference(r) => write!(f, "index.html references {r}"),
            Self::UnhashedAsset(p) => write!(f, "{p} is not content-hashed by name"),
            Self::ExternalUrl(p) => write!(f, "{p} names an external origin"),
        }
    }
}

impl std::error::Error for CspError {}

/// The `Content-Security-Policy` header value for `manifest`.
///
/// # Errors
/// [`CspError`] when the fixed policy would not fit what is embedded.
pub fn derive(manifest: &AssetManifest) -> Result<HeaderValue, CspError> {
    let document = manifest
        .get(INDEX)
        .and_then(|index| index.text())
        .ok_or(CspError::NoDocument)?;
    check_document(document, manifest)?;

    for (path, asset) in manifest.iter() {
        if let Some(name) = path.strip_prefix(ASSETS_PREFIX) {
            if !is_hash_named(name) {
                return Err(CspError::UnhashedAsset(path.to_owned()));
            }
        }
        // Every kind in the closed table maps to a directive that is `'self'`.
        match asset.kind() {
            AssetKind::Document
            | AssetKind::Script
            | AssetKind::Style
            | AssetKind::Image
            | AssetKind::Font
            | AssetKind::Data => {}
        }
        if let Some(text) = asset.text() {
            if names_external_origin(text) {
                return Err(CspError::ExternalUrl(path.to_owned()));
            }
        }
    }
    Ok(HeaderValue::from_static(POLICY))
}

/// `name-<hash>.ext` with a hash of at least eight URL-safe characters, as Vite
/// emits.
/// Length of the content hash Vite appends to a file name (`[name]-[hash]`).
const VITE_HASH_LEN: usize = 8;

/// `<base>-<8 hash characters>.<extension>`. The hash alphabet is base64url, so
/// the hash may itself contain `-` or `_`: it is taken by length, not by splitting
/// on the last `-` (which would refuse a genuine name such as `index-Ab-d1234.js`).
fn is_hash_named(name: &str) -> bool {
    let Some((stem, _extension)) = name.rsplit_once('.') else {
        return false;
    };
    let bytes = stem.as_bytes();
    if bytes.len() < VITE_HASH_LEN + 2 {
        return false;
    }
    let (base, hash) = bytes.split_at(bytes.len() - VITE_HASH_LEN);
    base.last() == Some(&b'-')
        && hash
            .iter()
            .all(|b| b.is_ascii_alphanumeric() || *b == b'_' || *b == b'-')
}

fn names_external_origin(text: &str) -> bool {
    ["http://", "https://"].iter().any(|scheme| {
        text.match_indices(scheme).any(|(at, _)| {
            let rest = &text[at..];
            !INERT_URL_PREFIXES
                .iter()
                .any(|inert| rest.starts_with(inert))
        })
    })
}

/// The tags of an HTML document, as `(lower-case name, lower-case raw tag text)`.
/// Deliberately crude: it is a refusal check on a file this project's own build
/// produced, not a parser for hostile input, and a construct it cannot read is
/// refused rather than guessed at.
fn tags(document: &str) -> impl Iterator<Item = (String, String)> + '_ {
    document.split('<').skip(1).filter_map(|chunk| {
        let raw = chunk.split('>').next()?.to_ascii_lowercase();
        if raw.starts_with('/') || raw.starts_with('!') {
            return None;
        }
        let name: String = raw
            .chars()
            .take_while(char::is_ascii_alphanumeric)
            .collect();
        Some((name, raw))
    })
}

/// Where the value of attribute `name` lies in a lower-cased raw tag, quoted or
/// not. Lower-casing ASCII does not move bytes, so the range also indexes the
/// original text.
fn attribute(raw: &str, name: &str) -> Option<std::ops::Range<usize>> {
    let needle = format!(" {name}=");
    let mut start = raw.find(&needle)? + needle.len();
    let rest = &raw[start..];
    let len = match rest.chars().next()? {
        quote @ ('"' | '\'') => {
            start += 1;
            rest[1..].find(quote)?
        }
        _ => rest.find([' ', '/']).unwrap_or(rest.len()),
    };
    Some(start..start + len)
}

fn has_event_handler(raw: &str) -> bool {
    raw.split_whitespace().skip(1).any(|attr| {
        attr.strip_prefix("on")
            .and_then(|rest| rest.split_once('='))
            .is_some_and(|(event, _)| {
                !event.is_empty() && event.bytes().all(|b| b.is_ascii_alphabetic())
            })
    })
}

fn check_document(document: &str, manifest: &AssetManifest) -> Result<(), CspError> {
    // Attribute lookups are case-insensitive, but manifest keys are not, so the
    // reference check reads the original text.
    let original_tags: Vec<&str> = document
        .split('<')
        .skip(1)
        .filter_map(|chunk| chunk.split('>').next())
        .filter(|raw| !raw.starts_with('/') && !raw.starts_with('!'))
        .collect();

    for ((name, raw), original) in tags(document).zip(original_tags) {
        match name.as_str() {
            "script" if attribute(&raw, "src").is_none() => {
                return Err(CspError::DocumentViolation("an inline script"));
            }
            "style" => return Err(CspError::DocumentViolation("a <style> element")),
            "base" => return Err(CspError::DocumentViolation("a <base> element")),
            "meta" if raw.contains("http-equiv") => {
                return Err(CspError::DocumentViolation("a <meta http-equiv> element"));
            }
            _ => {}
        }
        if attribute(&raw, "style").is_some() {
            return Err(CspError::DocumentViolation("a style attribute"));
        }
        if has_event_handler(&raw) {
            return Err(CspError::DocumentViolation("an inline event handler"));
        }
        for attr in ["src", "href"] {
            let Some(range) = attribute(&raw, attr) else {
                continue;
            };
            let value = original.get(range).unwrap_or_default();
            let known = value
                .strip_prefix('/')
                .is_some_and(|key| manifest.get(key).is_some());
            if !known {
                return Err(CspError::UnknownReference(value.to_owned()));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const JS: &str = "assets/index-AbCd1234.js";
    const CSS: &str = "assets/style-WxYz5678.css";

    fn manifest(index: &str, extra: &[(&str, &str)]) -> AssetManifest {
        let mut files = vec![
            (INDEX.to_owned(), index.as_bytes().to_vec()),
            (JS.to_owned(), b"export{};".to_vec()),
            (CSS.to_owned(), b"body{}".to_vec()),
        ];
        for (path, body) in extra {
            files.push(((*path).to_owned(), body.as_bytes().to_vec()));
        }
        AssetManifest::from_files(files).unwrap()
    }

    const GOOD: &str = "<!doctype html><html lang=\"en\"><head><meta charset=\"UTF-8\" />\
<script type=\"module\" crossorigin src=\"/assets/index-AbCd1234.js\"></script>\
<link rel=\"stylesheet\" crossorigin href=\"/assets/style-WxYz5678.css\"></head>\
<body><div id=\"root\"></div></body></html>";

    #[test]
    fn the_policy_is_the_designs_string() {
        // Typed from SECURITY.md §7.3, independently of `POLICY`'s line breaks.
        let expected = [
            "default-src 'none'",
            "script-src 'self'",
            "style-src 'self'",
            "img-src 'self' data:",
            "connect-src 'self'",
            "font-src 'self'",
            "frame-ancestors 'none'",
            "base-uri 'none'",
            "form-action 'none'",
        ]
        .join("; ");
        assert_eq!(POLICY, expected);
        for forbidden in ["unsafe", "report", "http", "*"] {
            assert!(!POLICY.contains(forbidden));
        }
    }

    #[test]
    fn a_vite_shaped_bundle_and_the_placeholder_derive_the_policy() {
        assert_eq!(derive(&manifest(GOOD, &[])).unwrap(), POLICY);
        assert_eq!(derive(&AssetManifest::placeholder()).unwrap(), POLICY);
    }

    #[test]
    fn document_violations_are_refused() {
        let cases = [
            ("<script>alert(1)</script>", "an inline script"),
            ("<style>body{}</style>", "a <style> element"),
            ("<STYLE>body{}</STYLE>", "a <style> element"),
            ("<div style=\"color:red\"></div>", "a style attribute"),
            ("<div onclick=\"x()\"></div>", "an inline event handler"),
            ("<body onload=x()>", "an inline event handler"),
            ("<base href=\"/\">", "a <base> element"),
            (
                "<meta http-equiv=\"Content-Security-Policy\" content=\"\">",
                "a <meta http-equiv> element",
            ),
        ];
        for (fragment, what) in cases {
            let doc = GOOD.replace("<div id=\"root\"></div>", fragment);
            assert_eq!(
                derive(&manifest(&doc, &[])).unwrap_err(),
                CspError::DocumentViolation(what),
                "{fragment}"
            );
        }
    }

    #[test]
    fn references_must_be_root_relative_manifest_entries() {
        for reference in [
            "https://cdn.example/x.js",
            "//cdn.example/x.js",
            "assets/index-AbCd1234.js",
            "/assets/missing-AbCd1234.js",
            "/assets/INDEX-AbCd1234.js",
        ] {
            let doc = GOOD.replace("/assets/index-AbCd1234.js", reference);
            assert_eq!(
                derive(&manifest(&doc, &[])).unwrap_err(),
                CspError::UnknownReference(reference.to_owned()),
            );
        }
    }

    #[test]
    fn a_hash_containing_a_dash_or_underscore_is_still_a_hash() {
        for name in ["index-Ab-d1234.js", "index-Ab_d123-.js", "a-b-AbCd1234.css"] {
            assert!(is_hash_named(name), "{name}");
        }
        for name in ["index-AbCd123.js", "indexAbCd1234.js", "index-AbCd12!4.js"] {
            assert!(!is_hash_named(name), "{name}");
        }
    }

    #[test]
    fn unhashed_assets_are_refused() {
        for name in ["assets/app.js", "assets/app-abc.js", "assets/-AbCd1234.js"] {
            let m = manifest(GOOD, &[(name, "export{};")]);
            assert_eq!(
                derive(&m).unwrap_err(),
                CspError::UnhashedAsset(name.to_owned())
            );
        }
    }

    #[test]
    fn external_origins_in_embedded_text_are_refused_but_inert_uris_pass() {
        let inert = "const ns='http://www.w3.org/2000/svg',e='https://react.dev/errors/418';";
        assert!(derive(&manifest(GOOD, &[("assets/ok-AbCd1234.js", inert)])).is_ok());
        for bad in [
            "fetch('https://example.com/x')",
            "@import url(http://fonts.example/css)",
            "https://www.w3.org.evil.example/",
        ] {
            let m = manifest(GOOD, &[("assets/bad-AbCd1234.js", bad)]);
            assert_eq!(
                derive(&m).unwrap_err(),
                CspError::ExternalUrl("assets/bad-AbCd1234.js".to_owned()),
                "{bad}"
            );
        }
    }
}
