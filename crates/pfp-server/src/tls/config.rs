//! The `rustls` server configuration (`SECURITY.md` §6.1): `ring` provider,
//! TLS 1.3 only, ALPN `http/1.1` only, no client authentication.
//!
//! TLS 1.2 is not merely disabled: the `tls12` feature of `rustls` is off, so the
//! code is not compiled. `aws-lc-rs` is never enabled either (ADR-001).

use std::fmt;
use std::sync::Arc;

use rustls::pki_types::PrivateKeyDer;
use rustls::ServerConfig;

use super::store::TlsMaterial;

/// The only application protocol offered (HTTP/2 would turn the `Host` rule into
/// `:authority` handling; revisit with response streaming).
pub const ALPN_HTTP_1_1: &[u8] = b"http/1.1";

/// Why a server configuration could not be built.
#[derive(Debug)]
pub struct TlsConfigError(rustls::Error);

impl fmt::Display for TlsConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // `rustls::Error` describes the failure; it never contains key bytes.
        write!(f, "cannot build the TLS server configuration: {}", self.0)
    }
}

impl std::error::Error for TlsConfigError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.0)
    }
}

/// Builds the server configuration that presents `material`'s leaf.
///
/// Only the leaf is sent: a client that trusts the local CA already has it, and
/// one that does not gains nothing from receiving it.
///
/// # Errors
/// When the leaf key cannot be used by the `ring` provider or does not match the
/// leaf certificate.
pub fn server_config(material: &TlsMaterial) -> Result<Arc<ServerConfig>, TlsConfigError> {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let key = PrivateKeyDer::Pkcs8(material.leaf_key.expose().clone_key());
    let mut config = ServerConfig::builder_with_provider(provider)
        .with_protocol_versions(&[&rustls::version::TLS13])
        .map_err(TlsConfigError)?
        .with_no_client_auth()
        .with_single_cert(vec![material.leaf_cert_der.clone()], key)
        .map_err(TlsConfigError)?;
    config.alpn_protocols = vec![ALPN_HTTP_1_1.to_vec()];
    Ok(Arc::new(config))
}
