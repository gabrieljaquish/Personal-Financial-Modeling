//! `Ratio`: an exact statutory rate or factor (`DOMAIN-MODEL.md` §2.1, ADR-007).

use core::fmt;
use core::str::FromStr;

use serde::de::{self, Deserialize, Deserializer, MapAccess, Visitor};
use serde::Serialize;

/// An exact rational `num / den`.
///
/// Every statutory rate and factor is one of these, parsed from a decimal string
/// (`"0.0765"`) or a fraction (`"5/900"`) and never from a float. It deserializes
/// from either the string form or the object form `{"num":765,"den":10000}` and
/// **always serializes as the object form**, so canonical JSON is unambiguous.
///
/// Parsing does not reduce: `"0.10"` is `10/100`, exactly what was written, so a
/// trace shows the rate the source printed. Equality is therefore structural
/// (`1/2 != 5/10`); [`Ratio::reduced`] gives the lowest-terms form.
///
/// A parsed or deserialized `Ratio` always has `den > 0`. The fields are public
/// (the design's shape), so a hand-built value may not; the arithmetic in this
/// crate accepts a negative denominator and rejects a zero one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
pub struct Ratio {
    /// Numerator; carries the sign.
    pub num: i64,
    /// Denominator; positive in every parsed value.
    pub den: i64,
}

/// Why a string or a pair of integers is not a `Ratio`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RatioError {
    /// Not `[-]digits[.digits]` and not `[-]digits/digits`.
    Malformed,
    /// The denominator is zero.
    ZeroDenominator,
    /// The denominator is negative; the sign belongs on the numerator.
    NegativeDenominator,
    /// The numerator or the denominator does not fit `i64`.
    OutOfRange,
}

impl fmt::Display for RatioError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Malformed => {
                "malformed ratio: expected a decimal such as \"0.0765\" or a fraction such as \"5/900\""
            }
            Self::ZeroDenominator => "ratio has a zero denominator",
            Self::NegativeDenominator => {
                "ratio has a negative denominator; put the sign on the numerator"
            }
            Self::OutOfRange => "ratio numerator or denominator does not fit i64",
        })
    }
}

impl std::error::Error for RatioError {}

impl Ratio {
    /// `0/1`.
    pub const ZERO: Self = Self { num: 0, den: 1 };
    /// `1/1`.
    pub const ONE: Self = Self { num: 1, den: 1 };

    /// Builds `num/den`, requiring `den > 0`.
    ///
    /// # Errors
    ///
    /// [`RatioError::ZeroDenominator`] or [`RatioError::NegativeDenominator`].
    pub const fn new(num: i64, den: i64) -> Result<Self, RatioError> {
        if den == 0 {
            Err(RatioError::ZeroDenominator)
        } else if den < 0 {
            Err(RatioError::NegativeDenominator)
        } else {
            Ok(Self { num, den })
        }
    }

    /// The same value in lowest terms with a positive denominator, or `None` when
    /// `den == 0` or the reduced form does not fit `i64` (only `i64::MIN` inputs).
    #[must_use]
    pub fn reduced(self) -> Option<Self> {
        if self.den == 0 {
            return None;
        }
        let (mut num, mut den) = (i128::from(self.num), i128::from(self.den));
        if den < 0 {
            num = -num;
            den = -den;
        }
        let g = gcd(num.unsigned_abs(), den.unsigned_abs());
        let g = i128::try_from(g).ok()?;
        Some(Self {
            num: i64::try_from(num / g).ok()?,
            den: i64::try_from(den / g).ok()?,
        })
    }

    fn parse_decimal(s: &str) -> Result<Self, RatioError> {
        let (negative, body) = match s.strip_prefix('-') {
            Some(rest) => (true, rest),
            None => (false, s),
        };
        let (int_part, frac_part) = match body.split_once('.') {
            Some((i, f)) => (i, Some(f)),
            None => (body, None),
        };
        if !all_digits(int_part) || !frac_part.is_none_or(all_digits) {
            return Err(RatioError::Malformed);
        }
        let mut num: i128 = 0;
        let mut den: i128 = 1;
        let limit = i128::from(i64::MAX);
        for b in int_part.bytes() {
            num = num * 10 + i128::from(b - b'0');
            if num > limit + 1 {
                return Err(RatioError::OutOfRange);
            }
        }
        for b in frac_part.unwrap_or("").bytes() {
            num = num * 10 + i128::from(b - b'0');
            den *= 10;
            if num > limit + 1 || den > limit {
                return Err(RatioError::OutOfRange);
            }
        }
        if negative {
            num = -num;
        }
        Ok(Self {
            num: i64::try_from(num).map_err(|_| RatioError::OutOfRange)?,
            den: i64::try_from(den).map_err(|_| RatioError::OutOfRange)?,
        })
    }

