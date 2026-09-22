//! Computing the validation report from the repository.
//!
//! Everything here is a function of the committable tree (`repo::files`: tracked
//! files plus untracked files `.gitignore` does not exclude, in sorted order) and
//! of nothing else: no clock, no host name, no absolute path, no user. Every
//! collection is sorted or keyed by a `BTreeMap`, so two runs over one tree
//! produce one report byte for byte.
//!
//! The parsing of `params/` is the data-hygiene gate's (`hygiene::parse_vintages`,
//! which is `pfp-params` itself), so the report and the engine cannot disagree
//! about what a vintage is. Fixture envelopes are read as JSON documents; a file
//! that does not parse is reported as invalid, never skipped silently.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde_json::Value;

use super::docs;
use super::model::{
    ArchiveCheck, CountByKey, FixtureFile, FixtureGroup, FixtureReport, InvalidFixture, Invariant,
    LockReport, OpenItem, ParameterReport, PerformanceBudget, PerformanceReport, Pins,
    PropertyFile, PropertyReport, ProvenanceReport, RefusedTarget, SectionState, SectionStatus,
    SecurityArea, SecurityReport, SnapshotReport, TableReport, TestInventory, TierReport,
    UnverifiedItem, UnverifiedReport, ValidationReport, VintageReport,
};
use crate::{hygiene, repo};

/// The four fixture directories the report always lists, with the milestone the
/// design introduces each in and where it says so.
const TIERS: [(&str, &str, &str); 4] = [
    ("tier1", "M0", "TESTING.md §2.1, §2.2 and §3.1"),
    (
        "tier2",
        "M1",
        "TESTING.md §2.1 and §3.5; §1.1 places the first pinned suite (Tax-Calculator) at M1",
    ),
    (
        "tier3",
        "M3",
        "TESTING.md §2.1 and §3.6; §1.1 places the persona goldens at M3 and the first recorded oracle at M1",
    ),
    (
        "pending",
        "M0",
        "fixtures/pending/README.md; TESTING.md §2.2 (the tier-1 loader rejects pending-hand-verification)",
    ),
];

const TESTING_DOC: &str = "docs/TESTING.md";
const PROVENANCE_DIR: &str = "params/provenance/";
const PROVENANCE_INDEX: &str = "params/provenance/INDEX.toml";

/// The verification values, worst first, so a table of counts reads top down
/// from what needs a person to what has one.
const VERIFICATION_ORDER: [&str; 5] = [
    "pending-hand-verification",
    "unstated",
    "derived",
    "hand-worked-reviewed",
    "primary-source-confirmed",
];

fn count(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

fn counts(map: &BTreeMap<String, u32>) -> Vec<CountByKey> {
    map.iter()
        .map(|(name, count)| CountByKey {
            name: name.clone(),
            count: *count,
        })
        .collect()
}

fn bump(map: &mut BTreeMap<String, u32>, key: &str) {
    *map.entry(key.to_owned()).or_insert(0) += 1;
}

/// Verification counts in [`VERIFICATION_ORDER`], then anything else alphabetically.
fn verification_counts(map: &BTreeMap<String, u32>) -> Vec<CountByKey> {
    let mut out: Vec<CountByKey> = VERIFICATION_ORDER
        .iter()
        .filter_map(|key| {
            map.get(*key).map(|n| CountByKey {
                name: (*key).to_owned(),
                count: *n,
            })
        })
        .collect();
    out.extend(
        map.iter()
            .filter(|(k, _)| !VERIFICATION_ORDER.contains(&k.as_str()))
            .map(|(k, n)| CountByKey {
                name: k.clone(),
                count: *n,
            }),
    );
    out
}

fn status(state: SectionState, milestone: &str, reference: &str, note: String) -> SectionStatus {
    SectionStatus {
        state,
        milestone: milestone.to_owned(),
        reference: reference.to_owned(),
        note,
    }
}

/// The whole tree, once: repository-relative paths and a reader.
struct Tree<'a> {
    root: &'a Path,
    rels: Vec<String>,
}

impl Tree<'_> {
    fn read(&self, rel: &str) -> Result<Vec<u8>, String> {
        repo::read(&self.root.join(rel))
    }

    fn text(&self, rel: &str) -> Result<String, String> {
        Ok(String::from_utf8_lossy(&self.read(rel)?).into_owned())
    }

    fn under<'b>(&'b self, prefix: &'b str) -> impl Iterator<Item = &'b String> + 'b {
        self.rels.iter().filter(move |r| r.starts_with(prefix))
    }

    fn has_dir(&self, dir: &str) -> bool {
        self.root.join(dir).is_dir()
    }

    /// The crate directories under `crates/`, plus `xtask`, sorted.
    fn crate_dirs(&self) -> Vec<String> {
        let mut out = BTreeSet::new();
        for rel in self.under("crates/") {
            if let Some(name) = rel
                .strip_prefix("crates/")
                .and_then(|r| r.split('/').next())
            {
                out.insert(format!("crates/{name}"));
            }
        }
        if self.has_dir("xtask") {
            out.insert("xtask".to_owned());
        }
        out.into_iter().collect()
    }
}

