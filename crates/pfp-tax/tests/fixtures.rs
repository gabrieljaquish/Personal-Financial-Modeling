//! Drives `schedule_tax` from every fixture under `fixtures/pending/schedule/`
//! (`TESTING.md` §2, `PLAN.md` §4.1 acceptance).
//!
//! The fixtures were written from the archived revenue procedures before this
//! crate existed and are **pending human verification**; this test reads them and
//! never writes them (`docs/contributing.md` §3.1). A disagreement here is
//! evidence about the code or a finding to report, never a reason to edit an
//! expected value. Inputs are SYNTHETIC grid points and describe no person.

// A fixture driver reads top to bottom like the fixture it checks.
#![allow(clippy::too_many_lines)]

use pfp_domain::FilingStatus;
use pfp_explain::{Lines, RuleId};
use pfp_money::{Cents, Ratio};
use pfp_params::shipped::{self, ORDINARY_BRACKETS};
use pfp_params::{Origin, ParamView, ProjectionError, Vintage};
use pfp_tax::{line_id, schedule_tax, schedule_tax_with, BracketTable, ScheduleError};
use serde_json::Value;

const GRIDS: [(&str, &str); 10] = [
    (
        "single-2025-grid",
        include_str!("../../../fixtures/pending/schedule/single-2025-grid.json"),
    ),
    (
        "single-2026-grid",
        include_str!("../../../fixtures/pending/schedule/single-2026-grid.json"),
    ),
    (
        "mfj-2025-grid",
        include_str!("../../../fixtures/pending/schedule/mfj-2025-grid.json"),
    ),
    (
        "mfj-2026-grid",
        include_str!("../../../fixtures/pending/schedule/mfj-2026-grid.json"),
    ),
    (
        "mfs-2025-grid",
        include_str!("../../../fixtures/pending/schedule/mfs-2025-grid.json"),
    ),
    (
        "mfs-2026-grid",
        include_str!("../../../fixtures/pending/schedule/mfs-2026-grid.json"),
    ),
    (
        "hoh-2025-grid",
        include_str!("../../../fixtures/pending/schedule/hoh-2025-grid.json"),
    ),
    (
        "hoh-2026-grid",
        include_str!("../../../fixtures/pending/schedule/hoh-2026-grid.json"),
    ),
    (
        "qss-2025-grid",
        include_str!("../../../fixtures/pending/schedule/qss-2025-grid.json"),
    ),
    (
        "qss-2026-grid",
        include_str!("../../../fixtures/pending/schedule/qss-2026-grid.json"),
    ),
];
const ACCEPTANCE: &str =
    include_str!("../../../fixtures/pending/schedule/mfj-2026-plan-acceptance.json");

/// The fixtures name the six edges `top10 .. top35`; the parameter table names
/// them `top_of_10 .. top_of_35`. Same order.
const FIXTURE_EDGES: [&str; 6] = ["top10", "top12", "top22", "top24", "top32", "top35"];
const POINTS_PER_GRID: usize = 8;

fn vintage() -> Vintage {
    shipped::federal_2026().expect("the embedded vintage validates")
}

fn load(text: &str, name: &str) -> Value {
    let v: Value = serde_json::from_str(text).unwrap();
    assert_eq!(v["id"], format!("pending/schedule/{name}"));
    // Not tier 1 and not locked: only a human who has read the primary document
    // may promote these (ADR-022).
    assert_eq!(v["tier"], "pending");
    assert_eq!(v["verification"], "pending-hand-verification");
    assert_eq!(v["tolerance"], serde_json::json!({"kind": "exact"}));
    assert_eq!(v["paramVintage"], Value::Null);
    assert_eq!(v["module"], "pfp-tax");
    assert_eq!(v["synthetic"], true);
    v
}

fn int(v: &Value) -> i64 {
    v.as_i64().unwrap_or_else(|| panic!("not an integer: {v}"))
}

fn opt_cents(v: &Value) -> Option<Cents> {
    (!v.is_null()).then(|| Cents(int(v)))
}

/// Every `lineId` string anywhere under `v`, however the fixture nests it.
fn line_ids_in<'a>(v: &'a Value, out: &mut Vec<&'a str>) {
    match v {
        Value::Object(map) => {
            for (key, inner) in map {
                match inner.as_str() {
                    Some(id) if key == "lineId" => out.push(id),
                    _ => line_ids_in(inner, out),
                }
            }
        }
        Value::Array(items) => items.iter().for_each(|inner| line_ids_in(inner, out)),
        _ => {}
    }
}

