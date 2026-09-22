//! Property tests for seam S2 (`PLAN.md` §4.1, ADR-007). Every input is random
//! and therefore SYNTHETIC (`docs/contributing.md` §1.3).
//!
//! The reference implementation below is deliberately not the crate's: it works
//! in arbitrary precision, takes the floor by hand from a truncating division,
//! and compares twice the remainder with the step where the crate compares the
//! remainder with its complement.

use num_bigint::BigInt;
use pfp_money::{Cents, MoneyError, Ratio, RoundingBasis, RoundingDirection, RoundingRule};
use proptest::prelude::*;

use RoundingDirection::{Down, HalfEven, HalfUp, Nearest, Up};

fn config() -> ProptestConfig {
    ProptestConfig {
        cases: 2048,
        // No regression files: tests write nothing into the source tree.
        failure_persistence: None,
        ..ProptestConfig::default()
    }
}

// ---------------------------------------------------------------------------
// Reference
// ---------------------------------------------------------------------------

/// Floor of `n / d` for `d > 0`, from `BigInt`'s truncating division.
fn floor_div(n: &BigInt, d: &BigInt) -> BigInt {
    let zero = BigInt::from(0);
    assert!(d > &zero);
    let q = n / d;
    if n < &zero && &q * d != *n {
        q - 1
    } else {
        q
    }
}

/// `n / d` cents (any sign of `d`, `d != 0`) rounded to a multiple of `increment`.
/// `None` where the design specifies no answer.
fn reference_round(
    n: &BigInt,
    d: &BigInt,
    increment: i64,
    direction: RoundingDirection,
) -> Option<BigInt> {
    let zero = BigInt::from(0);
    let (n, d) = if d < &zero {
        (-n, -d)
    } else {
        (n.clone(), d.clone())
    };
    let step = &d * increment;
    let q = floor_div(&n, &step);
    let twice_r: BigInt = (&n - &q * &step) * 2;
    let up = &q + 1;
    #[allow(
        clippy::match_same_arms,
        reason = "one arm per row of the rounding table reads better than merged arms"
    )]
    let multiple = match direction {
        Down => q,
        Up if twice_r == zero => q,
        Up => up,
        _ if twice_r < step => q,
        _ if twice_r > step => up,
        // Exact ties from here on.
        Nearest => return None,
        HalfUp if n < zero => return None,
        HalfUp => up,
        HalfEven if &q % 2 == zero => q,
        HalfEven => up,
    };
    Some(multiple * increment)
}

fn reference_mul_ratio(cents: i64, r: Ratio, rule: &RoundingRule) -> Option<BigInt> {
    let (c, num, den) = (
        BigInt::from(cents),
        BigInt::from(r.num),
        BigInt::from(r.den),
    );
    let increment = rule.increment.0;
    match rule.basis {
        RoundingBasis::Amount => reference_round(&(&c * &num), &den, increment, rule.direction),
        RoundingBasis::IncreaseOverBase => {
            reference_round(&(&c * (&num - &den)), &den, increment, rule.direction).map(|i| i + c)
        }
    }
}

fn expected(reference: Option<BigInt>, direction: RoundingDirection) -> Result<Cents, MoneyError> {
    match reference {
        None => Err(MoneyError::UnspecifiedTie { direction }),
        Some(exact) => i64::try_from(exact)
            .map(Cents)
            .map_err(|_| MoneyError::Overflow),
    }
}

// ---------------------------------------------------------------------------
// Strategies
// ---------------------------------------------------------------------------

/// Any `i64`, weighted toward the values where integer arithmetic breaks.
fn extreme_i64() -> impl Strategy<Value = i64> {
    prop_oneof![
        4 => any::<i64>(),
        4 => -1_000_000_000i64..1_000_000_000,
        1 => prop::sample::select(vec![
            i64::MIN, i64::MIN + 1, -1, 0, 1, 2, i64::MAX - 1, i64::MAX,
        ]),
    ]
}

fn increment() -> impl Strategy<Value = i64> {
    prop_oneof![
        4 => prop::sample::select(vec![1i64, 2, 3, 10, 100, 2500, 5000, 100_000, 500_000]),
        2 => 1i64..1_000_000,
        1 => 1i64..=i64::MAX,
        1 => Just(i64::MAX),
    ]
}

fn direction() -> impl Strategy<Value = RoundingDirection> {
    prop::sample::select(vec![Down, Up, HalfUp, HalfEven, Nearest])
}

fn basis() -> impl Strategy<Value = RoundingBasis> {
    prop::sample::select(vec![RoundingBasis::Amount, RoundingBasis::IncreaseOverBase])
}

