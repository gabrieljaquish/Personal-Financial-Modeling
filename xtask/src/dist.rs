//! `cargo xtask dist`: the **unsigned half** of the release pipeline
//! (`ARCHITECTURE.md` §9 items 3-5 and 7-8; `SECURITY.md` §12; `PLAN.md` §4.1).
//!
//! ```text
//! cargo xtask dist [--universal | --target TRIPLE ...] [--source-date-epoch N]
//!                  [--source-commit SHA] [--out DIR] [--skip-prepare]
//!                  [--skip-install]
//! cargo xtask dist --compare <dirA> <dirB> [--report FILE] [--strict]
//! ```
//!
//! In this order, and only this order:
//!
//! 1. **prepare** — the validation report and the front end (`cargo xtask
//!    validation-report`, `cargo xtask build-web`), which the release profile
//!    refuses to build without;
//! 2. **build** — `cargo build --release --locked -p pfp-app --target T` for
//!    each target, with no rustc wrapper (see below), `--remap-path-prefix` for
//!    the workspace, `$CARGO_HOME`
//!    and the target directory, `SOURCE_DATE_EPOCH` (from the argument or the
//!    last commit — never the wall clock), `MACOSX_DEPLOYMENT_TARGET=12.0`,
//!    `ZERO_AR_DATE=1` and `-Wl,-S` (no debug map: the linker's `N_OSO`/`SO` stabs
//!    name the build directory and a randomly named rustc temporary directory,
//!    and point at object files that exist only on the build machine). With both
//!    macOS targets, `lipo -create` makes one universal Mach-O; with one, the
//!    output is **labelled single-arch** in every file name and in the manifest,
//!    and is never called universal;
//! 3. **record** the SHA-256 of that executable before `codesign` touches it
//!    (the reproducibility digest; see the note on "pre-codesign" below);
//! 4. **sign once, ad hoc**: `codesign --sign - --options runtime
//!    --timestamp=none --identifier <APP_IDENTIFIER>` — no identity, no
//!    keychain, no entitlements. The bare signature must verify;
//! 5. **package both channels from that one signed file**: the three-file `.app`
//!    wrapper (`Contents/Info.plist`, `Contents/MacOS/pfp`; no icon yet) as a
//!    directory and as a deterministic `.tar.gz`, and the bare-executable
//!    `.tar.gz`, both with `LICENSE` and `NOTICE`. The executable in each is
//!    asserted byte-identical to the signed file;
//! 6. **check** the container magic over every input and archive (the DMG is
//!    built only in CI, from the `app/` directory checked here);
//! 7. **write** `dist-manifest.json` and the unsigned `SHA256SUMS`.
//!
//! **"Pre-codesign", not "unsigned".** On arm64 the linker already writes an
//! ad-hoc *linker-signed* code signature into every executable it produces, so
//! the digest recorded in step 3 is of the linker's output, which on arm64
//! carries that linker signature. It is still the right digest to compare across
//! builds: it is what exists before any signing step of this pipeline.
//!
//! **The `.app` signature.** The one signed file is signed as a bare executable.
//! Inside the wrapper, `codesign --verify --strict` of the *bundle* then reports
//! "code has no resources but signature indicates they must be present": a
//! bundle's main executable is normally signed *as the bundle*, which seals
//! `Info.plist` and a `_CodeSignature/CodeResources` into its signature — and a
//! bare executable carrying that seal no longer verifies on its own. One byte-
//! identical signed executable and two channels that both verify cannot both be
//! had. The result is recorded in the manifest, not hidden and not gating; the
//! choice belongs to the signing block (see `packaging/README.md`).
//!
//! **No rustc wrapper, and so no `cargo-auditable`.** Cargo mixes the
//! *absolute path* of `RUSTC_WORKSPACE_WRAPPER` into the `-C metadata` hash of
//! every workspace crate. `cargo auditable build` sets that wrapper to its own
//! path (`current_exe()`), so the executable's symbols, `__text`, `__const`,
//! `LC_UUID` and signature depended on where the tool was installed: by default
//! `/Users/<user>/.cargo/bin`, so no other account could reproduce a build. A
//! version pin does not pin the bytes, and no install path is the same for
//! every verifier without `sudo` or a world-writable directory. The embedded
//! list also over-reported (it named crates that feature unification lists but
//! the release never compiles). The `CycloneDX` SBOM (`cargo xtask sbom`) is the
//! dependency record instead. The build sets `RUSTC_WRAPPER` and
//! `RUSTC_WORKSPACE_WRAPPER` to the empty string, which overrides any Cargo
//! configuration too, and refuses an executable that carries the
//! `cargo-auditable` section.

use std::fs;
use std::io::Write as _;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime};

use serde_json::{json, Value};

use crate::sbom::MAC_TARGETS;
use crate::{archive, checksums, compare, macho, magic, repo};

#[allow(dead_code, unreachable_pub)]
#[path = "../../crates/pfp-app/src/identity.rs"]
mod identity;

