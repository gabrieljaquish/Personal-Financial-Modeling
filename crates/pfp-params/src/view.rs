//! A vintage and the view the engine reads it through.
//!
//! # Uprating
//!
//! A projected amount is always computed **from the statutory base year in one
//! step**: `base_value x (index[year - lag] / index[base_year])`, with the table's
//! `RoundingRule` applied exactly once — for `basis = IncreaseOverBase`, to the
//! increase over the base amount (`DOMAIN-MODEL.md` §15, `ENGINE-SPEC.md` §1.3).
//! Nothing here reads a published amount while projecting, so there is no chained
//! path to take: uprating is path-independent by construction, and the
//! year-over-year chain exists only in the negative-control test.
//!
//! The lag and the series are both **data**: `projection.lag_years` on the table,
//! and the `index_series` name the archived series document declares. Nothing in
//! this crate says which series a table reads or which year of it.
//!
//! # The override layer (seam documented, not built at M0)
//!
//! `ParamOverride` is `pfp-model`'s type and lives in the plan
//! (`DOMAIN-MODEL.md` §15); no M0 caller has a plan. [`ParamView`] is the place
//! the layer attaches: a later milestone adds a constructor taking the plan's
//! overrides and an [`Origin`] variant reporting the override **beside** the
//! sourced value, never merged into it. Every accessor already returns the origin
//! of what it hands out, so adding that variant changes no signature.
//!
//! Likewise `ENGINE-SPEC.md` §1.3's `ParamView::for_year(t, &inflation_index)`:
//! at M0 the only index is the archived series, and a year past the archive is
//! [`ProjectionError::IndexUnavailable`] rather than a guess.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use pfp_domain::Year;
use pfp_money::{Cents, Ratio, RoundingRule};
use sha2::{Digest, Sha256};

use crate::error::{ParamError, ProjectionError};
use crate::series::IndexSeries;
use crate::table::{BaseYear, Derivation, ParamTable, ProjectionRule};

/// `<name>@<sha256-of-contents>` (ADR-010, seam S1).
///
/// # What "contents" means (PROPOSED: no design document defines it)
///
/// A vintage is several documents, so the hash needs a definition. The one
/// implemented here, pending the human decision recorded as B9 in
/// `docs/verification/m0-hand-verification.md`:
///
/// ```text
/// sha256( "pfp-vintage-id/v1\n" <vintage name> "\n"
///         then, for each document in ascending byte order of its `id`:
///             <kind> " " <id> " " <byte length, decimal> "\n" <the document's bytes> )
/// ```
///
/// The documents are every parameter table of the vintage (`kind` = `table`) and
/// every archived index series at least one of those tables reads (`kind` =
/// `index-series`), wherever it is stored. Bytes are hashed exactly as stored:
/// there is no canonicalisation, so a comment edit is a new vintage, as it is for
/// the per-file digests of `params/VINTAGES.lock`. Prose manifests (`VINTAGE.md`)
/// are not loaded by the engine and are not part of the id; the lock file pins them.
///
/// Computing the id asserts nothing about verification or immutability: see
/// [`Vintage::content_id`] and [`Vintage::id`].
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VintageId {
    name: String,
    sha256: String,
}

impl VintageId {
    /// Parses `<name>@<64 lowercase hex digits>`.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        let (name, sha256) = s.split_once('@')?;
        let hex = sha256.len() == 64
            && sha256
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
        (!name.is_empty() && hex).then(|| Self {
            name: name.to_owned(),
            sha256: sha256.to_owned(),
        })
    }

    /// The vintage name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The content hash.
    #[must_use]
    pub fn sha256(&self) -> &str {
        &self.sha256
    }
}

impl core::fmt::Display for VintageId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}@{}", self.name, self.sha256)
    }
}

/// An immutable set of parameter tables and the archived series they read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Vintage {
    name: String,
    content_id: VintageId,
    locked: bool,
    tables: BTreeMap<String, ParamTable>,
    /// Keyed by the `index_series` name each series declares; only series a
    /// table of this vintage reads.
    series: BTreeMap<String, IndexSeries>,
}

