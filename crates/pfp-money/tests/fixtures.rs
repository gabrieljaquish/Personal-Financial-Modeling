//! Drives `pfp-money` from the rounding-table and `mul_ratio` fixtures under
//! `fixtures/pending/` (`TESTING.md` §3.1, `t1/rounding/table`, `t1/money/mul_ratio`).
//!
//! The fixtures were written from the archived primary documents before this
//! crate existed and are **pending human verification**; this test reads them and
//! never writes them. A disagreement is evidence about the code
//! (`docs/contributing.md` §3.1).

use pfp_money::{Cents, MoneyError, Ratio, RoundingBasis, RoundingDirection, RoundingRule};
use serde::Deserialize;
use serde_json::Value;

const ROUNDING_TABLE: &str = include_str!("../../../fixtures/pending/rounding/table.json");
const MUL_RATIO: &str = include_str!("../../../fixtures/pending/money/mul_ratio.json");

/// The fixtures spell the increment `incrementCents` so that they cannot inherit
/// the unit ambiguity recorded in the rounding table's `openQuestions`.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FixtureRule {
    increment_cents: i64,
    direction: RoundingDirection,
    basis: RoundingBasis,
}

impl FixtureRule {
    fn rule(&self) -> RoundingRule {
        RoundingRule::new(Cents(self.increment_cents), self.direction, self.basis).unwrap()
    }
}

#[derive(Deserialize)]
struct Envelope<C> {
    id: String,
    tier: String,
    verification: String,
    synthetic: bool,
    tolerance: Value,
    expect: Expect<C>,
    #[serde(default, rename = "openQuestions")]
    open_questions: Vec<Value>,
}

#[derive(Deserialize)]
struct Expect<C> {
    cases: Vec<C>,
}

fn load<C: for<'de> Deserialize<'de>>(text: &str, id: &str, cases: usize) -> Envelope<C> {
    let envelope: Envelope<C> = serde_json::from_str(text).unwrap();
    assert_eq!(envelope.id, id);
    // Not tier 1 and not locked: an AI-assisted session wrote these, and only a
    // human who has read the primary document may promote them (ADR-022).
    assert_eq!(envelope.tier, "pending");
    assert_eq!(envelope.verification, "pending-hand-verification");
    assert!(envelope.synthetic);
    assert_eq!(envelope.tolerance, serde_json::json!({"kind": "exact"}));
    // A pinned count: a case that stops deserializing cannot silently vanish.
    assert_eq!(envelope.expect.cases.len(), cases, "{id}: case count");
    envelope
}

fn gcd(mut a: u128, mut b: u128) -> u128 {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}

/// `n/d` in lowest terms with a positive denominator.
fn lowest_terms(n: i128, d: i128) -> (i128, i128) {
    assert!(d > 0);
    let g = i128::try_from(gcd(n.unsigned_abs(), d.unsigned_abs())).unwrap();
    (n / g, d / g)
}

