// The front-end input hash: one function shared, by `#[path]` inclusion, between
// `pfp-server/build.rs` (which checks it) and `cargo xtask build-web` (which
// writes it). Build-time only: nothing here is compiled into the server library.
//
// The hash covers every file that decides what `vite build` emits: the entry
// document, the package manifest and lockfile, the compiler and bundler
// configuration and everything under `web/src`. It is SHA-256 over the sorted
// list of `<relative path> NUL <sha256 of the file> LF`, so it does not depend on
// directory-walk order, modification times or the absolute location of the
// checkout.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

/// The single files under `web/` that are build inputs, besides `src/**`.
pub(crate) const INPUT_FILES: [&str; 5] = [
    "index.html",
    "package.json",
    "package-lock.json",
    "tsconfig.json",
    "vite.config.ts",
];

/// The directory under `web/` whose whole content is a build input.
pub(crate) const INPUT_DIR: &str = "src";

/// Where `cargo xtask build-web` records the input hash, relative to `web/`. It
/// sits inside `dist` so the repository's existing ignore rule covers it; the
/// asset loader skips it (it is not `index.html` and not under `assets/`).
pub(crate) const STAMP_PATH: &str = "dist/.dist-stamp";

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes.iter().fold(String::new(), |mut out, byte| {
        let _ = write!(out, "{byte:02x}");
        out
    })
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            walk(&path, out)?;
        } else {
            out.push(path);
        }
    }
    Ok(())
}

/// Every input file, as paths relative to `web_dir` with `/` separators, sorted.
///
/// # Errors
/// Any I/O error while walking `web/src`.
pub(crate) fn input_paths(web_dir: &Path) -> io::Result<Vec<String>> {
    let mut files: Vec<PathBuf> = INPUT_FILES
        .iter()
        .map(|name| web_dir.join(name))
        .filter(|path| path.is_file())
        .collect();
    let src = web_dir.join(INPUT_DIR);
    if src.is_dir() {
        walk(&src, &mut files)?;
    }
    let mut relative: Vec<String> = files
        .iter()
        .filter_map(|path| path.strip_prefix(web_dir).ok())
        .map(|path| {
            path.components()
                .map(|part| part.as_os_str().to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join("/")
        })
        .collect();
    relative.sort();
    Ok(relative)
}

/// The lower-case hex input hash of the front end rooted at `web_dir`.
///
/// # Errors
/// Any I/O error while reading an input.
pub(crate) fn input_hash(web_dir: &Path) -> io::Result<String> {
    let mut outer = Sha256::new();
    for relative in input_paths(web_dir)? {
        let bytes = fs::read(web_dir.join(&relative))?;
        outer.update(relative.as_bytes());
        outer.update([0]);
        outer.update(hex(&Sha256::digest(&bytes)).as_bytes());
        outer.update(b"\n");
    }
    Ok(hex(&outer.finalize()))
}
