//! `RoundingRule`: a rounding convention stored as data beside the parameter it
//! governs, never a global mode (ADR-007, `DOMAIN-MODEL.md` §15).

use serde::{Deserialize, Serialize};

use crate::cents::Cents;
use crate::error::MoneyError;
use crate::ratio::Ratio;

/// Which multiple of the increment an amount that is not already one rounds to.
///
/// Directions are defined on the signed number line, not on magnitudes: `Down` is
/// toward negative infinity ("the next lowest multiple", 26 USC 1(f)(7)(A)), not
/// truncation toward zero, and `Up` is toward positive infinity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RoundingDirection {
    /// The greatest multiple not above the amount (floor). Wire form `down`.
    Down,
    /// The least multiple not below the amount (ceiling). Wire form `up`.
    Up,
    /// The nearer multiple; an exact tie of a non-negative amount goes up. A tie
    /// of a negative amount is [`MoneyError::UnspecifiedTie`]: the design does
    /// not say whether it goes toward positive infinity or away from zero. Wire
    /// form `halfUp`.
    HalfUp,
    /// The nearer multiple; an exact tie goes to the even multiple. Wire form
    /// `halfEven`.
    HalfEven,
    /// The nearer multiple. An exact tie is [`MoneyError::UnspecifiedTie`]: the
    /// design names this direction beside `HalfUp` and `HalfEven`, so it is a
    /// synonym for neither, and states no tie rule for it. Wire form `nearest`.
    Nearest,
}

/// What the rounding is applied to.
///
/// The wire forms are the two spellings the design's parameter TOML and the
/// fixtures carry, `Amount` and `IncreaseOverBase`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum RoundingBasis {
    /// The amount itself is rounded.
    Amount,
    /// The **increase over the base** is rounded, once, and added back to the
    /// unrounded base (26 USC 1(f)(7)). The base is the receiver of
    /// [`Cents::mul_ratio`]; the ratio is the index factor.
    IncreaseOverBase,
}

/// `RoundingRule { increment, direction, basis }`.
///
/// `increment` is in **cents**, as its type says: `$50` is `Cents(5000)`. A
/// parameter table that writes its increment in its own unit converts when it
/// builds the rule. A deserialized rule always has a positive increment; the
/// fields are public (the design's shape), so the arithmetic re-checks it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "RawRule")]
pub struct RoundingRule {
    /// The multiple to round to, in cents. Positive.
    pub increment: Cents,
    /// Which neighbouring multiple is chosen.
    pub direction: RoundingDirection,
    /// Whether the amount or the increase over the base is rounded.
    pub basis: RoundingBasis,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRule {
    increment: Cents,
    direction: RoundingDirection,
    basis: RoundingBasis,
}

impl TryFrom<RawRule> for RoundingRule {
    type Error = MoneyError;

    fn try_from(raw: RawRule) -> Result<Self, MoneyError> {
        Self::new(raw.increment, raw.direction, raw.basis)
    }
}

impl RoundingRule {
    /// Builds a rule, requiring a positive increment.
    ///
    /// # Errors
    ///
    /// [`MoneyError::NonPositiveIncrement`].
    pub const fn new(
        increment: Cents,
        direction: RoundingDirection,
        basis: RoundingBasis,
    ) -> Result<Self, MoneyError> {
        if increment.0 <= 0 {
            return Err(MoneyError::NonPositiveIncrement);
        }
        Ok(Self {
            increment,
            direction,
            basis,
        })
    }

    /// Rounds `amount` itself to a multiple of the increment.
    ///
    /// `basis` is not consulted here: it says what [`Cents::mul_ratio`] feeds to
    /// the rounding, and a caller holding a bare amount has already decided.
    ///
    /// # Errors
    ///
    /// [`MoneyError::NonPositiveIncrement`], [`MoneyError::UnspecifiedTie`], or
    /// [`MoneyError::Overflow`] when the chosen multiple does not fit `i64`.
    pub fn checked_round(&self, amount: Cents) -> Result<Cents, MoneyError> {
        let rounded = self.round_rational(i128::from(amount.0), 1)?;
        i64::try_from(rounded)
            .map(Cents)
            .map_err(|_| MoneyError::Overflow)
    }

    /// [`RoundingRule::checked_round`], panicking where it errs.
    ///
    /// # Panics
    ///
    /// On every error `checked_round` returns, in every build profile.
    #[must_use]
    pub fn round(&self, amount: Cents) -> Cents {
        match self.checked_round(amount) {
            Ok(rounded) => rounded,
            Err(e) => panic!("RoundingRule::round({amount:?}) under {self:?}: {e}"),
        }
    }

