//! The state directory: the lock file and the local certificate. Never a plan.
//!
//! Default `$HOME/Library/Application Support/pfp`; every test and scripted run
//! passes `--state-dir`. Created `0700` and refused if someone else owns it or if
//! group/other can reach into it.

use std::fs;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};

/// Why the state directory cannot be used. Carries no path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateError {
    /// No `--state-dir` and no `HOME`.
    NoHome,
    /// It could not be created or inspected.
    Io,
    /// It exists but is not a directory owned by this user.
    NotOurs,
}

impl std::fmt::Display for StateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::NoHome => "no state directory: pass --state-dir (HOME is not set)",
            Self::Io => "the state directory could not be created",
            Self::NotOurs => "the state directory is not a directory owned by this user",
        })
    }
}

impl std::error::Error for StateError {}

/// The default location under a home directory.
#[must_use]
pub fn default_under(home: &Path) -> PathBuf {
    home.join("Library").join("Application Support").join("pfp")
}

/// Resolves (explicit, else the default under `home`) and prepares the directory:
/// created with mode `0700`, tightened to `0700` if it was looser, refused when it
/// is not a directory this user owns.
///
/// # Errors
/// [`StateError`].
pub fn prepare(explicit: Option<&Path>, home: Option<&Path>) -> Result<PathBuf, StateError> {
    let dir = match (explicit, home) {
        (Some(dir), _) => dir.to_path_buf(),
        (None, Some(home)) => default_under(home),
        (None, None) => return Err(StateError::NoHome),
    };
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(&dir)
        .map_err(|_| StateError::Io)?;
    let meta = fs::symlink_metadata(&dir).map_err(|_| StateError::Io)?;
    if !meta.is_dir() || meta.uid() != rustix::process::getuid().as_raw() {
        return Err(StateError::NotOurs);
    }
    if meta.permissions().mode() & 0o077 != 0 {
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)).map_err(|_| StateError::Io)?;
    }
    Ok(dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn created_0700_and_tightened() {
        let root = std::env::temp_dir().join(format!("pfp-state-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let dir = prepare(Some(&root.join("a/b")), None).unwrap();
        assert_eq!(
            fs::metadata(&dir).unwrap().permissions().mode() & 0o777,
            0o700
        );
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o755)).unwrap();
        prepare(Some(&dir), None).unwrap();
        assert_eq!(
            fs::metadata(&dir).unwrap().permissions().mode() & 0o777,
            0o700
        );

        // A file in the way is refused.
        let file = root.join("file");
        fs::write(&file, b"").unwrap();
        assert!(prepare(Some(&file), None).is_err());
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn default_and_missing_home() {
        assert_eq!(prepare(None, None), Err(StateError::NoHome));
        assert!(default_under(Path::new("/h")).ends_with("Library/Application Support/pfp"));
    }
}
