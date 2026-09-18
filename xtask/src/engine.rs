//! The engine crates: the workspace members bound by the purity/determinism
//! contract of `ARCHITECTURE.md` §4.3 and by the dollar-literal lint (ADR-022).

use std::path::Path;

/// `pfp-domain` through `pfp-report` are the engine (`ARCHITECTURE.md` §3). This is
/// the design's enumeration; a crate is created in the milestone that first
/// needs it, so callers take the subset that exists.
pub(crate) const ENGINE_CRATES: [&str; 11] = [
    "pfp-domain",
    "pfp-money",
    "pfp-params",
    "pfp-explain",
    "pfp-model",
    "pfp-tax",
    "pfp-ss",
    "pfp-ledger",
    "pfp-sim",
    "pfp-decide",
    "pfp-report",
];

/// The engine crates that exist under `crates/` today, in design order.
pub(crate) fn existing_engine_crates(root: &Path) -> Vec<&'static str> {
    ENGINE_CRATES
        .into_iter()
        .filter(|name| root.join("crates").join(name).join("Cargo.toml").is_file())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repo;
    use std::fs;

    #[test]
    fn only_engine_crates_that_exist_are_listed_in_design_order() {
        let root = repo::temp_dir("engine").unwrap();
        for name in ["pfp-tax", "pfp-domain", "pfp-server"] {
            let dir = root.join("crates").join(name);
            fs::create_dir_all(&dir).unwrap();
            fs::write(dir.join("Cargo.toml"), b"[package]\n").unwrap();
        }
        // A directory without a manifest is not a crate.
        fs::create_dir_all(root.join("crates").join("pfp-money")).unwrap();

        let found = existing_engine_crates(&root);
        fs::remove_dir_all(&root).unwrap();
        // `pfp-server` is not an engine crate, and the order is the design's, not the disk's.
        assert_eq!(found, vec!["pfp-domain", "pfp-tax"]);
    }
}