/// Builds the report for the repository at `root`.
pub(crate) fn build(root: &Path, generated_on: Option<String>) -> Result<ValidationReport, String> {
    let files = repo::files(root)?;
    let tree = Tree {
        root,
        rels: files.iter().map(|p| repo::relative(root, p)).collect(),
    };
    let testing = tree.text(TESTING_DOC)?;

    let fixtures = fixtures(&tree)?;
    let parameters = parameters(&tree)?;
    let unverified = unverified(&testing, &fixtures, &parameters)?;
    let pins = pins(&parameters);
    Ok(ValidationReport {
        generated_on,
        basis: "Computed by `cargo xtask validation-report` from the repository contents alone: \
the fixture envelopes, the parameter documents and the provenance catalogue, the test sources, \
and the tables of docs/TESTING.md. No clock, host name, path or person enters it; the same \
tree always yields the same report."
            .to_owned(),
        fixtures,
        parameters,
        properties: properties(&tree, &testing)?,
        contract_snapshots: snapshots(&tree),
        security_suite: security(&tree, &testing)?,
        tier2_suites: tier2_status(&tree),
        tier3_goldens: tier3_status(&tree),
        oracles: oracles_status(&tree),
        mutation_score: mutation_status(&tree),
        fuzz_corpora: fuzz_status(&tree),
        browser_end_to_end: browser_status(&tree),
        performance_budgets: performance(&tree, &testing)?,
        test_inventory: test_inventory(&tree)?,
        unverified,
        refused: refused(&testing)?,
        pins,
    })
}

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

/// Whether `rel` has the file extension `ext`, ignoring case.
fn ext_is(rel: &str, ext: &str) -> bool {
    Path::new(rel)
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case(ext))
}

fn is_note(rel: &str) -> bool {
    ext_is(rel, "md")
}

fn json_str(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(Value::as_str).map(str::to_owned)
}

/// One envelope, as declared. Every field is what the document says, or absent.
fn envelope(rel: &str, text: &str) -> Result<FixtureFile, String> {
    let value: Value = serde_json::from_str(text).map_err(|e| format!("not JSON: {e}"))?;
    if !value.is_object() {
        return Err("not a JSON object".to_owned());
    }
    let expect = value.get("expect");
    let case_count = expect
        .and_then(|e| e.get("cases").or_else(|| e.get("lines")))
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    Ok(FixtureFile {
        path: rel.to_owned(),
        id: json_str(&value, "id"),
        module: json_str(&value, "module"),
        milestone: json_str(&value, "milestone"),
        verification: json_str(&value, "verification").unwrap_or_else(|| "unstated".to_owned()),
        synthetic: value.get("synthetic").and_then(Value::as_bool),
        promotes_to: json_str(&value, "promotesTo"),
        param_vintage: json_str(&value, "paramVintage"),
        review_kind: value
            .get("reviewedBy")
            .and_then(|r| r.get("kind"))
            .and_then(Value::as_str)
            .map(str::to_owned),
        case_count: count(case_count),
    })
}

fn tier_status(
    tier: &str,
    milestone: &str,
    reference: &str,
    file_count: u32,
    pending: u32,
) -> SectionStatus {
    match (tier, file_count) {
        ("tier1", 0) => status(
            SectionState::Empty,
            milestone,
            reference,
            format!(
                "Specified for M0 and empty at this commit: no fixture has been promoted. \
{pending} fixture(s) wait under fixtures/pending/ for a human to read every expected value back \
against the cited primary document (docs/verification/m0-hand-verification.md)."
            ),
        ),
        ("pending", _) => status(
            SectionState::Present,
            milestone,
            reference,
            "Not a tier of TESTING.md §2.1. Every file here is `pending-hand-verification`, which the \
tier-1 loader rejects; nothing in this directory is validated, and nothing computed from it is \
for decisions."
                .to_owned(),
        ),
        (_, 0) => status(
            SectionState::NotYetIntroduced,
            milestone,
            reference,
            format!("fixtures/{tier}/ does not exist at this commit."),
        ),
        _ => status(
            SectionState::Present,
            milestone,
            reference,
            format!("{file_count} fixture document(s)."),
        ),
    }
}

/// One tier directory; unreadable envelopes go to `invalid`.
fn tier_report(
    tree: &Tree<'_>,
    tier: &str,
    milestone: &str,
    reference: &str,
    pending_count: u32,
    invalid: &mut Vec<InvalidFixture>,
) -> Result<TierReport, String> {
    let directory = format!("fixtures/{tier}");
    let prefix = format!("{directory}/");
    let mut files = Vec::new();
    let mut file_count = 0_usize;
    let mut by_milestone = BTreeMap::new();
    let mut by_verification = BTreeMap::new();
    let mut by_module = BTreeMap::new();
    let mut by_both: BTreeMap<(String, String), u32> = BTreeMap::new();
    let mut review_kinds = BTreeMap::new();
    for rel in tree.under(&prefix) {
        if is_note(rel) {
            continue;
        }
        file_count += 1;
        if !ext_is(rel, "json") {
            // Snapshots and recorded oracle output are counted, not read.
            continue;
        }
        match envelope(rel, &tree.text(rel)?) {
            Ok(file) => {
                let milestone = file
                    .milestone
                    .clone()
                    .unwrap_or_else(|| "unstated".to_owned());
                bump(&mut by_milestone, &milestone);
                bump(&mut by_verification, &file.verification);
                bump(&mut by_module, file.module.as_deref().unwrap_or("unstated"));
                *by_both
                    .entry((milestone, file.verification.clone()))
                    .or_insert(0) += 1;
                if let Some(kind) = &file.review_kind {
                    bump(&mut review_kinds, kind);
                }
                files.push(file);
            }
            Err(problem) => invalid.push(InvalidFixture {
                path: rel.clone(),
                problem,
            }),
        }
    }
    let file_count = count(file_count);
    Ok(TierReport {
        tier: tier.to_owned(),
        status: tier_status(tier, milestone, reference, file_count, pending_count),
        directory,
        file_count,
        by_milestone: counts(&by_milestone),
        by_verification: verification_counts(&by_verification),
        by_module: counts(&by_module),
        by_milestone_and_verification: by_both
            .into_iter()
            .map(|((milestone, verification), count)| FixtureGroup {
                milestone,
                verification,
                count,
            })
            .collect(),
        review_kinds: counts(&review_kinds),
        files,
    })
}