impl Vintage {
    /// Parses a vintage from text. Performs no I/O: the caller (the embedding in
    /// [`crate::shipped`], the application's asset bundle, the `data-hygiene`
    /// gate or a test) supplies the documents.
    ///
    /// `series` are the archived index-series documents available to the vintage.
    /// Each declares the `index_series` name it answers to, and a table's
    /// `projection.index_series` resolves against those names; a series no table
    /// reads is validated and then left out of the vintage and of its id.
    ///
    /// # Errors
    ///
    /// Any table's or series' [`ParamError`]; [`ParamError::DuplicateTable`];
    /// [`ParamError::UnresolvedIndexSeries`] when an indexed table names a series
    /// no supplied document declares.
    pub fn parse(name: &str, tables: &[&str], series: &[&str]) -> Result<Self, ParamError> {
        let mut available = BTreeMap::new();
        for text in series {
            let parsed = IndexSeries::parse(text)?;
            let declared = parsed.index_series().to_owned();
            if available
                .insert(declared.clone(), (parsed, *text))
                .is_some()
            {
                return Err(ParamError::DuplicateTable { table: declared });
            }
        }
        let mut out = BTreeMap::new();
        let mut read = BTreeMap::new();
        // (kind, id) -> bytes, for the content id.
        let mut documents: BTreeMap<String, (&str, &str)> = BTreeMap::new();
        for text in tables {
            let table = ParamTable::parse(text)?;
            if let Some(series_name) = &table.projection().index_series {
                if table.projection().rule == ProjectionRule::Index {
                    let (parsed, series_text) = available.get(series_name).ok_or_else(|| {
                        ParamError::UnresolvedIndexSeries {
                            table: table.id().to_owned(),
                            index_series: series_name.clone(),
                        }
                    })?;
                    let shadowed = documents
                        .insert(parsed.id().to_owned(), (KIND_SERIES, series_text))
                        .is_some_and(|(kind, _)| kind != KIND_SERIES);
                    if shadowed {
                        return Err(ParamError::DuplicateTable {
                            table: parsed.id().to_owned(),
                        });
                    }
                    read.insert(series_name.clone(), parsed.clone());
                }
            }
            let id = table.id().to_owned();
            if documents.insert(id.clone(), (KIND_TABLE, text)).is_some()
                || out.insert(id.clone(), table).is_some()
            {
                return Err(ParamError::DuplicateTable { table: id });
            }
        }
        Ok(Self {
            name: name.to_owned(),
            content_id: content_id(name, &documents),
            locked: false,
            tables: out,
            series: read,
        })
    }

    /// Marks the vintage as the locked one `recorded` names.
    ///
    /// Only a human who has read the primary documents records a vintage id
    /// (`docs/contributing.md` §3.2); this checks that the documents parsed are
    /// byte for byte the ones that id was taken over.
    ///
    /// # Errors
    ///
    /// [`ParamError::VintageIdMismatch`] when `recorded` is not this vintage's
    /// [`Vintage::content_id`].
    pub fn locked_as(mut self, recorded: &VintageId) -> Result<Self, ParamError> {
        if *recorded != self.content_id {
            return Err(ParamError::VintageIdMismatch {
                recorded: recorded.to_string(),
                computed: self.content_id.to_string(),
            });
        }
        self.locked = true;
        Ok(self)
    }

    /// The vintage name, e.g. `federal-2026`.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// `<name>@<sha256-of-contents>` over the documents this vintage was parsed
    /// from, as defined on [`VintageId`]. Always available, and **not** a claim
    /// that the vintage is verified or immutable: it is what a lock would record.
    #[must_use]
    pub const fn content_id(&self) -> &VintageId {
        &self.content_id
    }

    /// The vintage id a result may be stamped with (`paramVintage`): `Some` only
    /// once [`Vintage::locked_as`] has matched the recorded id. `None` for an
    /// unlocked vintage, which carries no immutability claim (`VINTAGE.md`).
    #[must_use]
    pub fn id(&self) -> Option<&VintageId> {
        self.locked.then_some(&self.content_id)
    }

