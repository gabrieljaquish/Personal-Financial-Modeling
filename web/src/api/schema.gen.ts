// GENERATED FILE - do not edit. Regenerate with `npm run gen:api`.
// Source: crates/pfp-server/tests/snapshots/openapi__openapi_v1.snap
// Generator: web/scripts/gen-api-types.mjs. tests/api-types-drift.test.mjs fails when this file
// and the snapshot disagree.

/**
 * An accepted request with nothing to report.
 */
export type Accepted = Readonly<Record<string, never>>;

/**
 * One parameter table: an entry of the Assumptions Registry.
 */
export interface AssumptionDto {
  /**
   * The law as in effect on this date (`YYYY-MM-DD`).
   */
  readonly asOf: string;
  /**
   * The named components of each value, low to high; empty for a single amount.
   */
  readonly components: readonly string[];
  /**
   * The table id.
   */
  readonly id: string;
  /**
   * Questions a human still owes this table.
   */
  readonly openItems: readonly string[];
  /**
   * The period the amounts apply to (for example `year`).
   */
  readonly period?: string | null;
  /**
   * See [`ProjectionDto`].
   */
  readonly projection: ProjectionDto;
  /**
   * The rate ladder in force in each published year; empty when the table has
   * no rates.
   */
  readonly rates: Readonly<Record<string, readonly RatioDto[]>>;
  readonly rounding?: RoundingDto | null;
  /**
   * The unit `values` and the projection's base values are **served** in.
   * Always `cents`: the engine holds every amount in integer cents.
   *
   * This is not the table's declared `unit` (`ARCHITECTURE.md` §6, seam S1).
   * The declared `unit` and `breakdown` are not published yet: `ParamTable`
   * parses and validates both but exposes neither. At M0 the loader accepts
   * only `USD` amounts, `ratio` rates and a `filingStatus` breakdown, so the
   * keys of `values` are `FilingStatus` wire forms. Both declarations must be
   * served from the table itself before the M2 override layer validates an
   * override against the declared unit (`ENGINE-SPEC.md` §15 check 10).
   */
  readonly servedUnit: string;
  /**
   * Primary sources.
   */
  readonly sources: readonly SourceDto[];
  /**
   * Published amounts: breakdown key → year → one amount per component.
   */
  readonly values: Readonly<Record<string, Readonly<Record<string, readonly Cents[]>>>>;
  /**
   * See [`VerificationDto`].
   */
  readonly verification: VerificationDto;
  /**
   * The id of the vintage this table belongs to, `<name>@sha256:<content
   * hash>`: the enclosing [`VintageDto::content_id`], repeated so one entry
   * taken alone still carries the pin a result records.
   */
  readonly vintageId: string;
}

/**
 * The Assumptions Registry.
 */
export interface AssumptionsResponse {
  /**
   * Every vintage compiled into this build.
   */
  readonly vintages: readonly VintageDto[];
}

/**
 * The launch token from the URL fragment.
 */
export interface BootstrapRequest {
  /**
   * 64 lower-case hex characters. Single use, 60 s.
   * Minimum length: 64.
   * Maximum length: 64.
   */
  readonly token: string;
}

/**
 * The port-scoped half of the session; the other half is the `__Host-` cookie.
 */
export interface BootstrapResponse {
  /**
   * Goes into `sessionStorage["pfp.proof"]` and then into `X-PFP-Proof`.
   */
  readonly proof: string;
}

/**
 * An amount of money in integer cents.
 *
 * Opaque on purpose: arithmetic on an amount is a compile error. The only ways
 * across the boundary are in `format/money.ts`.
 */
export type Cents = { readonly __cents: unique symbol };

/**
 * A key defined as an exact multiple of another key's rounded result.
 */
export interface DerivationDto {
  /**
   * The key it derives from.
   */
  readonly from: string;
  /**
   * The multiplier.
   */
  readonly multiplier: RatioDto;
}

/**
 * `{"code", "message"}`: the body of every refusal and failure.
 */
