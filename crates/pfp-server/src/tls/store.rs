//! [`TlsMaterial`], its pure validity check, and the [`LeafStore`] seam.
//!
//! There is **no filesystem code here**: `SECURITY.md` §11 allows filesystem access
//! only in the vault, the launcher and `xtask`, and the one crate that parses untrusted
//! socket bytes should not also hold the code that persists the TLS private key.
//! The file-backed store (0600, atomic rename) belongs to the launcher and plugs in
//! through [`LeafStore`]; [`MemoryLeafStore`] is what in-process tests use.

use std::fmt;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use rustls::client::danger::ServerCertVerifier;
use rustls::client::WebPkiServerVerifier;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer, ServerName, UnixTime};
use rustls::sign::CertifiedKey;
use rustls::RootCertStore;
use time::OffsetDateTime;

use crate::redact::Redacted;

/// The names the leaf must be valid for: the canonical origin's IP literal first,
/// then the two names that are in the certificate for completeness.
pub const LEAF_NAMES: [&str; 3] = ["127.0.0.1", "localhost", "::1"];

/// Stored material is re-issued when the leaf has fewer than this many days left.
pub const REISSUE_WHEN_DAYS_LEFT: u64 = 30;

const SECONDS_PER_DAY: u64 = 86_400;

/// Everything the TLS listener needs, and nothing more. There is no field for the
/// CA private key: it does not outlive [`issue`](super::issue::issue).
pub struct TlsMaterial {
    /// The local CA certificate (public). Kept so that trust installation, trust
    /// removal and the trust-status check know which anchor is this application's.
    pub ca_cert_der: CertificateDer<'static>,
    /// The leaf certificate presented by the listener (public).
    pub leaf_cert_der: CertificateDer<'static>,
    /// The leaf private key, PKCS#8 DER. `Redacted`, so no `Debug` can print it.
    pub leaf_key: Redacted<PrivatePkcs8KeyDer<'static>>,
}

impl Clone for TlsMaterial {
    fn clone(&self) -> Self {
        Self {
            ca_cert_der: self.ca_cert_der.clone(),
            leaf_cert_der: self.leaf_cert_der.clone(),
            leaf_key: Redacted::new(self.leaf_key.expose().clone_key()),
        }
    }
}

impl fmt::Debug for TlsMaterial {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Lengths only: certificates are public but noisy, the key is never shown.
        f.debug_struct("TlsMaterial")
            .field("ca_cert_der_len", &self.ca_cert_der.as_ref().len())
            .field("leaf_cert_der_len", &self.leaf_cert_der.as_ref().len())
            .field("leaf_key", &self.leaf_key)
            .finish()
    }
}

/// Why stored material cannot be used and must be re-issued.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaterialError {
    /// The CA certificate cannot be parsed as a trust anchor.
    CaUnusable,
    /// The private key cannot be loaded by the TLS provider.
    KeyUnusable,
    /// The private key is not the key the leaf certificate names.
    KeyMismatch,
    /// The leaf does not verify under the stored CA today for one of
    /// [`LEAF_NAMES`] — wrong issuer, outside the name constraints, not yet valid
    /// or already expired.
    ChainInvalid,
    /// The leaf verifies today but not [`REISSUE_WHEN_DAYS_LEFT`] days from now.
    ExpiresSoon,
    /// The supplied time is before the Unix epoch or out of range.
    ClockOutOfRange,
}

impl fmt::Display for MaterialError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::CaUnusable => "the stored CA certificate is not a usable trust anchor",
            Self::KeyUnusable => "the stored leaf key cannot be loaded",
            Self::KeyMismatch => "the stored leaf key does not match the leaf certificate",
            Self::ChainInvalid => "the stored leaf does not verify under the stored CA",
            Self::ExpiresSoon => "the stored leaf expires within 30 days",
            Self::ClockOutOfRange => "the supplied time is out of range",
        })
    }
}

impl std::error::Error for MaterialError {}

