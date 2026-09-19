//! A typed parameter table (`DOMAIN-MODEL.md` §15, seam S1).
//!
//! # The shape this loader reads
//!
//! The documented shape is `{id, unit, period, breakdown, values, projection{rule,
//! index, index_series, base_year, base_values, rounding}, [[source]]}`. The first
//! vintage could not be written in it (`params/vintages/federal-2026/VINTAGE.md`
//! lists the deviations), so this loader reads the documented scalar form **and**
//! each extension the shipped files carry, and rejects a table that states one
//! fact both ways:
//!
//! | Documented | Extension read here | Why |
//! |---|---|---|
//! | `values.<key>.<year> = int` | `edges = [...]` + `values.<key>.<year> = [int; edges]` | a rate schedule has six amounts per key |
//! | `projection.base_values.<key> = int` | `... = [int; edges]` | likewise |
//! | `projection.base_year = year` | `projection.base_year_by_edge.<tax year>.<edge> = year` | one table, two base years, from a given tax year |
//! | `projection.rounding.increment = int` | `projection.rounding.increment_by_key.<key> = int` | the increment varies by filing status |
//! | — | `projection.first_adjusted_year = year` | before it the base amounts apply unadjusted |
//! | — | `projection.lag_years = int` | which calendar year of the series the adjustment for a tax year reads (26 USC 1(f)(3)(A)(i): "the preceding calendar year"); required when `rule = "index"` |
//! | — | `[rates]` (`unit = "ratio"`, `rule = "flat"`, decimal strings) | the never-indexed rate ladder beside its edges |
//! | — | `verification`, `[hand_verification] open = [...]` | pending-verification status |
//! | — | `projection.derived.<key> = {from, multiplier}` | **proposed here, in no shipped file**: a key defined by statute as a multiple of another key's *rounded* result |
//!
//! Whether these extensions are the shape S1 freezes is a human decision; the
//! loader makes them readable so that the decision can be made against running
//! code, and it does not make them normative.

use std::collections::BTreeMap;

use pfp_domain::{FilingStatus, Year};
use pfp_money::{Cents, Ratio, RoundingBasis, RoundingDirection};
use toml::{Table, Value};

use crate::error::ParamError;
use crate::provenance::{
    read_open_items, read_sources, req_date, DateYmd, Source, VerificationStatus,
};
use crate::raw::{document_id, parse_document, At};

const TOP_FIELDS: [&str; 12] = [
    "id",
    "unit",
    "period",
    "breakdown",
    "as_of",
    "verification",
    "edges",
    "values",
    "rates",
    "projection",
    "hand_verification",
    "source",
];
const PROJECTION_FIELDS: [&str; 10] = [
    "rule",
    "index",
    "index_series",
    "lag_years",
    "base_year",
    "base_year_by_edge",
    "first_adjusted_year",
    "base_values",
    "derived",
    "rounding",
];
const ROUNDING_FIELDS: [&str; 4] = ["increment", "increment_by_key", "direction", "basis"];
const UNIT_USD: &str = "USD";
/// No statutory adjustment reads an index further back than this; a larger lag is
/// a transcription error rather than a rule.
const MAX_LAG_YEARS: i32 = 10;
const BREAKDOWN_FILING_STATUS: &str = "filingStatus";

/// How a table's amounts are obtained for a year it does not publish.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ProjectionRule {
    /// Uprated against a price index from the statutory base year.
    Index,
    /// Uprated against a wage index. Part of the frozen shape; not implemented at M0.
    Wage,
    /// Never indexed: the latest published amount carries forward.
    Flat,
    /// Part of the frozen shape; not implemented at M0.
    Zero,
    /// Part of the frozen shape; not implemented at M0.
    Schedule,
}

impl ProjectionRule {
    /// The wire form: `index | wage | flat | zero | schedule`.
    #[must_use]
    pub const fn wire(self) -> &'static str {
        match self {
            Self::Index => "index",
            Self::Wage => "wage",
            Self::Flat => "flat",
            Self::Zero => "zero",
            Self::Schedule => "schedule",
        }
    }

    fn from_wire(s: &str) -> Option<Self> {
        [
            Self::Index,
            Self::Wage,
            Self::Flat,
            Self::Zero,
            Self::Schedule,
        ]
        .into_iter()
        .find(|r| r.wire() == s)
    }
}

