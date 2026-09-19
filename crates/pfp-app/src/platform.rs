//! The platform seam: everything the launcher would ask of the operating system
//! beyond files and sockets — trust settings, a native alert, opening a browser.
//!
//! This library contains **traits, the do-nothing [`UnsupportedPlatform`] and
//! (under `cfg(test)`) in-memory fakes. It contains no real implementation**, and
//! at M0 neither does the binary: every `pfp` this repository can build runs with
//! [`Platform::unsupported`], so it never reads or changes a trust setting, never
//! shows an alert and never opens a browser. The macOS implementation
//! (`SecTrustSettings*` in the user domain for the SSL policy, an in-process
//! alert, `LSOpenCFURLRef` from inside the process — `SECURITY.md` §6.2, §7.1)
//! lands with the M0 trust spike, reviewed and first run by a person.
//!
//! Rules an implementation must keep:
//! * the launch URL reaches the browser through an in-process API — never argv,
//!   the environment, a file, the clipboard or a spawned `open(1)`;
//! * alerts are shown by this process, never by a spawned interpreter;
//! * trust is installed in the **user** domain for the SSL policy only.

use std::sync::Arc;

use pfp_server::Redacted;
pub use pfp_server::{InstallOutcome, PlatformError, TrustStatus, TrustStore};

/// What the user chose in the trust explanation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlertChoice {
    /// Go on to the system authorisation prompt.
    Continue,
    /// Do not install; run in decline mode.
    Decline,
}

/// Native, in-process alerts.
pub trait Alerter {
    /// The informed-consent explanation shown before a trust anchor is installed.
    ///
    /// # Errors
    /// [`PlatformError`] when no alert could be shown.
    fn explain_trust(&self) -> Result<AlertChoice, PlatformError>;

    /// "Another program is using this application's usual address": shown before
    /// anything is opened when the preferred port was occupied (`SECURITY.md` §6.3).
    ///
    /// # Errors
    /// [`PlatformError`] when no alert could be shown.
    fn warn_port_occupied(&self, preferred: u16) -> Result<(), PlatformError>;
}

/// Opens the launch URL in the user's browser from inside the process.
pub trait BrowserOpener: Send + Sync {
    /// Whether this build can open anything. When `false` the launcher skips the
    /// open and the server's re-open endpoint answers 503.
    fn available(&self) -> bool;

    /// Opens `url`, which carries the launch token in its fragment.
    ///
    /// # Errors
    /// [`PlatformError`] when nothing was opened.
    fn open(&self, url: &Redacted<String>) -> Result<(), PlatformError>;
}

/// The three platform services the launch sequence uses.
pub struct Platform {
    /// Trust settings.
    pub trust: Box<dyn TrustStore + Send>,
    /// Native alerts.
    pub alerter: Box<dyn Alerter + Send>,
    /// The browser opener; shared with the server's re-open hook.
    pub opener: Arc<dyn BrowserOpener>,
}

impl Platform {
    /// The platform of every build at M0: nothing is supported, nothing is touched.
    #[must_use]
    pub fn unsupported() -> Self {
        Self {
            trust: Box::new(UnsupportedPlatform),
            alerter: Box::new(UnsupportedPlatform),
            opener: Arc::new(UnsupportedPlatform),
        }
    }
}

impl std::fmt::Debug for Platform {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Platform")
            .field("can_open", &self.opener.available())
            .finish_non_exhaustive()
    }
}

/// Every operation returns [`PlatformError::Unsupported`].
#[derive(Debug, Clone, Copy, Default)]
pub struct UnsupportedPlatform;

impl TrustStore for UnsupportedPlatform {
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

impl Alerter for UnsupportedPlatform {
    fn explain_trust(&self) -> Result<AlertChoice, PlatformError> {
        Err(PlatformError::Unsupported)
    }
    fn warn_port_occupied(&self, _preferred: u16) -> Result<(), PlatformError> {
        Err(PlatformError::Unsupported)
    }
}

impl BrowserOpener for UnsupportedPlatform {
    fn available(&self) -> bool {
        false
    }
    fn open(&self, _url: &Redacted<String>) -> Result<(), PlatformError> {
        Err(PlatformError::Unsupported)
    }
}

/// In-memory fakes sharing one ordered call log. Test-only: they never exist in a
/// shipped build. The log records call names, never a URL.
#[cfg(test)]
pub(crate) mod fake {
    use std::sync::{Arc, Mutex, PoisonError};

