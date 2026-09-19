//! Drives `pfp-params` from the fixtures under `fixtures/pending/published/` and
//! `fixtures/pending/uprating/` (`TESTING.md` §3.1).
//!
//! The fixtures were written from the archived primary documents before this
//! crate existed and are **pending human verification**; this test reads them and
//! never writes them (`docs/contributing.md` §3.1). Where the fixture author
//! recorded a **residual** between the statutory pipeline and the publication,
//! the residual is asserted exactly as recorded: nothing here special-cases a
//! value to hide it, and a residual that moved would fail the test.

// Fixture drivers read top to bottom like the fixture they check.
#![allow(clippy::too_many_lines)]

use pfp_domain::FilingStatus;
use pfp_money::{Cents, Ratio, RoundingBasis, RoundingDirection, RoundingRule};
use pfp_params::shipped::{self, ORDINARY_BRACKETS, STD_DEDUCTION};
use pfp_params::{Origin, ParamView, Vintage};
use serde_json::Value;

const BRACKETS_2025: &str = include_str!("../../../fixtures/pending/published/brackets-2025.json");
const BRACKETS_2026: &str = include_str!("../../../fixtures/pending/published/brackets-2026.json");
const STDDED_2025: &str = include_str!("../../../fixtures/pending/published/stdded-2025.json");
const STDDED_2026: &str = include_str!("../../../fixtures/pending/published/stdded-2026.json");
const UP_BRACKETS_2026: &str =
    include_str!("../../../fixtures/pending/uprating/2026-brackets.json");
const UP_BRACKETS_2025: &str =
    include_str!("../../../fixtures/pending/uprating/2025-brackets-control.json");
const UP_STDDED_2026: &str = include_str!("../../../fixtures/pending/uprating/2026-stdded.json");
const CHAINED: &str =
    include_str!("../../../fixtures/pending/uprating/chained-path-divergence.json");

/// The fixtures name the six edges `top10 .. top35`, in the tables' `edges` order.
const EDGES: [&str; 6] = ["top10", "top12", "top22", "top24", "top32", "top35"];

fn vintage() -> Vintage {
    shipped::federal_2026().expect("the embedded vintage validates")
}

/// The `index_series` spelling `DOMAIN-MODEL.md` §15 prescribes. Stated here, not
/// imported: the crate no longer carries the name, the data does.
const CPI_CHAINED_AUG12M: &str = "cpi.chained.aug12m";

/// `projection.lag_years` as the table declares it; the uprating fixtures'
/// `numeratorCalendarYear` is what checks it.
fn lag_years(v: &Vintage, table: &str) -> i32 {
    v.table(table)
        .unwrap()
        .projection()
        .lag_years
        .expect("an indexed table declares its lag")
}

fn load(text: &str, id: &str) -> Value {
    let v: Value = serde_json::from_str(text).unwrap();
    assert_eq!(v["id"], id);
    // Not tier 1 and not locked: only a human who has read the primary document
    // may promote these (ADR-022).
    assert_eq!(v["tier"], "pending");
    assert_eq!(v["verification"], "pending-hand-verification");
    assert_eq!(v["tolerance"], serde_json::json!({"kind": "exact"}));
    assert_eq!(v["paramVintage"], Value::Null);
    v
}

fn int(v: &Value) -> i64 {
    v.as_i64().unwrap_or_else(|| panic!("not an integer: {v}"))
}

fn count(v: &Value) -> usize {
    usize::try_from(int(v)).unwrap()
}

fn dollars(v: &Value) -> Cents {
    Cents::from_dollars(int(v)).unwrap()
}

fn edge_index(v: &Value) -> usize {
    let name = v.as_str().unwrap();
    EDGES.iter().position(|e| *e == name).unwrap()
}

fn ratio_of(v: &Value) -> Ratio {
    Ratio::new(int(&v["num"]), int(&v["den"])).unwrap()
}