    fn parse_fraction(num: &str, den: &str) -> Result<Self, RatioError> {
        let unsigned = num.strip_prefix('-').unwrap_or(num);
        if !all_digits(unsigned) {
            return Err(RatioError::Malformed);
        }
        if let Some(rest) = den.strip_prefix('-') {
            return Err(if all_digits(rest) {
                RatioError::NegativeDenominator
            } else {
                RatioError::Malformed
            });
        }
        if !all_digits(den) {
            return Err(RatioError::Malformed);
        }
        let num = num.parse::<i64>().map_err(|_| RatioError::OutOfRange)?;
        let den = den.parse::<i64>().map_err(|_| RatioError::OutOfRange)?;
        Self::new(num, den)
    }
}

/// Non-empty and ASCII digits only: no sign, no white space, no `_`, no exponent.
fn all_digits(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())
}

fn gcd(mut a: u128, mut b: u128) -> u128 {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}

impl FromStr for Ratio {
    type Err = RatioError;

    /// Parses `[-]digits[.digits]` or `[-]digits/digits`. Nothing else: no leading
    /// `+`, no white space, no exponent, no bare `.5` or `5.`.
    fn from_str(s: &str) -> Result<Self, RatioError> {
        match s.split_once('/') {
            Some((num, den)) => Self::parse_fraction(num, den),
            None => Self::parse_decimal(s),
        }
    }
}

impl fmt::Display for Ratio {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.num, self.den)
    }
}

impl<'de> Deserialize<'de> for Ratio {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(serde::Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Object {
            num: i64,
            den: i64,
        }

        struct Either;

        impl<'de> Visitor<'de> for Either {
            type Value = Ratio;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a ratio: {\"num\":765,\"den\":10000}, \"0.0765\" or \"5/900\"")
            }

            fn visit_str<E: de::Error>(self, v: &str) -> Result<Ratio, E> {
                v.parse().map_err(E::custom)
            }

            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Ratio, A::Error> {
                let Object { num, den } =
                    Object::deserialize(de::value::MapAccessDeserializer::new(map))?;
                Ratio::new(num, den).map_err(de::Error::custom)
            }
        }

        deserializer.deserialize_any(Either)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(num: i64, den: i64) -> Ratio {
        Ratio { num, den }
    }

    #[test]
    fn decimals_parse_exactly_and_unreduced() {
        assert_eq!("0.0765".parse(), Ok(r(765, 10_000)));
        assert_eq!("0.10".parse(), Ok(r(10, 100)));
        assert_eq!("0.9235".parse(), Ok(r(9235, 10_000)));
        assert_eq!("1".parse(), Ok(r(1, 1)));
        assert_eq!("0".parse(), Ok(r(0, 1)));
        assert_eq!("-0.025".parse(), Ok(r(-25, 1000)));
        assert_eq!("-0".parse(), Ok(r(0, 1)));
        assert_eq!("12.5".parse(), Ok(r(125, 10)));
        assert_eq!("007.50".parse(), Ok(r(750, 100)));
    }

    #[test]
    fn fractions_parse_exactly_and_unreduced() {
        assert_eq!("5/900".parse(), Ok(r(5, 900)));
        assert_eq!("5/1200".parse(), Ok(r(5, 1200)));
        assert_eq!("-2/3".parse(), Ok(r(-2, 3)));
        assert_eq!("0/7".parse(), Ok(r(0, 7)));
        assert_eq!(
            "9223372036854775807/9223372036854775807".parse(),
            Ok(r(i64::MAX, i64::MAX))
        );
        assert_eq!("-9223372036854775808/1".parse(), Ok(r(i64::MIN, 1)));
    }

