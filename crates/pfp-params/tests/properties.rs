//! Uprating is **path-independent** (`ARCHITECTURE.md` §11, `DOMAIN-MODEL.md`
//! §15): every projected year is computed from the base year and the index
//! series, so no two routes to one year disagree. This is *not* associativity
//! over a chain of rounded results, which `IncreaseOverBase` does not satisfy and
//! which the fixture-driven negative control pins.
//!
//! SYNTHETIC: every amount and index value is generated for the test and
//! describes no law, person or publication (`docs/contributing.md` §1.3).

// Test documents are assembled as text; clarity beats an allocation here.
#![allow(clippy::format_push_string, clippy::type_complexity)]

use num_bigint::BigInt;
use pfp_money::Cents;
use pfp_params::{Origin, ParamView, ProjectionError, Vintage};
use proptest::prelude::*;

const BASE_YEAR: i32 = 2000;
const KEYS: [&str; 5] = ["single", "mfj", "mfs", "hoh", "qss"];
const SOURCE: &str = "[[source]]\ntitle = \"Synthetic\"\nurl = \"https://example.invalid/x\"\nretrieved = \"2001-02-03\"\nsha256 = \"0000000000000000000000000000000000000000000000000000000000000000\"\n";

/// A one-month-window series: `levels[i]` thousandths is the index for `BASE_YEAR + i`.
fn series_text(levels: &[u64]) -> String {
    let mut s = String::from(
        "id = \"test.series.table\"\nkind = \"index-series\"\nindex_series = \"s\"\nas_of = \"2001-01-01\"\n[window]\nkind = \"trailing-12-month-mean\"\nends_month = 12\nmonths = 1\n",
    );
    for (year, level) in (BASE_YEAR..).zip(levels) {
        s += &format!(
            "[observations.{year}]\n\"{year}-M12\" = \"{}.{:03}\"\n",
            level / 1000,
            level % 1000
        );
    }
    s + SOURCE
}

/// A single-amount indexed table; `published` rows are decoys the projection
/// must never read.
fn table_text(base_dollars: i64, increment: i64, lag: i32, published: &[(i32, i64)]) -> String {
    let mut s = String::from(
        "id = \"test.amount\"\nunit = \"USD\"\nbreakdown = [\"filingStatus\"]\nas_of = \"2001-01-01\"\n",
    );
    for key in KEYS {
        s += &format!("[values.{key}]\n{BASE_YEAR} = {base_dollars}\n");
        for (year, amount) in published {
            s += &format!("{year} = {amount}\n");
        }
    }
    s += &format!(
        "[projection]\nrule = \"index\"\nindex = \"i\"\nindex_series = \"s\"\nlag_years = {lag}\nbase_year = {BASE_YEAR}\n[projection.base_values]\n"
    );
    for key in KEYS {
        s += &format!("{key} = {base_dollars}\n");
    }
    s += &format!(
        "[projection.rounding]\nincrement = {increment}\ndirection = \"down\"\nbasis = \"IncreaseOverBase\"\n"
    );
    s + SOURCE
}

fn vintage(table: &str, series: &str) -> Vintage {
    Vintage::parse("synthetic", &[table], &[series]).unwrap()
}

/// `base + floor(base x (num/den - 1) / inc) x inc`, in cents, arbitrary precision.
fn reference(base_cents: i64, num: &BigInt, den: &BigInt, increment_cents: i64) -> BigInt {
    let base = BigInt::from(base_cents);
    let inc = BigInt::from(increment_cents);
    let increase_num = &base * (num - den);
    // Truncating division is the floor here: every operand is non-negative.
    assert!(increase_num >= BigInt::from(0) && *den > BigInt::from(0) && increment_cents > 0);
    let steps = increase_num / (den * &inc);
    base + steps * inc
}

fn levels() -> impl Strategy<Value = Vec<u64>> {
    // Non-decreasing: the statute adjusts by the percentage "(if any)" of increase.
    (
        50_000_u64..400_000,
        prop::collection::vec(0_u64..9_000, 2..12),
    )
        .prop_map(|(start, steps)| {
            let mut out = vec![start];
            for step in steps {
                out.push(out.last().unwrap() + step);
            }
            out
        })
}

