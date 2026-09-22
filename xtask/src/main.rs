//! `cargo xtask`: the repository task runner (`ARCHITECTURE.md` §3).
//!
//! The real subcommands are the repository-hygiene gates that `SECURITY.md` §13
//! and `TESTING.md` §11 require from M0: `check-magic`, `lint-dollars`,
//! `data-hygiene` (which includes `lint-server`) and `protected-paths`; the
//! build helpers `build-web`, `validation-report` and `assumption-catalogue`; and
//! the unsigned half of the release pipeline: `dist` (with its reproducibility
//! report, `dist --compare`), `sbom`, `checksums` and the fail-closed
//! `sign-checksums`. The remaining build helpers (`schema`, `openapi`) are
//! declared here so the command surface is stable, and each states the M0 step
//! that implements it.
//!
//! Exit codes: 0 clean, 1 violations found, 2 usage error or not implemented.

// Not an engine crate: the file system, the clock and process I/O are its job.
#![forbid(unsafe_code)]
#![allow(
    clippy::disallowed_types,
    clippy::disallowed_methods,
    clippy::disallowed_macros
)]

mod archive;
mod build_web;
mod catalogue;
mod checksums;
mod compare;
mod dist;
mod dollars;
mod engine;
mod hygiene;
mod lint_server;
mod macho;
mod magic;
mod protected;
mod repo;
mod report;
mod sbom;

use std::process::ExitCode;

const USAGE: &str = "\
usage: cargo xtask <subcommand> [args]

