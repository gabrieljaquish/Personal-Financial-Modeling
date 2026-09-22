//! `cargo xtask dist --compare <dirA> <dirB>`: the reproducibility report
//! (`PLAN.md` §4.1 "a diffoscope-style `xtask` comparison script that reports
//! *where* two builds differ rather than only that they do"; `ARCHITECTURE.md`
//! §9 item 4).
//!
//! Two `dist` output directories are walked side by side. Every file is digested;
//! a file that differs is opened up according to what it is:
//!
//! * a **Mach-O** (thin or universal) is split per architecture into the header
//!   and load commands, each section, each `__LINKEDIT` blob and each segment's
//!   unattributed bytes (`macho.rs`), and every region that differs is named with
//!   its size, its first differing offset and a printable excerpt of both sides;
//! * a **`.tar.gz` / `.tar`** is decompressed and compared member by member —
//!   header fields (mode, owner, mtime, type) and contents, recursing into a
//!   member that is itself a Mach-O;
//! * a **text file** (the manifest, which lists every `web/dist` file with its
//!   digest) is compared line by line, so a front-end difference is reported per
//!   bundle file;
//! * anything else gets its first differing offset.
//!
//! The report is **reported, not gating** at M0-M1 (`PLAN.md` R21): the command
//! exits 0 whatever it finds unless `--strict` is given, which is how the gate
//! will be switched on at M2.

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use crate::{archive, macho, repo};

/// How many differing regions, members or lines are detailed per file.
const DETAIL_LIMIT: usize = 40;
/// Bytes of context either side of a first difference.
const EXCERPT: usize = 48;

pub(crate) struct Outcome {
    pub(crate) report: String,
    pub(crate) identical: bool,
}

/// Entry point for `dist --compare A B [--report FILE] [--strict]`.
pub(crate) fn run(args: &[String]) -> Result<bool, String> {
    let mut dirs = Vec::new();
    let mut report_path = None;
    let mut strict = false;
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--report" => {
                report_path = Some(PathBuf::from(it.next().ok_or("--report needs a file")?));
            }
            "--strict" => strict = true,
            other if other.starts_with("--") => {
                return Err(format!("dist --compare: unknown option `{other}`"));
            }
            other => dirs.push(PathBuf::from(other)),
        }
    }
    let [a, b] = dirs.as_slice() else {
        return Err(
            "usage: cargo xtask dist --compare <dirA> <dirB> [--report FILE] [--strict]".to_owned(),
        );
    };
    let outcome = compare_dirs(a, b)?;
    if let Some(path) = report_path {
        fs::write(&path, &outcome.report).map_err(|e| format!("{}: {e}", path.display()))?;
        eprintln!("dist --compare: report written to {}", path.display());
    }
    print!("{}", outcome.report);
    if outcome.identical {
        eprintln!("dist --compare: the two outputs are byte-identical");
        Ok(true)
    } else if strict {
        eprintln!("dist --compare: the outputs differ (--strict: failing)");
        Ok(false)
    } else {
        eprintln!("dist --compare: the outputs differ; reported, not gating (PLAN.md R21)");
        Ok(true)
    }
}

fn relative_files(dir: &Path) -> Result<BTreeSet<String>, String> {
    if !dir.is_dir() {
        return Err(format!("{}: not a directory", dir.display()));
    }
    Ok(repo::walk(dir)?
        .iter()
        .map(|p| repo::relative(dir, p))
        .collect())
}

