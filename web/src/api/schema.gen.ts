// GENERATED FILE - do not edit. Regenerate with `npm run gen:api`.
// Source: crates/pfp-server/tests/snapshots/openapi__openapi_v1.snap
// Generator: web/scripts/gen-api-types.mjs. tests/api-types-drift.test.mjs fails when this file
// and the snapshot disagree.

/**
 * An accepted request with nothing to report.
 */
export type Accepted = Readonly<Record<string, never>>;

/**
 * How the archived copies a document's `[[source]]` blocks cite check out.
 */
export interface ArchiveCheck {
  /**
   * Sources naming an archived copy under `params/provenance/`.
   * Minimum: 0.
   */
  readonly archivedCount: number;
  /**
   * Archived copies whose SHA-256 equals the recorded one.
   * Minimum: 0.
   */
  readonly checksumMatchCount: number;
  /**
   * Archived copies whose SHA-256 differs from the recorded one.
   * Minimum: 0.
   */
  readonly checksumMismatchCount: number;
  /**
   * Sources naming an archive path that does not exist.
   * Minimum: 0.
   */
  readonly missingCount: number;
  /**
   * `[[source]]` blocks.
   * Minimum: 0.
   */
  readonly sourceCount: number;
}

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
 * A count keyed by a name (a milestone, a module, a verification value, a crate).
 */
export interface CountByKey {
  /**
   * How many.
   * Minimum: 0.
   */
  readonly count: number;
  /**
   * The name counted under (a milestone, a module, a verification value, a crate).
   */
  readonly name: string;
}

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
 * One fixture document, as its envelope declares it (`TESTING.md` §2.2).
 */
export interface FixtureFile {
  /**
   * How many `expect.cases` or `expect.lines` the document carries.
   * Minimum: 0.
   */
  readonly caseCount: number;
  /**
   * The envelope's `id`.
   */
  readonly id?: string | null;
  /**
   * The milestone it must be green in.
   */
  readonly milestone?: string | null;
  /**
   * The engine crate it drives (`module`).
   */
  readonly module?: string | null;
  /**
   * The `paramVintage` pin, when one is recorded.
   */
  readonly paramVintage?: string | null;
  /**
   * Repository-relative path.
   */
  readonly path: string;
  /**
   * The tier-1 id a pending fixture promotes to.
   */
  readonly promotesTo?: string | null;
  /**
   * `reviewedBy.kind` of a hand-worked-reviewed fixture.
   */
  readonly reviewKind?: string | null;
  /**
   * The `synthetic` marker, when declared.
   */
  readonly synthetic?: boolean | null;
  /**
   * The `verification` value, or `unstated`.
   */
  readonly verification: string;
}

/**
 * A fixture count for one milestone and one verification value.
 */
export interface FixtureGroup {
  /**
   * How many fixtures carry both.
   * Minimum: 0.
   */
  readonly count: number;
  /**
   * The fixture's declared `milestone`.
   */
  readonly milestone: string;
  /**
   * The fixture's declared `verification`.
   */
  readonly verification: string;
}

/**
 * The fixture corpus by tier, milestone and verification value.
 */
export interface FixtureReport {
  /**
   * Files that could not be read as envelopes.
   */
  readonly invalid: readonly InvalidFixture[];
  /**
   * Other `fixtures/` directories that exist (`personas`, `plans`, …) with their file counts.
   */
  readonly otherDirectories: readonly CountByKey[];
  /**
   * See [`SectionStatus`].
   */
  readonly status: SectionStatus;
  /**
   * The four directories, always all four.
   */
  readonly tiers: readonly TierReport[];
}

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
 * A file under `fixtures/` that is not a readable envelope.
 */
export interface InvalidFixture {
  /**
   * Repository-relative path.
   */
  readonly path: string;
  /**
   * What is wrong with it (never its content).
   */
  readonly problem: string;
}

/**
 * One row of the invariant inventory (`TESTING.md` §4).
 */
