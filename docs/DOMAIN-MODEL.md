# DOMAIN-MODEL

*The plan-file schema and domain model. Date: 2026-09-17. Binding companions: `PLAN.md` (what and when), `ARCHITECTURE.md` (how), `DECISIONS.md` (why). Every example in this document is synthetic or a published third-party worked example. Nothing here describes any real household, and no figure in this document comes from one.*

**Terminology used throughout (standing definition).** "The binary" or "the release" always means one thing: a **local web application for macOS**, shipped as a single self-contained executable (universal: Apple Silicon + Intel). Launching it starts a web server bound to loopback, serves the embedded web app over TLS, and opens the default browser. The user interface is a web app in the browser. It is not a native desktop GUI and not an Electron/Tauri window, and nothing else has to be installed.

---

## 0. Scope and how to read this

This document specifies the shapes that `pfp-model` owns: the plan document, its collections, scenarios, migrations, parameter and assumption vintages, result pins, the import/connector seam, and the fields the first-run flow collects. It does **not** specify the tax worksheets (`pfp-tax`), the next-dollar tier table's economics (`pfp-decide`), or generator configuration (`pfp-sim`); it specifies only the stored shapes those engines read and write.

**Notation.** Rust with `serde` + `schemars` derives is normative, because `pfp-model` is that code and the published JSON Schema is generated from it (ADR-017). Derives are elided for brevity: every type below derives `Serialize, Deserialize, JsonSchema, Clone, Debug, PartialEq`, and every enum is `#[serde(rename_all = "camelCase")]`. JSON appears once, in §19, as the synthetic example file.

**Section-to-milestone map.**

| § | Content | Milestone |
|---|---|---|
| 1-2 | Design rules, primitives | M0 (money/params), M2 (`MonthRef`, seam S5) |
| 3-9 | Plan document v1 and its live collections | M2 (S5) |
| 10 | `properties{}`, `insurance{}` reserved shapes | declared M2, populated **1.1** (the life modules moved out of M10 by decision, `PLAN.md` M10 and §5) |
| 11 | `policies{}` as data (S7) | M2 (contribution), M3 (withdrawal), M9 (rebalancing) |
| 12-13 | Scenarios, allowlist, referential integrity | collection + patch endpoint M2; UI, chaining, `hypotheticalFacts` M4 |
| 14 | Schema versioning, migration chain, `explorer` fold-in | M1 (`explorer`), M2 (chain + harness) |
| 15 | Parameter tables, vintages, overrides, `AssumptionSet` | M0 (S1), overrides M2, vintage update flow M9 |
| 16 | Result pins, fact/result snapshots, `Recommendation`, `ActionItem`, `Review`, file sections | pins, `Recommendation` and `ActionItem` M2 (S7/S8), snapshots M4, `Review` and review attribution M9 |
| 17 | `SnapshotDraft`, `ImportFormat`, `Connector`, `SecretRef` | M4 (file importers), trait freeze M10 |
| 18 | First-run setup flow fields | §18.1 Quick Start and the progressive fields the current-year waterfall reads (employer-plan flags, debt terms, the assumptions review) M2; the remaining §18.2 sections M4 (`PLAN.md` M2 Scope OUT, M4 Scope IN) |
| 19 | Synthetic example plan file | M2 |

---

## 1. Design rules

These are consequences of the spine, restated as rules an implementer can check against.