/// Compares two directory trees and renders the Markdown report.
pub(crate) fn compare_dirs(a: &Path, b: &Path) -> Result<Outcome, String> {
    let files_a = relative_files(a)?;
    let files_b = relative_files(b)?;
    let all: BTreeSet<&String> = files_a.iter().chain(files_b.iter()).collect();

    let mut summary = String::new();
    let mut details = String::new();
    let (mut same, mut differ, mut only) = (0, 0, 0);
    for rel in all {
        let in_a = files_a.contains(rel);
        let in_b = files_b.contains(rel);
        if !(in_a && in_b) {
            only += 1;
            let side = if in_a { "A" } else { "B" };
            let _ = writeln!(summary, "| `{rel}` | only in {side} | | |");
            continue;
        }
        let bytes_a = repo::read(&a.join(rel))?;
        let bytes_b = repo::read(&b.join(rel))?;
        let da = repo::sha256_hex(&bytes_a);
        let db = repo::sha256_hex(&bytes_b);
        if da == db {
            same += 1;
            let _ = writeln!(summary, "| `{rel}` | identical | `{da}` | |");
        } else {
            differ += 1;
            let _ = writeln!(summary, "| `{rel}` | **differs** | `{da}` | `{db}` |");
            let _ = writeln!(details, "\n### `{rel}`\n");
            describe(rel, &bytes_a, &bytes_b, &mut details);
        }
    }

    let identical = differ == 0 && only == 0;
    let mut report = String::new();
    let _ = writeln!(report, "# Reproducibility report\n");
    let _ = writeln!(report, "- A: `{}`", a.display());
    let _ = writeln!(report, "- B: `{}`", b.display());
    let _ =
        writeln!(
        report,
        "- Verdict: **{}** ({same} identical, {differ} differing, {only} present on one side only)",
        if identical { "byte-identical" } else { "different" }
    );
    let _ = writeln!(
        report,
        "- Status: reported, not gating at M0-M1 (PLAN.md R21; gating from M2)\n"
    );
    let _ = writeln!(report, "## Files\n");
    let _ = writeln!(report, "| File | Result | SHA-256 (A) | SHA-256 (B) |");
    let _ = writeln!(report, "|---|---|---|---|");
    report.push_str(&summary);
    if !details.is_empty() {
        let _ = writeln!(report, "\n## Where they differ");
        report.push_str(&details);
    }
    Ok(Outcome { report, identical })
}

/// Describes how two differing byte strings differ, by kind.
// The name is lowercased first, so the suffix comparisons are case-insensitive.
#[allow(clippy::case_sensitive_file_extension_comparisons)]
pub(crate) fn describe(name: &str, a: &[u8], b: &[u8], out: &mut String) {
    if macho::is_macho(a) && macho::is_macho(b) {
        match (macho::parse(a), macho::parse(b)) {
            (Ok(ma), Ok(mb)) => return describe_macho(&ma, &mb, a, b, out),
            (Err(e), _) | (_, Err(e)) => {
                let _ = writeln!(
                    out,
                    "- Mach-O could not be parsed ({e}); byte comparison follows"
                );
            }
        }
    }
    let lower = name.to_ascii_lowercase();
    let gz = lower.ends_with(".tar.gz") || lower.ends_with(".tgz");
    if gz || lower.ends_with(".tar") {
        let unpacked = if gz {
            archive::gunzip(a).and_then(|ta| archive::gunzip(b).map(|tb| (ta, tb)))
        } else {
            Ok((a.to_vec(), b.to_vec()))
        };
        match unpacked {
            Ok((ta, tb)) => {
                if gz && ta == tb {
                    let _ = writeln!(
                        out,
                        "- The decompressed tar streams are identical; only the gzip wrapper differs"
                    );
                    first_difference(a, b, out);
                    return;
                }
                match (archive::read_tar(&ta), archive::read_tar(&tb)) {
                    (Ok(ma), Ok(mb)) => return describe_tar(&ma, &mb, out),
                    (Err(e), _) | (_, Err(e)) => {
                        let _ = writeln!(out, "- tar stream could not be read ({e})");
                    }
                }
            }
            Err(e) => {
                let _ = writeln!(out, "- could not decompress ({e})");
            }
        }
    }
    if let (Ok(ta), Ok(tb)) = (std::str::from_utf8(a), std::str::from_utf8(b)) {
        return describe_text(ta, tb, out);
    }
    first_difference(a, b, out);
}

