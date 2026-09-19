//! The two error types of the parameter layer: [`ParamError`] when text is turned
//! into typed tables, [`ProjectionError`] when a loaded table is asked for a value.

use core::fmt;

use pfp_domain::Year;
use pfp_money::MoneyError;

/// Why parameter text is not a valid table, index series or vintage.
///
/// Every variant names the table (its `id`, or `<no id>` before one is read) and
/// the dotted path of the offending field, so a CI failure points at a line a
/// human can find (`TESTING.md` §11.2 gate 9).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParamError {
    /// The text is not TOML.
    Syntax {
        /// The parser's message, including line and column.
        message: String,
    },
    /// A required field is absent.
    Missing {
        /// Table id.
        table: String,
        /// Dotted field path.
        field: String,
    },
    /// A field holds the wrong TOML type.
    WrongType {
        /// Table id.
        table: String,
        /// Dotted field path.
        field: String,
        /// What the loader expected there.
        expected: &'static str,
    },
    /// A TOML float. Amounts are integers and rates and index observations are
    /// decimal strings: binary floating point never enters a parameter
    /// (`ARCHITECTURE.md` §4.3 D3, ADR-007).
    Float {
        /// Table id.
        table: String,
        /// Dotted field path.
        field: String,
    },
    /// A field the parameter-table shape does not define (seam S1 is closed: an
    /// unknown key is a typo or an unreviewed shape extension, never ignored).
    UnknownField {
        /// Table id.
        table: String,
        /// Dotted field path.
        field: String,
    },
    /// A field of the right type whose value is not acceptable.
    Invalid {
        /// Table id.
        table: String,
        /// Dotted field path.
        field: String,
        /// Why.
        reason: String,
    },
    /// A `breakdown` this loader has no enum for. M0 knows `["filingStatus"]`.
    UnsupportedBreakdown {
        /// Table id.
        table: String,
        /// The declared breakdown, joined with `,`.
        breakdown: String,
    },
    /// A breakdown key that is not a wire form of the breakdown's enum
    /// (`DOMAIN-MODEL.md` §15): for `filingStatus`, not one of
    /// `single | mfj | mfs | hoh | qss`.
    UnknownBreakdownKey {
        /// Table id.
        table: String,
        /// The map the key was found in (`values`, `projection.base_values`, ...).
        field: String,
        /// The offending key.
        key: String,
    },
    /// A wire form of the breakdown's enum with no entry. An absent key would
    /// price that status at zero, so it is an error, never a default.
    MissingBreakdownKey {
        /// Table id.
        table: String,
        /// The map the key is missing from.
        field: String,
        /// The missing wire form.
        key: String,
    },
    /// The table has no `[projection]` block or no `projection.rule`
    /// ("a table without a projection rule is a CI error").
    MissingProjectionRule {
        /// Table id.
        table: String,
    },
    /// An indexed table without a `[projection.rounding]` rule.
    MissingRounding {
        /// Table id.
        table: String,
    },
    /// `basis = "IncreaseOverBase"` without every input that basis needs. The
    /// increase over the statutory base year cannot be computed without the base
    /// year, the base amounts and the archived index series.
    IncreaseOverBaseIncomplete {
        /// Table id.
        table: String,
        /// Which of `base_year`, `base_values`, `index_series` are absent.
        missing: Vec<&'static str>,
    },
    /// No `[[source]]` entry at all.
    MissingProvenance {
        /// Table id.
        table: String,
    },
    /// An array whose length does not match the table's declared components.
    WidthMismatch {
        /// Table id.
        table: String,
        /// Dotted field path.
        field: String,
        /// The required length.
        expected: usize,
        /// The length found.
        found: usize,
    },
    /// Two fields that say the same thing two ways (`increment` and
    /// `increment_by_key`, `base_year` and `base_year_by_edge`).
    Conflict {
        /// Table id.
        table: String,
        /// The first field.
        first: &'static str,
        /// The second field.
        second: &'static str,
    },
    /// An index series whose declared `window_sum` is not the exact sum of its
    /// observations.
    WindowSumMismatch {
        /// Series id.
        table: String,
        /// The calendar year of the window.
        year: Year,
    },
    /// Two tables of one vintage share an id.
    DuplicateTable {
        /// The repeated id.
        table: String,
    },
    /// A table's `projection.index_series` names a series the vintage was not
    /// given: an index that is fetched rather than archived is a gate-9 error.
    UnresolvedIndexSeries {
        /// Table id.
        table: String,
        /// The unresolved `index_series` name.
        index_series: String,
    },
    /// The vintage id recorded at lock time is not the id of the documents parsed.
    VintageIdMismatch {
        /// The `<name>@<sha256>` the lock records.
        recorded: String,
        /// The `<name>@<sha256>` of the documents supplied.
        computed: String,
    },
}

