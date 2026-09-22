//! `data-hygiene`: the data-hygiene linter (`SECURITY.md` §13.3; ADR-023).
//!
//! The repository is the application only. Mechanically:
//!
//! 1. Any `.json`, `.csv`, `.ofx`, `.qif`, `.yaml`, `.yml` or data `.toml` file
//!    under version control lives in `fixtures/` or `params/`. Tool
//!    configuration in those formats (`Cargo.toml`, `package.json`, workflow
//!    YAML, …) is recognised by [`is_tool_config`] and is not data.
//! 2. Every `fixtures/` file carries `"synthetic": true` or a citation block
//!    with a `url` and a `sha256`.
//! 3. Every `params/` table carries `[[source]]` provenance (`title`, `url`,
//!    `retrieved`, `sha256`); a vintage table also carries an as-of date, a
//!    projection rule and a rounding rule. Archived primary text under
//!    `params/provenance/` is the thing the tables cite and is exempt.
//! 4. A locked vintage is immutable: every file named in `params/VINTAGES.lock`
//!    still hashes to its recorded SHA-256, and no file has been added to a
//!    locked vintage directory. Corrections ship as a new vintage.
//! 8. `TESTING.md` §11.2 gate 9, driven through the typed loader rather than
//!    restated here: every `.toml` under `params/index-series/` parses as an
//!    archived index series; every `.toml` under `params/vintages/<name>/`
//!    parses as a parameter table (projection rule; `basis = IncreaseOverBase`
//!    with its base year, base values and index series; breakdown keys that are
//!    `FilingStatus` wire forms; provenance); and each vintage assembles, which
//!    requires every `index_series` it names to resolve to an archived series
//!    table (archived, never fetched). This covers tables no crate embeds.
//!
//! It also carries the text-shaped rows of the user-data pattern check
//! (`SECURITY.md` §13.4), which apply to *every* committable file, not only to
//! data formats:
//!
//! 5. No SSN-shaped string (`NNN-NN-NNNN` on word boundaries) outside
//!    [`SSN_FIXTURE_ALLOWLIST`], which is empty.
//! 6. No JSON document carrying `"schemaVersion"` together with `"accounts"` or
//!    `"persons"` (the shape of a plan) outside `fixtures/plans/`.
//! 7. No JSON document carrying an `"asOf"` stamp (the shape of a fact file)
//!    outside `fixtures/`.
//!
//! 9. The passphrase-flag / `env::var` row of §13.4 and the `std::fs` / `std::net`
//!    capability bans of §11: both are [`crate::lint_server`], run from here so
//!    that every gate that runs `data-hygiene` (CI, the pre-commit hook) runs them.
//!
//! The remaining §13.4 rows live elsewhere or are not built yet: the container
//! magic is `check-magic`; the unlabelled-fixture row is rule 2 above; the
//! real-looking-address row arrives with `fixtures/` (`docs/contributing.md` §8).
//!
//! Rules 1–7 are presence checks on text: they stop a stray data file, an
//! unlabelled fixture or a silently edited vintage from landing at all. Rule 8 is
//! the shape check, and it is whatever `pfp-params` accepts, so the gate and the
//! engine cannot disagree about what a valid table is.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use crate::repo;

const DATA_EXTENSIONS: [&str; 7] = ["json", "csv", "ofx", "qif", "yaml", "yml", "toml"];

const CONFIG_JSON_BASENAMES: [&str; 6] = [
    "package.json",
    "package-lock.json",
    "renovate.json",
    ".eslintrc.json",
    ".prettierrc.json",
    "launch.json",
];

/// Directories whose JSON and YAML files are tool configuration by construction.
const CONFIG_DIRS: [&str; 3] = [".github/", ".claude/", ".vscode/"];

pub(crate) const LOCK_PATH: &str = "params/VINTAGES.lock";
const VINTAGES_DIR: &str = "params/vintages/";
const INDEX_SERIES_DIR: &str = "params/index-series/";