fn any_rule() -> impl Strategy<Value = RoundingRule> {
    (increment(), direction(), basis())
        .prop_map(|(i, d, b)| RoundingRule::new(Cents(i), d, b).unwrap())
}

fn amount_rule() -> impl Strategy<Value = RoundingRule> {
    (increment(), direction())
        .prop_map(|(i, d)| RoundingRule::new(Cents(i), d, RoundingBasis::Amount).unwrap())
}

/// Amounts and increments small enough that negation and neighbours never overflow.
fn small_amount() -> impl Strategy<Value = i64> {
    prop_oneof![-2_000_000i64..2_000_000, -(1i64 << 53)..(1i64 << 53),]
}

fn small_rule() -> impl Strategy<Value = RoundingRule> {
    (
        prop_oneof![
            prop::sample::select(vec![1i64, 2, 10, 100, 2500, 5000, 100_000]),
            1i64..10_000,
        ],
        direction(),
    )
        .prop_map(|(i, d)| RoundingRule::new(Cents(i), d, RoundingBasis::Amount).unwrap())
}

// ---------------------------------------------------------------------------
// Properties
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(config())]

    /// `mul_ratio` agrees with the arbitrary-precision reference for every input,
    /// including which inputs have no answer and why. Because the reference
    /// rounds the exact product once, agreement is also the rounds-once property.
    /// Reaching the end of the body is the no-panic property at the `i64` extremes.
    #[test]
    fn mul_ratio_agrees_with_the_reference(
        cents in extreme_i64(),
        num in extreme_i64(),
        den in extreme_i64(),
        rule in any_rule(),
    ) {
        let r = Ratio { num, den };
        let got = Cents(cents).checked_mul_ratio(r, &rule);
        if den == 0 {
            prop_assert_eq!(got, Err(MoneyError::ZeroDenominator));
        } else {
            prop_assert_eq!(got, expected(reference_mul_ratio(cents, r, &rule), rule.direction));
        }
    }

    /// The same, in the range real money and real rates live in, where almost
    /// every case has an answer.
    #[test]
    fn mul_ratio_agrees_with_the_reference_at_ordinary_sizes(
        cents in -1_000_000_000_000i64..1_000_000_000_000,
        num in -2_000_000i64..2_000_000,
        den in 1i64..2_000_000,
        rule in any_rule(),
    ) {
        let r = Ratio { num, den };
        let got = Cents(cents).checked_mul_ratio(r, &rule);
        prop_assert_eq!(got, expected(reference_mul_ratio(cents, r, &rule), rule.direction));
    }

    /// A hand-built rule with a bad increment is an error, never a division by zero.
    #[test]
    fn a_non_positive_increment_is_an_error(
        cents in extreme_i64(),
        num in extreme_i64(),
        den in extreme_i64(),
        bad in prop_oneof![Just(0i64), Just(i64::MIN), i64::MIN..=0],
        direction in direction(),
        basis in basis(),
    ) {
        let rule = RoundingRule { increment: Cents(bad), direction, basis };
        prop_assert!(RoundingRule::new(Cents(bad), direction, basis).is_err());
        let got = Cents(cents).checked_mul_ratio(Ratio { num, den }, &rule);
        prop_assert!(matches!(
            got,
            Err(MoneyError::NonPositiveIncrement | MoneyError::ZeroDenominator)
        ));
        prop_assert_eq!(rule.checked_round(Cents(cents)), Err(MoneyError::NonPositiveIncrement));
    }

    /// The value of a ratio decides the product, not its spelling: lowest terms,
    /// a common factor and a negated pair all give the same answer.
    #[test]
    fn mul_ratio_depends_on_the_value_of_the_ratio_only(
        cents in extreme_i64(),
        num in -2_000_000i64..2_000_000,
        den in 1i64..2_000_000,
        scale in 1i64..1_000_000,
        rule in any_rule(),
    ) {
        let r = Ratio { num, den };
        let got = Cents(cents).checked_mul_ratio(r, &rule);
        let scaled = Ratio { num: num * scale, den: den * scale };
        let negated = Ratio { num: -num, den: -den };
        prop_assert_eq!(Cents(cents).checked_mul_ratio(scaled, &rule), got);
        prop_assert_eq!(Cents(cents).checked_mul_ratio(negated, &rule), got);
        prop_assert_eq!(Cents(cents).checked_mul_ratio(r.reduced().unwrap(), &rule), got);
    }

    /// Rounding is idempotent: a rounded amount is a multiple and stays put.
    #[test]
    fn rounding_is_idempotent(amount in extreme_i64(), rule in amount_rule()) {
        if let Ok(once) = rule.checked_round(Cents(amount)) {
            prop_assert_eq!(once.0 % rule.increment.0, 0);
            prop_assert_eq!(rule.checked_round(once), Ok(once));
        }
    }

    /// The rounded amount is a multiple of the increment within one increment of
    /// the input, on the side the direction names, and within half an increment
    /// for the three nearest-style directions.
    #[test]
    fn rounding_lands_where_the_direction_says(amount in extreme_i64(), rule in amount_rule()) {
        if let Ok(rounded) = rule.checked_round(Cents(amount)) {
            let error = i128::from(rounded.0) - i128::from(amount);
            let increment = i128::from(rule.increment.0);
            match rule.direction {
                Down => prop_assert!(-increment < error && error <= 0),
                Up => prop_assert!(0 <= error && error < increment),
                HalfUp | HalfEven | Nearest => prop_assert!(2 * error.abs() <= increment),
            }
        }
    }

    /// Rounding is monotone: a larger amount never rounds to a smaller multiple.
    #[test]
    fn rounding_is_monotone(a in extreme_i64(), b in extreme_i64(), rule in amount_rule()) {
        let (lo, hi) = (a.min(b), a.max(b));
        if let (Ok(lo), Ok(hi)) = (rule.checked_round(Cents(lo)), rule.checked_round(Cents(hi))) {
            prop_assert!(lo <= hi);
        }
    }

    /// Monotonicity near a boundary, where random pairs rarely land.
    #[test]
    fn rounding_is_monotone_between_neighbours(amount in small_amount(), rule in small_rule()) {
        if let (Ok(lo), Ok(hi)) =
            (rule.checked_round(Cents(amount)), rule.checked_round(Cents(amount + 1)))
        {
            prop_assert!(lo <= hi);
            prop_assert!(hi.0 - lo.0 <= rule.increment.0);
        }
    }

    /// `mul_ratio` is monotone in the amount for a non-negative ratio, under
    /// either basis.
    #[test]
    fn mul_ratio_is_monotone_in_the_amount(
        a in small_amount(),
        b in small_amount(),
        num in 0i64..4_000_000,
        den in 1i64..2_000_000,
        rule in (small_rule(), basis()).prop_map(|(r, basis)| RoundingRule { basis, ..r }),
    ) {
        // Under IncreaseOverBase the map is base + round(base x (r - 1)), which is
        // monotone when r >= 1; a deflating factor is covered by the Amount case.
        prop_assume!(rule.basis == RoundingBasis::Amount || num >= den);
        let r = Ratio { num, den };
        let (lo, hi) = (a.min(b), a.max(b));
        if let (Ok(lo), Ok(hi)) =
            (Cents(lo).checked_mul_ratio(r, &rule), Cents(hi).checked_mul_ratio(r, &rule))
        {
            prop_assert!(lo <= hi);
        }
    }

    /// Sign symmetry, exactly where each rule implies it: `HalfEven` is odd;
    /// `Down` and `Up` are mirror images; `HalfUp` and `Nearest` are odd away
    /// from ties, and at a tie `Nearest` has no answer on either side.
    #[test]
    fn sign_symmetry_where_the_rule_implies_it(amount in small_amount(), rule in small_rule()) {
        let with = |direction| RoundingRule { direction, ..rule };
        let (x, minus_x) = (Cents(amount), Cents(-amount));

        let half_even = with(HalfEven);
        prop_assert_eq!(half_even.round(minus_x), -half_even.round(x));

        prop_assert_eq!(with(Down).round(minus_x), -with(Up).round(x));
        prop_assert_eq!(with(Up).round(minus_x), -with(Down).round(x));

        let nearest = with(Nearest);
        match (nearest.checked_round(x), nearest.checked_round(minus_x)) {
            (Ok(pos), Ok(neg)) => {
                prop_assert_eq!(neg, -pos);
                // Away from a tie the three nearest-style directions coincide.
                prop_assert_eq!(with(HalfUp).checked_round(x), Ok(pos));
                prop_assert_eq!(with(HalfUp).checked_round(minus_x), Ok(neg));
                prop_assert_eq!(half_even.round(x), pos);
            }
            (Err(a), Err(b)) => {
                prop_assert_eq!(a, MoneyError::UnspecifiedTie { direction: Nearest });
                prop_assert_eq!(a, b);
                // At a tie HalfUp answers for the non-negative side only.
                let half_up = with(HalfUp);
                let (non_negative, negative) = if amount >= 0 { (x, minus_x) } else { (minus_x, x) };
                prop_assert_eq!(half_up.checked_round(non_negative), Ok(with(Up).round(non_negative)));
                prop_assert_eq!(
                    half_up.checked_round(negative),
                    Err(MoneyError::UnspecifiedTie { direction: HalfUp })
                );
            }
            (a, b) => prop_assert!(false, "a tie on one side only: {a:?} vs {b:?}"),
        }
    }

    /// Path independence of `IncreaseOverBase` (`DOMAIN-MODEL.md` §15): the result
    /// depends on the base and the factor's value only, and rounding the increase
    /// as an amount and adding the base back gives the same cents.
    #[test]
    fn increase_over_base_is_base_plus_the_rounded_increase(
        base in 0i64..100_000_000,
        num in 1i64..4_000_000,
        den in 1i64..2_000_000,
        rule in small_rule(),
    ) {
        let factor = Ratio { num, den };
        let over_base = RoundingRule { basis: RoundingBasis::IncreaseOverBase, ..rule };
        let got = Cents(base).checked_mul_ratio(factor, &over_base);
        // base x (num - den) / den, rounded as an amount, plus the unrounded base.
        let increase = Cents(base).checked_mul_ratio(Ratio { num: num - den, den }, &rule);
        prop_assert_eq!(got, increase.map(|i| i + Cents(base)));
    }

    /// A decimal string parses to exactly the integer it spells over the power
    /// of ten it implies, unreduced.
    #[test]
    fn decimal_strings_parse_exactly(num in any::<i64>(), places in 0u32..=18) {
        let den = 10i64.pow(places);
        let magnitude = num.unsigned_abs();
        let scale = den.unsigned_abs();
        let sign = if num < 0 { "-" } else { "" };
        let text = if places == 0 {
            format!("{sign}{magnitude}")
        } else {
            let width = places as usize;
            format!("{sign}{}.{:0width$}", magnitude / scale, magnitude % scale)
        };
        prop_assert_eq!(text.parse::<Ratio>(), Ok(Ratio { num, den }));
        let fraction = format!("{num}/{den}");
        prop_assert_eq!(fraction.parse::<Ratio>(), Ok(Ratio { num, den }));
        let json = serde_json::to_string(&Ratio { num, den }).unwrap();
        prop_assert_eq!(serde_json::from_str::<Ratio>(&json).unwrap(), Ratio { num, den });
    }

    /// No string panics the parser.
    #[test]
    fn parsing_never_panics(text in "\\PC{0,24}", digits in "[-0-9./]{0,40}") {
        let _ = text.parse::<Ratio>();
        if let Ok(r) = digits.parse::<Ratio>() {
            prop_assert!(r.den > 0);
        }
    }

    /// `grow` is the IEEE product rounded half-even at the cent: checked against
    /// an exact integer decomposition of the product.
    #[test]
    fn grow_is_the_f64_product_rounded_half_even(
        cents in -(1i64 << 50)..(1i64 << 50),
        factor in prop_oneof![-4.0f64..4.0, 0.5f64..1.5, Just(1.0), Just(0.0)],
    ) {
        #[allow(clippy::cast_precision_loss, reason = "exact below 2^53")]
        let product = cents as f64 * factor;
        prop_assert_eq!(Cents(cents).grow(factor), Cents(reference_half_even(product)));
    }

    /// Growing by exactly one changes nothing wherever the amount is an exact `f64`.
    #[test]
    fn grow_by_one_is_the_identity(cents in -(1i64 << 53)..=(1i64 << 53)) {
        prop_assert_eq!(Cents(cents).grow(1.0), Cents(cents));
    }
}

/// Round a finite `f64` half-even to an integer using only integer arithmetic on
/// its bits.
fn reference_half_even(x: f64) -> i64 {
    let bits = x.to_bits();
    let negative = bits >> 63 == 1;
    let exponent = i64::try_from((bits >> 52) & 0x7ff).unwrap();
    let fraction = bits & ((1 << 52) - 1);
    // value = mantissa x 2^shift
    let (mantissa, shift) = if exponent == 0 {
        (fraction, -1074)
    } else {
        (fraction | (1 << 52), exponent - 1075)
    };
    let mantissa = BigInt::from(mantissa);
    let magnitude = if shift >= 0 {
        mantissa << usize::try_from(shift).unwrap()
    } else {
        let one = BigInt::from(1);
        let divisor = &one << usize::try_from(-shift).unwrap();
        let q = &mantissa / &divisor;
        let twice_r = (&mantissa - &q * &divisor) * 2;
        let odd = &q % 2 == one;
        if twice_r > divisor || (twice_r == divisor && odd) {
            q + 1
        } else {
            q
        }
    };
    let magnitude = i64::try_from(magnitude).unwrap();
    if negative {
        -magnitude
    } else {
        magnitude
    }
}
