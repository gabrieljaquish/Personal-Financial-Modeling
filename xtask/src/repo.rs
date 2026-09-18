//! Repository discovery and file enumeration shared by the hygiene gates.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The repository root: the parent of this crate's manifest directory.
pub(crate) fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask lives one level below the repository root")
        .to_path_buf()
}

/// Every file that is, or could be, committed: tracked files plus untracked files
/// that `.gitignore` does not exclude, exactly as `git status` would list them.
///
/// This is the working tree *as git sees it*. Ignored build output (`target/`,
/// `node_modules/`, `web/dist/`) and the developer scratch directory `local/` can
/// never reach a commit, so they are not part of the tree these gates protect.
///
/// This is the right enumeration for the gates that are about *what can be
/// committed* (`data-hygiene`, `lint-dollars`, `protected-paths`). It is the wrong
/// one for `check-magic`, which uses [`tree_files`] instead: see there.
///
/// Outside a git repository (an unpacked archive, a temporary directory) every
/// file under `dir` is returned.
pub(crate) fn files(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args([
            "ls-files",
            "-z",
            "--cached",
            "--others",
            "--exclude-standard",
        ])
        .output();
    match output {
        Ok(out) if out.status.success() => {
            let mut paths = Vec::new();
            for rel in out.stdout.split(|b| *b == 0).filter(|s| !s.is_empty()) {
                let rel = String::from_utf8_lossy(rel).into_owned();
                let path = dir.join(&rel);
                // A path in the index that no longer exists on disk, or a submodule
                // directory, has no bytes for a gate to read.
                if path.is_file() {
                    paths.push(path);
                }
            }
            paths.sort();
            Ok(paths)
        }
        _ => walk(dir),
    }
}

/// Directories the container-magic tree scan does not descend into, relative to
/// the repository root. This is the *whole* list, and it is build output and
/// tool state only:
///
/// - `.git`: git's own object store (compressed; history is scanned by
///   `check-magic --history`, which reads the objects through git).
/// - `target`: Cargo build output. The compiled `xtask` binary necessarily
///   contains the magic as the constant it searches for.
/// - `web/dist`: the Vite build output, regenerated from `web/src` on every build.
/// - `local`: developer scratch, git-ignored with no exceptions (`SECURITY.md`
///   §13.1); test runs may legitimately write containers there.
///
/// plus any directory named `node_modules`. It is deliberately NOT derived from
/// `.gitignore`: `.gitignore` lists `*.pfplan`, and a scan that honoured it would
/// skip exactly the file the gate exists to find (ADR-023: no allowlist, ever).
/// No source, fixture, parameter or documentation path may ever be added here.
const TREE_SCAN_SKIPPED_DIRS: [&str; 4] = [".git", "target", "web/dist", "local"];
const TREE_SCAN_SKIPPED_DIR_NAME: &str = "node_modules";

/// Every regular file in the working tree under `root`, ignored or not, except
/// the build-output directories named in [`TREE_SCAN_SKIPPED_DIRS`].
pub(crate) fn tree_files(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut out = Vec::new();
    tree_walk_into(root, root, &mut out)?;
    out.sort();
    Ok(out)
}

fn tree_walk_into(root: &Path, dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries = fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("{}: {e}", dir.display()))?;
        let path = entry.path();
        let kind = entry
            .file_type()
            .map_err(|e| format!("{}: {e}", path.display()))?;
        if kind.is_dir() {
            let rel = relative(root, &path);
            let skipped = TREE_SCAN_SKIPPED_DIRS.contains(&rel.as_str())
                || entry.file_name() == TREE_SCAN_SKIPPED_DIR_NAME;
            if !skipped {
                tree_walk_into(root, &path, out)?;
            }
        } else if kind.is_file() {
            out.push(path);
        }
    }
    Ok(())
}

/// Every regular file under `dir`, recursively, in sorted order.
pub(crate) fn walk(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let mut out = Vec::new();
    walk_into(dir, &mut out)?;
    out.sort();
    Ok(out)
}

fn walk_into(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries = fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("{}: {e}", dir.display()))?;
        let path = entry.path();
        let kind = entry
            .file_type()
            .map_err(|e| format!("{}: {e}", path.display()))?;
        if kind.is_dir() {
            walk_into(&path, out)?;
        } else if kind.is_file() {
            out.push(path);
        }
    }
    Ok(())
}

/// A path relative to `base`, with forward slashes, for reports and rule matching.
pub(crate) fn relative(base: &Path, path: &Path) -> String {
    path.strip_prefix(base)
        .unwrap_or(path)
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

pub(crate) fn read(path: &Path) -> Result<Vec<u8>, String> {
    fs::read(path).map_err(|e| format!("{}: {e}", path.display()))
}

/// A unique, empty temporary directory that the caller removes.
pub(crate) fn temp_dir(label: &str) -> Result<PathBuf, String> {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("pfp-xtask-{label}-{}-{n}", std::process::id()));
    fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    Ok(dir)
}

pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    use std::fmt::Write;
    let digest = Sha256::digest(bytes);
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        write!(hex, "{byte:02x}").expect("writing to a String cannot fail");
    }
    hex
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tree_files_sees_ignored_files_but_not_build_output() {
        let dir = temp_dir("tree-files").unwrap();
        for sub in [
            "src",
            "target/debug",
            "web/dist",
            "web/node_modules/pkg",
            "local",
            "docs/target",
        ] {
            fs::create_dir_all(dir.join(sub)).unwrap();
        }
        // A `.gitignore` that would hide the container from a git-aware listing.
        fs::write(dir.join(".gitignore"), b"*.pfplan\n").unwrap();
        for file in [
            "household.pfplan",
            "src/lib.rs",
            "docs/target/kept.md",
            "target/debug/xtask",
            "web/dist/index.html",
            "web/node_modules/pkg/index.js",
            "local/scratch.bin",
        ] {
            fs::write(dir.join(file), b"synthetic").unwrap();
        }

        let found: Vec<String> = tree_files(&dir)
            .unwrap()
            .iter()
            .map(|p| relative(&dir, p))
            .collect();
        fs::remove_dir_all(&dir).unwrap();

        assert_eq!(
            found,
            [
                ".gitignore",
                "docs/target/kept.md",
                "household.pfplan",
                "src/lib.rs"
            ]
        );
    }
}