/// `TESTING.md` §2.2's loader rule, independent of the envelope's nesting: a line
/// id a fixture names must exist in the worksheet, so a renamed line breaks the
/// build instead of silently skipping an assertion. (§2.2 words the rule for
/// `expect.lines[].id`; these fixtures nest it at `expect.cases[].lines[].lineId`.)
fn assert_every_named_line_exists(case: &Value, lines: &Lines, at: &str) {
    let mut ids = Vec::new();
    line_ids_in(case, &mut ids);
    assert!(!ids.is_empty(), "{at}: the case names no line id at all");
    for id in ids {
        let id = pfp_explain::LineId::new(id.to_owned())
            .unwrap_or_else(|e| panic!("{at}: `{id}` is not a line id: {e:?}"));
        assert!(lines.get(&id).is_some(), "{at}: no line `{}`", id.as_str());
    }
}

/// Checks one fixture case against the worksheet, line by line. Returns the tax.
fn check_case(case: &Value, table: &BracketTable, lines: &Lines, at: &str) -> Cents {
    assert_every_named_line_exists(case, lines, at);
    let ti = Cents(int(&case["inputs"]["taxableIncomeCents"]));
    assert_eq!(
        Cents::from_dollars(int(&case["inputs"]["taxableIncomeDollars"])).unwrap(),
        ti,
        "{at}: the fixture's two statements of the input agree"
    );
    assert_eq!(lines.value(&line_id::TAXABLE_INCOME), Some(ti), "{at}");

    let expected = case["lines"].as_array().unwrap();
    assert_eq!(expected.len(), table.rates().len(), "{at}: bracket count");
    let mut total_amount = Cents::ZERO;
    for (n, want) in expected.iter().enumerate() {
        let at = format!("{at} bracket {n}");
        assert_eq!(int(&want["bracket"]), i64::try_from(n).unwrap(), "{at}");
        let tax_id = line_id::bracket_tax(n).unwrap();
        let amount_id = line_id::bracket_amount(n).unwrap();
        assert_eq!(want["lineId"], tax_id.as_str(), "{at}");

        // The rate, exactly as the vintage writes it, and as the fixture prints it.
        let rate = table.rates()[n];
        assert_eq!(
            rate,
            Ratio::new(
                int(&want["rateRatio"]["num"]),
                int(&want["rateRatio"]["den"])
            )
            .unwrap(),
            "{at}"
        );
        assert_eq!(
            want["rate"].as_str().unwrap(),
            format!("{}%", rate.num * 100 / rate.den),
            "{at}"
        );

        // The band: "over bottom but not over top".
        let bottom = if n == 0 {
            Cents::ZERO
        } else {
            table.tops()[n - 1]
        };
        assert_eq!(Cents(int(&want["bandBottomCents"])), bottom, "{at}");
        assert_eq!(
            opt_cents(&want["bandTopCents"]),
            table.tops().get(n).copied(),
            "{at}"
        );

        // The intermediates.
        let amount = lines.get(&amount_id).unwrap();
        assert_eq!(amount.value, Cents(int(&want["amountTaxedCents"])), "{at}");
        assert_eq!(amount.inputs.as_slice(), [line_id::TAXABLE_INCOME], "{at}");
        let tax = lines.get(&tax_id).unwrap();
        assert_eq!(tax.value, Cents(int(&want["taxFromBracketCents"])), "{at}");
        assert_eq!(tax.inputs.as_slice(), [amount_id], "{at}");
        assert_eq!(
            tax.rounding,
            Some(RuleId::from_static("money.cent_half_even")),
            "{at}"
        );

        // Parameter references: the edges that bound the band, then the rate.
        let edges: Vec<&str> = amount
            .params
            .iter()
            .map(|p| {
                assert_eq!(p.param_id(), ORDINARY_BRACKETS, "{at}");
                assert_eq!(p.year(), Some(table.year()), "{at}");
                assert_eq!(p.breakdown_key(), Some(table.status().wire()), "{at}");
                p.element().unwrap()
            })
            .collect();
        let names = table.edge_names();
        let mut want_edges = Vec::new();
        if n > 0 {
            want_edges.push(names[n - 1].as_str());
        }
        if n < names.len() {
            want_edges.push(names[n].as_str());
        }
        assert_eq!(edges, want_edges, "{at}");
        assert_eq!(tax.params.len(), 1, "{at}");
        assert_eq!(tax.params[0].param_id(), ORDINARY_BRACKETS, "{at}");
        assert_eq!(tax.params[0].year(), Some(table.year()), "{at}");
        assert_eq!(
            tax.params[0].breakdown_key(),
            None,
            "{at}: rates have no breakdown"
        );
        assert_eq!(
            tax.params[0].element(),
            Some(format!("rates.{n}").as_str()),
            "{at}"
        );
        total_amount += amount.value;
    }
    assert_eq!(total_amount, ti, "{at}: the bands partition the income");

    let sum = lines.get(&line_id::SUM).unwrap();
    assert_eq!(sum.value, Cents(int(&case["exactSumCents"])), "{at}: sum");
    assert_eq!(sum.inputs.len(), expected.len(), "{at}");
    assert_eq!(sum.rounding, None, "{at}: the sum is exact");

    let tax = lines.last().unwrap();
    assert_eq!(
        tax.id,
        line_id::TAX,
        "{at}: the tax is the worksheet's result"
    );
    assert_eq!(tax.value, Cents(int(&case["expectTaxCents"])), "{at}: TAX");
    assert_eq!(
        tax.value,
        Cents::from_dollars(int(&case["expectTaxDollars"])).unwrap(),
        "{at}: to the dollar"
    );
    assert_eq!(tax.inputs.as_slice(), [line_id::SUM], "{at}");
    assert_eq!(
        tax.rounding,
        Some(RuleId::from_static("irs.whole_dollar")),
        "{at}"
    );
    assert_eq!(
        lines.len(),
        3 + 2 * expected.len(),
        "{at}: no line the fixture does not account for"
    );
    // Every line is reachable from the result: the trace is one tree.
    assert_eq!(
        lines.trail(&line_id::TAX).unwrap().len(),
        lines.len(),
        "{at}"
    );
    tax.value
}

