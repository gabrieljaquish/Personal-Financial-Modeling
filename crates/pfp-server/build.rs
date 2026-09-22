//! Build script of `pfp-server`: decides whether the front-end bundle is embedded.
//!
//! It reads `web/` and nothing else, touches no network and writes nothing but
//! cargo directives (`ARCHITECTURE.md` §9.2):
//!
//! * `web/dist/index.html` exists → `cfg(pfp_web_dist)` is set and `assets.rs`
//!   embeds the bundle with `rust-embed`;
//! * it is absent → a **release build fails**; a debug build warns and serves the
//!   built-in placeholder shell, so every Rust test passes on a checkout with no
//!   Node installed;
//! * the bundle is older than its inputs (the hash `cargo xtask build-web` wrote
//!   next to it no longer matches `web/src`, the lockfile, …) → a **release build
//!   fails** ("the front end is stale"); a debug build warns.
//!
//! The validation report follows the same pattern (`PLAN.md` §4.13 item 8):
//!
//! * `build/validation-report.json` exists (written by `cargo xtask
//!   validation-report`) → `cfg(pfp_validation_report)` is set and
//!   `validation.rs` embeds it;
//! * it is absent → a **release build fails**; a debug build warns and the API
//!   serves the explicit "report not generated" state.
//!
//! Its freshness is the drift gate's business (`cargo xtask validation-report
//! --check` in CI and the `xtask` test), not this script's: the report's inputs are
//! the whole tree.

// Not an engine crate, and a build script: the file system is its job.
#![forbid(unsafe_code)]
#![allow(
    clippy::disallowed_types,
    clippy::disallowed_methods,
    clippy::disallowed_macros
)]

use std::path::PathBuf;
use std::process::ExitCode;

#[allow(dead_code)]
#[path = "build_support/web_inputs.rs"]
mod web_inputs;

const REMEDY: &str = "run `cargo xtask build-web` (needs Node; see web/README.md)";
const REPORT_REMEDY: &str = "run `cargo xtask validation-report`";
/// Where `cargo xtask validation-report` writes the JSON, relative to the repository root.
const REPORT_PATH: &str = "build/validation-report.json";

fn main() -> ExitCode {
    println!("cargo:rustc-check-cfg=cfg(pfp_web_dist)");
    println!("cargo:rustc-check-cfg=cfg(pfp_validation_report)");

    let manifest_dir = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap_or_default());
    let root = manifest_dir.join("..").join("..");
    let web = root.join("web");
    let release = std::env::var("PROFILE").is_ok_and(|profile| profile == "release");

    // The validation report, first: it is independent of the bundle.
    let report = root.join(REPORT_PATH);
    println!("cargo:rerun-if-changed={}", report.display());
    if report.is_file() {
        println!("cargo:rustc-cfg=pfp_validation_report");
    } else if release {
        eprintln!(
            "error: {REPORT_PATH} is absent, so there is no validation report to embed: {REPORT_REMEDY}"
        );
        return ExitCode::FAILURE;
    } else {
        println!(
            "cargo:warning={REPORT_PATH} is absent: this debug build serves the \"report not generated\" state; {REPORT_REMEDY}"
        );
    }

    // Re-run when the bundle, its stamp or any input changes. Cargo scans a named
    // directory recursively.
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=build_support/web_inputs.rs");
    println!("cargo:rerun-if-changed={}", web.join("dist").display());
    println!(
        "cargo:rerun-if-changed={}",
        web.join(web_inputs::INPUT_DIR).display()
    );
    for name in web_inputs::INPUT_FILES {
        println!("cargo:rerun-if-changed={}", web.join(name).display());
    }

    if !web.join("dist").join("index.html").is_file() {
        if release {
            eprintln!("error: web/dist is absent, so there is no front end to embed: {REMEDY}");
            return ExitCode::FAILURE;
        }
        println!(
            "cargo:warning=web/dist is absent: this debug build serves the placeholder shell; {REMEDY}"
        );
        return ExitCode::SUCCESS;
    }
    println!("cargo:rustc-cfg=pfp_web_dist");

    let recorded = std::fs::read_to_string(web.join(web_inputs::STAMP_PATH))
        .ok()
        .map(|text| text.trim().to_owned());
    let fresh = match (web_inputs::input_hash(&web), recorded) {
        (Ok(now), Some(then)) => now == then,
        _ => false,
    };
    if !fresh {
        if release {
            eprintln!(
                "error: the front end is stale: web/dist was not built from the current web/ sources: {REMEDY}"
            );
            return ExitCode::FAILURE;
        }
        println!(
            "cargo:warning=the front end is stale: web/dist was not built from the current web/ sources; {REMEDY}"
        );
    }
    ExitCode::SUCCESS
}
