//! Every type that crosses the API, and nothing else (`ARCHITECTURE.md` §5,
//! ADR-017). The `OpenAPI` document is generated from these.
//!
//! The engine crates carry no `OpenAPI` dependency: these are server-side mirrors
//! with `From` conversions, and a test asserts that a [`LineDto`] serializes to
//! exactly what the engine's own `Line` serializes to, so the mirror cannot drift.
//! Nothing is recomputed or re-labelled here.
//!
//! Wire conventions: camelCase; money is an integer number of **cents**
//! ([`CentsDto`], annotated `x-money` in the schema); requests refuse unknown
//! fields; absent optional values are omitted, never `null`.

use std::borrow::Cow;
use std::collections::BTreeMap;

use pfp_explain::{Line, ParamRef};
use pfp_money::{Cents, Ratio, RoundingBasis, RoundingDirection};
use pfp_params::{
    BaseYear, DateYmd, Increment, IndexSeries, ParamTable, Projection, RoundingSpec, Source,
    VerificationStatus, Vintage,
};
use serde::{Deserialize, Serialize};
use utoipa::openapi::extensions::ExtensionsBuilder;
use utoipa::openapi::schema::{KnownFormat, ObjectBuilder, SchemaFormat, SchemaType, Type};
use utoipa::openapi::{RefOr, Schema};
use utoipa::{PartialSchema, ToSchema};

/// An amount of money in integer cents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CentsDto(pub i64);

impl From<Cents> for CentsDto {
    fn from(c: Cents) -> Self {
        Self(c.0)
    }
}

impl PartialSchema for CentsDto {
    fn schema() -> RefOr<Schema> {
        ObjectBuilder::new()
            .schema_type(SchemaType::Type(Type::Integer))
            .format(Some(SchemaFormat::KnownFormat(KnownFormat::Int64)))
            .description(Some("An amount of money in integer cents."))
            .extensions(Some(ExtensionsBuilder::new().add("x-money", true).build()))
            .into()
    }
}

impl ToSchema for CentsDto {
    fn name() -> Cow<'static, str> {
        Cow::Borrowed("Cents")
    }
}

/// `{"code", "message"}`: the body of every refusal and failure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ErrorBody {
    /// Stable, machine-readable code.
    pub code: String,
    /// Fixed human-readable text. Never echoes a request value.
    pub message: String,
}

/// The launch token from the URL fragment.
#[derive(Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BootstrapRequest {
    /// 64 lower-case hex characters. Single use, 60 s.
    #[schema(min_length = 64, max_length = 64)]
    pub token: String,
}

// A request body that carries a secret has no derived `Debug`.
impl std::fmt::Debug for BootstrapRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BootstrapRequest").finish_non_exhaustive()
    }
}

/// The port-scoped half of the session; the other half is the `__Host-` cookie.
#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BootstrapResponse {
    /// Goes into `sessionStorage["pfp.proof"]` and then into `X-PFP-Proof`.
    pub proof: String,
}

impl std::fmt::Debug for BootstrapResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BootstrapResponse").finish_non_exhaustive()
    }
}

/// An accepted request with nothing to report.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, ToSchema)]
pub struct Accepted {}

/// Whether the local certificate authority is trusted by this user's browsers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub enum TrustModeDto {
    /// The local CA was installed into the user's trust settings.
    Installed,
    /// Trust was declined or is unavailable; the browser shows a warning and the
    /// user compares the certificate fingerprint.
    Declined,
}

/// A parameter vintage's identity, without its tables.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct VintageSummaryDto {
    /// The vintage's name.
    pub name: String,
    /// `name@sha256:…` computed from the content; claims nothing by itself.
    pub content_id: String,
    /// The locked id, present only when `params/VINTAGES.lock` records this content.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub locked_id: Option<String>,
    /// Whether every table and series has been verified by a human.
    pub verified: bool,
}

impl From<&Vintage> for VintageSummaryDto {
    fn from(v: &Vintage) -> Self {
        Self {
            name: v.name().to_owned(),
            content_id: v.content_id().to_string(),
            locked_id: v.id().map(ToString::to_string),
            verified: v.is_verified(),
        }
    }
}