    use super::{
        AlertChoice, Alerter, BrowserOpener, InstallOutcome, Platform, PlatformError, Redacted,
        TrustStatus, TrustStore,
    };

    /// What the fakes share and the test inspects.
    #[derive(Default)]
    pub(crate) struct Recorder {
        pub(crate) calls: Mutex<Vec<String>>,
        pub(crate) opened: Mutex<Vec<String>>,
        pub(crate) trusted: Mutex<bool>,
    }

    impl Recorder {
        fn note(&self, call: &str) {
            self.calls
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push(call.to_owned());
        }
        pub(crate) fn calls(&self) -> Vec<String> {
            self.calls
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .clone()
        }
        pub(crate) fn opened(&self) -> Vec<String> {
            self.opened
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .clone()
        }
    }

    pub(crate) struct FakeTrustStore(pub(crate) Arc<Recorder>, pub(crate) InstallOutcome);
    pub(crate) struct FakeAlerter(pub(crate) Arc<Recorder>, pub(crate) AlertChoice);
    pub(crate) struct FakeOpener(pub(crate) Arc<Recorder>);

    impl TrustStore for FakeTrustStore {
        fn status(&self, _ca: &[u8]) -> Result<TrustStatus, PlatformError> {
            self.0.note("trust.status");
            Ok(
                if *self
                    .0
                    .trusted
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                {
                    TrustStatus::Trusted
                } else {
                    TrustStatus::NotTrusted
                },
            )
        }
        fn install(&self, _ca: &[u8]) -> Result<InstallOutcome, PlatformError> {
            self.0.note("trust.install");
            if self.1 == InstallOutcome::Installed {
                *self
                    .0
                    .trusted
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner) = true;
            }
            Ok(self.1)
        }
        fn remove(&self, _ca: &[u8]) -> Result<(), PlatformError> {
            self.0.note("trust.remove");
            *self
                .0
                .trusted
                .lock()
                .unwrap_or_else(PoisonError::into_inner) = false;
            Ok(())
        }
    }

    impl Alerter for FakeAlerter {
        fn explain_trust(&self) -> Result<AlertChoice, PlatformError> {
            self.0.note("alert.explain_trust");
            Ok(self.1)
        }
        fn warn_port_occupied(&self, preferred: u16) -> Result<(), PlatformError> {
            self.0.note(&format!("alert.port_occupied:{preferred}"));
            Ok(())
        }
    }

    impl BrowserOpener for FakeOpener {
        fn available(&self) -> bool {
            true
        }
        fn open(&self, url: &Redacted<String>) -> Result<(), PlatformError> {
            self.0.note("open");
            self.0
                .opened
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push(url.expose().clone());
            Ok(())
        }
    }

    /// A fully fake platform and its recorder.
    pub(crate) fn platform(
        choice: AlertChoice,
        outcome: InstallOutcome,
    ) -> (Platform, Arc<Recorder>) {
        let recorder = Arc::new(Recorder::default());
        let platform = Platform {
            trust: Box::new(FakeTrustStore(Arc::clone(&recorder), outcome)),
            alerter: Box::new(FakeAlerter(Arc::clone(&recorder), choice)),
            opener: Arc::new(FakeOpener(Arc::clone(&recorder))),
        };
        (platform, recorder)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsupported_platform_does_nothing() {
        let p = Platform::unsupported();
        assert_eq!(p.trust.status(b"x"), Err(PlatformError::Unsupported));
        assert_eq!(p.trust.install(b"x"), Err(PlatformError::Unsupported));
        assert_eq!(p.trust.remove(b"x"), Err(PlatformError::Unsupported));
        assert_eq!(p.alerter.explain_trust(), Err(PlatformError::Unsupported));
        assert_eq!(
            p.alerter.warn_port_occupied(1),
            Err(PlatformError::Unsupported)
        );
        assert!(!p.opener.available());
        assert_eq!(
            p.opener.open(&Redacted::new(String::new())),
            Err(PlatformError::Unsupported)
        );
        assert!(!format!("{p:?}").contains("https"));
    }
}
