//! The validation report (`TESTING.md` §13, `PLAN.md` §4.1 and §4.13 item 8,
//! ADR-022): what corpora exist at a commit, computed from the repository and
//! never from a clock, a host or a person.
//!
//! One definition, two readers. `cargo xtask validation-report` builds the report
//! from the tree and writes it as JSON; `pfp-server` embeds that JSON at build
//! time and serves it. So that the two cannot disagree about the shape, this file
//! is compiled into **both** crates: here as `pfp_server::api::report_dto`, and in
//! `xtask` by `#[path]` inclusion (the precedent is `build_support/web_inputs.rs`).
//! It therefore depends on `serde` and `utoipa` only, and every struct refuses
//! unknown fields, so an embedded report written by an older or newer `xtask`
//! fails to parse instead of being served with a hole in it.
//!
//! Every integer here is a **count** and is named with the suffix `Count`. No
//! field is money; the report carries no amount of any kind.
//!
//! **Honesty rules.** A section that the design specifies but the tree does not
//! yet hold is present with [`SectionState::NotYetIntroduced`] and the milestone
//! that introduces it, never omitted. Counts are what the tree contains, never a
//! target. `pending-hand-verification` material is never counted as verified.

// Read by two crates; in `xtask` the module is private, so the items are not
// reachable from that crate root although they are from this one.
#![allow(unreachable_pub)]

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Whether a corpus the design specifies exists at this commit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "kebab-case")]
pub enum SectionState {
    /// Nothing of it exists yet; `milestone` says when the design introduces it.
    NotYetIntroduced,
    /// The design places it at or before this commit's milestone, and nothing is
    /// in it yet: a tier-1 directory with no promoted fixture.
    Empty,
    /// Part of it exists and is counted; the note says what does not.
    Partial,
    /// It exists and is counted.
    Present,
}

/// The status line every section carries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SectionStatus {
    /// See [`SectionState`].
    pub state: SectionState,
    /// The milestone the design introduces the section in (`M0`, `M1`, …).
    pub milestone: String,
    /// Where the design specifies it (a document and section).
    pub reference: String,
    /// What is and is not counted, in one or two sentences.
    pub note: String,
}

/// A count keyed by a name (a milestone, a module, a verification value, a crate).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CountByKey {
    /// The name counted under (a milestone, a module, a verification value, a crate).
    pub name: String,
    /// How many.
    pub count: u32,
}

/// A fixture count for one milestone and one verification value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FixtureGroup {
    /// The fixture's declared `milestone`.
    pub milestone: String,
    /// The fixture's declared `verification`.
    pub verification: String,
    /// How many fixtures carry both.
    pub count: u32,
}

/// One fixture document, as its envelope declares it (`TESTING.md` §2.2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FixtureFile {
    /// Repository-relative path.
    pub path: String,
    /// The envelope's `id`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// The engine crate it drives (`module`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub module: Option<String>,
    /// The milestone it must be green in.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub milestone: Option<String>,
    /// The `verification` value, or `unstated`.
    pub verification: String,
    /// The `synthetic` marker, when declared.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub synthetic: Option<bool>,
    /// The tier-1 id a pending fixture promotes to.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub promotes_to: Option<String>,
    /// The `paramVintage` pin, when one is recorded.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub param_vintage: Option<String>,
    /// `reviewedBy.kind` of a hand-worked-reviewed fixture.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub review_kind: Option<String>,
    /// How many `expect.cases` or `expect.lines` the document carries.
    pub case_count: u32,
}

