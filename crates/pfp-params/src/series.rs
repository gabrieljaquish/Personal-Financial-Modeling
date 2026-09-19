//! An archived price-index series (`params/index-series/*.toml`).
//!
//! No design document gives this table a shape (`VINTAGE.md`, deviation 4). What
//! is read here: `id`, `kind = "index-series"`, `index_series` (the name tables
//! resolve against), `[window]`, `[observations.<calendar year>]`, an optional
//! checked `[window_sum]`, and the same `as_of`, `verification`,
//! `[hand_verification]` and `[[source]]` blocks a parameter table carries.
//!
//! Observations are decimal strings, parsed digit for digit into scaled integers.
//! A projection needs only the **ratio** of two calendar-year index values, in
//! which the averaging divisor cancels, so the series carries exact window
//! **sums** and never forms a (possibly non-terminating) mean.

use std::collections::BTreeMap;

use pfp_domain::Year;
use pfp_money::Ratio;
use toml::{Table, Value};

use crate::error::ParamError;
use crate::provenance::{
    read_open_items, read_sources, req_date, DateYmd, Source, VerificationStatus,
};
use crate::raw::{document_id, parse_document, At};

const KIND: &str = "index-series";
const WINDOW_KIND: &str = "trailing-12-month-mean";
const MONTHS_PER_YEAR: i64 = 12;
/// More fractional digits than any published index carries, and few enough that
/// a twelve-month sum of scaled observations cannot approach `i128`.
const MAX_SCALE: u32 = 9;

const TOP_FIELDS: [&str; 16] = [
    "id",
    "kind",
    "index_series",
    "series_id",
    "unit",
    "base_period",
    "periodicity",
    "as_of",
    "verification",
    "statutory_name",
    "window",
    "observations",
    "window_sum",
    "revision_vintage",
    "hand_verification",
    "source",
];

/// An archived index series: exact calendar-year window sums plus provenance.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IndexSeries {
    id: String,
    /// The name parameter tables write in `projection.index_series`.
    name: String,
    series_id: Option<String>,
    as_of: DateYmd,
    verification: VerificationStatus,
    months: u32,
    ends_month: u32,
    /// Every sum is an integer count of `10^-scale` index points.
    scale: u32,
    sums: BTreeMap<Year, i128>,
    sources: Vec<Source>,
    open_items: Vec<String>,
}

impl IndexSeries {
    /// Parses and validates one index-series document.
    ///
    /// Each calendar-year window must hold exactly the months the `[window]`
    /// block names (for the shipped series, September of the preceding year
    /// through August), and a declared `window_sum` must equal the exact sum of
    /// the observations.
    ///
    /// # Errors
    ///
    /// A [`ParamError`] naming the series and the field.
    pub fn parse(text: &str) -> Result<Self, ParamError> {
        let root = parse_document(text)?;
        let id = document_id(&root)?.to_owned();
        let at = At {
            table: &id,
            path: "",
        };
        at.deny_unknown(&root, &TOP_FIELDS)?;
        if at.req_str(&root, "kind")? != KIND {
            return Err(at.invalid("kind", "is not \"index-series\""));
        }
        let name = at.req_str(&root, "index_series")?.to_owned();
        if name.is_empty() {
            return Err(at.invalid("index_series", "is empty"));
        }
        let (months, ends_month) = read_window(&id, &root)?;
        let windows = read_observations(&id, &root, months, ends_month)?;
        let scale = windows
            .values()
            .flatten()
            .map(|(_, scale)| *scale)
            .max()
            .unwrap_or(0);
        let sums: BTreeMap<Year, i128> = windows
            .iter()
            .map(|(year, obs)| {
                let sum = obs
                    .iter()
                    .map(|(digits, s)| digits * 10_i128.pow(scale - s))
                    .sum();
                (*year, sum)
            })
            .collect();
        check_window_sums(&id, &root, scale, &sums)?;
        Ok(Self {
            series_id: at.opt_str(&root, "series_id")?.map(str::to_owned),
            as_of: req_date(at, &root, "as_of")?,
            verification: VerificationStatus::read(at, &root)?,
            months,
            ends_month,
            scale,
            sums,
            sources: read_sources(&id, &root)?,
            open_items: read_open_items(&id, &root)?,
            name,
            id,
        })
    }