impl fmt::Display for ParamError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Syntax { message } => write!(f, "parameter text is not TOML: {message}"),
            Self::Missing { table, field } => write!(f, "{table}: `{field}` is required"),
            Self::WrongType {
                table,
                field,
                expected,
            } => write!(f, "{table}: `{field}` must be {expected}"),
            Self::Float { table, field } => write!(
                f,
                "{table}: `{field}` is a TOML float; write an integer or a decimal string"
            ),
            Self::UnknownField { table, field } => {
                write!(
                    f,
                    "{table}: `{field}` is not a field of the parameter-table shape"
                )
            }
            Self::Invalid {
                table,
                field,
                reason,
            } => write!(f, "{table}: `{field}` {reason}"),
            Self::UnsupportedBreakdown { table, breakdown } => {
                write!(f, "{table}: breakdown [{breakdown}] is not supported")
            }
            Self::UnknownBreakdownKey { table, field, key } => write!(
                f,
                "{table}: `{field}` key `{key}` is not a wire form of the declared breakdown"
            ),
            Self::MissingBreakdownKey { table, field, key } => {
                write!(f, "{table}: `{field}` has no entry for `{key}`")
            }
            Self::MissingProjectionRule { table } => write!(
                f,
                "{table}: no `projection.rule`; never-indexed tables say `rule = \"flat\"`"
            ),
            Self::MissingRounding { table } => {
                write!(f, "{table}: an indexed table needs `[projection.rounding]`")
            }
            Self::IncreaseOverBaseIncomplete { table, missing } => write!(
                f,
                "{table}: basis IncreaseOverBase needs `projection.{}`",
                missing.join("`, `projection.")
            ),
            Self::MissingProvenance { table } => {
                write!(f, "{table}: no `[[source]]` provenance")
            }
            Self::WidthMismatch {
                table,
                field,
                expected,
                found,
            } => write!(
                f,
                "{table}: `{field}` has {found} entries where {expected} are required"
            ),
            Self::Conflict {
                table,
                first,
                second,
            } => write!(
                f,
                "{table}: `{first}` and `{second}` are mutually exclusive"
            ),
            Self::WindowSumMismatch { table, year } => write!(
                f,
                "{table}: `window_sum.{year}` is not the exact sum of `observations.{year}`"
            ),
            Self::DuplicateTable { table } => write!(f, "{table}: table id appears twice"),
            Self::UnresolvedIndexSeries {
                table,
                index_series,
            } => write!(
                f,
                "{table}: index_series `{index_series}` resolves to no archived series"
            ),
            Self::VintageIdMismatch { recorded, computed } => write!(
                f,
                "vintage id mismatch: the lock records `{recorded}`, the documents hash to `{computed}`"
            ),
        }
    }
}

impl std::error::Error for ParamError {}

/// Why a loaded vintage has no value for a request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProjectionError {
    /// No table with this id.
    UnknownTable {
        /// The requested id.
        table: String,
    },
    /// The table has no such breakdown key.
    UnknownKey {
        /// Table id.
        table: String,
        /// The requested key.
        key: String,
    },
    /// The table has no `[rates]` ladder, or none at or before the year.
    NoRates {
        /// Table id.
        table: String,
        /// The requested year.
        year: Year,
    },
    /// A projection rule that is part of the frozen shape but has no
    /// implementation yet (`wage`, `zero`, `schedule` at M0).
    UnsupportedRule {
        /// Table id.
        table: String,
        /// The rule's wire form.
        rule: &'static str,
    },
    /// `rule = "flat"` and no published year at or before the requested one.
    NothingToCarry {
        /// Table id.
        table: String,
        /// The requested year.
        year: Year,
    },
    /// No base year is recorded for the requested tax year.
    NoBaseYear {
        /// Table id.
        table: String,
        /// The requested year.
        year: Year,
    },
    /// The archived series has no window for a calendar year the rule reads.
    /// Beyond the archive the index comes from the projection's inflation path
    /// (`ENGINE-SPEC.md` §1.3), which is not an M0 input.
    IndexUnavailable {
        /// The `index_series` name.
        index_series: String,
        /// The calendar year whose window is missing.
        calendar_year: Year,
    },
    /// The index for the year read lies below the index for the base year. The
    /// statute adjusts by "the percentage (if any) by which" the index exceeds
    /// the base; what a table does then is a reading nobody has made yet, so it
    /// is an error rather than a guess.
    IndexBelowBase {
        /// Table id.
        table: String,
        /// The requested year.
        year: Year,
    },
    /// An index factor or a derived amount that is not exactly representable.
    Inexact {
        /// Table id.
        table: String,
        /// What could not be represented.
        what: &'static str,
    },
    /// The money arithmetic failed.
    Money(MoneyError),
}

impl fmt::Display for ProjectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownTable { table } => write!(f, "no parameter table `{table}`"),
            Self::UnknownKey { table, key } => write!(f, "{table}: no breakdown key `{key}`"),
            Self::NoRates { table, year } => write!(f, "{table}: no rate ladder for {year}"),
            Self::UnsupportedRule { table, rule } => {
                write!(
                    f,
                    "{table}: projection rule `{rule}` is not implemented yet"
                )
            }
            Self::NothingToCarry { table, year } => {
                write!(f, "{table}: no published year at or before {year}")
            }
            Self::NoBaseYear { table, year } => {
                write!(f, "{table}: no base year recorded for tax year {year}")
            }
            Self::IndexUnavailable {
                index_series,
                calendar_year,
            } => write!(
                f,
                "index series `{index_series}` has no archived window for {calendar_year}"
            ),
            Self::IndexBelowBase { table, year } => write!(
                f,
                "{table}: the index read for {year} is below the base-year index"
            ),
            Self::Inexact { table, what } => write!(f, "{table}: {what} is not exact"),
            Self::Money(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for ProjectionError {}

impl From<MoneyError> for ProjectionError {
    fn from(e: MoneyError) -> Self {
        Self::Money(e)
    }
}