/// One directory of `fixtures/` the design names (`TESTING.md` §2.2: the three
/// tiers, `personas/` and `plans/`), plus `pending/`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TierReport {
    /// `tier1`, `tier2`, `tier3`, `personas`, `plans` or `pending`.
    pub tier: String,
    /// The directory, repository-relative.
    pub directory: String,
    /// See [`SectionStatus`].
    pub status: SectionStatus,
    /// Fixture documents in the directory (Markdown notes excluded).
    pub file_count: u32,
    /// Counts by declared milestone.
    pub by_milestone: Vec<CountByKey>,
    /// Counts by declared verification value.
    pub by_verification: Vec<CountByKey>,
    /// Counts by declared module.
    pub by_module: Vec<CountByKey>,
    /// Counts by milestone and verification together.
    pub by_milestone_and_verification: Vec<FixtureGroup>,
    /// `reviewedBy.kind` counts (`second-reviewer`, `cooling-off-re-review`).
    pub review_kinds: Vec<CountByKey>,
    /// Every fixture document.
    pub files: Vec<FixtureFile>,
}

/// A file under `fixtures/` that is not a readable envelope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InvalidFixture {
    /// Repository-relative path.
    pub path: String,
    /// What is wrong with it (never its content).
    pub problem: String,
}

/// The fixture corpus by tier, milestone and verification value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FixtureReport {
    /// See [`SectionStatus`].
    pub status: SectionStatus,
    /// The six directories the design names, always all six, whether or not
    /// each exists at this commit.
    pub tiers: Vec<TierReport>,
    /// Files that could not be read as envelopes.
    pub invalid: Vec<InvalidFixture>,
    /// Other `fixtures/` directories that exist (`personas`, `plans`, …) with their file counts.
    pub other_directories: Vec<CountByKey>,
}

/// How the archived copies a document's `[[source]]` blocks cite check out.
// Every field is a count, by the rule stated at the top of this file.
#[allow(clippy::struct_field_names)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ArchiveCheck {
    /// `[[source]]` blocks.
    pub source_count: u32,
    /// Sources naming an archived copy under `params/provenance/`.
    pub archived_count: u32,
    /// Archived copies whose SHA-256 equals the recorded one.
    pub checksum_match_count: u32,
    /// Archived copies whose SHA-256 differs from the recorded one.
    pub checksum_mismatch_count: u32,
    /// Sources naming an archive path that does not exist.
    pub missing_count: u32,
}

/// One parameter table or index series of a vintage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TableReport {
    /// The table or series id.
    pub id: String,
    /// `table` or `index-series`.
    pub kind: String,
    /// The `verification` value, or `unstated`.
    pub verification: String,
    /// Hand-verification questions still open.
    pub open_item_count: u32,
    /// The open questions themselves.
    pub open_items: Vec<String>,
    /// See [`ArchiveCheck`].
    pub archive: ArchiveCheck,
}

/// One parameter vintage (`ARCHITECTURE.md` §6, ADR-010).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VintageReport {
    /// The vintage name.
    pub name: String,
    /// `<name>@<sha256>` as `pfp-params` computes it; claims nothing by itself.
    pub content_id: String,
    /// Whether `params/VINTAGES.lock` covers this vintage.
    pub locked: bool,
    /// The id a result may be pinned to: the content id, only when locked.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub locked_id: Option<String>,
    /// Whether every table and series carries a human sign-off.
    pub verified: bool,
    /// The verification values present, worst first.
    pub verification: Vec<CountByKey>,
    /// Parameter tables.
    pub table_count: u32,
    /// Archived index series the tables read.
    pub index_series_count: u32,
    /// Open hand-verification questions across the vintage.
    pub open_item_count: u32,
    /// Every table and series.
    pub documents: Vec<TableReport>,
    /// See [`ArchiveCheck`], summed over the vintage.
    pub archive: ArchiveCheck,
}

/// `params/VINTAGES.lock`, verified against the files rather than taken on
/// trust: an entry counts only when its file hashes to the recorded SHA-256,
/// and a vintage counts as locked only when every file under its directory is
/// listed and every entry for it matches (`SECURITY.md` §13.3 rule 4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LockReport {
    /// Whether the file exists.
    pub present: bool,
    /// Files it names.
    pub entry_count: u32,
    /// Vintage directories it names **and** verifies; only these are locked.
    pub locked_vintages: Vec<String>,
    /// Vintage directories it names but does not verify: an entry does not
    /// match, a named file is missing, or a file in the directory is unlisted.
    pub unverified_vintages: Vec<String>,
    /// Entries whose file hashes differently from the recorded SHA-256.
    pub checksum_mismatch_count: u32,
    /// Entries naming a file that does not exist.
    pub missing_count: u32,
    /// Files under a named vintage directory with no entry.
    pub unlisted_count: u32,
    /// What the verification found, in words.
    pub note: String,
}