fn fixtures(tree: &Tree<'_>) -> Result<FixtureReport, String> {
    let mut invalid = Vec::new();
    let mut tiers = Vec::new();
    let pending_count = count(
        tree.under("fixtures/pending/")
            .filter(|r| !is_note(r))
            .count(),
    );
    for (tier, milestone, reference) in TIERS {
        tiers.push(tier_report(
            tree,
            tier,
            milestone,
            reference,
            pending_count,
            &mut invalid,
        )?);
    }

    let mut other: BTreeMap<String, u32> = BTreeMap::new();
    for rel in tree.under("fixtures/") {
        let Some(dir) = rel
            .strip_prefix("fixtures/")
            .and_then(|r| r.split_once('/'))
            .map(|(d, _)| d)
        else {
            continue;
        };
        if TIERS.iter().any(|(t, _, _)| *t == dir) || is_note(rel) {
            continue;
        }
        bump(&mut other, &format!("fixtures/{dir}"));
    }

    let tier1_count = tiers[0].file_count;
    let state = if tier1_count == 0 {
        SectionState::Empty
    } else {
        SectionState::Partial
    };
    Ok(FixtureReport {
        status: status(
            state,
            "M0",
            "TESTING.md §2; PLAN.md §4.1",
            format!(
                "{tier1_count} tier-1 fixture(s); {pending_count} pending human verification. Counts are \
of fixture documents on disk, grouped by the tier directory they sit in and the milestone and \
verification value each envelope declares."
            ),
        ),
        tiers,
        invalid,
        other_directories: counts(&other),
    })
}

// ---------------------------------------------------------------------------
// Parameters
// ---------------------------------------------------------------------------

fn archive_check(tree: &Tree<'_>, sources: &[pfp_params::Source]) -> Result<ArchiveCheck, String> {
    let mut check = ArchiveCheck {
        source_count: count(sources.len()),
        archived_count: 0,
        checksum_match_count: 0,
        checksum_mismatch_count: 0,
        missing_count: 0,
    };
    for source in sources {
        let Some(archive) = source.archive() else {
            continue;
        };
        check.archived_count += 1;
        let path = tree.root.join(archive);
        if !path.is_file() {
            check.missing_count += 1;
            continue;
        }
        if repo::sha256_hex(&repo::read(&path)?) == source.sha256().to_ascii_lowercase() {
            check.checksum_match_count += 1;
        } else {
            check.checksum_mismatch_count += 1;
        }
    }
    Ok(check)
}

fn add_check(total: &mut ArchiveCheck, part: &ArchiveCheck) {
    total.source_count += part.source_count;
    total.archived_count += part.archived_count;
    total.checksum_match_count += part.checksum_match_count;
    total.checksum_mismatch_count += part.checksum_mismatch_count;
    total.missing_count += part.missing_count;
}

fn wire(v: pfp_params::VerificationStatus) -> String {
    v.wire().unwrap_or("unstated").to_owned()
}

fn empty_check() -> ArchiveCheck {
    ArchiveCheck {
        source_count: 0,
        archived_count: 0,
        checksum_match_count: 0,
        checksum_mismatch_count: 0,
        missing_count: 0,
    }
}

fn lock(tree: &Tree<'_>) -> Result<LockReport, String> {
    let path = tree.root.join(hygiene::LOCK_PATH);
    if !path.is_file() {
        return Ok(LockReport {
            present: false,
            entry_count: 0,
            locked_vintages: Vec::new(),
        });
    }
    let entries = hygiene::parse_lock(&String::from_utf8_lossy(&repo::read(&path)?))?;
    Ok(LockReport {
        present: true,
        entry_count: count(entries.len()),
        locked_vintages: hygiene::locked_vintage_dirs(&entries).into_iter().collect(),
    })
}