export interface ErrorBody {
  /**
   * Stable, machine-readable code.
   */
  readonly code: string;
  /**
   * Fixed human-readable text. Never echoes a request value.
   */
  readonly message: string;
}

/**
 * The file format of an export.
 */
export const ExportFormatDtoValues = ['csv', 'json'] as const;
export type ExportFormatDto = (typeof ExportFormatDtoValues)[number];

/**
 * Filing status, in the domain's wire form.
 */
export const FilingStatusDtoValues = ['single', 'mfj', 'mfs', 'hoh', 'qss'] as const;
export type FilingStatusDto = (typeof FilingStatusDtoValues)[number];

/**
 * Provenance of an archived index series a table projects with.
 */
export interface IndexSeriesDto {
  /**
   * Data as published on this date (`YYYY-MM-DD`).
   */
  readonly asOf: string;
  /**
   * The series document's id.
   */
  readonly id: string;
  /**
   * The name tables refer to it by.
   */
  readonly indexSeries: string;
  /**
   * Questions a human still owes this series.
   */
  readonly openItems: readonly string[];
  /**
   * The publisher's own series identifier.
   */
  readonly seriesId?: string | null;
  /**
   * Primary sources.
   */
  readonly sources: readonly SourceDto[];
  /**
   * See [`VerificationDto`].
   */
  readonly verification: VerificationDto;
  /**
   * The calendar years the series covers.
   */
  readonly years: readonly number[];
}

/**
 * One computed value and everything needed to audit it. The worksheet is the
 * ordered list of these; the tree is followed through `inputs`.
 */
export interface LineDto {
  /**
   * Stable id; the prefix names the worksheet.
   */
  readonly id: string;
  /**
   * Ids of the lines this value was computed from, in formula order.
   */
  readonly inputs?: readonly string[] | null;
  /**
   * Human-readable name.
   */
  readonly label: string;
  /**
   * The parameter cells this value read, in formula order.
   */
  readonly params?: readonly ParamRefDto[] | null;
  /**
   * The named rounding rule that produced the value, if one did.
   */
  readonly rounding?: string | null;
  /**
   * The value.
   */
  readonly value: Cents;
}

/**
 * A reference to one parameter cell.
 */
export interface ParamRefDto {
  /**
   * The breakdown key read (for example a filing status).
   */
  readonly breakdownKey?: string | null;
  /**
   * The component read (for example a bracket edge).
   */
  readonly element?: string | null;
  /**
   * The table id.
   */
  readonly paramId: string;
  /**
   * The year read.
   */
  readonly year?: number | null;
}

/**
 * How a year the table does not publish is projected.
 */
export interface ProjectionDto {
  /**
   * The statutory base amounts per breakdown key, one per component.
   */
  readonly baseValues: Readonly<Record<string, readonly Cents[]>>;
  /**
   * The statutory base year, when it is one year for the whole table.
   */
  readonly baseYear?: number | null;
  /**
   * The statutory base year per component, by the first tax year it applies to.
   */
  readonly baseYearByComponent?: Readonly<Record<string, readonly number[]>> | null;
  /**
   * Keys defined as multiples of another key.
   */
  readonly derived: Readonly<Record<string, DerivationDto>>;
  /**
   * The first tax year whose amounts are adjusted.
   */
  readonly firstAdjustedYear?: number | null;
  /**
   * The index the law names.
   */
  readonly index?: string | null;
  /**
   * The archived series that index resolves to.
   */
  readonly indexSeries?: string | null;
  /**
   * Which calendar year of the series a tax year reads, as an offset.
   */
  readonly lagYears?: number | null;
  /**
   * `index`, `wage`, `flat`, `zero` or `schedule`.
   */
  readonly rule: string;
}

/**
 * Inputs of the rate schedule, and the format to export its worksheet in.
 */
export interface RateScheduleExportRequest {
  /**
   * `single`, `mfj`, `mfs`, `hoh` or `qss`.
   */
  readonly filingStatus: FilingStatusDto;
  /**
   * `csv` or `json`.
   */
  readonly format: ExportFormatDto;
  /**
   * Taxable income in cents; not negative, at most 2^53 − 1.
   */
  readonly taxableIncome: Cents;
  /**
   * Tax year.
   */
  readonly year: number;
}