/// The fixture's own statement of the table equals what the vintage holds.
fn check_table(fx: &Value, table: &BracketTable, at: &str) {
    let tops = &fx["inputs"]["bracketTopsDollars"];
    assert_eq!(tops.as_object().unwrap().len(), FIXTURE_EDGES.len(), "{at}");
    for (n, edge) in FIXTURE_EDGES.iter().enumerate() {
        assert_eq!(
            Cents::from_dollars(int(&tops[edge])).unwrap(),
            table.tops()[n],
            "{at} {edge}"
        );
    }
    assert_eq!(*table.origin(), Origin::Published, "{at}");
}

#[test]
fn every_grid_point_matches_to_the_dollar_with_its_line_intermediates() {
    let v = vintage();
    let view = ParamView::new(&v);
    let mut seen = Vec::new();
    let mut points = 0;
    for (name, text) in GRIDS {
        let fx = load(text, name);
        assert_eq!(fx["promotesTo"], format!("t1/schedule/{name}"));
        let year = i32::try_from(int(&fx["lawYear"])).unwrap();
        let cases = fx["expect"]["cases"].as_array().unwrap();
        assert_eq!(cases.len(), POINTS_PER_GRID, "{name}");
        let mut status_of_file = None;
        for case in cases {
            let at = format!("{name}/{}", case["label"].as_str().unwrap());
            let status =
                FilingStatus::from_wire(case["inputs"]["filingStatus"].as_str().unwrap()).unwrap();
            assert_eq!(*status_of_file.get_or_insert(status), status, "{at}");
            assert_eq!(int(&case["inputs"]["year"]), i64::from(year), "{at}");
            assert!(name.starts_with(status.wire()), "{at}");

            let table = BracketTable::in_force(&view, year, status).unwrap();
            check_table(&fx, &table, &at);
            let ti = Cents(int(&case["inputs"]["taxableIncomeCents"]));
            let lines = schedule_tax_with(year, status, ti, &view).unwrap();
            check_case(case, &table, &lines, &at);
            assert_eq!(lines, table.tax(ti).unwrap(), "{at}: one computation");
            points += 1;
        }
        seen.push((year, status_of_file.unwrap()));
    }
    // Every filing status, both years, nothing twice.
    let mut want: Vec<_> = [2025, 2026]
        .into_iter()
        .flat_map(|y| FilingStatus::ALL.into_iter().map(move |s| (y, s)))
        .collect();
    want.sort_unstable();
    seen.sort_unstable();
    assert_eq!(seen, want);
    assert_eq!(points, POINTS_PER_GRID * GRIDS.len());
}

