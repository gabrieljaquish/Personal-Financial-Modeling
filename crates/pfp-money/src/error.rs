//! The one error type of the money arithmetic.

use core::fmt;

use crate::rounding::RoundingDirection;

/// Why a money operation has no answer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MoneyError {
    /// The exact result does not fit `Cents(i64)`.
    Overflow,
    /// A `Ratio` with `den == 0`.
    ZeroDenominator,
    /// A `RoundingRule` whose increment is zero or negative.
    NonPositiveIncrement,
    /// The amount is exactly half-way between two multiples of the increment and
    /// the design does not say which one this direction picks: `Nearest` at any
    /// tie, `HalfUp` at a tie of a negative amount.
    UnspecifiedTie {
        /// The direction whose tie rule is unspecified.
        direction: RoundingDirection,
    },
    /// A growth factor that is NaN or infinite.
    NonFinite,
}

impl fmt::Display for MoneyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Overflow => f.write_str("money arithmetic overflowed the i64 cent range"),
            Self::ZeroDenominator => f.write_str("ratio has a zero denominator"),
            Self::NonPositiveIncrement => {
                f.write_str("rounding increment must be a positive number of cents")
            }
            Self::UnspecifiedTie { direction } => write!(
                f,
                "exact tie under rounding direction {direction:?}, whose tie rule the design does not specify"
            ),
            Self::NonFinite => f.write_str("growth factor is NaN or infinite"),
        }
    }
}

impl std::error::Error for MoneyError {}
