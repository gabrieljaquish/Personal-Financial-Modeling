//! Source-level rules of this crate, asserted on the source itself (test id S-02,
//! `SECURITY.md` §6.1 and §11). These read the crate's own `src/` and nothing else.
//!
//! * There is no plaintext acceptor **in any profile**: hyper is driven from exactly
//!   one call site, in `accept.rs`, in a function that takes the TLS stream by
//!   concrete type; `axum::serve` and hyper-util's protocol-sniffing builder are not
//!   even compiled (the manifest enables neither feature).
//! * The crate that parses untrusted socket bytes has no filesystem access.

#![allow(
    clippy::disallowed_types,
    clippy::disallowed_methods,
    clippy::disallowed_macros
)]

use std::fs;
use std::path::{Path, PathBuf};

fn sources(dir: &Path, out: &mut Vec<(PathBuf, String)>) {
    for entry in fs::read_dir(dir).expect("read src") {
        let path = entry.expect("entry").path();
        if path.is_dir() {
            sources(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            let text = fs::read_to_string(&path).expect("read source");
            out.push((path, text));
        }
    }
}

/// The code of a file: every line that is not a comment.
fn code(text: &str) -> String {
    text.lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn all_sources() -> Vec<(PathBuf, String)> {
    let mut out = Vec::new();
    sources(&Path::new(env!("CARGO_MANIFEST_DIR")).join("src"), &mut out);
    assert!(out.len() > 10);
    out
}

#[test]
fn hyper_is_driven_from_one_tls_only_call_site() {
    let mut call_sites = Vec::new();
    for (path, text) in all_sources() {
        let code = code(&text);
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        for _ in code.matches("serve_connection") {
            call_sites.push(name.clone());
        }
        for forbidden in [
            "axum::serve",
            "conn::auto",
            "serve_connection_with_upgrades",
        ] {
            assert!(!code.contains(forbidden), "{name} uses {forbidden}");
        }
        if !["accept.rs", "bind.rs"].contains(&name.as_str()) {
            for forbidden in ["TcpListener", ".accept("] {
                assert!(!code.contains(forbidden), "{name} uses {forbidden}");
            }
        }
        if name == "accept.rs" {
            // Whitespace-insensitive: rustfmt breaks the parameter list over lines.
            let squeezed: String = code.split_whitespace().collect();
            assert!(
                squeezed.contains("asyncfnserve_http<S>(stream:TlsStream<TcpStream>,"),
                "the one hyper call site takes the TLS stream by concrete type"
            );
        }
    }
    assert_eq!(call_sites, ["accept.rs"]);
}

#[test]
fn the_crate_has_no_filesystem_access() {
    for (path, text) in all_sources() {
        let code = code(&text);
        for forbidden in [
            "std::fs",
            "tokio::fs",
            "std::os::unix",
            "File::",
            "OpenOptions",
        ] {
            assert!(
                !code.contains(forbidden),
                "{} uses {forbidden}",
                path.display()
            );
        }
    }
}

#[test]
fn no_plaintext_or_client_feature_is_enabled() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Cargo.toml");
    let manifest = fs::read_to_string(manifest).expect("workspace manifest");
    let line = |name: &str| {
        manifest
            .lines()
            .find(|l| l.starts_with(&format!("{name} = ")))
            .unwrap_or_else(|| panic!("{name} is a workspace dependency"))
            .to_owned()
    };
    // `axum::serve` exists only behind axum's `tokio` feature.
    assert!(line("axum").contains("default-features = false"));
    assert!(!line("axum").contains("\"tokio\""));
    // No HTTP client (ADR-020), no HTTP/2.
    assert!(line("hyper").contains("default-features = false"));
    assert!(!line("hyper").contains("client") && !line("hyper").contains("http2"));
    assert!(!line("hyper-util").contains("client") && !line("hyper-util").contains("server"));
    // TLS 1.3 only, ring only.
    assert!(!line("rustls").contains("tls12") && !line("rustls").contains("aws"));
}
