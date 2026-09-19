//! The web shell and its content-hashed assets, served from memory.
//!
//! An [`AssetManifest`] is built once, at startup, from `(path, bytes)` pairs: the
//! shell document `index.html` and the files under `assets/`, whose names carry a
//! content hash. This module does not read a directory and has no filesystem
//! access; whoever embeds the front-end bundle into the binary hands the pairs in
//! ([`AssetManifest::from_files`]). Until a bundle is handed in, the built-in
//! [`AssetManifest::placeholder`] is served: a static page with no script, no
//! style and no external reference, which obeys every header and admission rule.
//!
//! Serving is exact-key lookup in an in-memory map, so there is no directory
//! listing and no path traversal to defend against:
//!
//! * `/` → `index.html`, `Cache-Control: no-store`, no `ETag`;
//! * `/assets/<file>` → the file, `immutable`, a strong `ETag` of its SHA-256; a
//!   matching `If-None-Match` → 304;
//! * anything else → 404. `HEAD` mirrors `GET` without the body.
//!
//! [`AssetManifest::embedded`] is that hand-over for the shipped binary: when the
//! build script found `web/dist` it sets `cfg(pfp_web_dist)` and the bundle is
//! compiled into the executable with `rust-embed` (a proc-macro that inlines the
//! bytes at compile time; nothing is read at run time, in any profile). Without
//! a bundle — a debug build on a checkout with no Node — it is the placeholder; a
//! release build without a fresh bundle does not compile (`build.rs`).
//!
//! Content types come from a closed table; an unknown extension is a startup
//! error, never `application/octet-stream`.

use std::collections::BTreeMap;
use std::fmt;

use axum::body::Body;
use axum::http::header::{CONTENT_LENGTH, CONTENT_TYPE, ETAG, IF_NONE_MATCH};
use axum::http::{HeaderMap, HeaderValue, Response, StatusCode};
use sha2::{Digest, Sha256};

use crate::error::ApiError;

/// The manifest key of the shell document.
pub const INDEX: &str = "index.html";
/// The manifest prefix (and URL prefix, after `/`) of the hashed assets.
pub const ASSETS_PREFIX: &str = "assets/";

const PLACEHOLDER_HTML: &str = "<!doctype html>\n<html lang=\"en\">\n  <head>\n    <meta charset=\"UTF-8\" />\n    <title>Personal Financial Modeling</title>\n  </head>\n  <body>\n    <h1>Personal Financial Modeling</h1>\n    <p>The web bundle has not been built; run cargo xtask build-web.</p>\n  </body>\n</html>\n";

/// `web/dist`, compiled into the binary. The folder is relative to this crate's
/// manifest directory: `rust-embed`'s path interpolation feature is off on purpose
/// (it pulls a crate outside the permitted licence set).
#[cfg(pfp_web_dist)]
#[derive(rust_embed::RustEmbed)]
#[folder = "../../web/dist"]
struct Dist;

/// What kind of resource a file is; decides the CSP directive that governs it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetKind {
    /// The shell document.
    Document,
    /// `script-src`.
    Script,
    /// `style-src`.
    Style,
    /// `img-src`.
    Image,
    /// `font-src`.
    Font,
    /// `connect-src` (fetched data).
    Data,
}

/// The closed content-type table.
fn classify(path: &str) -> Option<(&'static str, AssetKind)> {
    let (_, extension) = path.rsplit_once('.')?;
    Some(match extension {
        "html" => ("text/html; charset=utf-8", AssetKind::Document),
        "js" => ("text/javascript; charset=utf-8", AssetKind::Script),
        "css" => ("text/css; charset=utf-8", AssetKind::Style),
        "svg" => ("image/svg+xml", AssetKind::Image),
        "png" => ("image/png", AssetKind::Image),
        "woff2" => ("font/woff2", AssetKind::Font),
        "json" => ("application/json", AssetKind::Data),
        "txt" => ("text/plain; charset=utf-8", AssetKind::Data),
        _ => return None,
    })
}

/// One embedded file.
#[derive(Clone, PartialEq, Eq)]
pub struct Asset {
    bytes: Vec<u8>,
    content_type: &'static str,
    kind: AssetKind,
    sha256_hex: String,
}

impl fmt::Debug for Asset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Asset")
            .field("len", &self.bytes.len())
            .field("content_type", &self.content_type)
            .field("sha256", &self.sha256_hex)
            .finish_non_exhaustive()
    }
}

impl Asset {
    fn new(path: &str, bytes: Vec<u8>) -> Result<Self, AssetError> {
        let (content_type, kind) =
            classify(path).ok_or_else(|| AssetError::UnknownType(path.to_owned()))?;
        let sha256_hex =
            Sha256::digest(&bytes)
                .iter()
                .fold(String::with_capacity(64), |mut hex, byte| {
                    use fmt::Write as _;
                    let _ = write!(hex, "{byte:02x}");
                    hex
                });
        Ok(Self {
            bytes,
            content_type,
            kind,
            sha256_hex,
        })
    }

