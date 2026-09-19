//! Binding the listener: `127.0.0.1` only, and a self-check that refuses anything
//! else (`SECURITY.md` §6.3, test id S-03).
//!
//! There is no bind-address parameter anywhere in this crate: the address is the
//! constant [`LOOPBACK`], and the only thing a caller can choose is a preferred
//! port. `::1` appears in the certificate for completeness but is not bound in v1.

use std::fmt;
use std::io;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV4, TcpListener};

/// The one address the server ever binds.
pub const LOOPBACK: Ipv4Addr = Ipv4Addr::LOCALHOST;

/// A bound socket that has passed the loopback self-check, and how it was obtained.
///
/// The fields are private and [`bind_loopback`] is the only constructor, so holding
/// a `BindOutcome` is proof that the socket is bound to `127.0.0.1`: the TLS listener
/// accepts nothing else.
#[derive(Debug)]
pub struct BindOutcome {
    listener: TcpListener,
    local_addr: SocketAddrV4,
    fell_back: bool,
}

impl BindOutcome {
    /// The checked local address: always `127.0.0.1:<port>`.
    #[must_use]
    pub fn local_addr(&self) -> SocketAddrV4 {
        self.local_addr
    }

    /// `true` when a preferred port was asked for and was not available, so the
    /// port is OS-assigned instead. The caller must tell the user **before**
    /// opening a browser (`SECURITY.md` §6.3, "the fallback is loud").
    #[must_use]
    pub fn fell_back(&self) -> bool {
        self.fell_back
    }

    pub(crate) fn into_parts(self) -> (TcpListener, SocketAddrV4) {
        (self.listener, self.local_addr)
    }
}

/// Why binding failed.
#[derive(Debug)]
pub enum BindError {
    /// The operating system refused the bind.
    Io(io::Error),
    /// The socket's local address is not `127.0.0.1`. The server refuses to start.
    NotLoopback(SocketAddr),
}

impl fmt::Display for BindError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "cannot bind the loopback listener: {error}"),
            Self::NotLoopback(addr) => write!(
                f,
                "refusing to start: the listener is bound to {addr}, not to 127.0.0.1"
            ),
        }
    }
}

impl std::error::Error for BindError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::NotLoopback(_) => None,
        }
    }
}

/// Binds `127.0.0.1:<preferred>` when a non-zero preferred port is given, and
/// falls back to an OS-assigned port when that port is taken. With `None` (or
/// `Some(0)`) the port is OS-assigned from the start and `fell_back` is `false`.
///
/// **What "taken" means — a known limitation (S-29).** The occupancy probe *is* this
/// bind attempt, so it detects exactly the listeners that make the bind fail. A
/// process listening on the wildcard address `0.0.0.0:<preferred>` with
/// `SO_REUSEADDR` does not: both binds succeed, `fell_back` stays `false` and no
/// warning is raised. That is accepted rather than probed for, because the more
/// specific `127.0.0.1` bind receives every loopback connection while this server
/// runs (verified in both bind orders), so such a listener can never answer at the
/// canonical origin; when this server is not running there is no launch to warn at,
/// and what remains is the ordinary same-user case the leaf certificate and the
/// fingerprint comparison of `SECURITY.md` §6.2 cover.
///
/// # Errors
/// [`BindError::Io`] when no loopback port can be bound, [`BindError::NotLoopback`]
/// when the self-check of [`check_bound_addr`] fails.
pub fn bind_loopback(preferred: Option<u16>) -> Result<BindOutcome, BindError> {
    let preferred = preferred.filter(|port| *port != 0);
    let (listener, fell_back) = match preferred {
        Some(port) => match TcpListener::bind(SocketAddrV4::new(LOOPBACK, port)) {
            Ok(listener) => (listener, false),
            Err(_) => (bind_os_assigned()?, true),
        },
        None => (bind_os_assigned()?, false),
    };
    let local_addr = check_bound_addr(listener.local_addr().map_err(BindError::Io)?)?;
    Ok(BindOutcome {
        listener,
        local_addr,
        fell_back,
    })
}

fn bind_os_assigned() -> Result<TcpListener, BindError> {
    TcpListener::bind(SocketAddrV4::new(LOOPBACK, 0)).map_err(BindError::Io)
}