/// Health and session status.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SessionStatus {
    /// The application version.
    pub app_version: String,
    /// The API version: the `v1` of `/api/v1`.
    pub api_version: String,
    /// See [`TrustModeDto`].
    pub trust_mode: TrustModeDto,
    /// The parameter vintages compiled into this build.
    pub vintages: Vec<VintageSummaryDto>,
}

/// Inputs of the rate schedule.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RateScheduleRequest {
    /// Tax year.
    pub year: i32,
    /// `single`, `mfj`, `mfs`, `hoh` or `qss`.
    pub filing_status: FilingStatusDto,
    /// Taxable income in cents; not negative, at most 2^53 − 1.
    pub taxable_income: CentsDto,
}

/// The file format of an export.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub enum ExportFormatDto {
    /// Comma-separated values with type-aware formula-injection escaping.
    Csv,
    /// The worksheet's JSON body, verbatim.
    Json,
}

/// Inputs of the rate schedule, and the format to export its worksheet in.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RateScheduleExportRequest {
    /// Tax year.
    pub year: i32,
    /// `single`, `mfj`, `mfs`, `hoh` or `qss`.
    pub filing_status: FilingStatusDto,
    /// Taxable income in cents; not negative, at most 2^53 − 1.
    pub taxable_income: CentsDto,
    /// `csv` or `json`.
    pub format: ExportFormatDto,
}

/// Filing status, in the domain's wire form.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub enum FilingStatusDto {
    /// Single.
    Single,
    /// Married filing jointly.
    Mfj,
    /// Married filing separately.
    Mfs,
    /// Head of household.
    Hoh,
    /// Qualifying surviving spouse.
    Qss,
}

impl From<FilingStatusDto> for pfp_domain::FilingStatus {
    fn from(s: FilingStatusDto) -> Self {
        match s {
            FilingStatusDto::Single => Self::Single,
            FilingStatusDto::Mfj => Self::Mfj,
            FilingStatusDto::Mfs => Self::Mfs,
            FilingStatusDto::Hoh => Self::Hoh,
            FilingStatusDto::Qss => Self::Qss,
        }
    }
}

/// A reference to one parameter cell.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ParamRefDto {
    /// The table id.
    pub param_id: String,
    /// The year read.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub year: Option<i32>,
    /// The breakdown key read (for example a filing status).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub breakdown_key: Option<String>,
    /// The component read (for example a bracket edge).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub element: Option<String>,
}

impl From<&ParamRef> for ParamRefDto {
    fn from(p: &ParamRef) -> Self {
        Self {
            param_id: p.param_id().to_owned(),
            year: p.year(),
            breakdown_key: p.breakdown_key().map(str::to_owned),
            element: p.element().map(str::to_owned),
        }
    }
}

/// One computed value and everything needed to audit it. The worksheet is the
/// ordered list of these; the tree is followed through `inputs`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct LineDto {
    /// Stable id; the prefix names the worksheet.
    pub id: String,
    /// Human-readable name.
    pub label: String,
    /// The value.
    pub value: CentsDto,
    /// Ids of the lines this value was computed from, in formula order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub inputs: Vec<String>,
    /// The parameter cells this value read, in formula order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub params: Vec<ParamRefDto>,
    /// The named rounding rule that produced the value, if one did.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rounding: Option<String>,
}

impl From<&Line> for LineDto {
    fn from(line: &Line) -> Self {
        Self {
            id: line.id.as_str().to_owned(),
            label: line.label.clone().into_owned(),
            value: line.value.into(),
            inputs: line
                .inputs
                .iter()
                .map(|id| id.as_str().to_owned())
                .collect(),
            params: line.params.iter().map(ParamRefDto::from).collect(),
            rounding: line.rounding.as_ref().map(|r| r.as_str().to_owned()),
        }
    }
}