    /// The table id, e.g. `bls.cpi.chained.suur0000sa0`.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// The name a parameter table's `projection.index_series` resolves against,
    /// e.g. `cpi.chained.aug12m`. The series document declares it, so the binding
    /// of a table to its archived series is data and not code.
    #[must_use]
    pub fn index_series(&self) -> &str {
        &self.name
    }

    /// The publisher's series identifier, if recorded.
    #[must_use]
    pub fn series_id(&self) -> Option<&str> {
        self.series_id.as_deref()
    }

    /// The snapshot date of the archived series.
    #[must_use]
    pub const fn as_of(&self) -> DateYmd {
        self.as_of
    }

    /// Whether a human has read the observations back.
    #[must_use]
    pub const fn verification(&self) -> VerificationStatus {
        self.verification
    }

    /// The archived primary documents.
    #[must_use]
    pub fn sources(&self) -> &[Source] {
        &self.sources
    }

    /// Open hand-verification items recorded beside the series.
    #[must_use]
    pub fn open_items(&self) -> &[String] {
        &self.open_items
    }

    /// Months per window and the calendar month each window ends in.
    #[must_use]
    pub const fn window(&self) -> (u32, u32) {
        (self.months, self.ends_month)
    }

    /// The calendar years with a complete archived window, ascending.
    pub fn years(&self) -> impl Iterator<Item = Year> + '_ {
        self.sums.keys().copied()
    }

    /// The exact sum of the window's observations for `calendar_year`, as
    /// `digits / 10^scale` (not reduced). `None` when the window is not archived
    /// or the sum does not fit a [`Ratio`].
    #[must_use]
    pub fn window_sum(&self, calendar_year: Year) -> Option<Ratio> {
        let sum = i64::try_from(*self.sums.get(&calendar_year)?).ok()?;
        let den = i64::try_from(10_i128.pow(self.scale)).ok()?;
        Ratio::new(sum, den).ok()
    }

    /// `index(numerator_year) / index(denominator_year)`, exactly and in lowest
    /// terms. The averaging divisor cancels, so this is the ratio of the two
    /// window sums. `None` when either window is not archived, the denominator
    /// sum is not positive, or the reduced ratio does not fit `i64`.
    #[must_use]
    pub fn factor(&self, numerator_year: Year, denominator_year: Year) -> Option<Ratio> {
        let num = *self.sums.get(&numerator_year)?;
        let den = *self.sums.get(&denominator_year)?;
        if den <= 0 {
            return None;
        }
        let g = i128::try_from(gcd(num.unsigned_abs(), den.unsigned_abs())).ok()?;
        Ratio::new(i64::try_from(num / g).ok()?, i64::try_from(den / g).ok()?).ok()
    }

    /// Whether `calendar_year` has an archived window.
    #[must_use]
    pub fn has_year(&self, calendar_year: Year) -> bool {
        self.sums.contains_key(&calendar_year)
    }
}

fn gcd(mut a: u128, mut b: u128) -> u128 {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}

fn read_window(id: &str, root: &Table) -> Result<(u32, u32), ParamError> {
    let top = At {
        table: id,
        path: "",
    };
    let at = At {
        table: id,
        path: "window",
    };
    let window = top
        .opt_table(root, "window")?
        .ok_or_else(|| top.missing("window"))?;
    if at.req_str(window, "kind")? != WINDOW_KIND {
        return Err(at.invalid("kind", "is not \"trailing-12-month-mean\""));
    }
    let ranged = |key: &str| -> Result<u32, ParamError> {
        let raw = at.opt_int(window, key)?.ok_or_else(|| at.missing(key))?;
        u32::try_from(raw)
            .ok()
            .filter(|m| (1..=12).contains(m))
            .ok_or_else(|| at.invalid(key, "is not in 1..=12"))
    };
    Ok((ranged("months")?, ranged("ends_month")?))
}

/// `[observations.<calendar year>]`: each observation as `(digits, scale)`.
type Windows = BTreeMap<Year, Vec<(i128, u32)>>;