    /// The file's bytes.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// The `Content-Type` it is served with.
    #[must_use]
    pub const fn content_type(&self) -> &'static str {
        self.content_type
    }

    /// Which CSP directive governs it.
    #[must_use]
    pub const fn kind(&self) -> AssetKind {
        self.kind
    }

    /// Lower-case hex SHA-256 of the bytes; the strong `ETag`.
    #[must_use]
    pub fn sha256_hex(&self) -> &str {
        &self.sha256_hex
    }

    /// The bytes as text, for the files that are text.
    #[must_use]
    pub fn text(&self) -> Option<&str> {
        match self.kind {
            AssetKind::Font => None,
            AssetKind::Image if self.content_type == "image/png" => None,
            _ => std::str::from_utf8(&self.bytes).ok(),
        }
    }
}

/// Why a set of files is not a servable bundle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssetError {
    /// There is no `index.html`.
    MissingIndex,
    /// A file is neither `index.html` nor under `assets/`.
    UnexpectedPath(String),
    /// A file's extension is not in the content-type table.
    UnknownType(String),
    /// The same path was given twice.
    Duplicate(String),
}

impl fmt::Display for AssetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingIndex => f.write_str("the web bundle has no index.html"),
            Self::UnexpectedPath(p) => write!(f, "unexpected file in the web bundle: {p}"),
            Self::UnknownType(p) => write!(f, "no content type is defined for {p}"),
            Self::Duplicate(p) => write!(f, "the web bundle names {p} twice"),
        }
    }
}

impl std::error::Error for AssetError {}

/// Every file the server can serve, keyed by bundle-relative path, sorted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetManifest {
    files: BTreeMap<String, Asset>,
    placeholder: bool,
}

impl AssetManifest {
    /// The built-in shell served when no bundle was embedded.
    #[must_use]
    pub fn placeholder() -> Self {
        let mut files = BTreeMap::new();
        if let Ok(index) = Asset::new(INDEX, PLACEHOLDER_HTML.as_bytes().to_vec()) {
            files.insert(INDEX.to_owned(), index);
        }
        Self {
            files,
            placeholder: true,
        }
    }

    /// A manifest of `index.html` plus files under `assets/` (one level, as Vite
    /// emits them). Paths are bundle-relative with `/` separators.
    ///
    /// # Errors
    /// [`AssetError`] when the set is not such a bundle.
    pub fn from_files(
        files: impl IntoIterator<Item = (String, Vec<u8>)>,
    ) -> Result<Self, AssetError> {
        let mut map = BTreeMap::new();
        for (path, bytes) in files {
            let is_asset = path
                .strip_prefix(ASSETS_PREFIX)
                .is_some_and(|name| !name.is_empty() && !name.contains('/'));
            if path != INDEX && !is_asset {
                return Err(AssetError::UnexpectedPath(path));
            }
            let asset = Asset::new(&path, bytes)?;
            if map.insert(path.clone(), asset).is_some() {
                return Err(AssetError::Duplicate(path));
            }
        }
        if !map.contains_key(INDEX) {
            return Err(AssetError::MissingIndex);
        }
        Ok(Self {
            files: map,
            placeholder: false,
        })
    }

    /// The bundle compiled into this binary, or the placeholder when the build
    /// found no `web/dist` (debug builds only; a release build refuses to compile
    /// without one). Files are taken in sorted order; dot-files — the build stamp
    /// `cargo xtask build-web` leaves beside the bundle — are not part of it.
    ///
    /// # Errors
    /// [`AssetError`] when what was embedded is not `index.html` + `assets/*`.
    pub fn embedded() -> Result<Self, AssetError> {
        #[cfg(pfp_web_dist)]
        {
            let mut paths: Vec<String> = Dist::iter()
                .map(std::borrow::Cow::into_owned)
                .filter(|path| {
                    !path
                        .rsplit('/')
                        .next()
                        .is_some_and(|name| name.starts_with('.'))
                })
                .collect();
            paths.sort();
            Self::from_files(paths.into_iter().filter_map(|path| {
                let bytes = Dist::get(&path)?.data.into_owned();
                Some((path, bytes))
            }))
        }
        #[cfg(not(pfp_web_dist))]
        {
            Ok(Self::placeholder())
        }
    }

    /// Whether this is the built-in placeholder.
    #[must_use]
    pub const fn is_placeholder(&self) -> bool {
        self.placeholder
    }

    /// The file at a bundle-relative path.
    #[must_use]
    pub fn get(&self, path: &str) -> Option<&Asset> {
        self.files.get(path)
    }