/// The rate-schedule worksheet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RateScheduleResponse {
    /// Echo of the request's year.
    pub year: i32,
    /// Echo of the request's filing status.
    pub filing_status: FilingStatusDto,
    /// Echo of the request's taxable income.
    pub taxable_income: CentsDto,
    /// The tax: the value of the root line.
    pub tax: CentsDto,
    /// Id of the line that is the result.
    pub root_line_id: String,
    /// Every line, in computation order.
    pub lines: Vec<LineDto>,
    /// The vintage the parameters came from.
    pub vintage: VintageSummaryDto,
    /// Whether that vintage is hand-verified. `false` means: not for decisions.
    pub verified: bool,
}

/// A rate as an exact fraction, as the source printed it (`10/100`, not `1/10`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
pub struct RatioDto {
    /// Numerator.
    pub num: i64,
    /// Denominator; positive.
    pub den: i64,
}

impl From<Ratio> for RatioDto {
    fn from(r: Ratio) -> Self {
        Self {
            num: r.num,
            den: r.den,
        }
    }
}

/// One primary source of a table or series.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SourceDto {
    /// Title of the document.
    pub title: String,
    /// Who published it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub publisher: Option<String>,
    /// Where it was retrieved from. Displayed, never fetched by the application.
    pub url: String,
    /// When it was retrieved (`YYYY-MM-DD`).
    pub retrieved: String,
    /// Repository-relative path of the archived copy.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub archive: Option<String>,
    /// SHA-256 of the archived copy.
    pub sha256: String,
    /// Where in the document the values are.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub locator: Option<String>,
    /// The source's own as-of date, when it differs from the table's.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub as_of: Option<String>,
}

fn date(d: DateYmd) -> String {
    d.to_string()
}

impl From<&Source> for SourceDto {
    fn from(s: &Source) -> Self {
        Self {
            title: s.title().to_owned(),
            publisher: s.publisher().map(str::to_owned),
            url: s.url().to_owned(),
            retrieved: date(s.retrieved()),
            archive: s.archive().map(str::to_owned),
            sha256: s.sha256().to_owned(),
            locator: s.locator().map(str::to_owned),
            as_of: s.as_of().map(date),
        }
    }
}

/// The verification status of a table or series.
///
/// The wire forms are exactly `pfp_params::VerificationStatus::wire()` — the
/// kebab-case strings a parameter file carries — so the registry and the files
/// it lists spell one enum one way. `Unstated` has no file form (it is the
/// absence of the field); the API spells it `unstated`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
pub enum VerificationDto {
    /// Transcribed, not yet read back by a human. Not for decisions.
    #[serde(rename = "pending-hand-verification")]
    PendingHandVerification,
    /// Read back against the primary source.
    #[serde(rename = "primary-source-confirmed")]
    PrimarySourceConfirmed,
    /// Derived by hand and reviewed.
    #[serde(rename = "hand-worked-reviewed")]
    HandWorkedReviewed,
    /// The document does not say.
    #[serde(rename = "unstated")]
    Unstated,
}

impl From<VerificationStatus> for VerificationDto {
    fn from(v: VerificationStatus) -> Self {
        match v {
            VerificationStatus::PendingHandVerification => Self::PendingHandVerification,
            VerificationStatus::PrimarySourceConfirmed => Self::PrimarySourceConfirmed,
            VerificationStatus::HandWorkedReviewed => Self::HandWorkedReviewed,
            VerificationStatus::Unstated => Self::Unstated,
        }
    }
}

/// How amounts are rounded when a year is projected.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RoundingDto {
    /// The multiple rounded to, when it is one amount for every key.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub increment: Option<CentsDto>,
    /// The multiple rounded to, per breakdown key, when it differs by key.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub increment_by_key: Option<BTreeMap<String, CentsDto>>,
    /// `down`, `up`, `halfUp`, `halfEven` or `nearest`.
    pub direction: String,
    /// `Amount` or `IncreaseOverBase`.
    pub basis: String,
}