fn provenance(tree: &Tree<'_>) -> Result<ProvenanceReport, String> {
    let file_count = count(tree.under(PROVENANCE_DIR).count());
    let mut report = ProvenanceReport {
        file_count,
        catalogued_count: 0,
        checksum_match_count: 0,
        checksum_mismatch_count: 0,
        missing_count: 0,
        verification: None,
    };
    if !tree.root.join(PROVENANCE_INDEX).is_file() {
        return Ok(report);
    }
    // A line scan of the catalogue: `[[document]]` opens an entry, `path` and
    // `sha256` are its checked fields, and a top-level `verification` is quoted.
    let index = tree.text(PROVENANCE_INDEX)?;
    let mut in_document = false;
    let mut path: Option<String> = None;
    let mut sha: Option<String> = None;
    let flush = |path: &mut Option<String>,
                 sha: &mut Option<String>,
                 report: &mut ProvenanceReport|
     -> Result<(), String> {
        if let (Some(p), Some(s)) = (path.take(), sha.take()) {
            let file = tree.root.join(&p);
            if !file.is_file() {
                report.missing_count += 1;
            } else if repo::sha256_hex(&repo::read(&file)?) == s.to_ascii_lowercase() {
                report.checksum_match_count += 1;
            } else {
                report.checksum_mismatch_count += 1;
            }
        }
        Ok(())
    };
    for line in index.lines() {
        let line = line.trim();
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        if line == "[[document]]" {
            flush(&mut path, &mut sha, &mut report)?;
            in_document = true;
            report.catalogued_count += 1;
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim().trim_matches('"').to_owned();
        match (in_document, key) {
            (false, "verification") => report.verification = Some(value),
            (true, "path") => path = Some(value),
            (true, "sha256") => sha = Some(value),
            _ => {}
        }
    }
    flush(&mut path, &mut sha, &mut report)?;
    Ok(report)
}

/// One document of a vintage: a table or the series it reads.
fn document_report(
    tree: &Tree<'_>,
    id: &str,
    kind: &str,
    verification: pfp_params::VerificationStatus,
    open_items: &[String],
    sources: &[pfp_params::Source],
) -> Result<TableReport, String> {
    Ok(TableReport {
        id: id.to_owned(),
        kind: kind.to_owned(),
        verification: wire(verification),
        open_item_count: count(open_items.len()),
        open_items: open_items.to_vec(),
        archive: archive_check(tree, sources)?,
    })
}

fn vintage_report(
    tree: &Tree<'_>,
    name: &str,
    vintage: &pfp_params::Vintage,
    lock: &LockReport,
) -> Result<VintageReport, String> {
    let mut docs = Vec::new();
    for table in vintage.tables() {
        docs.push(document_report(
            tree,
            table.id(),
            "table",
            table.verification(),
            table.open_items(),
            table.sources(),
        )?);
    }
    let series_names: BTreeSet<&str> = vintage
        .tables()
        .filter_map(|t| t.projection().index_series.as_deref())
        .collect();
    let mut series_count = 0_usize;
    for series_name in series_names {
        let Some(series) = vintage.index_series(series_name) else {
            continue;
        };
        series_count += 1;
        docs.push(document_report(
            tree,
            series.id(),
            "index-series",
            series.verification(),
            series.open_items(),
            series.sources(),
        )?);
    }
    let mut verification: BTreeMap<String, u32> = BTreeMap::new();
    let mut archive = empty_check();
    let mut open_item_count = 0_u32;
    for doc in &docs {
        bump(&mut verification, &doc.verification);
        add_check(&mut archive, &doc.archive);
        open_item_count += doc.open_item_count;
    }
    let locked = lock
        .locked_vintages
        .iter()
        .any(|dir| dir == &format!("params/vintages/{name}/"));
    let content_id = vintage.content_id().to_string();
    Ok(VintageReport {
        name: name.to_owned(),
        locked,
        locked_id: locked.then(|| content_id.clone()),
        content_id,
        verified: vintage.is_verified(),
        verification: verification_counts(&verification),
        table_count: count(vintage.tables().count()),
        index_series_count: count(series_count),
        open_item_count,
        documents: docs,
        archive,
    })
}

fn parameters(tree: &Tree<'_>) -> Result<ParameterReport, String> {
    let mut documents = Vec::new();
    for rel in &tree.rels {
        if hygiene::is_vintage_document(rel) {
            documents.push((rel.clone(), tree.text(rel)?));
        }
    }
    let (violations, vintages) = hygiene::parse_vintages(&documents);
    if !violations.is_empty() {
        return Err(format!(
            "params/ does not pass the loader, so no report is generated:\n  {}",
            violations.join("\n  ")
        ));
    }
    let lock = lock(tree)?;
    let mut reports = Vec::new();
    for (name, vintage) in &vintages {
        reports.push(vintage_report(tree, name, vintage, &lock)?);
    }
    let note = if reports.is_empty() {
        "No vintage exists under params/vintages/.".to_owned()
    } else {
        reports
            .iter()
            .map(|v| {
                format!(
                    "{} is {} and {}",
                    v.name,
                    if v.locked { "locked" } else { "unlocked" },
                    if v.verified {
                        "verified"
                    } else {
                        "pending human verification"
                    }
                )
            })
            .collect::<Vec<_>>()
            .join("; ")
            + "."
    };
    let state = if reports.is_empty() {
        SectionState::NotYetIntroduced
    } else {
        SectionState::Present
    };
    Ok(ParameterReport {
        status: status(
            state,
            "M0",
            "PLAN.md §4.1; ARCHITECTURE.md §6; TESTING.md §11.2 gate 9 and §12",
            note,
        ),
        vintages: reports,
        lock,
        provenance: provenance(tree)?,
    })
}

// ---------------------------------------------------------------------------
// Properties, snapshots, security, performance, inventory
// ---------------------------------------------------------------------------

/// The region of `text` inside the braces of the first `proptest! {` block. The
/// macro name followed by anything but a brace (a word in a comment, say) is not
/// a block.
fn proptest_block(text: &str) -> Option<&str> {
    let mut search = 0;
    let open = loop {
        let at = search + text[search..].find("proptest!")?;
        let after = text[at + "proptest!".len()..].trim_start();
        if after.starts_with('{') {
            break text.len() - after.len();
        }
        search = at + "proptest!".len();
    };
    let mut depth = 0_usize;
    for (i, ch) in text[open..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&text[open + 1..open + i]);
                }
            }
            _ => {}
        }
    }
    None
}