/**
 * Inputs of the rate schedule.
 */
export interface RateScheduleRequest {
  /**
   * `single`, `mfj`, `mfs`, `hoh` or `qss`.
   */
  readonly filingStatus: FilingStatusDto;
  /**
   * Taxable income in cents; not negative, at most 2^53 − 1.
   */
  readonly taxableIncome: Cents;
  /**
   * Tax year.
   */
  readonly year: number;
}

/**
 * The rate-schedule worksheet.
 */
export interface RateScheduleResponse {
  /**
   * Echo of the request's filing status.
   */
  readonly filingStatus: FilingStatusDto;
  /**
   * Every line, in computation order.
   */
  readonly lines: readonly LineDto[];
  /**
   * Id of the line that is the result.
   */
  readonly rootLineId: string;
  /**
   * The tax: the value of the root line.
   */
  readonly tax: Cents;
  /**
   * Echo of the request's taxable income.
   */
  readonly taxableIncome: Cents;
  /**
   * Whether that vintage is hand-verified. `false` means: not for decisions.
   */
  readonly verified: boolean;
  /**
   * The vintage the parameters came from.
   */
  readonly vintage: VintageSummaryDto;
  /**
   * Echo of the request's year.
   */
  readonly year: number;
}

/**
 * A rate as an exact fraction, as the source printed it (`10/100`, not `1/10`).
 */
export interface RatioDto {
  /**
   * Denominator; positive.
   */
  readonly den: number;
  /**
   * Numerator.
   */
  readonly num: number;
}

/**
 * How amounts are rounded when a year is projected.
 */
export interface RoundingDto {
  /**
   * `Amount` or `IncreaseOverBase`.
   */
  readonly basis: string;
  /**
   * `down`, `up`, `halfUp`, `halfEven` or `nearest`.
   */
  readonly direction: string;
  readonly increment?: Cents | null;
  /**
   * The multiple rounded to, per breakdown key, when it differs by key.
   */
  readonly incrementByKey?: Readonly<Record<string, Cents>> | null;
}

/**
 * Health and session status.
 */
export interface SessionStatus {
  /**
   * The API version: the `v1` of `/api/v1`.
   */
  readonly apiVersion: string;
  /**
   * The application version.
   */
  readonly appVersion: string;
  /**
   * See [`TrustModeDto`].
   */
  readonly trustMode: TrustModeDto;
  /**
   * The parameter vintages compiled into this build.
   */
  readonly vintages: readonly VintageSummaryDto[];
}

/**
 * One primary source of a table or series.
 */
export interface SourceDto {
  /**
   * Repository-relative path of the archived copy.
   */
  readonly archive?: string | null;
  /**
   * The source's own as-of date, when it differs from the table's.
   */
  readonly asOf?: string | null;
  /**
   * Where in the document the values are.
   */
  readonly locator?: string | null;
  /**
   * Who published it.
   */
  readonly publisher?: string | null;
  /**
   * When it was retrieved (`YYYY-MM-DD`).
   */
  readonly retrieved: string;
  /**
   * SHA-256 of the archived copy.
   */
  readonly sha256: string;
  /**
   * Title of the document.
   */
  readonly title: string;
  /**
   * Where it was retrieved from. Displayed, never fetched by the application.
   */
  readonly url: string;
}

/**
 * Whether the local certificate authority is trusted by this user's browsers.
 */
export const TrustModeDtoValues = ['installed', 'declined'] as const;
export type TrustModeDto = (typeof TrustModeDtoValues)[number];

/**
 * The verification status of a table or series.
 *
 * The wire forms are exactly `pfp_params::VerificationStatus::wire()` — the
 * kebab-case strings a parameter file carries — so the registry and the files
 * it lists spell one enum one way. `Unstated` has no file form (it is the
 * absence of the field); the API spells it `unstated`.
 */