impl From<&RoundingSpec> for RoundingDto {
    fn from(r: &RoundingSpec) -> Self {
        let (increment, increment_by_key) = match &r.increment {
            Increment::Scalar(c) => (Some((*c).into()), None),
            Increment::ByKey(map) => (
                None,
                Some(map.iter().map(|(k, c)| (k.clone(), (*c).into())).collect()),
            ),
        };
        Self {
            increment,
            increment_by_key,
            direction: match r.direction {
                RoundingDirection::Down => "down",
                RoundingDirection::Up => "up",
                RoundingDirection::HalfUp => "halfUp",
                RoundingDirection::HalfEven => "halfEven",
                RoundingDirection::Nearest => "nearest",
            }
            .to_owned(),
            basis: match r.basis {
                RoundingBasis::Amount => "Amount",
                RoundingBasis::IncreaseOverBase => "IncreaseOverBase",
            }
            .to_owned(),
        }
    }
}

/// A key defined as an exact multiple of another key's rounded result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DerivationDto {
    /// The key it derives from.
    pub from: String,
    /// The multiplier.
    pub multiplier: RatioDto,
}

/// How a year the table does not publish is projected.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProjectionDto {
    /// `index`, `wage`, `flat`, `zero` or `schedule`.
    pub rule: String,
    /// The index the law names.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub index: Option<String>,
    /// The archived series that index resolves to.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub index_series: Option<String>,
    /// Which calendar year of the series a tax year reads, as an offset.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lag_years: Option<i32>,
    /// The statutory base year, when it is one year for the whole table.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_year: Option<i32>,
    /// The statutory base year per component, by the first tax year it applies to.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_year_by_component: Option<BTreeMap<String, Vec<i32>>>,
    /// The first tax year whose amounts are adjusted.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_adjusted_year: Option<i32>,
    /// The statutory base amounts per breakdown key, one per component.
    pub base_values: BTreeMap<String, Vec<CentsDto>>,
    /// Keys defined as multiples of another key.
    pub derived: BTreeMap<String, DerivationDto>,
}

impl From<&Projection> for ProjectionDto {
    fn from(p: &Projection) -> Self {
        let (base_year, base_year_by_component) = match &p.base_year {
            None => (None, None),
            Some(BaseYear::Scalar(y)) => (Some(*y), None),
            Some(BaseYear::ByComponent(map)) => (
                None,
                Some(
                    map.iter()
                        .map(|(y, years)| (y.to_string(), years.clone()))
                        .collect(),
                ),
            ),
        };
        Self {
            rule: p.rule.wire().to_owned(),
            index: p.index.clone(),
            index_series: p.index_series.clone(),
            lag_years: p.lag_years,
            base_year,
            base_year_by_component,
            first_adjusted_year: p.first_adjusted_year,
            base_values: p
                .base_values
                .iter()
                .map(|(k, v)| (k.clone(), v.iter().copied().map(CentsDto::from).collect()))
                .collect(),
            derived: p
                .derived
                .iter()
                .map(|(k, d)| {
                    (
                        k.clone(),
                        DerivationDto {
                            from: d.from.clone(),
                            multiplier: d.multiplier.into(),
                        },
                    )
                })
                .collect(),
        }
    }
}

/// One parameter table: an entry of the Assumptions Registry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AssumptionDto {
    /// The table id.
    pub id: String,
    /// The period the amounts apply to (for example `year`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub period: Option<String>,
    /// The unit `values` and the projection's base values are **served** in.
    /// Always `cents`: the engine holds every amount in integer cents.
    ///
    /// This is not the table's declared `unit` (`ARCHITECTURE.md` §6, seam S1).
    /// The declared `unit` and `breakdown` are not published yet: `ParamTable`
    /// parses and validates both but exposes neither. At M0 the loader accepts
    /// only `USD` amounts, `ratio` rates and a `filingStatus` breakdown, so the
    /// keys of `values` are `FilingStatus` wire forms. Both declarations must be
    /// served from the table itself before the M2 override layer validates an
    /// override against the declared unit (`ENGINE-SPEC.md` §15 check 10).
    pub served_unit: String,
    /// The named components of each value, low to high; empty for a single amount.
    pub components: Vec<String>,
    /// The law as in effect on this date (`YYYY-MM-DD`).
    pub as_of: String,
    /// See [`VerificationDto`].
    pub verification: VerificationDto,
    /// The id of the vintage this table belongs to, `<name>@sha256:<content
    /// hash>`: the enclosing [`VintageDto::content_id`], repeated so one entry
    /// taken alone still carries the pin a result records.
    pub vintage_id: String,
    /// See [`ProjectionDto`].
    pub projection: ProjectionDto,
    /// See [`RoundingDto`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rounding: Option<RoundingDto>,
    /// Primary sources.
    pub sources: Vec<SourceDto>,
    /// Published amounts: breakdown key → year → one amount per component.
    pub values: BTreeMap<String, BTreeMap<String, Vec<CentsDto>>>,
    /// The rate ladder in force in each published year; empty when the table has
    /// no rates.
    pub rates: BTreeMap<String, Vec<RatioDto>>,
    /// Questions a human still owes this table.
    pub open_items: Vec<String>,
}