/// Structural equality of two exact rationals, by cross-multiplication.
fn same_value(a: Ratio, b: Ratio) -> bool {
    i128::from(a.num) * i128::from(b.den) == i128::from(b.num) * i128::from(a.den)
}

// ---------------------------------------------------------------------------
// (1) Published lookups
// ---------------------------------------------------------------------------

#[test]
fn the_embedded_vintage_is_unlocked_and_unverified() {
    let v = vintage();
    assert_eq!(v.name(), "federal-2026");
    assert_eq!(v.id(), None, "no vintage id exists until a human locks it");
    assert!(!v.is_verified());
    assert_eq!(v.tables().count(), 2);
}

#[test]
fn the_content_id_is_name_at_sha256_and_tracks_every_byte() {
    let v = vintage();
    let id = v.content_id();
    assert_eq!(id.name(), "federal-2026");
    assert_eq!(
        pfp_params::VintageId::parse(&id.to_string()).as_ref(),
        Some(id)
    );
    assert_eq!(vintage().content_id(), id, "deterministic");

    let [brackets, stdded] = shipped::federal_2026_tables();
    let series = shipped::federal_2026_series();
    // Table order is not content; the documents are hashed in id order.
    let swapped = Vintage::parse("federal-2026", &[stdded, brackets], &series).unwrap();
    assert_eq!(swapped.content_id(), id);
    // One byte of one document, even a comment, is a different vintage ...
    let edited = format!("{stdded}# edited\n");
    let other = Vintage::parse("federal-2026", &[brackets, &edited], &series).unwrap();
    assert_ne!(other.content_id().sha256(), id.sha256());
    // ... and so is the same content under another name.
    let renamed = Vintage::parse("federal-2027", &[brackets, stdded], &series).unwrap();
    assert_ne!(renamed.content_id().sha256(), id.sha256());

    // Locking is a comparison with an id a human recorded, never a computation.
    let locked = vintage().locked_as(id).unwrap();
    assert_eq!(locked.id(), Some(id));
    assert!(matches!(
        vintage().locked_as(other.content_id()).unwrap_err(),
        pfp_params::ParamError::VintageIdMismatch { .. }
    ));
}

#[test]
fn the_tables_say_which_series_they_read_and_which_year_of_it() {
    let v = vintage();
    for table in [ORDINARY_BRACKETS, STD_DEDUCTION] {
        let p = v.table(table).unwrap().projection();
        assert_eq!(p.index_series.as_deref(), Some(CPI_CHAINED_AUG12M));
        assert_eq!(p.lag_years, Some(1), "{table}: 26 USC 1(f)(3)(A)(i)");
    }
    let series = v.index_series(CPI_CHAINED_AUG12M).unwrap();
    assert_eq!(series.index_series(), CPI_CHAINED_AUG12M);
    assert_eq!(series.id(), "bls.cpi.chained.suur0000sa0");
}

