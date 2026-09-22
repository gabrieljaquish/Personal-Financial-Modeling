//! `cargo xtask build-web`: builds the front end that `pfp-server` embeds
//! (`ARCHITECTURE.md` §9.2).
//!
//! 1. `npm ci --ignore-scripts` in `web/` — exactly what the lockfile pins, no
//!    lifecycle script runs (skipped with `--skip-install` when `node_modules` is
//!    already in place);
//! 2. `npm run build` — `tsc --noEmit`, the web tests, `vite build`, and
//!    `web/scripts/write-stamp.mjs`, which records the front-end input hash;
//! 3. the emitted tree is checked: `index.html` plus a flat `assets/` directory
//!    whose every file name carries a content hash, and nothing else;
//! 4. the files are listed in sorted order with their SHA-256, so two builds of one
//!    commit can be compared line by line;
//! 5. the front-end **input hash** that `npm run build` recorded beside the bundle
//!    is recomputed here, in Rust, with the function `pfp-server`'s build script
//!    uses, and the two must agree. A release build refuses a bundle that is
//!    absent or older than its inputs, so a stamp the build script would not
//!    accept is an error now rather than at release time.
//!
//! The stamp is written by `npm run build` itself and not by this command, because
//! `vite build` empties `dist`: a bare `npm run build` (which is what the CI web
//! job and most people type) must leave a tree that still builds in release.
//!
//! Node is a build-time tool: this is the only place the repository runs it.

use std::fs;
use std::path::Path;
use std::process::Command;

use sha2::{Digest, Sha256};

use crate::repo;

#[path = "../../crates/pfp-server/build_support/web_inputs.rs"]
mod web_inputs;

/// Length of the content hash Vite appends to a file name.
const VITE_HASH_LEN: usize = 8;

fn npm(web: &Path, args: &[&str]) -> Result<(), String> {
    let status = Command::new("npm")
        .args(args)
        .current_dir(web)
        .status()
        .map_err(|e| format!("cannot run npm (is Node installed? see web/.nvmrc): {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("`npm {}` failed: {status}", args.join(" ")))
    }
}

/// Whether `name` is `<base>-<8 hash characters>.<extension>`, as the Vite
/// configuration names every emitted asset. The hash alphabet is base64url, so it
/// may itself contain `-` and `_`.
pub(crate) fn is_hash_named(name: &str) -> bool {
    let Some((stem, extension)) = name.rsplit_once('.') else {
        return false;
    };
    let bytes = stem.as_bytes();
    if extension.is_empty() || bytes.len() < VITE_HASH_LEN + 2 {
        return false;
    }
    let (base, hash) = bytes.split_at(bytes.len() - VITE_HASH_LEN);
    base.last() == Some(&b'-')
        && hash
            .iter()
            .all(|b| b.is_ascii_alphanumeric() || *b == b'_' || *b == b'-')
}

/// The bundle's files relative to `dist`, sorted; an error names the first file
/// that is not `index.html` or a hash-named file directly under `assets/`.
pub(crate) fn check_tree(dist: &Path) -> Result<Vec<String>, String> {
    let mut files = Vec::new();
    for entry in fs::read_dir(dist).map_err(|e| format!("cannot read {}: {e}", dist.display()))? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name().to_string_lossy().into_owned();
        match name.as_str() {
            "index.html" => files.push(name),
            "assets" if entry.path().is_dir() => {
                for asset in fs::read_dir(entry.path()).map_err(|e| e.to_string())? {
                    let asset = asset.map_err(|e| e.to_string())?;
                    let asset_name = asset.file_name().to_string_lossy().into_owned();
                    if asset.path().is_dir() || !is_hash_named(&asset_name) {
                        return Err(format!(
                            "web/dist/assets/{asset_name} is not a content-hashed file"
                        ));
                    }
                    files.push(format!("assets/{asset_name}"));
                }
            }
            // The input-hash stamp `npm run build` writes last. Not an asset: the
            // server's loader embeds `index.html` and `assets/` only.
            ".dist-stamp" => {}
            other => return Err(format!("unexpected entry in web/dist: {other}")),
        }
    }
    files.sort();
    if !files.iter().any(|f| f == "index.html") {
        return Err("web/dist has no index.html".to_owned());
    }
    Ok(files)
}