/// `params/provenance/`: the archived primary documents.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProvenanceReport {
    /// Files under the directory, catalogues included.
    pub file_count: u32,
    /// `[[document]]` entries in `params/provenance/INDEX.toml`.
    pub catalogued_count: u32,
    /// Catalogued documents whose file hashes to the recorded SHA-256.
    pub checksum_match_count: u32,
    /// Catalogued documents whose file hashes differently.
    pub checksum_mismatch_count: u32,
    /// Catalogued documents whose file is absent.
    pub missing_count: u32,
    /// The catalogue's own `verification` field, verbatim.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verification: Option<String>,
}

/// Parameter vintages, the lock and the provenance archive.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ParameterReport {
    /// See [`SectionStatus`].
    pub status: SectionStatus,
    /// Every vintage under `params/vintages/`.
    pub vintages: Vec<VintageReport>,
    /// See [`LockReport`].
    pub lock: LockReport,
    /// See [`ProvenanceReport`].
    pub provenance: ProvenanceReport,
}

/// One source file holding `proptest!` properties.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PropertyFile {
    /// The crate.
    #[serde(rename = "crate")]
    pub crate_name: String,
    /// Repository-relative path.
    pub path: String,
    /// The `cases:` budget the file's `ProptestConfig` states, when it states one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub case_count: Option<u32>,
    /// The property functions, in file order.
    pub property_names: Vec<String>,
}

/// One row of the invariant inventory (`TESTING.md` §4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Invariant {
    /// `I1`, `I4b`, …
    pub id: String,
    /// Its name.
    pub name: String,
    /// The crate that guards it.
    pub crate_name: String,
    /// The milestone that introduces it.
    pub milestone: String,
}

/// Property-based and metamorphic tests.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PropertyReport {
    /// See [`SectionStatus`].
    pub status: SectionStatus,
    /// Files holding `proptest!` blocks.
    pub files: Vec<PropertyFile>,
    /// Property functions across those files.
    pub property_count: u32,
    /// The design's invariant inventory.
    pub invariants: Vec<Invariant>,
    /// Invariants whose milestone is M0.
    pub invariants_due_at_m0: Vec<String>,
}

/// Contract snapshots (`insta`): the `OpenAPI` document, the header set, golden bodies.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SnapshotReport {
    /// See [`SectionStatus`].
    pub status: SectionStatus,
    /// `.snap` files by crate.
    pub by_crate: Vec<CountByKey>,
    /// `.snap` files in total.
    pub file_count: u32,
}

/// One area of the security test matrix (`TESTING.md` §8).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SecurityArea {
    /// The area.
    pub area: String,
    /// The milestone it is green from.
    pub milestone: String,
}

/// The security suite.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SecurityReport {
    /// See [`SectionStatus`].
    pub status: SectionStatus,
    /// The matrix's areas with their milestones.
    pub areas: Vec<SecurityArea>,
    /// Test ids (`S-NN`) named in test sources.
    pub test_ids_named: Vec<String>,
}

/// One performance budget (`TESTING.md` §10).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PerformanceBudget {
    /// The budget's name.
    pub budget: String,
    /// The threshold as the design states it.
    pub threshold: String,
    /// The milestone it gates from.
    pub milestone: String,
}

/// Performance budgets.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PerformanceReport {
    /// See [`SectionStatus`].
    pub status: SectionStatus,
    /// The budgets the design states; none is measured until a bench exists.
    pub budgets: Vec<PerformanceBudget>,
}

/// Test functions declared in the source tree: a count of attributes, not a run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TestInventory {
    /// `#[test]` and `#[tokio::test]` attributes per crate.
    pub by_crate: Vec<CountByKey>,
    /// `test(` declarations in `web/tests/*.test.mjs`.
    pub web_test_count: u32,
    /// What the numbers are and are not.
    pub note: String,
}