fn describe_macho(ma: &macho::MachO, mb: &macho::MachO, a: &[u8], b: &[u8], out: &mut String) {
    let _ = writeln!(
        out,
        "- Mach-O: {} [{}] vs {} [{}]",
        if ma.fat { "universal" } else { "thin" },
        ma.arches().join(", "),
        if mb.fat { "universal" } else { "thin" },
        mb.arches().join(", ")
    );
    if ma.fat && mb.fat && a.len() >= 4096 && b.len() >= 4096 && a[..4096] != b[..4096] {
        let _ = writeln!(out, "- the fat header (first page) differs");
    }
    for sa in &ma.slices {
        let Some(sb) = mb.slices.iter().find(|s| s.arch == sa.arch) else {
            let _ = writeln!(out, "- slice {} exists only in A", sa.arch);
            continue;
        };
        let slice_a = &a[sa.file_offset..sa.file_offset + sa.size];
        let slice_b = &b[sb.file_offset..sb.file_offset + sb.size];
        if slice_a == slice_b {
            let _ = writeln!(out, "- slice {}: identical", sa.arch);
            continue;
        }
        let _ = writeln!(
            out,
            "- slice {}: size {} vs {}; LC_UUID {} vs {}; minos {} vs {}; code signature {} vs {}",
            sa.arch,
            sa.size,
            sb.size,
            sa.uuid.as_deref().unwrap_or("none"),
            sb.uuid.as_deref().unwrap_or("none"),
            sa.minos.as_deref().unwrap_or("?"),
            sb.minos.as_deref().unwrap_or("?"),
            sa.has_code_signature,
            sb.has_code_signature
        );
        let names: BTreeSet<&str> = sa
            .regions
            .iter()
            .chain(sb.regions.iter())
            .map(|r| r.name.as_str())
            .collect();
        let mut shown = 0;
        let mut differing = 0;
        let mut table = String::new();
        for name in names {
            let ra = sa.regions.iter().find(|r| r.name == name);
            let rb = sb.regions.iter().find(|r| r.name == name);
            let (Some(ra), Some(rb)) = (ra, rb) else {
                differing += 1;
                let _ = writeln!(table, "  - `{name}`: present on one side only");
                continue;
            };
            let ba = &slice_a[ra.offset..ra.offset + ra.size];
            let bb = &slice_b[rb.offset..rb.offset + rb.size];
            if ba == bb {
                continue;
            }
            differing += 1;
            if shown >= DETAIL_LIMIT {
                continue;
            }
            shown += 1;
            let _ = writeln!(
                table,
                "  - `{name}`: {} vs {} bytes, SHA-256 `{}` vs `{}`",
                ba.len(),
                bb.len(),
                &repo::sha256_hex(ba)[..16],
                &repo::sha256_hex(bb)[..16]
            );
            let mut inner = String::new();
            first_difference(ba, bb, &mut inner);
            for line in inner.lines() {
                let _ = writeln!(table, "    {line}");
            }
        }
        let _ = writeln!(out, "  - {differing} region(s) differ:");
        out.push_str(&table);
        if differing > shown {
            let _ = writeln!(out, "  - … {} more not shown", differing - shown);
        }
    }
    for sb in &mb.slices {
        if !ma.slices.iter().any(|s| s.arch == sb.arch) {
            let _ = writeln!(out, "- slice {} exists only in B", sb.arch);
        }
    }
}