#[test]
fn published_brackets_equal_the_transcription_for_every_status() {
    let v = vintage();
    let view = ParamView::new(&v);
    for (year, text) in [(2025, BRACKETS_2025), (2026, BRACKETS_2026)] {
        let fx = load(text, &format!("pending/published/brackets-{year}"));
        assert_eq!(int(&fx["lawYear"]), i64::from(year));
        let rows = fx["expect"]["filingStatuses"].as_array().unwrap();
        assert_eq!(rows.len(), FilingStatus::ALL.len());
        let mut seen = Vec::new();
        for row in rows {
            let key = row["filingStatus"].as_str().unwrap();
            assert!(FilingStatus::from_wire(key).is_some(), "{key}");
            seen.push(key);
            let got = view
                .published(ORDINARY_BRACKETS, year, key)
                .unwrap()
                .unwrap();
            assert_eq!(got.len(), EDGES.len());
            for (i, edge) in EDGES.iter().enumerate() {
                assert_eq!(
                    got[i],
                    dollars(&row["bracketTopsDollars"][edge]),
                    "{year} {key} {edge}"
                );
                assert_eq!(
                    got[i].0,
                    int(&row["bracketTopsCents"][edge]),
                    "{year} {key} {edge}"
                );
            }
            // `value` hands a published year out as published, never projected.
            let value = view.value(ORDINARY_BRACKETS, year, key).unwrap();
            assert_eq!(value.origin, Origin::Published);
            assert_eq!(value.amounts, got);

            let rates = view.rates(ORDINARY_BRACKETS, year).unwrap();
            let expected: Vec<Ratio> = row["rates"]
                .as_array()
                .unwrap()
                .iter()
                .map(|r| {
                    let pct = r.as_str().unwrap().strip_suffix('%').unwrap();
                    Ratio::new(pct.parse().unwrap(), 100).unwrap()
                })
                .collect();
            assert_eq!(rates.len(), EDGES.len() + 1);
            for (got, want) in rates.iter().zip(&expected) {
                assert!(same_value(*got, *want), "{year} {key}: {got} vs {want}");
            }
        }
        seen.sort_unstable();
        assert_eq!(seen, ["hoh", "mfj", "mfs", "qss", "single"]);
    }
}

#[test]
fn published_standard_deductions_equal_the_transcription_for_every_status() {
    let v = vintage();
    let view = ParamView::new(&v);
    for (year, text) in [(2025, STDDED_2025), (2026, STDDED_2026)] {
        let fx = load(text, &format!("pending/published/stdded-{year}"));
        let by_dollars = fx["expect"]["basicStandardDeductionDollars"]
            .as_object()
            .unwrap();
        let by_cents = &fx["expect"]["basicStandardDeductionCents"];
        assert_eq!(by_dollars.len(), FilingStatus::ALL.len());
        for status in FilingStatus::ALL {
            let key = status.wire();
            let got = view.published(STD_DEDUCTION, year, key).unwrap().unwrap();
            assert_eq!(got, [dollars(&by_dollars[key])], "{year} {key}");
            assert_eq!(got[0].0, int(&by_cents[key]), "{year} {key}");
            assert_eq!(
                view.value(STD_DEDUCTION, year, key).unwrap().scalar(),
                Some(got[0])
            );
        }
    }
}

#[test]
fn an_unpublished_year_is_not_silently_priced() {
    let v = vintage();
    let view = ParamView::new(&v);
    assert_eq!(view.published(STD_DEDUCTION, 2024, "single").unwrap(), None);
    assert_eq!(view.published(STD_DEDUCTION, 2027, "single").unwrap(), None);
    assert!(view.published(STD_DEDUCTION, 2026, "joint").is_err());
    assert!(view.published("irs.no_such_table", 2026, "single").is_err());
    // 2027 needs the calendar-year-2026 window, which the archive does not hold
    // (and which has a missing month): an error, never a guess.
    let err = view.value(STD_DEDUCTION, 2027, "single").unwrap_err();
    assert_eq!(
        err,
        pfp_params::ProjectionError::IndexUnavailable {
            index_series: CPI_CHAINED_AUG12M.to_owned(),
            calendar_year: 2026,
        }
    );
    // The rates are `flat`: the latest ladder carries forward, and nothing precedes 2025.
    assert_eq!(
        view.rates(ORDINARY_BRACKETS, 2031).unwrap(),
        view.rates(ORDINARY_BRACKETS, 2026).unwrap()
    );
    assert!(view.rates(ORDINARY_BRACKETS, 2024).is_err());
}

// ---------------------------------------------------------------------------
// Provenance
// ---------------------------------------------------------------------------