proptest! {
    #[test]
    fn every_year_is_the_base_uprated_once(
        levels in levels(),
        base_dollars in 1_i64..2_000_000,
        increment in prop::sample::select(vec![1_i64, 25, 50, 100, 1000]),
        lag in 0_i32..2,
    ) {
        let v = vintage(&table_text(base_dollars, increment, lag, &[]), &series_text(&levels));
        let view = ParamView::new(&v);
        let den = BigInt::from(levels[0]);
        for (i, level) in levels.iter().enumerate() {
            let tax_year = BASE_YEAR + i32::try_from(i).unwrap() + lag;
            let got = view.project("test.amount", tax_year, "hoh").unwrap().scalar().unwrap();
            let want = reference(base_dollars * 100, &BigInt::from(*level), &den, increment * 100);
            prop_assert_eq!(BigInt::from(got.0), want);

            // Any route through an intermediate year m, composed from exact
            // factors and rounded once, lands on the same amount.
            for m in &levels[..=i] {
                let num = BigInt::from(*m) * BigInt::from(*level);
                let route_den = &den * BigInt::from(*m);
                let via = reference(base_dollars * 100, &num, &route_den, increment * 100);
                prop_assert_eq!(BigInt::from(got.0), via);
            }
        }
    }

    #[test]
    fn published_rows_are_never_an_input_to_a_projection(
        levels in levels(),
        base_dollars in 1_i64..2_000_000,
        decoys in prop::collection::vec(0_i64..5_000_000, 1..6),
    ) {
        let published: Vec<(i32, i64)> = (BASE_YEAR + 1..).zip(decoys).collect();
        let plain = vintage(&table_text(base_dollars, 50, 0, &[]), &series_text(&levels));
        let decoyed = vintage(&table_text(base_dollars, 50, 0, &published), &series_text(&levels));
        let (plain, decoyed) = (ParamView::new(&plain), ParamView::new(&decoyed));
        // Ask in a scrambled order too: a view keeps no state between calls.
        let years: Vec<i32> = (BASE_YEAR..).take(levels.len()).collect();
        let forward: Vec<Cents> = years.iter().map(|y| plain.project("test.amount", *y, "mfs").unwrap().scalar().unwrap()).collect();
        let backward: Vec<Cents> = years.iter().rev().map(|y| decoyed.project("test.amount", *y, "mfs").unwrap().scalar().unwrap()).collect();
        prop_assert_eq!(&forward, &backward.into_iter().rev().collect::<Vec<_>>());
        // ... while `value` hands the published row out, labelled as such.
        for (year, amount) in &published {
            if let Some(i) = years.iter().position(|y| y == year) {
                let v = decoyed.value("test.amount", *year, "mfs").unwrap();
                prop_assert_eq!(&v.origin, &Origin::Published);
                prop_assert_eq!(v.scalar().unwrap(), Cents::from_dollars(*amount).unwrap());
                prop_assert_eq!(plain.value("test.amount", *year, "mfs").unwrap().scalar().unwrap(), forward[i]);
            }
        }
    }
}

#[test]
fn a_chain_of_rounded_years_is_not_the_statutory_result() {
    // SYNTHETIC worked example: base 1,000, index 100 -> 104 -> 108.16, $50 down.
    // Direct: increase 81.60 -> 50 -> 1,050. Chained: 1,000 -> (40 -> 0) 1,000 ->
    // (40 -> 0) 1,000. The library offers no way to take the second path.
    let v = vintage(
        &table_text(1000, 50, 0, &[]),
        &series_text(&[100_000, 104_000, 108_160]),
    );
    let view = ParamView::new(&v);
    let at = |y| {
        view.project("test.amount", y, "single")
            .unwrap()
            .scalar()
            .unwrap()
    };
    assert_eq!(at(2000), Cents(100_000));
    assert_eq!(at(2001), Cents(100_000));
    assert_eq!(at(2002), Cents(105_000));
}

#[test]
fn a_falling_index_and_a_missing_window_are_errors_not_guesses() {
    let v = vintage(
        &table_text(1000, 50, 0, &[]),
        &series_text(&[100_000, 99_000]),
    );
    let view = ParamView::new(&v);
    assert_eq!(
        view.project("test.amount", 2001, "single").unwrap_err(),
        ProjectionError::IndexBelowBase {
            table: "test.amount".into(),
            year: 2001
        }
    );
    assert_eq!(
        view.project("test.amount", 2002, "single").unwrap_err(),
        ProjectionError::IndexUnavailable {
            index_series: "s".into(),
            calendar_year: 2002
        }
    );
    assert!(matches!(
        view.project("test.amount", 2001, "joint").unwrap_err(),
        ProjectionError::UnknownKey { .. }
    ));
}