/// The startup self-check: the address must be exactly IPv4 `127.0.0.1` with a
/// real port. The unspecified address, a LAN address, another `127/8` address and
/// IPv6 loopback are all refused.
///
/// # Errors
/// [`BindError::NotLoopback`] carrying the offending address.
pub fn check_bound_addr(addr: SocketAddr) -> Result<SocketAddrV4, BindError> {
    match addr {
        SocketAddr::V4(v4) if *v4.ip() == LOOPBACK && v4.port() != 0 => Ok(v4),
        other => Err(BindError::NotLoopback(other)),
    }
}

/// `true` when `ip` is the one address this server binds.
#[must_use]
pub fn is_canonical_loopback(ip: IpAddr) -> bool {
    ip == IpAddr::V4(LOOPBACK)
}

#[cfg(test)]
mod tests {
    use super::{bind_loopback, check_bound_addr, is_canonical_loopback, BindError, LOOPBACK};
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

    fn refused(addr: SocketAddr) -> bool {
        matches!(check_bound_addr(addr), Err(BindError::NotLoopback(a)) if a == addr)
    }

    #[test]
    fn accepts_exactly_127_0_0_1() {
        let addr = SocketAddr::from((LOOPBACK, 8443));
        assert_eq!(check_bound_addr(addr).unwrap().port(), 8443);
        assert!(is_canonical_loopback(addr.ip()));
    }

    #[test]
    fn rejects_unspecified() {
        assert!(refused(SocketAddr::from((Ipv4Addr::UNSPECIFIED, 8443))));
        assert!(refused(SocketAddr::from((Ipv6Addr::UNSPECIFIED, 8443))));
    }

    #[test]
    fn rejects_lan() {
        assert!(refused(SocketAddr::from((
            Ipv4Addr::new(192, 168, 1, 10),
            8443
        ))));
        assert!(refused(SocketAddr::from((
            Ipv4Addr::new(10, 0, 0, 1),
            8443
        ))));
    }

    #[test]
    fn rejects_other_loopback_block_addresses() {
        assert!(refused(SocketAddr::from((
            Ipv4Addr::new(127, 0, 0, 2),
            8443
        ))));
        assert!(!is_canonical_loopback(IpAddr::V4(Ipv4Addr::new(
            127, 0, 0, 2
        ))));
    }

    #[test]
    fn rejects_v6_loopback() {
        assert!(refused(SocketAddr::from((Ipv6Addr::LOCALHOST, 8443))));
        // The IPv4-mapped form is an IPv6 socket address and is refused as well.
        let mapped = Ipv4Addr::LOCALHOST.to_ipv6_mapped();
        assert!(refused(SocketAddr::from((mapped, 8443))));
    }

    #[test]
    fn rejects_port_zero() {
        assert!(refused(SocketAddr::from((LOOPBACK, 0))));
    }

    #[test]
    fn os_assigned_bind_is_loopback_and_not_a_fallback() {
        for preferred in [None, Some(0)] {
            let outcome = bind_loopback(preferred).unwrap();
            assert_eq!(*outcome.local_addr().ip(), LOOPBACK);
            assert_ne!(outcome.local_addr().port(), 0);
            assert!(!outcome.fell_back());
            assert_eq!(
                outcome.listener.local_addr().unwrap(),
                SocketAddr::V4(outcome.local_addr())
            );
        }
    }

    #[test]
    fn occupied_preferred_port_falls_back_and_says_so() {
        let first = bind_loopback(None).unwrap();
        let taken = first.local_addr().port();

        let second = bind_loopback(Some(taken)).unwrap();
        assert!(second.fell_back());
        assert_ne!(second.local_addr().port(), taken);
        assert_eq!(*second.local_addr().ip(), LOOPBACK);
    }

    #[test]
    fn free_preferred_port_is_used() {
        // Learn a port that is free right now, release it, then ask for it. Another
        // process could take it in between; then the outcome must say `fell_back`.
        let port = bind_loopback(None).unwrap().local_addr().port();
        let outcome = bind_loopback(Some(port)).unwrap();
        assert_eq!(outcome.fell_back(), outcome.local_addr().port() != port);
    }

    #[test]
    fn errors_name_the_address_and_never_a_hostname() {
        let error = check_bound_addr(SocketAddr::from((Ipv4Addr::UNSPECIFIED, 1))).unwrap_err();
        assert!(error.to_string().contains("0.0.0.0:1"));
    }
}