#[test]
fn every_table_and_series_carries_checksummed_archived_provenance() {
    let v = vintage();
    let series = v.index_series(CPI_CHAINED_AUG12M).unwrap();
    let all_sources = v
        .tables()
        .map(|t| (t.id(), t.sources(), t.verification(), t.open_items().len()))
        .chain([(
            series.id(),
            series.sources(),
            series.verification(),
            series.open_items().len(),
        )]);
    for (id, sources, verification, open_items) in all_sources {
        assert!(!sources.is_empty(), "{id}");
        assert_eq!(
            verification.wire(),
            Some("pending-hand-verification"),
            "{id}"
        );
        assert!(
            open_items > 0,
            "{id}: the open hand-verification items are surfaced"
        );
        for s in sources {
            assert!(s.url().starts_with("https://"), "{id}");
            assert!(
                s.archive().unwrap().starts_with("params/provenance/"),
                "{id}"
            );
            assert_eq!(s.sha256().len(), 64, "{id}");
            assert_eq!(s.retrieved().to_string(), "2026-09-18", "{id}");
            assert!(!s.title().is_empty() && s.publisher().is_some(), "{id}");
        }
    }
    let brackets = v.table(ORDINARY_BRACKETS).unwrap();
    assert_eq!(brackets.as_of().to_string(), "2025-10-09");
    // The fixtures cite the same archived bytes the tables do.
    let fx = load(BRACKETS_2026, "pending/published/brackets-2026");
    let cited = fx["source"]["sha256"].as_str().unwrap();
    let source = brackets
        .sources()
        .iter()
        .find(|s| s.sha256() == cited)
        .unwrap();
    assert_eq!(source.archive(), fx["source"]["archive"].as_str());
    assert_eq!(source.as_of().unwrap().to_string(), "2025-10-09");
}

// ---------------------------------------------------------------------------
// (2) The statutory uprating pipeline, residuals asserted exactly as recorded
// ---------------------------------------------------------------------------

/// Checks the fixture's own inputs against the vintage, so that a disagreement
/// below is about the pipeline and not about two copies of an input.
fn check_index_windows(v: &Vintage, fx: &Value) {
    let series = v.index_series(CPI_CHAINED_AUG12M).unwrap();
    for w in fx["inputs"]["indexWindows"].as_array().unwrap() {
        let year = i32::try_from(int(&w["calendarYear"])).unwrap();
        let want: Ratio = w["sumOfTwelve"].as_str().unwrap().parse().unwrap();
        let got = series.window_sum(year).unwrap();
        assert!(same_value(got, want), "window {year}: {got} vs {want}");
        // mean = sum / 12
        let mean = ratio_of(&w["meanExact"]);
        let sum_over_12 = Ratio::new(got.num, got.den * 12).unwrap();
        assert!(same_value(sum_over_12, mean), "mean {year}");
        assert_eq!(w["monthly"].as_array().unwrap().len(), 12);
    }
}

