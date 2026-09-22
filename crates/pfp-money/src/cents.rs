//! `Cents`: all money (`DOMAIN-MODEL.md` §2.1, ADR-007).

use core::fmt;
use core::iter::Sum;
use core::ops::{Add, AddAssign, Neg, Sub, SubAssign};

use serde::{Deserialize, Serialize};

use crate::error::MoneyError;
use crate::ratio::Ratio;
use crate::rounding::RoundingRule;

/// An amount of money in integer cents. JSON carries the integer:
/// `12_500_000 == $125,000.00`.
///
/// `Cents * Cents` does not compile, and neither does `Cents * i64`: a rate is a
/// [`Ratio`] applied with [`Cents::mul_ratio`] under a named [`RoundingRule`].
/// `+`, `-`, unary `-` and `sum()` panic on overflow **in every build profile**
/// (never wrap); the `checked_*` forms return the error instead.
///
/// ```compile_fail
/// use pfp_money::Cents;
/// let _ = Cents(2) * Cents(3);
/// ```
///
/// ```compile_fail
/// use pfp_money::Cents;
/// let _ = Cents(2) * 3;
/// ```
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct Cents(pub i64);

const CENTS_PER_DOLLAR: i64 = 100;

impl Cents {
    /// No money.
    pub const ZERO: Self = Self(0);

    /// Whole dollars to cents: parameter tables publish integer dollars.
    ///
    /// # Errors
    ///
    /// [`MoneyError::Overflow`].
    pub const fn from_dollars(dollars: i64) -> Result<Self, MoneyError> {
        match dollars.checked_mul(CENTS_PER_DOLLAR) {
            Some(cents) => Ok(Self(cents)),
            None => Err(MoneyError::Overflow),
        }
    }

    /// `self + rhs`.
    ///
    /// # Errors
    ///
    /// [`MoneyError::Overflow`].
    pub const fn checked_add(self, rhs: Self) -> Result<Self, MoneyError> {
        match self.0.checked_add(rhs.0) {
            Some(cents) => Ok(Self(cents)),
            None => Err(MoneyError::Overflow),
        }
    }

    /// `self - rhs`.
    ///
    /// # Errors
    ///
    /// [`MoneyError::Overflow`].
    pub const fn checked_sub(self, rhs: Self) -> Result<Self, MoneyError> {
        match self.0.checked_sub(rhs.0) {
            Some(cents) => Ok(Self(cents)),
            None => Err(MoneyError::Overflow),
        }
    }

    /// `-self`.
    ///
    /// # Errors
    ///
    /// [`MoneyError::Overflow`] for `Cents(i64::MIN)`.
    pub const fn checked_neg(self) -> Result<Self, MoneyError> {
        match self.0.checked_neg() {
            Some(cents) => Ok(Self(cents)),
            None => Err(MoneyError::Overflow),
        }
    }

    /// `self x r`, computed exactly in `i128` and rounded **exactly once** under
    /// `rule` (ADR-007, determinism rule D3).
    ///
    /// Under [`RoundingBasis::Amount`](crate::RoundingBasis::Amount) the product is
    /// rounded. Under
    /// [`RoundingBasis::IncreaseOverBase`](crate::RoundingBasis::IncreaseOverBase)
    /// `self` is the base, `r` the index factor, and the result is
    /// `self + round(self x r - self)`: the increase is rounded, once, and the
    /// unrounded base is added back (26 USC 1(f)(7)).
    ///
    /// # Errors
    ///
    /// [`MoneyError::ZeroDenominator`], [`MoneyError::NonPositiveIncrement`],
    /// [`MoneyError::UnspecifiedTie`], or [`MoneyError::Overflow`] when the
    /// rounded result does not fit `i64`. The `i128` intermediate itself cannot
    /// overflow for any `i64` inputs.
    pub fn checked_mul_ratio(self, r: Ratio, rule: &RoundingRule) -> Result<Self, MoneyError> {
        rule.mul_ratio(self, r)
    }

    /// [`Cents::checked_mul_ratio`], panicking where it errs. This is the
    /// design's `Cents::mul_ratio(self, r, rule) -> Cents`.
    ///
    /// # Panics
    ///
    /// On every error `checked_mul_ratio` returns, in every build profile.
    #[must_use]
    pub fn mul_ratio(self, r: Ratio, rule: &RoundingRule) -> Self {
        match self.checked_mul_ratio(r, rule) {
            Ok(product) => product,
            Err(e) => panic!("{self:?}.mul_ratio({r}, {rule:?}): {e}"),
        }
    }