/// The deployment target of both slices (`ARCHITECTURE.md` §9 item 3).
pub(crate) const DEPLOYMENT_TARGET: &str = "12.0";
/// The section `cargo-auditable` embeds its dependency list in. A release
/// executable must not have it: its presence means a rustc wrapper ran, and
/// the wrapper's install path is then part of the bytes (see the module note).
const AUDITABLE_SECTION: (&str, &str) = ("__DATA", ".dep-v0");
/// Rustc wrappers, set to the empty string for the release build: an empty
/// value also overrides `build.rustc-wrapper` / `build.rustc-workspace-wrapper`
/// in any Cargo configuration file, which merely unsetting them would not.
const NO_WRAPPERS: [&str; 2] = ["RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER"];
/// The Info.plist template, relative to the repository root.
const INFO_PLIST_TEMPLATE: &str = "packaging/Info.plist";
const ENTITLEMENTS: &str = "packaging/entitlements.plist";
/// What the remapped prefixes become inside the binary.
const REMAP_WORKSPACE: &str = "/pfp";
const REMAP_CARGO_HOME: &str = "/cargo";
const REMAP_TARGET: &str = "/target";

struct Options {
    targets: Vec<String>,
    epoch: Option<u64>,
    commit: Option<String>,
    out: Option<PathBuf>,
    skip_prepare: bool,
    skip_install: bool,
}

fn parse(args: &[String]) -> Result<Options, String> {
    let mut o = Options {
        targets: Vec::new(),
        epoch: None,
        commit: None,
        out: None,
        skip_prepare: false,
        skip_install: false,
    };
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        let mut value = |flag: &str| {
            it.next()
                .cloned()
                .ok_or_else(|| format!("{flag} needs a value"))
        };
        match arg.as_str() {
            "--universal" => {
                o.targets = MAC_TARGETS.iter().map(|t| (*t).to_owned()).collect();
            }
            "--target" => o.targets.push(value("--target")?),
            "--source-date-epoch" => {
                let v = value("--source-date-epoch")?;
                o.epoch = Some(
                    v.parse()
                        .map_err(|_| format!("--source-date-epoch: `{v}` is not a number"))?,
                );
            }
            "--source-commit" => o.commit = Some(value("--source-commit")?),
            "--out" => o.out = Some(PathBuf::from(value("--out")?)),
            "--skip-prepare" => o.skip_prepare = true,
            "--skip-install" => o.skip_install = true,
            other => return Err(format!("dist: unknown argument `{other}`")),
        }
    }
    o.targets.sort();
    o.targets.dedup();
    Ok(o)
}

/// How the build is labelled: in every output name and in the manifest.
pub(crate) fn label(targets: &[String]) -> Result<String, String> {
    let arch = |t: &str| match t {
        "aarch64-apple-darwin" => Ok("arm64"),
        "x86_64-apple-darwin" => Ok("x86_64"),
        other => Err(format!(
            "`{other}` is not a release target (only {} are)",
            MAC_TARGETS.join(" and ")
        )),
    };
    match targets {
        [one] => Ok(format!("macos-{}-single-arch-adhoc", arch(one)?)),
        [a, b] => {
            arch(a)?;
            arch(b)?;
            Ok("macos-universal-adhoc".to_owned())
        }
        _ => Err("dist builds one macOS target (single-arch) or both (universal)".to_owned()),
    }
}

fn run_capture(program: &str, args: &[&str], cwd: &Path) -> Result<String, String> {
    let out = Command::new(program)
        .args(args)
        .current_dir(cwd)
        .output()
        .map_err(|e| format!("cannot run {program}: {e}"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_owned())
    } else {
        Err(format!(
            "`{program} {}` failed ({}): {}",
            args.join(" "),
            out.status,
            String::from_utf8_lossy(&out.stderr).trim()
        ))
    }
}

fn run_status(cmd: &mut Command, what: &str) -> Result<(), String> {
    let status = cmd.status().map_err(|e| format!("{what}: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{what} failed ({status})"))
    }
}

/// Cargo's target directory for the workspace (honours `CARGO_TARGET_DIR`).
pub(crate) fn target_dir(root: &Path) -> PathBuf {
    run_capture(
        "cargo",
        &["metadata", "--format-version", "1", "--no-deps"],
        root,
    )
    .ok()
    .and_then(|text| serde_json::from_str::<Value>(&text).ok())
    .and_then(|v| v["target_directory"].as_str().map(PathBuf::from))
    .unwrap_or_else(|| root.join("target"))
}

/// The committer time of `HEAD`: the only fallback for `SOURCE_DATE_EPOCH`.
pub(crate) fn commit_epoch(root: &Path) -> Result<u64, String> {
    let text = run_capture("git", &["log", "-1", "--format=%ct"], root)?;
    text.parse()
        .map_err(|_| format!("git log gave `{text}`, not a commit time"))
}

fn host_target(root: &Path) -> Result<String, String> {
    let vv = run_capture("rustc", &["-vV"], root)?;
    vv.lines()
        .find_map(|l| l.strip_prefix("host: "))
        .map(str::to_owned)
        .ok_or_else(|| "rustc -vV printed no host".to_owned())
}

fn target_installed(root: &Path, target: &str) -> Result<bool, String> {
    let sysroot = run_capture("rustc", &["--print", "sysroot"], root)?;
    Ok(Path::new(&sysroot)
        .join("lib/rustlib")
        .join(target)
        .join("lib")
        .is_dir())
}

/// The release build of one target: `cargo build --release --locked`, with no
/// rustc wrapper of any kind (see the module note), the remapped rustflags and
/// the fixed environment. Nothing else is changed in the environment.
fn release_build_command(root: &Path, target: &str, rustflags: &str, epoch: u64) -> Command {
    let mut cmd = Command::new("cargo");
    cmd.args([
        "build",
        "--release",
        "--locked",
        "-p",
        "pfp-app",
        "--target",
        target,
    ])
    .current_dir(root)
    .env_remove("RUSTFLAGS")
    .env("CARGO_ENCODED_RUSTFLAGS", rustflags)
    .env("SOURCE_DATE_EPOCH", epoch.to_string())
    .env("MACOSX_DEPLOYMENT_TARGET", DEPLOYMENT_TARGET)
    .env("ZERO_AR_DATE", "1");
    for var in NO_WRAPPERS {
        cmd.env(var, "");
    }
    cmd
}