/// Runs one bracket-uprating fixture. Returns `(reproduced, residual rows)`.
fn run_bracket_uprating(text: &str, id: &str, year: i32) -> (usize, Vec<(String, String, i64)>) {
    let v = vintage();
    let view = ParamView::new(&v);
    let fx = load(text, id);
    assert_eq!(int(&fx["lawYear"]), i64::from(year));
    assert_eq!(fx["inputs"]["roundingBasis"], "IncreaseOverBase");
    assert_eq!(fx["inputs"]["roundingDirection"], "down");
    assert_eq!(
        int(&fx["inputs"]["numeratorCalendarYear"]),
        i64::from(year - lag_years(&v, ORDINARY_BRACKETS))
    );
    check_index_windows(&v, &fx);

    let table = v.table(ORDINARY_BRACKETS).unwrap();
    let rows = fx["expect"]["rows"].as_array().unwrap();
    assert_eq!(rows.len(), count(&fx["expect"]["rowCount"]));
    assert_eq!(rows.len(), FilingStatus::ALL.len() * EDGES.len());

    let mut reproduced = 0;
    let mut residuals = Vec::new();
    for row in rows {
        let key = row["filingStatus"].as_str().unwrap();
        let edge = row["edge"].as_str().unwrap();
        let i = edge_index(&row["edge"]);
        let at = format!("{id} {key} {edge}");

        // Inputs: base amount, base year, increment.
        let base = table.projection().base_values[key][i];
        assert_eq!(base, dollars(&row["baseValueDollars"]), "{at}: base");
        assert_eq!(
            base,
            dollars(&fx["inputs"]["baseValuesDollars"][key][edge]),
            "{at}"
        );
        let increment = table
            .projection()
            .rounding
            .as_ref()
            .unwrap()
            .increment_for(key);
        assert_eq!(
            increment,
            Some(dollars(&row["roundingIncrementDollars"])),
            "{at}"
        );

        let projected = view.project(ORDINARY_BRACKETS, year, key).unwrap();
        let Origin::Projected {
            steps,
            numerator_year,
            derived_from,
            ..
        } = &projected.origin
        else {
            panic!("{at}: not projected");
        };
        assert_eq!(*numerator_year, year - lag_years(&v, ORDINARY_BRACKETS));
        assert_eq!(*derived_from, None);
        let step = steps[i];
        assert_eq!(
            i64::from(step.base_year),
            int(&row["baseYear"]),
            "{at}: base year"
        );
        assert_eq!(step.rule.basis, RoundingBasis::IncreaseOverBase);
        assert_eq!(step.rule.direction, RoundingDirection::Down);

        // cola = factor - 1, exactly.
        let cola = Ratio::new(step.factor.num - step.factor.den, step.factor.den).unwrap();
        assert!(
            same_value(cola, ratio_of(&row["colaExact"])),
            "{at}: cola {cola}"
        );
        // raw increase = base dollars x cola, exactly.
        let raw = ratio_of(&row["rawIncreaseExact"]);
        assert_eq!(
            i128::from(base.0 / 100) * i128::from(cola.num) * i128::from(raw.den),
            i128::from(raw.num) * i128::from(cola.den),
            "{at}: raw increase"
        );

        let computed = projected.amounts[i];
        assert_eq!(computed, step.amount);
        assert_eq!(computed, dollars(&row["computedDollars"]), "{at}: computed");
        assert_eq!(
            computed - base,
            dollars(&row["flooredIncreaseDollars"]),
            "{at}: floored increase"
        );

        // The published figure comes from the vintage, not from the fixture.
        let published = view
            .published(ORDINARY_BRACKETS, year, key)
            .unwrap()
            .unwrap()[i];
        assert_eq!(
            published,
            dollars(&row["publishedDollars"]),
            "{at}: published"
        );
        let residual = (computed - published).0 / 100;
        assert_eq!(residual, int(&row["residualDollars"]), "{at}: residual");
        assert_eq!(
            residual == 0,
            row["reproducesPublished"].as_bool().unwrap(),
            "{at}"
        );
        if residual == 0 {
            reproduced += 1;
        } else {
            residuals.push((key.to_owned(), edge.to_owned(), residual));
        }
    }
    assert_eq!(reproduced, count(&fx["expect"]["reproducedCount"]), "{id}");
    assert_eq!(
        residuals.len(),
        count(&fx["expect"]["residualCount"]),
        "{id}"
    );

    // The fixture's residual list is exactly the residuals found.
    let listed: Vec<(String, String, i64)> = fx["residuals"]["rows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            (
                r["filingStatus"].as_str().unwrap().to_owned(),
                r["edge"].as_str().unwrap().to_owned(),
                int(&r["residualDollars"]),
            )
        })
        .collect();
    assert_eq!(residuals, listed, "{id}: residual list");
    assert_eq!(listed.len(), count(&fx["residuals"]["count"]));
    (reproduced, residuals)
}

#[test]
fn uprating_2026_brackets_matches_the_fixture_residuals_included() {
    let (reproduced, residuals) =
        run_bracket_uprating(UP_BRACKETS_2026, "pending/uprating/2026-brackets", 2026);
    // FINDING, not a pass: PLAN.md §4.1 asks for every published 2026 threshold.
    // Against the archived (2026-09-18 snapshot) series the statutory pipeline
    // reproduces 6 of 30; the other 24 are computed ABOVE the publication. The
    // fixture author attributes this to the index vintage 26 USC 1(f)(6)(A)
    // freezes, which the archive does not hold. Pinned so that it cannot drift.
    assert_eq!(reproduced, 6);
    assert_eq!(residuals.len(), 24);
    assert!(
        residuals.iter().all(|(_, _, r)| (25..=400).contains(r)),
        "{residuals:?}"
    );
}