/// The base year(s) the index ratio's denominator is read for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BaseYear {
    /// One base year for every component and every tax year (the documented shape).
    Scalar(Year),
    /// Per component, keyed by the first tax year the assignment applies to. A
    /// tax year reads the entry with the greatest key not above it.
    ByComponent(BTreeMap<Year, Vec<Year>>),
}

/// The rounding increment, in cents.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Increment {
    /// One increment for every breakdown key (the documented shape).
    Scalar(Cents),
    /// Per breakdown key.
    ByKey(BTreeMap<String, Cents>),
}

/// `[projection.rounding]`: a `RoundingRule` whose increment may vary by key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoundingSpec {
    /// The multiple rounded to, in cents.
    pub increment: Increment,
    /// Which neighbouring multiple is chosen.
    pub direction: RoundingDirection,
    /// Whether the amount or the increase over the base is rounded.
    pub basis: RoundingBasis,
}

impl RoundingSpec {
    /// The increment that governs `key`, in cents.
    #[must_use]
    pub fn increment_for(&self, key: &str) -> Option<Cents> {
        match &self.increment {
            Increment::Scalar(c) => Some(*c),
            Increment::ByKey(map) => map.get(key).copied(),
        }
    }
}

/// A breakdown key defined as an exact multiple of another key's projected,
/// **already rounded**, amount (26 USC 63(c)(2)(A): "200 percent of the dollar
/// amount in effect under subparagraph (C) for the taxable year").
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Derivation {
    /// The key whose projected amount is multiplied.
    pub from: String,
    /// The exact multiplier.
    pub multiplier: Ratio,
}

/// The `[projection]` block.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Projection {
    /// The projection rule.
    pub rule: ProjectionRule,
    /// The index the statute names, e.g. `cpi.chained`.
    pub index: Option<String>,
    /// The archived series the rule reads, e.g. `cpi.chained.aug12m`. It resolves
    /// against the `index_series` name an archived series table declares.
    pub index_series: Option<String>,
    /// How many calendar years before the tax year the numerator window lies:
    /// the adjustment for tax year `Y` reads the index for calendar year
    /// `Y - lag_years`. Always present on an `index` table.
    pub lag_years: Option<i32>,
    /// The statutory base year(s).
    pub base_year: Option<BaseYear>,
    /// The first tax year the adjustment applies to; earlier years take the base
    /// amounts unadjusted.
    pub first_adjusted_year: Option<Year>,
    /// Statutory base-year amounts per breakdown key, one per component.
    pub base_values: BTreeMap<String, Vec<Cents>>,
    /// Keys projected as a multiple of another key's rounded result.
    pub derived: BTreeMap<String, Derivation>,
    /// The rounding rule.
    pub rounding: Option<RoundingSpec>,
}

/// One parameter table of a vintage.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParamTable {
    id: String,
    period: Option<String>,
    as_of: DateYmd,
    verification: VerificationStatus,
    components: Vec<String>,
    values: BTreeMap<String, BTreeMap<Year, Vec<Cents>>>,
    rates: BTreeMap<Year, Vec<Ratio>>,
    projection: Projection,
    sources: Vec<Source>,
    open_items: Vec<String>,
}

impl ParamTable {
    /// Parses and validates one parameter-table document.
    ///
    /// # Errors
    ///
    /// A [`ParamError`] naming the table and the field. In particular a table
    /// with no projection rule, a table declaring `basis = "IncreaseOverBase"`
    /// without `base_year`, `base_values` and `index_series`, a breakdown key
    /// that is not a `FilingStatus` wire form, a missing status, a TOML float
    /// and a table without `[[source]]` provenance are all rejected.
    pub fn parse(text: &str) -> Result<Self, ParamError> {
        let root = parse_document(text)?;
        let id = document_id(&root)?.to_owned();
        let at = At {
            table: &id,
            path: "",
        };
        at.deny_unknown(&root, &TOP_FIELDS)?;
        let unit = at.req_str(&root, "unit")?;
        if unit != UNIT_USD {
            return Err(at.invalid("unit", "is not \"USD\", the only unit M0 tables carry"));
        }
        let breakdown = at
            .opt_strings(&root, "breakdown")?
            .ok_or_else(|| at.missing("breakdown"))?;
        if breakdown != [BREAKDOWN_FILING_STATUS] {
            return Err(ParamError::UnsupportedBreakdown {
                table: id.clone(),
                breakdown: breakdown.join(","),
            });
        }
        let components = read_components(at, &root)?;
        let width = components.len().max(1);
        let scalar = components.is_empty();

        let values_block = at
            .opt_table(&root, "values")?
            .ok_or_else(|| at.missing("values"))?;
        let mut values = BTreeMap::new();
        for (key, by_year) in keyed(&id, "values", values_block, &[])? {
            let path = format!("values.{key}");
            let inner = At {
                table: &id,
                path: &path,
            };
            let Value::Table(by_year) = by_year else {
                return Err(at.wrong(&path, by_year, "a table of year = amount"));
            };
            let mut years = BTreeMap::new();
            for (year_key, value) in by_year {
                let year = inner.year_key(year_key)?;
                years.insert(year, amounts(inner, year_key, value, scalar, width)?);
            }
            if years.is_empty() {
                return Err(at.invalid(&path, "publishes no year"));
            }
            values.insert(key.to_owned(), years);
        }

        let projection = read_projection(&id, &root, &components)?;
        let rates = read_rates(&id, &root, &components)?;
        Ok(Self {
            period: at.opt_str(&root, "period")?.map(str::to_owned),
            as_of: req_date(at, &root, "as_of")?,
            verification: VerificationStatus::read(at, &root)?,
            components,
            values,
            rates,
            projection,
            sources: read_sources(&id, &root)?,
            open_items: read_open_items(&id, &root)?,
            id,
        })
    }

