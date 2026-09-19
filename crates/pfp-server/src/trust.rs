//! The trust-installation seam (`SECURITY.md` §6.2).
//!
//! Adding the local CA to the user's trust settings is the one step of the TLS
//! bootstrap that changes the machine. It is therefore a **trait**, and this crate
//! contains no implementation that touches an operating system: only
//! [`UnsupportedTrustStore`], which refuses everything and is what every build
//! without a platform implementation uses (the application then runs in decline
//! mode: TLS-only, fingerprint shown in the status console).
//!
//! The real macOS implementation is unsafe FFI into `Security.framework`. It does
//! not belong in this crate (`#![forbid(unsafe_code)]`, and nothing that links this
//! library may be able to name it); it is a module of the launcher **binary**,
//! behind that crate's compile-time and run-time guards. Tests use an in-memory
//! fake and can never reach a keychain.

use std::fmt;

/// Whether the local CA is a trust anchor for TLS server authentication.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrustStatus {
    /// Trusted in the **user** domain for the SSL policy.
    Trusted,
    /// Not trusted.
    NotTrusted,
}

/// How an installation attempt that did not fail ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallOutcome {
    /// The anchor is now trusted.
    Installed,
    /// The user dismissed the system authorisation prompt. Supported: the
    /// application continues in decline mode.
    DeclinedByUser,
}

/// Why a platform operation did not happen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlatformError {
    /// This build has no platform implementation.
    Unsupported,
    /// The run-time interlock refused: the process was not started with the
    /// explicit opt-in the platform implementation requires.
    NotAuthorised,
    /// The operating system reported a failure (its status code; never a path or
    /// certificate bytes).
    Os(i32),
}

impl fmt::Display for PlatformError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsupported => f.write_str("not supported by this build"),
            Self::NotAuthorised => f.write_str("not authorised for this run"),
            Self::Os(code) => write!(f, "the operating system reported status {code}"),
        }
    }
}

impl std::error::Error for PlatformError {}

/// The user's trust settings, as far as this application is concerned. Every
/// method takes the CA certificate's DER bytes (public).
///
/// An implementation must only ever use the **user** trust domain, restricted to
/// the SSL policy — never the admin or system domain.
pub trait TrustStore {
    /// Whether `ca_der` is currently trusted. Must not prompt and must not change anything.
    ///
    /// # Errors
    /// [`PlatformError`] when the status cannot be determined.
    fn status(&self, ca_der: &[u8]) -> Result<TrustStatus, PlatformError>;

    /// Asks the system to trust `ca_der`. May show the system authorisation prompt.
    ///
    /// # Errors
    /// [`PlatformError`] when the attempt could not be made; a user who declines
    /// is `Ok(InstallOutcome::DeclinedByUser)`, not an error.
    fn install(&self, ca_der: &[u8]) -> Result<InstallOutcome, PlatformError>;

    /// Removes the trust setting for `ca_der` (`pfp trust remove`). Removing an
    /// anchor that is not trusted succeeds.
    ///
    /// # Errors
    /// [`PlatformError`] when the setting could not be removed.
    fn remove(&self, ca_der: &[u8]) -> Result<(), PlatformError>;
}

/// The [`TrustStore`] of every build without a platform implementation: each
/// operation returns [`PlatformError::Unsupported`] and nothing is touched.
#[derive(Debug, Clone, Copy, Default)]
pub struct UnsupportedTrustStore;

impl TrustStore for UnsupportedTrustStore {
    fn status(&self, _ca_der: &[u8]) -> Result<TrustStatus, PlatformError> {
        Err(PlatformError::Unsupported)
    }

    fn install(&self, _ca_der: &[u8]) -> Result<InstallOutcome, PlatformError> {
        Err(PlatformError::Unsupported)
    }

    fn remove(&self, _ca_der: &[u8]) -> Result<(), PlatformError> {
        Err(PlatformError::Unsupported)
    }
}