/// The encoded rustflags (0x1f-separated: the workspace path may contain
/// spaces, which plain `RUSTFLAGS` would split). rustc applies the **last**
/// matching remap, so the most specific prefix comes last.
fn encoded_rustflags(root: &Path, cargo_home: &Path, target: &Path) -> String {
    let mut flags = Vec::new();
    let mut remap = |from: &Path, to: &str| {
        let mut variants = vec![from.to_path_buf()];
        if let Ok(canonical) = fs::canonicalize(from) {
            if canonical != from {
                variants.push(canonical);
            }
        }
        for v in variants {
            flags.push(format!("--remap-path-prefix={}={to}", v.display()));
        }
    };
    remap(root, REMAP_WORKSPACE);
    remap(cargo_home, REMAP_CARGO_HOME);
    remap(target, REMAP_TARGET);
    flags.push("-Clink-arg=-Wl,-S".to_owned());
    flags.join("\u{1f}")
}

/// The rustflags as the manifest records them: prefixes by role, not by path,
/// so that two builds in different directories record the same thing.
fn recorded_rustflags() -> Vec<String> {
    vec![
        format!("--remap-path-prefix=<workspace>={REMAP_WORKSPACE}"),
        format!("--remap-path-prefix=<CARGO_HOME>={REMAP_CARGO_HOME}"),
        format!("--remap-path-prefix=<target dir>={REMAP_TARGET}"),
        "-Clink-arg=-Wl,-S".to_owned(),
    ]
}

/// Renders `packaging/Info.plist`: every placeholder substituted, none left.
pub(crate) fn render_info_plist(template: &str, version: &str) -> Result<String, String> {
    // The template's comments document it for this repository; the shipped
    // file carries none of them.
    let mut text = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(open) = rest.find("<!--") {
        text.push_str(&rest[..open]);
        let close = rest[open..]
            .find("-->")
            .ok_or_else(|| format!("{INFO_PLIST_TEMPLATE}: unterminated comment"))?;
        rest = rest[open + close + 3..].trim_start_matches('\n');
    }
    text.push_str(rest);
    let text = text
        .replace("@IDENTIFIER@", identity::APP_IDENTIFIER)
        .replace("@EXECUTABLE@", identity::EXECUTABLE_NAME)
        .replace("@NAME@", identity::BUNDLE_NAME)
        .replace("@VERSION@", version)
        .replace("@MIN_OS@", DEPLOYMENT_TARGET);
    if let Some(at) = text.find('@') {
        let rest = &text[at..];
        let end = rest
            .find(|c: char| c == '<' || c.is_whitespace())
            .unwrap_or(rest.len());
        return Err(format!(
            "{INFO_PLIST_TEMPLATE}: unsubstituted placeholder {}",
            &rest[..end]
        ));
    }
    Ok(text)
}

fn copy_file(from: &Path, to: &Path, mode: u32) -> Result<(), String> {
    if let Some(parent) = to.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    }
    fs::copy(from, to).map_err(|e| format!("{} -> {}: {e}", from.display(), to.display()))?;
    fs::set_permissions(to, fs::Permissions::from_mode(mode))
        .map_err(|e| format!("{}: {e}", to.display()))
}

fn write_file(to: &Path, bytes: &[u8], mode: u32) -> Result<(), String> {
    if let Some(parent) = to.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    }
    fs::write(to, bytes).map_err(|e| format!("{}: {e}", to.display()))?;
    fs::set_permissions(to, fs::Permissions::from_mode(mode))
        .map_err(|e| format!("{}: {e}", to.display()))
}

/// Every file and directory under `dir` gets mode 0755/0644 and mtime `epoch`,
/// so that the staging directories (the DMG's input in CI) carry no build time.
fn normalise_tree(dir: &Path, epoch: u64) -> Result<(), String> {
    let time = SystemTime::UNIX_EPOCH + Duration::from_secs(epoch);
    let mut stack = vec![dir.to_path_buf()];
    let mut dirs = Vec::new();
    while let Some(d) = stack.pop() {
        for entry in fs::read_dir(&d).map_err(|e| format!("{}: {e}", d.display()))? {
            let path = entry.map_err(|e| e.to_string())?.path();
            let meta =
                fs::symlink_metadata(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            if meta.is_dir() {
                stack.push(path);
            } else if meta.is_file() {
                let exec = meta.permissions().mode() & 0o111 != 0;
                fs::set_permissions(
                    &path,
                    fs::Permissions::from_mode(if exec { 0o755 } else { 0o644 }),
                )
                .map_err(|e| format!("{}: {e}", path.display()))?;
                fs::File::options()
                    .write(true)
                    .open(&path)
                    .and_then(|f| f.set_modified(time))
                    .map_err(|e| format!("{}: {e}", path.display()))?;
            }
        }
        dirs.push(d);
    }
    // Directories last and deepest first: writing a child changes its parent's mtime.
    for d in dirs.iter().rev() {
        fs::set_permissions(d, fs::Permissions::from_mode(0o755))
            .map_err(|e| format!("{}: {e}", d.display()))?;
        fs::File::open(d)
            .and_then(|f| f.set_modified(time))
            .map_err(|e| format!("{}: {e}", d.display()))?;
    }
    Ok(())
}

fn codesign_verify(path: &Path) -> Result<(), String> {
    let out = Command::new("codesign")
        .args(["--verify", "--strict"])
        .arg(path)
        .output()
        .map_err(|e| format!("codesign: {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr)
            .trim()
            .replace(&*path.to_string_lossy(), "<file>"))
    }
}

/// Whether a slice carries the section `cargo-auditable` writes.
fn has_auditable_section(sections: &[(String, String)]) -> bool {
    sections
        .iter()
        .any(|(seg, sect)| seg == AUDITABLE_SECTION.0 && sect == AUDITABLE_SECTION.1)
}

fn arch_of(target: &str) -> &'static str {
    if target.starts_with("aarch64") {
        "arm64"
    } else {
        "x86_64"
    }
}

