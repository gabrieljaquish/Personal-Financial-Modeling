//! [`FileLeafStore`]: the design's `0600`-file fallback for the leaf key
//! (`SECURITY.md` §6.1), and the only TLS-key filesystem code in the product. It
//! lives in the launcher because `pfp-server` — the crate that parses untrusted
//! socket bytes — has no filesystem access at all (`SECURITY.md` §11).
//!
//! Layout under `<state-dir>/tls/` (directory `0700`), DER only:
//!
//! * `ca-cert.der` — the local CA **certificate** (public). Kept so that trust
//!   removal and the trust-status check know which anchor is this application's;
//! * `leaf-cert.der` — the leaf certificate (public);
//! * `leaf-key.p8` — the leaf private key, PKCS#8, mode `0600`.
//!
//! The CA **private key is never here**: it does not outlive certificate issuance
//! and [`TlsMaterial`] has no field for it.
//!
//! Every file is written to a fresh `create_new` temporary at mode `0600`,
//! `fsync`ed and renamed into place. [`LeafStore::load`] refuses material whose
//! key file is readable by anyone else or is owned by someone else; the caller
//! then re-issues. Errors carry no path and no bytes.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write as _};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

use pfp_server::{LeafStore, Redacted, StoreError, TlsMaterial};
use rustls::pki_types::{CertificateDer, PrivatePkcs8KeyDer};

/// Directory under the state directory.
pub const TLS_DIR: &str = "tls";
/// The CA certificate (public).
pub const CA_CERT_FILE: &str = "ca-cert.der";
/// The leaf certificate (public).
pub const LEAF_CERT_FILE: &str = "leaf-cert.der";
/// The leaf private key.
pub const LEAF_KEY_FILE: &str = "leaf-key.p8";

/// Upper bound on any stored file; a DER certificate or P-256 key is far smaller.
const MAX_FILE_BYTES: u64 = 64 * 1024;

/// The file-backed store.
#[derive(Debug, Clone)]
pub struct FileLeafStore {
    dir: PathBuf,
}

impl FileLeafStore {
    /// A store under `<state_dir>/tls`. Nothing is touched until it is used.
    #[must_use]
    pub fn new(state_dir: &Path) -> Self {
        Self {
            dir: state_dir.join(TLS_DIR),
        }
    }

    /// Reads a file of ours: regular, owned by this user and — when `private` —
    /// mode exactly `0600`.
    fn read(&self, name: &str, private: bool) -> Result<Option<Vec<u8>>, StoreError> {
        let path = self.dir.join(name);
        let meta = match fs::symlink_metadata(&path) {
            Ok(meta) => meta,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err(StoreError::Read),
        };
        let ours = meta.is_file() && meta.uid() == rustix::process::getuid().as_raw();
        let tight = !private || meta.permissions().mode() & 0o777 == 0o600;
        if !ours || !tight || meta.len() > MAX_FILE_BYTES {
            return Err(StoreError::Refused);
        }
        fs::read(&path).map(Some).map_err(|_| StoreError::Read)
    }

    fn write_atomically(&self, name: &str, bytes: &[u8]) -> io::Result<()> {
        let temporary = self.dir.join(format!(".{name}.tmp-{}", std::process::id()));
        let _ = fs::remove_file(&temporary);
        let result = (|| {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&temporary)?;
            file.write_all(bytes)?;
            file.sync_all()?;
            fs::rename(&temporary, self.dir.join(name))
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result
    }
}

impl LeafStore for FileLeafStore {
    fn load(&self) -> Result<Option<TlsMaterial>, StoreError> {
        let (Some(ca), Some(leaf), Some(key)) = (
            self.read(CA_CERT_FILE, false)?,
            self.read(LEAF_CERT_FILE, false)?,
            self.read(LEAF_KEY_FILE, true)?,
        ) else {
            return Ok(None);
        };
        Ok(Some(TlsMaterial {
            ca_cert_der: CertificateDer::from(ca),
            leaf_cert_der: CertificateDer::from(leaf),
            leaf_key: Redacted::new(PrivatePkcs8KeyDer::from(key)),
        }))
    }

    fn save(&self, material: &TlsMaterial) -> Result<(), StoreError> {
        let write = || -> io::Result<()> {
            fs::DirBuilder::new()
                .recursive(true)
                .mode(0o700)
                .create(&self.dir)?;
            fs::set_permissions(&self.dir, fs::Permissions::from_mode(0o700))?;
            // The key goes last: a crash in between leaves a set that fails
            // validation and is re-issued, never a key without its certificate.
            self.write_atomically(CA_CERT_FILE, material.ca_cert_der.as_ref())?;
            self.write_atomically(LEAF_CERT_FILE, material.leaf_cert_der.as_ref())?;
            self.write_atomically(LEAF_KEY_FILE, material.leaf_key.expose().secret_pkcs8_der())?;
            // Make the renames durable.
            File::open(&self.dir)?.sync_all()
        };
        write().map_err(|_| StoreError::Write)
    }

    fn clear(&self) -> Result<(), StoreError> {
        for name in [LEAF_KEY_FILE, LEAF_CERT_FILE, CA_CERT_FILE] {
            match fs::remove_file(self.dir.join(name)) {
                Ok(()) => {}
                Err(e) if e.kind() == io::ErrorKind::NotFound => {}
                Err(_) => return Err(StoreError::Write),
            }
        }
        // Best effort: the directory stays if something else is in it.
        let _ = fs::remove_dir(&self.dir);
        Ok(())
    }
}