/// `fn name(` and `fn name<` identifiers, in order.
fn fn_names(block: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = block;
    while let Some(at) = rest.find("fn ") {
        let after = &rest[at + 3..];
        let name: String = after
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
            .collect();
        let end = after[name.len()..].trim_start();
        if !name.is_empty() && (end.starts_with('(') || end.starts_with('<')) {
            out.push(name);
        }
        rest = after;
    }
    out
}

/// The integer after the first `cases:` in `text`.
fn cases_in(text: &str) -> Option<u32> {
    let at = text.find("cases:")?;
    let digits: String = text[at + 6..]
        .trim_start()
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '_')
        .filter(|c| *c != '_')
        .collect();
    digits.parse().ok()
}

fn properties(tree: &Tree<'_>, testing: &str) -> Result<PropertyReport, String> {
    let mut files = Vec::new();
    for dir in tree.crate_dirs() {
        let prefix = format!("{dir}/");
        for rel in tree.under(&prefix) {
            if !ext_is(rel, "rs") {
                continue;
            }
            let text = tree.text(rel)?;
            let Some(block) = proptest_block(&text) else {
                continue;
            };
            files.push(PropertyFile {
                crate_name: dir.rsplit('/').next().unwrap_or(&dir).to_owned(),
                path: rel.clone(),
                case_count: cases_in(&text),
                property_names: fn_names(block),
            });
        }
    }
    let property_count = count(files.iter().map(|f| f.property_names.len()).sum());
    let invariants: Vec<Invariant> = docs::table_rows(&docs::section(testing, "## 4.")?)
        .into_iter()
        .filter(|row| row.len() >= 5 && row[0].starts_with('I'))
        .map(|row| Invariant {
            id: row[0].clone(),
            name: row[1].clone(),
            crate_name: row[row.len() - 2].clone(),
            milestone: docs::milestone_in(&row[row.len() - 1]),
        })
        .collect();
    let due: Vec<String> = invariants
        .iter()
        .filter(|i| i.milestone == "M0")
        .map(|i| i.id.clone())
        .collect();
    let state = if files.is_empty() {
        SectionState::NotYetIntroduced
    } else {
        SectionState::Partial
    };
    Ok(PropertyReport {
        status: status(
            state,
            "M0",
            "TESTING.md §4; ADR-022",
            format!(
                "Counted from source: {} file(s) with a proptest! block declaring {property_count} \
property function(s), with the case budget each file's ProptestConfig states. Which of the {} \
invariants of TESTING.md §4 a function asserts is not recorded in the source, so none is counted \
as covered by id; the {} due at M0 ({}) are listed for a person to map. Nothing here is a run \
result.",
                files.len(),
                invariants.len(),
                due.len(),
                if due.is_empty() {
                    "none".to_owned()
                } else {
                    due.join(", ")
                }
            ),
        ),
        files,
        property_count,
        invariants,
        invariants_due_at_m0: due,
    })
}

fn snapshots(tree: &Tree<'_>) -> SnapshotReport {
    let mut by_crate: BTreeMap<String, u32> = BTreeMap::new();
    for rel in tree.under("crates/") {
        if ext_is(rel, "snap") && rel.contains("/tests/snapshots/") {
            if let Some(name) = rel
                .strip_prefix("crates/")
                .and_then(|r| r.split('/').next())
            {
                bump(&mut by_crate, name);
            }
        }
    }
    let file_count = by_crate.values().sum();
    let state = if file_count == 0 {
        SectionState::NotYetIntroduced
    } else {
        SectionState::Present
    };
    SnapshotReport {
        status: status(
            state,
            "M0",
            "TESTING.md §9 item 1 and §3.6 (contracts); §1.1 places the contract snapshots at M0",
            "insta snapshot files under crates/*/tests/snapshots/: the OpenAPI document, the exact \
response-header set and the golden API bodies the front-end tests render. These are contract \
snapshots, not tier-3 goldens; a change to one is reviewed as an API change."
                .to_owned(),
        ),
        by_crate: counts(&by_crate),
        file_count,
    }
}