/// The documented fixture allowlist for SSN-shaped strings (`SECURITY.md` §13.4).
/// Exact repository-relative paths under `fixtures/` only; each entry needs a
/// comment saying why a synthetic fixture must carry that shape. Empty today.
const SSN_FIXTURE_ALLOWLIST: [&str; 0] = [];

pub(crate) fn run() -> Result<bool, String> {
    let root = repo::root();
    let files = repo::files(&root)?;
    let mut violations = Vec::new();
    let mut rels = Vec::new();
    let mut documents = Vec::new();
    for path in &files {
        let rel = repo::relative(&root, path);
        if rel == LOCK_PATH {
            continue;
        }
        let bytes = repo::read(path)?;
        for msg in check_file(&rel, &bytes) {
            violations.push(format!("{rel}: {msg}"));
        }
        for msg in check_patterns(&rel, &bytes) {
            violations.push(format!("{rel}: {msg}"));
        }
        if is_loader_document(&rel) {
            documents.push((rel.clone(), String::from_utf8_lossy(&bytes).into_owned()));
        }
        rels.push(rel);
    }
    let (loader_violations, vintage_ids) = check_vintages(&documents);
    violations.extend(loader_violations);
    let lock_file = root.join(LOCK_PATH);
    if lock_file.is_file() {
        let lock = String::from_utf8_lossy(&repo::read(&lock_file)?).into_owned();
        violations.extend(check_lock(&root, &lock, &rels)?);
    }
    violations.extend(crate::lint_server::violations(&root)?);
    for v in &violations {
        eprintln!("data-hygiene: {v}");
    }
    if violations.is_empty() {
        for id in &vintage_ids {
            // What a lock would record; printing it claims nothing about the vintage.
            eprintln!("data-hygiene: vintage content id {id}");
        }
        eprintln!("data-hygiene: clean");
        Ok(true)
    } else {
        eprintln!("data-hygiene: {} violation(s)", violations.len());
        Ok(false)
    }
}

fn extension(rel: &str) -> String {
    Path::new(rel)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
}

fn basename(rel: &str) -> &str {
    rel.rsplit('/').next().unwrap_or(rel)
}

fn is_data_extension(rel: &str) -> bool {
    DATA_EXTENSIONS.contains(&extension(rel).as_str())
}

/// Rules 1–3 for one file, given its repository-relative path and contents.
pub(crate) fn check_file(rel: &str, bytes: &[u8]) -> Vec<String> {
    if !is_data_extension(rel) {
        return Vec::new();
    }
    let text = String::from_utf8_lossy(bytes);
    if rel.starts_with("fixtures/") {
        fixture_rules(&text)
    } else if rel.starts_with("params/") {
        params_rules(rel, &text)
    } else if is_tool_config(rel, &text) {
        Vec::new()
    } else {
        vec!["data file outside fixtures/ or params/ (SECURITY.md §13.3 rule 1)".to_string()]
    }
}

/// Tool configuration in a data format. TOML outside `params/` is configuration
/// unless it has the shape of a parameter table; JSON and YAML are configuration
/// only at the known tool locations.
pub(crate) fn is_tool_config(rel: &str, text: &str) -> bool {
    let ext = extension(rel);
    let base = basename(rel);
    match ext.as_str() {
        "toml" => !looks_like_parameter_table(text),
        "json" => {
            CONFIG_JSON_BASENAMES.contains(&base)
                // `tsconfig.json`, `tsconfig.node.json`, …; the extension is known to be JSON here.
                || base.starts_with("tsconfig")
                || CONFIG_DIRS.iter().any(|d| rel.starts_with(d))
        }
        "yaml" | "yml" => {
            CONFIG_DIRS.iter().any(|d| rel.starts_with(d))
                || matches!(
                    base,
                    "lefthook.yml" | "lefthook.yaml" | ".pre-commit-config.yaml"
                )
        }
        _ => false,
    }
}

fn looks_like_parameter_table(text: &str) -> bool {
    let stripped = strip_whitespace(text);
    stripped.contains("[[source]]")
        || stripped.contains("[projection")
        || text.lines().any(|l| l.starts_with("values"))
}