#[allow(clippy::too_many_lines)]
pub(crate) fn run(args: &[String]) -> Result<bool, String> {
    if args.first().map(String::as_str) == Some("--compare") {
        return compare::run(&args[1..]);
    }
    let mut o = parse(args)?;
    let root = repo::root();
    let started = std::time::Instant::now();

    // --- what to build ------------------------------------------------------
    if o.targets.is_empty() {
        o.targets.push(host_target(&root)?);
    }
    let label = label(&o.targets)?;
    for t in &o.targets {
        if !target_installed(&root, t)? {
            return Err(format!(
                "target {t} is not installed for this toolchain. The universal build needs both \
                 macOS targets (`rustup target add {t}`); a Homebrew Rust has only its host \
                 target, so on such a machine only a single-arch build is possible and the \
                 universal build is CI-only"
            ));
        }
    }
    let epoch = match o.epoch {
        Some(e) => e,
        None => commit_epoch(&root).map_err(|e| {
            format!("no --source-date-epoch and no commit time to take it from ({e}); the wall clock is never used")
        })?,
    };
    let commit = o
        .commit
        .clone()
        .or_else(|| run_capture("git", &["rev-parse", "HEAD"], &root).ok());
    let dirty = run_capture("git", &["status", "--porcelain"], &root)
        .ok()
        .map(|s| !s.is_empty());
    let version = env!("CARGO_PKG_VERSION");
    let target_dir = target_dir(&root);
    let out_root = o.out.clone().unwrap_or_else(|| target_dir.join("dist"));
    let dir = out_root.join(&label);
    eprintln!(
        "dist: {label}; SOURCE_DATE_EPOCH={epoch}; targets {}; output {}",
        o.targets.join(", "),
        dir.display()
    );

    // --- 1. prepare ------------------------------------------------------------
    if o.skip_prepare {
        eprintln!(
            "dist: --skip-prepare: using the existing build/validation-report.json and web/dist"
        );
    } else {
        if !crate::report::run(&[])? {
            return Err("validation-report failed".to_owned());
        }
        let web_args: Vec<String> = if o.skip_install {
            vec!["--skip-install".to_owned()]
        } else {
            Vec::new()
        };
        if !crate::build_web::run(&web_args)? {
            return Err("build-web failed".to_owned());
        }
    }
    let web_dist = root.join("web").join("dist");
    let mut web_listing = Vec::new();
    for path in repo::walk(&web_dist)? {
        web_listing.push(format!(
            "{}  web/dist/{}",
            repo::sha256_hex(&repo::read(&path)?),
            repo::relative(&web_dist, &path)
        ));
    }

    // --- 2. build ----------------------------------------------------------------
    if dir.exists() {
        fs::remove_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    let work = dir.join("work");
    fs::create_dir_all(&work).map_err(|e| format!("{}: {e}", work.display()))?;
    let cargo_home = std::env::var_os("CARGO_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cargo")))
        .ok_or("neither CARGO_HOME nor HOME is set")?;
    let rustflags = encoded_rustflags(&root, &cargo_home, &target_dir);
    let mut pre = serde_json::Map::new();
    let mut slices = Vec::new();
    for t in &o.targets {
        let mut cmd = release_build_command(&root, t, &rustflags, epoch);
        eprintln!("dist: building {t}");
        run_status(&mut cmd, &format!("cargo build --release for {t}"))?;
        let built = target_dir
            .join(t)
            .join("release")
            .join(identity::EXECUTABLE_NAME);
        let copy = work.join(format!("{}-{t}.pre-codesign", identity::EXECUTABLE_NAME));
        copy_file(&built, &copy, 0o755)?;
        pre.insert(t.clone(), json!(repo::sha256_hex(&repo::read(&copy)?)));
        slices.push(copy);
    }
    let unsigned = if slices.len() == 2 {
        let fat = work.join(format!(
            "{}-universal.pre-codesign",
            identity::EXECUTABLE_NAME
        ));
        let mut cmd = Command::new("lipo");
        cmd.arg("-create").arg("-output").arg(&fat).args(&slices);
        run_status(&mut cmd, "lipo -create")?;
        pre.insert(
            "universal".to_owned(),
            json!(repo::sha256_hex(&repo::read(&fat)?)),
        );
        fat
    } else {
        slices[0].clone()
    };

    // --- 3. record, and check what was built ---------------------------------------
    let unsigned_bytes = repo::read(&unsigned)?;
    let unsigned_sha = repo::sha256_hex(&unsigned_bytes);
    let parsed = macho::parse(&unsigned_bytes)?;
    let mut want: Vec<String> = o.targets.iter().map(|t| arch_of(t).to_owned()).collect();
    let mut got = parsed.arches();
    want.sort();
    got.sort();
    if want != got {
        return Err(format!(
            "the executable holds {got:?}, not the requested {want:?}"
        ));
    }
    if parsed.fat != (o.targets.len() == 2) {
        return Err("a single-arch build must be thin and a universal one fat".to_owned());
    }
    let mut uuids = serde_json::Map::new();
    let mut sdk = None;
    for s in &parsed.slices {
        if s.minos.as_deref() != Some(DEPLOYMENT_TARGET) {
            return Err(format!(
                "slice {}: minimum OS {:?}, not {DEPLOYMENT_TARGET}",
                s.arch, s.minos
            ));
        }
        if has_auditable_section(&s.sections) {
            return Err(format!(
                "slice {}: a {},{} section: a rustc wrapper (cargo-auditable) ran, and its \
                 install path is then part of the executable's bytes",
                s.arch, AUDITABLE_SECTION.0, AUDITABLE_SECTION.1
            ));
        }
        uuids.insert(s.arch.clone(), json!(s.uuid));
        sdk.clone_from(&s.sdk);
    }
    eprintln!("dist: pre-codesign SHA-256 {unsigned_sha}");

    // --- 4. sign once, ad hoc --------------------------------------------------------
    let signed = work.join(identity::EXECUTABLE_NAME);
    copy_file(&unsigned, &signed, 0o755)?;
    let sign_args = [
        "--sign",
        "-",
        "--force",
        "--options",
        "runtime",
        "--timestamp=none",
        "--identifier",
        identity::APP_IDENTIFIER,
    ];
    let mut cmd = Command::new("codesign");
    cmd.args(sign_args).arg(&signed);
    run_status(&mut cmd, "codesign (ad hoc)")?;
    codesign_verify(&signed).map_err(|e| format!("the ad-hoc signature does not verify: {e}"))?;
    let signed_bytes = repo::read(&signed)?;
    let signed_sha = repo::sha256_hex(&signed_bytes);
    for s in &macho::parse(&signed_bytes)?.slices {
        if !s.has_code_signature {
            return Err(format!(
                "slice {} carries no code signature after codesign",
                s.arch
            ));
        }
    }
    // Smoke test: the hardened, ad-hoc signed executable runs. `--version` starts nothing.
    let host = host_target(&root)?;
    let smoke = if o.targets.contains(&host) {
        let out = run_capture(&signed.to_string_lossy(), &["--version"], &root)?;
        if out != format!("pfp {version}") {
            return Err(format!(
                "the signed executable printed `{out}` for --version"
            ));
        }
        format!("`pfp --version` printed `{out}` on {host}")
    } else {
        format!("not run: {host} cannot execute this build")
    };
    eprintln!("dist: signed once (ad hoc) {signed_sha}; {smoke}");

    // --- 5. package both channels from the one signed file ----------------------------
    let licence = root.join("LICENSE");
    let notice = root.join("NOTICE");
    let bundle = format!("{}.app", identity::BUNDLE_NAME);
    let app_root = dir.join("app");
    let contents = app_root.join(&bundle).join("Contents");
    let template = fs::read_to_string(root.join(INFO_PLIST_TEMPLATE))
        .map_err(|e| format!("{INFO_PLIST_TEMPLATE}: {e}"))?;
    let plist = render_info_plist(&template, version)?;
    write_file(&contents.join("Info.plist"), plist.as_bytes(), 0o644)?;
    copy_file(
        &signed,
        &contents.join("MacOS").join(identity::EXECUTABLE_NAME),
        0o755,
    )?;
    copy_file(&licence, &app_root.join("LICENSE"), 0o644)?;
    copy_file(&notice, &app_root.join("NOTICE"), 0o644)?;
    normalise_tree(&app_root, epoch)?;
    let app_verify = match codesign_verify(&app_root.join(&bundle)) {
        Ok(()) => "valid".to_owned(),
        Err(e) => format!("does not verify: {e}"),
    };

    let bare_name = format!("{}-{version}-{label}", identity::EXECUTABLE_NAME);
    let bare_root = dir.join("bare").join(&bare_name);
    copy_file(&signed, &bare_root.join(identity::EXECUTABLE_NAME), 0o755)?;
    copy_file(&licence, &bare_root.join("LICENSE"), 0o644)?;
    copy_file(&notice, &bare_root.join("NOTICE"), 0o644)?;
    normalise_tree(&dir.join("bare"), epoch)?;

    let artifacts = dir.join("artifacts");
    fs::create_dir_all(&artifacts).map_err(|e| format!("{}: {e}", artifacts.display()))?;
    let bare_tgz = artifacts.join(format!("{bare_name}.tar.gz"));
    let app_name = format!("{bare_name}-app");
    let app_tgz = artifacts.join(format!("{app_name}.tar.gz"));
    archive::write_tar_gz(&bare_root, &bare_name, epoch, &bare_tgz)?;
    archive::write_tar_gz(&app_root, &app_name, epoch, &app_tgz)?;

    // The same bytes in every channel, read back from what was written.
    let exe_in_app = format!(
        "{app_name}/{bundle}/Contents/MacOS/{}",
        identity::EXECUTABLE_NAME
    );
    let exe_in_bare = format!("{bare_name}/{}", identity::EXECUTABLE_NAME);
    for (tgz, member) in [(&app_tgz, &exe_in_app), (&bare_tgz, &exe_in_bare)] {
        let members = archive::read_tar(&archive::gunzip(&repo::read(tgz)?)?)?;
        let found = members
            .iter()
            .find(|m| &m.path == member)
            .ok_or_else(|| format!("{}: no member {member}", tgz.display()))?;
        if found.bytes != signed_bytes {
            return Err(format!(
                "{}: {member} is not the signed executable",
                tgz.display()
            ));
        }
    }
    for staged in [
        contents.join("MacOS").join(identity::EXECUTABLE_NAME),
        bare_root.join(identity::EXECUTABLE_NAME),
    ] {
        if repo::read(&staged)? != signed_bytes {
            return Err(format!("{}: not the signed executable", staged.display()));
        }
    }

    // --- 6. the container magic, over every input and archive ---------------------------
    let mut hits = Vec::new();
    for target in [&work, &app_root, &dir.join("bare"), &bare_tgz, &app_tgz] {
        magic::scan_target(target, &mut hits)?;
    }
    if !hits.is_empty() {
        for h in &hits {
            eprintln!("check-magic: {h}");
        }
        return Err(
            "the .pfplan container magic is in a release input; this gate has no allowlist"
                .to_owned(),
        );
    }

    // --- 7. manifest and SHA256SUMS ---------------------------------------------------------
    let tool = |p: &str, a: &[&str]| {
        run_capture(p, a, &root).unwrap_or_else(|e| format!("unavailable: {e}"))
    };
    let entitlements =
        fs::read_to_string(root.join(ENTITLEMENTS)).map_err(|e| format!("{ENTITLEMENTS}: {e}"))?;
    let manifest = json!({
        "schema": "pfp-dist-manifest/1",
        "label": label,
        "architectures": parsed.arches(),
        "universal": parsed.fat,
        "single_arch": !parsed.fat,
        "application": {
            "identifier": identity::APP_IDENTIFIER,
            "identifier_is_placeholder": true,
            "version": version,
            "executable": identity::EXECUTABLE_NAME,
            "bundle": bundle,
        },
        "source": {
            "commit": commit,
            "worktree_dirty": dirty,
            "source_date_epoch": epoch,
        },
        "build": {
            "command": "cargo build --release --locked -p pfp-app --target <target>",
            "targets": o.targets,
            "rustflags": recorded_rustflags(),
            "env": {
                "MACOSX_DEPLOYMENT_TARGET": DEPLOYMENT_TARGET,
                "SOURCE_DATE_EPOCH": epoch.to_string(),
                "ZERO_AR_DATE": "1",
                "RUSTC_WRAPPER": "",
                "RUSTC_WORKSPACE_WRAPPER": "",
            },
            "rustc_wrapper": "none: both wrapper variables are set empty, which also overrides any Cargo configuration",
            "cargo_auditable": {
                "used": false,
                "why": "cargo auditable sets RUSTC_WORKSPACE_WRAPPER to its own absolute path, which Cargo hashes into every workspace crate's -C metadata: the executable would depend on where the tool is installed (per user), not only on its version. The CycloneDX SBOM is the dependency record",
            },
            "profile": "release (Cargo.toml: codegen-units = 1, lto = \"fat\", strip = \"none\", panic = \"abort\")",
        },
        "toolchain": {
            "rustc": tool("rustc", &["-vV"]).lines().collect::<Vec<_>>(),
            "cargo": tool("cargo", &["-V"]),
            "node": tool("node", &["--version"]),
            "npm": tool("npm", &["--version"]),
            "macos_sdk_in_binary": sdk,
        },
        "web_dist": web_listing,
        "executable": {
            "pre_codesign_sha256": Value::Object(pre),
            "pre_codesign_note": "the linker's output before codesign; on arm64 it already carries the linker's ad-hoc 'linker-signed' signature",
            "reproducibility_digest": unsigned_sha,
            "signed_sha256": signed_sha,
            "minimum_macos": DEPLOYMENT_TARGET,
            "lc_uuid": Value::Object(uuids),
            "embedded_dependency_list": format!("none (asserted: no {},{} section)", AUDITABLE_SECTION.0, AUDITABLE_SECTION.1),
            "smoke_test": smoke,
        },
        "signing": {
            "kind": "ad-hoc (no identity, no keychain, no timestamp)",
            "command": format!("codesign {}", sign_args.join(" ")),
            "entitlements": "none (packaging/entitlements.plist is the empty set and is not passed)",
            "entitlements_file_sha256": repo::sha256_hex(entitlements.as_bytes()),
            "signed_once": true,
            "verify_bare_executable": "valid",
            "verify_app_bundle": app_verify,
            "not_done_here": [
                "Developer ID signing", "notarization", "stapling", "spctl assessment",
                "Ed25519 signature over SHA256SUMS (cargo xtask sign-checksums fails closed)"
            ],
        },
        "channels": {
            "bare": format!("{bare_name}.tar.gz: {exe_in_bare}, LICENSE, NOTICE"),
            "app": format!("{app_name}.tar.gz: {app_name}/{bundle} (Contents/Info.plist, Contents/MacOS/{}), LICENSE, NOTICE; the same tree is the app/ directory", identity::EXECUTABLE_NAME),
            "dmg": "built only in CI (hdiutil) from the app/ directory plus an /Applications link; never on a developer machine",
            "identical_executable_in_every_channel": true,
        },
        "check_magic": "clean: work/, app/, bare/ and both archives",
    });
    let mut text = serde_json::to_string_pretty(&manifest).map_err(|e| e.to_string())?;
    text.push('\n');
    let manifest_path = artifacts.join("dist-manifest.json");
    fs::write(&manifest_path, &text).map_err(|e| format!("{}: {e}", manifest_path.display()))?;
    let sums = checksums::write(&artifacts)?;

    let mut stdout = std::io::stdout().lock();
    // Machine-readable first line: the directory comes last because it may contain spaces.
    let _ = writeln!(
        stdout,
        "DIST label={label} name={bare_name} dir={}",
        dir.display()
    );
    let _ = writeln!(
        stdout,
        "pre-codesign  {unsigned_sha}  ({})",
        unsigned.file_name().unwrap_or_default().to_string_lossy()
    );
    let _ = writeln!(
        stdout,
        "signed        {signed_sha}  (work/{})",
        identity::EXECUTABLE_NAME
    );
    let _ = write!(stdout, "{sums}");
    let _ = writeln!(
        stdout,
        ".app bundle codesign --verify --strict: {app_verify}"
    );
    eprintln!("dist: done in {:.0?}", started.elapsed());
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_say_single_arch_or_universal_and_nothing_else() {
        let t = |v: &[&str]| label(&v.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>());
        assert_eq!(
            t(&["aarch64-apple-darwin"]).unwrap(),
            "macos-arm64-single-arch-adhoc"
        );
        assert_eq!(
            t(&["x86_64-apple-darwin"]).unwrap(),
            "macos-x86_64-single-arch-adhoc"
        );
        assert_eq!(
            t(&["aarch64-apple-darwin", "x86_64-apple-darwin"]).unwrap(),
            "macos-universal-adhoc"
        );
        assert!(t(&["x86_64-unknown-linux-gnu"]).is_err());
        assert!(t(&[]).is_err());
        assert!(!t(&["aarch64-apple-darwin"]).unwrap().contains("universal"));
    }

    #[test]
    fn options_parse() {
        let o = parse(&[
            "--universal".into(),
            "--source-date-epoch".into(),
            "1700000000".into(),
        ])
        .unwrap();
        assert_eq!(o.targets, MAC_TARGETS);
        assert_eq!(o.epoch, Some(1_700_000_000));
        assert!(parse(&["--source-date-epoch".into(), "yesterday".into()]).is_err());
        assert!(parse(&["--bogus".into()]).is_err());
        // cargo-auditable is gone, and with it the switch that turned it off.
        assert!(parse(&["--no-auditable".into()]).is_err());
    }

    /// Regression (reproducibility review H1): a rustc wrapper's absolute path
    /// is hashed into every workspace crate's `-C metadata`, so the release
    /// build runs none — not `cargo auditable`, not one from the caller's
    /// environment, not one from a Cargo configuration file.
    #[test]
    fn the_release_build_runs_no_rustc_wrapper() {
        let cmd = release_build_command(Path::new("/w s"), "aarch64-apple-darwin", "F", 7);
        assert_eq!(cmd.get_program(), "cargo");
        let args: Vec<_> = cmd.get_args().map(|a| a.to_string_lossy()).collect();
        assert_eq!(
            args,
            [
                "build",
                "--release",
                "--locked",
                "-p",
                "pfp-app",
                "--target",
                "aarch64-apple-darwin"
            ]
        );
        let envs: std::collections::BTreeMap<_, _> = cmd
            .get_envs()
            .map(|(k, v)| {
                (
                    k.to_string_lossy().into_owned(),
                    v.map(|v| v.to_string_lossy().into_owned()),
                )
            })
            .collect();
        // Set to the empty string (Some("")), not removed (None): only an
        // empty value also overrides `build.rustc-workspace-wrapper` in config.
        for var in ["RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER"] {
            assert_eq!(envs.get(var), Some(&Some(String::new())), "{var}");
        }
        assert_eq!(envs.get("RUSTFLAGS"), Some(&None));
        assert_eq!(
            envs.get("CARGO_ENCODED_RUSTFLAGS"),
            Some(&Some("F".to_owned()))
        );
        assert_eq!(envs.get("SOURCE_DATE_EPOCH"), Some(&Some("7".to_owned())));
        assert_eq!(
            envs.get("MACOSX_DEPLOYMENT_TARGET"),
            Some(&Some(DEPLOYMENT_TARGET.to_owned()))
        );
        assert_eq!(cmd.get_current_dir(), Some(Path::new("/w s")));
    }

    #[test]
    fn an_auditable_section_is_recognised() {
        let s = |seg: &str, sect: &str| (seg.to_owned(), sect.to_owned());
        assert!(!has_auditable_section(&[
            s("__TEXT", "__text"),
            s("__DATA", "__data")
        ]));
        assert!(has_auditable_section(&[
            s("__TEXT", "__text"),
            s("__DATA", ".dep-v0")
        ]));
        assert!(!has_auditable_section(&[s("__TEXT", ".dep-v0")]));
    }

    /// Regression (reproducibility review M1): the directory `dist` wrote is
    /// what the rebuild job compares, so the release workflow must not add the
    /// DMG, the SBOMs or a second SHA256SUMS to it. Those go to a sibling
    /// release directory.
    #[test]
    fn the_release_workflow_leaves_the_dist_output_pristine() {
        let text = fs::read_to_string(repo::root().join(".github/workflows/release.yml")).unwrap();
        // Logical lines: shell continuations joined.
        let joined = text.replace("\\\n", " ");
        let writers = [
            "hdiutil create",
            "xtask sbom",
            "xtask checksums",
            "> \"$DIST_DIR",
            ">> \"$DIST_DIR",
        ];
        let mut seen = 0;
        for line in joined.lines() {
            if writers.iter().any(|w| line.contains(w)) {
                seen += 1;
                assert!(
                    !line.contains("DIST_DIR"),
                    "writes into the dist output: {line}"
                );
                assert!(
                    !line.contains("REBUILD_DIR"),
                    "writes into the dist output: {line}"
                );
            }
        }
        assert!(seen >= 3, "the DMG, SBOM and checksum steps were not found");
        assert!(joined.contains("name: dist-${{ steps.dist.outputs.label }}"));
        assert!(joined.contains("name: release-${{ steps.dist.outputs.label }}"));
    }

    #[test]
    fn the_info_plist_is_rendered_from_the_identity_constants() {
        let root = repo::root();
        let template = fs::read_to_string(root.join(INFO_PLIST_TEMPLATE)).unwrap();
        let text = render_info_plist(&template, "9.9.9").unwrap();
        assert!(
            !text.contains("<!--"),
            "the template's comments are not shipped"
        );
        let body = text.as_str();
        for (key, value) in [
            ("CFBundleIdentifier", identity::APP_IDENTIFIER),
            ("CFBundleExecutable", identity::EXECUTABLE_NAME),
            ("CFBundleName", identity::BUNDLE_NAME),
            ("CFBundleShortVersionString", "9.9.9"),
            ("LSMinimumSystemVersion", DEPLOYMENT_TARGET),
        ] {
            let needle = format!("<key>{key}</key>\n\t<string>{value}</string>");
            assert!(body.contains(&needle), "{key}: {body}");
        }
        assert!(body.contains("<key>LSUIElement</key>\n\t<true/>"), "{body}");
        assert!(!body.contains('@'));
        // `plutil` agrees that it is a property list.
        let dir = repo::temp_dir("plist").unwrap();
        fs::write(dir.join("Info.plist"), &text).unwrap();
        let lint = Command::new("plutil")
            .arg("-lint")
            .arg(dir.join("Info.plist"))
            .output()
            .unwrap();
        fs::remove_dir_all(&dir).unwrap();
        assert!(
            lint.status.success(),
            "{}",
            String::from_utf8_lossy(&lint.stdout)
        );
    }

    #[test]
    fn an_unsubstituted_placeholder_is_refused() {
        let t = "<!-- @IDENTIFIER@ documented -->\n<plist><string>@UNKNOWN@</string></plist>";
        assert!(render_info_plist(t, "1")
            .unwrap_err()
            .ends_with("@UNKNOWN@"));
        assert!(render_info_plist("<!-- open", "1").is_err());
    }

    #[test]
    fn the_entitlements_set_is_empty() {
        let text = fs::read_to_string(repo::root().join(ENTITLEMENTS)).unwrap();
        let body = text.split("-->").last().unwrap();
        assert!(body.contains("<dict/>"), "{body}");
        assert!(!body.contains("<key>"), "an entitlement was added: {body}");
    }

    #[test]
    #[allow(clippy::case_sensitive_file_extension_comparisons)]
    fn the_identifier_is_a_marked_placeholder_in_one_place() {
        assert!(identity::is_reverse_dns(identity::APP_IDENTIFIER));
        let source =
            fs::read_to_string(repo::root().join("crates/pfp-app/src/identity.rs")).unwrap();
        assert!(source.contains("PLACEHOLDER"));
        // No other tracked source spells the identifier out.
        for path in repo::files(&repo::root()).unwrap() {
            let rel = repo::relative(&repo::root(), &path);
            if rel == "crates/pfp-app/src/identity.rs"
                || !(rel.ends_with(".rs") || rel.ends_with(".plist") || rel.ends_with(".yml"))
            {
                continue;
            }
            let text = fs::read_to_string(&path).unwrap_or_default();
            assert!(
                !text.contains(identity::APP_IDENTIFIER),
                "{rel} spells the identifier out"
            );
        }
    }

    #[test]
    fn rustflags_are_encoded_with_the_most_specific_remap_last() {
        let flags = encoded_rustflags(
            Path::new("/no such/work space"),
            Path::new("/no such/cargo"),
            Path::new("/no such/work space/target"),
        );
        let parts: Vec<&str> = flags.split('\u{1f}').collect();
        assert_eq!(
            parts,
            [
                "--remap-path-prefix=/no such/work space=/pfp",
                "--remap-path-prefix=/no such/cargo=/cargo",
                "--remap-path-prefix=/no such/work space/target=/target",
                "-Clink-arg=-Wl,-S"
            ]
        );
        assert_eq!(recorded_rustflags().len(), parts.len());
    }

    #[test]
    fn normalise_tree_sets_modes_and_times() {
        let dir = repo::temp_dir("normalise").unwrap();
        fs::create_dir_all(dir.join("a/b")).unwrap();
        fs::write(dir.join("a/b/f"), b"x").unwrap();
        fs::set_permissions(dir.join("a/b/f"), fs::Permissions::from_mode(0o600)).unwrap();
        normalise_tree(&dir, 1_000_000).unwrap();
        let meta = fs::metadata(dir.join("a/b/f")).unwrap();
        let dmeta = fs::metadata(dir.join("a")).unwrap();
        fs::remove_dir_all(&dir).unwrap();
        let when = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000);
        assert_eq!(meta.permissions().mode() & 0o777, 0o644);
        assert_eq!(meta.modified().unwrap(), when);
        assert_eq!(dmeta.modified().unwrap(), when);
    }
}