fn describe_tar(ma: &[archive::Member], mb: &[archive::Member], out: &mut String) {
    let names: BTreeSet<&str> = ma
        .iter()
        .chain(mb.iter())
        .map(|m| m.path.as_str())
        .collect();
    let _ = writeln!(out, "- tar members: {} vs {}", ma.len(), mb.len());
    let mut shown = 0;
    for name in names {
        let a = ma.iter().find(|m| m.path == name);
        let b = mb.iter().find(|m| m.path == name);
        let (a, b) = match (a, b) {
            (Some(a), Some(b)) => (a, b),
            (Some(_), None) => {
                let _ = writeln!(out, "  - `{name}`: only in A");
                continue;
            }
            _ => {
                let _ = writeln!(out, "  - `{name}`: only in B");
                continue;
            }
        };
        let mut header = Vec::new();
        if a.typeflag != b.typeflag {
            header.push(format!(
                "type {} vs {}",
                a.typeflag as char, b.typeflag as char
            ));
        }
        if a.mode != b.mode {
            header.push(format!("mode {:o} vs {:o}", a.mode, b.mode));
        }
        if (a.uid, a.gid, &a.uname, &a.gname) != (b.uid, b.gid, &b.uname, &b.gname) {
            header.push(format!(
                "owner {}:{} ({}/{}) vs {}:{} ({}/{})",
                a.uid, a.gid, a.uname, a.gname, b.uid, b.gid, b.uname, b.gname
            ));
        }
        if a.mtime != b.mtime {
            header.push(format!("mtime {} vs {}", a.mtime, b.mtime));
        }
        let content_differs = a.bytes != b.bytes;
        if header.is_empty() && !content_differs {
            continue;
        }
        shown += 1;
        if shown > DETAIL_LIMIT {
            continue;
        }
        let _ = writeln!(
            out,
            "  - `{name}`: {}{}",
            if header.is_empty() {
                String::new()
            } else {
                format!("header: {}; ", header.join(", "))
            },
            if content_differs {
                "contents differ"
            } else {
                "contents identical"
            }
        );
        if content_differs {
            let mut inner = String::new();
            describe(name, &a.bytes, &b.bytes, &mut inner);
            for line in inner.lines() {
                let _ = writeln!(out, "    {line}");
            }
        }
    }
}

fn describe_text(a: &str, b: &str, out: &mut String) {
    let la: BTreeSet<&str> = a.lines().collect();
    let lb: BTreeSet<&str> = b.lines().collect();
    let only_a: Vec<&&str> = la.difference(&lb).collect();
    let only_b: Vec<&&str> = lb.difference(&la).collect();
    if only_a.is_empty() && only_b.is_empty() {
        let _ = writeln!(
            out,
            "- text: the same lines in a different order or with different line endings"
        );
        return;
    }
    let _ = writeln!(
        out,
        "- text: {} line(s) only in A, {} line(s) only in B",
        only_a.len(),
        only_b.len()
    );
    for line in only_a.iter().take(DETAIL_LIMIT) {
        let _ = writeln!(out, "  - A: `{}`", line.replace('`', "'"));
    }
    for line in only_b.iter().take(DETAIL_LIMIT) {
        let _ = writeln!(out, "  - B: `{}`", line.replace('`', "'"));
    }
}

fn printable(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|&c| {
            if c.is_ascii_graphic() || c == b' ' {
                char::from(c).to_string()
            } else {
                "·".to_owned()
            }
        })
        .collect::<String>()
        .replace('`', "'")
}