    /// Whether **every** table and series has a human sign-off. `false` for the
    /// first vintage, all of which is pending hand verification.
    #[must_use]
    pub fn is_verified(&self) -> bool {
        self.tables.values().all(|t| t.verification().is_verified())
            && self.series.values().all(|s| s.verification().is_verified())
    }

    /// The tables, ordered by id.
    pub fn tables(&self) -> impl Iterator<Item = &ParamTable> {
        self.tables.values()
    }

    /// One table by id.
    #[must_use]
    pub fn table(&self, id: &str) -> Option<&ParamTable> {
        self.tables.get(id)
    }

    /// The archived series bound to an `index_series` name.
    #[must_use]
    pub fn index_series(&self, name: &str) -> Option<&IndexSeries> {
        self.series.get(name)
    }
}

/// Where an amount came from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Origin {
    /// Transcribed from the publication for that year.
    Published,
    /// The statutory base amount, for a year before the table's first adjusted year.
    UnadjustedBase,
    /// A never-indexed (`flat`) table's latest published amount, carried forward.
    CarriedForward {
        /// The published year the amount was carried from.
        from_year: Year,
    },
    /// Uprated from the statutory base year, rounded once.
    Projected {
        /// The `index_series` name read.
        index_series: String,
        /// The calendar year of the numerator window.
        numerator_year: Year,
        /// One step per component, in component order. Empty for a derived key,
        /// whose steps are those of [`Origin::Projected::derived_from`].
        steps: Vec<ProjectionStep>,
        /// For a derived key: the key it is a multiple of, and the multiplier.
        derived_from: Option<(String, Ratio)>,
    },
}

/// The arithmetic behind one projected component, for the explain trace.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProjectionStep {
    /// The statutory base-year amount.
    pub base: Cents,
    /// The base year whose index is the denominator.
    pub base_year: Year,
    /// `index[numerator year] / index[base year]`, exact, in lowest terms.
    pub factor: Ratio,
    /// The rounding rule applied, once.
    pub rule: RoundingRule,
    /// The result.
    pub amount: Cents,
}

/// Amounts for one `(table, year, key)`, one per component, with their origin.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParamValue {
    /// One amount per component; exactly one for a single-amount table.
    pub amounts: Vec<Cents>,
    /// Where they came from.
    pub origin: Origin,
}

impl ParamValue {
    /// The amount of a single-amount table; `None` when there are several.
    #[must_use]
    pub fn scalar(&self) -> Option<Cents> {
        match self.amounts.as_slice() {
            [one] => Some(*one),
            _ => None,
        }
    }
}

/// The engine's read-only view of a vintage (plus, later, the plan's overrides).
#[derive(Clone, Copy, Debug)]
pub struct ParamView<'a> {
    vintage: &'a Vintage,
}

// `&self` although the view is one reference today: the design's signatures take
// `&ParamView`, and the override layer will make it larger than a pointer.
#[allow(clippy::trivially_copy_pass_by_ref)]
impl<'a> ParamView<'a> {
    /// A view of `vintage` with no override layer.
    #[must_use]
    pub const fn new(vintage: &'a Vintage) -> Self {
        Self { vintage }
    }

