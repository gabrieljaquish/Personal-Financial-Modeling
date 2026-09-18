//! `check-magic`: the container-magic gate (`SECURITY.md` §13.2, §13.4; ADR-023).
//!
//! A `.pfplan` container begins with a seven-byte magic. If that byte sequence
//! appears **anywhere** in any file of the working tree, or anywhere inside a
//! release archive, the gate fails. It is the one mechanical control that
//! catches a real household's plan file committed by accident (threat T5), and
//! it takes **no allowlist, ever**: not for a demo, a fixture, a test or a path.
//! The demo plan therefore ships as plaintext JSON, and the reference
//! decryptor's containers are generated at test time outside the tree.
//!
//! Two scopes. The default scan walks the working tree itself, ignored files
//! included, skipping only build output (`repo::tree_files`): it never defers
//! to `.gitignore`, which lists `*.pfplan` and would hide the very file this
//! gate exists to find. `--history` byte-scans every blob in the git object
//! database, which `gitleaks git` cannot do for this rule because git diffs a
//! file containing a NUL as "Binary files differ".
//!
//! The sequence is spelled here as hex so that this source file, and the
//! compiled `xtask` binary's own source, never contain it as text.

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::repo;

/// The container magic, `PFPLAN` followed by a NUL byte, written as hex.
const MAGIC: [u8; 7] = [0x50, 0x46, 0x50, 0x4c, 0x41, 0x4e, 0x00];

/// Returns the offset of the first occurrence of the magic in `bytes`.
pub(crate) fn find_magic(bytes: &[u8]) -> Option<usize> {
    bytes.windows(MAGIC.len()).position(|w| w == MAGIC)
}

pub(crate) fn run(args: &[String]) -> Result<bool, String> {
    let root = repo::root();
    let mut hits = Vec::new();
    if args.is_empty() {
        // The working tree itself, NOT git's view of it: `.gitignore` lists
        // `*.pfplan`, so a git-aware listing would skip the file this gate
        // exists to find. Only build output is skipped (`repo::tree_files`).
        scan_files(&root, &repo::tree_files(&root)?, &mut hits)?;
    } else {
        for arg in args {
            if arg == "--history" {
                scan_history(&root, &mut hits)?;
            } else {
                scan_target(Path::new(arg), &mut hits)?;
            }
        }
    }
    for hit in &hits {
        eprintln!("check-magic: {hit}");
    }
    if hits.is_empty() {
        eprintln!("check-magic: clean");
        Ok(true)
    } else {
        eprintln!(
            "check-magic: {} file(s) contain the .pfplan container magic; this gate has no allowlist",
            hits.len()
        );
        Ok(false)
    }
}

/// Scans a file, a directory tree, or an archive (unpacked into a temporary
/// directory first, because a compressed archive hides the bytes it contains).
fn scan_target(target: &Path, hits: &mut Vec<String>) -> Result<(), String> {
    if target.is_dir() {
        return scan_files(target, &repo::walk(target)?, hits);
    }
    if !target.is_file() {
        return Err(format!("{}: no such file or directory", target.display()));
    }
    let Some(kind) = archive_kind(target) else {
        let base = target.parent().unwrap_or(Path::new(""));
        return scan_files(base, std::slice::from_ref(&target.to_path_buf()), hits);
    };
    let tmp = repo::temp_dir("archive")?;
    let result = unpack(kind, target, &tmp).and_then(|()| {
        let mut inner = Vec::new();
        scan_files(&tmp, &repo::walk(&tmp)?, &mut inner)?;
        hits.extend(
            inner
                .into_iter()
                .map(|h| format!("{}: {h}", target.display())),
        );
        Ok(())
    });
    let _ = std::fs::remove_dir_all(&tmp);
    result
}