fn strip_whitespace(text: &str) -> String {
    text.chars().filter(|c| !c.is_whitespace()).collect()
}

fn has_key(stripped: &str, key: &str) -> bool {
    // JSON `"key":`, TOML `key=`, YAML `key:`; each after whitespace removal.
    stripped.contains(&format!("\"{key}\":"))
        || stripped.contains(&format!("{key}="))
        || stripped.contains(&format!("{key}:"))
}

/// `"synthetic": true` in any of the three syntaxes.
pub(crate) fn has_synthetic_marker(text: &str) -> bool {
    let s = strip_whitespace(text);
    s.contains("\"synthetic\":true") || s.contains("synthetic=true") || s.contains("synthetic:true")
}

/// A citation block: at minimum a `url` and the `sha256` of an archived copy.
pub(crate) fn has_citation(text: &str) -> bool {
    let s = strip_whitespace(text);
    has_key(&s, "sha256") && has_key(&s, "url")
}

fn fixture_rules(text: &str) -> Vec<String> {
    if has_synthetic_marker(text) || has_citation(text) {
        Vec::new()
    } else {
        vec![
            "fixture carries neither \"synthetic\": true nor a citation block with url and sha256 (rule 2)".to_string(),
        ]
    }
}

fn params_rules(rel: &str, text: &str) -> Vec<String> {
    if rel.starts_with("params/provenance/") {
        return Vec::new();
    }
    if extension(rel) != "toml" {
        return vec![
            "params/ data must be a TOML table with [[source]] provenance, or an archived primary text under params/provenance/ (rule 3)".to_string(),
        ];
    }
    let s = strip_whitespace(text);
    let mut missing = Vec::new();
    if !s.contains("[[source]]") {
        missing.push("[[source]]");
    }
    for key in ["title", "url", "retrieved", "sha256"] {
        if !s.contains(&format!("{key}=")) {
            missing.push(key);
        }
    }
    if rel.starts_with("params/vintages/") {
        if !(s.contains("as_of=") || s.contains("asOf=")) {
            missing.push("as_of");
        }
        if !(s.contains("[projection") || s.contains(".projection]") || s.contains("projection={"))
        {
            missing.push("projection");
        }
        if !s.contains("rule=") {
            missing.push("projection.rule");
        }
        if !(s.contains("rounding=") || s.contains("rounding]")) {
            missing.push("rounding");
        }
    }
    if missing.is_empty() {
        Vec::new()
    } else {
        vec![format!(
            "parameter file is missing {} (rule 3)",
            missing.join(", ")
        )]
    }
}

/// A TOML document rule 8 hands to the typed loader.
fn is_loader_document(rel: &str) -> bool {
    extension(rel) == "toml" && (rel.starts_with(VINTAGES_DIR) || rel.starts_with(INDEX_SERIES_DIR))
}

/// Rule 8 over `(repository-relative path, text)` pairs. Returns the violations
/// and, for each vintage that assembles, its `<name>@<sha256>` content id.
pub(crate) fn check_vintages(documents: &[(String, String)]) -> (Vec<String>, Vec<String>) {
    let (violations, vintages) = parse_vintages(documents);
    let ids = vintages
        .values()
        .map(|v| v.content_id().to_string())
        .collect();
    (violations, ids)
}

/// Whether `rel` is a document [`parse_vintages`] reads: a `.toml` under
/// `params/vintages/` or `params/index-series/`.
pub(crate) fn is_vintage_document(rel: &str) -> bool {
    is_loader_document(rel)
}