    /// The table id, e.g. `irs.std_deduction`.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// The period the amounts are stated for, e.g. `year`.
    #[must_use]
    pub fn period(&self) -> Option<&str> {
        self.period.as_deref()
    }

    /// The law date the table states its values as of.
    #[must_use]
    pub const fn as_of(&self) -> DateYmd {
        self.as_of
    }

    /// Whether a human has read the values back against the primary documents.
    #[must_use]
    pub const fn verification(&self) -> VerificationStatus {
        self.verification
    }

    /// The archived primary documents the table cites; never empty.
    #[must_use]
    pub fn sources(&self) -> &[Source] {
        &self.sources
    }

    /// Open hand-verification items recorded beside the table.
    #[must_use]
    pub fn open_items(&self) -> &[String] {
        &self.open_items
    }

    /// The named components of each value (`edges`), low to high; empty for a
    /// table whose value is a single amount.
    #[must_use]
    pub fn components(&self) -> &[String] {
        &self.components
    }

    /// The breakdown keys, in wire-form order.
    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.values.keys().map(String::as_str)
    }

    /// The years `key` publishes, ascending.
    pub fn published_years<'a>(&'a self, key: &str) -> impl Iterator<Item = Year> + 'a {
        self.values
            .get(key)
            .into_iter()
            .flat_map(|years| years.keys().copied())
    }

    /// The published amounts for `(year, key)`, one per component.
    #[must_use]
    pub fn published(&self, year: Year, key: &str) -> Option<&[Cents]> {
        self.values.get(key)?.get(&year).map(Vec::as_slice)
    }

    /// The latest published `(year, amounts)` for `key` at or before `year`.
    #[must_use]
    pub fn latest_published(&self, year: Year, key: &str) -> Option<(Year, &[Cents])> {
        let (y, v) = self.values.get(key)?.range(..=year).next_back()?;
        Some((*y, v.as_slice()))
    }

    /// The rate ladder in force for `year`: the latest `[rates]` row at or before
    /// it (`rule = "flat"`). One more rate than there are components.
    #[must_use]
    pub fn rates(&self, year: Year) -> Option<&[Ratio]> {
        self.rates
            .range(..=year)
            .next_back()
            .map(|(_, r)| r.as_slice())
    }

    /// The `[projection]` block.
    #[must_use]
    pub const fn projection(&self) -> &Projection {
        &self.projection
    }
}

fn read_components(at: At<'_>, root: &Table) -> Result<Vec<String>, ParamError> {
    let Some(edges) = at.opt_strings(root, "edges")? else {
        return Ok(Vec::new());
    };
    if edges.is_empty() {
        return Err(at.invalid("edges", "is empty; omit it for a single-amount table"));
    }
    for (i, edge) in edges.iter().enumerate() {
        if edge.is_empty() || edges[..i].contains(edge) {
            return Err(at.invalid("edges", "has an empty or repeated name"));
        }
    }
    Ok(edges)
}

