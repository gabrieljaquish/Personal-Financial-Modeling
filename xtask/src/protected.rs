//! `protected-paths`: the protected-path check (ADR-022; `SECURITY.md` §13;
//! `TESTING.md` §11 stage 0).
//!
//! Two parts of the tree are the ground truth the engine is tested against, so
//! a change to them is never routine and never an assistant's to make:
//!
//! 1. `fixtures/tier1/`: statute and publisher worked examples and hand-worked
//!    ledgers. A change needs a primary-source citation or a fresh sign-off
//!    (`TESTING.md` §2).
//! 2. Locked `params/` vintages: every file named in `params/VINTAGES.lock`, and
//!    everything inside a vintage directory (`params/vintages/<name>/`) that the
//!    lock covers. A locked vintage is immutable; corrections ship as a new
//!    vintage.
//!
//! The gate is given the paths a change touches (as arguments, or on stdin one
//! per line, e.g. from `git diff --name-only`) and fails if any of them is
//! protected. It is a pure path check: `data-hygiene` is the gate that hashes
//! locked files. The lock is read from the working tree by default; a caller
//! that wants to stop a change from unlocking a vintage and editing it in one
//! step passes the base revision's lock with `--lock FILE`.
//!
//! The gate has no override of its own. A legitimate tier-1 change is a human
//! decision made where the gate is called (review), not a flag here.

use std::collections::BTreeSet;
use std::io::{IsTerminal, Read};
use std::path::{Component, Path};

use crate::{hygiene, repo};

/// The tier-1 fixture directory, without a trailing slash.
const TIER1: &str = "fixtures/tier1";

/// What the lock protects: the files it names and the vintage directories it covers.
#[derive(Debug, Default)]
pub(crate) struct Protection {
    files: BTreeSet<String>,
    dirs: BTreeSet<String>,
}

impl Protection {
    /// Builds the protected set from the text of `params/VINTAGES.lock`.
    pub(crate) fn from_lock(text: &str) -> Result<Self, String> {
        let entries = hygiene::parse_lock(text)?;
        let dirs = hygiene::locked_vintage_dirs(&entries)
            .into_iter()
            .map(|d| d.trim_end_matches('/').to_ascii_lowercase())
            .collect();
        let files = entries
            .into_iter()
            .map(|(_, path)| path.to_ascii_lowercase())
            .collect();
        Ok(Self { files, dirs })
    }

    /// Why the normalized, repository-relative path `rel` is protected, if it is.
    ///
    /// Matching ignores ASCII case: on a case-insensitive file system a path
    /// spelled differently still names the protected file.
    pub(crate) fn reason(&self, rel: &str) -> Option<String> {
        let rel = rel.to_ascii_lowercase();
        if is_at_or_under(&rel, TIER1) {
            return Some(format!(
                "tier-1 fixture ({TIER1}/): a change needs a primary-source citation or a fresh sign-off"
            ));
        }
        if let Some(dir) = self.dirs.iter().find(|d| is_at_or_under(&rel, d)) {
            return Some(format!(
                "locked vintage ({dir}/ is covered by {}): corrections ship as a new vintage",
                hygiene::LOCK_PATH
            ));
        }
        if self.files.contains(&rel) {
            return Some(format!(
                "locked parameter file (named in {}): corrections ship as a new vintage",
                hygiene::LOCK_PATH
            ));
        }
        None
    }
}

fn is_at_or_under(rel: &str, dir: &str) -> bool {
    rel.strip_prefix(dir)
        .is_some_and(|rest| rest.is_empty() || rest.starts_with('/'))
}

/// Normalizes one input path to a repository-relative path with forward slashes.
///
/// Accepts what `git diff --name-only` prints (including its double-quoted form
/// for unusual names), `./`-prefixed paths, backslash separators, `.` and `..`
/// components, and absolute paths under `root`.
pub(crate) fn normalize(root: &Path, raw: &str) -> String {
    let trimmed = raw.trim().trim_matches('"').replace('\\', "/");
    let path = Path::new(&trimmed);
    let path = path.strip_prefix(root).unwrap_or(path);
    let mut parts: Vec<String> = Vec::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => parts.push(part.to_string_lossy().into_owned()),
            Component::ParentDir => {
                parts.pop();
            }
            Component::CurDir | Component::RootDir | Component::Prefix(_) => {}
        }
    }
    parts.join("/")
}

/// Every protected path among `paths`, as `path: reason` messages in input order.
pub(crate) fn check(root: &Path, protection: &Protection, paths: &[String]) -> Vec<String> {
    paths
        .iter()
        .filter(|raw| !raw.trim().is_empty())
        .filter_map(|raw| {
            let rel = normalize(root, raw);
            protection.reason(&rel).map(|why| format!("{rel}: {why}"))
        })
        .collect()
}

/// Splits `[--lock FILE] [PATH ...]` into the lock override and the paths.
fn parse_args(args: &[String]) -> Result<(Option<&str>, Vec<String>), String> {
    let mut lock = None;
    let mut paths = Vec::new();
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        if arg == "--lock" {
            let file = iter
                .next()
                .ok_or("protected-paths: --lock needs a file argument")?;
            lock = Some(file.as_str());
        } else if arg == "--" {
            paths.extend(iter.by_ref().cloned());
        } else {
            paths.push(arg.clone());
        }
    }
    Ok((lock, paths))
}