/// The first differing offset, how many bytes differ (when the lengths agree)
/// and a printable excerpt of both sides around it.
pub(crate) fn first_difference(a: &[u8], b: &[u8], out: &mut String) {
    let Some(at) = a
        .iter()
        .zip(b.iter())
        .position(|(x, y)| x != y)
        .or_else(|| (a.len() != b.len()).then(|| a.len().min(b.len())))
    else {
        let _ = writeln!(out, "- identical");
        return;
    };
    let count = if a.len() == b.len() {
        format!(
            "{} differing byte(s)",
            a.iter().zip(b.iter()).filter(|(x, y)| x != y).count()
        )
    } else {
        format!("lengths {} vs {}", a.len(), b.len())
    };
    let from = at.saturating_sub(EXCERPT);
    let _ = writeln!(out, "- first difference at offset {at:#x}; {count}");
    let _ = writeln!(
        out,
        "  - A: `{}`",
        printable(&a[from.min(a.len())..(at + EXCERPT).min(a.len())])
    );
    let _ = writeln!(
        out,
        "  - B: `{}`",
        printable(&b[from.min(b.len())..(at + EXCERPT).min(b.len())])
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::archive::{Entry, EntryKind};

    fn dirs(label: &str) -> (PathBuf, PathBuf) {
        let root = repo::temp_dir(label).unwrap();
        let a = root.join("a");
        let b = root.join("b");
        fs::create_dir_all(&a).unwrap();
        fs::create_dir_all(&b).unwrap();
        (a, b)
    }

    #[test]
    fn identical_trees_are_reported_identical() {
        let (a, b) = dirs("cmp-same");
        for d in [&a, &b] {
            fs::write(d.join("x.txt"), b"same\n").unwrap();
        }
        let outcome = compare_dirs(&a, &b).unwrap();
        fs::remove_dir_all(a.parent().unwrap()).unwrap();
        assert!(outcome.identical);
        assert!(outcome.report.contains("byte-identical"));
    }

    #[test]
    fn names_the_differing_macho_section() {
        let (a, b) = dirs("cmp-macho");
        let one = macho::tests::synthetic(b"CODE-AAA", b"\0/build/one/x\0", [1; 16]);
        let two = macho::tests::synthetic(b"CODE-AAA", b"\0/build/two/x\0", [1; 16]);
        fs::write(a.join("pfp"), &one).unwrap();
        fs::write(b.join("pfp"), &two).unwrap();
        fs::write(a.join("only-a"), b"x").unwrap();
        let outcome = compare_dirs(&a, &b).unwrap();
        fs::remove_dir_all(a.parent().unwrap()).unwrap();
        assert!(!outcome.identical);
        let r = &outcome.report;
        assert!(r.contains("`__LINKEDIT string table`"), "{r}");
        assert!(!r.contains("`__TEXT,__text`:"), "{r}");
        assert!(
            r.contains("/build/one/x") && r.contains("/build/two/x"),
            "{r}"
        );
        assert!(r.contains("`only-a` | only in A"), "{r}");
    }

    #[test]
    fn opens_tarballs_member_by_member() {
        let (a, b) = dirs("cmp-tar");
        let make = |text: &[u8], mtime: u64| {
            let entries = [
                Entry {
                    path: "p".into(),
                    kind: EntryKind::Dir,
                },
                Entry {
                    path: "p/f.txt".into(),
                    kind: EntryKind::File {
                        bytes: text.to_vec(),
                        executable: false,
                    },
                },
            ];
            archive::gzip(&archive::tar_bytes(&entries, mtime).unwrap()).unwrap()
        };
        fs::write(a.join("x.tar.gz"), make(b"line one\n", 10)).unwrap();
        fs::write(b.join("x.tar.gz"), make(b"line two\n", 11)).unwrap();
        let outcome = compare_dirs(&a, &b).unwrap();
        fs::remove_dir_all(a.parent().unwrap()).unwrap();
        let r = &outcome.report;
        assert!(
            r.contains("`p/f.txt`: header: mtime 10 vs 11; contents differ"),
            "{r}"
        );
        assert!(
            r.contains("A: `line one`") && r.contains("B: `line two`"),
            "{r}"
        );
    }

    #[test]
    fn text_differences_are_listed_by_line() {
        let mut out = String::new();
        describe(
            "web.sha256",
            b"aaa  index.html\nbbb  assets/x.js\n",
            b"aaa  index.html\nccc  assets/x.js\n",
            &mut out,
        );
        assert!(
            out.contains("1 line(s) only in A, 1 line(s) only in B"),
            "{out}"
        );
        assert!(out.contains("B: `ccc  assets/x.js`"), "{out}");
    }

    #[test]
    fn first_difference_reports_offset_and_count() {
        let mut out = String::new();
        first_difference(b"abcdef", b"abXdeY", &mut out);
        assert!(out.contains("offset 0x2; 2 differing byte(s)"), "{out}");
        let mut out = String::new();
        first_difference(b"abc", b"abcd", &mut out);
        assert!(out.contains("offset 0x3; lengths 3 vs 4"), "{out}");
    }
}