fn security(tree: &Tree<'_>, testing: &str) -> Result<SecurityReport, String> {
    let areas: Vec<SecurityArea> = docs::table_rows(&docs::section(testing, "## 8.")?)
        .into_iter()
        .filter(|row| row.len() >= 2)
        .map(|row| {
            let area = row[0]
                .split_once(" (")
                .map_or(row[0].clone(), |(a, _)| a.to_owned());
            SecurityArea {
                milestone: docs::milestone_in(&row[0]),
                area,
            }
        })
        .collect();
    let mut ids = BTreeSet::new();
    for rel in &tree.rels {
        let is_test_source =
            (rel.starts_with("crates/") && rel.contains("/tests/") && ext_is(rel, "rs"))
                || (rel.starts_with("web/tests/") && ext_is(rel, "mjs"));
        if !is_test_source {
            continue;
        }
        let text = tree.text(rel)?;
        let bytes = text.as_bytes();
        for (i, window) in bytes.windows(4).enumerate() {
            if window[0] == b'S'
                && window[1] == b'-'
                && window[2].is_ascii_digit()
                && window[3].is_ascii_digit()
                && (i == 0 || !bytes[i - 1].is_ascii_alphanumeric())
                && !bytes.get(i + 4).is_some_and(u8::is_ascii_alphanumeric)
            {
                ids.insert(String::from_utf8_lossy(window).into_owned());
            }
        }
    }
    let m0_areas: Vec<&str> = areas
        .iter()
        .filter(|a| a.milestone == "M0")
        .map(|a| a.area.as_str())
        .collect();
    Ok(SecurityReport {
        status: status(
            SectionState::Partial,
            "M0",
            "TESTING.md §8; SECURITY.md §15",
            format!(
                "The M0 areas ({}) are asserted in part by unit and integration tests that name \
these test ids in their source; the browser rows (storage, CSP violations, third-party \
requests, the trust flow) and the release rows need a browser and a signed artifact, neither of \
which exists yet. Ids are counted from test sources, not from a run.",
                if m0_areas.is_empty() {
                    "none".to_owned()
                } else {
                    m0_areas.join("; ")
                }
            ),
        ),
        areas,
        test_ids_named: ids.into_iter().collect(),
    })
}

fn performance(tree: &Tree<'_>, testing: &str) -> Result<PerformanceReport, String> {
    let budgets: Vec<PerformanceBudget> = docs::table_rows(&docs::section(testing, "## 10.")?)
        .into_iter()
        .filter(|row| row.len() >= 4)
        .map(|row| PerformanceBudget {
            budget: row[0].clone(),
            threshold: row[1].clone(),
            milestone: docs::milestone_in(&row[row.len() - 1]),
        })
        .collect();
    let benches = tree
        .rels
        .iter()
        .any(|r| r.starts_with("crates/") && r.contains("/benches/"));
    Ok(PerformanceReport {
        status: status(
            if benches {
                SectionState::Partial
            } else {
                SectionState::NotYetIntroduced
            },
            "M0",
            "TESTING.md §10; ARCHITECTURE.md §4.3",
            if benches {
                "Benchmarks exist; no measurement is recorded in this report.".to_owned()
            } else {
                "No criterion bench and no Playwright cold-start measurement exists at this commit. \
The M0 budget (cold start under 2 s) is specified but not measured; the tax-kernel probe gates \
from M1. The budgets are listed as the design states them."
                    .to_owned()
            },
        ),
        budgets,
    })
}

fn test_inventory(tree: &Tree<'_>) -> Result<TestInventory, String> {
    let mut by_crate: BTreeMap<String, u32> = BTreeMap::new();
    for dir in tree.crate_dirs() {
        let prefix = format!("{dir}/");
        let mut n = 0_usize;
        for rel in tree.under(&prefix) {
            if ext_is(rel, "rs") {
                let text = tree.text(rel)?;
                n += text.matches("#[test]").count() + text.matches("#[tokio::test").count();
            }
        }
        by_crate.insert(dir.rsplit('/').next().unwrap_or(&dir).to_owned(), count(n));
    }
    let mut web = 0_usize;
    for rel in tree.under("web/tests/") {
        if rel.ends_with(".test.mjs") {
            web += tree
                .text(rel)?
                .lines()
                .filter(|l| l.starts_with("test("))
                .count();
        }
    }
    Ok(TestInventory {
        by_crate: counts(&by_crate),
        web_test_count: count(web),
        note: "Test attributes and `test(` declarations counted in the source tree: what is declared, \
not what ran or passed. A property function counts once whatever its case budget."
            .to_owned(),
    })
}

// ---------------------------------------------------------------------------
// Sections that do not exist yet
// ---------------------------------------------------------------------------

fn tier2_status(tree: &Tree<'_>) -> SectionStatus {
    let n = tree.under("fixtures/tier2/").count();
    if n == 0 {
        status(
            SectionState::NotYetIntroduced,
            "M1",
            "TESTING.md §2.1 and §3.5; §1.1",
            "No suite has been transliterated; there is no skip count and no baseline for §11.2 gate 3."
                .to_owned(),
        )
    } else {
        status(
            SectionState::Present,
            "M1",
            "TESTING.md §2.1 and §3.5",
            format!("{n} file(s) under fixtures/tier2/. Skip counts need the runner, which this report does not have."),
        )
    }
}

fn tier3_status(tree: &Tree<'_>) -> SectionStatus {
    let n = tree.under("fixtures/tier3/snapshots/").count();
    if n == 0 {
        status(
            SectionState::NotYetIntroduced,
            "M3",
            "TESTING.md §2.1 and §3.6; §1.1 (persona goldens at M3, migrations from M2)",
            "No tier-3 golden exists; the contract snapshots counted above are not goldens."
                .to_owned(),
        )
    } else {
        status(
            SectionState::Present,
            "M3",
            "TESTING.md §2.1 and §3.6",
            format!("{n} snapshot file(s) under fixtures/tier3/snapshots/."),
        )
    }
}

