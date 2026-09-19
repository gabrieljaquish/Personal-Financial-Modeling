//! Properties of the rate schedule, over every table the embedded vintage can
//! produce: the published 2025 and 2026 rows and the rows projected for the same
//! years from the statutory base, for every filing status (`PLAN.md` §4.1).
//!
//! SYNTHETIC: every income is generated for the test and describes no person
//! (`docs/contributing.md` §1.3). No bracket edge or rate is written here; the
//! tables come from the vintage, and the uprating fixture is read, never written.

use pfp_domain::FilingStatus;
use pfp_money::{Cents, Ratio};
use pfp_params::shipped;
use pfp_params::{Origin, ParamView, Vintage};
use pfp_tax::{line_id, BracketTable};
use proptest::prelude::*;
use serde_json::Value;

const UP_BRACKETS_2026: &str =
    include_str!("../../../fixtures/pending/uprating/2026-brackets.json");
const YEARS: [i32; 2] = [2025, 2026];
const DOLLAR: i64 = 100;
/// Incomes up to 20 million dollars, well past every top edge.
const MAX_INCOME: i64 = 2_000_000_000;

fn config() -> ProptestConfig {
    ProptestConfig {
        cases: 512,
        failure_persistence: None,
        ..ProptestConfig::default()
    }
}

fn vintage() -> Vintage {
    shipped::federal_2026().expect("the embedded vintage validates")
}

/// Published and projected, both years, every status: 20 tables.
fn tables(v: &Vintage) -> Vec<BracketTable> {
    let view = ParamView::new(v);
    let mut out = Vec::new();
    for year in YEARS {
        for status in FilingStatus::ALL {
            let published = BracketTable::in_force(&view, year, status).unwrap();
            assert_eq!(*published.origin(), Origin::Published);
            let projected = BracketTable::projected(&view, year, status).unwrap();
            assert!(matches!(projected.origin(), Origin::Projected { .. }));
            out.push(published);
            out.push(projected);
        }
    }
    assert_eq!(out.len(), 20);
    out
}

fn sum(table: &BracketTable, income: i64) -> i64 {
    table
        .tax(Cents(income))
        .unwrap()
        .value(&line_id::SUM)
        .unwrap()
        .0
}

fn tax(table: &BracketTable, income: i64) -> i64 {
    table.tax(Cents(income)).unwrap().last().unwrap().value.0
}

/// `a <= b` for ratios with positive denominators.
fn le(a: Ratio, b: Ratio) -> bool {
    i128::from(a.num) * i128::from(b.den) <= i128::from(b.num) * i128::from(a.den)
}

/// An independent statement of the schedule: exact rational arithmetic over a
/// common denominator, rounded half-up to the whole dollar exactly once. Returns
/// cents. Shares no code with the crate (no `mul_ratio`, no `RoundingRule`).
fn reference_tax(table: &BracketTable, income: i64) -> i128 {
    let common: i128 = table.rates().iter().map(|r| i128::from(r.den)).product();
    let mut numerator = 0_i128;
    let mut bottom = 0_i128;
    for (n, rate) in table.rates().iter().enumerate() {
        let top = table.tops().get(n).map_or(i128::MAX, |t| i128::from(t.0));
        let amount = (i128::from(income).min(top) - bottom).max(0);
        numerator += amount * i128::from(rate.num) * (common / i128::from(rate.den));
        bottom = top;
    }
    // Half-up to a multiple of 100 cents: floor((2n + 100c) / (200c)) * 100.
    let unit = i128::from(DOLLAR) * common;
    (2 * numerator + unit).div_euclid(2 * unit) * i128::from(DOLLAR)
}

#[test]
fn zero_income_is_zero_tax_on_every_line() {
    for table in tables(&vintage()) {
        let lines = table.tax(Cents::ZERO).unwrap();
        assert!(lines.iter().all(|l| l.value == Cents::ZERO));
        assert_eq!(lines.last().unwrap().id, line_id::TAX);
    }
}