/// In-memory fake: a set of fingerprints and an ordered call log. Test-only, so it
/// never exists in a shipped build.
#[cfg(test)]
pub(crate) mod fake {
    use super::{InstallOutcome, PlatformError, TrustStatus, TrustStore};
    use std::cell::RefCell;
    use std::collections::BTreeSet;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) enum Call {
        Status,
        Install,
        Remove,
    }

    /// What the simulated user and system do when `install` is called.
    #[derive(Debug, Clone, Copy)]
    pub(crate) enum Script {
        Approve,
        Decline,
        Fail(PlatformError),
    }

    pub(crate) struct FakeTrustStore {
        script: Script,
        trusted: RefCell<BTreeSet<Vec<u8>>>,
        calls: RefCell<Vec<Call>>,
    }

    impl FakeTrustStore {
        pub(crate) fn new(script: Script) -> Self {
            Self {
                script,
                trusted: RefCell::default(),
                calls: RefCell::default(),
            }
        }

        pub(crate) fn calls(&self) -> Vec<Call> {
            self.calls.borrow().clone()
        }
    }

    impl TrustStore for FakeTrustStore {
        fn status(&self, ca_der: &[u8]) -> Result<TrustStatus, PlatformError> {
            self.calls.borrow_mut().push(Call::Status);
            if let Script::Fail(error) = self.script {
                return Err(error);
            }
            Ok(if self.trusted.borrow().contains(ca_der) {
                TrustStatus::Trusted
            } else {
                TrustStatus::NotTrusted
            })
        }

        fn install(&self, ca_der: &[u8]) -> Result<InstallOutcome, PlatformError> {
            self.calls.borrow_mut().push(Call::Install);
            match self.script {
                Script::Approve => {
                    self.trusted.borrow_mut().insert(ca_der.to_vec());
                    Ok(InstallOutcome::Installed)
                }
                Script::Decline => Ok(InstallOutcome::DeclinedByUser),
                Script::Fail(error) => Err(error),
            }
        }

        fn remove(&self, ca_der: &[u8]) -> Result<(), PlatformError> {
            self.calls.borrow_mut().push(Call::Remove);
            if let Script::Fail(error) = self.script {
                return Err(error);
            }
            self.trusted.borrow_mut().remove(ca_der);
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::fake::{Call, FakeTrustStore, Script};
    use super::{InstallOutcome, PlatformError, TrustStatus, TrustStore, UnsupportedTrustStore};

    const CA_A: &[u8] = b"public certificate bytes A";
    const CA_B: &[u8] = b"public certificate bytes B";

    #[test]
    fn unsupported_store_refuses_everything() {
        let store: &dyn TrustStore = &UnsupportedTrustStore;
        assert_eq!(store.status(CA_A), Err(PlatformError::Unsupported));
        assert_eq!(store.install(CA_A), Err(PlatformError::Unsupported));
        assert_eq!(store.remove(CA_A), Err(PlatformError::Unsupported));
    }

    #[test]
    fn approve_installs_exactly_the_given_anchor_and_remove_undoes_it() {
        let store = FakeTrustStore::new(Script::Approve);
        assert_eq!(store.status(CA_A), Ok(TrustStatus::NotTrusted));
        assert_eq!(store.install(CA_A), Ok(InstallOutcome::Installed));
        assert_eq!(store.status(CA_A), Ok(TrustStatus::Trusted));
        assert_eq!(store.status(CA_B), Ok(TrustStatus::NotTrusted));
        assert_eq!(store.remove(CA_A), Ok(()));
        assert_eq!(store.status(CA_A), Ok(TrustStatus::NotTrusted));
        // Removing an anchor that is not trusted succeeds.
        assert_eq!(store.remove(CA_B), Ok(()));
        assert_eq!(
            store.calls(),
            [
                Call::Status,
                Call::Install,
                Call::Status,
                Call::Status,
                Call::Remove,
                Call::Status,
                Call::Remove
            ]
        );
    }

    #[test]
    fn declining_is_an_outcome_not_an_error_and_trusts_nothing() {
        let store = FakeTrustStore::new(Script::Decline);
        assert_eq!(store.install(CA_A), Ok(InstallOutcome::DeclinedByUser));
        assert_eq!(store.status(CA_A), Ok(TrustStatus::NotTrusted));
    }

    #[test]
    fn interlock_and_os_failures_surface_and_trust_nothing() {
        for error in [PlatformError::NotAuthorised, PlatformError::Os(-25_293)] {
            let store = FakeTrustStore::new(Script::Fail(error));
            assert_eq!(store.install(CA_A), Err(error));
            assert_eq!(store.remove(CA_A), Err(error));
            assert_eq!(store.status(CA_A), Err(error));
        }
    }

    #[test]
    fn errors_describe_themselves_without_any_input() {
        assert_eq!(
            PlatformError::Unsupported.to_string(),
            "not supported by this build"
        );
        assert_eq!(
            PlatformError::NotAuthorised.to_string(),
            "not authorised for this run"
        );
        assert!(PlatformError::Os(-1).to_string().ends_with("status -1"));
    }
}
