//! The canonical origin: `https://127.0.0.1:<port>` and nothing else.
//!
//! There is no hostname form. `localhost` is in the certificate's SANs for
//! completeness, but a request that names it is refused (`SECURITY.md` §7.2): the
//! `Host` allowlist has exactly one entry, and it is a string, not a pattern.

use std::net::SocketAddrV4;

/// The one origin this server answers to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalOrigin {
    port: u16,
    host_header: String,
    origin: String,
}

impl CanonicalOrigin {
    /// The origin for a listener bound to `127.0.0.1:<port>`.
    #[must_use]
    pub fn new(port: u16) -> Self {
        Self {
            port,
            host_header: format!("127.0.0.1:{port}"),
            origin: format!("https://127.0.0.1:{port}"),
        }
    }

    /// The origin of a checked listener address.
    #[must_use]
    pub fn of(addr: SocketAddrV4) -> Self {
        Self::new(addr.port())
    }

    /// The port.
    #[must_use]
    pub const fn port(&self) -> u16 {
        self.port
    }

    /// The only accepted `Host` value: `127.0.0.1:<port>`.
    #[must_use]
    pub fn host_header(&self) -> &str {
        &self.host_header
    }

    /// The only accepted `Origin` value: `https://127.0.0.1:<port>`.
    #[must_use]
    pub fn origin(&self) -> &str {
        &self.origin
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forms_are_the_ip_literal_with_the_port() {
        let origin = CanonicalOrigin::new(8443);
        assert_eq!(origin.host_header(), "127.0.0.1:8443");
        assert_eq!(origin.origin(), "https://127.0.0.1:8443");
        assert_eq!(origin.port(), 8443);
    }
}