**R1. Owner-tagged rows, never parallel per-person arrays.** Every account, stream, debt, policy and property carries `owner: Owner` (`p1 | p2 | joint`). Parallel per-person lists — the encoding Owl uses, with index `i` tying every quantity to a person ([PARAMETERS.md](https://github.com/mdlacasse/Owl/blob/main/info/PARAMETERS.md)) — block add/remove-a-person and per-account detail. A one-person household is the same schema with `p2` absent. A joint account is **never** two half-accounts: RMD and beneficiary logic differ.

**R2. Every collection is `Record<id, item>`** (`BTreeMap<Id, T>`), never an array, so RFC-6902 paths use stable ids and survive reordering. The key must equal `item.id`; a validator enforces it. Scenario patch paths, snapshot references and the change log all address rows by path, which makes this the most load-bearing rule in the file.

**R3. Facts and assumptions are separate layers.** Facts are what is true now (balances, cost basis, account type, debt rate, date of birth); assumptions are beliefs about the future (returns, inflation, longevity, claim ages, growth rules). Boldin states the boundary plainly: shared fields "represent facts about your life right now—not assumptions about your future" ([help 14452778](https://help.boldin.com/en/articles/14452778-scenario-modeling-best-practices)). Ordinary scenarios patch assumptions, goals and policies, never facts (§12).

**R4. Money is integer cents; rates are exact rationals.** `Cents(i64)` serializes as a JSON integer; every rate, fraction and factor is `Ratio{num, den}`. No `f64` appears anywhere in the schema (ADR-007, D5).

**R5. Dates are person-anchored.** A `MonthRef` names a calendar month, a person's age, or a named event on a person, so removing a person is a graph operation, not a field delete (§13).

**R6. Provenance on every fact.** Every row carries `asOf`; volatile scalars carry a `Sourced<T>` envelope. Metadata is excluded from the canonical JSON `inputsHash` covers (§2.6), so refreshing a date does not invalidate a pinned result.

**R7. Store the primitive, not the derived value.** The Social Security earnings record or an entered PIA, never a claimed benefit; cost basis and lots, never an unrealized gain; the match formula, never a match dollar amount. Scenarios must recompute.

**R8. Reserved collections are declared, not empty holes.** The five collections seam S5 reserves (`properties`, `insurance`, `factSnapshots`, `resultSnapshots`, `reviews`) have both their key **and their item type** declared in schema v1 (§10, §16), even though no item exists in them until a later milestone — `factSnapshots`/`resultSnapshots` from M4, `reviews` from M9, `properties`/`insurance` from 1.1. The other three collections that once shared that label — `scenarios`, `recommendations`, `actions` — are **live from M2** (`PLAN.md` §3 S5, M2: every recommendation is a stored object, *Adopt* creates an `ActionItem` and appends to the Proposed Plan scenario; §19's example carries two populated scenarios), and their item types are declared in §12 and §16 like any other live collection. That is what keeps migrations additive and scenario paths stable across the roadmap.

**R9. Nothing in the repository is a user figure, and no `.pfplan` byte sequence is ever in the repository.** The schema, the JSON Schema artefact, the golden migration fixtures and the demo plan are public; every value in them is synthetic — generated by `cargo xtask persona` from a fixed seed once the generator lands at M2, and hand-written synthetic until then, with the M2 CI check of §19 asserting that each committed example equals the generator's output for its declared seed (ADR-023). The demo plan ships as **plaintext synthetic JSON at `fixtures/plans/demo.plan.json`**, carrying `"synthetic": true` plus its generator name and seed — never as a committed `.pfplan`. `rust-embed` embeds that JSON; the binary encrypts it into a `.pfplan` in the user's own directory (or a temp plan for a Playwright run) at first use. This matters because `SECURITY.md` §13.1 git-ignores `*.pfplan`, §13.2's magic-byte commit hook rejects any file beginning with the container magic and §13.4 fails on container magic anywhere — and **those two controls take no allowlist, ever**: an allowlisted path would blind the one check that catches a real household file committed by accident. The same rule binds test data for `tools/pfplan-ref/`, which is generated at test time and never committed. Populated plans exist only as encrypted `.pfplan` files outside the repository.

**R10. The document is versioned independently of the container.** Container `formatVersion` (ADR-005) describes bytes and crypto; plan `schemaVersion` describes shape.

---

## 2. Primitives

### 2.1 Identifiers, money, rates

**This document is the single definition site** for the shared primitives: `Year`, `Seed`, `AssetClass` and `BasisPoints` below, `Owner` and `PersonId` in §2.2, `FilingStatus` in §4. `ENGINE-SPEC.md` §1.1 and `SIMULATION-SPEC.md` §2.1/§2.3 **reference** these types; where they print a shape for the reader's convenience it is a verbatim copy labelled as such, with the spellings below (`FilingStatus { Single, Mfj, Mfs, Hoh, Qss }`, `Year = i32`, `Month = MonthYm`), and the definition site stays here (`pfp-domain` / `pfp-model`; `ARCHITECTURE.md` §2's one-home CI check is what enforces it). A primitive declared twice is a primitive that drifts, and several of these had already drifted in opposite directions.

```rust
/// Opaque, stable, unique within its collection. Lowercase [a-z0-9-], 1..=48 chars.
/// Generated as a short ULID-derived slug; never reused, never renumbered.
pub struct Id(String);

/// A parameter-table identifier: resolves against `params/`, never against a plan collection.
/// Distinct type from `Id` so "every Id reference resolves inside the document" stays a true statement.
pub struct ParamTableId(String);              // JSON: string, e.g. "nchs-2023-female"

pub struct Cents(pub i64);                   // JSON: integer, e.g. 12_500_000 == $125,000.00
pub struct Ratio { pub num: i64, pub den: i64 } // JSON: {"num":765,"den":10000} or "0.0765"
pub struct BasisPoints(pub u16);             // JSON: integer; 10_000 bp == 100%
pub type Year = i32;                          // calendar year; the validator enforces 1900..=2200
pub struct DateYmd { y: i32, m: u8, d: u8 }   // JSON: "2026-04-15"
pub struct MonthYm  { y: i32, m: u8 }         // JSON: "2026-04"
pub type Month = MonthYm;                     // the engines' short alias; the same type, not a second one

/// 32 bytes of RNG seed material, created by `pfp-server` (engine crates have no entropy).
pub struct Seed(pub [u8; 32]);               // JSON: 64 lowercase hex characters
```

`Year` is **`i32`, not `u16`**: the ledger subtracts years (`year - base_year`, `year - death_year`) and an unsigned difference underflows silently. The 1900..=2200 domain is a validator check, not a type bound.

`Ratio` deserializes from either the object form or a string (`"0.0765"`, `"5/900"`), and always serializes as the object form so canonical JSON is unambiguous. `Cents * Cents` does not compile. `Seed` deserializes only from exactly 64 hex characters — an integer seed is a schema error, because `ChaCha8Rng::from_seed(seed.0)` consumes all 32 bytes and an integer would silently zero-pad 28 of them.

```rust
#[repr(u8)]
pub enum AssetClass { UsLarge=0, UsSmall=1, IntlDev=2, EmergingMkts=3,
                      UsAggBond=4, TreasuryInt=5, Tips=6, Cash=7, /* 8..=23 reserved */ }
pub const MAX_CLASSES: usize = 24;
```

The registry is **closed and ordered**: the discriminant is part of the RNG addressing contract (`SIMULATION-SPEC.md` §7), so a class is appended, never reordered and never renumbered. Every allocation in the document is `BTreeMap<AssetClass, BasisPoints>` summing to exactly **10,000**, checked by the validator.

### 2.2 Owner and person references

```rust
pub enum Owner { P1, P2, Joint }
pub enum PersonId { P1, P2 }           // exactly two slots; p2 present only in a two-person household
```

`Owner` and `PersonId` are defined here and referenced elsewhere (§2.1). `Owner::Joint` is a first-class value, not a pair. In a **one-person household `Joint` is not a valid owner**: the setup flow never offers it, an import that proposes it normalizes to `P1` at the merge step, and the validator rejects `Owner::Joint`, `Owner::P2` and `PersonId::P2` anywhere in the document when `household.persons.p2` is absent. The M2 persona journey for an early-career single filer asserts that its generated file passes this rule, and `removePerson` (§13) surfaces every `Joint` row as something the user must reassign rather than rewriting it silently.

### 2.3 `MonthRef` — the person-anchored timeline

```rust
pub enum MonthRef {
    Calendar { month: MonthYm },
    Age      { person: PersonId, years: u8, months: u8 },
    Event    { person: PersonId, event: LifeEvent },
    Never,
}

pub enum LifeEvent {
    Retirement, LastWorkingMonth, SocialSecurityClaim, Medicare65,
    RmdStart, AssumedDeath, PlanEnd,
}
```

Resolution is a pure function `resolve_month(&MonthRef, &ResolvedPlan) -> MonthYm` evaluated once per projection; `Never` resolves to the horizon end. `Medicare65` and `RmdStart` resolve from statute, not user input — `RmdStart` uses the applicable age by birth year (pre-1949: 70½; 1949-50: 72; 1951-59: **73**; 1960+: **75**, [26 USC 401(a)(9)(C)(v)](https://www.law.cornell.edu/uscode/text/26/401); 1959 births satisfy both clauses and are assigned 73). Storing a resolved calendar month where an `Age` ref was meant is the error this type exists to prevent.

### 2.4 Real vs nominal

```rust
pub enum Basis { Real, Nominal }   // required on every amount that grows
```

Every stored amount with a growth rule carries `basis`. The ledger runs nominal internally and displays real (ADR-008); a stream tagged `Real` is uprated by the path's inflation before entering the ledger. Omitting this tag is how today's dollars silently mix with nominal dollars.

### 2.5 Growth rules

```rust
pub enum GrowthRule {
    Flat,                                  // no growth (never-indexed by intent); a pension with no COLA
    Inflation,                             // the assumption set's general inflation; a full-CPI COLA
    InflationCapped { cap: Ratio },        // CPI up to `cap`; a capped pension COLA
    FersDiet,                              // CPI <= 2% -> full; 2-3% -> 2%; > 3% -> CPI - 1 (SIMULATION-SPEC §15)
    Index { assumption_key: String },      // e.g. "medicalInflation", "wageGrowth", "housing"
    FixedRate { annual: Ratio },
    Schedule { by_year: BTreeMap<Year, Cents>, after_last: AfterLast },
}
pub enum AfterLast { Zero, Repeat, IndexToInflation }
```

A pension's or annuity's COLA rule is its stream's `growth`, nothing separate: the conventional COLA taxonomy `none | fixed_pct | cpi | capped_pct | fers_diet` maps onto `Flat | FixedRate | Inflation | InflationCapped | FersDiet` one for one. `Schedule` is the year-table form used for planned per-person flows. A schedule with no `after_last` is a validation error, for the same reason a parameter table with no projection rule is a CI error (ADR-010): silence there manufactures a cliff or a frozen value.

### 2.6 Provenance envelope and the `inputsHash` boundary

```rust
pub struct Sourced<T> {
    pub value: T,
    pub as_of: DateYmd,
    pub confidence: Option<Confidence>,     // Estimated | Stated | Verified
    pub origin: Origin,                     // how the value got here
}
pub enum Origin { Manual, Import { provenance_id: Id }, Computed { engine_version: String }, Default }
```

Each **row** carries a required `asOf: DateYmd` (when the row was last reviewed). `Sourced<T>` wraps only scalars that actually go stale: account `balance` and `costBasis`, holding `unitPrice`, property `value`, debt `balance` and `apr`, income `amount`, an entered Social Security estimate (§4). Everything else is a plain value on a row that has its own `asOf`.

**Canonicalization rule (D10).** `inputsHash` = SHA-256 over RFC 8785 canonical JSON of the resolved plan, overrides and assumption set, **with `as_of`, `confidence`, `origin`, `label`, `notes` and `uiOrder` elided**. Re-confirming a balance that did not move therefore leaves `inputsHash` unchanged and does not stale a pinned result, while any change to a number, an id, a policy rule or an enum does. Eliding is a `#[canonical(skip)]` attribute read by one canonicalizer, with a property test asserting that touching only skipped fields is hash-invariant.

**Staleness meter.** The setup flow's completeness/staleness meter (M2) compares row `asOf` against `asOfPolicy { balance_days: 90, income_days: 365, debt_days: 180, insurance_days: 365 }` — editable defaults matching a quarterly-light / annual-comprehensive review cadence.

---

## 3. The plan document (schema v1)

```rust
pub struct Plan {
    pub schema_version: u16,               // 1
    pub plan_id: Id,
    pub created: DateYmd,
    pub as_of_policy: AsOfPolicy,

    pub household:       Household,
    pub persons:         BTreeMap<PersonId, Person>,
    pub employer_plans:  BTreeMap<Id, EmployerPlan>,
    pub accounts:        BTreeMap<Id, Account>,
    pub income_streams:  BTreeMap<Id, IncomeStream>,
    pub expense_streams: BTreeMap<Id, ExpenseStream>,
    pub debts:           BTreeMap<Id, Debt>,
    pub goals:           BTreeMap<Id, Goal>,
    pub events:          BTreeMap<Id, PlannedEvent>,
    pub target_allocations: BTreeMap<Id, TargetAllocation>,
    pub properties:      BTreeMap<Id, Property>,        // reserved (populated 1.1)
    pub insurance:       BTreeMap<Id, InsurancePolicy>, // reserved (populated 1.1)

    pub ytd:             Option<YearToDate>,            // year-0 actuals (§3.1); None = annualize

    pub policies:        Policies,
    pub assumption_sets: BTreeMap<Id, AssumptionSetRef>,
    pub param_overrides: BTreeMap<Id, ParamOverride>,

    pub scenarios:        BTreeMap<Id, Scenario>,
    pub fact_snapshots:   BTreeMap<Id, FactSnapshotIndex>,
    pub result_snapshots: BTreeMap<Id, ResultSnapshotIndex>,
    pub recommendations:  BTreeMap<Id, Recommendation>,
    pub actions:          BTreeMap<Id, ActionItem>,
    pub reviews:          BTreeMap<Id, Review>,
    pub import_provenance: BTreeMap<Id, ImportProvenance>,
}
```

**File-section mapping (resolving an ambiguity between ADR-005 and seam S5).** The `Plan` struct above is the entire content of the container's `plan` section. The `snapshots` and `results` sections hold the **bodies** of fact and result snapshots; the document's `fact_snapshots{}` / `result_snapshots{}` collections hold only **index entries** (id, timestamps, pins, a small summary, a `body_ref`). This satisfies `PLAN.md` risk R17 — the whole-file rewrite stays proportional to the plan, not to accumulated run history — while keeping scenario paths and the change log on stable ids. `changelog` holds the append-only `{timestamp, action, reverseDiff}` entries; `secrets` holds only `SecretRef` payloads (§17), decrypted on demand.

### 3.1 Year-to-date actuals (`ytd`)

Year 0 is always a **full tax year** (`ENGINE-SPEC.md` §1.4): tax, payroll, caps and phase-outs are calendar-year constructs, so the engine assembles year 0 from what has already happened plus the remaining-month flows, never from the remaining months alone. This document owns the stored shape the engine reads:

```rust
pub struct YearToDate {
    pub as_of: DateYmd,                                // the date the actuals are stated through
    pub wages: BTreeMap<PersonId, Cents>,              // gross wages paid so far this year, per person
    pub withholding: BTreeMap<PersonId, Cents>,        // federal income-tax withheld so far, per person
    pub contributions: BTreeMap<Id, Cents>,            // employee money already into each account (by account id)
    pub employer_contrib: BTreeMap<Id, Cents>,         // employer money already into each account
    pub realized_gains: Cents,                         // net realized gains so far in taxable accounts
}
```

`ytd` is a **fact** (`/ytd/**` is `HypotheticalFacts`-only in §12.1) and is optional: when it is `None` the engine annualizes the entered current rates to a full year and prints that as a caveat on every card that depends on it. It is collected in the Wages section of the progressive flow (§18.2, item 5), not in Quick Start. Every cap the next-dollar engine enforces in year 0 is net of these figures (`ENGINE-SPEC.md` §5.5, §5.7, §5.8), and the Tier-1 case that the same household opened in January and in October yields the same `t_now` and eligibility (`TESTING.md` §4, I28) is what this shape exists to make possible.

---

## 4. Household and persons

```rust
pub struct Household {
    pub filing_status: FilingStatus,       // the current-year status; later years are computed
    pub resident_state: StateCode,         // applied to the whole household
    pub dependents: BTreeMap<Id, Dependent>,
    pub survivor: SurvivorPolicy,
    pub planning_age: PlanningAge,
    /// The two prior tax years' MAGI pair, with the status each return was filed under:
    /// [t-1, t-2]. Plan years 0 and 1 have no prior ledger year to read, and IRMAA looks
    /// back two years (`ENGINE-SPEC.md` §3.4), so these are ordinary plan facts; None = not entered.
    pub prior_magi: [Option<MagiPair>; 2],
}

/// The pair the IRMAA and PTC lookups read (`ENGINE-SPEC.md` §2.1 `magi_history`).
pub struct MagiPair { pub aca: Cents, pub irmaa: Cents, pub status: FilingStatus }

/// Defined here, referenced by the tax engine (§2.1). Wire forms are the short ones the
/// tax kernel and its fixtures use: "single" | "mfj" | "mfs" | "hoh" | "qss".
/// The long names (Married Filing Jointly, Qualifying Surviving Spouse) are UI labels only.
pub enum FilingStatus { Single, Mfj, Mfs, Hoh, Qss }

pub struct Dependent { pub id: Id, pub label: String, pub dob: MonthYm,
                       pub relationship: DependentKind, pub in_household_until: MonthRef,
                       pub as_of: DateYmd }
pub enum DependentKind { Child, Other }

pub struct PlanningAge { pub mode: PlanningAgeMode, pub preset_age: Option<u8> }
pub enum PlanningAgeMode { Computed, Fixed }   // presets 95 and 100 offered
```

`filing_status` is a **fact about the current year only**. Later years are computed by the first-death state machine: `Mfj` through the death year, `Qss` (qualifying surviving spouse) in the two following years only with a dependent child and an unmarried survivor, `Single` thereafter ([Pub 501](https://www.irs.gov/publications/p501)). Flipping to `Single` *in* the death year is wrong and a named test asserts against it (M3).

```rust
pub struct SurvivorPolicy {
    /// Per-expense-category multipliers applied from the year after first death.
    pub spending_multipliers: BTreeMap<ExpenseCategory, Ratio>,
    pub sensitivity_bounds: (Ratio, Ratio),      // shown, not applied
    pub basis_step_up: StepUpPolicy,
}
pub enum StepUpPolicy { None, HalfStepUp, FullStepUp }
```

Defaults per `PLAN.md` M3: **0.70** for core categories, **1.0** for `Housing`, `Dependents` and `Education`, with the pair **(0.60, 0.74)** displayed as sensitivity bounds and never silently applied. The shipped 0.60 sensitivity bound is this project's own choice, informed by a cited comparison — Owl's flat household fraction (`chi = 0.60`, [plan.py](https://github.com/mdlacasse/Owl/blob/main/src/owlplanner/plan.py)) — and not adopted from Owl's parameter tables (ADR-004: third-party defaults are cited as comparisons with attribution, never sourced into `params/`); the per-category shape is this project's own and is what the schema encodes. `basis_step_up` defaults to `HalfStepUp`, with a UI note that community-property states may warrant `FullStepUp` ([Pub 551](https://www.irs.gov/publications/p551)).

```rust
pub struct Person {
    pub id: PersonId,
    pub label: String,                      // user-chosen display label, never required to be a legal name
    pub dob: MonthYm,                       // month precision is sufficient for every rule
    pub sex_for_mortality: Option<MortalityBasis>,
    pub employment: EmploymentStatus,
    pub retirement: MonthRef,
    pub expected_separation: Option<MonthRef>, // when employment with the current plan sponsor ends;
                                              // drives the vesting discount on the match tier (§5)
    pub assumed_death: Option<MonthRef>,     // deterministic first-death input (M3); sampled per path (M7)
    pub mortality_table_id: Option<ParamTableId>, // per-person OVERRIDE of the assumption set's default table (§15);
                                                  // None = use the default, so a table is always resolved
    pub social_security: SocialSecurityInput,
    pub coverage: CoverageInput,             // reserved shape, populated 1.1
    pub as_of: DateYmd,
}
pub enum EmploymentStatus { Employed, SelfEmployed, NotWorking, Retired }
```

`label` exists because the UI needs something to print; it is never used as a key, never appears in logs, and the synthetic personas use `"Person 1"` / `"Person 2"`. `expected_separation` is a plan fact and may be absent; when it is, the vesting discount is 1.0 with a printed caveat, never a guess. `mortality_table_id` is a `ParamTableId` because the table lives in `params/`, not in the document — typing it as `Id` made §13's "every `Id` reference resolves" false for the one field that cannot resolve inside the plan. **Precedence:** a person's own `mortality_table_id`, else the assumption set's `mortality_table_id` (§15), which is not optional — so the resolved table exists for every person and no engine carries a "no table selected" branch (`ENGINE-SPEC.md` §11.1, `SIMULATION-SPEC.md` §6). §19's example shows both cases: one person names a sex-specific table, the other inherits the set's default.

```rust
pub struct SocialSecurityInput {
    pub source: SsSource,
    pub claim_age: MonthRef,                       // an Age ref, e.g. 67y0m
    pub survivor_claim_age: Option<MonthRef>,      // independent claim; see below
    pub earnings_record: Option<EarningsRecord>,   // M5
    pub entered_estimate: Option<SsEstimateEntry>, // M3 stopgap; the PIA, or what the statement shows
    pub future_earnings: Option<GrowthRule>,       // "if work stops at age X"
}
pub enum SsSource { EnteredEstimate, EarningsRecord, NotClaiming }

/// What the user actually has in front of them. Both forms resolve to a PIA before the ledger
/// sees them; the derivation is shown in the Explain panel.
pub enum SsEstimateEntry {
    Pia { amount: Sourced<Cents> },                        // the primary insurance amount itself
    BenefitAtAge { amount: Sourced<Cents>, at: MonthRef },  // a statement figure, at the age it assumes
}

pub struct EarningsRecord {
    pub rows: BTreeMap<Year, Cents>,               // Medicare/covered earnings as reported
    pub as_of: DateYmd,
}
```

Per R7 the record or the PIA is stored, never a *claimed* benefit — claim-age scenarios must recompute. `BenefitAtAge` does not violate that rule and exists because of it: an SSA statement prints claim-age-adjusted benefits, so a field labelled "PIA" that a user fills from a statement is silently reduced twice, once by the user's source and again when the ledger applies `payable = floor_dollar(PIA × claim_factor)`. A $2,850 age-62 figure entered as a PIA with claim age 62 yields $1,995 — roughly a 30% error in the largest guaranteed income stream, propagating into the ledger, the funded ratio, the fail-safe rate and the Roth verdict. `BenefitAtAge` therefore carries **the age its amount assumes**, and `pfp-ss` back-solves the PIA from it, storing the derivation, not the derived figure. The stored shape stays a PIA or a record; the *entry* accepts what the user has, and `PLAN.md` M3/M5 and `SIMULATION-SPEC.md` §14.1 name this type rather than a loose "benefit estimate". Own retirement and survivor benefits are independent claims and need independent claim ages, which is why `survivor_claim_age` is its own field rather than a modifier; Open Social Security models the same separation on its `Person` (MIT, [license.txt](https://github.com/MikePiper/open-social-security/blob/master/license.txt)). `EarningsRecord` lives only in the encrypted plan file and is never written to a fixture (ADR-023).

---

## 5. Employer plans

A separate collection rather than fields on an account, because plan **features** drive the next-dollar engine (mega-backdoor availability, catch-up type, match shape) independently of the balances sitting in the account.

```rust
pub struct EmployerPlan {
    pub id: Id,
    pub owner: PersonId,                       // never Joint
    pub kind: EmployerPlanKind,                // Plan401k | Plan403b | Plan457b | Sep | Simple | Tsp
    pub match_formula: Vec<MatchTier>,         // ordered; evaluated on eligible compensation
    pub match_cap: Option<MatchCap>,
    pub true_up: bool,                         // employer trues up an under-matched year
    pub per_pay_period_match: bool,            // true = matched per period, so front-loading forfeits
    pub vesting: Vesting,
    pub roth_deferral_allowed: bool,
    pub after_tax_contributions_allowed: bool, // precondition for mega-backdoor
    pub in_plan_roth_conversion: bool,         // the other precondition
    pub in_service_withdrawal_age: Option<u8>, // Some(age) = withdrawals allowed from that age;
                                               // None = no in-service withdrawal path at all
    pub accepts_roll_in: bool,                 // pre-tax IRA can be rolled in (clears backdoor pro-rata)
    pub loan_allowed: bool,
    pub hsa_eligible_coverage: bool,           // HDHP coverage through this employer
    pub hsa_coverage_tier: Option<HsaTier>,    // SelfOnly | Family
    pub brokerage_window: bool,
    pub menu: Vec<MenuFund>,                   // constrains asset location (M9)
    pub as_of: DateYmd,
}
pub struct MatchTier { pub employee_pct: Ratio, pub employer_pct: Ratio }  // "100% of first 3%, 50% of next 2%"
pub enum MatchCap { PctOfComp(Ratio), Dollars(Cents) }   // plans express the cap both ways
pub enum Vesting { Immediate, Cliff { years: u8 }, Graded { pct_by_year: Vec<Ratio> } }
```

Every field is a boolean, a rate or a dollar figure answerable from a summary plan description, and each gates engine behaviour: `after_tax_contributions_allowed && in_plan_roth_conversion` gates the mega-backdoor tier entirely; `true_up == false` together with `per_pay_period_match == true` makes front-loading deferrals a match-forfeiture risk the recommendation must name, and is what lets the engine compute the per-period deferral percentage that keeps every period matched; `accepts_roll_in` is the precondition for the backdoor-Roth alternative that clears a pre-tax IRA balance out of the Form 8606 pro-rata denominator; `hsa_eligible_coverage` gates the HSA tier. `match_cap` is an **enum, not a rate**: a plan capping the match at "4% of compensation" and one capping it at "$6,000" are different facts, and a single `Ratio` field named for the percent form silently mistypes the dollar form. The engines read these fields by the spellings above; a CI check (`lint:schema-identifiers`, `TESTING.md` §9) resolves every stored-field identifier in the blocks that `ENGINE-SPEC.md` and `SIMULATION-SPEC.md` mark as reading the plan — `ENGINE-SPEC.md` §5 and §11.1, `SIMULATION-SPEC.md` §14.1, §15 and §16.3 — against the published `plan.schema.json`, so a rename in either document fails the build rather than the run. Its scope covers every block that reads the plan, not `ENGINE-SPEC.md` §5 alone, because a rename in any one of them (`survivor_pct` for `survivor_fraction`, say) would otherwise pass unnoticed. Statutory room lives in `params/`, never in the plan file (2026: elective deferral **$24,500**, catch-up **$8,000**, ages 60-63 **$11,250**, 415(c) **$72,000**, compensation limit **$360,000** — [Notice 2025-67](https://www.irs.gov/pub/irs-drop/n-25-67.pdf); HSA **$4,400 / $8,750** plus a $1,000 age-55 add-on — [Rev. Proc. 2025-19](https://www.irs.gov/pub/irs-drop/rp-25-19.pdf)). Mega-backdoor headroom (415(c) less deferrals less employer contributions, up to $47,500 in 2026) is computed and labelled, never stored; that derivation is **unverified** (`TESTING.md` §5.2).

---

## 6. Accounts, holdings, lots, beneficiaries

```rust
pub struct Account {
    pub id: Id,
    pub label: String,
    pub owner: Owner,
    pub tax_type: TaxType,
    pub institution: Option<String>,
    pub employer_plan_id: Option<Id>,          // required iff tax_type is an employer-plan type
    pub balance: Sourced<Cents>,
    pub cost_basis: Option<Sourced<Cents>>,    // taxable, and after-tax basis in traditional
    pub holdings: BTreeMap<Id, Holding>,       // empty, or a complete decomposition of balance
    pub target_allocation_id: Option<Id>,
    pub beneficiary: Beneficiary,
    pub roth_ledger: Option<RothLedger>,       // contributions, conversions with their five-year clocks
    pub inherited: Option<InheritedDetails>,
    pub fees: Option<AccountFees>,
    pub tombstoned: Option<DateYmd>,           // see §13
    pub as_of: DateYmd,
}

pub enum TaxType {
    Cash, Taxable, Traditional, Roth, Hsa, Plan529,
    TraditionalEmployer, RothEmployer, AfterTaxEmployer,
    InheritedTraditional, InheritedRoth,
}
```

The tax type is the axis every downstream rule keys off — withdrawal order, RMDs, conversions, asset location, liquidity class. `AfterTaxEmployer` is separate from `TraditionalEmployer` because 415(c) room and in-plan conversion apply only to it.

`target_allocation_id` resolves into the document's own `target_allocations{}` collection (§3):

```rust
pub struct TargetAllocation {
    pub id: Id, pub label: String,
    pub weights: BTreeMap<AssetClass, BasisPoints>,   // must sum to exactly 10,000
    pub glide_path: Option<GlidePath>,
    pub as_of: DateYmd,
}
pub enum GlidePath {
    Static,
    LinearToAge { end_weights: BTreeMap<AssetClass, BasisPoints>, from: MonthRef, to: MonthRef },
    RuleRef { rule_id: String, params: BTreeMap<String, Ratio> },   // a registry rule (ADR-021)
}
```

The collection is **shared, not per-account**: several accounts point at one policy, which is what makes household-level reallocation (M9) expressible at all, and the engines read `w_k` from it (`ENGINE-SPEC.md` §2.2 step 7). The validator checks that `weights` sums to exactly 10,000 basis points, that every `end_weights` does too, and that `rule_id` is a known registry id — nothing is loaded dynamically. Integer basis points rather than `Ratio` so rebalancing dollars are exact `mul_ratio` results (`SIMULATION-SPEC.md` §2.1). The collection must exist for `target_allocation_id` and `/accounts/*/targetAllocationId` (§12.1) to resolve, and for §13 to check the reference; a declared-but-missing collection would leave a dangling id nothing catches. The collection lands **before seam S5 freezes at M2**, so it is part of schema v1 rather than a v2 migration.

```rust
pub struct Holding {
    pub id: Id, pub symbol: Option<String>, pub label: String,
    pub asset_class: AssetClass, pub units: Ratio,
    pub unit_price: Sourced<Cents>, pub expense_ratio: Option<Ratio>,
    pub lots: Vec<Lot>,                        // taxable only; empty elsewhere
    pub cost_basis_method: BasisMethod,        // SpecificId | AverageCost | FifoDefault
}
pub struct Lot { pub id: Id, pub units: Ratio, pub basis_per_unit: Cents,
                 pub acquired: DateYmd, pub wash_sale_adjusted: bool,
                 pub special_basis: Option<SpecialBasis> }  // Rsu | Espp | Nua
```

`Lot` is a `Vec` rather than a `Record` — the one deliberate exception to R2 — because no scenario path addresses a lot and imports replace lot sets wholesale; lots carry `id` anyway so trade lists can reference them. `BasisMethod::AverageCost` is stored because that election removes lot choice, and the trade generator must say so rather than propose specific-lot sales the user cannot execute.

**Holdings either decompose the account or are absent — never partially.** `holdings` is empty (the account is modelled at `balance` with its `target_allocation_id`), or it is a complete decomposition, and the validator enforces all three sums exactly, in cents:

```
Σ_h round_half_even(h.units × h.unit_price)            == account.balance
Σ_l l.units                                            == h.units          (per holding, taxable)
Σ_h Σ_l round_half_even(l.units × l.basis_per_unit)    == account.cost_basis
```

A partial import that cannot satisfy these leaves `holdings` empty and records the shortfall as an `ImportWarning`; it never writes holdings that fail to reconcile to the balance the user can see on a statement. The one exception is `BasisMethod::AverageCost`, where the lot sums are skipped and the holding carries a single synthetic lot at the average basis, flagged as such.

```rust
pub struct Beneficiary { pub spouse_fraction: Ratio, pub remainder: Remainder }
pub enum Remainder { Estate, Child { dependent_id: Id }, Charity }

pub struct RothLedger {
    pub contributions_basis: Sourced<Cents>,
    pub conversions: Vec<ConversionRow>,       // {year, taxable_amount, nontaxable_amount}
}
pub struct InheritedDetails { pub death_year: Year, pub decedent_age_at_death: u8,
                              pub decedent_was_taking_rmds: bool,
                              pub beneficiary_class: BeneficiaryClass }
```

Beneficiary fractions live **per account**, not per tax bucket as in Owl's `beneficiary_fractions` (taxable, tax-deferred, tax-free, HSA; HSA defaults 1.0 — [PARAMETERS.md](https://github.com/mdlacasse/Owl/blob/main/info/PARAMETERS.md)): per-account pairs correctly with owner-tagged rows and costs nothing, since a bucket-level default is a UI affordance over the same field. When `spouse_fraction < 1` the untransferred part genuinely leaves the plan as a bequest, taxed at the editable heirs rate (default **24%** — `DECISIONS.md` open decision 8, `ENGINE-SPEC.md` §7 and §11.1 row 5; Owl ships 30%, `nu = 0.300`, cited as a comparison and not adopted). `InheritedDetails` mirrors muirjc's `InheritedIraDetails` (MIT, [models.py](https://github.com/muirjc/retirement-planner/blob/main/src/retirement_planner/scenario/models.py)); Pub 590-B's spousal options (treat as own, roll over, remain beneficiary with distributions deferrable to the year the decedent would have reached their RBD) are a withdrawal-policy field, not a fact ([Pub 590-B](https://www.irs.gov/publications/p590b)).

---

## 7. Income streams

```rust
pub struct IncomeStream {
    pub id: Id, pub label: String, pub owner: Owner,
    pub kind: IncomeKind,
    pub amount: Sourced<Cents>, pub period: Period,   // Annual | Monthly
    pub basis: Basis, pub growth: GrowthRule,
    pub start: MonthRef, pub end: MonthRef,           // end: Never = for life (the survivor scaling below
                                                      // covers the joint-life case); Calendar = fixed end;
                                                      // Event = until that event
    pub survivor_fraction: Ratio,                     // applies to this stream from the year after its owner's
                                                      // death (ENGINE-SPEC §11.1 row 4); 0 = life-only
    pub tax_character: TaxCharacter,
    pub payroll: Option<PayrollDetail>,               // wages only
    pub as_of: DateYmd,
}
pub enum IncomeKind { Wages, SelfEmployment, Pension, SocialSecurity, Annuity,   // Annuity: SPIA, DIA, QLAC
                      Rental, Interest, Dividends, CapitalGains, Other }

/// How the tax function reads the stream (`ENGINE-SPEC.md` §3.1 `TaxInputs`). Enumerated once, here.
pub enum TaxCharacter {
    OrdinaryEarned,                          // wages and SE income: payroll tax, the earnings test, earned-income rules
    Ordinary,                                // pensions, interest, non-qualified dividends, rental net: ordinary, not earned
    SsSection86,                             // Social Security: the Pub 915 worksheet decides the taxable share
    Capital,                                 // gains and qualified dividends: preferential stacking
    ExclusionRatio { excluded: Ratio },      // non-qualified annuity: this fraction of each payment is return of basis
    Exempt,                                  // Roth distributions, tax-exempt interest; still counted where MAGI says so
}

pub struct PayrollDetail {
    pub employer_plan_id: Option<Id>,
    pub deferral_pct: Ratio, pub deferral_type: DeferralType,
    pub after_tax_pct: Ratio,
    pub hsa_payroll_deduction: Cents,                            // payroll-deducted: FICA-free
    pub bonus: Option<BonusDetail>,
    pub equity_comp: Option<EquityComp>,                         // RSU vests with supplemental withholding
    pub group_life_face: Option<Cents>,                          // >$50k triggers §79 imputed income
    pub group_ltd: Option<GroupLtd>,
}
pub struct GroupLtd { pub replacement_pct: Ratio, pub monthly_cap: Cents,
                      pub elimination_days: u16, pub employer_paid: bool,
                      pub own_occ_months: Option<u16> }

/// The pre-tax / Roth split of elective deferrals is a STORED DECISION, never a tie-break
/// (`ENGINE-SPEC.md` §2.3): the next-dollar loop routes an amount into the employer plan and
/// divides it by this field; only the Roth-vs-Traditional card recommends changing it.
pub enum DeferralType { Traditional, Roth, Split { pct_roth: Ratio } }
```

**This is the one `IncomeStream` shape; `SIMULATION-SPEC.md` §15 reads it and adds nothing.** A conventional guaranteed-income schema maps onto it field by field: `amount_monthly` is `amount` with `period: Monthly`; `dollars: real|nominal` is the mandatory `basis` of §2.4; `end_rule: life|joint_life|fixed_end|until_event` is `end` (`Never`, `Never` with the survivor scaling, `Calendar`, `Event`); `cola_rule` is `growth` (§2.5); `survivor_pct` is `survivor_fraction`, a `Ratio` rather than a `{0, 50, 75, 100}` enumeration because those four are the common joint-and-survivor elections, offered as entry presets, not the only ones a plan document can carry; `tax_character` is enumerated above. Two fields of that schema are **derived, not stored** (R7): `counts_for_earnings_test` is true exactly for `OrdinaryEarned` streams — only wages and net self-employment income count (`SIMULATION-SPEC.md` §14.4) — and `counts_for_irmaa_magi` is what the tax function's MAGI definitions decide from `tax_character` (`ENGINE-SPEC.md` §3.4). The `popup` flag (a joint-and-survivor election that reverts to the single-life amount if the non-owner spouse dies first) is not stored in v1: `SIMULATION-SPEC.md` §15 takes amounts as entered, and the reversion needs the single-life amount that pension pricing introduces after 1.0.

`survivor_fraction` sits on the stream because that is what it modifies — a pension's joint-and-survivor election belongs to the pension, not the household. Social Security is the exception: the survivor benefit is a distinct stream computed from the decedent's record under the **three-branch rule of `SIMULATION-SPEC.md` §14.3** (the base depends on whether the decedent filed before FRA, at or after FRA, or died unfiled; the survivor's own age reduction against the survivor-FRA schedule is applied only afterwards), so a `SocialSecurity` stream carries `survivor_fraction = 1` and the computation runs in `pfp-ss` (M5). `GroupLtd` defaults reflect typical group design (60% of salary to a cap, 90- or 180-day elimination, own-occupation for 24 months, taxable when employer-paid); these are **secondary-sourced, to be confirmed against an actual plan document**, and that caveat rides on the field.

---

## 8. Expenses, goals, events

```rust
pub struct ExpenseStream {
    pub id: Id, pub label: String, pub owner: Owner,
    pub category: ExpenseCategory, pub essential: bool,
    pub amount: Sourced<Cents>, pub period: Period,
    pub basis: Basis, pub growth: GrowthRule,
    pub start: MonthRef, pub end: MonthRef,
    pub survivor_treatment: SurvivorTreatment,   // Multiplier | Unchanged | EndsAtFirstDeath
    pub as_of: DateYmd,
}
pub enum ExpenseCategory { Housing, Food, Transport, Healthcare, Insurance,
                           Dependents, Education, Discretionary, Charity, Taxes, Other }
```

`essential` is a required boolean, not an inference: the scorecard's essential-vs-total funded ratio and its tiered thresholds key off it, and the split cannot be recovered from a category alone. `Healthcare` defaults to `Index{ "medicalInflation" }`.

```rust
pub struct Goal {
    pub id: Id, pub label: String, pub owner: Owner,
    pub kind: GoalKind,                  // Retirement | Education | Purchase | Travel | Legacy | Custom
    pub priority: Priority,              // Need | Want | Wish
    pub target_amount: Option<Cents>, pub basis: Basis,
    pub target_date: MonthRef, pub recurrence: Option<Recurrence>,
    pub funded_by_account_ids: Vec<Id>,  // e.g. a 529 funds an Education goal
    pub as_of: DateYmd,
}

pub struct PlannedEvent {
    pub id: Id, pub label: String, pub owner: Owner,
    pub when: MonthRef, pub kind: EventKind,
}
pub enum EventKind {
    LumpSum { amount: Cents, basis: Basis, to_account_id: Option<Id> },
    PlannedContribution { to_account_id: Id, amount: Cents, basis: Basis,
                          recurrence: Option<Recurrence>, ends: MonthRef },
    Relocation { new_state: StateCode },
    PropertySale { property_id: Id }, PropertyPurchase { /* 1.1 */ },
    RothConversion { from_account_id: Id, to_account_id: Id, amount: ConversionAmount },
    EmploymentChange { new_status: EmploymentStatus },
    CoverageChange { regime: CoverageRegime },
}
pub enum ConversionAmount { Fixed(Cents), FillToThreshold { threshold: FillTarget } }
```

`FillToThreshold` stores the *intent* (bracket top, LTCG boundary, IRMAA tier, FPL multiple), not a dollar figure, so the conversion re-solves when facts or law move (M8). Goal `priority` is `Need | Want | Wish` because withdrawal and shortfall logic ranks by it.

`PlannedContribution` is the **adopt target for every recommendation that is not payroll-routed** — a direct IRA or HSA contribution, a taxable deposit. A backdoor Roth adopts as two events, exactly as `ENGINE-SPEC.md` §5.6 models it: a `PlannedContribution` of the nondeductible amount into the traditional IRA, then a `RothConversion`. A mega-backdoor adopts as an `afterTaxPct` change plus a `RothConversion` (§5.7's two legs). Without this variant those two options — the two highest-value tiers after the match — had no writable path, so *Adopt* had nothing to write and the recommendation could be produced but never recorded.

---

## 9. Debts

```rust
pub struct Debt {
    pub id: Id, pub label: String, pub owner: Owner,
    pub kind: DebtKind,     // Mortgage | HomeEquity | Student | Auto | CreditCard | Personal | Medical | Other
    pub balance: Sourced<Cents>,
    pub apr: Sourced<Ratio>, pub rate_type: RateType,
    pub minimum_payment: MinimumPaymentRule,
    pub origination: Option<DateYmd>, pub term_months: Option<u16>,
    pub interest_deductibility: Deductibility,
    pub promo: Option<PromoTerms>,
    pub prepayment_penalty: Option<PrepaymentPenalty>,
    pub secured_by_property_id: Option<Id>,
    pub student: Option<StudentLoanDetail>,
    pub extra_payment: Option<Cents>,
    pub as_of: DateYmd,
}
pub enum RateType { Fixed, Variable { index: String, margin: Ratio, cap: Option<Ratio>,
                                      reset_months: u16 } }
pub enum MinimumPaymentRule {
    Amortizing { payment: Cents },
    PercentOfBalance { pct: Ratio, floor: Cents },
    InterestOnly,
    Fixed { payment: Cents },
}
pub enum Deductibility { None, MortgageInterest, StudentLoanInterest, QualifyingAutoLoan, Investment }
pub struct PromoTerms { pub ends: MonthYm, pub promo_apr: Ratio, pub deferred_interest: bool }
pub struct StudentLoanDetail { pub federal: bool, pub pslf_track: bool,
                               pub idr_plan: Option<String>, pub forgiveness_month: Option<MonthRef> }
```

`Deductibility` is an enum, not a stored fraction: the engine **computes** `deductible_fraction` by running the tax function twice (M2), so a mortgage in a standard-deduction household yields 0 and one itemizing through the SALT cap yields 1. A user-entered fraction would defeat the mechanism. `pslf_track` forces minimum payments with `r_u = 0` and a printed constraint. `deferred_interest` becomes a hard zero-by-`ends` constraint rather than a rate comparison, because retroactive interest is not an APR.

---

## 10. Reserved shapes: properties and insurance

Declared in schema v1 per R8; populated at **1.1**, when the life modules land (`PLAN.md` M10 moved `Property`, the coverage state machine and the insurance grid out of 1.0 by decision; `ENGINE-SPEC.md` §§9-11). Declaring the item types now is what makes that release additive.

```rust
pub struct Property {
    pub id: Id, pub label: String, pub owner: Owner,
    pub use_kind: PropertyUse,                     // Primary | Secondary | Rental | Land
    pub value: Sourced<Cents>, pub appreciation: GrowthRule,
    pub basis_ledger: BasisLedger,
    pub acquired: MonthYm,
    pub occupancy: Vec<OccupancySpan>,             // drives the §121 ownership/use tests
    pub carrying: CarryingCosts,                   // tax, insurance, maintenance, HOA, capex
    pub rental: Option<RentalTerms>,               // rent, vacancy, management, depreciation
    pub linked_debt_ids: Vec<Id>,
    pub as_of: DateYmd,
}
pub struct BasisLedger { pub purchase_price: Cents, pub closing_costs: Cents,
                         pub improvements: BTreeMap<Year, Cents>,
                         pub depreciation_taken: BTreeMap<Year, Cents>,
                         pub casualty_adjustments: BTreeMap<Year, Cents> }

pub struct InsurancePolicy {
    pub id: Id, pub label: String, pub insured: PersonId, pub owner: Owner,
    pub kind: PolicyKind,                          // TermLife | PermanentLife | Ltd | Std | Umbrella | Ltc
    pub face_or_benefit: Cents, pub premium: Cents, pub premium_period: Period,
    pub group: bool, pub employer_paid: bool,
    pub start: MonthRef, pub end: MonthRef,
    pub elimination_days: Option<u16>, pub own_occ_months: Option<u16>,
    pub offsets_social_insurance: bool,
    pub as_of: DateYmd,
}

pub struct CoverageInput {                         // on Person
    pub regime_by_span: Vec<CoverageSpan>,         // Employer | Cobra | Aca | Medicare | Uninsured
    pub aca_rating_area: Option<String>, pub tobacco: bool,
    pub employer_premium_share: Option<Cents>,
    pub medicare_parts: Option<MedicareElection>,
}
```

The basis ledger is why `Property` is an object rather than an expense line: a relocation or downsizing flow without basis cannot compute seller's costs, the §121 exclusion or depreciation recapture, and every consumer tool observed makes the user enter those by hand. `occupancy` spans exist so the ownership-and-use test is computed, not asserted. Coverage is per person per span, while the premium tax credit and IRMAA are per tax household — kept separate from the start because their two MAGI definitions run on two different clocks (PTC on current-year household MAGI, IRMAA on MAGI from two years earlier).

---

## 11. Policies as data (seam S7)

```rust
pub struct Policies {
    pub contribution: ContributionPolicy,
    pub withdrawal: WithdrawalPolicy,
    pub rebalancing: RebalancingPolicy,
    pub roth_verdict: RothVerdictPolicy,
    pub gains_budget: Option<Cents>,
}

/// Read by the Roth-vs-Traditional card (`ENGINE-SPEC.md` §6): a verdict is withheld as
/// `Sensitive` when the assumed survivor window is shorter than this. Default 5; a preference,
/// listed in `DECISIONS.md` open decision 8.
pub struct RothVerdictPolicy { pub min_survivor_years: u8 }

pub struct ContributionPolicy {
    pub id: Id,
    pub preset: Option<PresetId>,                 // "default-tiers" | "fixed-hurdle" | "age-indexed"
    pub preset_vintage_id: Option<String>,        // the vintage the rules below were expanded from
    pub rules: Vec<ContributionRule>,             // ordered; the LIST ORDER is the tier order (normative); never empty
    pub thresholds: BTreeMap<String, Threshold>,  // every named threshold, editable and printed
    pub step_cents: Cents,                        // greedy-loop step size; data, never a code literal
}
pub struct ContributionRule {
    pub id: Id,
    pub tier_label: String,             // display label only ("1a", "2", "7b", "8-10"); consecutive rules sharing a
                                        // label form one tier; it never decides order, which is the list position
    pub label: String,
    pub option: NextDollarOptionKind,   // which option this rule dispatches to; the loop's switch
    pub test: RuleTest,                 // CashBelow{months: Ratio} | MatchUnfilled | DebtRateAbove{ratio} | …
    pub capacity: Capacity,             // RemainingStatutoryRoom{limit_param} | MatchGap | Amount | DebtBalance | …
    pub r_u: RuFormula,                 // named formula id + its inputs; the engine computes the value
    pub liquidity_class: LiquidityClass,// SafeLiquid | LiquidRisky | IlliquidPre59Half
    pub enabled: bool,
}

/// The payload-free discriminant of `ENGINE-SPEC.md` §5.1's `NextDollarOption`. A stored rule names the kind;
/// the engine resolves it at run time to the payload-carrying option for the person, plan, debt, goal or account
/// the rule applies to. The wire forms are the camelCase variant names, enumerated in plan.schema.json.
pub enum NextDollarOptionKind {
    CashBuffer, EmergencyFund, EmployerMatch, DebtPrepay, HsaPayroll, HsaDirect,
    IraTraditional, IraRoth, BackdoorRoth, PlanPreTax, PlanRoth, MegaBackdoor,
    Plan529,                            // active at M10; the variant exists from M2 so the shape is stable
    Taxable, TaxableSafe,
}
```

`NextDollarOptionKind` is the stored, payload-free form of `ENGINE-SPEC.md` §5.1's `NextDollarOption` (`EmployerMatch{person, plan}`, `DebtPrepay{debt}`, ...): the rule stores the kind, and the engine resolves it to the payload-carrying option at run time. The variant list is closed and is what `lint:schema-identifiers` (`TESTING.md` §9) resolves `option` against; a variant added to one enum without the other fails the build.

The rule list is **stored data the user can edit and reorder**, not a hard-coded sequence: each rule has a test, a capacity and an `r_u` formula reference, and the engine's greedy loop walks the list **in list order** (`ENGINE-SPEC.md` §5.8). `tier_label` is a display label and nothing else: the default table's labels (`ENGINE-SPEC.md` §5.4) are the labels of the published ten-tier ordering `ENGINE-SPEC.md` §5.4 reconciles — `1a`, `2`, `3`, `1b`, `4-7`, `7b`, `8-10` — and the income-shock fund labelled `1b` deliberately sits *after* high-interest debt, an order no integer field could carry. Consecutive rules sharing a label form one tier for the "order by `r_u`" toggle. `option` is what makes the loop dispatchable: without a field tying a stored rule to a `NextDollarOptionKind`, a rule is a label the engine cannot act on.

**Presets expand at plan creation; they do not resolve at run time.** The preset files live in `params/policies/*.toml` under the same provenance, vintage and lock rules as every other parameter table (`ARCHITECTURE.md` §3). Creating a plan **copies** the chosen preset's rules into `rules` and records `preset_vintage_id`, so `preset` is thereafter a provenance label, not an indirection. The two forms have different consequences, which is why this is decided rather than implied: an expanded copy is editable and reorderable — what this section promises — and it pins the ordering a recommendation was produced under, while run-time resolution would be pinnable but not editable. A **non-empty `rules` is therefore a validator requirement** (§13 check 9): the greedy loop takes the first tier in list order having an eligible option, so an empty list allocates nothing. Loading a newer preset vintage is an explicit, diffed action that shows what the user's edits would lose. Default thresholds ship as sourced parameters — high-interest hurdle **8%** (White Coat Investor), the age-indexed preset (Money Guy: 20s >6%, 30s >5%, 40s >4%, [FOO](https://moneyguy.com/foo/)), spending-shock buffer `max($2,000, 0.5 × monthly essential)` and income-shock reserve 3-6 months ([Vanguard](https://investor.vanguard.com/investor-resources-education/emergency-fund)) — never constants in engine code, which the dollar-literal lint forbids.

```rust
/// One source class in the deficit-routing order of `ENGINE-SPEC.md` §2.4 (steps 2-6). A unit enum,
/// so it serializes as a bare camelCase string: "cash" | "taxable" | "traditional" | "roth" | "hsa".
/// RMDs are not a tier — they are already in income (`ENGINE-SPEC.md` §2.4 step 1) and `rmd_first` says so.
pub enum WithdrawalTier {
    Cash,          // cash above the spending-shock buffer
    Taxable,       // taxable accounts, realized gain pro rata to basis
    Traditional,   // traditional / employer pre-tax, 59½ guard applies
    Roth,          // contribution basis, then seasoned conversions, then earnings
    Hsa,           // banked receipts tax-free, then ordinary income after 65
}

pub struct WithdrawalPolicy {
    pub order: Vec<WithdrawalTier>,       // default (the "conventional" order, ENGINE-SPEC §2.4):
                                          // [Cash, Taxable, Traditional, Roth, Hsa]; RMDs come first regardless
    pub rmd_first: bool,                  // true; a 59½ guard is separate
    pub spending_rule: SpendingRuleConfig, // ConstantReal (M3); VanguardDynamic, GuytonKlinger, Vpw,
                                          // Ratchet, Cape, RiskBasedGuardrails (M7)
    pub early_access: Vec<EarlyAccessRule>, // opt-in, empty by default = SHORTFALL before 59½
    pub inherited_spousal_election: SpousalElection,  // TreatAsOwn | RemainBeneficiary
}
pub enum EarlyAccessRule {
    /// The explicit opt-in to BOOK the penalized alternative the engine otherwise only prints:
    /// after penalty-free sources are exhausted, draw from these accounts with the
    /// early-distribution additional tax booked on its own named line (`ENGINE-SPEC.md` §3.2 `f5329.*`).
    Penalized { account_ids: Vec<Id> },
    SeparatedAt55 { employer_plan_id: Id },
    Sepp72t { account_id: Id, started: MonthYm, method: SeppMethod },  // permitted set owned by the tax spec
}
pub struct RebalancingPolicy {            // M9
    pub look_frequency: LookFrequency, pub band: BandSpec,
    pub destination: BandDestination,     // Target | Halfway | Edge   (default Halfway)
    pub scope: RebalanceScope,            // BreachingOnly | All
    pub location_priority: Vec<AssetClass>,
}
pub struct BandSpec { pub absolute_pp: Ratio, pub relative_pct: Ratio, pub combine: CombineRule }
pub enum CombineRule { Smaller, Larger, AbsoluteOnly, RelativeOnly }
```

**Rebalancing band default: `{absolute_pp: 5, relative_pct: 25, combine: Smaller}`.** That is the 5/25 rule `SIMULATION-SPEC.md` §16.3 specifies, and its own arithmetic confirms the 25: a 10% target banded 7.5–12.5 is 25% relative, not 20%. The 20%-relative figure comes from a single study offered there as evidence that the width is worth editing; it ships as a **named alternative preset**, not as the default. `combine` is part of the shape because "5 percentage points or 25% relative" is ambiguous without it — one band is wider for large targets and the other for small ones, and `Smaller` (trade on whichever triggers first) is the rule the default encodes.

**Early access is opt-in and empty by default.** `EarlyAccessRule` exists so that a household can say, as a stored fact of its own plan, either that it accepts the penalty (`Penalized`) or that it genuinely has a penalty-free route before 59½ (`SeparatedAt55`, `Sepp72t`), rather than the ledger inferring or defaulting either. Absent any entry, the engine takes the default path: it **never silently books a penalized withdrawal**; it reports a `SHORTFALL` with the computed penalized alternative printed beside it, so the user sees both the gap and what closing it would cost (`ENGINE-SPEC.md` §2.4 owns that rule; `SIMULATION-SPEC.md` §13.1 references it rather than restating it). `Penalized` is the one entry the engine honours from M3, because it asserts no statutory eligibility — it only consents to the tax the engine already computes. The statutory conditions on the other two — which separations qualify, the substantially-equal-payment methods and their modification window — are **not covered by any archived source** and are owned by `ENGINE-SPEC.md` §2.4 and §3.2, like the early-distribution additional tax's own rate and exception list; the fields are declared here so the shape is stable at seam S7, and no engine honours them until those conditions are verified against statute at the hand-verification gate (`PLAN.md` R11), which is why they land at M8 (`PLAN.md` M3, `ARCHITECTURE.md` §7).

Other defaults follow the convergent published evidence `SIMULATION-SPEC.md` §16.3 cites (look often, trade rarely, wide bands, cash flows first) and every one is editable and printed on the resulting recommendation.

---

## 12. Scenarios as ordered diffs

```rust
pub struct Scenario {
    pub id: Id, pub name: String,
    pub parent_id: Option<Id>,
    pub kind: ScenarioKind,
    pub assumption_set_id: Id,
    pub param_vintage_ids: Vec<String>,
    pub seed: Seed,
    pub ops: Vec<PatchOp>,             // RFC 6902, ordered
    pub created: DateYmd, pub notes: Option<String>,
}
pub enum ScenarioKind { Baseline, Proposed, WhatIf, HypotheticalFacts, Stress }
```

```
resolve(s) = validate(apply_patch(s.parent_id ? resolve(parent) : base, s.ops))
```

`Baseline` ("Current Course") has `parent_id = None` and **`ops` empty by construction**: a validator rejects a non-empty baseline, because a baseline that diffs from the fact base is not a baseline. *Adopt* appends ops to `Proposed` and creates an `ActionItem`. The ops list doubles as the human-readable diff. Copy-per-scenario is rejected — copies diverge the moment a fact changes.

### 12.1 The fact-path allowlist

Classification is by path pattern. **Unlisted paths are denied for every kind** — the allowlist is closed, not open.

| Path pattern | Class | Patchable by |
|---|---|---|
| `/assumptionSets/*`, `/paramOverrides/*` | assumption | `Proposed`, `WhatIf`, `Stress`, `HypotheticalFacts` |
| `/policies/**` | policy | `Proposed`, `WhatIf`, `Stress`, `HypotheticalFacts` |
| `/goals/*`, `/events/*` | plan intent | `Proposed`, `WhatIf`, `HypotheticalFacts` |
| `/persons/*/retirement`, `/persons/*/socialSecurity/claimAge`, `/persons/*/socialSecurity/survivorClaimAge`, `/persons/*/socialSecurity/futureEarnings` | decision | `Proposed`, `WhatIf`, `HypotheticalFacts` |
| `/incomeStreams/*/amount`, `/growth`, `/start`, `/end`; `/expenseStreams/*/amount`, `/growth`, `/essential` | forward-looking | `Proposed`, `WhatIf`, `HypotheticalFacts` |
| `/incomeStreams/*/payroll/deferralPct`, `/deferralType`, `/afterTaxPct`, `/hsaPayrollDeduction` | decision | `Proposed`, `WhatIf`, `HypotheticalFacts` |
| `/debts/*/extraPayment`, `/debts/*/apr` (refinance), `/accounts/*/targetAllocationId`, `/targetAllocations/*`, `/accounts/*/beneficiary/**` | decision | `Proposed`, `WhatIf`, `HypotheticalFacts` |
| `/household/filingStatus`, `/household/residentState`, `/household/survivor/**`, `/household/planningAge` | mixed fact/decision | `Proposed`, `WhatIf`, `HypotheticalFacts` |
| `/accounts/*/balance`, `/costBasis`, `/holdings/**`, `/rothLedger/**`, `/taxType`, `/owner`, `/inherited/**`; `/debts/*/balance`; `/persons/*/dob`, `/assumedDeath`, `/employment`; `/household/priorMagi`; `/ytd/**`; `/properties/*/value`, `/basisLedger/**`; `/insurance/**`; `/employerPlans/**` | **fact** | `HypotheticalFacts` **only** |
| `/schemaVersion`, `/planId`, `/created`, `/scenarios/**`, `/factSnapshots/**`, `/resultSnapshots/**`, `/recommendations/**`, `/actions/**`, `/reviews/**`, `/importProvenance/**` | **immutable** | **no kind, ever** |

**The payroll row is deliberately narrow.** The four named leaves are the contribution decisions the next-dollar engine recommends and *Adopt* writes: `PlanPreTax` and `PlanRoth` move `deferralPct`/`deferralType`, `MegaBackdoor` moves `afterTaxPct`, `HsaPayroll` moves `hsaPayrollDeduction`. It is **not** widened to `/incomeStreams/**` or `/payroll/**`, because `equityComp`, `groupLifeFace` and `groupLtd` are facts about an employment arrangement, not decisions a scenario gets to invent. Without this row a closed allowlist would reject every contribution recommendation the product exists to make — the ops would be generated, then refused at save time.

**Every option has a writable adopt path**, and a test asserts it option by option against `ENGINE-SPEC.md` §5.1:

| Option | Adopt writes |
|---|---|
| `PlanPreTax`, `PlanRoth` | `/incomeStreams/*/payroll/deferralPct`, `/deferralType` |
| `MegaBackdoor` | `/incomeStreams/*/payroll/afterTaxPct` + a `RothConversion` event (§8) |
| `HsaPayroll` | `/incomeStreams/*/payroll/hsaPayrollDeduction` |
| `HsaDirect`, `IraTraditional`, `IraRoth`, `Taxable` | a `PlannedContribution` event (§8) |
| `TaxableSafe` | the same path as `Taxable`: a `PlannedContribution` event (§8) into the account the option names — the household's T-bill / HYSA sleeve, a `Cash`-type account or a `Taxable` account whose `target_allocation_id` resolves to `Cash`, `TreasuryInt` and `Tips` weights only (`ENGINE-SPEC.md` §5.1, §5.4) |
| `BackdoorRoth` | a `PlannedContribution` into the traditional IRA + a `RothConversion` event |
| `DebtPrepay` | `/debts/*/extraPayment` |
| `EmployerMatch` | nothing: it is reached by `deferralPct`, and the per-period warning is an `ActionItem` |
| `CashBuffer`, `EmergencyFund` | `/policies/contribution/thresholds/**` (target), balances stay facts |
| `Plan529` (M10) | a `PlannedContribution` into the 529 account |
| `PlanPreTax` / `PlanRoth` split change (Roth-vs-Traditional card only, M3) | `/incomeStreams/*/payroll/deferralType` (`Split{pctRoth}`, §7) |

Two tests carry this: one resolves §19's example scenario against the allowlist and fails if any op in the shipped example is refused by the shipped validator, and one walks the option set above and fails on an option with no writable path.

The last row of the classification table is a soundness requirement, not a convenience: a scenario able to patch the scenario collection, the audit trail or a stored pin could rewrite its own provenance. `HypotheticalFacts` is the single escape hatch, for exactly the questions that need counterfactual facts — an early death, a disability, a crash the day before retirement. It is labelled wherever it appears, and its results are never compared to the baseline without that label.

Validation after patching is mandatory and total: a scenario resolving to an invalid plan is rejected at save time, not run time. An op whose path does not exist is rejected (RFC 6902 `replace` semantics), which is what makes a migration that fails to rewrite patch paths fail loudly (§14).

---

## 13. Referential integrity

A validator runs on every save and after every patch application. It checks:

1. `Record` key equals `item.id`, everywhere.
2. Every `Id` reference resolves (`employer_plan_id`, `target_allocation_id`, every `glide_path` account or class reference, `secured_by_property_id`, `funded_by_account_ids`, `dependent_id`, `to_account_id`, `assumption_set_id`, `body_ref`, `early_access` plan and account ids including every `Penalized.account_ids` entry, every key of `ytd.contributions` and `ytd.employer_contrib`). Every `ParamTableId` (`mortality_table_id`, parameter and assumption vintage ids) resolves against the pinned vintages in `params/`, reported as a distinct class of failure — a missing table is a packaging bug, a dangling `Id` is a document bug.
3. No `P2`-owned row and no `Owner::Joint` when `persons.p2` is absent (§2.2).
4. Every `MonthRef::Age`/`Event` names a present person.
5. Every `GrowthRule::Schedule` has `after_last`; every growing amount has `basis`.
6. `beneficiary.spouse_fraction ∈ [0,1]`; `Ratio.den != 0`; every `Year` in `1900..=2200` (§2.1 makes the domain a check, not a type bound); `start ≤ end` after resolution.
7. Holdings and lots reconcile to `balance` and `cost_basis` exactly, or `holdings` is empty (§6).
8. Every `TargetAllocation.weights` — and every `LinearToAge.end_weights` — sums to exactly 10,000 basis points (§6).
9. Every `ContributionPolicy` has a non-empty `rules` list (§11).
10. Every `OverrideValue` variant matches the declared unit of what it overrides (§15): a `USD` table takes `Cents`, a rate or factor table takes `Ratio`, and an `AssumptionSet` field takes the variant of its own type; a `ParamOverride.breakdown_key` is a wire form of the table's declared breakdown (for `filingStatus`, the `FilingStatus` wire forms of §4).

**`removePerson(p2)` is a specified graph operation, not a field delete.** It collects every referencing path — accounts owned by P2 or Joint, `employer_plans.owner == P2`, streams and debts owned by P2 or Joint, every `MonthRef` anchored to P2, `insurance.insured == P2`, `household.filing_status ∈ {Mfj, Mfs, Qss}`, `household.survivor`, `household.prior_magi`, `ytd.wages[P2]` — and returns them **as a list the user must resolve** (reassign, retarget, or delete). It never silently rewrites and never leaves a dangling reference; TPAW does the same scrub before flipping `withPartner` ([deletePartner](https://github.com/bengmathew/tpaw/blob/main/packages/common/src/Params/PlanParams/PlanParamsChangeAction/GetPlanParamsChangeActionImpl/GetDeletePartnerChangeActionImpl.ts)).

**Deletion is tombstoning for anything a snapshot may reference.** Accounts, debts and properties set `tombstoned: Some(date)`: excluded from projection, retained in the document, so an old `ResultSnapshot` still resolves its per-account rows. Hard deletion is offered only where no snapshot references the row, and that check is mechanical.

**Tombstoning plus an append-only log means nothing leaves the file on its own, so purge is a specified operation.** The change log is `{timestamp, action, reverseDiff}` (ADR-009), and a `reverseDiff` carries by construction the prior value of every fact ever changed. Combined with tombstoning, a mistyped earnings figure, a former employer's plan detail or a balance the user explicitly deleted is retained indefinitely, and `removePerson(p2)` leaves that person's entire history behind even after every live reference is resolved. The exposure of the plan file is therefore **the union of everything ever entered**, not the current plan — which makes retention a confidentiality question, not only the file-growth question ADR-009's open decision #4 frames it as.

```
vault compact --purge-before <date>     # also offered in the UI, never automatic
```

Purge drops `reverseDiff` payloads and snapshot **bodies** older than the date while retaining **metadata-only** log entries (`timestamp`, action kind, affected collection) so the audit trail — what changed and when — survives the removal of what it changed *to*. Purged entries are no longer undoable and the UI says so before the action, not after. `removePerson` offers purge of that person's history as an explicit, separately confirmed step, distinct from resolving live references. Purge forces a whole-file rewrite and rotates backups, because a compaction that leaves the old ciphertext in `*.pfplan.bak` has removed nothing. The limit is stated plainly in the UI and in `docs/threat-model.md`: **purge cannot reach older backups, copies, or any file the user made outside the application.** Open decision #4 is re-decided on those terms in `DECISIONS.md` (its "Open decisions" list, item 4; carried in §21 below), with data minimisation as a stated input alongside file growth.

---

## 14. Schema versioning and migrations

```rust
pub trait Migration {
    const FROM: u16; const TO: u16;
    fn migrate_document(&self, prev: serde_json::Value) -> Result<serde_json::Value, MigrationError>;
    fn rewrite_patch_path(&self, path: &str) -> Option<String>;  // None = drop the op with a warning
}
```

On load: read `schemaVersion`; for each `k` in `v+1..=CURRENT`, apply `migrate_document`, then apply `rewrite_patch_path` to **every op of every stored scenario**, then parse against `schema_k` and run the validator. Old shapes are frozen in `pfp-model/src/schema/v{N}.rs` and never edited again. `schemaVersion` bumps only for shape changes; an additive field whose default reproduces prior output needs no bump.

**Migrations must rewrite scenario patch paths, not just the base.** A renamed field silently breaks every stored op targeting it, and RFC 6902 then rejects the op or — worse, if the rename is a move — applies it somewhere plausible. `rewrite_patch_path` returning `None` drops the op and records a migration warning surfaced in the UI; it never guesses.

**Golden fixtures.** `fixtures/plans/plan.v{N}.json` for every released `N`, each migrated to current in CI with an `insta` snapshot of *both* the resolved base **and** every scenario's resolved output — that second half is what catches a migration silently moving a number. A fuzz target drives patch application and the chain with a committed synthetic corpus (M2).

**`migrate_1`: folding in the M1 `explorer` section.** M1 ships before schema v1 and stores Tax Year Explorer worksheets in a pre-schema section:

```rust
pub struct ExplorerV0 {                       // container section "plan", schemaVersion 0
    pub worksheets: Vec<ExplorerWorksheet>,   // {id, year, filingStatus, taxInputs, savedAt, label}
}
```

`migrate_1` builds a v1 `Plan` with a generated `planId`, a `Household` carrying the most recent worksheet's `filing_status` and `residentState: Unknown` (which forces the state question in setup), an empty `persons` map for the setup flow to fill, and each worksheet preserved as a `FactSnapshotIndex` of kind `TaxYearExplorer` whose body holds the original `TaxInputs`. No worksheet is discarded, no number invented. It ships with its own `plan.v0.json` fixture.

---

## 15. Public parameters, vintages, overrides and assumption sets

Public parameter tables live in the **repository** (`params/`), not in the plan file — they are public law and public research with provenance, and they are exactly what the no-financial-data rule permits (ADR-023).

```toml
# params/vintages/2026-federal/std_deduction.toml
id        = "irs.std_deduction"
unit      = "USD"
period    = "year"
breakdown = ["filingStatus"]      # breakdown keys are the FilingStatus WIRE FORMS of §4:
                                  # single | mfj | mfs | hoh | qss — never a long name, never an alias

[values.single]                 # integer dollars as published
2025 = 15750
2026 = 16100
[values.mfj]
2025 = 31500
2026 = 32200
# [values.mfs], [values.hoh], [values.qss] likewise; abridged here

[projection]
rule         = "index"
index        = "cpi.chained"
index_series = "cpi.chained.aug12m"   # the archived chained-CPI series this table is uprated against;
                                      # the series' identity is confirmed at the M0 hand-verification gate
base_year    = 2024               # statutory base year for this item: 2024 post-OBBBA, 26 USC 63(c)(4)
[projection.base_values]          # statutory base-year amounts per breakdown key, integer dollars
single = 0                        #   same keys as [values]; transcribed from the statute at the
mfj    = 0                        #   hand-verification gate (PLAN.md R11), never back-solved from a
mfs    = 0                        #   published later year. Zero here means "not yet transcribed";
hoh    = 0                        #   a vintage cannot lock with a zero base value.
qss    = 0
[projection.rounding]
increment = 50
direction = "down"
basis     = "IncreaseOverBase"   # 26 USC 1(f)(7)

[[source]]
title     = "Rev. Proc. 2025-32"
url       = "https://www.irs.gov/pub/irs-drop/rp-25-32.pdf"
retrieved = "2026-09-17"
sha256    = "…"                  # of the archived copy under params/provenance/
```

**The projection block carries `index_series`, `base_year` and `base_values` because `basis: IncreaseOverBase` cannot be computed without them.** Under 26 USC 1(f)(7) the rounding applies to the *increase over the statutory base year*, not to the final amount ([26 USC 1(f)](https://www.law.cornell.edu/uscode/text/26/1)). The statute states that rule, and 26 USC 63(c)(4) the standard deduction's 2024 post-OBBBA base ([26 USC 63](https://www.law.cornell.edu/uscode/text/26/63)); no archived source states the bracket tables' 26 USC 1(f) base year or names the index series, so each table's `base_year`, `base_values` and `index_series` are read from statute and hand-verified before the M0 vintage locks (`TESTING.md` §5.2; correction C3) rather than asserted here. The pipeline is therefore `base_value × (index_year / index_base_year)`, with the $50 (or $25, or $1,000) reduction applied **once**, to the increase. Chaining year over year — uprating 2025 from 2024, then 2026 from the rounded 2025 — rounds twice and diverges from the published figure. That is a shape requirement, not an implementation note: seam S1 froze the parameter-table shape at M0 with year-keyed values, a projection rule and a `RoundingRule{increment, direction, basis}` and **no field holding the base-year amount**, so the statutory rule could not be expressed by the structure meant to express it. The fields above close that before S1 freezes, and archiving the chained-CPI index series and the base-year amounts belongs to M0 scope alongside them. The corresponding property is **path independence** — uprating 2024→2026 directly equals uprating 2024→2026 through any intermediate year *computed from the base*, which is true — and not associativity over a chain of rounded results, which is false under `IncreaseOverBase`; a negative test asserts that a year-over-year chain diverges from the statutory result on at least one 2026 threshold, so a correct implementation cannot be "fixed" into a wrong one to satisfy a wrong property.

**Breakdown keys are the wire forms of the breakdown's enum, and nothing else.** A table declaring `breakdown = ["filingStatus"]` keys `values` and `base_values` by `single | mfj | mfs | hoh | qss` — the `FilingStatus` wire forms §4 settles (§20 deviation 5) — not by the long names, not by an uppercase alias and not by a word such as `JOINT` that names no `FilingStatus` variant at all (`Joint` belongs to `Owner`, §2.2). The loader and `TESTING.md` §11.2 gate 9 resolve every breakdown key against the enum's wire forms, so a key no `FilingStatus` deserializes fails the build rather than silently pricing a status as zero; `ParamOverride.breakdown_key` is checked the same way (§13 check 10).

Rules: **a table with no projection rule is a CI error** — never-indexed items carry `rule = "flat"` explicitly (NIIT thresholds, the Social Security base amounts, the Additional Medicare thresholds, the $3,000 loss cap and the IRMAA top tier are all in that class). `RoundingRule` is data beside the parameter it governs, never a global mode: standard deduction $50 down on the increase over base, 415(c) $1,000 down, IRMAA nearest $1,000, PIA dime-truncated at each COLA, payable benefit dollar-truncated. IRS *projection* rounding conventions are **unverified**, so each increment is hand-verified against statute or notice before its vintage is locked (`PLAN.md` R11).

**Vintages are immutable.** `vintageId = "<name>@<sha256-of-contents>"`; `params/VINTAGES.lock` carries a hash per file and CI fails if a locked file changes. Corrections ship as a *new* vintage, never an edit, and released vintages are path-protected from AI-assisted edits. Values may be vendored verbatim from Tax-Calculator's registry, which is **public domain / CC0 1.0** and *not* MIT ([LICENSE](https://github.com/PSLmodels/Tax-Calculator/blob/master/LICENSE)); PolicyEngine-US's file **shape** may be imitated freely, its AGPL code never linked ([amount.yaml](https://github.com/PolicyEngine/policyengine-us/blob/master/policyengine_us/parameters/gov/irs/deductions/standard/amount.yaml)).

**Overrides are a separate layer inside the plan**, displayed beside the sourced value and never merged into it:

```rust
pub struct ParamOverride { pub id: Id, pub param_id: String, pub year: Option<Year>,
                           pub breakdown_key: Option<String>, pub value: OverrideValue,
                           pub reason: String, pub as_of: DateYmd }

/// The closed set of values an override may carry — the same one for `ParamOverride` (a public
/// parameter table) and `AssumptionOverride` (a field of an `AssumptionSet`, below). One variant
/// per `unit` a table or assumption field can declare; there is no float variant (R4).
/// Externally tagged, camelCase on the wire like every enum here:
/// {"cents": 1610000} | {"ratio": {"num":27,"den":1000}} | {"int": 67} | {"bool": true} | {"text": "nchs-2023-female"}
pub enum OverrideValue {
    Cents(Cents),      // a USD table (`unit = "USD"`, integer dollars as published; stored as cents, R4)
    Ratio(Ratio),      // a rate, factor, divisor, mean or standard deviation
    Int(i64),          // an age, a year, a count (e.g. `horizon_years`)
    Bool(bool),        // a dated, switchable rule flag
    Text(String),      // an identifier, e.g. `mortality_table_id`, an inflation preset id, `mean_kind`
}
```

`reason` is required. An override without a stated reason is indistinguishable a year later from a typo. `OverrideValue` is `pfp-model`'s type, declared once here and serialized by both `paramOverrides{}` and `assumptionSets{}.overrides`; the validator checks that the variant matches the target's declared unit (§13 check 10), because a `Ratio` written over a `USD` table is exactly the silent unit confusion `Cents(i64)` exists to make uncompilable.

**Assumption sets are vintages too.** The repository ships dated `AssumptionSet` files; the plan references them by id and may carry plan-local overrides in the same layered way. The plan-side row is the **reference**, declared here beside the vintage shape it points at; the two are different types, and only the reference lives in the document (§3 `assumption_sets`):

```rust
/// What `Plan.assumption_sets{}` holds (§3): a pointer to a shipped `AssumptionSet` vintage plus the
/// plan-local override layer, never a copy of the vintage. §19 shows one: {"id", "vintageId", "overrides": {}}.
pub struct AssumptionSetRef {
    pub id: Id,
    pub vintage_id: String,                                // "<name>@<content-hash>" of the AssumptionSet file
                                                           // (the same form as Scenario.param_vintage_ids); must resolve
                                                           // against the pinned vintages in params/ (§13 check 2)
    pub overrides: BTreeMap<String, AssumptionOverride>,   // keyed by the overridden field's path in the set below,
                                                           // e.g. "inflation.mean", "stockBondCorrelation",
                                                           // "classes.usLarge.mean"; displayed beside the sourced value
}
pub struct AssumptionOverride { pub value: OverrideValue, pub reason: String, pub as_of: DateYmd }   // reason required, as for ParamOverride

pub struct AssumptionSet {
    pub id: Id, pub label: String, pub as_of: DateYmd,
    pub horizon_years: u8,                              // scalar, matching the vintage TOML
    pub basis: ReturnBasis,                             // Nominal | Real
    pub mean_kind: MeanKind,                            // Arithmetic | Geometric; absent is a CI error
    pub classes: BTreeMap<AssetClass, ClassAssumption>,
    pub correlation: CorrelationMatrix,
    pub stock_bond_correlation: Option<Ratio>,          // the knob; replaces equity × nominal-bond entries
    pub inflation: InflationAssumption,                 // mean + AR(1) preset id and params (SIMULATION-SPEC §5)
    pub chained_cpi_wedge: Ratio,                       // default 0: annual amount the statutory chained index is
                                                        // assumed to run below the path's CPI (ENGINE-SPEC §1.3)
    pub medical_inflation: Ratio, pub wage_growth: Ratio,
    pub housing_appreciation: Ratio, pub ss_cola: Ratio,
    pub market_data: MarketData,
    pub discount: DiscountRates,
    pub mortality_table_id: ParamTableId,               // the household DEFAULT table; Person.mortality_table_id (§4)
                                                        // overrides it per person; never optional, so one always resolves
    pub swr_default: Ratio,
    pub sources: Vec<SourceRef>,
}
pub struct ClassAssumption { pub mean: Ratio, pub sd: Ratio, pub geometric: Option<Ratio> }
pub struct MarketData {                                 // each dated; v1 never fetches them
    pub tips_real_yield_20y: Sourced<Ratio>,
    pub treasury_10y: Sourced<Ratio>,
    pub cape: Sourced<Ratio>,
}
pub struct DiscountRates { pub nominal_safe: Ratio, pub real_safe: Ratio }
```

**Every field here is read by an engine, and several decide answers.** `treasury_10y` feeds `hurdle_low = y_10yr + 3pp`, the medium/low debt boundary in the next-dollar waterfall (`ENGINE-SPEC.md` §5.4, D1). `tips_real_yield_20y` is the funded ratio's discount rate (`SIMULATION-SPEC.md` §11.1), where a one-point move changes a sustainable budget by more than 15%. `discount.nominal_safe` prices the IRMAA change two years out and so enters every marginal rate and every `r_u` (`ENGINE-SPEC.md` §3.3). `mean_kind` decides whether a figure is fed to a generator as-is or converted through the lognormal identity — feeding a CAGR to a draw double-counts volatility drag — and `basis` decides whether inflation is applied. `geometric` is per class because the conversion is per class. Values not carried here are values the user cannot see, source, pin or override, which is exactly what the transparency requirement forbids; these decide prepay-versus-invest and the funded ratio, the two answers most sensitive to a market input. `horizon_years` is a **scalar**, matching `SIMULATION-SPEC.md` §3.1's `horizon_years = 10`, because the normative TOML the loader parses carries a scalar.

Capital-market assumptions are dated, immutable vintages of the shape above. **Third-party capital-market-assumption sets are not bundled until their publisher's redistribution terms are confirmed and recorded in `docs/licence-watchlist.md`**; v1 ships the vintage file shape, the loader, published checksums and the user's own entry, and no figure from a published set is reprinted in this document or in `params/` (`SIMULATION-SPEC.md` §3.2 and `ARCHITECTURE.md` §10.3 state the same rule; an annual publication such as J.P. Morgan's [Long-Term Capital Market Assumptions](https://am.jpmorgan.com/us/en/asset-management/institutional/insights/portfolio-insights/ltcma/) is a reference the user may obtain and enter). Optimistic and pessimistic variants are **derived** as ±20% around the average rather than hand-typed ([Boldin](https://help.boldin.com/en/articles/11049646-assumptions-for-inflation-and-appreciation)). The default SWR is **below 4%** (≈3.5%) for long horizons: 4% at 50/50 succeeds roughly 95% over 30 years and roughly 65% over 60 ([ERN](https://earlyretirementnow.com/2016/12/07/the-ultimate-guide-to-safe-withdrawal-rates-part-1-intro/)).

`ReturnPath` — the per-path `returns[t][class]` and `inflation[t]` tensor — is **engine-internal and never stored**. Monte Carlo paths are not persisted anywhere; a run stores summaries plus the seed, and any single path is reproduced from `(seed, path_index)`.

---

## 16. Result pinning and snapshots

```rust
pub struct ResultPin {
    pub engine_version: String,        // semver + commit
    pub binary_digest: String,         // unsigned SHA-256 of the executable
    pub schema_version: u16,
    pub param_vintage_ids: Vec<String>,
    pub assumption_set_id: Id,
    pub market_data_as_of: Option<DateYmd>,
    pub generator: GeneratorConfig,
    pub seed: Seed, pub n_paths: u32,
    pub seed_source_scenario_id: Id,   // whose seed this run actually used
    pub criterion: CriterionConfig,
    pub inputs_hash: String,           // §2.6
}
```

Every stored or exported output carries one: `Recommendation`, `ResultSnapshot`, every report view-model, every CSV/JSON export, every print.

`seed_source_scenario_id` exists because `Scenario.seed` is per scenario while `simulate()` takes **one** seed for the whole slice (common random numbers are a property of the API, not of caller discipline — ADR-013). A `PairedDelta` across two scenarios is therefore pinned to a seed that is not the seed stored on every scenario compared, and without this field the pin does not say which one won. It names the scenario whose seed was used; the UI shows it on any comparison whose members' stored seeds differ, so "pinned to the RNG seed" remains a true statement for multi-plan runs and not only single-plan ones.

```rust
pub struct FactSnapshotIndex  { pub id: Id, pub as_of: DateYmd, pub note: Option<String>,
                                pub summary: FactSummary, pub body_ref: SectionRef }
pub struct ResultSnapshotIndex{ pub id: Id, pub run_at: DateYmd, pub scenario_id: Id,
                                pub pin: ResultPin, pub summary: ResultSummary,
                                pub body_ref: SectionRef }
```

Bodies live in the container's `snapshots` and `results` sections (§3). A `FactSnapshot` body is `Record<accountId, {balance, costBasis}>` plus debts and property values — **account ids, never names** — written automatically (toggleable) whenever balances update, the pattern ProjectionLab calls Progress Points ([balances](https://projectionlab.com/help/update-account-balances)). Tombstoned rows keep old snapshots resolvable (§13).

**Staleness, honestly.** A stored result whose pins no longer match the running engine, vintages or `inputsHash` is shown as **stale**, re-run on request with the difference displayed, never silently recomputed. The project does not claim bit-reproduction of an old result under a new binary, because a release does not embed old engines. The annual review (M9) reads exactly these pins to attribute each KPI delta to facts vs assumptions vs law (vintage) vs engine version — the whole answer to "why did the plan change?".

```rust
pub struct Review { pub id: Id, pub date: DateYmd, pub kind: ReviewKind,  // Annual | Quarterly
                    pub snapshot_before: Id, pub snapshot_after: Id,
                    pub material_changes: Vec<String>, pub assumption_changes: Vec<Id>,
                    pub kpi_deltas: Vec<KpiDelta>, pub action_ids: Vec<Id> }
pub struct ActionItem { pub id: Id, pub label: String, pub owner: Owner,
                        pub due: Option<DateYmd>, pub status: ActionStatus,
                        pub source_recommendation_id: Option<Id>, pub pin: ResultPin }

/// The stored recommendation (`Plan.recommendations{}`, live from M2). Field for field the shape
/// `ARCHITECTURE.md` §4.2 and `ENGINE-SPEC.md` §12 declare — a verbatim reference copy with those
/// spellings, as §2.1 does for the shared primitives; `pfp-decide` produces it, `pfp-model` stores it.
pub struct Recommendation {
    pub id: Id,
    pub kind: RecommendationKind,        // the producing module: nd | rvt | conv | debt | ins (ENGINE-SPEC §12 template namespaces)
    pub scenario_id: Id,                 // the scenario it was computed against
    pub year: Year,
    pub action: RecommendedAction,       // the typed action *Adopt* writes, one of the adopt paths of §12.1
    pub dollars: Cents,
    pub tier: String,                    // the `tier_label` of the ContributionRule it came from (§11); display only
    pub r_u: Ratio,                      // the ranked quantity, ENGINE-SPEC §5.3
    pub explanation: Explanation,        // ARCHITECTURE §4.2; reconciliation invariant, TESTING I13
    pub pins: ResultPin,                 // spelled `pins` in ARCHITECTURE §4.2 and ENGINE-SPEC §12; one ResultPin (above)
    pub basis: ValidationBasis,          // Tier1Exact | OracleChecked | SingleOracle | PropertyTestedOnly
    pub status: RecommendationStatus,    // whether *Adopt* has written it (§12: adopting creates an ActionItem and
                                         // appends ops to the Proposed Plan) and whether its pins still match (stale rule below)
}
```

`Explanation`, `Bound` and `ValidationBasis` are `pfp-explain`'s types (`ARCHITECTURE.md` §3, §4.2) and are stored as-is; `RecommendationKind`, `RecommendedAction` and `RecommendationStatus` are `pfp-decide`'s enums, closed sets an engine and a UI both switch on, so a new option or status is a schema change like any other.

---

## 17. The snapshot-import model and the connector seam

The product imports **snapshots**, never transactions. There is no ledger, no categorisation and no net-worth history product; current state is an input.

```rust
pub struct SnapshotDraft {
    pub source: DraftSource,             // File{format, filename_hash} | Connector{id} | Manual
    pub fetched: DateYmd,
    pub accounts: Vec<DraftAccount>,     // {external_id, label, kind_hint, balance, as_of,
                                         //  holdings: Vec<DraftHolding>, lots: Vec<DraftLot>}
    pub debts: Vec<DraftDebt>,
    pub warnings: Vec<ImportWarning>,    // unparsed rows, ambiguous units, currency mismatch
}

pub trait ImportFormat {
    fn sniff(bytes: &[u8]) -> bool;
    fn parse(bytes: &[u8], limits: &ParseLimits) -> Result<SnapshotDraft, ImportError>;
}

pub trait Connector {
    fn describe(&self) -> ConnectorInfo;
    fn fetch(&self, secrets: &SecretRef, since: Option<DateYmd>)
        -> Result<Vec<SnapshotDraft>, ConnectorError>;
}
pub struct SecretRef { pub id: Id, pub store: SecretStore }   // Keychain | PlanFileSecrets
```

**Nothing writes facts directly.** Every import produces a draft; the user reviews a per-row diff (matched account, new account, changed balance, ignored row) and merges. The merge writes facts *and* one `ImportProvenance` row:

```rust
pub struct ImportProvenance {
    pub id: Id, pub imported: DateYmd, pub source: DraftSource,
    pub file_sha256: Option<String>,             // of the imported bytes, not the bytes themselves
    pub row_count: u32, pub accepted: u32, pub rejected: u32,
    pub account_map: BTreeMap<String, Id>,       // external id -> account id, remembered for next time
}
```

The same seam serves the v1 file importers (CSV, OFX, QIF at M4) and future institution connectors with no engine change; it is exercised from M4 and frozen at M10 with a sandbox connector proving the Keychain-held-secret path. Parsers are bounded (size and depth limits, **no XML entity expansion**), fuzzed with committed synthetic corpora at 1 CPU-hour per parser per release, and run **before** anything touches the fact base. Network connectors live only inside `pfp-net`; credentials resolve only through `SecretRef` and never appear in API responses, logs or the change log. No connector ships in v1, and the known constraint on the leading candidate is already recorded: SimpleFIN Bridge's protocol does **not** document investment holdings, so brokerage and employer-plan positions stay CSV/OFX ([SimpleFIN](https://beta-bridge.simplefin.org/info/developers)).

---

## 18. The first-run setup flow, expressed as fields

### 18.1 Quick Start — about sixteen fields, a baseline in about ten minutes

| # | Field | Schema path | Default |
|---|---|---|---|
| 1 | One or two people | `/persons` | one |
| 2 | Person 1 birth month | `/persons/p1/dob` | — |
| 3 | Person 2 birth month (if two) | `/persons/p2/dob` | — |
| 4 | Filing status | `/household/filingStatus` | inferred from 1 |
| 5 | Resident state | `/household/residentState` | — |
| 6 | Dependents (count + birth years) | `/household/dependents` | none |
| 7 | Person 1 annual wages | `/incomeStreams/{id}` (`Wages`, owner p1) | — |
| 8 | Person 2 annual wages | `/incomeStreams/{id}` (`Wages`, owner p2) | — |
| 9 | Current deferral % and type, per earner | `/incomeStreams/*/payroll/deferralPct`, `deferralType` | 0, Traditional |
| 10 | Employer match, in words | `/employerPlans/{id}/matchFormula` | none |
| 11 | Annual household spending (essential / total) | two `ExpenseStream` rows | — |
| 12 | Balances by tax type (cash, taxable, traditional, Roth, HSA, 529) | one `Account` per non-zero bucket | 0 |
| 13 | Debts (balance + APR + kind), repeatable | `/debts/{id}` | none |
| 14 | Target retirement age per person | `/persons/*/retirement` | 67 |
| 15 | Social Security: PIA or statement benefit per person, **or an explicit skip** | `/persons/*/socialSecurity` | skip |
| 16 | Assumption set | `/assumptionSets` | shipped default vintage |

Fields 1-14 and 16 produce the current-year ledger row, the tax picture, the KPI strip and the next-dollar waterfall — everything M2 shows. Field 15 is what makes Quick Start still sufficient from **M3**, when the multi-year ledger begins and Social Security becomes the largest guaranteed income stream in most projections: `SocialSecurityInput` is not optional on `Person`, and a flow that never asks for it cannot produce a plan M3 can run. Skipping is a first-class answer (it stores `SsSource::NotClaiming` and says on the result what that suppresses), but it is an answer the user gives, not one the flow makes for them. Field 12 deliberately collects *buckets*, not institutions: a bucket becomes a real `Account` row the user can split later, and the FI/liquidity KPIs need nothing finer.

### 18.2 Progressive sections, in order

Each section is optional, resumable, and reports its own completeness and staleness. **Milestone split (`PLAN.md` M2 Scope IN and Scope OUT, M4 Scope IN):** at M2 the flow collects, beyond Quick Start, only what the current-year waterfall reads — the employer-plan flags the tier table tests (section 6: match formula, true-up, vesting, Roth deferral, after-tax contributions, in-plan conversion, HSA-eligible coverage and tier), the debt terms the hurdle test reads (section 8) and the assumptions review (section 11); the remaining detail — the rest of section 6 (in-service withdrawals, loans, brokerage window, menu), holdings and lots and the beneficiary and inherited details of section 7, per-category spending growth and survivor treatment (section 9), and goals beyond a target amount and date (section 10) — lands at M4. Every section writes typed actions against the schema frozen at S5, so the deferral costs no rewrite and the completeness meter reports the deferred sections as unfilled in the meantime.

1. **Household** — `/household`: filing status, state, dependents, survivor policy, planning age.
2. **People** — `/persons/*`: dob, employment, retirement `MonthRef`, assumed death, mortality table.
3. **Filing status and state** — confirmation, including the `Fidelity` label the state module will carry (`Exact | NoIncomeTax | EffectiveRate`) and, for `EffectiveRate`, the entered rate.
4. **Social Security, per person** — `/persons/*/socialSecurity`: the source (`EnteredEstimate | EarningsRecord | NotClaiming`), then either the PIA or the statement's benefit-at-age with the age it assumes (§4), the claim age, the survivor claim age, and from M5 the earnings record. Placed before Wages because the claim age is a decision the ledger and every Roth comparison read from M3, and because the entry form is where the PIA-versus-benefit distinction is explained rather than assumed.
5. **Wages** — `/incomeStreams` of kind `Wages`/`SelfEmployment` with `PayrollDetail`, bonus, equity comp, group life face (the >$50,000 §79 imputed-income line) and group LTD terms; and the **year-to-date actuals** (`/ytd`, §3.1: wages and withholding per person, contributions and employer money per account, realized gains) that make year 0 a full tax year — optional, with annualization as the printed fallback.
6. **Employer plan features, per person** — `/employerPlans/*`: match formula, true-up, vesting, Roth deferral, after-tax contributions, in-plan Roth conversion, in-service withdrawals, loans, HSA-eligible coverage and tier, brokerage window, menu.
7. **Accounts by tax type** — `/accounts/*`: type, owner, balance, cost basis, beneficiary, Roth ledger, inherited details; holdings and lots optional.
8. **Debts** — `/debts/*`: kind, balance, APR and rate type, minimum-payment rule, term, deductibility class, promo terms, prepayment penalty, PSLF/IDR flags, extra payment.
9. **Spending** — `/expenseStreams/*`, essential vs discretionary by category, each with growth rule and survivor treatment.
10. **Goals** — `/goals/*`: kind, priority, target amount and date, funding accounts.
11. **Assumptions review** — the Assumptions Registry: every parameter with source, as-of date, vintage id, projection rule, rounding rule and override state, overridable with a required reason.

The flow writes typed actions, never patches: the front end posts `{action: "setWages", personId, amount}`, the server produces the RFC-6902 ops, appends `{timestamp, action, reverseDiff}` to the change log and saves atomically. Undo is built on that log. Before a passphrase is ever typed, the flow states plainly that a forgotten passphrase with no recovery code and no Keychain slot means the file is unrecoverable.

---

## 19. A synthetic example plan file

Fully synthetic: a two-earner couple with a mortgage, student debt and a mega-backdoor-capable plan — one of the three M2 personas. **A hand-written synthetic example**, marked `synthetic: true` and abridged to one row per collection; no field (birth months, wages, balances, debt terms, state) derives from any real household. It is to be **regenerated from `cargo xtask persona --seed 7` when the generator lands at M2**, and from M2 a CI check asserts that the committed example equals the generator's output for its declared seed. Money is integer cents; rates are `Ratio`.

```json
{
  "synthetic": true,
  "schemaVersion": 1,
  "planId": "pl-7k2m",
  "created": "2026-01-15",
  "asOfPolicy": { "balanceDays": 90, "incomeDays": 365, "debtDays": 180, "insuranceDays": 365 },

  "household": {
    "filingStatus": "mfj",
    "residentState": "TX",
    "dependents": { "d1": { "id": "d1", "label": "Child 1", "dob": "2021-06",
                            "relationship": "child",
                            "inHouseholdUntil": { "type": "age", "person": "p1", "years": 57, "months": 0 },
                            "asOf": "2026-01-15" } },
    "survivor": {
      "spendingMultipliers": { "housing": {"num":1,"den":1}, "dependents": {"num":1,"den":1},
                               "food": {"num":70,"den":100}, "discretionary": {"num":70,"den":100} },
      "sensitivityBounds": [ {"num":60,"den":100}, {"num":74,"den":100} ],
      "basisStepUp": "halfStepUp"
    },
    "planningAge": { "mode": "computed", "presetAge": null },
    "priorMagi": [ null, null ]
  },

  "ytd": null,

  "persons": {
    "p1": { "id": "p1", "label": "Person 1", "dob": "1988-03", "employment": "employed",
            "retirement": { "type": "age", "person": "p1", "years": 62, "months": 0 },
            "expectedSeparation": { "type": "event", "person": "p1", "event": "lastWorkingMonth" },
            "assumedDeath": null, "mortalityTableId": "nchs-2023-female",
            "socialSecurity": { "source": "enteredEstimate",
                                "claimAge": { "type": "age", "person": "p1", "years": 67, "months": 0 },
                                "survivorClaimAge": null, "earningsRecord": null,
                                "enteredEstimate": { "type": "pia",
                                  "amount": { "value": 285000, "asOf": "2026-01-10",
                                              "confidence": "estimated", "origin": "manual" } } },
            "asOf": "2026-01-15" },
    "p2": { "id": "p2", "label": "Person 2", "dob": "1990-11", "employment": "employed",
            "retirement": { "type": "age", "person": "p2", "years": 62, "months": 0 },
            "expectedSeparation": null, "assumedDeath": null,
            "socialSecurity": { "source": "enteredEstimate",
                                "claimAge": { "type": "age", "person": "p2", "years": 70, "months": 0 },
                                "enteredEstimate": { "type": "benefitAtAge",
                                  "amount": { "value": 189000, "asOf": "2026-01-10",
                                              "confidence": "stated", "origin": "manual" },
                                  "at": { "type": "age", "person": "p2", "years": 62, "months": 0 } } },
            "asOf": "2026-01-15" }
  },


  "employerPlans": {
    "ep1": { "id": "ep1", "owner": "p1", "kind": "plan401k",
             "matchFormula": [ { "employeePct": {"num":3,"den":100}, "employerPct": {"num":100,"den":100} },
                               { "employeePct": {"num":2,"den":100}, "employerPct": {"num":50,"den":100} } ],
             "matchCap": { "type": "pctOfComp", "value": {"num":4,"den":100} },
             "trueUp": true, "perPayPeriodMatch": true, "vesting": { "type": "immediate" },
             "rothDeferralAllowed": true, "afterTaxContributionsAllowed": true,
             "inPlanRothConversion": true, "inServiceWithdrawalAge": null,
             "acceptsRollIn": true, "loanAllowed": false,
             "hsaEligibleCoverage": true, "hsaCoverageTier": "family",
             "brokerageWindow": false, "asOf": "2026-01-15" }
  },

  "accounts": {
    "a2": { "id": "a2", "label": "P1 401(k)", "owner": "p1", "taxType": "traditionalEmployer",
            "employerPlanId": "ep1",
            "balance": { "value": 31500000, "asOf": "2026-01-12", "origin": "manual" },
            "beneficiary": { "spouseFraction": {"num":1,"den":1}, "remainder": "estate" },
            "asOf": "2026-01-12" },
    "a3": { "id": "a3", "label": "Joint taxable", "owner": "joint", "taxType": "taxable",
            "targetAllocationId": "ta1",
            "balance": { "value": 9800000, "asOf": "2026-01-12", "origin": "manual" },
            "costBasis": { "value": 7100000, "asOf": "2026-01-12", "origin": "manual" },
            "holdings": { "h1": { "id": "h1", "symbol": "SYNTH-TOTMKT", "label": "Total market index",
                                  "assetClass": "usLarge", "units": {"num":4900,"den":1},
                                  "unitPrice": { "value": 2000, "asOf": "2026-01-12", "origin": "manual" },
                                  "expenseRatio": {"num":3,"den":10000},
                                  "costBasisMethod": "specificId",
                                  "lots": [ { "id": "l1", "units": {"num":2900,"den":1},
                                              "basisPerUnit": 1500, "acquired": "2021-04-06",
                                              "washSaleAdjusted": false },
                                            { "id": "l2", "units": {"num":2000,"den":1},
                                              "basisPerUnit": 1375, "acquired": "2023-09-11",
                                              "washSaleAdjusted": false } ] } },
            "beneficiary": { "spouseFraction": {"num":1,"den":1}, "remainder": "estate" },
            "asOf": "2026-01-12" }
  },

  "incomeStreams": {
    "i1": { "id": "i1", "label": "P1 salary", "owner": "p1", "kind": "wages",
            "amount": { "value": 18500000, "asOf": "2026-01-05", "origin": "manual" },
            "period": "annual", "basis": "nominal", "growth": { "type": "index", "assumptionKey": "wageGrowth" },
            "start": { "type": "calendar", "month": "2026-01" },
            "end": { "type": "event", "person": "p1", "event": "lastWorkingMonth" },
            "survivorFraction": {"num":0,"den":1}, "taxCharacter": "ordinaryEarned",
            "payroll": { "employerPlanId": "ep1", "deferralPct": {"num":10,"den":100},
                         "deferralType": "traditional", "afterTaxPct": {"num":0,"den":100},
                         "hsaPayrollDeduction": 875000, "groupLifeFace": 37000000 },
            "asOf": "2026-01-05" }
  },

  "expenseStreams": {
    "e1": { "id": "e1", "label": "Core household spending", "owner": "joint", "category": "food",
            "essential": true,
            "amount": { "value": 7200000, "asOf": "2026-01-15", "origin": "manual" },
            "period": "annual", "basis": "real", "growth": { "type": "inflation" },
            "start": { "type": "calendar", "month": "2026-01" }, "end": { "type": "never" },
            "survivorTreatment": "multiplier", "asOf": "2026-01-15" }
  },

  "debts": {
    "dbt1": { "id": "dbt1", "label": "Mortgage", "owner": "joint", "kind": "mortgage",
              "balance": { "value": 38500000, "asOf": "2026-01-12", "origin": "manual" },
              "apr": { "value": {"num":575,"den":10000}, "asOf": "2026-01-12", "origin": "manual" },
              "rateType": { "type": "fixed" },
              "minimumPayment": { "type": "amortizing", "payment": 241800 },
              "origination": "2021-05-01", "termMonths": 360,
              "interestDeductibility": "mortgageInterest",
              "extraPayment": null, "asOf": "2026-01-12" },
    "dbt2": { "id": "dbt2", "label": "Student loan", "owner": "p2", "kind": "student",
              "balance": { "value": 2840000, "asOf": "2026-01-12", "origin": "manual" },
              "apr": { "value": {"num":600,"den":10000}, "asOf": "2026-01-12", "origin": "manual" },
              "rateType": { "type": "fixed" },
              "minimumPayment": { "type": "amortizing", "payment": 31500 },
              "interestDeductibility": "studentLoanInterest",
              "student": { "federal": true, "pslfTrack": false, "idrPlan": null },
              "asOf": "2026-01-12" }
  },

  "goals": {
    "g1": { "id": "g1", "label": "Retirement", "owner": "joint", "kind": "retirement",
            "priority": "need", "basis": "real",
            "targetDate": { "type": "event", "person": "p1", "event": "retirement" },
            "asOf": "2026-01-15" }
  },

  "events": {}, "properties": {}, "insurance": {},

  "targetAllocations": {
    "ta1": { "id": "ta1", "label": "Household 80/20", "asOf": "2026-01-15",
             "weights": { "usLarge": 4800, "usSmall": 800, "intlDev": 1900, "emergingMkts": 500,
                          "usAggBond": 1800, "cash": 200 },
             "glidePath": { "type": "linearToAge",
                            "endWeights": { "usLarge": 3000, "usSmall": 400, "intlDev": 1200,
                                            "emergingMkts": 400, "usAggBond": 4500, "cash": 500 },
                            "from": { "type": "age", "person": "p1", "years": 52, "months": 0 },
                            "to":   { "type": "event", "person": "p1", "event": "retirement" } } }
  },

  "policies": {
    "contribution": {
      "id": "cp-default", "preset": "default-tiers",
      "presetVintageId": "default-tiers@4b81…", "stepCents": 10000,
      "rules": [
        { "id": "r-1a", "tierLabel": "1a", "label": "Spending-shock cash buffer", "option": "cashBuffer",
          "test": { "type": "cashBelow", "months": {"num":1,"den":2} },
          "capacity": { "type": "amount" }, "rU": { "formula": "cashVehicleYield" },
          "liquidityClass": "safeLiquid", "enabled": true },
        { "id": "r-2", "tierLabel": "2", "label": "Employer match, to the full match", "option": "employerMatch",
          "test": { "type": "matchUnfilled" },
          "capacity": { "type": "matchGap" }, "rU": { "formula": "matchRateTimesVesting" },
          "liquidityClass": "illiquidPre59Half", "enabled": true }
      ],
      "thresholds": {
        "highInterestHurdle": { "value": {"num":8,"den":100}, "source": "params:decide.hurdle_high@2026" } }
    },
    "withdrawal": { "order": ["cash","taxable","traditional","roth","hsa"], "rmdFirst": true,
                    "spendingRule": { "type": "constantReal" },
                    "earlyAccess": [],
                    "inheritedSpousalElection": "treatAsOwn" },
    "rebalancing": { "lookFrequency": "monthly",
                     "band": { "absolutePp": {"num":5,"den":100}, "relativePct": {"num":25,"den":100},
                               "combine": "smaller" },
                     "destination": "halfway", "scope": "breachingOnly", "locationPriority": [] },
    "rothVerdict": { "minSurvivorYears": 5 },
    "gainsBudget": null
  },

  "assumptionSets": { "as1": { "id": "as1", "vintageId": "user-entry-2026@c4d1…", "overrides": {} } },
  "paramOverrides": {},

  "scenarios": {
    "sc-base": { "id": "sc-base", "name": "Current Course", "parentId": null, "kind": "baseline",
                 "assumptionSetId": "as1", "paramVintageIds": ["2026-federal@9f2c…"],
                 "seed": "7f3a1c08…",
                 "ops": [], "created": "2026-01-15" },
    "sc-prop": { "id": "sc-prop", "name": "Proposed Plan", "parentId": "sc-base", "kind": "proposed",
                 "assumptionSetId": "as1", "paramVintageIds": ["2026-federal@9f2c…"],
                 "seed": "7f3a1c08…",
                 "ops": [ { "op": "replace", "path": "/incomeStreams/i1/payroll/afterTaxPct",
                            "value": {"num":12,"den":100} } ],
                 "created": "2026-02-02" }
  },

  "factSnapshots": {}, "resultSnapshots": {},
  "recommendations": {}, "actions": {}, "reviews": {}, "importProvenance": {}
}
```

Every figure above is invented, and the two `seed` values are truncated for display (a stored seed is 64 lowercase hex characters, §2.1), as the vintage hashes are. The file exercises owner tagging (`p1`, `p2`, `joint` all present), a `Ratio`-typed match formula and a `MatchCap`, the mega-backdoor preconditions, a specific-id lot, each `MonthRef` kind, both `SsEstimateEntry` forms (a PIA for one person, a statement benefit-at-age for the other), a shared `TargetAllocation` with a glide path, an expanded contribution rule list, a baseline with empty `ops`, and a proposed scenario whose single op targets an allowlisted decision path — `/incomeStreams/i1/payroll/afterTaxPct`, the mega-backdoor adopt path added to §12.1. A test resolves this file against the shipped allowlist and validator — it exercises exactly the two failures a naive example would hit, an unlisted patch path and an empty `rules` list that allocates nothing. The `assumptionSets` entry is a complete `AssumptionSetRef` (§15) with an empty override layer, and the `withdrawal.order` array is the conventional `WithdrawalTier` sequence of §11; the vintage the reference names carries the `classes`, `market_data` and `discount` blocks §15 requires.

---

## 20. Deviations from the spine

1. **`importProvenance` is a `Record<id, item>`, not an array.** Facts reference import rows by `Origin::Import{provenance_id}`, and an array index would break the moment a row is removed. `ARCHITECTURE.md` §6 and `PLAN.md` seam S5 carry `importProvenance{}`, together with `events{}`, `targetAllocations{}`, `ytd` and the five-member `policies{}`, so the seam has one enumeration.
2. **Snapshot bodies live in container sections; the document holds index entries.** Seam S5 lists `factSnapshots{}`/`resultSnapshots{}` as document collections; ADR-005 lists `snapshots`/`results` as container sections. Both are specified here, split by role, so `PLAN.md` R17 holds. No spine statement is contradicted, but the split is a choice and is stated so it is not re-litigated.
3. **`Lot` is a `Vec`, not a `Record`** — the one deliberate exception to R2, because no scenario path addresses a lot and imports replace lot sets wholesale. Lots still carry ids for trade-list references.
4. **`explorer` (schema v0) is given a concrete shape here.** `PLAN.md` M1 names it without defining it; §14 defines it and `migrate_1` so the first migration is buildable.
5. **`FilingStatus` wire forms are the short ones** (`"single" | "mfj" | "mfs" | "hoh" | "qss"`), not long spellings. The engines and their fixtures are written against the short forms; one spelling has to win and the consuming side wins. The long names survive as UI labels. Similarly `Year` is `i32`, not `u16`, and `MonthYm` is the single definition with `Month` as its alias (§2.1).
6. **The demo plan is plaintext JSON, not a `.pfplan`.** A committed `plan.example.pfplan` would be rejected by the security controls in `SECURITY.md` §13.1/§13.2/§13.4 by construction, and must keep being rejected without an allowlist. The fixture is `fixtures/plans/demo.plan.json`; the binary encrypts it at first use. `ARCHITECTURE.md` §9 item 2 embeds the JSON.
7. **Seam S1's parameter-table shape is extended before it freezes** (§15: `index_series`, `base_year`, `base_values`). The frozen shape could not express the statutory uprating rule its own M0 acceptance test demands. Extending a seam before the freeze is cheap; the alternative is an engine bent to pass a gate it cannot pass correctly.

No other deviation. Where a third-party default differs from the spine — notably a flat 0.60 survivor spending fraction — the spine's per-category multipliers are encoded and the third-party value kept as a cited sensitivity bound.

---

## 21. Open questions (carried, not invented)

- **Change-log retention cap.** ADR-009 leaves the default open. TPAW's 100-entry cap with one entry per day retained is a citable reference point, not this project's choice; a compaction command exists either way. The decision is **no longer only about plan-file growth**: because `reverseDiff` payloads and snapshot bodies retain every superseded fact, retention sets the blast radius of the plan file, and data minimisation is a stated input to it alongside size (§13, and ADR-009's open decision #4 re-decided on those terms). The purge operation exists regardless of where the default lands; what is open is whether the default is unbounded retention with purge available, or a bounded window with purge as the escape hatch.
- **Statutory conditions for `EarlyAccessRule`.** The separation-at-55 and 72(t) SEPP rules are declared as opt-in shapes in §11 and are **not covered by any archived source**; `ENGINE-SPEC.md` §2.4 owns their conditions, and no engine honours either field until they are verified against statute at the hand-verification gate.
- **Whether to model `Qss` (qualifying surviving spouse) at all.** It needs a dependent child and an unmarried survivor; Owl skips it and collapses MFJ → Single. The schema supports it; whether the branch earns its keep is decided by usage.
- **Inherited traditional IRA election** (treat-as-own vs remain-beneficiary), which matters when the survivor is under 59½ or the decedent was younger. The field exists, `TreatAsOwn` is the default, the guidance is unsettled.
- **Basis step-up default** (half vs full) depends on state community-property law; `HalfStepUp` is the conservative default.
- **Values marked unverified**, to be hand-verified before any vintage carrying them is locked: the 402(g)/219(b)(5)/414(v) rounding increments and IRS projection-rounding conventions; the SALT phase-down schedule; the 2026 SSA and CMS constants; the mega-backdoor $47,500 derivation; Roth catch-up regulation dates; the funded-ratio formula and its 1.0-1.2 comfort band; typical group-LTD design. The 400%-FPL cliff of $84,600 for two people in 2026 is **computed** from confirmed poverty guidelines, not a fetched published figure.