impl AssumptionDto {
    /// The registry entry for `table` of the vintage whose content id is
    /// `vintage_id`.
    #[must_use]
    pub fn of(table: &ParamTable, vintage_id: &str) -> Self {
        let mut values: BTreeMap<String, BTreeMap<String, Vec<CentsDto>>> = BTreeMap::new();
        let mut rates = BTreeMap::new();
        for key in table.keys() {
            for year in table.published_years(key) {
                if let Some(amounts) = table.published(year, key) {
                    values.entry(key.to_owned()).or_default().insert(
                        year.to_string(),
                        amounts.iter().copied().map(CentsDto::from).collect(),
                    );
                }
                if let Some(ladder) = table.rates(year) {
                    rates
                        .entry(year.to_string())
                        .or_insert_with(|| ladder.iter().copied().map(RatioDto::from).collect());
                }
            }
        }
        let projection = table.projection();
        Self {
            id: table.id().to_owned(),
            period: table.period().map(str::to_owned),
            served_unit: "cents".to_owned(),
            components: table.components().to_vec(),
            as_of: date(table.as_of()),
            verification: table.verification().into(),
            vintage_id: vintage_id.to_owned(),
            projection: projection.into(),
            rounding: projection.rounding.as_ref().map(RoundingDto::from),
            sources: table.sources().iter().map(SourceDto::from).collect(),
            values,
            rates,
            open_items: table.open_items().to_vec(),
        }
    }
}

/// Provenance of an archived index series a table projects with.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct IndexSeriesDto {
    /// The series document's id.
    pub id: String,
    /// The name tables refer to it by.
    pub index_series: String,
    /// The publisher's own series identifier.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub series_id: Option<String>,
    /// Data as published on this date (`YYYY-MM-DD`).
    pub as_of: String,
    /// See [`VerificationDto`].
    pub verification: VerificationDto,
    /// The calendar years the series covers.
    pub years: Vec<i32>,
    /// Primary sources.
    pub sources: Vec<SourceDto>,
    /// Questions a human still owes this series.
    pub open_items: Vec<String>,
}

impl From<&IndexSeries> for IndexSeriesDto {
    fn from(s: &IndexSeries) -> Self {
        Self {
            id: s.id().to_owned(),
            index_series: s.index_series().to_owned(),
            series_id: s.series_id().map(str::to_owned),
            as_of: date(s.as_of()),
            verification: s.verification().into(),
            years: s.years().collect(),
            sources: s.sources().iter().map(SourceDto::from).collect(),
            open_items: s.open_items().to_vec(),
        }
    }
}

/// A vintage with every table in it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct VintageDto {
    /// The vintage's name.
    pub name: String,
    /// `name@sha256:…` computed from the content.
    pub content_id: String,
    /// The locked id, when the lock file records this content.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub locked_id: Option<String>,
    /// Whether every table and series has been verified by a human.
    pub verified: bool,
    /// Every parameter table.
    pub tables: Vec<AssumptionDto>,
    /// The index series those tables project with.
    pub index_series: Vec<IndexSeriesDto>,
}

