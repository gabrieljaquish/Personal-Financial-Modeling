//! Deterministic archives for the release channels (`ARCHITECTURE.md` §9 items
//! 4-5): a ustar writer and reader, and `gzip -n`.
//!
//! The tar stream is written here rather than by `tar(1)` because every property
//! that makes an archive reproducible is then a line of this file instead of a
//! flag whose meaning differs between bsdtar and GNU tar:
//!
//! * entries in byte-wise sorted path order, directories included explicitly;
//! * `uid = gid = 0`, empty user and group names;
//! * `mtime` = `SOURCE_DATE_EPOCH` for every entry;
//! * modes normalised to `0755` (directories and executables) or `0644`;
//! * plain ustar: no pax header, no extended attribute, no `AppleDouble` `._`
//!   file, no ACL, no file flag — macOS `bsdtar` adds the last three by default;
//! * symbolic links, devices and anything that is not a file or a directory are
//!   refused rather than silently archived.
//!
//! Compression is `gzip -n -9`: `-n` writes neither the file name nor a
//! timestamp into the gzip header, so the compressed bytes are a function of the
//! tar bytes (for one `gzip` implementation; the build platform is pinned).

use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Command, Stdio};

const BLOCK: usize = 512;
/// The record size `tar(1)` pads to (blocking factor 20).
const RECORD: usize = 20 * BLOCK;

/// One archive member.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Entry {
    /// Relative path with `/` separators; a directory has no trailing slash here.
    pub(crate) path: String,
    pub(crate) kind: EntryKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum EntryKind {
    Dir,
    File { bytes: Vec<u8>, executable: bool },
}

/// A member read back from a tar stream, header fields included, for the
/// reproducibility report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Member {
    pub(crate) path: String,
    pub(crate) typeflag: u8,
    pub(crate) mode: u64,
    pub(crate) uid: u64,
    pub(crate) gid: u64,
    pub(crate) mtime: u64,
    pub(crate) uname: String,
    pub(crate) gname: String,
    pub(crate) bytes: Vec<u8>,
}

/// Every file and directory under `root`, as entries whose paths start with
/// `prefix` (the archive's top-level name). Refuses symbolic links and special
/// files.
pub(crate) fn entries_from_dir(root: &Path, prefix: &str) -> Result<Vec<Entry>, String> {
    let mut out = vec![Entry {
        path: prefix.to_owned(),
        kind: EntryKind::Dir,
    }];
    collect(root, prefix, &mut out)?;
    Ok(out)
}

fn collect(dir: &Path, rel: &str, out: &mut Vec<Entry>) -> Result<(), String> {
    let mut names = Vec::new();
    for entry in fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))? {
        let entry = entry.map_err(|e| format!("{}: {e}", dir.display()))?;
        names.push(entry.file_name().to_string_lossy().into_owned());
    }
    names.sort();
    for name in names {
        let path = dir.join(&name);
        let child = format!("{rel}/{name}");
        let meta = fs::symlink_metadata(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        if meta.file_type().is_symlink() {
            return Err(format!(
                "{}: a symbolic link cannot enter a release archive",
                path.display()
            ));
        }
        if meta.is_dir() {
            out.push(Entry {
                path: child.clone(),
                kind: EntryKind::Dir,
            });
            collect(&path, &child, out)?;
        } else if meta.is_file() {
            let bytes = fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            out.push(Entry {
                path: child,
                kind: EntryKind::File {
                    bytes,
                    executable: meta.permissions().mode() & 0o111 != 0,
                },
            });
        } else {
            return Err(format!(
                "{}: not a regular file or directory",
                path.display()
            ));
        }
    }
    Ok(())
}

fn octal(field: &mut [u8], value: u64) -> Result<(), String> {
    // `len - 1` octal digits, zero-padded, then NUL.
    let digits = field.len() - 1;
    let text = format!("{value:0digits$o}");
    if text.len() > digits {
        return Err(format!(
            "value {value} does not fit a {digits}-digit tar field"
        ));
    }
    field[..digits].copy_from_slice(text.as_bytes());
    field[digits] = 0;
    Ok(())
}