#[test]
fn the_schedule_is_continuous_at_every_bracket_edge() {
    for table in tables(&vintage()) {
        let at = format!("{} {} {:?}", table.year(), table.status(), table.tops());
        for (n, top) in table.tops().iter().enumerate() {
            let (below, above) = (table.rates()[n], table.rates()[n + 1]);
            // One dollar either side of the edge moves the unrounded sum by exactly
            // the marginal rate on that side: no jump, no gap, no double count.
            assert_eq!(
                below.den, 100,
                "{at}: whole-percent rates make a dollar exact"
            );
            assert_eq!(above.den, 100, "{at}");
            assert_eq!(
                sum(&table, top.0) - sum(&table, top.0 - DOLLAR),
                below.num,
                "{at}"
            );
            assert_eq!(
                sum(&table, top.0 + DOLLAR) - sum(&table, top.0),
                above.num,
                "{at}"
            );
            // To the cent: at most one cent of tax for one cent of income.
            let step = sum(&table, top.0 + 1) - sum(&table, top.0);
            assert!((0..=1).contains(&step), "{at}");
            // After the whole-dollar rounding the step is the marginal rate within
            // that rounding: zero or one dollar, never more.
            let rounded = tax(&table, top.0 + DOLLAR) - tax(&table, top.0);
            assert!(rounded == 0 || rounded == DOLLAR, "{at}: {rounded}");
        }
    }
}

#[test]
fn projected_and_published_2026_agree_exactly_where_the_uprating_fixture_says_they_do() {
    let fx: Value = serde_json::from_str(UP_BRACKETS_2026).unwrap();
    assert_eq!(fx["id"], "pending/uprating/2026-brackets");
    assert_eq!(fx["verification"], "pending-hand-verification");
    let v = vintage();
    let view = ParamView::new(&v);
    for status in FilingStatus::ALL {
        let published = BracketTable::in_force(&view, 2026, status).unwrap();
        let projected = BracketTable::projected(&view, 2026, status).unwrap();
        assert_eq!(published.rates(), projected.rates());
        // The fixture's rows for this status, in edge order.
        let reproduced: Vec<bool> = fx["expect"]["rows"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|r| r["filingStatus"] == status.wire())
            .map(|r| r["reproducesPublished"].as_bool().unwrap())
            .collect();
        let agree: Vec<bool> = published
            .tops()
            .iter()
            .zip(projected.tops())
            .map(|(a, b)| a == b)
            .collect();
        assert_eq!(agree, reproduced, "{status}: the residuals are as recorded");

        match coincidence_limit(&published, &projected) {
            // Every edge reproduced: the two tables are one table.
            None => assert_eq!(published.tops(), projected.tops()),
            // Up to the first edge that differs the worksheets are identical, and
            // past the higher of the two they are not: a test that passed under
            // both tables everywhere would test nothing.
            Some((limit, n)) => {
                assert_eq!(
                    published.tax(Cents(limit)).unwrap(),
                    projected.tax(Cents(limit)).unwrap(),
                    "{status}"
                );
                let past = published.tops()[n].max(projected.tops()[n]).0;
                assert_ne!(sum(&published, past), sum(&projected, past), "{status}");
            }
        }
    }
}

/// The income up to which two tables must give one worksheet: the lower of the
/// first pair of edges that differ, with that edge's index. `None` when no edge
/// differs.
fn coincidence_limit(a: &BracketTable, b: &BracketTable) -> Option<(i64, usize)> {
    a.tops()
        .iter()
        .zip(b.tops())
        .enumerate()
        .find(|(_, (x, y))| x != y)
        .map(|(n, (x, y))| (x.min(y).0, n))
}