fn oracles_status(tree: &Tree<'_>) -> SectionStatus {
    let recorded = tree.under("fixtures/tier3/oracle/").count();
    if recorded == 0 && !tree.has_dir("oracles") {
        status(
            SectionState::NotYetIntroduced,
            "M1",
            "TESTING.md §7 and §3.6; PLAN.md §4.13 item 8 (oracle versions from M1)",
            "No oracle project exists under oracles/ and nothing is recorded under fixtures/tier3/oracle/; \
there is no oracle version, lockfile hash or recording to report."
                .to_owned(),
        )
    } else {
        status(
            SectionState::Partial,
            "M1",
            "TESTING.md §7 and §3.6",
            format!("{recorded} recorded file(s) under fixtures/tier3/oracle/."),
        )
    }
}

fn mutation_status(tree: &Tree<'_>) -> SectionStatus {
    if tree.root.join("xtask/mutants.toml").is_file() {
        status(
            SectionState::Partial,
            "M1",
            "TESTING.md §11.3",
            "xtask/mutants.toml exists; the surviving-mutant count needs a cargo-mutants run, which this report does not have."
                .to_owned(),
        )
    } else {
        status(
            SectionState::NotYetIntroduced,
            "M1",
            "TESTING.md §11.3; PLAN.md §4.13 item 1 (cargo-mutants introduced at M1, budgets set at M1 exit)",
            "No budget is declared (xtask/mutants.toml does not exist) and no mutation run has happened."
                .to_owned(),
        )
    }
}

fn fuzz_status(tree: &Tree<'_>) -> SectionStatus {
    let exists = tree.has_dir("fuzz")
        || tree
            .rels
            .iter()
            .any(|r| r.starts_with("crates/") && r.contains("/fuzz/"));
    if exists {
        status(
            SectionState::Partial,
            "M4",
            "TESTING.md §11.3 and §8 (parsers)",
            "A fuzz target exists; CPU-hours and crash counts need a run, which this report does not have."
                .to_owned(),
        )
    } else {
        status(
            SectionState::NotYetIntroduced,
            "M4",
            "TESTING.md §11.3 and §8; §1.1 (parser fuzzing at M4; the vault header fuzz at M1)",
            "No fuzz target and no committed corpus exists: the parsers and the container they would fuzz do not exist yet."
                .to_owned(),
        )
    }
}

fn browser_status(tree: &Tree<'_>) -> SectionStatus {
    let exists = tree
        .rels
        .iter()
        .any(|r| r.to_ascii_lowercase().contains("playwright") || r.starts_with("web/e2e/"));
    if exists {
        status(
            SectionState::Partial,
            "M0",
            "TESTING.md §9",
            "A browser suite exists; its results are not part of this report.".to_owned(),
        )
    } else {
        status(
            SectionState::NotYetIntroduced,
            "M0",
            "TESTING.md §9 and §8 (browser rows); PLAN.md §4.1",
            "Specified for M0 (Playwright against the real binary) and not yet introduced: no browser \
suite exists, so the storage, CSP-violation, third-party-request, trust-flow, axe and cold-start \
assertions have not been made by any test."
                .to_owned(),
        )
    }
}

// ---------------------------------------------------------------------------
// Unverified, refused, pins
// ---------------------------------------------------------------------------

fn unverified(
    testing: &str,
    fixtures: &FixtureReport,
    parameters: &ParameterReport,
) -> Result<UnverifiedReport, String> {
    let items: Vec<UnverifiedItem> = docs::table_rows(&docs::section(testing, "### 5.2")?)
        .into_iter()
        .filter(|row| row.len() >= 2)
        .map(|row| UnverifiedItem {
            item: row[0].clone(),
            milestone: docs::milestone_in(&row[1]),
            gate: row[1].clone(),
        })
        .collect();
    let tier1 = fixtures.tiers.iter().find(|t| t.tier == "tier1");
    let tier1_count = |value: &str| {
        tier1
            .and_then(|t| t.by_verification.iter().find(|c| c.name == value))
            .map_or(0, |c| c.count)
    };
    let pending = fixtures
        .tiers
        .iter()
        .find(|t| t.tier == "pending")
        .map_or(0, |t| t.file_count);
    let mut parameter_open_items = Vec::new();
    let mut pending_documents = 0_u32;
    for vintage in &parameters.vintages {
        for document in &vintage.documents {
            if document.verification != "primary-source-confirmed"
                && document.verification != "hand-worked-reviewed"
            {
                pending_documents += 1;
            }
            for item in &document.open_items {
                parameter_open_items.push(OpenItem {
                    document: document.id.clone(),
                    item: item.clone(),
                });
            }
        }
    }
    Ok(UnverifiedReport {
        tier1_primary_source_confirmed_count: tier1_count("primary-source-confirmed"),
        tier1_hand_worked_reviewed_count: tier1_count("hand-worked-reviewed"),
        pending_fixture_count: pending,
        pending_parameter_document_count: pending_documents,
        items,
        parameter_open_items,
    })
}