    /// The growth step: `self x factor`, rounded half-even at the cent. One of the
    /// two named `f64`-to-money entry points (`ARCHITECTURE.md` D5(b),
    /// `SIMULATION-SPEC.md` §2.6); every application of a return, an inflation
    /// index or a half-year factor to money is a call to this function.
    ///
    /// The computation is one IEEE-754 multiplication and one round-to-nearest-
    /// even, both exactly specified, so the result is bit-identical on every
    /// architecture (D2, D6: no `mul_add`, no libm). An amount beyond 2^53 cents
    /// is not exactly representable as `f64` and is converted to the nearest one
    /// first; that is some ninety trillion dollars.
    ///
    /// # Panics
    ///
    /// When `factor` is NaN or infinite, or the grown amount does not fit `i64`,
    /// in every build profile. The factor comes from a generator or an
    /// assumption set and the balance from a validated plan, so either is a bug
    /// upstream, and a silently saturated balance would hide it.
    #[must_use]
    #[allow(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        reason = "the fenced f64 boundary: the i64 -> f64 conversion is documented above, and the f64 -> i64 conversion is range-checked first"
    )]
    pub fn grow(self, factor: f64) -> Self {
        assert!(
            factor.is_finite(),
            "{self:?}.grow({factor}): {}",
            MoneyError::NonFinite
        );
        let grown = (self.0 as f64 * factor).round_ties_even();
        // 2^63 exactly. Every f64 in [-2^63, 2^63) converts to i64 exactly.
        let limit = -(i64::MIN as f64);
        assert!(
            grown >= -limit && grown < limit,
            "{self:?}.grow({factor}): {}",
            MoneyError::Overflow
        );
        Self(grown as i64)
    }
}

impl Add for Cents {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        match self.checked_add(rhs) {
            Ok(sum) => sum,
            Err(e) => panic!("{self:?} + {rhs:?}: {e}"),
        }
    }
}

impl Sub for Cents {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self {
        match self.checked_sub(rhs) {
            Ok(difference) => difference,
            Err(e) => panic!("{self:?} - {rhs:?}: {e}"),
        }
    }
}

impl Neg for Cents {
    type Output = Self;

    fn neg(self) -> Self {
        match self.checked_neg() {
            Ok(negated) => negated,
            Err(e) => panic!("-{self:?}: {e}"),
        }
    }
}

impl AddAssign for Cents {
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

impl SubAssign for Cents {
    fn sub_assign(&mut self, rhs: Self) {
        *self = *self - rhs;
    }
}

impl Sum for Cents {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.fold(Self::ZERO, Add::add)
    }
}

impl<'a> Sum<&'a Cents> for Cents {
    fn sum<I: Iterator<Item = &'a Self>>(iter: I) -> Self {
        iter.copied().sum()
    }
}