/// Rule 8's parse, shared with the validation report and the assumption
/// catalogue so that every reader of `params/` goes through the one loader.
/// Returns the violations and every vintage that assembles, by name.
pub(crate) fn parse_vintages(
    documents: &[(String, String)],
) -> (Vec<String>, BTreeMap<String, pfp_params::Vintage>) {
    let mut violations = Vec::new();
    let mut series = Vec::new();
    let mut vintages: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for (rel, text) in documents {
        if rel.starts_with(INDEX_SERIES_DIR) {
            match pfp_params::IndexSeries::parse(text) {
                Ok(_) => series.push(text.as_str()),
                Err(e) => violations.push(format!(
                    "{rel}: not a valid archived index series: {e} (rule 8)"
                )),
            }
        } else if let Some((name, _)) = rel
            .strip_prefix(VINTAGES_DIR)
            .and_then(|rest| rest.split_once('/'))
        {
            match pfp_params::ParamTable::parse(text) {
                Ok(_) => vintages.entry(name).or_default().push(text),
                Err(e) => {
                    violations.push(format!("{rel}: not a valid parameter table: {e} (rule 8)"));
                }
            }
        } else {
            violations.push(format!(
                "{rel}: a parameter table belongs in a vintage directory, {VINTAGES_DIR}<name>/ (rule 8)"
            ));
        }
    }
    let mut parsed = BTreeMap::new();
    for (name, tables) in &vintages {
        match pfp_params::Vintage::parse(name, tables, &series) {
            Ok(v) => {
                parsed.insert((*name).to_owned(), v);
            }
            Err(e) => violations.push(format!(
                "{VINTAGES_DIR}{name}/: the vintage does not assemble: {e} (rule 8)"
            )),
        }
    }
    (violations, parsed)
}

/// Rules 5–7 for one file of any type. Reports never echo the matched text.
pub(crate) fn check_patterns(rel: &str, bytes: &[u8]) -> Vec<String> {
    let mut out = Vec::new();
    if !SSN_FIXTURE_ALLOWLIST.contains(&rel) {
        if let Some(line) = ssn_shaped_line(bytes) {
            out.push(format!(
                "line {line}: SSN-shaped string NNN-NN-NNNN (SECURITY.md §13.4)"
            ));
        }
    }
    let text = String::from_utf8_lossy(bytes);
    if is_json_document(rel, &text) {
        let s = strip_whitespace(&text);
        let plan_shaped = s.contains("\"schemaVersion\":")
            && (s.contains("\"accounts\":") || s.contains("\"persons\":"));
        if plan_shaped && !rel.starts_with("fixtures/plans/") {
            out.push(
                "plan-shaped JSON (\"schemaVersion\" with \"accounts\" or \"persons\") outside fixtures/plans/ (SECURITY.md §13.4)".to_string(),
            );
        }
        if s.contains("\"asOf\":") && !rel.starts_with("fixtures/") {
            out.push(
                "\"asOf\"-stamped fact file outside fixtures/ (SECURITY.md §13.4)".to_string(),
            );
        }
    }
    out
}

/// A `.json` file, or any file whose content opens like a JSON object: a plan
/// export saved under another extension is still a plan.
fn is_json_document(rel: &str, text: &str) -> bool {
    extension(rel) == "json" || text.trim_start().starts_with('{')
}

fn is_word_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// The 1-based line of the first `\b\d{3}-\d{2}-\d{4}\b` match, if any.
fn ssn_shaped_line(bytes: &[u8]) -> Option<usize> {
    const SHAPE: [u8; 11] = *b"ddd-dd-dddd";
    let hit = bytes.windows(SHAPE.len()).enumerate().find(|(i, w)| {
        let shaped = w.iter().zip(SHAPE).all(|(b, s)| match s {
            b'd' => b.is_ascii_digit(),
            _ => *b == b'-',
        });
        let bounded_before = *i == 0 || !is_word_byte(bytes[*i - 1]);
        let bounded_after = !bytes
            .get(*i + SHAPE.len())
            .is_some_and(|b| is_word_byte(*b));
        shaped && bounded_before && bounded_after
    })?;
    // Newline-separated pieces before the match: one more than the newlines.
    Some(bytes[..hit.0].split(|b| *b == b'\n').count())
}