pub(crate) fn run(args: &[String]) -> Result<bool, String> {
    let mut skip_install = false;
    for arg in args {
        match arg.as_str() {
            "--skip-install" => skip_install = true,
            other => return Err(format!("build-web: unknown argument `{other}`")),
        }
    }
    let web = repo::root().join("web");
    if skip_install {
        if !web.join("node_modules").is_dir() {
            return Err("--skip-install needs web/node_modules; run without it once".to_owned());
        }
    } else {
        npm(&web, &["ci", "--ignore-scripts"])?;
    }
    npm(&web, &["run", "build"])?;

    let dist = web.join("dist");
    let files = check_tree(&dist)?;
    for file in &files {
        let bytes = fs::read(dist.join(file)).map_err(|e| format!("{file}: {e}"))?;
        let digest = Sha256::digest(&bytes)
            .iter()
            .fold(String::new(), |mut out, byte| {
                use std::fmt::Write as _;
                let _ = write!(out, "{byte:02x}");
                out
            });
        println!("{digest}  {file}");
    }

    let hash = web_inputs::input_hash(&web).map_err(|e| format!("input hash: {e}"))?;
    let recorded = fs::read_to_string(web.join(web_inputs::STAMP_PATH))
        .map_err(|e| format!("`npm run build` left no build stamp: {e}"))?;
    check_stamp(&hash, &recorded)?;
    println!("build-web: {} files; input hash {hash}", files.len());
    Ok(true)
}

/// The stamp `npm run build` wrote must be the hash the Rust side computes.
pub(crate) fn check_stamp(computed: &str, recorded: &str) -> Result<(), String> {
    if recorded.trim() == computed {
        Ok(())
    } else {
        Err(format!(
            "web/scripts/write-stamp.mjs recorded input hash `{}` but \
             build_support/web_inputs.rs computes `{computed}`: the two \
             implementations have diverged, and a release build would be refused as stale",
            recorded.trim()
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_names() {
        for good in [
            "index-okEzKqcJ.js",
            "style-WlaRkXBp.css",
            "a-b-_-AbCd12.svg",
            "x-ab-cd_ef.js",
        ] {
            assert!(is_hash_named(good), "{good}");
        }
        for bad in [
            "index.js",
            "index-abc.js",
            "-okEzKqcJ.js",
            "indexokEzKqcJ.js",
            "index-okEzKq!J.js",
            "noext-okEzKqcJ",
        ] {
            assert!(!is_hash_named(bad), "{bad}");
        }
    }

    fn scratch(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("pfp-xtask-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("assets")).unwrap();
        dir
    }

    #[test]
    fn tree_is_index_plus_hashed_assets_sorted() {
        let dir = scratch("good");
        fs::write(dir.join("index.html"), "x").unwrap();
        fs::write(dir.join("assets/style-WlaRkXBp.css"), "x").unwrap();
        fs::write(dir.join("assets/index-okEzKqcJ.js"), "x").unwrap();
        fs::write(dir.join(".dist-stamp"), "x").unwrap();
        assert_eq!(
            check_tree(&dir).unwrap(),
            [
                "assets/index-okEzKqcJ.js",
                "assets/style-WlaRkXBp.css",
                "index.html"
            ]
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_stamp_that_disagrees_with_the_rust_hash_is_refused() {
        assert!(check_stamp("abc", "abc\n").is_ok());
        let err = check_stamp("abc", "abd\n").unwrap_err();
        assert!(err.contains("diverged"), "{err}");
        assert!(check_stamp("abc", "").is_err());
    }

    #[test]
    fn tree_refuses_unhashed_stray_and_missing_index() {
        let dir = scratch("bad");
        assert!(check_tree(&dir).unwrap_err().contains("no index.html"));
        fs::write(dir.join("index.html"), "x").unwrap();
        fs::write(dir.join("assets/logo.svg"), "x").unwrap();
        assert!(check_tree(&dir)
            .unwrap_err()
            .contains("not a content-hashed"));
        fs::remove_file(dir.join("assets/logo.svg")).unwrap();
        fs::write(dir.join("favicon.ico"), "x").unwrap();
        assert!(check_tree(&dir).unwrap_err().contains("unexpected entry"));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn input_hash_ignores_walk_order_and_sees_content() {
        let dir = scratch("inputs");
        fs::create_dir_all(dir.join("src/session")).unwrap();
        fs::write(dir.join("index.html"), "a").unwrap();
        fs::write(dir.join("src/main.tsx"), "b").unwrap();
        fs::write(dir.join("src/session/x.ts"), "c").unwrap();
        let first = web_inputs::input_hash(&dir).unwrap();
        // The known answer web/tests/stamp.test.mjs asserts over the same three
        // files: `npm run build` writes the stamp, this function checks it.
        assert_eq!(
            first,
            "8c94bd698a53be52353f448cbd178dd29732f99925abb856c4e101e37c85a6c7"
        );
        assert_eq!(first, web_inputs::input_hash(&dir).unwrap());
        assert_eq!(
            web_inputs::input_paths(&dir).unwrap(),
            ["index.html", "src/main.tsx", "src/session/x.ts"]
        );
        fs::write(dir.join("src/session/x.ts"), "d").unwrap();
        assert_ne!(first, web_inputs::input_hash(&dir).unwrap());
        // Not an input: the bundle itself.
        let second = web_inputs::input_hash(&dir).unwrap();
        fs::create_dir_all(dir.join("dist")).unwrap();
        fs::write(dir.join("dist/index.html"), "z").unwrap();
        assert_eq!(second, web_inputs::input_hash(&dir).unwrap());
        fs::remove_dir_all(&dir).unwrap();
    }
}
