//! `validation-report`: the validation report from whatever corpora exist
//! (`PLAN.md` §4.1 and §4.13 item 8; `TESTING.md` §13; ADR-022).
//!
//! ```text
//! cargo xtask validation-report [--generated-on YYYY-MM-DD]
//!     writes build/validation-report.json (git-ignored; pfp-server embeds it at
//!     build time) and docs/validation-report.md (committed)
//! cargo xtask validation-report --check
//!     recomputes the report and fails if docs/validation-report.md, or an existing
//!     build/validation-report.json, differs from it
//! ```
//!
//! The report is a pure function of the repository (`collect`): no clock, host,
//! path or person enters it, so `--check` is exact. A generation date appears only
//! when given on the command line. The model is the server's DTO file, compiled
//! in here by `#[path]` so that the writer and the reader share one shape.
//!
//! The JSON goes under `build/` rather than into the tree because the data-hygiene
//! gate reserves `.json` documents for `fixtures/` and `params/` (SECURITY.md
//! §13.3 rule 1) and `/build/` is git-ignored; the Markdown is the committed,
//! reviewable rendering. `pfp-server/build.rs` embeds the JSON exactly as it
//! embeds `web/dist`: absent, a release build fails and a debug build serves the
//! "report not generated" state.

#[path = "../../../crates/pfp-server/src/api/report_dto.rs"]
pub(crate) mod model;

pub(crate) mod collect;
pub(crate) mod docs;
pub(crate) mod markdown;

use std::fs;
use std::path::Path;

use crate::repo;

/// Where the JSON the server embeds is written, relative to the repository root.
pub(crate) const JSON_PATH: &str = "build/validation-report.json";
/// The committed Markdown rendering.
pub(crate) const MARKDOWN_PATH: &str = "docs/validation-report.md";

/// The report as JSON: keys sorted, pretty-printed, one trailing newline.
pub(crate) fn to_json(report: &model::ValidationReport) -> Result<String, String> {
    // Through `Value`, whose maps are `BTreeMap`s: every object's keys are sorted.
    let value = serde_json::to_value(report).map_err(|e| e.to_string())?;
    let mut text = serde_json::to_string_pretty(&value).map_err(|e| e.to_string())?;
    text.push('\n');
    Ok(text)
}

fn is_date(text: &str) -> bool {
    let b = text.as_bytes();
    b.len() == 10
        && b[4] == b'-'
        && b[7] == b'-'
        && b.iter()
            .enumerate()
            .all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit())
}

/// Reads the committed Markdown, if any.
fn committed(root: &Path, rel: &str) -> Result<Option<String>, String> {
    let path = root.join(rel);
    if !path.is_file() {
        return Ok(None);
    }
    Ok(Some(
        String::from_utf8_lossy(&repo::read(&path)?).into_owned(),
    ))
}

pub(crate) fn run(args: &[String]) -> Result<bool, String> {
    let mut check = false;
    let mut generated_on = None;
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--check" => check = true,
            "--generated-on" => {
                let date = it
                    .next()
                    .ok_or("validation-report: --generated-on needs YYYY-MM-DD")?;
                if !is_date(date) {
                    return Err("validation-report: --generated-on needs YYYY-MM-DD".to_owned());
                }
                generated_on = Some(date.clone());
            }
            other => return Err(format!("validation-report: unknown argument `{other}`")),
        }
    }
    let root = repo::root();
    let report = collect::build(&root, generated_on)?;
    let json = to_json(&report)?;
    let markdown = markdown::render(&report);
    eprintln!("validation-report: {}", markdown::headline(&report));

    if check {
        let mut stale = Vec::new();
        match committed(&root, MARKDOWN_PATH)? {
            Some(text) if text == markdown => {}
            Some(_) | None => stale.push(MARKDOWN_PATH),
        }
        match committed(&root, JSON_PATH)? {
            Some(text) if text == json => {}
            Some(_) => stale.push(JSON_PATH),
            // Not generated: the server serves the "report not generated" state
            // and says so; nothing to compare.
            None => eprintln!("validation-report: {JSON_PATH} is not generated (not checked)"),
        }
        if stale.is_empty() {
            eprintln!("validation-report: {MARKDOWN_PATH} is fresh");
            return Ok(true);
        }
        for path in &stale {
            eprintln!("validation-report: {path} differs from a fresh build (run `cargo xtask validation-report`)");
        }
        return Ok(false);
    }

    let json_path = root.join(JSON_PATH);
    if let Some(dir) = json_path.parent() {
        fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    fs::write(&json_path, json).map_err(|e| format!("{}: {e}", json_path.display()))?;
    let md_path = root.join(MARKDOWN_PATH);
    fs::write(&md_path, markdown).map_err(|e| format!("{}: {e}", md_path.display()))?;
    eprintln!("validation-report: wrote {JSON_PATH} and {MARKDOWN_PATH}");
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The drift test: the committed rendering is what a fresh build renders.
    #[test]
    fn the_committed_report_is_fresh() {
        let root = repo::root();
        let report = collect::build(&root, None).unwrap();
        let fresh = markdown::render(&report);
        let on_disk = committed(&root, MARKDOWN_PATH)
            .unwrap()
            .expect("docs/validation-report.md is committed");
        assert!(
            on_disk == fresh,
            "docs/validation-report.md is stale: run `cargo xtask validation-report`"
        );
        if let Some(json) = committed(&root, JSON_PATH).unwrap() {
            assert!(
                json == to_json(&report).unwrap(),
                "{JSON_PATH} is stale: run `cargo xtask validation-report`"
            );
        }
    }

    fn sorted(v: &serde_json::Value) -> bool {
        match v {
            serde_json::Value::Object(map) => {
                let keys: Vec<&String> = map.keys().collect();
                let mut s = keys.clone();
                s.sort();
                keys == s && map.values().all(sorted)
            }
            serde_json::Value::Array(items) => items.iter().all(sorted),
            _ => true,
        }
    }

    #[test]
    fn json_is_sorted_and_newline_terminated() {
        let report = collect::build(&repo::root(), Some("2001-02-03".to_owned())).unwrap();
        let json = to_json(&report).unwrap();
        assert!(json.ends_with("}\n") && !json.ends_with("\n\n"));
        assert!(json.contains("\"generatedOn\": \"2001-02-03\""));
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(sorted(&value));
        // Without a date the key is absent, not null.
        let undated = to_json(&collect::build(&repo::root(), None).unwrap()).unwrap();
        assert!(!undated.contains("generatedOn"));
    }

    #[test]
    fn the_headline_is_computed_not_typed() {
        let mut report = collect::build(&repo::root(), None).unwrap();
        let now = markdown::headline(&report);
        assert!(now.contains("tier-1 fixture"), "{now}");
        assert!(now.contains("pending human verification"), "{now}");
        // A future tree: promoted fixtures and a locked, verified vintage.
        report.fixtures.tiers[0].file_count = 3;
        report.unverified.pending_fixture_count = 1;
        for vintage in &mut report.parameters.vintages {
            vintage.locked = true;
            vintage.verified = true;
        }
        let future = markdown::headline(&report);
        assert!(
            future.starts_with("3 tier-1 fixtures; 1 fixture pending human verification"),
            "{future}"
        );
        assert!(future.contains("locked and verified"), "{future}");
        assert!(is_date("2026-09-22") && !is_date("2026-9-22") && !is_date("today"));
    }
}