export interface Invariant {
  /**
   * The crate that guards it.
   */
  readonly crateName: string;
  /**
   * `I1`, `I4b`, …
   */
  readonly id: string;
  /**
   * The milestone that introduces it.
   */
  readonly milestone: string;
  /**
   * Its name.
   */
  readonly name: string;
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
 * `params/VINTAGES.lock`.
 */
export interface LockReport {
  /**
   * Files it names.
   * Minimum: 0.
   */
  readonly entryCount: number;
  /**
   * Vintage directories it covers.
   */
  readonly lockedVintages: readonly string[];
  /**
   * Whether the file exists.
   */
  readonly present: boolean;
}

/**
 * An open hand-verification question recorded in a parameter document.
 */
export interface OpenItem {
  /**
   * The table or series id.
   */
  readonly document: string;
  /**
   * The question.
   */
  readonly item: string;
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
 * Parameter vintages, the lock and the provenance archive.
 */
export interface ParameterReport {
  /**
   * See [`LockReport`].
   */
  readonly lock: LockReport;
  /**
   * See [`ProvenanceReport`].
   */
  readonly provenance: ProvenanceReport;
  /**
   * See [`SectionStatus`].
   */
  readonly status: SectionStatus;
  /**
   * Every vintage under `params/vintages/`.
   */
  readonly vintages: readonly VintageReport[];
}

/**
 * One performance budget (`TESTING.md` §10).
 */
export interface PerformanceBudget {
  /**
   * The budget's name.
   */
  readonly budget: string;
  /**
   * The milestone it gates from.
   */
  readonly milestone: string;
  /**
   * The threshold as the design states it.
   */
  readonly threshold: string;
}

/**
 * Performance budgets.
 */
export interface PerformanceReport {
  /**
   * The budgets the design states; none is measured until a bench exists.
   */
  readonly budgets: readonly PerformanceBudget[];
  /**
   * See [`SectionStatus`].
   */
  readonly status: SectionStatus;
}

/**
 * What a result would be pinned to (ADR-010), as far as anything exists.
 */
export interface Pins {
  /**
   * The workspace version the report was generated for.
   */
  readonly applicationVersion: string;
  /**
   * The binary digest; absent until the release pipeline exists.
   */
  readonly binaryDigest?: string | null;
  /**
   * `engineVersion`; absent until the engine declares one.
   */
  readonly engineVersion?: string | null;
  /**
   * The licence declared in the workspace manifest.
   */
  readonly licence: string;
  /**
   * Ids of the vintages the lock covers.
   */
  readonly lockedParamVintageIds: readonly string[];
  /**
   * Why the absent pins are absent.
   */
  readonly note: string;
  /**
   * Content ids of every vintage, locked or not.
   */
  readonly paramVintageContentIds: readonly string[];
  /**
   * The plan `schemaVersion`; absent until the plan schema exists.
   */
  readonly schemaVersion?: string | null;
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
 * One source file holding `proptest!` properties.
 */
export interface PropertyFile {
  /**
   * The `cases:` budget the file's `ProptestConfig` states, when it states one.
   * Minimum: 0.
   */
  readonly caseCount?: number | null;
  /**
   * The crate.
   */
  readonly crate: string;
  /**
   * Repository-relative path.
   */
  readonly path: string;
  /**
   * The property functions, in file order.
   */
  readonly propertyNames: readonly string[];
}

/**
 * Property-based and metamorphic tests.
 */
export interface PropertyReport {
  /**
   * Files holding `proptest!` blocks.
   */
  readonly files: readonly PropertyFile[];
  /**
   * The design's invariant inventory.
   */
  readonly invariants: readonly Invariant[];
  /**
   * Invariants whose milestone is M0.
   */
  readonly invariantsDueAtM0: readonly string[];
  /**
   * Property functions across those files.
   * Minimum: 0.
   */
  readonly propertyCount: number;
  /**
   * See [`SectionStatus`].
   */
  readonly status: SectionStatus;
}

/**
 * `params/provenance/`: the archived primary documents.
 */
export interface ProvenanceReport {
  /**
   * `[[document]]` entries in `params/provenance/INDEX.toml`.
   * Minimum: 0.
   */
  readonly cataloguedCount: number;
  /**
   * Catalogued documents whose file hashes to the recorded SHA-256.
   * Minimum: 0.
   */
  readonly checksumMatchCount: number;
  /**
   * Catalogued documents whose file hashes differently.
   * Minimum: 0.
   */
  readonly checksumMismatchCount: number;
  /**
   * Files under the directory, catalogues included.
   * Minimum: 0.
   */
  readonly fileCount: number;
  /**
   * Catalogued documents whose file is absent.
   * Minimum: 0.
   */
  readonly missingCount: number;
  /**
   * The catalogue's own `verification` field, verbatim.
   */
  readonly verification?: string | null;
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
 * One row of `TESTING.md` §5.3: a target the corpus refuses to encode.
 */
export interface RefusedTarget {
  /**
   * Why, and what replaces it.
   */
  readonly reason: string;
  /**
   * The target.
   */
  readonly target: string;
}

/**
 * Whether this build carries a validation report.
 */
export const ReportStateDtoValues = ['generated', 'not-generated'] as const;
export type ReportStateDto = (typeof ReportStateDtoValues)[number];

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
 * Whether a corpus the design specifies exists at this commit.
 */
export const SectionStateValues = ['not-yet-introduced', 'empty', 'partial', 'present'] as const;
export type SectionState = (typeof SectionStateValues)[number];

/**
 * The status line every section carries.
 */
export interface SectionStatus {
  /**
   * The milestone the design introduces the section in (`M0`, `M1`, …).
   */
  readonly milestone: string;
  /**
   * What is and is not counted, in one or two sentences.
   */
  readonly note: string;
  /**
   * Where the design specifies it (a document and section).
   */
  readonly reference: string;
  /**
   * See [`SectionState`].
   */
  readonly state: SectionState;
}

/**
 * One area of the security test matrix (`TESTING.md` §8).
 */
export interface SecurityArea {
  /**
   * The area.
   */
  readonly area: string;
  /**
   * The milestone it is green from.
   */
  readonly milestone: string;
}

/**
 * The security suite.
 */
export interface SecurityReport {
  /**
   * The matrix's areas with their milestones.
   */
  readonly areas: readonly SecurityArea[];
  /**
   * See [`SectionStatus`].
   */
  readonly status: SectionStatus;
  /**
   * Test ids (`S-NN`) named in test sources.
   */
  readonly testIdsNamed: readonly string[];
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
   * The application's licence, as the workspace manifest declares it (SPDX).
   */
  readonly licence: string;
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
 * Contract snapshots (`insta`): the `OpenAPI` document, the header set, golden bodies.
 */
export interface SnapshotReport {
  /**
   * `.snap` files by crate.
   */
  readonly byCrate: readonly CountByKey[];
  /**
   * `.snap` files in total.
   * Minimum: 0.
   */
  readonly fileCount: number;
  /**
   * See [`SectionStatus`].
   */
  readonly status: SectionStatus;
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
 * One parameter table or index series of a vintage.
 */
export interface TableReport {
  /**
   * See [`ArchiveCheck`].
   */
  readonly archive: ArchiveCheck;
  /**
   * The table or series id.
   */
  readonly id: string;
  /**
   * `table` or `index-series`.
   */
  readonly kind: string;
  /**
   * Hand-verification questions still open.
   * Minimum: 0.
   */
  readonly openItemCount: number;
  /**
   * The open questions themselves.
   */
  readonly openItems: readonly string[];
  /**
   * The `verification` value, or `unstated`.
   */
  readonly verification: string;
}

/**
 * Test functions declared in the source tree: a count of attributes, not a run.
 */
export interface TestInventory {
  /**
   * `#[test]` and `#[tokio::test]` attributes per crate.
   */
  readonly byCrate: readonly CountByKey[];
  /**
   * What the numbers are and are not.
   */
  readonly note: string;
  /**
   * `test(` declarations in `web/tests/*.test.mjs`.
   * Minimum: 0.
   */
  readonly webTestCount: number;
}

/**
 * One tier directory of `fixtures/` (`TESTING.md` §2.2), plus `pending/`.
 */
export interface TierReport {
  /**
   * Counts by declared milestone.
   */
  readonly byMilestone: readonly CountByKey[];
  /**
   * Counts by milestone and verification together.
   */
  readonly byMilestoneAndVerification: readonly FixtureGroup[];
  /**
   * Counts by declared module.
   */
  readonly byModule: readonly CountByKey[];
  /**
   * Counts by declared verification value.
   */
  readonly byVerification: readonly CountByKey[];
  /**
   * The directory, repository-relative.
   */
  readonly directory: string;
  /**
   * Fixture documents in the directory (Markdown notes excluded).
   * Minimum: 0.
   */
  readonly fileCount: number;
  /**
   * Every fixture document.
   */
  readonly files: readonly FixtureFile[];
  /**
   * `reviewedBy.kind` counts (`second-reviewer`, `cooling-off-re-review`).
   */
  readonly reviewKinds: readonly CountByKey[];
  /**
   * See [`SectionStatus`].
   */
  readonly status: SectionStatus;
  /**
   * `tier1`, `tier2`, `tier3` or `pending`.
   */
  readonly tier: string;
}

/**
 * Whether the local certificate authority is trusted by this user's browsers.
 */
export const TrustModeDtoValues = ['installed', 'declined'] as const;
export type TrustModeDto = (typeof TrustModeDtoValues)[number];

/**
 * One row of `TESTING.md` §5.2: a value no locked vintage may carry yet.
 */
export interface UnverifiedItem {
  /**
   * The gate, verbatim.
   */
  readonly gate: string;
  /**
   * The item and why it is open.
   */
  readonly item: string;
  /**
   * The milestone named in the gate, or `not stated`.
   */
  readonly milestone: string;
}

/**
 * The `unverified` block: printed, not hidden (`TESTING.md` §2.2, §5.2, §13).
 */
export interface UnverifiedReport {
  /**
   * `TESTING.md` §5.2, row by row.
   */
  readonly items: readonly UnverifiedItem[];
  /**
   * Open questions the parameter documents record.
   */
  readonly parameterOpenItems: readonly OpenItem[];
  /**
   * Fixtures awaiting a human read-back (`fixtures/pending/`).
   * Minimum: 0.
   */
  readonly pendingFixtureCount: number;
  /**
   * Parameter documents (tables and series) still pending hand verification.
   * Minimum: 0.
   */
  readonly pendingParameterDocumentCount: number;
  /**
   * Tier-1 fixtures computed by this project and reviewed.
   * Minimum: 0.
   */
  readonly tier1HandWorkedReviewedCount: number;
  /**
   * Tier-1 fixtures transcribed from a publication.
   * Minimum: 0.
   */
  readonly tier1PrimarySourceConfirmedCount: number;
}

/**
 * The validation report.
 */
export interface ValidationReport {
  /**
   * How the report was computed.
   */
  readonly basis: string;
  /**
   * Browser end-to-end journeys (`TESTING.md` §9).
   */
  readonly browserEndToEnd: SectionStatus;
  /**
   * See [`SnapshotReport`].
   */
  readonly contractSnapshots: SnapshotReport;
  /**
   * See [`FixtureReport`].
   */
  readonly fixtures: FixtureReport;
  /**
   * Fuzz corpora (`TESTING.md` §11.3).
   */
  readonly fuzzCorpora: SectionStatus;
  /**
   * A date given explicitly to the generator; never read from a clock.
   */
  readonly generatedOn?: string | null;
  /**
   * The surviving-mutant budget (`TESTING.md` §11.3).
   */
  readonly mutationScore: SectionStatus;
  /**
   * Recorded out-of-process oracles (`TESTING.md` §7).
   */
  readonly oracles: SectionStatus;
  /**
   * See [`ParameterReport`].
   */
  readonly parameters: ParameterReport;
  /**
   * See [`PerformanceReport`].
   */
  readonly performanceBudgets: PerformanceReport;
  /**
   * See [`Pins`].
   */
  readonly pins: Pins;
  /**
   * See [`PropertyReport`].
   */
  readonly properties: PropertyReport;
  /**
   * See [`RefusedTarget`].
   */
  readonly refused: readonly RefusedTarget[];
  /**
   * See [`SecurityReport`].
   */
  readonly securitySuite: SecurityReport;
  /**
   * See [`TestInventory`].
   */
  readonly testInventory: TestInventory;
  /**
   * Tier-2 pinned suites (`TESTING.md` §3.5).
   */
  readonly tier2Suites: SectionStatus;
  /**
   * Tier-3 goldens (`TESTING.md` §3.6).
   */
  readonly tier3Goldens: SectionStatus;
  /**
   * See [`UnverifiedReport`].
   */
  readonly unverified: UnverifiedReport;
}

/**
 * The validation report endpoint's answer.
 */
export interface ValidationReportResponse {
  readonly report?: ValidationReport | null;
  /**
   * See [`ReportStateDto`].
   */
  readonly state: ReportStateDto;
}

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
 * One parameter vintage (`ARCHITECTURE.md` §6, ADR-010).
 */
export interface VintageReport {
  /**
   * See [`ArchiveCheck`], summed over the vintage.
   */
  readonly archive: ArchiveCheck;
  /**
   * `<name>@<sha256>` as `pfp-params` computes it; claims nothing by itself.
   */
  readonly contentId: string;
  /**
   * Every table and series.
   */
  readonly documents: readonly TableReport[];
  /**
   * Archived index series the tables read.
   * Minimum: 0.
   */
  readonly indexSeriesCount: number;
  /**
   * Whether `params/VINTAGES.lock` covers this vintage.
   */
  readonly locked: boolean;
  /**
   * The id a result may be pinned to: the content id, only when locked.
   */
  readonly lockedId?: string | null;
  /**
   * The vintage name.
   */
  readonly name: string;
  /**
   * Open hand-verification questions across the vintage.
   * Minimum: 0.
   */
  readonly openItemCount: number;
  /**
   * Parameter tables.
   * Minimum: 0.
   */
  readonly tableCount: number;
  /**
   * The verification values present, worst first.
   */
  readonly verification: readonly CountByKey[];
  /**
   * Whether every table and series carries a human sign-off.
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
  /**
   * The validation report compiled into this build, or the `not-generated` state.
   */
  readonly '/api/v1/validation/report': {
    readonly request: undefined;
    readonly ok: ValidationReportResponse;
    readonly okStatus: 200;
    readonly expect: 'json';
    readonly errorStatuses: 401 | 409;
  };
}

export const API_PATHS = [
  '/api/v1/assumptions/list',
  '/api/v1/session/bootstrap',
  '/api/v1/session/relaunch',
  '/api/v1/session/status',
  '/api/v1/tax/rate-schedule',
  '/api/v1/tax/rate-schedule/export',
  '/api/v1/validation/report',
] as const;