    /// The single rounding step: the exact rational `n / d` cents, `d > 0`, to a
    /// multiple of the increment. Every rounding in the crate goes through here
    /// exactly once.
    fn round_rational(&self, n: i128, d: i128) -> Result<i128, MoneyError> {
        debug_assert!(d > 0);
        let increment = i128::from(self.increment.0);
        if increment <= 0 {
            return Err(MoneyError::NonPositiveIncrement);
        }
        // n/d = (q + r/step) increments, with 0 <= r < step: floor division.
        let step = d.checked_mul(increment).ok_or(MoneyError::Overflow)?;
        let q = n.div_euclid(step);
        let r = n.rem_euclid(step);
        let tie = MoneyError::UnspecifiedTie {
            direction: self.direction,
        };
        // `r` against `step - r` compares the remainder with one half exactly and
        // cannot overflow, where `2 * r` could.
        let above_half = r > step - r;
        let exact_tie = r == step - r;
        let round_up = match self.direction {
            RoundingDirection::Down => false,
            RoundingDirection::Up => r != 0,
            RoundingDirection::HalfUp if exact_tie && n < 0 => return Err(tie),
            RoundingDirection::HalfUp => above_half || exact_tie,
            RoundingDirection::HalfEven => above_half || (exact_tie && q.rem_euclid(2) == 1),
            RoundingDirection::Nearest if exact_tie => return Err(tie),
            RoundingDirection::Nearest => above_half,
        };
        q.checked_add(i128::from(round_up))
            .and_then(|multiple| multiple.checked_mul(increment))
            .ok_or(MoneyError::Overflow)
    }