/// One row of `TESTING.md` §5.2: a value no locked vintage may carry yet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UnverifiedItem {
    /// The item and why it is open.
    pub item: String,
    /// The gate, verbatim.
    pub gate: String,
    /// The milestone named in the gate, or `not stated`.
    pub milestone: String,
}

/// An open hand-verification question recorded in a parameter document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OpenItem {
    /// The table or series id.
    pub document: String,
    /// The question.
    pub item: String,
}

/// The `unverified` block: printed, not hidden (`TESTING.md` §2.2, §5.2, §13).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UnverifiedReport {
    /// Tier-1 fixtures transcribed from a publication.
    pub tier1_primary_source_confirmed_count: u32,
    /// Tier-1 fixtures computed by this project and reviewed.
    pub tier1_hand_worked_reviewed_count: u32,
    /// Fixtures awaiting a human read-back (`fixtures/pending/`).
    pub pending_fixture_count: u32,
    /// Parameter documents (tables and series) still pending hand verification.
    pub pending_parameter_document_count: u32,
    /// `TESTING.md` §5.2, row by row.
    pub items: Vec<UnverifiedItem>,
    /// Open questions the parameter documents record.
    pub parameter_open_items: Vec<OpenItem>,
}

/// One row of `TESTING.md` §5.3: a target the corpus refuses to encode.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RefusedTarget {
    /// The target.
    pub target: String,
    /// Why, and what replaces it.
    pub reason: String,
}

/// What a result would be pinned to (ADR-010), as far as anything exists.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Pins {
    /// The workspace version the report was generated for.
    pub application_version: String,
    /// The licence declared in the workspace manifest.
    pub licence: String,
    /// `engineVersion`; absent until the engine declares one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub engine_version: Option<String>,
    /// The plan `schemaVersion`; absent until the plan schema exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schema_version: Option<String>,
    /// Content ids of every vintage, locked or not.
    pub param_vintage_content_ids: Vec<String>,
    /// Ids of the vintages the lock covers.
    pub locked_param_vintage_ids: Vec<String>,
    /// The binary digest; absent until the release pipeline exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub binary_digest: Option<String>,
    /// Why the absent pins are absent.
    pub note: String,
}

/// The validation report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ValidationReport {
    /// A date given explicitly to the generator; never read from a clock.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generated_on: Option<String>,
    /// How the report was computed.
    pub basis: String,
    /// See [`FixtureReport`].
    pub fixtures: FixtureReport,
    /// See [`ParameterReport`].
    pub parameters: ParameterReport,
    /// See [`PropertyReport`].
    pub properties: PropertyReport,
    /// See [`SnapshotReport`].
    pub contract_snapshots: SnapshotReport,
    /// See [`SecurityReport`].
    pub security_suite: SecurityReport,
    /// Tier-2 pinned suites (`TESTING.md` §3.5).
    pub tier2_suites: SectionStatus,
    /// Tier-3 goldens (`TESTING.md` §3.6).
    pub tier3_goldens: SectionStatus,
    /// Recorded out-of-process oracles (`TESTING.md` §7).
    pub oracles: SectionStatus,
    /// The surviving-mutant budget (`TESTING.md` §11.3).
    pub mutation_score: SectionStatus,
    /// Fuzz corpora (`TESTING.md` §11.3).
    pub fuzz_corpora: SectionStatus,
    /// Browser end-to-end journeys (`TESTING.md` §9).
    pub browser_end_to_end: SectionStatus,
    /// See [`PerformanceReport`].
    pub performance_budgets: PerformanceReport,
    /// See [`TestInventory`].
    pub test_inventory: TestInventory,
    /// See [`UnverifiedReport`].
    pub unverified: UnverifiedReport,
    /// See [`RefusedTarget`].
    pub refused: Vec<RefusedTarget>,
    /// See [`Pins`].
    pub pins: Pins,
}
