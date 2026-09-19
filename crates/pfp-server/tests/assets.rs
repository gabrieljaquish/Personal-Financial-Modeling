//! The embedded shell and its hashed assets over real TLS, and the static half of
//! "zero third-party requests" (test id S-06).

// Not an engine crate: tests here legitimately touch sockets, the clock and files.
#![allow(
    clippy::disallowed_types,
    clippy::disallowed_methods,
    clippy::disallowed_macros
)]

mod support;

use pfp_server::{AssetManifest, Server, ServerConfig, ServerError};
use support::{HttpOptions, HttpServer};

const JS: &str = "/assets/index-AbCd1234.js";
const CSS: &str = "/assets/style-WxYz5678.css";

fn bundle_server() -> HttpServer {
    HttpServer::start_with(HttpOptions {
        assets: support::vite_like_bundle(),
        ..Default::default()
    })
}

#[tokio::test]
async fn document_is_no_store_and_assets_are_immutable_with_a_strong_etag() {
    let server = bundle_server();
    let document = server.send(server.navigate("/")).await;
    assert_eq!(document.status, 200);
    assert_eq!(document.header("cache-control"), Some(support::NO_STORE));
    assert!(document.header("etag").is_none());
    let html = String::from_utf8(document.body.clone()).unwrap();
    assert!(html.contains(JS) && html.contains(CSS));

    for (path, content_type) in [
        (JS, "text/javascript; charset=utf-8"),
        (CSS, "text/css; charset=utf-8"),
    ] {
        let asset = server.send(server.subresource(path)).await;
        assert_eq!(asset.status, 200);
        assert_eq!(asset.header("content-type"), Some(content_type));
        assert_eq!(asset.header("cache-control"), Some(support::IMMUTABLE));
        let etag = asset.header("etag").unwrap().to_owned();
        assert!(etag.starts_with('"') && etag.ends_with('"') && etag.len() == 66);

        let again = server
            .send(server.subresource(path).header("If-None-Match", &etag))
            .await;
        assert_eq!(again.status, 304);
        assert!(again.body.is_empty());
        assert_eq!(again.header("etag"), Some(etag.as_str()));
        assert_eq!(again.header("cache-control"), Some(support::IMMUTABLE));

        let stale = server
            .send(server.subresource(path).header("If-None-Match", "\"0000\""))
            .await;
        assert_eq!(stale.status, 200);
    }
    server.stop().await;
}

#[tokio::test]
async fn head_mirrors_get_without_a_body() {
    let server = bundle_server();
    for request in [
        server.navigate("/"),
        server.subresource(JS),
        server.subresource("/nope"),
    ] {
        let get = server.send(request.clone()).await;
        let head = server.send(request.method("HEAD")).await;
        assert_eq!(head.status, get.status);
        assert!(head.body.is_empty());
        assert_eq!(head.header("content-length"), get.header("content-length"));
        assert_eq!(head.header("content-type"), get.header("content-type"));
    }
    server.stop().await;
}

#[tokio::test]
async fn the_placeholder_shell_obeys_the_same_rules() {
    let server = HttpServer::start();
    let document = server.send(server.navigate("/")).await;
    assert_eq!(document.status, 200);
    let html = String::from_utf8(document.body).unwrap();
    assert!(html.contains("cargo xtask build-web"));
    assert!(!html.contains("<script") && !html.contains("http"));
    assert_eq!(server.send(server.subresource(JS)).await.status, 404);
    server.stop().await;
}

#[test]
fn embedded_assets_name_no_external_origin() {
    // The derivation that gates startup is the same function that proves, on every
    // test run, that a bundle naming a third-party origin cannot be served.
    let tainted = AssetManifest::from_files([
        (
            "index.html".to_owned(),
            b"<!doctype html><title>t</title>".to_vec(),
        ),
        (
            "assets/index-AbCd1234.js".to_owned(),
            b"fetch('https://telemetry.example/collect')".to_vec(),
        ),
    ])
    .unwrap();
    assert!(pfp_server::csp::derive(&tainted).is_err());
    assert!(pfp_server::csp::derive(&support::vite_like_bundle()).is_ok());
    assert!(pfp_server::csp::derive(&AssetManifest::placeholder()).is_ok());
}

#[tokio::test]
async fn startup_refuses_a_bundle_the_policy_does_not_fit() {
    let inline = AssetManifest::from_files([(
        "index.html".to_owned(),
        b"<!doctype html><script>alert(1)</script>".to_vec(),
    )])
    .unwrap();
    let material = pfp_server::issue(time::OffsetDateTime::now_utc()).unwrap();
    let mut config = ServerConfig::new(material);
    config.assets = inline;
    assert!(matches!(Server::bind(config), Err(ServerError::Csp(_))));
}

/// What `build.rs` decided. Without `web/dist` (a checkout with no Node) the
/// embedded manifest is the placeholder; with it, the real bundle.
#[test]
fn the_embedded_manifest_is_the_bundle_or_the_placeholder() {
    let manifest = AssetManifest::embedded().expect("what was embedded is a bundle");
    assert_eq!(manifest.is_placeholder(), cfg!(not(pfp_web_dist)));
    // Whatever was embedded, startup would accept it.
    assert!(pfp_server::csp::derive(&manifest).is_ok());
    // The build stamp beside the bundle is not part of it.
    assert!(manifest
        .iter()
        .all(|(path, _)| !path.contains(".dist-stamp")));
}

#[cfg(pfp_web_dist)]
#[tokio::test]
async fn the_real_bundle_is_served_with_hashed_immutable_assets() {
    let manifest = AssetManifest::embedded().unwrap();
    let paths: Vec<String> = manifest.iter().map(|(p, _)| p.to_owned()).collect();
    let mut sorted = paths.clone();
    sorted.sort();
    assert_eq!(paths, sorted, "deterministic order");
    let has = |extension: &str| {
        paths.iter().any(|p| {
            std::path::Path::new(p)
                .extension()
                .is_some_and(|e| e == extension)
        })
    };
    assert!(has("js") && has("css"));

    let server = HttpServer::start_with(HttpOptions {
        assets: manifest,
        ..Default::default()
    });
    let document = server.send(server.navigate("/")).await;
    assert_eq!(document.status, 200);
    assert_eq!(document.header("cache-control"), Some(support::NO_STORE));
    let html = String::from_utf8(document.body).unwrap();
    assert!(
        !html.contains("cargo xtask build-web"),
        "not the placeholder"
    );
    for path in paths.iter().filter(|p| p.starts_with("assets/")) {
        assert!(html.contains(path.as_str()), "the shell names {path}");
        let asset = server.send(server.subresource(&format!("/{path}"))).await;
        assert_eq!(asset.status, 200, "{path}");
        assert_eq!(asset.header("cache-control"), Some(support::IMMUTABLE));
        assert!(asset.header("etag").is_some());
    }
    // The stamp is not reachable.
    assert_eq!(
        server.send(server.subresource("/.dist-stamp")).await.status,
        404
    );
    server.stop().await;
}