    /// `base x r` under this rule: the body of [`Cents::checked_mul_ratio`].
    pub(crate) fn mul_ratio(&self, base: Cents, r: Ratio) -> Result<Cents, MoneyError> {
        if r.den == 0 {
            return Err(MoneyError::ZeroDenominator);
        }
        let (mut num, mut den) = (i128::from(r.num), i128::from(r.den));
        if den < 0 {
            num = -num;
            den = -den;
        }
        let base = i128::from(base.0);
        let result = match self.basis {
            RoundingBasis::Amount => {
                let product = base.checked_mul(num).ok_or(MoneyError::Overflow)?;
                self.round_rational(product, den)?
            }
            RoundingBasis::IncreaseOverBase => {
                // base x r - base = base x (num - den) / den, exactly.
                let increase = base.checked_mul(num - den).ok_or(MoneyError::Overflow)?;
                self.round_rational(increase, den)?
                    .checked_add(base)
                    .ok_or(MoneyError::Overflow)?
            }
        };
        i64::try_from(result)
            .map(Cents)
            .map_err(|_| MoneyError::Overflow)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use RoundingBasis::{Amount, IncreaseOverBase};
    use RoundingDirection::{Down, HalfEven, HalfUp, Nearest, Up};

    fn rule(increment: i64, direction: RoundingDirection) -> RoundingRule {
        RoundingRule::new(Cents(increment), direction, Amount).unwrap()
    }

    #[test]
    fn wire_forms_are_the_designs() {
        let json = |d: RoundingDirection| serde_json::to_string(&d).unwrap();
        assert_eq!(json(Down), "\"down\"");
        assert_eq!(json(Up), "\"up\"");
        assert_eq!(json(HalfUp), "\"halfUp\"");
        assert_eq!(json(HalfEven), "\"halfEven\"");
        assert_eq!(json(Nearest), "\"nearest\"");
        assert_eq!(serde_json::to_string(&Amount).unwrap(), "\"Amount\"");
        assert_eq!(
            serde_json::to_string(&IncreaseOverBase).unwrap(),
            "\"IncreaseOverBase\""
        );
        for bad in ["\"Down\"", "\"half_up\"", "\"truncate\"", "\"amount\"", "0"] {
            assert!(
                serde_json::from_str::<RoundingDirection>(bad).is_err(),
                "{bad}"
            );
        }
        for bad in ["\"amount\"", "\"increaseOverBase\"", "\"Total\""] {
            assert!(serde_json::from_str::<RoundingBasis>(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn a_rule_round_trips_and_rejects_a_bad_increment() {
        let r = RoundingRule::new(Cents(5000), Down, IncreaseOverBase).unwrap();
        let json = serde_json::to_string(&r).unwrap();
        assert_eq!(
            json,
            r#"{"increment":5000,"direction":"down","basis":"IncreaseOverBase"}"#
        );
        assert_eq!(serde_json::from_str::<RoundingRule>(&json).unwrap(), r);
        for bad in [
            r#"{"increment":0,"direction":"down","basis":"Amount"}"#,
            r#"{"increment":-50,"direction":"down","basis":"Amount"}"#,
            r#"{"increment":50.0,"direction":"down","basis":"Amount"}"#,
            r#"{"increment":50,"direction":"down"}"#,
            r#"{"increment":50,"direction":"down","basis":"Amount","x":1}"#,
        ] {
            assert!(serde_json::from_str::<RoundingRule>(bad).is_err(), "{bad}");
        }
        assert_eq!(
            RoundingRule::new(Cents(0), Down, Amount),
            Err(MoneyError::NonPositiveIncrement)
        );
        let hand_built = RoundingRule {
            increment: Cents(-1),
            direction: Down,
            basis: Amount,
        };
        assert_eq!(
            hand_built.checked_round(Cents(5)),
            Err(MoneyError::NonPositiveIncrement)
        );
    }

    #[test]
    fn the_two_unspecified_ties_are_errors_and_nothing_else_is() {
        let tie = |direction| Err(MoneyError::UnspecifiedTie { direction });
        // SYNTHETIC amounts. Nearest: a tie either side of zero is unspecified.
        assert_eq!(rule(100, Nearest).checked_round(Cents(150)), tie(Nearest));
        assert_eq!(rule(100, Nearest).checked_round(Cents(-150)), tie(Nearest));
        assert_eq!(rule(100, Nearest).checked_round(Cents(149)), Ok(Cents(100)));
        assert_eq!(rule(100, Nearest).checked_round(Cents(151)), Ok(Cents(200)));
        assert_eq!(
            rule(100, Nearest).checked_round(Cents(-149)),
            Ok(Cents(-100))
        );
        assert_eq!(
            rule(100, Nearest).checked_round(Cents(-151)),
            Ok(Cents(-200))
        );
        // HalfUp: only the negative tie is unspecified.
        assert_eq!(rule(100, HalfUp).checked_round(Cents(150)), Ok(Cents(200)));
        assert_eq!(rule(100, HalfUp).checked_round(Cents(-150)), tie(HalfUp));
        assert_eq!(
            rule(100, HalfUp).checked_round(Cents(-149)),
            Ok(Cents(-100))
        );
        assert_eq!(
            rule(100, HalfUp).checked_round(Cents(-151)),
            Ok(Cents(-200))
        );
        // HalfEven is specified everywhere.
        assert_eq!(
            rule(100, HalfEven).checked_round(Cents(-150)),
            Ok(Cents(-200))
        );
        assert_eq!(
            rule(100, HalfEven).checked_round(Cents(-250)),
            Ok(Cents(-200))
        );
        // An odd increment has no ties at whole cents.
        assert_eq!(rule(3, Nearest).checked_round(Cents(4)), Ok(Cents(3)));
        assert_eq!(rule(3, Nearest).checked_round(Cents(5)), Ok(Cents(6)));
    }

    #[test]
    fn rounding_at_the_i64_edges_errs_instead_of_wrapping() {
        assert_eq!(
            rule(100, Up).checked_round(Cents(i64::MAX)),
            Err(MoneyError::Overflow)
        );
        assert_eq!(
            rule(100, Down).checked_round(Cents(i64::MIN)),
            Err(MoneyError::Overflow)
        );
        assert_eq!(
            rule(100, Down).checked_round(Cents(i64::MAX)),
            Ok(Cents(i64::MAX - i64::MAX % 100))
        );
        assert_eq!(
            rule(1, HalfEven).checked_round(Cents(i64::MIN)),
            Ok(Cents(i64::MIN))
        );
        assert_eq!(
            rule(i64::MAX, Down).checked_round(Cents(i64::MAX)),
            Ok(Cents(i64::MAX))
        );
        assert_eq!(
            rule(i64::MAX, Down).checked_round(Cents(-1)),
            Ok(Cents(-i64::MAX))
        );
        assert_eq!(
            rule(i64::MAX, Up).checked_round(Cents(1)),
            Ok(Cents(i64::MAX))
        );
    }

    #[test]
    #[should_panic(expected = "exact tie under rounding direction Nearest")]
    fn the_infallible_form_panics_loudly() {
        let _ = rule(100, Nearest).round(Cents(150));
    }
}