    /// The underlying vintage.
    #[must_use]
    pub const fn vintage(&self) -> &'a Vintage {
        self.vintage
    }

    fn table(&self, id: &str) -> Result<&'a ParamTable, ProjectionError> {
        self.vintage
            .table(id)
            .ok_or_else(|| ProjectionError::UnknownTable {
                table: id.to_owned(),
            })
    }

    /// The published amounts for `(year, key)`, or `None` when that year is not
    /// published.
    ///
    /// # Errors
    ///
    /// [`ProjectionError::UnknownTable`], [`ProjectionError::UnknownKey`].
    pub fn published(
        &self,
        table: &str,
        year: Year,
        key: &str,
    ) -> Result<Option<&'a [Cents]>, ProjectionError> {
        let t = self.table(table)?;
        if !t.keys().any(|k| k == key) {
            return Err(ProjectionError::UnknownKey {
                table: table.to_owned(),
                key: key.to_owned(),
            });
        }
        Ok(t.published(year, key))
    }

    /// The amounts in force for `(year, key)`: the published ones where the year
    /// is published, otherwise [`ParamView::project`].
    ///
    /// # Errors
    ///
    /// As [`ParamView::published`] and [`ParamView::project`].
    pub fn value(&self, table: &str, year: Year, key: &str) -> Result<ParamValue, ProjectionError> {
        match self.published(table, year, key)? {
            Some(amounts) => Ok(ParamValue {
                amounts: amounts.to_vec(),
                origin: Origin::Published,
            }),
            None => self.project(table, year, key),
        }
    }

    /// The table's projection rule applied for `(year, key)`, **whether or not
    /// the year is published** — which is what lets a test compare the statutory
    /// pipeline with the publication. Published amounts are never an input.
    ///
    /// # Errors
    ///
    /// [`ProjectionError`]: an unknown table or key, a rule not implemented at
    /// M0, no base year for the tax year, a calendar year outside the archived
    /// series, an index below its base, or failed money arithmetic.
    pub fn project(
        &self,
        table: &str,
        year: Year,
        key: &str,
    ) -> Result<ParamValue, ProjectionError> {
        let t = self.table(table)?;
        if !t.keys().any(|k| k == key) {
            return Err(ProjectionError::UnknownKey {
                table: table.to_owned(),
                key: key.to_owned(),
            });
        }
        match t.projection().rule {
            ProjectionRule::Index => self.project_index(t, year, key),
            ProjectionRule::Flat => {
                let (from_year, amounts) = t.latest_published(year, key).ok_or_else(|| {
                    ProjectionError::NothingToCarry {
                        table: table.to_owned(),
                        year,
                    }
                })?;
                Ok(ParamValue {
                    amounts: amounts.to_vec(),
                    origin: Origin::CarriedForward { from_year },
                })
            }
            rule @ (ProjectionRule::Wage | ProjectionRule::Zero | ProjectionRule::Schedule) => {
                Err(ProjectionError::UnsupportedRule {
                    table: table.to_owned(),
                    rule: rule.wire(),
                })
            }
        }
    }

    /// The rate ladder in force for `year`.
    ///
    /// # Errors
    ///
    /// [`ProjectionError::UnknownTable`], [`ProjectionError::NoRates`].
    pub fn rates(&self, table: &str, year: Year) -> Result<&'a [Ratio], ProjectionError> {
        self.table(table)?
            .rates(year)
            .ok_or_else(|| ProjectionError::NoRates {
                table: table.to_owned(),
                year,
            })
    }

    /// A derived key: an exact multiple of the source key's ROUNDED result.
    fn project_derived(
        &self,
        t: &ParamTable,
        year: Year,
        d: &Derivation,
    ) -> Result<ParamValue, ProjectionError> {
        let source = self.project_index(t, year, &d.from)?;
        let amounts = source
            .amounts
            .iter()
            .map(|c| multiply_exact(t.id(), *c, d.multiplier))
            .collect::<Result<_, _>>()?;
        let origin = match source.origin {
            Origin::Projected {
                index_series,
                numerator_year,
                ..
            } => Origin::Projected {
                index_series,
                numerator_year,
                steps: Vec::new(),
                derived_from: Some((d.from.clone(), d.multiplier)),
            },
            other => other,
        };
        Ok(ParamValue { amounts, origin })
    }

    fn project_index(
        &self,
        t: &ParamTable,
        year: Year,
        key: &str,
    ) -> Result<ParamValue, ProjectionError> {
        let p = t.projection();
        if let Some(d) = p.derived.get(key) {
            return self.project_derived(t, year, d);
        }
        let unknown_key = || ProjectionError::UnknownKey {
            table: t.id().to_owned(),
            key: key.to_owned(),
        };
        let bases = p.base_values.get(key).ok_or_else(unknown_key)?;
        if p.first_adjusted_year.is_some_and(|first| year < first) {
            return Ok(ParamValue {
                amounts: bases.clone(),
                origin: Origin::UnadjustedBase,
            });
        }
        let no_base_year = || ProjectionError::NoBaseYear {
            table: t.id().to_owned(),
            year,
        };
        let base_years: Vec<Year> = match p.base_year.as_ref().ok_or_else(no_base_year)? {
            BaseYear::Scalar(y) => vec![*y; bases.len()],
            BaseYear::ByComponent(map) => map
                .range(..=year)
                .next_back()
                .map(|(_, years)| years.clone())
                .ok_or_else(no_base_year)?,
        };
        // Validation guarantees an indexed table has all of these.
        let spec = p.rounding.as_ref().ok_or_else(unknown_key)?;
        let increment = spec.increment_for(key).ok_or_else(unknown_key)?;
        let rule = RoundingRule::new(increment, spec.direction, spec.basis)?;
        let series_name = p.index_series.as_deref().ok_or_else(no_base_year)?;
        let series = self.vintage.series.get(series_name).ok_or_else(|| {
            ProjectionError::IndexUnavailable {
                index_series: series_name.to_owned(),
                calendar_year: year,
            }
        })?;
        let numerator_year = p
            .lag_years
            .and_then(|lag| year.checked_sub(lag))
            .ok_or_else(|| ProjectionError::IndexUnavailable {
                index_series: series_name.to_owned(),
                calendar_year: year,
            })?;

        let mut steps = Vec::with_capacity(bases.len());
        for (base, base_year) in bases.iter().zip(base_years) {
            for calendar_year in [numerator_year, base_year] {
                if !series.has_year(calendar_year) {
                    return Err(ProjectionError::IndexUnavailable {
                        index_series: series_name.to_owned(),
                        calendar_year,
                    });
                }
            }
            let factor = series.factor(numerator_year, base_year).ok_or_else(|| {
                ProjectionError::Inexact {
                    table: t.id().to_owned(),
                    what: "the index factor as an i64 ratio",
                }
            })?;
            if factor.num < factor.den {
                return Err(ProjectionError::IndexBelowBase {
                    table: t.id().to_owned(),
                    year,
                });
            }
            steps.push(ProjectionStep {
                base: *base,
                base_year,
                factor,
                rule,
                amount: base.checked_mul_ratio(factor, &rule)?,
            });
        }
        Ok(ParamValue {
            amounts: steps.iter().map(|s| s.amount).collect(),
            origin: Origin::Projected {
                index_series: series_name.to_owned(),
                numerator_year,
                steps,
                derived_from: None,
            },
        })
    }
}