fn header(
    path: &str,
    typeflag: u8,
    mode: u64,
    size: u64,
    mtime: u64,
) -> Result<[u8; BLOCK], String> {
    let mut h = [0_u8; BLOCK];
    let (prefix, name) = split_name(path)?;
    h[..name.len()].copy_from_slice(name.as_bytes());
    octal(&mut h[100..108], mode)?;
    octal(&mut h[108..116], 0)?; // uid
    octal(&mut h[116..124], 0)?; // gid
    octal(&mut h[124..136], size)?;
    octal(&mut h[136..148], mtime)?;
    h[156] = typeflag;
    h[257..263].copy_from_slice(b"ustar\0");
    h[263..265].copy_from_slice(b"00");
    // uname and gname stay empty: numeric ids only.
    octal(&mut h[329..337], 0)?; // devmajor
    octal(&mut h[337..345], 0)?; // devminor
    h[345..345 + prefix.len()].copy_from_slice(prefix.as_bytes());
    // The checksum is computed with its own field read as eight spaces.
    h[148..156].copy_from_slice(b"        ");
    let sum: u64 = h.iter().map(|&b| u64::from(b)).sum();
    let text = format!("{sum:06o}\0 ");
    h[148..156].copy_from_slice(text.as_bytes());
    Ok(h)
}

/// ustar splits a long path into a 155-byte prefix and a 100-byte name at a `/`.
fn split_name(path: &str) -> Result<(&str, &str), String> {
    if !path.is_ascii() {
        return Err(format!("{path}: archive paths are ASCII only"));
    }
    if path.len() <= 100 {
        return Ok(("", path));
    }
    for (i, b) in path.bytes().enumerate().rev() {
        if b == b'/' && i <= 155 && path.len() - i - 1 <= 100 {
            return Ok((&path[..i], &path[i + 1..]));
        }
    }
    Err(format!("{path}: too long for a ustar header"))
}

/// The uncompressed tar stream for `entries` (sorted here; the caller's order
/// does not matter), every member stamped with `mtime`.
pub(crate) fn tar_bytes(entries: &[Entry], mtime: u64) -> Result<Vec<u8>, String> {
    let mut sorted: Vec<&Entry> = entries.iter().collect();
    sorted.sort_by(|a, b| a.path.as_bytes().cmp(b.path.as_bytes()));
    for pair in sorted.windows(2) {
        if pair[0].path == pair[1].path {
            return Err(format!("{}: duplicate archive member", pair[0].path));
        }
    }
    let mut out = Vec::new();
    for entry in sorted {
        if entry.path.is_empty()
            || entry.path.starts_with('/')
            || entry.path.split('/').any(|c| c == ".." || c.is_empty())
        {
            return Err(format!("{:?}: not a clean relative path", entry.path));
        }
        match &entry.kind {
            EntryKind::Dir => {
                out.extend_from_slice(&header(&format!("{}/", entry.path), b'5', 0o755, 0, mtime)?);
            }
            EntryKind::File { bytes, executable } => {
                let mode = if *executable { 0o755 } else { 0o644 };
                out.extend_from_slice(&header(&entry.path, b'0', mode, bytes.len() as u64, mtime)?);
                out.extend_from_slice(bytes);
                let pad = (BLOCK - bytes.len() % BLOCK) % BLOCK;
                out.extend(std::iter::repeat_n(0_u8, pad));
            }
        }
    }
    out.extend(std::iter::repeat_n(0_u8, 2 * BLOCK));
    let pad = (RECORD - out.len() % RECORD) % RECORD;
    out.extend(std::iter::repeat_n(0_u8, pad));
    Ok(out)
}

/// `gzip -n -9` over `bytes`: no file name, no timestamp in the header.
pub(crate) fn gzip(bytes: &[u8]) -> Result<Vec<u8>, String> {
    pipe("gzip", &["-n", "-9", "-c"], bytes)
}

/// `gzip -d -c` over `bytes`.
pub(crate) fn gunzip(bytes: &[u8]) -> Result<Vec<u8>, String> {
    pipe("gzip", &["-d", "-c"], bytes)
}