/// One `<sha256 hex>  <repository-relative path>` per line; `#` starts a comment.
pub(crate) fn parse_lock(text: &str) -> Result<Vec<(String, String)>, String> {
    let mut entries = Vec::new();
    for (n, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut parts = line.split_whitespace();
        match (parts.next(), parts.next(), parts.next()) {
            (Some(hash), Some(path), None)
                if hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit()) =>
            {
                entries.push((hash.to_ascii_lowercase(), path.to_string()));
            }
            _ => return Err(format!("{LOCK_PATH}:{}: expected `<sha256> <path>`", n + 1)),
        }
    }
    Ok(entries)
}

/// The vintage directories (`params/vintages/<name>/`) that the lock covers.
pub(crate) fn locked_vintage_dirs(entries: &[(String, String)]) -> BTreeSet<String> {
    entries
        .iter()
        .filter_map(|(_, path)| {
            let rest = path.strip_prefix("params/vintages/")?;
            let name = rest.split('/').next()?;
            Some(format!("params/vintages/{name}/"))
        })
        .collect()
}

/// One way rule 4 can fail for one file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum LockProblem {
    /// Named in the lock; no such file.
    Missing,
    /// The file no longer hashes to its recorded SHA-256.
    Modified { actual: String, recorded: String },
    /// Under a locked vintage directory with no lock entry.
    Unlisted,
}

/// A file that fails rule 4, and how.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LockFinding {
    /// Repository-relative path.
    pub(crate) rel: String,
    pub(crate) problem: LockProblem,
}

/// Rule 4 over the whole tree, as findings: every entry's file is hashed and
/// compared, and every file under a vintage directory the lock names must be
/// listed. `tracked` is every repository-relative path. This is the one
/// implementation; the data-hygiene gate formats it and the validation report
/// counts it, so the two cannot disagree about whether a vintage is locked.
pub(crate) fn lock_findings(
    root: &Path,
    entries: &[(String, String)],
    tracked: &[String],
) -> Result<Vec<LockFinding>, String> {
    let mut out = Vec::new();
    let mut listed = BTreeSet::new();
    for (hash, rel) in entries {
        listed.insert(rel.clone());
        let path = root.join(rel);
        if !path.is_file() {
            out.push(LockFinding {
                rel: rel.clone(),
                problem: LockProblem::Missing,
            });
            continue;
        }
        let actual = repo::sha256_hex(&repo::read(&path)?);
        if &actual != hash {
            out.push(LockFinding {
                rel: rel.clone(),
                problem: LockProblem::Modified {
                    actual,
                    recorded: hash.clone(),
                },
            });
        }
    }
    let dirs = locked_vintage_dirs(entries);
    for rel in tracked {
        if dirs.iter().any(|d| rel.starts_with(d)) && !listed.contains(rel) {
            out.push(LockFinding {
                rel: rel.clone(),
                problem: LockProblem::Unlisted,
            });
        }
    }
    Ok(out)
}