export const VerificationDtoValues = ['pending-hand-verification', 'primary-source-confirmed', 'hand-worked-reviewed', 'unstated'] as const;
export type VerificationDto = (typeof VerificationDtoValues)[number];

/**
 * A vintage with every table in it.
 */
export interface VintageDto {
  /**
   * `name@sha256:…` computed from the content.
   */
  readonly contentId: string;
  /**
   * The index series those tables project with.
   */
  readonly indexSeries: readonly IndexSeriesDto[];
  /**
   * The locked id, when the lock file records this content.
   */
  readonly lockedId?: string | null;
  /**
   * The vintage's name.
   */
  readonly name: string;
  /**
   * Every parameter table.
   */
  readonly tables: readonly AssumptionDto[];
  /**
   * Whether every table and series has been verified by a human.
   */
  readonly verified: boolean;
}

/**
 * A parameter vintage's identity, without its tables.
 */
export interface VintageSummaryDto {
  /**
   * `name@sha256:…` computed from the content; claims nothing by itself.
   */
  readonly contentId: string;
  /**
   * The locked id, present only when `params/VINTAGES.lock` records this content.
   */
  readonly lockedId?: string | null;
  /**
   * The vintage's name.
   */
  readonly name: string;
  /**
   * Whether every table and series has been verified by a human.
   */
  readonly verified: boolean;
}

/** Every API operation: request body, success body and statuses. All are POST. */
export interface Operations {
  /**
   * Lists every vintage compiled into this build, with every table in it.
   */
  readonly '/api/v1/assumptions/list': {
    readonly request: undefined;
    readonly ok: AssumptionsResponse;
    readonly okStatus: 200;
    readonly expect: 'json';
    readonly errorStatuses: 401 | 409;
  };
  /**
   * Exchanges the launch token for the session pair.
   */
  readonly '/api/v1/session/bootstrap': {
    readonly request: BootstrapRequest;
    readonly ok: BootstrapResponse;
    readonly okStatus: 200;
    readonly expect: 'json';
    readonly errorStatuses: 401;
  };
  /**
   * Asks the running application to re-open itself in the browser: the recovery
   * from a displaced cookie. Needs the proof only. The fresh launch token goes to
   * the opener inside the process and is never in this response.
   */
  readonly '/api/v1/session/relaunch': {
    readonly request: undefined;
    readonly ok: Accepted;
    readonly okStatus: 202;
    readonly expect: 'json';
    readonly errorStatuses: 401 | 429 | 503;
  };
  /**
   * Health, versions and trust mode.
   */
  readonly '/api/v1/session/status': {
    readonly request: undefined;
    readonly ok: SessionStatus;
    readonly okStatus: 200;
    readonly expect: 'json';
    readonly errorStatuses: 401 | 409;
  };
  /**
   * Computes the ordinary-income rate schedule and returns the whole worksheet.
   */
  readonly '/api/v1/tax/rate-schedule': {
    readonly request: RateScheduleRequest;
    readonly ok: RateScheduleResponse;
    readonly okStatus: 200;
    readonly expect: 'json';
    readonly errorStatuses: 400 | 401 | 409 | 422;
  };
  /**
   * Exports the rate-schedule worksheet as a CSV or JSON attachment.
   *
   * The worksheet is recomputed from the inputs; the client never posts lines
   * back. CSV text cells carry spreadsheet formula-injection escaping and amounts
   * are bare numbers; the JSON is the worksheet's values verbatim.
   */
  readonly '/api/v1/tax/rate-schedule/export': {
    readonly request: RateScheduleExportRequest;
    readonly ok: string;
    readonly okStatus: 200;
    readonly expect: 'text';
    readonly errorStatuses: 400 | 401 | 409 | 422;
  };
}

export const API_PATHS = [
  '/api/v1/assumptions/list',
  '/api/v1/session/bootstrap',
  '/api/v1/session/relaunch',
  '/api/v1/session/status',
  '/api/v1/tax/rate-schedule',
  '/api/v1/tax/rate-schedule/export',
] as const;