    #[test]
    fn malformed_strings_are_rejected() {
        for s in [
            "", "-", ".", ".5", "5.", "+1", " 1", "1 ", "1e-3", "1_000", "0x10", "1.2.3", "1/2/3",
            "1/", "/2", "1.5/2", "1/2.5", "--1", "1/-", "abc", "0,5", "NaN", "inf", "１",
        ] {
            assert_eq!(s.parse::<Ratio>(), Err(RatioError::Malformed), "{s:?}");
        }
    }

    #[test]
    fn bad_denominators_and_ranges_are_named() {
        assert_eq!("1/0".parse::<Ratio>(), Err(RatioError::ZeroDenominator));
        assert_eq!(
            "1/-2".parse::<Ratio>(),
            Err(RatioError::NegativeDenominator)
        );
        assert_eq!(
            "9223372036854775808".parse::<Ratio>(),
            Err(RatioError::OutOfRange)
        );
        assert_eq!("-9223372036854775808".parse::<Ratio>(), Ok(r(i64::MIN, 1)));
        assert_eq!(
            "-9223372036854775809".parse::<Ratio>(),
            Err(RatioError::OutOfRange)
        );
        assert_eq!(
            "1/9223372036854775808".parse::<Ratio>(),
            Err(RatioError::OutOfRange)
        );
        // 19 fractional digits need a denominator of 10^19 > i64::MAX.
        assert_eq!(
            "0.0000000000000000001".parse::<Ratio>(),
            Err(RatioError::OutOfRange)
        );
        assert_eq!(
            "0.000000000000000001".parse::<Ratio>(),
            Ok(r(1, 1_000_000_000_000_000_000))
        );
        let long = "9".repeat(60);
        assert_eq!(long.parse::<Ratio>(), Err(RatioError::OutOfRange));
        assert_eq!(
            format!("0.{long}").parse::<Ratio>(),
            Err(RatioError::OutOfRange)
        );
        assert_eq!(Ratio::new(1, 0), Err(RatioError::ZeroDenominator));
        assert_eq!(Ratio::new(1, -1), Err(RatioError::NegativeDenominator));
        assert_eq!(Ratio::new(-1, 1), Ok(r(-1, 1)));
    }

    #[test]
    fn serde_reads_both_forms_and_writes_the_object_form() {
        let from_string: Ratio = serde_json::from_str("\"0.0765\"").unwrap();
        let from_fraction: Ratio = serde_json::from_str("\"5/900\"").unwrap();
        let from_object: Ratio = serde_json::from_str(r#"{"num":765,"den":10000}"#).unwrap();
        assert_eq!(from_string, r(765, 10_000));
        assert_eq!(from_object, from_string);
        assert_eq!(from_fraction, r(5, 900));
        assert_eq!(
            serde_json::to_string(&from_string).unwrap(),
            r#"{"num":765,"den":10000}"#
        );
        for bad in [
            "0.0765",
            "1",
            "null",
            r#"{"num":1}"#,
            r#"{"num":1,"den":0}"#,
            r#"{"num":1,"den":-2}"#,
            r#"{"num":1,"den":2,"extra":3}"#,
            r#"{"num":1.5,"den":2}"#,
            r#"{"num":"1","den":2}"#,
            r#""1/0""#,
            r#""abc""#,
            "[1,2]",
        ] {
            assert!(serde_json::from_str::<Ratio>(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn reduction_is_exact_and_total() {
        assert_eq!(r(5000, 10_000).reduced(), Some(r(1, 2)));
        assert_eq!(r(5, 900).reduced(), Some(r(1, 180)));
        assert_eq!(r(-6, 4).reduced(), Some(r(-3, 2)));
        assert_eq!(r(6, -4).reduced(), Some(r(-3, 2)));
        assert_eq!(r(0, 9).reduced(), Some(r(0, 1)));
        assert_eq!(r(1, 0).reduced(), None);
        assert_eq!(r(i64::MIN, i64::MIN).reduced(), Some(r(1, 1)));
        assert_eq!(r(i64::MIN, 2).reduced(), Some(r(i64::MIN / 2, 1)));
        // -(i64::MIN) does not fit: the sign cannot move to the numerator.
        assert_eq!(r(i64::MIN, -1).reduced(), None);
        assert_eq!(r(1, i64::MIN).reduced(), None);
    }

    #[test]
    fn display_is_the_fraction() {
        assert_eq!(r(-5, 900).to_string(), "-5/900");
    }
}