const KIND_TABLE: &str = "table";
const KIND_SERIES: &str = "index-series";
const ID_PREAMBLE: &str = "pfp-vintage-id/v1";

/// The content id defined on [`VintageId`]. `documents` maps a document id to its
/// kind and bytes; a `BTreeMap` over `String` iterates in ascending byte order.
fn content_id(name: &str, documents: &BTreeMap<String, (&str, &str)>) -> VintageId {
    let mut hasher = Sha256::new();
    hasher.update(format!("{ID_PREAMBLE}\n{name}\n"));
    for (id, (kind, text)) in documents {
        hasher.update(format!("{kind} {id} {}\n", text.len()));
        hasher.update(text.as_bytes());
    }
    let sha256 = hasher
        .finalize()
        .iter()
        .fold(String::with_capacity(64), |mut hex, byte| {
            // Writing to a `String` cannot fail.
            let _ = write!(hex, "{byte:02x}");
            hex
        });
    VintageId {
        name: name.to_owned(),
        sha256,
    }
}

/// `amount x multiplier`, which must be a whole number of cents.
fn multiply_exact(table: &str, amount: Cents, multiplier: Ratio) -> Result<Cents, ProjectionError> {
    let inexact = || ProjectionError::Inexact {
        table: table.to_owned(),
        what: "a derived amount in whole cents",
    };
    let product = i128::from(amount.0) * i128::from(multiplier.num);
    let den = i128::from(multiplier.den);
    if den <= 0 || product % den != 0 {
        return Err(inexact());
    }
    i64::try_from(product / den)
        .map(Cents)
        .map_err(|_| ProjectionError::Money(pfp_money::MoneyError::Overflow))
}