pub(crate) fn run(args: &[String]) -> Result<bool, String> {
    let root = repo::root();
    let (lock_override, mut paths) = parse_args(args)?;
    if paths.is_empty() {
        let mut stdin = std::io::stdin();
        if stdin.is_terminal() {
            return Err(
                "protected-paths: give paths as arguments or on stdin, one per line".to_string(),
            );
        }
        let mut text = String::new();
        stdin
            .read_to_string(&mut text)
            .map_err(|e| format!("stdin: {e}"))?;
        paths = text.lines().map(str::to_string).collect();
    }

    let lock_file = lock_override.map_or_else(
        || root.join(hygiene::LOCK_PATH),
        |p| Path::new(p).to_path_buf(),
    );
    let protection = if lock_file.is_file() {
        Protection::from_lock(&String::from_utf8_lossy(&repo::read(&lock_file)?))?
    } else if lock_override.is_some() {
        return Err(format!("{}: no such lock file", lock_file.display()));
    } else {
        // No vintage has been locked yet; tier-1 fixtures are still protected.
        Protection::default()
    };

    let hits = check(&root, &protection, &paths);
    for hit in &hits {
        eprintln!("protected-paths: {hit}");
    }
    if hits.is_empty() {
        eprintln!("protected-paths: clean ({} path(s) checked)", paths.len());
        Ok(true)
    } else {
        eprintln!(
            "protected-paths: {} protected path(s) touched (ADR-022)",
            hits.len()
        );
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HASH: &str = "0000000000000000000000000000000000000000000000000000000000000000";

    fn lock() -> Protection {
        // Synthetic lock: one vintage directory and one assumption set.
        let text = format!(
            "# synthetic\n{HASH}  params/vintages/federal-2026/brackets.toml\n{HASH}  params/assumptions/cma-a.toml\n"
        );
        Protection::from_lock(&text).unwrap()
    }

    fn strings(paths: &[&str]) -> Vec<String> {
        paths.iter().map(|p| (*p).to_string()).collect()
    }

    #[test]
    fn tier1_fixtures_are_protected_even_without_a_lock() {
        let root = Path::new("/repo");
        let hits = check(
            root,
            &Protection::default(),
            &strings(&["fixtures/tier1/tax/ex1.json", "fixtures/tier1", "README.md"]),
        );
        assert_eq!(hits.len(), 2, "{hits:?}");
        assert!(
            hits[0].starts_with("fixtures/tier1/tax/ex1.json: tier-1 fixture"),
            "{}",
            hits[0]
        );
    }

    #[test]
    fn unprotected_paths_pass() {
        let root = Path::new("/repo");
        let paths = strings(&[
            "fixtures/tier2/x.json",
            "fixtures/tier10/x.json",
            "fixtures/tier1-notes.md",
            "params/vintages/federal-2027/brackets.toml",
            "params/VINTAGES.lock",
            "params/assumptions/cma-b.toml",
            "crates/pfp-tax/src/lib.rs",
            "",
        ]);
        assert!(check(root, &lock(), &paths).is_empty());
    }

    #[test]
    fn locked_vintage_directory_and_locked_files_are_protected() {
        let root = Path::new("/repo");
        let hits = check(
            root,
            &lock(),
            &strings(&[
                "params/vintages/federal-2026/brackets.toml",
                // A file added to a locked vintage directory is a change to the vintage.
                "params/vintages/federal-2026/new-table.toml",
                "params/vintages/federal-2026",
                "params/assumptions/cma-a.toml",
            ]),
        );
        assert_eq!(hits.len(), 4, "{hits:?}");
        assert!(hits[0].contains("locked vintage"), "{}", hits[0]);
        assert!(hits[3].contains("locked parameter file"), "{}", hits[3]);
    }

    #[test]
    fn spelling_variants_do_not_evade_the_gate() {
        let root = Path::new("/repo");
        let hits = check(
            root,
            &lock(),
            &strings(&[
                "./fixtures/tier1/a.json",
                "/repo/fixtures/tier1/a.json",
                "docs/../fixtures/tier1/a.json",
                "fixtures\\tier1\\a.json",
                "\"fixtures/tier1/odd name.json\"",
                "Fixtures/TIER1/a.json",
                "  params/vintages/Federal-2026/brackets.toml  ",
            ]),
        );
        assert_eq!(hits.len(), 7, "{hits:?}");
    }

    #[test]
    fn normalize_produces_repository_relative_paths() {
        let root = Path::new("/repo");
        assert_eq!(normalize(root, "./a/./b/../c.txt"), "a/c.txt");
        assert_eq!(normalize(root, "/repo/params/x.toml"), "params/x.toml");
        assert_eq!(normalize(root, "../../outside"), "outside");
    }

    #[test]
    fn a_malformed_lock_is_an_error_not_a_pass() {
        assert!(Protection::from_lock("not-a-hash params/vintages/x/y.toml\n").is_err());
    }

    #[test]
    fn arguments_split_into_lock_and_paths() {
        let args = strings(&["--lock", "base.lock", "a", "--", "--lock"]);
        let (lock, paths) = parse_args(&args).unwrap();
        assert_eq!(lock, Some("base.lock"));
        assert_eq!(paths, strings(&["a", "--lock"]));
        assert!(parse_args(&strings(&["--lock"])).is_err());
    }
}