impl From<&Vintage> for VintageDto {
    fn from(v: &Vintage) -> Self {
        let mut series_names: Vec<&str> = v
            .tables()
            .filter_map(|t| t.projection().index_series.as_deref())
            .collect();
        series_names.sort_unstable();
        series_names.dedup();
        let content_id = v.content_id().to_string();
        Self {
            name: v.name().to_owned(),
            locked_id: v.id().map(ToString::to_string),
            verified: v.is_verified(),
            tables: v
                .tables()
                .map(|t| AssumptionDto::of(t, &content_id))
                .collect(),
            content_id,
            index_series: series_names
                .into_iter()
                .filter_map(|name| v.index_series(name))
                .map(IndexSeriesDto::from)
                .collect(),
        }
    }
}

/// The Assumptions Registry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AssumptionsResponse {
    /// Every vintage compiled into this build.
    pub vintages: Vec<VintageDto>,
}

#[cfg(test)]
mod tests {
    use pfp_domain::FilingStatus;

    use super::*;

    #[test]
    fn line_dto_serializes_exactly_as_the_engines_line() {
        // SYNTHETIC input; the worksheet is whatever the engine produces for it.
        let lines =
            pfp_tax::schedule_tax(2026, FilingStatus::Mfj, Cents(12_345_600)).expect("schedule");
        assert!(lines.len() > 3);
        for line in &lines {
            assert_eq!(
                serde_json::to_value(LineDto::from(line)).unwrap(),
                serde_json::to_value(line).unwrap()
            );
        }
    }

    #[test]
    fn filing_status_wire_forms_match_the_domain() {
        for status in FilingStatus::ALL {
            let dto: FilingStatusDto =
                serde_json::from_value(serde_json::Value::String(status.wire().to_owned()))
                    .unwrap();
            assert_eq!(FilingStatus::from(dto), status);
            assert_eq!(serde_json::to_value(dto).unwrap(), status.wire());
        }
    }

    #[test]
    fn rounding_wire_forms_match_the_money_crate() {
        let spec = RoundingSpec {
            increment: Increment::Scalar(Cents(5000)),
            direction: RoundingDirection::HalfEven,
            basis: RoundingBasis::IncreaseOverBase,
        };
        let dto = RoundingDto::from(&spec);
        assert_eq!(
            serde_json::Value::String(dto.direction),
            serde_json::to_value(spec.direction).unwrap()
        );
        assert_eq!(
            serde_json::Value::String(dto.basis),
            serde_json::to_value(spec.basis).unwrap()
        );
    }

    #[test]
    fn verification_wire_forms_match_the_params_crate() {
        for status in [
            VerificationStatus::PendingHandVerification,
            VerificationStatus::PrimarySourceConfirmed,
            VerificationStatus::HandWorkedReviewed,
        ] {
            // Exact equality: a normalizing comparison passes because the two
            // spellings differ, and keeps passing as they drift.
            assert_eq!(
                serde_json::to_value(VerificationDto::from(status)).unwrap(),
                serde_json::Value::String(status.wire().unwrap().to_owned())
            );
        }
        assert_eq!(VerificationStatus::Unstated.wire(), None);
        assert_eq!(
            serde_json::to_value(VerificationDto::from(VerificationStatus::Unstated)).unwrap(),
            "unstated"
        );
    }

    #[test]
    fn a_registry_entry_carries_the_vintage_id_not_the_bare_name() {
        let vintage = pfp_params::shipped::federal_2026().expect("vintage");
        let dto = VintageDto::from(&vintage);
        assert!(dto.content_id.starts_with("federal-2026@"));
        assert!(!dto.tables.is_empty());
        for table in &dto.tables {
            assert_eq!(table.vintage_id, dto.content_id, "{}", table.id);
            assert_eq!(table.served_unit, "cents", "{}", table.id);
        }
    }

    #[test]
    fn secrets_in_dtos_have_no_debug_form() {
        let request = BootstrapRequest {
            token: "ab".repeat(32),
        };
        let response = BootstrapResponse {
            proof: "cd".repeat(32),
        };
        let text = format!("{request:?} {response:?}");
        assert!(!text.contains("abab") && !text.contains("cdcd"), "{text}");
    }
}