#[test]
fn the_three_plan_acceptance_points_match_with_their_line_intermediates() {
    let fx = load(ACCEPTANCE, "mfj-2026-plan-acceptance");
    // Its own tier-1 id: it once shared the grid's, and one file would have
    // shadowed the other at promotion.
    assert_eq!(fx["promotesTo"], "t1/schedule/mfj-2026-plan-acceptance");
    let v = vintage();
    let view = ParamView::new(&v);
    let table = BracketTable::in_force(&view, 2026, FilingStatus::Mfj).unwrap();
    let cases = fx["expect"]["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 3);
    assert_eq!(fx["expect"]["allAgreeWithPlanMd"], true);
    let mut got = Vec::new();
    for case in cases {
        let ti = Cents(int(&case["inputs"]["taxableIncomeCents"]));
        let at = format!("plan-acceptance/{ti}");
        assert_eq!(case["inputs"]["filingStatus"], "mfj");
        assert_eq!(int(&case["inputs"]["year"]), 2026);
        check_table(&fx, &table, &at);
        // The design's three-argument form, reading the embedded vintage.
        let lines = schedule_tax(2026, FilingStatus::Mfj, ti).unwrap();
        let tax = check_case(case, &table, &lines, &at);
        assert_eq!(
            tax,
            Cents::from_dollars(int(&case["planMdFigureDollars"])).unwrap(),
            "{at}: PLAN.md section 4.1"
        );
        got.push((ti, tax));
    }
    // SYNTHETIC inputs; the expected figures are PLAN.md section 4.1's, which the
    // fixture derived independently from Rev. Proc. 2025-32 section 4.01.
    assert_eq!(
        got,
        [
            (Cents(10_000_000), Cents(1_150_400)),
            (Cents(15_000_000), Cents(2_242_400)),
            (Cents(25_000_000), Cents(4_519_600)),
        ]
    );
}

#[test]
fn an_income_on_an_edge_is_not_over_it() {
    // "Over X but not over Y": every published table, every edge.
    let v = vintage();
    let view = ParamView::new(&v);
    for year in [2025, 2026] {
        for status in FilingStatus::ALL {
            let table = BracketTable::in_force(&view, year, status).unwrap();
            for (n, top) in table.tops().iter().enumerate() {
                let above = line_id::bracket_amount(n + 1).unwrap();
                let here = line_id::bracket_amount(n).unwrap();
                let bottom = if n == 0 {
                    Cents::ZERO
                } else {
                    table.tops()[n - 1]
                };
                let on = table.tax(*top).unwrap();
                assert_eq!(on.value(&above), Some(Cents::ZERO), "{year} {status} {n}");
                assert_eq!(on.value(&here), Some(*top - bottom), "{year} {status} {n}");
                let over = table.tax(*top + Cents(1)).unwrap();
                assert_eq!(over.value(&above), Some(Cents(1)), "{year} {status} {n}");
                assert_eq!(
                    over.value(&here),
                    Some(*top - bottom),
                    "{year} {status} {n}"
                );
            }
        }
    }
}

#[test]
fn what_the_schedule_cannot_answer_is_an_error_not_a_guess() {
    let v = vintage();
    let view = ParamView::new(&v);
    assert_eq!(
        schedule_tax(2026, FilingStatus::Single, Cents(-1)),
        Err(ScheduleError::NegativeTaxableIncome(Cents(-1)))
    );
    // A year past the archived index series is not projected from a guess, and a
    // year before the table's first base-year mapping has no schedule here.
    assert!(matches!(
        schedule_tax_with(2027, FilingStatus::Single, Cents::ZERO, &view),
        Err(ScheduleError::Params(
            ProjectionError::IndexUnavailable { .. }
        ))
    ));
    assert!(matches!(
        schedule_tax_with(2024, FilingStatus::Single, Cents::ZERO, &view),
        Err(ScheduleError::Params(ProjectionError::NoBaseYear { .. }))
    ));
    // The top of the cent range still has an answer: tax never exceeds income.
    let lines = schedule_tax_with(2026, FilingStatus::Single, Cents(i64::MAX), &view).unwrap();
    let tax = lines.last().unwrap().value;
    assert!(Cents::ZERO < tax && tax < Cents(i64::MAX));
}

#[test]
fn the_worksheet_survives_a_json_round_trip() {
    let lines = schedule_tax(2026, FilingStatus::Hoh, Cents(12_345_678)).unwrap();
    let json = serde_json::to_string(&lines).unwrap();
    assert_eq!(serde_json::from_str::<Lines>(&json).unwrap(), lines);
}