/// Scans every blob in the git object database of the repository at `root`:
/// every commit on every ref, plus objects no ref reaches any more.
///
/// This exists because `gitleaks git` cannot do it. The magic ends in a NUL, so
/// git treats any file containing it as binary and renders its diff as "Binary
/// files differ"; gitleaks scans diffs, so its magic rule never sees the bytes
/// in history. A container committed once and deleted later is found only here.
fn scan_history(root: &Path, hits: &mut Vec<String>) -> Result<(), String> {
    // Object id -> the first path it was reachable under, for the report only.
    let listing = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["rev-list", "--objects", "--all"])
        .output()
        .map_err(|e| format!("git rev-list: {e}"))?;
    if !listing.status.success() {
        return Err(format!(
            "{}: `git rev-list` failed; --history needs a git repository",
            root.display()
        ));
    }
    let mut names: BTreeMap<String, String> = BTreeMap::new();
    for line in String::from_utf8_lossy(&listing.stdout).lines() {
        if let Some((id, path)) = line.split_once(' ') {
            names
                .entry(id.to_owned())
                .or_insert_with(|| path.to_owned());
        }
    }

    let mut child = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["cat-file", "--batch", "--batch-all-objects"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|e| format!("git cat-file: {e}"))?;
    let stdout = child.stdout.take().expect("stdout was requested as piped");
    let mut reader = BufReader::new(stdout);
    let mut header = Vec::new();
    let mut scanned = 0_u64;
    loop {
        header.clear();
        let n = reader
            .read_until(b'\n', &mut header)
            .map_err(|e| format!("git cat-file: {e}"))?;
        if n == 0 {
            break;
        }
        // "<object id> <type> <size>\n", then <size> bytes, then "\n".
        let text = String::from_utf8_lossy(&header);
        let mut fields = text.split_whitespace();
        let (Some(id), Some(kind), Some(size)) = (fields.next(), fields.next(), fields.next())
        else {
            return Err(format!("git cat-file: unexpected header `{}`", text.trim()));
        };
        let size: usize = size
            .parse()
            .map_err(|_| format!("git cat-file: unexpected size in `{}`", text.trim()))?;
        let mut body = vec![0_u8; size + 1];
        reader
            .read_exact(&mut body)
            .map_err(|e| format!("git cat-file: object {id}: {e}"))?;
        body.truncate(size);
        if kind != "blob" {
            continue;
        }
        scanned += 1;
        if let Some(offset) = find_magic(&body) {
            let name = names
                .get(id)
                .map_or("not reachable from any ref", String::as_str);
            hits.push(format!(
                "history: blob {id} ({name}): contains the container magic (byte offset {offset})"
            ));
        }
    }
    let status = child.wait().map_err(|e| format!("git cat-file: {e}"))?;
    if !status.success() {
        return Err(format!("git cat-file failed ({status})"));
    }
    eprintln!("check-magic: history: {scanned} blob(s) scanned");
    Ok(())
}

