//! Single instance (`SECURITY.md` §5): an advisory `flock` on `<state-dir>/pfp.lock`,
//! held for the life of the process. The operating system releases it when the
//! process exits, however it exits, so a crash never leaves a stale lock.

use std::fs::{File, OpenOptions};
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

use rustix::fs::{flock, FlockOperation};

/// The lock file's name inside the state directory.
pub const LOCK_FILE: &str = "pfp.lock";

/// Why the lock was not taken.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LockError {
    /// Another instance holds it.
    AlreadyRunning,
    /// The lock file could not be opened.
    Io,
}

/// The held lock. Dropping it (or exiting) releases it.
#[derive(Debug)]
pub struct SingleInstance {
    _file: File,
}

impl SingleInstance {
    /// Takes the lock without blocking.
    ///
    /// # Errors
    /// [`LockError::AlreadyRunning`] when another process (or another handle in
    /// this one) holds it.
    pub fn acquire(state_dir: &Path) -> Result<Self, LockError> {
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .mode(0o600)
            .open(state_dir.join(LOCK_FILE))
            .map_err(|_| LockError::Io)?;
        match flock(&file, FlockOperation::NonBlockingLockExclusive) {
            Ok(()) => Ok(Self { _file: file }),
            Err(e) if e == rustix::io::Errno::WOULDBLOCK => Err(LockError::AlreadyRunning),
            Err(_) => Err(LockError::Io),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn second_holder_is_refused_until_the_first_lets_go() {
        let dir = std::env::temp_dir().join(format!("pfp-lock-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let first = SingleInstance::acquire(&dir).unwrap();
        assert_eq!(
            SingleInstance::acquire(&dir).unwrap_err(),
            LockError::AlreadyRunning
        );
        drop(first);
        let again = SingleInstance::acquire(&dir).unwrap();
        drop(again);
        assert_eq!(
            SingleInstance::acquire(&dir.join("missing")).unwrap_err(),
            LockError::Io
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
