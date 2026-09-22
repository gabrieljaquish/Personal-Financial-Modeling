//! Golden API bodies for the front-end tests (`web/tests/support/golden.mjs` reads
//! these snapshots). The views are therefore rendered from **what the server really
//! sends**: a DTO change that would break a screen fails a web test, not a user
//! session.
//!
//! JSON bodies are recorded pretty-printed with sorted keys so a diff is
//! reviewable; every map the API serves is a sorted map already, and no consumer
//! depends on key order. The two exports are recorded as sent, with each carriage
//! return drawn as `␍` because a snapshot file does not keep one.
//!
//! They are insta snapshots rather than `.json` files because the repository's
//! data-hygiene gate reserves JSON documents for `fixtures/` and `params/`.
//!
//! The input is SYNTHETIC: the first grid point of `docs/PLAN.md` §4.1.

// Not an engine crate: tests here legitimately touch sockets, the clock and files.
#![allow(
    clippy::disallowed_types,
    clippy::disallowed_methods,
    clippy::disallowed_macros
)]

mod support;

use serde_json::Value;
use support::{HttpServer, Resp};

const INPUT: &str = r#"{"year":2026,"filingStatus":"mfj","taxableIncome":10000000}"#;
const INPUT_CSV: &str =
    r#"{"year":2026,"filingStatus":"mfj","taxableIncome":10000000,"format":"csv"}"#;
const INPUT_JSON: &str =
    r#"{"year":2026,"filingStatus":"mfj","taxableIncome":10000000,"format":"json"}"#;

fn pretty(response: &Resp) -> String {
    assert_eq!(response.status, 200);
    let value: Value = response.json();
    serde_json::to_string_pretty(&value).expect("JSON")
}

fn text(response: &Resp) -> String {
    assert_eq!(response.status, 200);
    String::from_utf8(response.body.clone()).expect("UTF-8")
}

#[tokio::test]
async fn golden_bodies() {
    let server = HttpServer::start();
    let session = server.establish().await;

    let status = server
        .send(server.authed("/api/v1/session/status", &session))
        .await;
    let mut status_value: Value = status.json();
    // The one run-specific value: the crate version. Everything else is content.
    status_value["appVersion"] = Value::String("0.0.0-golden".to_owned());
    insta::assert_snapshot!(
        "ui_golden_session_status",
        serde_json::to_string_pretty(&status_value).expect("JSON")
    );

    let assumptions = server
        .send(server.authed("/api/v1/assumptions/list", &session))
        .await;
    insta::assert_snapshot!("ui_golden_assumptions_list", pretty(&assumptions));

    let schedule = server
        .send(
            server
                .authed("/api/v1/tax/rate-schedule", &session)
                .json(INPUT),
        )
        .await;
    assert_eq!(schedule.json()["tax"], 1_150_400);
    insta::assert_snapshot!("ui_golden_rate_schedule", pretty(&schedule));

    let csv = server
        .send(
            server
                .authed("/api/v1/tax/rate-schedule/export", &session)
                .json(INPUT_CSV),
        )
        .await;
    let csv = text(&csv);
    assert!(!csv.contains('␍'));
    insta::assert_snapshot!("ui_golden_rate_schedule_csv", csv.replace('\r', "␍"));

    let exported = server
        .send(
            server
                .authed("/api/v1/tax/rate-schedule/export", &session)
                .json(INPUT_JSON),
        )
        .await;
    insta::assert_snapshot!("ui_golden_rate_schedule_json", text(&exported));

    server.stop().await;
}