fn pipe(program: &str, args: &[&str], input: &[u8]) -> Result<Vec<u8>, String> {
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("cannot run {program}: {e}"))?;
    let mut stdin = child.stdin.take().expect("stdin was requested as piped");
    let data = input.to_vec();
    let writer = std::thread::spawn(move || stdin.write_all(&data));
    let output = child
        .wait_with_output()
        .map_err(|e| format!("{program}: {e}"))?;
    writer
        .join()
        .map_err(|_| format!("{program}: the writer thread panicked"))?
        .map_err(|e| format!("{program}: {e}"))?;
    if output.status.success() {
        Ok(output.stdout)
    } else {
        Err(format!(
            "{program} {} failed ({}): {}",
            args.join(" "),
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

/// Writes `<dir contents>` as a deterministic `.tar.gz` at `archive`, every
/// member under the top-level name `prefix`.
pub(crate) fn write_tar_gz(
    dir: &Path,
    prefix: &str,
    mtime: u64,
    archive: &Path,
) -> Result<(), String> {
    let tar = tar_bytes(&entries_from_dir(dir, prefix)?, mtime)?;
    let gz = gzip(&tar)?;
    fs::write(archive, gz).map_err(|e| format!("{}: {e}", archive.display()))
}

fn field_str(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).into_owned()
}

fn field_octal(bytes: &[u8]) -> Result<u64, String> {
    let text = field_str(bytes);
    let text = text.trim_matches(|c: char| c == ' ' || c == '\0');
    if text.is_empty() {
        return Ok(0);
    }
    u64::from_str_radix(text, 8).map_err(|_| format!("bad octal field {text:?}"))
}

/// Reads a tar stream (ustar, as written above; pax and GNU headers are
/// reported as members of their own type rather than interpreted).
pub(crate) fn read_tar(bytes: &[u8]) -> Result<Vec<Member>, String> {
    let mut out = Vec::new();
    let mut at = 0;
    while at + BLOCK <= bytes.len() {
        let h = &bytes[at..at + BLOCK];
        if h.iter().all(|&b| b == 0) {
            break;
        }
        let name = field_str(&h[0..100]);
        let prefix = field_str(&h[345..500]);
        let path = if prefix.is_empty() {
            name
        } else {
            format!("{prefix}/{name}")
        };
        let size = usize::try_from(field_octal(&h[124..136])?).map_err(|e| e.to_string())?;
        let data_start = at + BLOCK;
        let data_end = data_start + size;
        if data_end > bytes.len() {
            return Err(format!("{path}: truncated tar member"));
        }
        out.push(Member {
            path,
            typeflag: h[156],
            mode: field_octal(&h[100..108])?,
            uid: field_octal(&h[108..116])?,
            gid: field_octal(&h[116..124])?,
            mtime: field_octal(&h[136..148])?,
            uname: field_str(&h[265..297]),
            gname: field_str(&h[297..329]),
            bytes: bytes[data_start..data_end].to_vec(),
        });
        at = data_start + size.div_ceil(BLOCK) * BLOCK;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repo;

    fn sample() -> Vec<Entry> {
        vec![
            Entry {
                path: "top/z.txt".into(),
                kind: EntryKind::File {
                    bytes: b"zed".to_vec(),
                    executable: false,
                },
            },
            Entry {
                path: "top".into(),
                kind: EntryKind::Dir,
            },
            Entry {
                path: "top/bin".into(),
                kind: EntryKind::File {
                    bytes: vec![7_u8; 1000],
                    executable: true,
                },
            },
        ]
    }

    #[test]
    fn tar_is_sorted_normalised_and_order_independent() {
        let a = tar_bytes(&sample(), 1_700_000_000).unwrap();
        let mut reversed = sample();
        reversed.reverse();
        let b = tar_bytes(&reversed, 1_700_000_000).unwrap();
        assert_eq!(a, b, "input order must not matter");
        assert_eq!(a.len() % RECORD, 0);

        let members = read_tar(&a).unwrap();
        let paths: Vec<&str> = members.iter().map(|m| m.path.as_str()).collect();
        assert_eq!(paths, ["top/", "top/bin", "top/z.txt"]);
        for m in &members {
            assert_eq!((m.uid, m.gid, m.mtime), (0, 0, 1_700_000_000));
            assert!(m.uname.is_empty() && m.gname.is_empty());
        }
        assert_eq!(members[0].mode, 0o755);
        assert_eq!(members[1].mode, 0o755);
        assert_eq!(members[2].mode, 0o644);
        assert_eq!(members[1].bytes, vec![7_u8; 1000]);
        assert_eq!(members[2].bytes, b"zed");
    }

    #[test]
    fn a_different_mtime_changes_the_stream() {
        assert_ne!(
            tar_bytes(&sample(), 1).unwrap(),
            tar_bytes(&sample(), 2).unwrap()
        );
    }

    #[test]
    fn system_tar_reads_the_stream() {
        let dir = repo::temp_dir("archive-systar").unwrap();
        let tarball = dir.join("x.tar");
        fs::write(&tarball, tar_bytes(&sample(), 1_700_000_000).unwrap()).unwrap();
        let out = Command::new("tar")
            .arg("-tvf")
            .arg(&tarball)
            .output()
            .unwrap();
        fs::remove_dir_all(&dir).unwrap();
        assert!(out.status.success());
        let listing = String::from_utf8_lossy(&out.stdout);
        assert!(listing.contains("top/bin"), "{listing}");
        assert!(listing.contains("top/z.txt"), "{listing}");
    }

    #[test]
    fn gzip_header_carries_no_name_and_no_time() {
        let tar = tar_bytes(&sample(), 5).unwrap();
        let a = gzip(&tar).unwrap();
        let b = gzip(&tar).unwrap();
        assert_eq!(a, b);
        assert_eq!(&a[..2], &[0x1f, 0x8b]);
        // FLG: no FNAME (0x08), no FCOMMENT (0x10).
        assert_eq!(a[3] & 0x18, 0);
        // MTIME, bytes 4..8, is zero.
        assert_eq!(&a[4..8], &[0, 0, 0, 0]);
        assert_eq!(gunzip(&a).unwrap(), tar);
    }

    #[test]
    fn refuses_unclean_paths_duplicates_and_symlinks() {
        let bad = |path: &str| Entry {
            path: path.into(),
            kind: EntryKind::Dir,
        };
        assert!(tar_bytes(&[bad("../x")], 0).is_err());
        assert!(tar_bytes(&[bad("/abs")], 0).is_err());
        assert!(tar_bytes(&[bad("a//b")], 0).is_err());
        assert!(tar_bytes(&[bad("a"), bad("a")], 0).is_err());

        let dir = repo::temp_dir("archive-symlink").unwrap();
        fs::write(dir.join("f"), b"x").unwrap();
        std::os::unix::fs::symlink("/tmp", dir.join("link")).unwrap();
        let result = entries_from_dir(&dir, "top");
        fs::remove_dir_all(&dir).unwrap();
        assert!(result.unwrap_err().contains("symbolic link"));
    }

    #[test]
    fn long_paths_use_the_prefix_field() {
        let long = format!("{}/{}", "d".repeat(120), "f".repeat(90));
        let entries = [Entry {
            path: long.clone(),
            kind: EntryKind::File {
                bytes: b"x".to_vec(),
                executable: false,
            },
        }];
        let members = read_tar(&tar_bytes(&entries, 0).unwrap()).unwrap();
        assert_eq!(members[0].path, long);
        assert!(split_name(&"x".repeat(300)).is_err());
    }

    #[test]
    fn entries_from_dir_reads_modes() {
        let dir = repo::temp_dir("archive-dir").unwrap();
        fs::create_dir_all(dir.join("sub")).unwrap();
        fs::write(dir.join("sub/run"), b"#!").unwrap();
        fs::set_permissions(dir.join("sub/run"), fs::Permissions::from_mode(0o700)).unwrap();
        fs::write(dir.join("a.txt"), b"a").unwrap();
        let entries = entries_from_dir(&dir, "p").unwrap();
        fs::remove_dir_all(&dir).unwrap();
        let summary: Vec<(String, bool)> = entries
            .iter()
            .map(|e| {
                (
                    e.path.clone(),
                    matches!(
                        e.kind,
                        EntryKind::File {
                            executable: true,
                            ..
                        }
                    ),
                )
            })
            .collect();
        assert_eq!(
            summary,
            [
                ("p".to_string(), false),
                ("p/a.txt".to_string(), false),
                ("p/sub".to_string(), false),
                ("p/sub/run".to_string(), true)
            ]
        );
    }
}