hygiene gates (real; each exits 1 on a violation):
  check-magic [--history] [PATH ...]
                           fail if the .pfplan container magic appears anywhere in the
                           working tree (default; ignored files included, build output
                           only skipped), or in the given files, directories or archives
                           (.tar, .tgz, .tar.gz, .zip). --history byte-scans every blob
                           in the git object database. No allowlist, ever.
  lint-dollars             fail on dollar literals in engine crates outside tests (ADR-022)
  data-hygiene             data files only under fixtures/ or params/, fixtures carry the
                           synthetic marker or a citation, params carry provenance, locked
                           vintages are unchanged (SECURITY.md §13.3), and every table
                           under params/vintages/ and params/index-series/ passes the
                           pfp-params loader with its index_series resolved to an
                           archived series (TESTING.md §11.2 gate 9); no SSN-shaped
                           string, plan-shaped JSON outside fixtures/plans/ or asOf-stamped
                           JSON outside fixtures/ in any file (§13.4); and everything
                           lint-server checks
  lint-server              std::fs only in pfp-vault/pfp-app, std::net only in pfp-server,
                           across the shipped source of every workspace crate (SECURITY.md
                           §11); no credential-shaped CLI flag, no environment read outside
                           a named allowlist and no credential-shaped environment write
                           anywhere in pfp-app, tests included (§3.7, §7.1, §13.4)
  protected-paths [--lock FILE] [PATH ...]
                           fail if any given path (or stdin, one per line) touches
                           fixtures/tier1/ or a locked params/ vintage (ADR-022). --lock
                           reads the lock from FILE (the base revision's copy) instead
                           of params/VINTAGES.lock
  engine-crates            print the engine crates present in the workspace, one per line

build helpers:
  build-web [--skip-install]
                           npm ci --ignore-scripts (unless --skip-install), npm run build,
                           check that web/dist is index.html + content-hashed assets, list
                           them sorted with their SHA-256 and record the front-end input
                           hash that a release build of pfp-server verifies

  validation-report [--check] [--generated-on YYYY-MM-DD]
                           the validation report from whatever corpora exist (TESTING.md
                           §13; PLAN.md §4.13 item 8): build/validation-report.json, which
                           pfp-server embeds at build time, and docs/validation-report.md.
                           A pure function of the repository; a date appears only when
                           given. --check fails if the committed Markdown (or an existing
                           generated JSON) differs from a fresh build
  assumption-catalogue [--check]
                           docs/assumption-catalogue.md from params/ through the pfp-params
                           loader (authoritative for parameter ids). --check fails on drift

release pipeline, unsigned half (ad-hoc signing only; no key of any kind):
  dist [--universal | --target TRIPLE ...] [--source-date-epoch N] [--source-commit SHA]
       [--out DIR] [--skip-prepare] [--skip-install]
                           validation report and front end, then cargo build --release
                           --locked with no rustc wrapper (so no cargo-auditable: its
                           install path would change the bytes), remapped paths, SOURCE_DATE_EPOCH (the
                           argument, else the last commit time; never the clock) and
                           MACOSX_DEPLOYMENT_TARGET=12.0; lipo for --universal, otherwise a
                           build LABELLED single-arch; record the pre-codesign SHA-256; sign
                           ONCE ad hoc (codesign -s -, hardened runtime, no entitlements);
                           package the .app wrapper and the bare-executable tarball from that
                           one file; check-magic; dist-manifest.json; SHA256SUMS. Output:
                           <target dir>/dist/<label>/ unless --out. No DMG: that is CI-only
  dist --compare <dirA> <dirB> [--report FILE] [--strict]
                           where two dist outputs differ: per file, per Mach-O section and
                           __LINKEDIT blob, per tar member, per manifest line (web/dist per
                           file). Reported, not gating (PLAN.md R21) unless --strict
  sbom [--out DIR] [--target TRIPLE ...] [--source-date-epoch N]
                           CycloneDX JSON: cargo-cyclonedx (pinned) per macOS target, and the
                           npm packages transcribed from web/package-lock.json; then the
                           no-copyleft assertion
  sbom --check FILE...     the assertion alone: no GPL/AGPL/LGPL/PolyForm/Parity/unknown
                           component; no MPL-family or non-allowlisted licence that ships
  checksums DIR            DIR/SHA256SUMS over every file under DIR (unsigned)
  sign-checksums           the Ed25519 signature over SHA256SUMS: NOT implemented; always
                           fails (exit 2) so that nothing ships it unsigned by mistake

build helpers (declared; not implemented at this step of M0):
  schema | openapi
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(cmd) = args.first() else {
        eprint!("{USAGE}");
        return ExitCode::from(2);
    };
    let rest = &args[1..];
    let outcome = match cmd.as_str() {
        "check-magic" => magic::run(rest),
        "lint-dollars" => dollars::run(),
        "data-hygiene" => hygiene::run(),
        "lint-server" => lint_server::run(),
        "protected-paths" => protected::run(rest),
        "engine-crates" => {
            for name in engine::existing_engine_crates(&repo::root()) {
                println!("{name}");
            }
            Ok(true)
        }
        "build-web" => build_web::run(rest),
        "dist" => dist::run(rest),
        "sbom" => sbom::run(rest),
        "checksums" => checksums::run(rest),
        "sign-checksums" => checksums::sign(rest),
        "validation-report" => report::run(rest),
        "assumption-catalogue" => catalogue::run(rest),
        "schema" => not_implemented(cmd, "M0 step 4 (JSON-Schema export via schemars; the plan schema itself freezes at M2, seam S5)"),
        "openapi" => not_implemented(cmd, "M0 step 4 (OpenAPI export from the Rust DTOs via utoipa, snapshot-tested)"),
        "-h" | "--help" | "help" => {
            print!("{USAGE}");
            Ok(true)
        }
        other => {
            eprintln!("xtask: unknown subcommand `{other}`\n");
            eprint!("{USAGE}");
            return ExitCode::from(2);
        }
    };
    match outcome {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::from(1),
        Err(e) => {
            eprintln!("xtask: {e}");
            ExitCode::from(2)
        }
    }
}

/// The build helpers are declared now so hooks and CI can name them; each one
/// says which M0 step implements it rather than pretending to succeed.
fn not_implemented(cmd: &str, step: &str) -> Result<bool, String> {
    Err(format!("`{cmd}` is not implemented until {step}"))
}