/// A SYNTHETIC validation report in the shape of a **future** tree: promoted
/// tier-1 fixtures of both admissible kinds, one still pending, a locked and
/// verified vintage, and corpora in every state. The front-end tests render the
/// About page from this golden and from the M0 shape they derive from it, so a
/// DTO change that would break the page fails a web test; and nothing in the
/// page can be hard-coded to today's zeros, because this shape has none.
///
/// It is built here, not recorded from the server: the report a server carries
/// is whatever `cargo xtask validation-report` generated for the tree, which
/// differs between a checkout that ran it and one that did not.
#[allow(clippy::too_many_lines)]
fn synthetic_future_report() -> pfp_server::api::report_dto::ValidationReport {
    use pfp_server::api::report_dto::{
        ArchiveCheck, CountByKey, FixtureFile, FixtureGroup, FixtureReport, InvalidFixture,
        Invariant, LockReport, OpenItem, ParameterReport, PerformanceBudget, PerformanceReport,
        Pins, PropertyFile, PropertyReport, ProvenanceReport, RefusedTarget, SectionState,
        SectionStatus, SecurityArea, SecurityReport, SnapshotReport, TableReport, TestInventory,
        TierReport, UnverifiedItem, UnverifiedReport, ValidationReport, VintageReport,
    };
    let status = |state: SectionState, milestone: &str, note: &str| SectionStatus {
        state,
        milestone: milestone.to_owned(),
        reference: "TESTING.md (synthetic golden)".to_owned(),
        note: note.to_owned(),
    };
    let key = |k: &str, n: u32| CountByKey {
        name: k.to_owned(),
        count: n,
    };
    let archive = |n: u32| ArchiveCheck {
        source_count: n,
        archived_count: n,
        checksum_match_count: n,
        checksum_mismatch_count: 0,
        missing_count: 0,
    };
    let file = |path: &str, id: &str, verification: &str, review: Option<&str>| FixtureFile {
        path: path.to_owned(),
        id: Some(id.to_owned()),
        module: Some("pfp-money".to_owned()),
        milestone: Some("M0".to_owned()),
        verification: verification.to_owned(),
        synthetic: Some(review.is_some()),
        promotes_to: None,
        param_vintage: Some("federal-2026@synthetic".to_owned()),
        review_kind: review.map(str::to_owned),
        case_count: 4,
    };
    let tier = |name: &str, state: SectionState, files: Vec<FixtureFile>| {
        let mut by_verification = std::collections::BTreeMap::new();
        let mut review_kinds = std::collections::BTreeMap::new();
        for f in &files {
            *by_verification.entry(f.verification.clone()).or_insert(0) += 1;
            if let Some(k) = &f.review_kind {
                *review_kinds.entry(k.clone()).or_insert(0) += 1;
            }
        }
        TierReport {
            tier: name.to_owned(),
            directory: format!("fixtures/{name}"),
            status: status(state, "M0", "synthetic"),
            file_count: u32::try_from(files.len()).unwrap(),
            by_milestone: vec![key("M0", u32::try_from(files.len()).unwrap())],
            by_verification: by_verification.iter().map(|(k, n)| key(k, *n)).collect(),
            by_module: vec![key("pfp-money", u32::try_from(files.len()).unwrap())],
            by_milestone_and_verification: by_verification
                .iter()
                .map(|(k, n)| FixtureGroup {
                    milestone: "M0".to_owned(),
                    verification: k.clone(),
                    count: *n,
                })
                .collect(),
            review_kinds: review_kinds.iter().map(|(k, n)| key(k, *n)).collect(),
            files,
        }
    };
    ValidationReport {
        generated_on: Some("2001-02-03".to_owned()),
        basis: "SYNTHETIC golden for the front-end tests; describes no real tree.".to_owned(),
        fixtures: FixtureReport {
            status: status(SectionState::Partial, "M0", "synthetic"),
            tiers: vec![
                tier(
                    "tier1",
                    SectionState::Present,
                    vec![
                        file(
                            "fixtures/tier1/money/a.json",
                            "t1/money/a",
                            "primary-source-confirmed",
                            None,
                        ),
                        file(
                            "fixtures/tier1/money/b.json",
                            "t1/money/b",
                            "primary-source-confirmed",
                            None,
                        ),
                        file(
                            "fixtures/tier1/ledger/c.json",
                            "t1/ledger/c",
                            "hand-worked-reviewed",
                            Some("cooling-off-re-review"),
                        ),
                    ],
                ),
                tier("tier2", SectionState::NotYetIntroduced, vec![]),
                tier("tier3", SectionState::NotYetIntroduced, vec![]),
                tier("personas", SectionState::NotYetIntroduced, vec![]),
                // One readable plan fixture and, below, one the generator could
                // not read: the page must show both, so the columns add up.
                {
                    let mut plans = tier(
                        "plans",
                        SectionState::Present,
                        vec![file(
                            "fixtures/plans/demo.plan.json",
                            "plans/demo",
                            "unstated",
                            Some("generated"),
                        )],
                    );
                    plans.file_count = 2;
                    plans
                },
                tier(
                    "pending",
                    SectionState::Present,
                    vec![file(
                        "fixtures/pending/money/d.json",
                        "pending/money/d",
                        "pending-hand-verification",
                        None,
                    )],
                ),
            ],
            invalid: vec![InvalidFixture {
                path: "fixtures/plans/broken.json".to_owned(),
                problem: "not JSON: synthetic parse error".to_owned(),
            }],
            other_directories: vec![key("fixtures/scratch", 1)],
        },
        parameters: ParameterReport {
            status: status(
                SectionState::Present,
                "M0",
                "federal-2026 is locked and verified.",
            ),
            vintages: vec![VintageReport {
                name: "federal-2026".to_owned(),
                content_id: "federal-2026@synthetic".to_owned(),
                locked: true,
                locked_id: Some("federal-2026@synthetic".to_owned()),
                verified: true,
                verification: vec![key("primary-source-confirmed", 3)],
                table_count: 2,
                index_series_count: 1,
                open_item_count: 0,
                documents: vec![
                    TableReport {
                        id: "irs.ordinary_brackets".to_owned(),
                        kind: "table".to_owned(),
                        verification: "primary-source-confirmed".to_owned(),
                        open_item_count: 0,
                        open_items: vec![],
                        archive: archive(4),
                    },
                    TableReport {
                        id: "bls.cpi.chained.suur0000sa0".to_owned(),
                        kind: "index-series".to_owned(),
                        verification: "primary-source-confirmed".to_owned(),
                        open_item_count: 0,
                        open_items: vec![],
                        archive: archive(6),
                    },
                ],
                archive: archive(10),
            }],
            lock: LockReport {
                present: true,
                entry_count: 3,
                locked_vintages: vec!["params/vintages/federal-2026/".to_owned()],
                unverified_vintages: vec![],
                checksum_mismatch_count: 0,
                missing_count: 0,
                unlisted_count: 0,
                note: "Every entry verifies (synthetic).".to_owned(),
            },
            provenance: ProvenanceReport {
                file_count: 23,
                catalogued_count: 5,
                checksum_match_count: 5,
                checksum_mismatch_count: 0,
                missing_count: 0,
                verification: Some("primary-source-confirmed".to_owned()),
            },
        },
        properties: PropertyReport {
            status: status(SectionState::Partial, "M0", "synthetic"),
            files: vec![PropertyFile {
                crate_name: "pfp-money".to_owned(),
                path: "crates/pfp-money/tests/properties.rs".to_owned(),
                case_count: Some(2048),
                property_names: vec!["rounding_is_idempotent".to_owned()],
            }],
            property_count: 1,
            invariants: vec![Invariant {
                id: "I7".to_owned(),
                name: "Uprating idempotent and path-independent".to_owned(),
                crate_name: "pfp-params".to_owned(),
                milestone: "M0".to_owned(),
            }],
            invariants_due_at_m0: vec!["I7".to_owned()],
        },
        contract_snapshots: SnapshotReport {
            status: status(SectionState::Present, "M0", "synthetic"),
            by_crate: vec![key("pfp-server", 11)],
            file_count: 11,
        },
        security_suite: SecurityReport {
            status: status(SectionState::Partial, "M0", "synthetic"),
            areas: vec![SecurityArea {
                area: "Server and browser".to_owned(),
                milestone: "M0".to_owned(),
            }],
            test_ids_named: vec!["S-01".to_owned(), "S-02".to_owned()],
        },
        tier2_suites: status(
            SectionState::NotYetIntroduced,
            "M1",
            "No suite has been transliterated.",
        ),
        tier3_goldens: status(SectionState::NotYetIntroduced, "M3", "No golden exists."),
        oracles: status(SectionState::NotYetIntroduced, "M1", "No oracle exists."),
        mutation_score: status(
            SectionState::NotYetIntroduced,
            "M1",
            "No budget is declared.",
        ),
        fuzz_corpora: status(
            SectionState::NotYetIntroduced,
            "M4",
            "No fuzz target exists.",
        ),
        browser_end_to_end: status(
            SectionState::Partial,
            "M0",
            "A browser suite exists (synthetic).",
        ),
        performance_budgets: PerformanceReport {
            status: status(SectionState::NotYetIntroduced, "M0", "No bench exists."),
            budgets: vec![PerformanceBudget {
                budget: "Cold start".to_owned(),
                threshold: "Launch to browser under 2 s".to_owned(),
                milestone: "M0".to_owned(),
            }],
        },
        test_inventory: TestInventory {
            by_crate: vec![key("pfp-money", 43)],
            web_test_count: 184,
            note: "Declared, not run (synthetic).".to_owned(),
        },
        unverified: UnverifiedReport {
            tier1_primary_source_confirmed_count: 2,
            tier1_hand_worked_reviewed_count: 1,
            pending_fixture_count: 1,
            pending_parameter_document_count: 0,
            items: vec![UnverifiedItem {
                item: "A synthetic open item".to_owned(),
                gate: "M1, before a synthetic lock".to_owned(),
                milestone: "M1".to_owned(),
            }],
            parameter_open_items: vec![OpenItem {
                document: "irs.ordinary_brackets".to_owned(),
                item: "A synthetic open question".to_owned(),
            }],
        },
        refused: vec![RefusedTarget {
            target: "A synthetic folklore number".to_owned(),
            reason: "Appears nowhere in its cited source (synthetic).".to_owned(),
        }],
        pins: Pins {
            application_version: "0.0.0-golden".to_owned(),
            licence: "Apache-2.0".to_owned(),
            engine_version: None,
            schema_version: None,
            param_vintage_content_ids: vec!["federal-2026@synthetic".to_owned()],
            locked_param_vintage_ids: vec!["federal-2026@synthetic".to_owned()],
            binary_digest: None,
            note: "Synthetic.".to_owned(),
        },
    }
}

#[test]
fn golden_validation_report_future_shape() {
    let report = synthetic_future_report();
    // The golden is the wire form the endpoint would send for this report.
    let body = pfp_server::api::dto::ValidationReportResponse {
        state: pfp_server::api::dto::ReportStateDto::Generated,
        report: Some(report.clone()),
    };
    let value = serde_json::to_value(&body).expect("JSON");
    // Round trip through the reader: the golden is a report this build accepts.
    let back = pfp_server::validation::parse(&value["report"].to_string()).expect("parses");
    assert_eq!(back, report);
    insta::assert_snapshot!(
        "ui_golden_validation_report_future",
        serde_json::to_string_pretty(&value).expect("JSON")
    );
}