    /// Every `(path, file)`, sorted by path.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &Asset)> {
        self.files
            .iter()
            .map(|(path, asset)| (path.as_str(), asset))
    }

    /// Answers a `GET`/`HEAD` for `url_path` (admission has already limited the
    /// method). The caller adds the policy headers.
    #[must_use]
    pub fn respond(&self, url_path: &str, head: bool, request: &HeaderMap) -> Response<Body> {
        let (asset, hashed) = if url_path == "/" {
            (self.files.get(INDEX), false)
        } else {
            let key = url_path
                .strip_prefix('/')
                .filter(|key| key.starts_with(ASSETS_PREFIX));
            (key.and_then(|key| self.files.get(key)), true)
        };
        let Some(asset) = asset else {
            return ApiError::NOT_FOUND.response(!head);
        };

        let etag = format!("\"{}\"", asset.sha256_hex);
        let not_modified = hashed
            && request
                .get_all(IF_NONE_MATCH)
                .iter()
                .filter_map(|value| value.to_str().ok())
                .flat_map(|value| value.split(','))
                .any(|candidate| candidate.trim() == etag);

        let mut response = Response::new(if head || not_modified {
            Body::empty()
        } else {
            Body::from(asset.bytes.clone())
        });
        let headers = response.headers_mut();
        if hashed {
            if let Ok(value) = HeaderValue::from_str(&etag) {
                headers.insert(ETAG, value);
            }
        }
        if not_modified {
            *response.status_mut() = StatusCode::NOT_MODIFIED;
        } else {
            headers.insert(CONTENT_TYPE, HeaderValue::from_static(asset.content_type));
            headers.insert(CONTENT_LENGTH, HeaderValue::from(asset.bytes.len()));
        }
        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bundle() -> AssetManifest {
        AssetManifest::from_files([
            (
                INDEX.to_owned(),
                b"<!doctype html><title>t</title>".to_vec(),
            ),
            ("assets/index-AbCd1234.js".to_owned(), b"export{};".to_vec()),
        ])
        .unwrap()
    }

    #[test]
    fn unknown_extension_is_an_error_not_octet_stream() {
        let err = AssetManifest::from_files([
            (INDEX.to_owned(), vec![]),
            ("assets/blob-AbCd1234.bin".to_owned(), vec![]),
        ])
        .unwrap_err();
        assert_eq!(
            err,
            AssetError::UnknownType("assets/blob-AbCd1234.bin".into())
        );
    }

    #[test]
    fn the_bundle_is_index_plus_flat_assets() {
        for bad in ["favicon.svg", "assets/", "assets/a/b.js", "../x.js"] {
            let err =
                AssetManifest::from_files([(INDEX.to_owned(), vec![]), (bad.to_owned(), vec![])])
                    .unwrap_err();
            assert_eq!(err, AssetError::UnexpectedPath(bad.into()));
        }
        assert_eq!(
            AssetManifest::from_files([("assets/a-AbCd1234.js".to_owned(), vec![])]).unwrap_err(),
            AssetError::MissingIndex
        );
    }

    #[test]
    fn document_has_no_etag_and_assets_have_a_strong_one() {
        let m = bundle();
        let none = HeaderMap::new();
        let doc = m.respond("/", false, &none);
        assert_eq!(doc.status(), StatusCode::OK);
        assert!(doc.headers().get(ETAG).is_none());

        let js = m.respond("/assets/index-AbCd1234.js", false, &none);
        let etag = js.headers()[ETAG].clone();
        assert_eq!(etag.len(), 66);
        assert_eq!(js.headers()[CONTENT_LENGTH], "9");

        let mut conditional = HeaderMap::new();
        conditional.insert(IF_NONE_MATCH, etag.clone());
        let again = m.respond("/assets/index-AbCd1234.js", false, &conditional);
        assert_eq!(again.status(), StatusCode::NOT_MODIFIED);
        assert_eq!(again.headers()[ETAG], etag);
        assert!(again.headers().get(CONTENT_LENGTH).is_none());
    }

    #[test]
    fn only_exact_keys_are_served() {
        let m = bundle();
        let none = HeaderMap::new();
        for path in [
            "/index.html",
            "/assets",
            "/assets/",
            "/assets/../index.html",
            "/assets/index-AbCd1234.js/",
            "/assets/INDEX-AbCd1234.js",
            "//",
        ] {
            assert_eq!(
                m.respond(path, false, &none).status(),
                StatusCode::NOT_FOUND,
                "{path}"
            );
        }
    }

    #[test]
    fn placeholder_is_inert() {
        let m = AssetManifest::placeholder();
        assert!(m.is_placeholder());
        let text = m.get(INDEX).unwrap().text().unwrap();
        for forbidden in ["<script", "<style", "style=", "http", "src=", "href="] {
            assert!(!text.contains(forbidden), "{forbidden}");
        }
    }
}