/// Checks a breakdown-keyed map: every key a `FilingStatus` wire form, and every
/// wire form present unless listed in `exempt`. Yields the entries in wire order.
fn keyed<'m>(
    table: &str,
    field: &str,
    map: &'m Table,
    exempt: &[&str],
) -> Result<Vec<(&'m str, &'m Value)>, ParamError> {
    if let Some(key) = map.keys().find(|k| FilingStatus::from_wire(k).is_none()) {
        return Err(ParamError::UnknownBreakdownKey {
            table: table.to_owned(),
            field: field.to_owned(),
            key: key.clone(),
        });
    }
    if let Some(status) = FilingStatus::ALL
        .iter()
        .find(|s| !map.contains_key(s.wire()) && !exempt.contains(&s.wire()))
    {
        return Err(ParamError::MissingBreakdownKey {
            table: table.to_owned(),
            field: field.to_owned(),
            key: status.wire().to_owned(),
        });
    }
    Ok(map.iter().map(|(k, v)| (k.as_str(), v)).collect())
}

/// Integer dollars as published, to cents; negative amounts are rejected.
fn dollars(at: At<'_>, key: &str, value: &Value) -> Result<Cents, ParamError> {
    let Value::Integer(d) = value else {
        return Err(at.wrong(key, value, "integer dollars"));
    };
    if *d < 0 {
        return Err(at.invalid(key, "is negative"));
    }
    Cents::from_dollars(*d).map_err(|_| at.invalid(key, "does not fit the cent range"))
}

fn amounts(
    at: At<'_>,
    key: &str,
    value: &Value,
    scalar: bool,
    width: usize,
) -> Result<Vec<Cents>, ParamError> {
    if scalar {
        return Ok(vec![dollars(at, key, value)?]);
    }
    let Value::Array(items) = value else {
        return Err(at.wrong(key, value, "an array with one amount per edge"));
    };
    if items.len() != width {
        return Err(ParamError::WidthMismatch {
            table: at.table.to_owned(),
            field: at.field(key),
            expected: width,
            found: items.len(),
        });
    }
    let out: Vec<Cents> = items
        .iter()
        .map(|item| dollars(at, key, item))
        .collect::<Result<_, _>>()?;
    if out.windows(2).any(|w| w[0] > w[1]) {
        return Err(at.invalid(key, "is not ordered low to high"));
    }
    Ok(out)
}

fn read_projection(
    id: &str,
    root: &Table,
    components: &[String],
) -> Result<Projection, ParamError> {
    let top = At {
        table: id,
        path: "",
    };
    let at = At {
        table: id,
        path: "projection",
    };
    let no_rule = || ParamError::MissingProjectionRule {
        table: id.to_owned(),
    };
    let block = top.opt_table(root, "projection")?.ok_or_else(no_rule)?;
    at.deny_unknown(block, &PROJECTION_FIELDS)?;
    let rule = at.opt_str(block, "rule")?.ok_or_else(no_rule)?;
    let rule = ProjectionRule::from_wire(rule)
        .ok_or_else(|| at.invalid("rule", "is not index, wage, flat, zero or schedule"))?;

    let scalar = components.is_empty();
    let width = components.len().max(1);
    let derived = read_derived(id, block)?;
    let exempt: Vec<&str> = derived.keys().map(String::as_str).collect();
    let mut base_values = BTreeMap::new();
    if let Some(map) = at.opt_table(block, "base_values")? {
        let inner = At {
            table: id,
            path: "projection.base_values",
        };
        for (key, value) in keyed(id, "projection.base_values", map, &exempt)? {
            let cents = amounts(inner, key, value, scalar, width)?;
            if cents.contains(&Cents::ZERO) {
                // DOMAIN-MODEL §15: zero means "not yet transcribed".
                return Err(inner.invalid(key, "is zero, which means not yet transcribed"));
            }
            base_values.insert(key.to_owned(), cents);
        }
    }
    for (key, d) in &derived {
        let inner = At {
            table: id,
            path: "projection.derived",
        };
        if derived.contains_key(&d.from) || FilingStatus::from_wire(&d.from).is_none() {
            return Err(inner.invalid(key, "derives from a key that is unknown or itself derived"));
        }
    }

    let base_year = read_base_year(id, block, components)?;
    let rounding = read_rounding(id, block)?;
    let index_series = at.opt_str(block, "index_series")?.map(str::to_owned);
    let lag_years = read_lag_years(at, block)?;

    if rounding
        .as_ref()
        .is_some_and(|r| r.basis == RoundingBasis::IncreaseOverBase)
    {
        let mut missing = Vec::new();
        if base_year.is_none() {
            missing.push("base_year");
        }
        if base_values.is_empty() {
            missing.push("base_values");
        }
        if index_series.is_none() {
            missing.push("index_series");
        }
        if !missing.is_empty() {
            return Err(ParamError::IncreaseOverBaseIncomplete {
                table: id.to_owned(),
                missing,
            });
        }
    }
    if rule == ProjectionRule::Index {
        if rounding.is_none() {
            return Err(ParamError::MissingRounding {
                table: id.to_owned(),
            });
        }
        for (key, present) in [
            ("index", block.contains_key("index")),
            ("index_series", index_series.is_some()),
            ("lag_years", lag_years.is_some()),
            ("base_year", base_year.is_some()),
            ("base_values", !base_values.is_empty()),
        ] {
            if !present {
                return Err(at.missing(key));
            }
        }
    }
    Ok(Projection {
        rule,
        index: at.opt_str(block, "index")?.map(str::to_owned),
        index_series,
        lag_years,
        base_year,
        first_adjusted_year: at.opt_year(block, "first_adjusted_year")?,
        base_values,
        derived,
        rounding,
    })
}