#[test]
fn uprating_2025_brackets_control_matches_the_fixture_residuals_included() {
    let (reproduced, residuals) = run_bracket_uprating(
        UP_BRACKETS_2025,
        "pending/uprating/2025-brackets-control",
        2025,
    );
    // The positive control: every edge on base year 2017, residuals of the
    // OPPOSITE sign (computed below the publication).
    assert_eq!(reproduced, 1);
    assert_eq!(residuals.len(), 29);
    assert!(
        residuals.iter().all(|(_, _, r)| (-650..=-25).contains(r)),
        "{residuals:?}"
    );
}

/// The derivation 26 USC 63(c)(2)(A) states and no field of the shipped table
/// carries. Appended to the embedded text for the derived rows only; the shipped
/// bytes are not modified.
const STDDED_DERIVATION: &str = r#"
[projection.derived.mfj]
from = "single"
multiplier = "2"

[projection.derived.qss]
from = "single"
multiplier = "2"
"#;

#[test]
fn uprating_2026_standard_deduction_matches_the_fixture_residual_included() {
    let v = vintage();
    let view = ParamView::new(&v);
    let fx = load(UP_STDDED_2026, "pending/uprating/2026-stdded");
    assert_eq!(fx["inputs"]["roundingBasis"], "IncreaseOverBase");
    assert_eq!(int(&fx["inputs"]["baseYear"]), 2024);
    check_index_windows(&v, &fx);

    // The same vintage with the derivation declared, for the two derived rows.
    let [brackets, stdded] = shipped::federal_2026_tables();
    let with_derivation = format!("{stdded}{STDDED_DERIVATION}");
    let derived_vintage = Vintage::parse(
        "federal-2026+derivation",
        &[brackets, &with_derivation],
        &shipped::federal_2026_series(),
    )
    .unwrap();
    let derived_view = ParamView::new(&derived_vintage);

    let table = v.table(STD_DEDUCTION).unwrap();
    let rows = fx["expect"]["rows"].as_array().unwrap();
    assert_eq!(rows.len(), FilingStatus::ALL.len());
    let mut residuals = Vec::new();
    for row in rows {
        let key = row["filingStatus"].as_str().unwrap();
        let published = view.published(STD_DEDUCTION, 2026, key).unwrap().unwrap()[0];
        assert_eq!(published, dollars(&row["publishedDollars"]), "{key}");

        let computed = if row["baseValueDollars"].is_null() {
            // mfj / qss: 200% of the ROUNDED single result.
            assert!(row["derivation"]
                .as_str()
                .unwrap()
                .contains("ROUNDED single"));
            let p = derived_view.project(STD_DEDUCTION, 2026, key).unwrap();
            let Origin::Projected { derived_from, .. } = &p.origin else {
                panic!("{key}: not projected");
            };
            assert_eq!(
                *derived_from,
                Some(("single".to_owned(), Ratio::new(2, 1).unwrap()))
            );
            p.scalar().unwrap()
        } else {
            let base = table.projection().base_values[key][0];
            assert_eq!(base, dollars(&row["baseValueDollars"]), "{key}");
            assert_eq!(
                base,
                dollars(&fx["inputs"]["baseValuesDollars"][key]),
                "{key}"
            );
            let p = view.project(STD_DEDUCTION, 2026, key).unwrap();
            let Origin::Projected {
                steps,
                numerator_year,
                ..
            } = &p.origin
            else {
                panic!("{key}: not projected");
            };
            assert_eq!(*numerator_year, 2025);
            let step = steps[0];
            assert_eq!(i64::from(step.base_year), int(&row["baseYear"]));
            assert_eq!(
                step.rule.increment,
                dollars(&row["roundingIncrementDollars"])
            );
            let cola = Ratio::new(step.factor.num - step.factor.den, step.factor.den).unwrap();
            assert!(same_value(cola, ratio_of(&row["colaExact"])), "{key}");
            assert_eq!(
                step.amount - base,
                dollars(&row["flooredIncreaseDollars"]),
                "{key}"
            );
            // The reading the statute does NOT state, recorded by the fixture:
            // flooring the total instead of the increase.
            let floor_total = RoundingRule::new(
                step.rule.increment,
                RoundingDirection::Down,
                RoundingBasis::Amount,
            )
            .unwrap();
            assert_eq!(
                base.mul_ratio(step.factor, &floor_total),
                dollars(&row["alternativeReadingFloorTheTotalDollars"]),
                "{key}"
            );
            // The derivation changes nothing for a non-derived key.
            assert_eq!(
                derived_view
                    .project(STD_DEDUCTION, 2026, key)
                    .unwrap()
                    .amounts,
                p.amounts
            );
            p.scalar().unwrap()
        };
        assert_eq!(
            computed,
            dollars(&row["computedDollars"]),
            "{key}: computed"
        );
        let residual = (computed - published).0 / 100;
        assert_eq!(residual, int(&row["residualDollars"]), "{key}: residual");
        assert_eq!(residual == 0, row["reproducesPublished"].as_bool().unwrap());
        if residual != 0 {
            residuals.push((key.to_owned(), residual));
        }
    }
    // FINDING, not a pass: the statute floors the INCREASE to $50 and lands on
    // 24,175 for hoh; the publication prints 24,150 (floor the TOTAL). Recorded,
    // not fixed; a human decides (std_deduction.toml, [hand_verification]).
    assert_eq!(residuals, [("hoh".to_owned(), 25)]);
    assert_eq!(int(&fx["expect"]["residualCount"]), 1);
    assert_eq!(int(&fx["expect"]["reproducedCount"]), 4);

    // 2025 precedes the first adjusted year: the base amounts, unadjusted.
    for status in FilingStatus::ALL {
        let p = view.project(STD_DEDUCTION, 2025, status.wire()).unwrap();
        assert_eq!(p.origin, Origin::UnadjustedBase);
        assert_eq!(
            p.amounts,
            view.published(STD_DEDUCTION, 2025, status.wire())
                .unwrap()
                .unwrap()
        );
    }
}