fn scan_files(base: &Path, files: &[PathBuf], hits: &mut Vec<String>) -> Result<(), String> {
    for path in files {
        let bytes = repo::read(path)?;
        if let Some(offset) = find_magic(&bytes) {
            let rel = repo::relative(base, path);
            let how = if offset == 0 {
                "begins with"
            } else {
                "contains"
            };
            hits.push(format!(
                "{rel}: {how} the container magic (byte offset {offset})"
            ));
        }
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum ArchiveKind {
    Tar,
    Zip,
}

// The name is lowercased first, so the suffix comparisons are case-insensitive.
#[allow(clippy::case_sensitive_file_extension_comparisons)]
fn archive_kind(path: &Path) -> Option<ArchiveKind> {
    let name = path.file_name()?.to_str()?.to_ascii_lowercase();
    if name.ends_with(".zip") {
        Some(ArchiveKind::Zip)
    } else if [".tar", ".tgz", ".tar.gz", ".tar.xz", ".tar.zst", ".tar.bz2"]
        .iter()
        .any(|ext| name.ends_with(ext))
    {
        Some(ArchiveKind::Tar)
    } else {
        None
    }
}

fn unpack(kind: ArchiveKind, archive: &Path, into: &Path) -> Result<(), String> {
    let status = match kind {
        ArchiveKind::Tar => Command::new("tar")
            .arg("-xf")
            .arg(archive)
            .arg("-C")
            .arg(into)
            .status(),
        ArchiveKind::Zip => Command::new("unzip")
            .arg("-q")
            .arg(archive)
            .arg("-d")
            .arg(into)
            .status(),
    }
    .map_err(|e| format!("{}: cannot unpack: {e}", archive.display()))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{}: unpack failed ({status})", archive.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn magic() -> Vec<u8> {
        MAGIC.to_vec()
    }

    #[test]
    fn magic_is_pfplan_then_nul() {
        // The sequence is checked against its bytes, never spelled as text.
        assert_eq!(&MAGIC[..6], b"PFPLAN");
        assert_eq!(MAGIC[6], 0);
    }

    #[test]
    fn finds_magic_at_start_and_inside() {
        assert_eq!(find_magic(&magic()), Some(0));
        let mut inside = b"abc".to_vec();
        inside.extend(magic());
        assert_eq!(find_magic(&inside), Some(3));
    }

    #[test]
    fn text_mention_without_nul_is_not_the_magic() {
        // The design documents write the magic as text with an escaped NUL; that
        // is not the byte sequence and must not trip the gate.
        assert_eq!(find_magic(b"magic PFPLAN\\0 in prose"), None);
        assert_eq!(find_magic(b"PFPLAN"), None);
        assert_eq!(find_magic(b""), None);
    }

    #[test]
    fn scans_directory_and_reports_offsets() {
        let dir = repo::temp_dir("magic-dir").unwrap();
        fs::write(dir.join("clean.txt"), b"nothing here").unwrap();
        fs::write(dir.join("head.bin"), magic()).unwrap();
        let mut tail = b"prefix-".to_vec();
        tail.extend(magic());
        fs::create_dir_all(dir.join("nested")).unwrap();
        fs::write(dir.join("nested").join("tail.bin"), tail).unwrap();

        let mut hits = Vec::new();
        scan_target(&dir, &mut hits).unwrap();
        fs::remove_dir_all(&dir).unwrap();

        hits.sort();
        assert_eq!(hits.len(), 2);
        assert!(hits[0].starts_with("head.bin: begins with"), "{}", hits[0]);
        assert!(
            hits[1].starts_with("nested/tail.bin: contains"),
            "{}",
            hits[1]
        );
        assert!(hits[1].ends_with("(byte offset 7)"), "{}", hits[1]);
    }

    #[test]
    fn scans_inside_a_tar_archive() {
        let dir = repo::temp_dir("magic-tar").unwrap();
        let payload = dir.join("payload");
        fs::create_dir_all(&payload).unwrap();
        fs::write(payload.join("ok.json"), b"{\"synthetic\": true}").unwrap();
        fs::write(payload.join("household.pfplan"), magic()).unwrap();
        let archive = dir.join("release.tar");
        let status = Command::new("tar")
            .arg("-cf")
            .arg(&archive)
            .arg("-C")
            .arg(&dir)
            .arg("payload")
            .status()
            .unwrap();
        assert!(status.success());

        let mut hits = Vec::new();
        scan_target(&archive, &mut hits).unwrap();
        fs::remove_dir_all(&dir).unwrap();

        assert_eq!(hits.len(), 1, "{hits:?}");
        assert!(
            hits[0].contains("payload/household.pfplan: begins with"),
            "{}",
            hits[0]
        );
    }

    #[test]
    fn the_repository_tree_is_clean() {
        let root = repo::root();
        let mut hits = Vec::new();
        scan_files(&root, &repo::tree_files(&root).unwrap(), &mut hits).unwrap();
        assert!(hits.is_empty(), "{hits:?}");
    }

    fn git(dir: &Path, args: &[&str]) {
        // A throwaway repository: synthetic identity, no signing, no hooks, no
        // network. Global and system configuration are not read.
        let status = Command::new("git")
            .arg("-C")
            .arg(dir)
            .args([
                "-c",
                "user.name=Synthetic Test",
                "-c",
                "user.email=synthetic@example.invalid",
                "-c",
                "commit.gpgsign=false",
                "-c",
                "core.hooksPath=/dev/null",
            ])
            .args(args)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .unwrap();
        assert!(status.success(), "git {args:?}");
    }

    #[test]
    fn history_scan_finds_a_container_that_was_committed_and_deleted() {
        let dir = repo::temp_dir("magic-history").unwrap();
        git(&dir, &["init", "-q"]);
        fs::write(dir.join("readme.txt"), b"synthetic").unwrap();
        let mut body = b"prefix".to_vec();
        body.extend(magic());
        fs::write(dir.join("household.pfplan"), body).unwrap();
        git(&dir, &["add", "-A"]);
        git(&dir, &["commit", "-q", "-m", "add"]);
        fs::remove_file(dir.join("household.pfplan")).unwrap();
        git(&dir, &["add", "-A"]);
        git(&dir, &["commit", "-q", "-m", "remove"]);

        // The working tree is clean again; only the history still holds it.
        let mut tree = Vec::new();
        scan_files(&dir, &repo::tree_files(&dir).unwrap(), &mut tree).unwrap();
        let mut history = Vec::new();
        scan_history(&dir, &mut history).unwrap();
        fs::remove_dir_all(&dir).unwrap();

        assert!(tree.is_empty(), "{tree:?}");
        assert_eq!(history.len(), 1, "{history:?}");
        assert!(history[0].contains("(household.pfplan)"), "{}", history[0]);
        assert!(history[0].ends_with("(byte offset 6)"), "{}", history[0]);
    }

    #[test]
    fn the_repository_history_is_clean() {
        let root = repo::root();
        if !root.join(".git").exists() {
            // An unpacked source archive has no history to scan.
            return;
        }
        let mut hits = Vec::new();
        scan_history(&root, &mut hits).unwrap();
        assert!(hits.is_empty(), "{hits:?}");
    }
}