impl TlsMaterial {
    /// Pure check that the material is self-consistent and has at least
    /// [`REISSUE_WHEN_DAYS_LEFT`] days left at `now` (wall clock — certificate
    /// validity is calendar time; this is never compared with the session clock).
    ///
    /// The verification is the one a TLS client performs (`rustls`/webpki): chain
    /// to the stored CA, its critical name constraints, `serverAuth`, validity.
    ///
    /// # Errors
    /// The first [`MaterialError`] found; any of them means "re-issue".
    pub fn validate(&self, now: OffsetDateTime) -> Result<(), MaterialError> {
        let provider = Arc::new(rustls::crypto::ring::default_provider());

        let key = PrivateKeyDer::Pkcs8(self.leaf_key.expose().clone_key());
        let signing_key = provider
            .key_provider
            .load_private_key(key)
            .map_err(|_| MaterialError::KeyUnusable)?;
        CertifiedKey::new(vec![self.leaf_cert_der.clone()], signing_key)
            .keys_match()
            .map_err(|_| MaterialError::KeyMismatch)?;

        let mut roots = RootCertStore::empty();
        roots
            .add(self.ca_cert_der.clone())
            .map_err(|_| MaterialError::CaUnusable)?;
        let verifier = WebPkiServerVerifier::builder_with_provider(Arc::new(roots), provider)
            .build()
            .map_err(|_| MaterialError::CaUnusable)?;

        let today = unix_time(now, 0)?;
        let later = unix_time(now, REISSUE_WHEN_DAYS_LEFT)?;
        for (at, failure) in [
            (today, MaterialError::ChainInvalid),
            (later, MaterialError::ExpiresSoon),
        ] {
            for name in LEAF_NAMES {
                let name = ServerName::try_from(name).map_err(|_| MaterialError::ChainInvalid)?;
                verifier
                    .verify_server_cert(&self.leaf_cert_der, &[], &name, &[], at)
                    .map_err(|_| failure)?;
            }
        }
        Ok(())
    }
}

fn unix_time(now: OffsetDateTime, plus_days: u64) -> Result<UnixTime, MaterialError> {
    let seconds = u64::try_from(now.unix_timestamp())
        .ok()
        .and_then(|s| s.checked_add(plus_days.checked_mul(SECONDS_PER_DAY)?))
        .ok_or(MaterialError::ClockOutOfRange)?;
    Ok(UnixTime::since_unix_epoch(Duration::from_secs(seconds)))
}

/// Why a store operation failed. Carries no path and no bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoreError {
    /// The backing store could not be read.
    Read,
    /// The backing store could not be written.
    Write,
    /// The stored material was refused (wrong permissions or owner, unparsable).
    Refused,
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Read => "the TLS material store could not be read",
            Self::Write => "the TLS material store could not be written",
            Self::Refused => "the stored TLS material was refused",
        })
    }
}

impl std::error::Error for StoreError {}

/// Where the leaf (and the public CA certificate) live between launches. The CA
/// private key is never offered to a store: [`TlsMaterial`] has no field for it.
pub trait LeafStore {
    /// The stored material, or `None` when nothing is stored.
    ///
    /// # Errors
    /// [`StoreError`] when the store exists but cannot be used.
    fn load(&self) -> Result<Option<TlsMaterial>, StoreError>;

    /// Replaces whatever is stored.
    ///
    /// # Errors
    /// [`StoreError::Write`] when the material cannot be persisted.
    fn save(&self, material: &TlsMaterial) -> Result<(), StoreError>;

    /// Removes whatever is stored (`pfp trust remove`). Clearing an empty store succeeds.
    ///
    /// # Errors
    /// [`StoreError::Write`] when the material cannot be removed.
    fn clear(&self) -> Result<(), StoreError>;
}

/// An in-memory [`LeafStore`]: nothing leaves the process.
#[derive(Default)]
pub struct MemoryLeafStore {
    slot: Mutex<Option<TlsMaterial>>,
}

impl MemoryLeafStore {
    /// An empty store.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl fmt::Debug for MemoryLeafStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MemoryLeafStore").finish_non_exhaustive()
    }
}

impl LeafStore for MemoryLeafStore {
    fn load(&self) -> Result<Option<TlsMaterial>, StoreError> {
        Ok(self
            .slot
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone())
    }

    fn save(&self, material: &TlsMaterial) -> Result<(), StoreError> {
        *self.slot.lock().unwrap_or_else(PoisonError::into_inner) = Some(material.clone());
        Ok(())
    }

    fn clear(&self) -> Result<(), StoreError> {
        *self.slot.lock().unwrap_or_else(PoisonError::into_inner) = None;
        Ok(())
    }
}