fn read_observations(
    id: &str,
    root: &Table,
    months: u32,
    ends_month: u32,
) -> Result<Windows, ParamError> {
    let top = At {
        table: id,
        path: "",
    };
    let at = At {
        table: id,
        path: "observations",
    };
    let block = top
        .opt_table(root, "observations")?
        .ok_or_else(|| top.missing("observations"))?;
    let mut windows = Windows::new();
    for (year_key, value) in block {
        let year = at.year_key(year_key)?;
        let path = at.field(year_key);
        let inner = At {
            table: id,
            path: &path,
        };
        let Value::Table(obs) = value else {
            return Err(at.wrong(year_key, value, "a table of monthly observations"));
        };
        let expected = expected_months(year, months, ends_month);
        if obs.len() != expected.len() {
            return Err(ParamError::WidthMismatch {
                table: id.to_owned(),
                field: path.clone(),
                expected: expected.len(),
                found: obs.len(),
            });
        }
        let mut parsed = Vec::with_capacity(expected.len());
        for month in &expected {
            let text = inner.req_str(obs, month)?;
            parsed.push(
                parse_decimal(text)
                    .ok_or_else(|| inner.invalid(month, "is not a non-negative decimal string"))?,
            );
        }
        windows.insert(year, parsed);
    }
    Ok(windows)
}

/// The observation keys (`YYYY-Mmm`) of the window ending in `ends_month` of `year`.
fn expected_months(year: Year, months: u32, ends_month: u32) -> Vec<String> {
    let last = i64::from(year) * MONTHS_PER_YEAR + i64::from(ends_month) - 1;
    (0..i64::from(months))
        .rev()
        .map(|back| {
            let m = last - back;
            format!(
                "{:04}-M{:02}",
                m.div_euclid(MONTHS_PER_YEAR),
                m.rem_euclid(MONTHS_PER_YEAR) + 1
            )
        })
        .collect()
}

/// `digits[.digits]`, no sign, no exponent: `(all digits as an integer, scale)`.
fn parse_decimal(s: &str) -> Option<(i128, u32)> {
    let (int_part, frac_part) = s.split_once('.').unwrap_or((s, ""));
    let plain = |p: &str| p.bytes().all(|b| b.is_ascii_digit());
    if int_part.is_empty() || !plain(int_part) || !plain(frac_part) {
        return None;
    }
    if s.contains('.') && frac_part.is_empty() {
        return None;
    }
    let scale = u32::try_from(frac_part.len())
        .ok()
        .filter(|s| *s <= MAX_SCALE)?;
    if int_part.len() > 18 {
        return None;
    }
    let digits: i128 = format!("{int_part}{frac_part}").parse().ok()?;
    Some((digits, scale))
}

fn check_window_sums(
    id: &str,
    root: &Table,
    scale: u32,
    sums: &BTreeMap<Year, i128>,
) -> Result<(), ParamError> {
    let top = At {
        table: id,
        path: "",
    };
    let at = At {
        table: id,
        path: "window_sum",
    };
    let Some(declared) = top.opt_table(root, "window_sum")? else {
        return Ok(());
    };
    for (year_key, value) in declared {
        let year = at.year_key(year_key)?;
        let Value::String(text) = value else {
            return Err(at.wrong(year_key, value, "a decimal string"));
        };
        let (digits, s) = parse_decimal(text)
            .ok_or_else(|| at.invalid(year_key, "is not a non-negative decimal string"))?;
        // Compare on the common scale; a declared sum may carry more digits.
        let common = scale.max(s);
        let lhs = digits * 10_i128.pow(common - s);
        let rhs = sums.get(&year).map(|sum| sum * 10_i128.pow(common - scale));
        if rhs != Some(lhs) {
            return Err(ParamError::WindowSumMismatch {
                table: id.to_owned(),
                year,
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_trailing_window_names_the_right_months() {
        let w = expected_months(2016, 12, 8);
        assert_eq!(w.first().unwrap(), "2015-M09");
        assert_eq!(w.last().unwrap(), "2016-M08");
        assert_eq!(w.len(), 12);
        let calendar = expected_months(2016, 12, 12);
        assert_eq!(calendar.first().unwrap(), "2016-M01");
        assert_eq!(calendar.last().unwrap(), "2016-M12");
        assert_eq!(
            expected_months(2020, 3, 1),
            ["2019-M11", "2019-M12", "2020-M01"]
        );
    }

    #[test]
    fn decimals_are_read_digit_for_digit() {
        assert_eq!(parse_decimal("135.837"), Some((135_837, 3)));
        assert_eq!(parse_decimal("171.490"), Some((171_490, 3)));
        assert_eq!(parse_decimal("100"), Some((100, 0)));
        for bad in [
            "",
            ".5",
            "5.",
            "-1.0",
            "+1",
            "1e3",
            "1_0",
            " 1",
            "1.0000000000",
        ] {
            assert_eq!(parse_decimal(bad), None, "{bad:?}");
        }
    }
}