/// `projection.lag_years`: a whole number of years in `0..=MAX_LAG_YEARS`.
fn read_lag_years(at: At<'_>, block: &Table) -> Result<Option<i32>, ParamError> {
    let Some(raw) = at.opt_int(block, "lag_years")? else {
        return Ok(None);
    };
    i32::try_from(raw)
        .ok()
        .filter(|lag| (0..=MAX_LAG_YEARS).contains(lag))
        .map(Some)
        .ok_or_else(|| at.invalid("lag_years", "is not a whole number of years in 0..=10"))
}

fn read_derived(id: &str, block: &Table) -> Result<BTreeMap<String, Derivation>, ParamError> {
    let at = At {
        table: id,
        path: "projection",
    };
    let inner = At {
        table: id,
        path: "projection.derived",
    };
    let mut out = BTreeMap::new();
    let Some(map) = at.opt_table(block, "derived")? else {
        return Ok(out);
    };
    for (key, value) in map {
        if FilingStatus::from_wire(key).is_none() {
            return Err(ParamError::UnknownBreakdownKey {
                table: id.to_owned(),
                field: "projection.derived".to_owned(),
                key: key.clone(),
            });
        }
        let Value::Table(entry) = value else {
            return Err(inner.wrong(key, value, "a table {from, multiplier}"));
        };
        let path = inner.field(key);
        let entry_at = At {
            table: id,
            path: &path,
        };
        entry_at.deny_unknown(entry, &["from", "multiplier"])?;
        let multiplier = entry_at
            .req_str(entry, "multiplier")?
            .parse::<Ratio>()
            .ok()
            .filter(|r| r.num > 0)
            .ok_or_else(|| {
                entry_at.invalid("multiplier", "is not a positive decimal or fraction string")
            })?;
        out.insert(
            key.clone(),
            Derivation {
                from: entry_at.req_str(entry, "from")?.to_owned(),
                multiplier,
            },
        );
    }
    Ok(out)
}

fn read_base_year(
    id: &str,
    block: &Table,
    components: &[String],
) -> Result<Option<BaseYear>, ParamError> {
    let at = At {
        table: id,
        path: "projection",
    };
    let scalar = at.opt_year(block, "base_year")?;
    let by_edge = at.opt_table(block, "base_year_by_edge")?;
    match (scalar, by_edge) {
        (Some(_), Some(_)) => Err(ParamError::Conflict {
            table: id.to_owned(),
            first: "projection.base_year",
            second: "projection.base_year_by_edge",
        }),
        (Some(year), None) => Ok(Some(BaseYear::Scalar(year))),
        (None, None) => Ok(None),
        (None, Some(map)) => {
            let outer = At {
                table: id,
                path: "projection.base_year_by_edge",
            };
            if components.is_empty() {
                return Err(at.invalid("base_year_by_edge", "needs a table that declares `edges`"));
            }
            let mut out = BTreeMap::new();
            for (year_key, value) in map {
                let tax_year = outer.year_key(year_key)?;
                let Value::Table(edges) = value else {
                    return Err(outer.wrong(year_key, value, "a table of edge = base year"));
                };
                let path = outer.field(year_key);
                let inner = At {
                    table: id,
                    path: &path,
                };
                inner.deny_unknown(
                    edges,
                    &components.iter().map(String::as_str).collect::<Vec<_>>(),
                )?;
                let years: Vec<Year> = components
                    .iter()
                    .map(|edge| {
                        inner
                            .opt_year(edges, edge)?
                            .ok_or_else(|| inner.missing(edge))
                    })
                    .collect::<Result<_, _>>()?;
                out.insert(tax_year, years);
            }
            if out.is_empty() {
                return Err(at.invalid("base_year_by_edge", "is empty"));
            }
            Ok(Some(BaseYear::ByComponent(out)))
        }
    }
}