// ---------------------------------------------------------------------------
// fixtures/pending/rounding/table.json
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RoundingCase {
    id: String,
    #[serde(rename = "ruleId")]
    _rule_id: String,
    rule: FixtureRule,
    inputs: RoundingInputs,
    expect_cents: i64,
    #[serde(rename = "derivation")]
    _derivation: String,
    #[serde(default, rename = "note")]
    _note: Option<String>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum RoundingInputs {
    Increase(IncreaseInputs),
    Amount(AmountInputs),
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct IncreaseInputs {
    base_cents: i64,
    factor: Ratio,
    raw_increase_cents: Fraction,
    rounded_increase_cents: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AmountInputs {
    amount_cents: Fraction,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Fraction {
    num: i64,
    den: i64,
}

#[test]
fn rounding_table_fixture() {
    let fixture: Envelope<RoundingCase> = load(ROUNDING_TABLE, "pending/rounding/table", 30);
    let (mut increase_cases, mut amount_cases) = (0, 0);
    for case in &fixture.expect.cases {
        let id = &case.id;
        let rule = case.rule.rule();
        match &case.inputs {
            RoundingInputs::Increase(IncreaseInputs {
                base_cents,
                factor,
                raw_increase_cents,
                rounded_increase_cents,
            }) => {
                increase_cases += 1;
                assert_eq!(rule.basis, RoundingBasis::IncreaseOverBase, "{id}");
                let base = Cents(*base_cents);
                let got = base.checked_mul_ratio(*factor, &rule);
                assert_eq!(got, Ok(Cents(case.expect_cents)), "{id}: uprated amount");
                // The fixture's own intermediates, so that a right total reached
                // through a wrong increase cannot pass.
                assert_eq!(
                    case.expect_cents - base_cents,
                    *rounded_increase_cents,
                    "{id}: fixture is internally inconsistent"
                );
                let exact = lowest_terms(
                    i128::from(*base_cents) * (i128::from(factor.num) - i128::from(factor.den)),
                    i128::from(factor.den),
                );
                assert_eq!(
                    exact,
                    lowest_terms(raw_increase_cents.num.into(), raw_increase_cents.den.into()),
                    "{id}: exact increase before rounding"
                );
                // The rounded increase is the exact increase rounded as an amount.
                if let Ok(num) = i64::try_from(exact.0) {
                    let den = i64::try_from(exact.1).unwrap();
                    let as_amount = RoundingRule {
                        basis: RoundingBasis::Amount,
                        ..rule
                    };
                    assert_eq!(
                        Cents(num).checked_mul_ratio(Ratio::new(1, den).unwrap(), &as_amount),
                        Ok(Cents(*rounded_increase_cents)),
                        "{id}: rounded increase"
                    );
                }
            }
            RoundingInputs::Amount(AmountInputs { amount_cents }) => {
                amount_cases += 1;
                assert_eq!(rule.basis, RoundingBasis::Amount, "{id}");
                let unit = Ratio::new(1, amount_cents.den).unwrap();
                let got = Cents(amount_cents.num).checked_mul_ratio(unit, &rule);
                assert_eq!(got, Ok(Cents(case.expect_cents)), "{id}: rounded amount");
                if amount_cents.den == 1 {
                    assert_eq!(
                        rule.checked_round(Cents(amount_cents.num)),
                        Ok(Cents(case.expect_cents)),
                        "{id}: checked_round"
                    );
                }
            }
        }
    }
    assert_eq!((increase_cases, amount_cases), (9, 21));
}

/// The fixture leaves four questions open **with no expected value**. Two of them
/// are about this crate's arithmetic; both are an explicit error here rather than
/// an invented answer. When a human settles one, the fixture gains a case, this
/// test is deleted with the error, and nothing that already passed changes.
#[test]
fn open_questions_in_the_rounding_table_are_errors_not_guesses() {
    let fixture: Envelope<RoundingCase> = load(ROUNDING_TABLE, "pending/rounding/table", 30);
    let ids: Vec<&str> = fixture
        .open_questions
        .iter()
        .map(|q| q["id"].as_str().unwrap())
        .collect();
    assert_eq!(
        ids,
        [
            "nearest-exact-tie",
            "down-versus-truncate",
            "halfup-negative-tie",
            "increment-unit"
        ]
    );
    let rule = |increment, direction| {
        RoundingRule::new(Cents(increment), direction, RoundingBasis::Amount).unwrap()
    };
    // nearest-exact-tie: the question's own SYNTHETIC example, $109,500.00 at $1,000.
    assert_eq!(
        rule(100_000, RoundingDirection::Nearest).checked_round(Cents(10_950_000)),
        Err(MoneyError::UnspecifiedTie {
            direction: RoundingDirection::Nearest
        })
    );
    // halfup-negative-tie: the question's own example, -12,350 cents at 100.
    assert_eq!(
        rule(100, RoundingDirection::HalfUp).checked_round(Cents(-12_350)),
        Err(MoneyError::UnspecifiedTie {
            direction: RoundingDirection::HalfUp
        })
    );
}

// ---------------------------------------------------------------------------
// fixtures/pending/money/mul_ratio.json
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct MulRatioCase {
    id: String,
    inputs: MulRatioInputs,
    intermediate_product_abs: String,
    exceeds_i64_max: bool,
    /// Four numerators exceed `u64` and reach `serde_json` as a float, so this is
    /// compared only when it is an exact integer; `intermediateProductAbs`, a
    /// string, carries the same information exactly for every case.
    exact_product_cents: Value,
    expect_cents: i64,
    #[serde(rename = "resultFitsI64")]
    result_fits_i64: bool,
    #[serde(rename = "derivation")]
    _derivation: String,
    #[serde(default, rename = "note")]
    _note: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MulRatioInputs {
    cents: i64,
    ratio: Ratio,
    rule: FixtureRule,
}

#[test]
fn mul_ratio_fixture() {
    let fixture: Envelope<MulRatioCase> = load(MUL_RATIO, "pending/money/mul_ratio", 14);
    let mut forced_i128 = 0;
    for case in &fixture.expect.cases {
        let id = &case.id;
        let MulRatioInputs { cents, ratio, rule } = &case.inputs;
        let got = Cents(*cents).checked_mul_ratio(*ratio, &rule.rule());
        assert_eq!(got, Ok(Cents(case.expect_cents)), "{id}");
        assert!(case.result_fits_i64, "{id}");
        assert_eq!(
            Cents(*cents).mul_ratio(*ratio, &rule.rule()),
            Cents(case.expect_cents)
        );

        let product = i128::from(*cents) * i128::from(ratio.num);
        assert_eq!(
            product.unsigned_abs().to_string(),
            case.intermediate_product_abs,
            "{id}: intermediate product"
        );
        let exceeds = product.unsigned_abs() > u128::from(i64::MAX.unsigned_abs());
        assert_eq!(exceeds, case.exceeds_i64_max, "{id}: exceedsI64Max");
        forced_i128 += usize::from(exceeds);

        let exact = &case.exact_product_cents;
        if let (Some(num), Some(den)) = (exact["num"].as_i64(), exact["den"].as_i64()) {
            assert_eq!(
                lowest_terms(product, ratio.den.into()),
                lowest_terms(num.into(), den.into()),
                "{id}: exact product"
            );
        } else {
            assert!(
                exceeds,
                "{id}: only an i128-sized numerator may be inexact in JSON"
            );
        }
    }
    // The point of the file: an i64 intermediate would have overflowed four times.
    assert_eq!(forced_i128, 4);
}
