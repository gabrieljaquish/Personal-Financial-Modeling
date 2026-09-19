//! Process hardening that applies at M0 (`SECURITY.md` §5): no core files, and a
//! panic hook that prints where, never what.
//!
//! Not here yet, by milestone: `mlock`, the fatal-signal zeroizing handlers and
//! zeroize-before-abort arrive with the first key (M1); the hardened runtime is a
//! signing option of the release pipeline.

use std::io::Write as _;

use rustix::process::{getrlimit, setrlimit, Resource, Rlimit};

/// The marker that starts the panic hook's single line.
pub const PANIC_MARKER: &str = "PFP-PANIC";

/// `RLIMIT_CORE = 0` (soft and hard), set before any secret exists. This
/// suppresses BSD core files only; it does not stop the operating system's own
/// crash reporter (`SECURITY.md` §2.3).
///
/// # Errors
/// The OS error number when the limit could not be set.
pub fn disable_core_dumps() -> Result<(), i32> {
    setrlimit(
        Resource::Core,
        Rlimit {
            current: Some(0),
            maximum: Some(0),
        },
    )
    .map_err(rustix::io::Errno::raw_os_error)
}

/// Whether both core limits are zero.
#[must_use]
pub fn core_dumps_disabled() -> bool {
    let limit = getrlimit(Resource::Core);
    limit.current == Some(0) && limit.maximum == Some(0)
}

/// Replaces the default panic hook with one that prints `PFP-PANIC <file>:<line>`
/// and nothing else: never the payload, which is formatted from values.
pub fn install_panic_hook() {
    std::panic::set_hook(Box::new(|info| {
        let mut stderr = std::io::stderr().lock();
        let _ = match info.location() {
            Some(at) => writeln!(stderr, "{PANIC_MARKER} {}:{}", at.file(), at.line()),
            None => writeln!(stderr, "{PANIC_MARKER} unknown"),
        };
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn core_limit_is_zero_after_call() {
        disable_core_dumps().expect("setrlimit");
        assert!(core_dumps_disabled());
        // Idempotent, and it cannot be raised again.
        disable_core_dumps().expect("setrlimit twice");
        assert!(setrlimit(
            Resource::Core,
            Rlimit {
                current: Some(1024),
                maximum: Some(1024)
            }
        )
        .is_err());
        assert!(core_dumps_disabled());
    }
}