fn read_rounding(id: &str, block: &Table) -> Result<Option<RoundingSpec>, ParamError> {
    let outer = At {
        table: id,
        path: "projection",
    };
    let at = At {
        table: id,
        path: "projection.rounding",
    };
    let Some(map) = outer.opt_table(block, "rounding")? else {
        return Ok(None);
    };
    at.deny_unknown(map, &ROUNDING_FIELDS)?;
    let positive = |inner: At<'_>, key: &str, value: &Value| -> Result<Cents, ParamError> {
        let cents = dollars(inner, key, value)?;
        if cents == Cents::ZERO {
            return Err(inner.invalid(key, "is not a positive increment"));
        }
        Ok(cents)
    };
    let increment = match (map.get("increment"), at.opt_table(map, "increment_by_key")?) {
        (Some(_), Some(_)) => {
            return Err(ParamError::Conflict {
                table: id.to_owned(),
                first: "projection.rounding.increment",
                second: "projection.rounding.increment_by_key",
            })
        }
        (None, None) => return Err(at.missing("increment")),
        (Some(value), None) => Increment::Scalar(positive(at, "increment", value)?),
        (None, Some(by_key)) => {
            let inner = At {
                table: id,
                path: "projection.rounding.increment_by_key",
            };
            let mut out = BTreeMap::new();
            for (key, value) in keyed(id, inner.path, by_key, &[])? {
                out.insert(key.to_owned(), positive(inner, key, value)?);
            }
            Increment::ByKey(out)
        }
    };
    let direction = match at.req_str(map, "direction")? {
        "down" => RoundingDirection::Down,
        "up" => RoundingDirection::Up,
        "halfUp" => RoundingDirection::HalfUp,
        "halfEven" => RoundingDirection::HalfEven,
        "nearest" => RoundingDirection::Nearest,
        _ => return Err(at.invalid("direction", "is not down, up, halfUp, halfEven or nearest")),
    };
    let basis = match at.req_str(map, "basis")? {
        "Amount" => RoundingBasis::Amount,
        "IncreaseOverBase" => RoundingBasis::IncreaseOverBase,
        _ => return Err(at.invalid("basis", "is not Amount or IncreaseOverBase")),
    };
    Ok(Some(RoundingSpec {
        increment,
        direction,
        basis,
    }))
}

fn read_rates(
    id: &str,
    root: &Table,
    components: &[String],
) -> Result<BTreeMap<Year, Vec<Ratio>>, ParamError> {
    let top = At {
        table: id,
        path: "",
    };
    let at = At {
        table: id,
        path: "rates",
    };
    let mut out = BTreeMap::new();
    let Some(block) = top.opt_table(root, "rates")? else {
        return Ok(out);
    };
    if components.is_empty() {
        return Err(top.invalid("rates", "needs a table that declares `edges`"));
    }
    if at.req_str(block, "unit")? != "ratio" {
        return Err(at.invalid("unit", "is not \"ratio\""));
    }
    if at.req_str(block, "rule")? != ProjectionRule::Flat.wire() {
        return Err(at.invalid("rule", "is not \"flat\": no statutory rule indexes a rate"));
    }
    for (key, value) in block {
        if key == "unit" || key == "rule" {
            continue;
        }
        let year = at.year_key(key)?;
        let Value::Array(items) = value else {
            return Err(at.wrong(key, value, "an array of decimal strings"));
        };
        if items.len() != components.len() + 1 {
            return Err(ParamError::WidthMismatch {
                table: id.to_owned(),
                field: at.field(key),
                expected: components.len() + 1,
                found: items.len(),
            });
        }
        let ladder: Vec<Ratio> = items
            .iter()
            .map(|item| match item {
                Value::String(s) => s
                    .parse::<Ratio>()
                    .ok()
                    .filter(|r| r.num >= 0 && r.num <= r.den)
                    .ok_or_else(|| at.invalid(key, "holds a string that is not a rate in 0..=1")),
                other => Err(at.wrong(key, other, "an array of decimal strings")),
            })
            .collect::<Result<_, _>>()?;
        out.insert(year, ladder);
    }
    if out.is_empty() {
        return Err(top.invalid("rates", "publishes no year"));
    }
    Ok(out)
}
