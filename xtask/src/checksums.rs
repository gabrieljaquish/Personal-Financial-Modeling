//! `cargo xtask checksums <DIR>`: `SHA256SUMS` over every release artifact, and
//! the Ed25519 signing step as a stub that **fails closed** (`ARCHITECTURE.md`
//! §9 item 7; `SECURITY.md` §11 "Release-key publication and rotation").
//!
//! The file is written in the format `shasum -a 256 -c` reads: one line per
//! regular file under `DIR` (recursively, symbolic links skipped), `<hex>  <path>`
//! with the path relative to `DIR`, sorted byte-wise. It is **unsigned**.
//!
//! `cargo xtask sign-checksums` exists so that the pipeline has a named place for
//! the signature, and so that anything which calls it before the signing block
//! lands stops instead of shipping an unsigned file under a signed name. It
//! generates, reads and stores no key of any kind; it only refuses.

use std::fs;
use std::path::Path;

use crate::repo;

pub(crate) const SUMS_FILE: &str = "SHA256SUMS";

/// The text of `SHA256SUMS` for `dir` (the file itself, and any signature
/// beside it, excluded).
pub(crate) fn render(dir: &Path) -> Result<String, String> {
    let mut lines = Vec::new();
    for path in repo::walk(dir)? {
        let rel = repo::relative(dir, &path);
        if rel == SUMS_FILE || rel.starts_with("SHA256SUMS.") {
            continue;
        }
        if rel.contains('\n') {
            return Err(format!(
                "{rel:?}: a file name with a newline cannot be listed"
            ));
        }
        lines.push(format!(
            "{}  {rel}\n",
            repo::sha256_hex(&repo::read(&path)?)
        ));
    }
    lines.sort_by(|a, b| a.as_bytes()[66..].cmp(&b.as_bytes()[66..]));
    Ok(lines.concat())
}

/// Writes `dir/SHA256SUMS` and returns its text.
pub(crate) fn write(dir: &Path) -> Result<String, String> {
    let text = render(dir)?;
    if text.is_empty() {
        return Err(format!("{}: no artifacts to list", dir.display()));
    }
    let path = dir.join(SUMS_FILE);
    fs::write(&path, &text).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(text)
}

pub(crate) fn run(args: &[String]) -> Result<bool, String> {
    let [dir] = args else {
        return Err("usage: cargo xtask checksums <DIR>".to_owned());
    };
    let text = write(Path::new(dir))?;
    print!("{text}");
    eprintln!(
        "checksums: wrote {}/{SUMS_FILE} ({} file(s)); UNSIGNED",
        dir,
        text.lines().count()
    );
    Ok(true)
}

/// The Ed25519 signature over `SHA256SUMS`: out of scope until the signing
/// block, so it refuses — always, whatever it is given.
pub(crate) fn sign(_args: &[String]) -> Result<bool, String> {
    Err(
        "sign-checksums: the Ed25519 signature over SHA256SUMS is NOT implemented. \
         It belongs to the M0 signing block (release-key custody is open decision 2; \
         SECURITY.md §11). No key exists in this repository or its pipeline, none is \
         generated here, and this step fails closed so that nothing ships an unsigned \
         SHA256SUMS as if it were signed."
            .to_owned(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_every_file_sorted_and_excludes_itself() {
        let dir = repo::temp_dir("sums").unwrap();
        fs::create_dir_all(dir.join("sub")).unwrap();
        fs::write(dir.join("b.tar.gz"), b"b").unwrap();
        fs::write(dir.join("a.dmg"), b"a").unwrap();
        fs::write(dir.join("sub").join("c.json"), b"c").unwrap();
        fs::write(dir.join(SUMS_FILE), b"stale").unwrap();
        let text = write(&dir).unwrap();
        let again = render(&dir).unwrap();
        // `shasum` agrees with every line.
        let check = std::process::Command::new("shasum")
            .args(["-a", "256", "-c", SUMS_FILE])
            .current_dir(&dir)
            .output()
            .unwrap();
        fs::remove_dir_all(&dir).unwrap();
        assert_eq!(text, again);
        let names: Vec<&str> = text.lines().map(|l| &l[66..]).collect();
        assert_eq!(names, ["a.dmg", "b.tar.gz", "sub/c.json"]);
        assert!(text.starts_with(&format!("{}  a.dmg\n", repo::sha256_hex(b"a"))));
        assert!(
            check.status.success(),
            "{}",
            String::from_utf8_lossy(&check.stdout)
        );
    }

    #[test]
    fn an_empty_directory_is_an_error() {
        let dir = repo::temp_dir("sums-empty").unwrap();
        let result = write(&dir);
        fs::remove_dir_all(&dir).unwrap();
        assert!(result.is_err());
    }

    #[test]
    fn signing_fails_closed_whatever_it_is_given() {
        assert!(sign(&[]).is_err());
        assert!(sign(&["SHA256SUMS".to_owned(), "--key".to_owned()]).is_err());
    }
}