proptest! {
    #![proptest_config(config())]

    #[test]
    fn tax_is_monotone_non_decreasing_in_income(
        a in 0..=MAX_INCOME, b in 0..=MAX_INCOME, which in 0_usize..20
    ) {
        let table = &tables(&vintage())[which];
        let (lo, hi) = (a.min(b), a.max(b));
        prop_assert!(sum(table, lo) <= sum(table, hi));
        prop_assert!(tax(table, lo) <= tax(table, hi));
    }

    #[test]
    fn the_marginal_rate_never_exceeds_the_top_statutory_rate(
        dollars in 0..=MAX_INCOME / DOLLAR, extra in 1..=1_000_000_i64, which in 0_usize..20
    ) {
        let table = &tables(&vintage())[which];
        let top = table.top_rate();
        prop_assert!(table.rates().iter().all(|r| le(*r, top)));
        let (x, d) = (dollars * DOLLAR, extra * DOLLAR);
        // Whole dollars and whole-percent rates: every product is exact, so the
        // bound is exact. d x top.num / top.den, cross-multiplied.
        let rise = i128::from(sum(table, x + d) - sum(table, x));
        prop_assert!(rise >= 0);
        prop_assert!(rise * i128::from(top.den) <= i128::from(d) * i128::from(top.num));
        // After the whole-dollar rounding: within that one dollar.
        let rounded = i128::from(tax(table, x + d) - tax(table, x));
        prop_assert!(
            rounded * i128::from(top.den)
                <= (i128::from(d) * i128::from(top.num)) + i128::from(DOLLAR) * i128::from(top.den)
        );
    }

    #[test]
    fn to_the_cent_the_rise_stays_within_the_products_rounding(
        x in 0..=MAX_INCOME, d in 1..=100_000_000_i64, which in 0_usize..20
    ) {
        let table = &tables(&vintage())[which];
        let top = table.top_rate();
        // Each of the products is rounded to the cent once, so two sums differ
        // from their exact values by at most half a cent per bracket each.
        let slack = i128::try_from(table.rates().len()).unwrap();
        let rise = i128::from(sum(table, x + d) - sum(table, x));
        prop_assert!(rise >= 0);
        prop_assert!(
            rise * i128::from(top.den)
                <= i128::from(d) * i128::from(top.num) + slack * i128::from(top.den)
        );
    }

    #[test]
    fn a_whole_dollar_income_matches_an_independent_exact_evaluation(
        dollars in 0..=MAX_INCOME / DOLLAR, which in 0_usize..20
    ) {
        let table = &tables(&vintage())[which];
        let x = dollars * DOLLAR;
        prop_assert_eq!(i128::from(tax(table, x)), reference_tax(table, x));
    }

    #[test]
    fn the_lines_reconcile(x in 0..=MAX_INCOME, which in 0_usize..20) {
        let table = &tables(&vintage())[which];
        let lines = table.tax(Cents(x)).unwrap();
        let brackets = table.rates().len();
        let mut amounts = Cents::ZERO;
        let mut taxes = Cents::ZERO;
        for n in 0..brackets {
            let amount = lines.value(&line_id::bracket_amount(n).unwrap()).unwrap();
            let width = table.tops().get(n).map(|t| {
                *t - if n == 0 { Cents::ZERO } else { table.tops()[n - 1] }
            });
            prop_assert!(amount >= Cents::ZERO);
            prop_assert!(width.is_none_or(|w| amount <= w));
            amounts += amount;
            taxes += lines.value(&line_id::bracket_tax(n).unwrap()).unwrap();
        }
        prop_assert_eq!(amounts, Cents(x));
        prop_assert_eq!(lines.value(&line_id::SUM), Some(taxes));
        let tax = lines.value(&line_id::TAX).unwrap();
        prop_assert_eq!(tax.0 % DOLLAR, 0);
        prop_assert!((tax.0 - taxes.0).abs() * 2 <= DOLLAR);
        prop_assert!(tax <= Cents(x));
    }

    #[test]
    fn published_and_projected_2026_give_one_worksheet_where_they_should_coincide(
        fraction in 0_u32..=1_000_000, status in 0_usize..5
    ) {
        let v = vintage();
        let view = ParamView::new(&v);
        let status = FilingStatus::ALL[status];
        let published = BracketTable::in_force(&view, 2026, status).unwrap();
        let projected = BracketTable::projected(&view, 2026, status).unwrap();
        let limit = coincidence_limit(&published, &projected).map_or(MAX_INCOME, |(l, _)| l);
        let x = i64::try_from(i128::from(limit) * i128::from(fraction) / 1_000_000).unwrap();
        prop_assert_eq!(published.tax(Cents(x)).unwrap(), projected.tax(Cents(x)).unwrap());
    }
}