fn refused(testing: &str) -> Result<Vec<RefusedTarget>, String> {
    Ok(docs::table_rows(&docs::section(testing, "### 5.3")?)
        .into_iter()
        .filter(|row| row.len() >= 2)
        .map(|row| RefusedTarget {
            target: row[0].clone(),
            reason: row[1].clone(),
        })
        .collect())
}

fn pins(parameters: &ParameterReport) -> Pins {
    Pins {
        application_version: env!("CARGO_PKG_VERSION").to_owned(),
        licence: env!("CARGO_PKG_LICENSE").to_owned(),
        engine_version: None,
        schema_version: None,
        param_vintage_content_ids: parameters
            .vintages
            .iter()
            .map(|v| v.content_id.clone())
            .collect(),
        locked_param_vintage_ids: parameters
            .vintages
            .iter()
            .filter_map(|v| v.locked_id.clone())
            .collect(),
        binary_digest: None,
        note:
            "No engineVersion constant exists yet (ADR-010: the first stored result arrives with \
the plan file, M1-M2); the plan schema freezes at M2 (seam S5); a binary digest needs the release \
pipeline (M0 signing block). A content id claims nothing about verification or immutability; only \
a locked id may pin a result, and no vintage is locked."
                .to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn envelopes_are_read_as_declared_and_never_upgraded() {
        // SYNTHETIC envelope.
        let text = r#"{"id":"pending/x","tier":"pending","module":"pfp-money","milestone":"M0",
            "verification":"pending-hand-verification","synthetic":true,"promotesTo":"t1/x",
            "paramVintage":null,"expect":{"cases":[{},{},{}]}}"#;
        let file = envelope("fixtures/pending/x.json", text).unwrap();
        assert_eq!(file.verification, "pending-hand-verification");
        assert_eq!(file.case_count, 3);
        assert_eq!(file.promotes_to.as_deref(), Some("t1/x"));
        assert_eq!(file.param_vintage, None);
        assert_eq!(file.review_kind, None);

        let bare = envelope("fixtures/tier1/y.json", r#"{"expect":{"lines":[{}]}}"#).unwrap();
        assert_eq!(bare.verification, "unstated");
        assert_eq!(bare.case_count, 1);
        assert!(envelope("fixtures/tier1/z.json", "[]").is_err());
        assert!(envelope("fixtures/tier1/z.json", "{").is_err());

        let reviewed = envelope(
            "fixtures/tier1/r.json",
            r#"{"verification":"hand-worked-reviewed","reviewedBy":{"kind":"cooling-off-re-review"}}"#,
        )
        .unwrap();
        assert_eq!(
            reviewed.review_kind.as_deref(),
            Some("cooling-off-re-review")
        );
    }

    #[test]
    fn property_functions_and_case_budgets_are_read_from_source() {
        let text = "fn config() -> ProptestConfig { ProptestConfig { cases: 2_048, ..Default::default() } }\n\
                    proptest! {\n  #![proptest_config(config())]\n  #[test]\n  fn a_holds(x in 0..1) { let f = |y: i32| y; }\n\
                    fn b_holds<T>(x in 0..1) {}\n}\nfn not_a_property() {}\n";
        assert_eq!(cases_in(text), Some(2048));
        let block = proptest_block(text).unwrap();
        assert_eq!(fn_names(block), ["a_holds", "b_holds"]);
        assert_eq!(cases_in("no budget here"), None);
        assert!(proptest_block("nothing").is_none());
    }

    #[test]
    fn verification_counts_read_worst_first() {
        let mut map = BTreeMap::new();
        bump(&mut map, "primary-source-confirmed");
        bump(&mut map, "pending-hand-verification");
        bump(&mut map, "pending-hand-verification");
        bump(&mut map, "something-else");
        let counted = verification_counts(&map);
        let keys: Vec<&str> = counted.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(
            keys,
            [
                "pending-hand-verification",
                "primary-source-confirmed",
                "something-else"
            ]
        );
        assert_eq!(verification_counts(&map)[0].count, 2);
    }

    #[test]
    fn the_report_is_a_function_of_the_tree() {
        let root = repo::root();
        let first = build(&root, None).unwrap();
        let second = build(&root, None).unwrap();
        assert_eq!(first, second);
        let json = serde_json::to_string(&first).unwrap();
        let back: ValidationReport = serde_json::from_str(&json).unwrap();
        assert_eq!(back, first);
        // Nothing machine- or person-specific is in it.
        let home = std::env::var("HOME").unwrap_or_default();
        assert!(home.is_empty() || !json.contains(&home));
        assert!(!json.contains(&root.display().to_string()));
        // The four tiers are always listed, in order.
        let tiers: Vec<&str> = first
            .fixtures
            .tiers
            .iter()
            .map(|t| t.tier.as_str())
            .collect();
        assert_eq!(tiers, ["tier1", "tier2", "tier3", "pending"]);
        // The design's tables were found.
        assert!(!first.unverified.items.is_empty());
        assert!(!first.refused.is_empty());
        assert!(first.properties.invariants.len() > 20);
        assert!(first.performance_budgets.budgets.len() >= 5);
        assert!(first.security_suite.areas.len() >= 5);
    }
}
