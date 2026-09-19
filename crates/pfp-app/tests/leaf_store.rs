//! `FileLeafStore`: permissions, ownership, atomicity, and the absence of any CA
//! private key on disk. Temporary directories only; key material is generated at
//! run time and removed with the directory.

// Not an engine crate: spawning the built binary, sockets and temp files are the point.
#![forbid(unsafe_code)]
#![allow(
    clippy::disallowed_types,
    clippy::disallowed_methods,
    clippy::disallowed_macros
)]

mod support;

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use pfp_app::leaf_store::{FileLeafStore, CA_CERT_FILE, LEAF_CERT_FILE, LEAF_KEY_FILE, TLS_DIR};
use pfp_server::{issue, LeafStore, StoreError};
use time::OffsetDateTime;

struct Temp(PathBuf);

impl Temp {
    fn new(label: &str) -> Self {
        let dir = support::fresh_temp_dir(label);
        fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }
}

impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn mode(path: &Path) -> u32 {
    fs::metadata(path).unwrap().permissions().mode() & 0o777
}

#[test]
fn round_trip_with_0700_directory_and_0600_key() {
    let temp = Temp::new("leaf-roundtrip");
    let store = FileLeafStore::new(&temp.0);
    assert!(store.load().unwrap().is_none(), "empty store");

    let material = issue(OffsetDateTime::now_utc()).unwrap();
    store.save(&material).unwrap();
    let tls = temp.0.join(TLS_DIR);
    assert_eq!(mode(&tls), 0o700);
    assert_eq!(mode(&tls.join(LEAF_KEY_FILE)), 0o600);

    let loaded = store.load().unwrap().expect("stored");
    assert_eq!(loaded.ca_cert_der, material.ca_cert_der);
    assert_eq!(loaded.leaf_cert_der, material.leaf_cert_der);
    assert!(
        loaded.leaf_key.expose().secret_pkcs8_der()
            == material.leaf_key.expose().secret_pkcs8_der(),
        "<redacted> key differs"
    );
    loaded.validate(OffsetDateTime::now_utc()).unwrap();

    store.clear().unwrap();
    assert!(!tls.exists());
    assert!(store.load().unwrap().is_none());
    store.clear().unwrap();
}

#[test]
fn state_dir_holds_no_ca_key() {
    let temp = Temp::new("leaf-no-ca-key");
    let store = FileLeafStore::new(&temp.0);
    let material = issue(OffsetDateTime::now_utc()).unwrap();
    store.save(&material).unwrap();

    let mut names: Vec<String> = fs::read_dir(temp.0.join(TLS_DIR))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(names, [CA_CERT_FILE, LEAF_CERT_FILE, LEAF_KEY_FILE]);

    // Exactly one private key exists on disk, and it is the LEAF's.
    let key = fs::read(temp.0.join(TLS_DIR).join(LEAF_KEY_FILE)).unwrap();
    assert!(
        key == material.leaf_key.expose().secret_pkcs8_der(),
        "<redacted>"
    );
    let ca = fs::read(temp.0.join(TLS_DIR).join(CA_CERT_FILE)).unwrap();
    assert_eq!(ca, material.ca_cert_der.as_ref());
    // And the leaf certificate file is the certificate: three files, two of them
    // public certificates byte for byte, one the leaf's key. There is no fourth
    // file and no room for a CA key in any of the three.
    let leaf = fs::read(temp.0.join(TLS_DIR).join(LEAF_CERT_FILE)).unwrap();
    assert_eq!(leaf, material.leaf_cert_der.as_ref());
}

#[test]
fn leaf_key_file_is_0600_and_refused_when_loosened() {
    let temp = Temp::new("leaf-loosened");
    let store = FileLeafStore::new(&temp.0);
    store
        .save(&issue(OffsetDateTime::now_utc()).unwrap())
        .unwrap();
    let key = temp.0.join(TLS_DIR).join(LEAF_KEY_FILE);
    for loose in [0o644, 0o640, 0o604, 0o700] {
        fs::set_permissions(&key, fs::Permissions::from_mode(loose)).unwrap();
        assert_eq!(store.load().unwrap_err(), StoreError::Refused, "{loose:o}");
    }
    fs::set_permissions(&key, fs::Permissions::from_mode(0o600)).unwrap();
    assert!(store.load().unwrap().is_some());
}

#[test]
fn a_symlinked_key_is_refused() {
    let temp = Temp::new("leaf-symlink");
    let store = FileLeafStore::new(&temp.0);
    store
        .save(&issue(OffsetDateTime::now_utc()).unwrap())
        .unwrap();
    let key = temp.0.join(TLS_DIR).join(LEAF_KEY_FILE);
    let elsewhere = temp.0.join("elsewhere.p8");
    fs::rename(&key, &elsewhere).unwrap();
    std::os::unix::fs::symlink(&elsewhere, &key).unwrap();
    assert_eq!(store.load().unwrap_err(), StoreError::Refused);
}

#[test]
fn a_missing_file_is_an_empty_store_not_an_error() {
    let temp = Temp::new("leaf-partial");
    let store = FileLeafStore::new(&temp.0);
    store
        .save(&issue(OffsetDateTime::now_utc()).unwrap())
        .unwrap();
    fs::remove_file(temp.0.join(TLS_DIR).join(LEAF_CERT_FILE)).unwrap();
    assert!(store.load().unwrap().is_none());
}

#[test]
fn save_is_atomic_and_leaves_no_temporary() {
    let temp = Temp::new("leaf-atomic");
    let store = FileLeafStore::new(&temp.0);
    let first = issue(OffsetDateTime::now_utc()).unwrap();
    store.save(&first).unwrap();
    // A stale temporary from a crashed run does not block the next save.
    let stale = temp
        .0
        .join(TLS_DIR)
        .join(format!(".{LEAF_KEY_FILE}.tmp-{}", std::process::id()));
    fs::write(&stale, b"partial").unwrap();
    let second = issue(OffsetDateTime::now_utc()).unwrap();
    store.save(&second).unwrap();

    let names: Vec<String> = fs::read_dir(temp.0.join(TLS_DIR))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names.len(), 3, "{names:?}");
    assert!(names.iter().all(|n| !n.contains("tmp")));
    let loaded = store.load().unwrap().unwrap();
    assert_eq!(loaded.leaf_cert_der, second.leaf_cert_der);
    assert_eq!(mode(&temp.0.join(TLS_DIR).join(LEAF_KEY_FILE)), 0o600);
}

#[test]
fn an_unwritable_location_is_a_write_error_without_a_path() {
    let temp = Temp::new("leaf-unwritable");
    let file = temp.0.join("not-a-dir");
    fs::write(&file, b"").unwrap();
    let store = FileLeafStore::new(&file);
    let error = store
        .save(&issue(OffsetDateTime::now_utc()).unwrap())
        .unwrap_err();
    assert_eq!(error, StoreError::Write);
    assert!(!error.to_string().contains("not-a-dir"));
}