/// Plain decimal dollars, `-1234.56`: no currency symbol and no grouping, which
/// are presentation and belong to the UI.
impl fmt::Display for Cents {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let magnitude = self.0.unsigned_abs();
        let per_dollar = CENTS_PER_DOLLAR.unsigned_abs();
        let sign = if self.0 < 0 { "-" } else { "" };
        write!(
            f,
            "{sign}{}.{:02}",
            magnitude / per_dollar,
            magnitude % per_dollar
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_is_the_integer_and_only_the_integer() {
        assert_eq!(
            serde_json::to_string(&Cents(12_500_000)).unwrap(),
            "12500000"
        );
        assert_eq!(serde_json::to_string(&Cents(-1)).unwrap(), "-1");
        assert_eq!(
            serde_json::from_str::<Cents>("12500000").unwrap(),
            Cents(12_500_000)
        );
        assert_eq!(
            serde_json::from_str::<Cents>("-9223372036854775808").unwrap(),
            Cents(i64::MIN)
        );
        for bad in [
            "1.0",
            "1.5",
            "1e2",
            "\"1\"",
            "null",
            "9223372036854775808",
            "[1]",
            "{}",
        ] {
            assert!(serde_json::from_str::<Cents>(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn display_is_plain_decimal_dollars() {
        assert_eq!(Cents(0).to_string(), "0.00");
        assert_eq!(Cents(5).to_string(), "0.05");
        assert_eq!(Cents(-5).to_string(), "-0.05");
        assert_eq!(Cents(123_456).to_string(), "1234.56");
        assert_eq!(Cents(-123_450).to_string(), "-1234.50");
        assert_eq!(Cents(i64::MAX).to_string(), "92233720368547758.07");
        assert_eq!(Cents(i64::MIN).to_string(), "-92233720368547758.08");
    }

    #[test]
    fn dollars_convert_or_err() {
        assert_eq!(Cents::from_dollars(125_000), Ok(Cents(12_500_000)));
        assert_eq!(Cents::from_dollars(-1), Ok(Cents(-100)));
        assert_eq!(
            Cents::from_dollars(i64::MAX / 100),
            Ok(Cents(i64::MAX / 100 * 100))
        );
        assert_eq!(
            Cents::from_dollars(i64::MAX / 100 + 1),
            Err(MoneyError::Overflow)
        );
        assert_eq!(Cents::from_dollars(i64::MIN), Err(MoneyError::Overflow));
    }

    #[test]
    fn checked_arithmetic_names_overflow() {
        assert_eq!(Cents(1).checked_add(Cents(2)), Ok(Cents(3)));
        assert_eq!(
            Cents(i64::MAX).checked_add(Cents(1)),
            Err(MoneyError::Overflow)
        );
        assert_eq!(
            Cents(i64::MIN).checked_sub(Cents(1)),
            Err(MoneyError::Overflow)
        );
        assert_eq!(Cents(i64::MIN).checked_neg(), Err(MoneyError::Overflow));
        assert_eq!(Cents(i64::MAX).checked_neg(), Ok(Cents(-i64::MAX)));
        let mut total = Cents(10);
        total += Cents(5);
        total -= Cents(20);
        assert_eq!(total, Cents(-5));
        assert_eq!(-total, Cents(5));
        assert_eq!(
            [Cents(1), Cents(2), Cents(3)].iter().sum::<Cents>(),
            Cents(6)
        );
        assert_eq!([Cents(1), Cents(2)].into_iter().sum::<Cents>(), Cents(3));
        assert_eq!(Cents(3).max(Cents(7)).min(Cents(5)), Cents(5));
    }

    #[test]
    #[should_panic(expected = "overflowed")]
    fn addition_never_wraps() {
        let _ = Cents(i64::MAX) + Cents(1);
    }

    #[test]
    #[should_panic(expected = "overflowed")]
    fn a_sum_never_wraps() {
        let _: Cents = [Cents(i64::MAX), Cents(1)].into_iter().sum();
    }

    #[test]
    #[should_panic(expected = "overflowed")]
    fn negation_never_wraps() {
        let _ = -Cents(i64::MIN);
    }

    // SYNTHETIC amounts and factors throughout (docs/contributing.md §1.3).
    #[test]
    fn grow_rounds_half_even_at_the_cent() {
        assert_eq!(Cents(100_000).grow(1.07), Cents(107_000));
        assert_eq!(Cents(0).grow(1.5), Cents(0));
        assert_eq!(Cents(123_456).grow(1.0), Cents(123_456));
        assert_eq!(Cents(123_456).grow(0.0), Cents(0));
        // Exact binary ties: 0.5, 1.5, 2.5 cents and their negatives.
        assert_eq!(Cents(1).grow(0.5), Cents(0));
        assert_eq!(Cents(3).grow(0.5), Cents(2));
        assert_eq!(Cents(5).grow(0.5), Cents(2));
        assert_eq!(Cents(7).grow(0.5), Cents(4));
        assert_eq!(Cents(-1).grow(0.5), Cents(0));
        assert_eq!(Cents(-3).grow(0.5), Cents(-2));
        assert_eq!(Cents(-5).grow(0.5), Cents(-2));
        // A loss, and a sign flip.
        assert_eq!(Cents(100_000).grow(0.63), Cents(63_000));
        assert_eq!(Cents(100_000).grow(-1.0), Cents(-100_000));
    }

    #[test]
    fn grow_reaches_the_i64_edges_without_wrapping() {
        assert_eq!(Cents(i64::MIN).grow(1.0), Cents(i64::MIN));
        assert_eq!(Cents(1 << 62).grow(-2.0), Cents(i64::MIN));
        assert_eq!(Cents(i64::MAX).grow(0.0), Cents(0));
    }

    #[test]
    #[should_panic(expected = "overflowed")]
    fn grow_past_i64_max_panics() {
        // i64::MAX converts to 2^63 as f64, which is one past the range.
        let _ = Cents(i64::MAX).grow(1.0);
    }

    #[test]
    #[should_panic(expected = "overflowed")]
    fn grow_past_i64_min_panics() {
        let _ = Cents(i64::MIN).grow(1.5);
    }

    #[test]
    #[should_panic(expected = "NaN or infinite")]
    fn grow_by_nan_panics() {
        let _ = Cents(1).grow(f64::NAN);
    }

    #[test]
    #[should_panic(expected = "NaN or infinite")]
    fn grow_by_infinity_panics() {
        let _ = Cents(0).grow(f64::INFINITY);
    }
}
