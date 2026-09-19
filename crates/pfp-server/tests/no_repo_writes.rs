//! Key material never reaches the repository: this crate has no filesystem code at
//! all, and a full issue → store → serve → handshake cycle leaves the crate
//! directory untouched (`SECURITY.md` §11; the machine-safety contract).

// Not an engine crate: tests here legitimately touch sockets, the clock and files.
#![allow(
    clippy::disallowed_types,
    clippy::disallowed_methods,
    clippy::disallowed_macros
)]

mod support;

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use pfp_server::{LeafStore, MemoryLeafStore};
use support::{client_trusting, read_greeting, tls_connect, TestServer, GREETING};

fn crate_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn walk(dir: &Path, visit: &mut dyn FnMut(&Path)) {
    let mut entries: Vec<PathBuf> = fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            walk(&path, visit);
        } else {
            visit(&path);
        }
    }
}

fn snapshot(dir: &Path) -> BTreeMap<PathBuf, (u64, SystemTime)> {
    let mut files = BTreeMap::new();
    walk(dir, &mut |path| {
        let meta = fs::metadata(path).unwrap();
        files.insert(path.to_owned(), (meta.len(), meta.modified().unwrap()));
    });
    files
}

#[test]
fn library_source_has_no_filesystem_access() {
    // Assembled from pieces so this file would not match itself if it were scanned.
    let forbidden = [
        ["std", "::fs"].concat(),
        ["tokio", "::fs"].concat(),
        ["std::os::unix", "::fs"].concat(),
        ["File", "::"].concat(),
        ["Open", "Options"].concat(),
    ];
    let mut scanned = 0;
    walk(&crate_dir().join("src"), &mut |path| {
        let text = fs::read_to_string(path).unwrap();
        for needle in &forbidden {
            assert!(
                !text.contains(needle.as_str()),
                "{} names `{needle}`",
                path.display()
            );
        }
        scanned += 1;
    });
    assert!(
        scanned >= 9,
        "the scan saw the source tree ({scanned} files)"
    );
}

#[tokio::test]
async fn a_full_tls_cycle_writes_nothing_into_the_crate_directory() {
    let before = snapshot(&crate_dir());
    assert!(before.keys().any(|p| p.ends_with("Cargo.toml")));

    let server = TestServer::start();
    let store = MemoryLeafStore::new();
    store.save(&server.material).unwrap();
    let config = client_trusting(&store.load().unwrap().unwrap().ca_cert_der);
    let mut stream = tls_connect(server.addr, config, "127.0.0.1").await.unwrap();
    assert_eq!(read_greeting(&mut stream).await, GREETING);
    drop(stream);
    server.stop().await;

    assert_eq!(snapshot(&crate_dir()), before);
}

#[test]
fn no_key_or_certificate_file_is_tracked_in_this_crate() {
    walk(&crate_dir(), &mut |path| {
        let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        assert!(
            !["pem", "der", "p8", "p12", "pfx", "key", "crt", "cer"].contains(&extension),
            "{} looks like key material",
            path.display()
        );
    });
}