/// Rule 4 over the whole tree: `tracked` is every repository-relative path.
fn check_lock(root: &Path, lock: &str, tracked: &[String]) -> Result<Vec<String>, String> {
    let entries = parse_lock(lock)?;
    Ok(lock_findings(root, &entries, tracked)?
        .into_iter()
        .map(|finding| {
            let rel = finding.rel;
            match finding.problem {
                LockProblem::Missing => format!(
                    "{rel}: named in {LOCK_PATH} but missing (locked vintages are immutable, rule 4)"
                ),
                LockProblem::Modified { actual, recorded } => format!(
                    "{rel}: locked vintage modified (sha256 {actual} != {recorded}); corrections ship as a new vintage (rule 4)"
                ),
                LockProblem::Unlisted => format!(
                    "{rel}: added to a locked vintage directory without a {LOCK_PATH} entry (rule 4)"
                ),
            }
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn data_outside_fixtures_and_params_is_rejected() {
        assert_eq!(check_file("web/src/data.json", b"{}").len(), 1);
        assert_eq!(check_file("docs/table.csv", b"a,b\n1,2\n").len(), 1);
        assert_eq!(
            check_file("crates/pfp-tax/src/brackets.yaml", b"a: 1").len(),
            1
        );
        assert!(check_file("README.md", b"text").is_empty());
    }

    #[test]
    fn tool_configuration_is_not_data() {
        assert!(check_file("Cargo.toml", b"[workspace]\nmembers = []\n").is_empty());
        assert!(check_file("deny.toml", b"[licenses]\nallow = [\"MIT\"]\n").is_empty());
        assert!(check_file("web/package.json", b"{\"name\": \"web\"}").is_empty());
        assert!(check_file("web/tsconfig.node.json", b"{}").is_empty());
        assert!(check_file(".github/workflows/ci.yml", b"on: push").is_empty());
        assert!(check_file("lefthook.yml", b"pre-commit:").is_empty());
    }

    #[test]
    fn a_parameter_table_outside_params_is_data() {
        let table = b"id = \"x\"\n[projection]\nrule = \"index\"\n[[source]]\nurl = \"u\"\n";
        assert_eq!(check_file("crates/pfp-tax/brackets.toml", table).len(), 1);
    }

    #[test]
    fn fixtures_need_the_synthetic_marker_or_a_citation() {
        assert!(check_file(
            "fixtures/plans/demo.plan.json",
            b"{ \"synthetic\" : true, \"seed\": 7 }"
        )
        .is_empty());
        assert!(check_file(
            "fixtures/tier1/tax/ex1.json",
            b"{\"source\": {\"url\": \"https://example.invalid/p\", \"sha256\": \"ab\"}}"
        )
        .is_empty());
        assert!(check_file("fixtures/tier2/x.yaml", b"synthetic: true\n").is_empty());
        assert_eq!(
            check_file("fixtures/tier1/tax/bare.json", b"{\"inputs\": {}}").len(),
            1
        );
        assert_eq!(check_file("fixtures/personas/p.csv", b"a,b\n").len(), 1);
    }

    #[test]
    fn params_tables_need_provenance_and_projection() {
        let good = b"id = \"fed.brackets\"\nas_of = \"2026-01-01\"\n[projection]\nrule = \"index\"\n[projection.rounding]\nincrement = 5000\n[[source]]\ntitle = \"t\"\nurl = \"u\"\nretrieved = \"2026-09-17\"\nsha256 = \"aa\"\n";
        assert!(check_file("params/vintages/federal-2026/brackets.toml", good).is_empty());
        let v = check_file("params/vintages/federal-2026/bare.toml", b"id = \"x\"\n");
        assert_eq!(v.len(), 1);
        assert!(
            v[0].contains("[[source]]")
                && v[0].contains("projection.rule")
                && v[0].contains("rounding"),
            "{}",
            v[0]
        );
        // A state module or assumption set needs provenance but not a projection rule.
        let state = b"[[source]]\ntitle = \"t\"\nurl = \"u\"\nretrieved = \"d\"\nsha256 = \"aa\"\n";
        assert!(check_file("params/states/xx.toml", state).is_empty());
        assert!(check_file("params/provenance/irs/rp25-32.csv", b"raw archived text").is_empty());
        assert_eq!(
            check_file("params/vintages/federal-2026/series.csv", b"1,2").len(),
            1
        );
    }

    /// SYNTHETIC: describes no law and no publication.
    const SYNTHETIC_TABLE: &str = r#"
id = "test.amount"
unit = "USD"
breakdown = ["filingStatus"]
as_of = "2001-01-01"
[values.single]
2001 = 1000
[values.mfj]
2001 = 2000
[values.mfs]
2001 = 1000
[values.hoh]
2001 = 1500
[values.qss]
2001 = 2000
[projection]
rule = "index"
index = "test.index"
index_series = "test.series"
lag_years = 1
base_year = 2000
[projection.base_values]
single = 1000
mfj = 2000
mfs = 1000
hoh = 1500
qss = 2000
[projection.rounding]
increment = 50
direction = "down"
basis = "IncreaseOverBase"
[[source]]
title = "Synthetic source"
url = "https://example.invalid/doc.pdf"
retrieved = "2001-02-03"
sha256 = "0000000000000000000000000000000000000000000000000000000000000000"
"#;

    /// SYNTHETIC.
    const SYNTHETIC_SERIES: &str = r#"
id = "test.series.table"
kind = "index-series"
index_series = "test.series"
as_of = "2001-01-01"
[window]
kind = "trailing-12-month-mean"
ends_month = 12
months = 1
[observations.2000]
"2000-M12" = "100.0"
[[source]]
title = "Synthetic series"
url = "https://example.invalid/series"
retrieved = "2001-02-03"
sha256 = "0000000000000000000000000000000000000000000000000000000000000000"
"#;

    fn docs(items: &[(&str, &str)]) -> Vec<(String, String)> {
        items
            .iter()
            .map(|(rel, text)| ((*rel).to_string(), (*text).to_string()))
            .collect()
    }

    #[test]
    fn the_loader_checks_every_vintage_table_whether_or_not_a_crate_embeds_it() {
        let table = "params/vintages/synthetic-2001/amount.toml";
        let series = "params/index-series/test.toml";
        let (v, ids) = check_vintages(&docs(&[
            (table, SYNTHETIC_TABLE),
            (series, SYNTHETIC_SERIES),
        ]));
        assert!(v.is_empty(), "{v:?}");
        assert_eq!(ids.len(), 1);
        assert!(ids[0].starts_with("synthetic-2001@"), "{}", ids[0]);

        // A named index series that is not archived is a gate-9 error.
        let (v, ids) = check_vintages(&docs(&[(table, SYNTHETIC_TABLE)]));
        assert!(ids.is_empty());
        assert_eq!(v.len(), 1);
        assert!(v[0].contains("resolves to no archived series"), "{}", v[0]);

        // Each gate-9 field, removed in turn, fails the table that lost it.
        for (from, to) in [
            ("base_year = 2000\n", ""),
            ("index_series = \"test.series\"\n", ""),
            ("lag_years = 1\n", ""),
            ("rule = \"index\"\n", ""),
            ("[projection.rounding]\nincrement = 50\ndirection = \"down\"\nbasis = \"IncreaseOverBase\"\n", ""),
            ("[values.hoh]", "[values.head]"),
            ("hoh = 1500\n", ""),
            ("2001 = 1500", "2001 = 1500.0"),
        ] {
            assert!(SYNTHETIC_TABLE.contains(from), "{from}");
            let mutated = SYNTHETIC_TABLE.replacen(from, to, 1);
            let (v, _) = check_vintages(&docs(&[(table, &mutated), (series, SYNTHETIC_SERIES)]));
            assert_eq!(v.len(), 1, "{from:?}: {v:?}");
            assert!(v[0].starts_with(table), "{}", v[0]);
        }

        // A malformed series fails on its own line, and the vintage reading it
        // then fails to resolve.
        let broken = SYNTHETIC_SERIES.replacen("kind = \"index-series\"", "kind = \"x\"", 1);
        let (v, _) = check_vintages(&docs(&[(table, SYNTHETIC_TABLE), (series, &broken)]));
        assert_eq!(v.len(), 2, "{v:?}");

        // A table dropped straight into params/vintages/ belongs to no vintage.
        let (v, _) = check_vintages(&docs(&[("params/vintages/loose.toml", SYNTHETIC_TABLE)]));
        assert_eq!(v.len(), 1);
    }

    #[test]
    fn lock_parses_and_detects_edits_and_additions() {
        let root = repo::temp_dir("lock").unwrap();
        let vintage = root.join("params/vintages/federal-2026");
        fs::create_dir_all(&vintage).unwrap();
        fs::write(vintage.join("brackets.toml"), b"values = 1\n").unwrap();
        let hash = repo::sha256_hex(b"values = 1\n");
        let lock = format!("# locked\n{hash}  params/vintages/federal-2026/brackets.toml\n");
        let tracked = vec!["params/vintages/federal-2026/brackets.toml".to_string()];
        assert!(check_lock(&root, &lock, &tracked).unwrap().is_empty());

        fs::write(vintage.join("brackets.toml"), b"values = 2\n").unwrap();
        let v = check_lock(&root, &lock, &tracked).unwrap();
        assert_eq!(v.len(), 1);
        assert!(v[0].contains("locked vintage modified"), "{}", v[0]);

        fs::write(vintage.join("brackets.toml"), b"values = 1\n").unwrap();
        let added = vec![
            tracked[0].clone(),
            "params/vintages/federal-2026/extra.toml".to_string(),
        ];
        let v = check_lock(&root, &lock, &added).unwrap();
        assert_eq!(v.len(), 1);
        assert!(v[0].contains("added to a locked vintage"), "{}", v[0]);

        fs::remove_dir_all(&root).unwrap();
        assert!(parse_lock("notahash path").is_err());
        assert_eq!(
            locked_vintage_dirs(&parse_lock(&lock).unwrap())
                .into_iter()
                .collect::<Vec<_>>(),
            vec!["params/vintages/federal-2026/".to_string()]
        );
    }

    #[test]
    fn ssn_shaped_strings_fail_everywhere() {
        // Assembled at run time so that this file does not carry the shape.
        let shaped = ["900", "00", "0000"].join("-");
        let text = format!("first line\nid = \"{shaped}\"\n");
        let v = check_patterns("crates/pfp-domain/src/lib.rs", text.as_bytes());
        assert_eq!(v.len(), 1);
        assert!(v[0].starts_with("line 2:"), "{}", v[0]);
        assert!(
            !v[0].contains(&shaped),
            "the report must not echo the match"
        );
        assert_eq!(
            check_patterns("fixtures/personas/p.json", text.as_bytes()).len(),
            1
        );
        assert_eq!(check_patterns("notes.txt", shaped.as_bytes()).len(), 1);
        // Word boundaries: a longer digit run or an identifier is not the shape.
        for near in [
            format!("1{shaped}"),
            format!("{shaped}5"),
            format!("x_{shaped}"),
        ] {
            assert!(
                check_patterns("a.txt", near.as_bytes()).is_empty(),
                "{near}"
            );
        }
        assert!(check_patterns("a.txt", b"2026-09-18 and 555-0100").is_empty());
    }

    #[test]
    fn plan_shaped_json_lives_only_under_fixtures_plans() {
        let plan = b"{ \"synthetic\": true, \"schemaVersion\": 1, \"persons\": [] }";
        assert!(check_patterns("fixtures/plans/demo.plan.json", plan).is_empty());
        assert_eq!(check_patterns("fixtures/tier2/p.json", plan).len(), 1);
        assert_eq!(check_patterns("web/src/sample.json", plan).len(), 1);
        // A plan saved under another extension is still a plan.
        assert_eq!(check_patterns("docs/example.txt", plan).len(), 1);
        let accounts = b"{\"schemaVersion\":1,\"accounts\":[]}";
        assert_eq!(check_patterns("plan-copy.json", accounts).len(), 1);
        // `schemaVersion` alone is not a plan, and prose about the keys is not JSON.
        assert!(check_patterns("web/x.json", b"{\"schemaVersion\": 1}").is_empty());
        assert!(check_patterns(
            "docs/notes.md",
            b"A plan has \"schemaVersion\": 1 and \"persons\": [].",
        )
        .is_empty());
    }

    #[test]
    fn as_of_stamped_fact_files_live_only_under_fixtures() {
        let fact = b"{ \"synthetic\": true, \"asOf\": \"2026-01-01\", \"value\": 1 }";
        assert!(check_patterns("fixtures/tier2/fact.json", fact).is_empty());
        assert_eq!(check_patterns("web/src/fact.json", fact).len(), 1);
        assert_eq!(check_patterns("facts.dat", fact).len(), 1);
        // A parameter table's TOML as-of date is not a JSON fact stamp.
        assert!(check_patterns("params/states/xx.toml", b"asOf = \"2026-01-01\"\n").is_empty());
    }

    #[test]
    fn the_repository_tree_is_clean() {
        assert!(run().unwrap());
    }
}