#[test]
fn the_shipped_table_cannot_express_the_joint_derivation() {
    // FINDING. `std_deduction.toml` carries `base_values.mfj` "for shape" and has
    // no field for 63(c)(2)(A)'s "200 percent of the [rounded] amount in effect
    // under subparagraph (C)". Read as shipped, mfj and qss are therefore indexed
    // independently off their own base — which lands $50 ABOVE both the fixture's
    // statutory figure and the publication for 2026. This does not affect the
    // published 2025/2026 lookups; it would affect the first projected year.
    let v = vintage();
    let view = ParamView::new(&v);
    let fx = load(UP_STDDED_2026, "pending/uprating/2026-stdded");
    for key in ["mfj", "qss"] {
        let row = fx["expect"]["rows"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["filingStatus"] == key)
            .unwrap();
        let independent = view
            .project(STD_DEDUCTION, 2026, key)
            .unwrap()
            .scalar()
            .unwrap();
        let statutory = dollars(&row["computedDollars"]);
        assert_eq!((independent - statutory).0 / 100, 50, "{key}");
    }
}

// ---------------------------------------------------------------------------
// (3) The chained-path negative control
// ---------------------------------------------------------------------------

#[test]
fn a_year_over_year_chain_diverges_where_the_fixture_says_it_does() {
    let v = vintage();
    let view = ParamView::new(&v);
    let fx = load(CHAINED, "pending/uprating/chained-path-divergence");
    check_index_windows(&v, &fx);
    let series = v.index_series(CPI_CHAINED_AUG12M).unwrap();
    let one_year = series.factor(2025, 2024).unwrap();
    let step = ratio_of(&fx["inputs"]["oneYearStepExact"]);
    assert!(same_value(
        Ratio::new(one_year.num - one_year.den, one_year.den).unwrap(),
        step
    ));
    let spec = v
        .table(ORDINARY_BRACKETS)
        .unwrap()
        .projection()
        .rounding
        .clone()
        .unwrap();

    let asserted = fx["expect"]["rows"].as_array().unwrap();
    let context = fx["context"]["rows"].as_array().unwrap();
    assert_eq!(
        asserted.len() + context.len(),
        count(&fx["expect"]["totalRowCount"])
    );
    let mut diverging = 0;
    let mut statuses = Vec::new();
    for (row, is_asserted) in asserted
        .iter()
        .map(|r| (r, true))
        .chain(context.iter().map(|r| (r, false)))
    {
        let key = row["filingStatus"].as_str().unwrap();
        let i = edge_index(&row["edge"]);
        let at = format!("{key} {}", row["edge"]);

        // THE WRONG PATH, built here and nowhere in the library: one year of index
        // applied to the ALREADY-ROUNDED 2025 publication, rounded again.
        let from_2025 = view
            .published(ORDINARY_BRACKETS, 2025, key)
            .unwrap()
            .unwrap()[i];
        assert_eq!(from_2025, dollars(&row["from2025PublishedDollars"]), "{at}");
        let rule = RoundingRule::new(spec.increment_for(key).unwrap(), spec.direction, spec.basis)
            .unwrap();
        let chained = from_2025.mul_ratio(one_year, &rule);
        assert_eq!(chained, dollars(&row["chainedDollars"]), "{at}: chained");

        let statutory = view.project(ORDINARY_BRACKETS, 2026, key).unwrap().amounts[i];
        assert_eq!(
            statutory,
            dollars(&row["statutoryFromArchivedSeriesDollars"]),
            "{at}"
        );
        let published = view
            .published(ORDINARY_BRACKETS, 2026, key)
            .unwrap()
            .unwrap()[i];
        assert_eq!(published, dollars(&row["publishedDollars"]), "{at}");

        assert_eq!(
            (chained - statutory).0 / 100,
            int(&row["chainedMinusStatutoryDollars"]),
            "{at}"
        );
        assert_eq!(
            (chained - published).0 / 100,
            int(&row["chainedMinusPublishedDollars"]),
            "{at}"
        );
        assert_eq!(
            (statutory - published).0 / 100,
            int(&row["statutoryMinusPublishedDollars"]),
            "{at}"
        );
        assert_eq!(
            chained != statutory,
            row["chainedDivergesFromStatutory"].as_bool().unwrap(),
            "{at}"
        );
        assert_eq!(
            statutory == published,
            row["statutoryEqualsPublished"].as_bool().unwrap(),
            "{at}"
        );
        assert_ne!(
            chained, published,
            "{at}: a chain must not pass by coincidence"
        );
        if chained != statutory {
            diverging += 1;
        }
        if is_asserted {
            // TESTING.md §3.1: the two disagree AND the statutory value is the published one.
            assert_ne!(chained, statutory, "{at}");
            assert_eq!(statutory, published, "{at}");
            statuses.push(key.to_owned());
        }
    }
    assert_eq!(diverging, count(&fx["expect"]["divergingRowCount"]));
    assert_eq!(
        int(&fx["expect"]["rowsWhereChainedCoincidesWithPublished"]),
        0
    );
    statuses.sort();
    statuses.dedup();
    let covered: Vec<&str> = fx["expect"]["statusesCovered"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s.as_str().unwrap())
        .collect();
    assert_eq!(
        statuses, covered,
        "every filing status has a clean diverging threshold"
    );
}
