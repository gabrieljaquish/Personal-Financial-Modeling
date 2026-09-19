# M0 design errata: proposed text for the documents seam S1 freezes against

**Status: proposals, pending a human decision. Nothing here is normative until a
maintainer copies it into the design document it names.**

The M0 engine slice froze nothing on its own authority. Where the first
parameter vintage, the fixtures or the engine could not be written in the shape
the design documents describe, the departure was declared where it occurs
(`params/vintages/federal-2026/VINTAGE.md`, the header of
`crates/pfp-params/src/table.rs`, `fixtures/pending/README.md`) and listed as a
blocking decision in `m0-hand-verification.md` §0. The design documents
themselves are not edited by an AI-assisted session (`docs/contributing.md` §3);
an erratum to them is a maintainer's change.

Review found that listing the decisions is not enough: seam S1 freezes with this
pull request, so the definition sites have to be amended **in the change that
freezes it** or they are permanently wrong. This file therefore gives each
erratum as text that can be pasted, so that accepting one is a mechanical edit
and rejecting one is an explicit choice. Each entry states the sites, the
decision, the proposed wording, which gate follows, and what the erratum
deliberately does **not** settle.

No erratum below changes a parameter value, a fixture's input or expected value,
an archived document or a rounding result. Every one renames, adds or documents
a field, or moves a sentence.

| Id | Blocks | Sites | Checklist item |
|---|---|---|---|
| E1 | S1 freeze | `DOMAIN-MODEL.md` §15, `ENGINE-SPEC.md` §1.3, `TESTING.md` §3.1, §11.2 gate 9 | B12 (with B3, B4, B7) |
| E2 | S1 freeze | `DOMAIN-MODEL.md` §15, `ENGINE-SPEC.md` §1.1 | B12(a) |
| E3 | S1 freeze (part 1); S3 freeze at M1 (part 2) | `DOMAIN-MODEL.md` §15, `ARCHITECTURE.md` D3, `ENGINE-SPEC.md` §3.2 | B8 |
| E4 | S1 freeze, the lock, `paramVintage` | `DOMAIN-MODEL.md` §15, `DECISIONS.md` ADR-010 | B9 |
| E5 | fixture promotion | `TESTING.md` §2.2 | B13 |
| E6 | fixture promotion (critical path: seven-day cooling-off) | `TESTING.md` §2.2, §3.1 | B16 |
| E7 | M0 acceptance wording | `PLAN.md` §4.1, `ENGINE-SPEC.md` §1.3 | B17 |

---

## E1 - the parameter-table shape S1 freezes

**Sites.** `DOMAIN-MODEL.md` §15 (the definition site), `ENGINE-SPEC.md` §1.3,
`TESTING.md` §3.1 (last paragraph) and §11.2 gate 9.

**What the documents say.** `projection{rule, index, index_series, base_year,
base_values, rounding}`, `base_year` a scalar, `base_values` "broken down per
filing status exactly as `values` is", `rounding.increment` a scalar.

**Why the first vintage cannot be written that way** (each reason is statutory
and is quoted in `VINTAGE.md`): a rate schedule has six thresholds per filing
status; the bracket increment is 25 dollars for `single` and `mfs` and 50
otherwise (26 USC 1(f)(7)(B), 1(j)(3)(B)(ii)); from tax year 2026 one table has
two base years and in tax year 2025 it has one (1(j)(3)(B)(i) as amended by
Public Law 119-21 §70101(b)); the standard-deduction adjustment first applies
to a taxable year beginning after 2025 (63(c)(7)(B)(ii)); the adjustment reads
the index of the *preceding* calendar year (1(f)(3)(A)(i)); rates are ratios and
are never indexed.

**The decision.** Adopt the shape below (what `pfp-params` reads today, scalar
forms included), or choose another and have the loader and the two tables
follow. The loader accepts **both** the documented scalar form and each
extension and rejects a table that states one fact both ways, so adopting the
text below changes no file.

**Proposed text for `DOMAIN-MODEL.md` §15**, to follow the worked TOML (which
stays valid as the single-amount case):

> **A table is either single-amount or edged.** A single-amount table (the
> standard deduction) has one integer per breakdown key per year, as above. An
> *edged* table (a rate schedule) declares `edges = ["top_of_10", ...]`, an
> ordered list of threshold names, and then every `values.<key>.<year>` and
> every `projection.base_values.<key>` is an array with exactly one integer per
> edge, in that order. `base_values` is broken down exactly as `values` is - by
> key, and by edge where the table is edged.
>
> **Fields of `[projection]`.** `rule` (`index | wage | flat | zero |
> schedule`) is required on every table. When `rule = "index"` the block also
> requires `index`, `index_series`, `lag_years`, `base_values`, `rounding`, and
> exactly one of `base_year` and `base_year_by_edge`:
>
> | Field | Type | Meaning |
> |---|---|---|
> | `index_series` | string | the name an archived index-series table declares (below); a vintage whose `index_series` resolves to no archived series fails gate 9 |
> | `lag_years` | integer, 0..=10 | the adjustment for tax year `Y` reads the series value for calendar year `Y - lag_years`. 26 USC 1(f)(3)(A)(i) ("the preceding calendar year") gives 1. Data, never a constant in code |
> | `base_year` | year | the one statutory base year of the table |
> | `base_year_by_edge.<tax year>.<edge>` | year | edged tables only, instead of `base_year`, where the statute gives different thresholds different base years or changes them from a given tax year. The entry for the greatest `<tax year>` not after the year being projected applies; every edge is listed in every entry |
> | `first_adjusted_year` | year, optional | the first tax year the adjustment applies to; for earlier years the base amounts apply unadjusted. Absent means every year after the base year is adjusted |
> | `base_values.<key>` | integer, or array per edge | statutory base-year amounts in the table's `unit` |
> | `rounding.increment` | integer | one increment for every key (unit: see E2) |
> | `rounding.increment_by_key.<key>` | integer | instead of `increment`, where the statute gives breakdown keys different increments; every key of `values` is listed |
>
> Stating one fact both ways (`base_year` and `base_year_by_edge`, or
> `increment` and `increment_by_key`) is a load error.
>
> **`[rates]`.** An edged rate schedule carries its never-indexed rate ladder
> beside its edges: `[rates]` with `unit = "ratio"`, `rule = "flat"` and, per
> year, an array of decimal strings with one more entry than `edges`. This is
> the only place a sub-table declares its own `unit`; the table's `unit` and its
> one `RoundingRule` govern the edges, and no rule rounds a rate. The rates are
> kept in the same table because a separate rates table would need a
> `RoundingRule` that no statute supplies.
>
> **Verification status.** Every file under `params/` carries
> `verification = "pending-hand-verification" | "primary-source-confirmed"` (the
> `TESTING.md` §2.2 spellings; the other two §2.2 values are not admissible for
> a parameter table) and, while pending, `[hand_verification] open = [ ... ]`,
> the list of readings a person must confirm. A vintage cannot lock while any of
> its tables or the series they read is pending or has a non-empty `open` list.
>
> **Archived index series.** A series a projection reads is archived in the
> repository as its own table under `params/index-series/`, never fetched. It
> declares `kind = "index-series"` and has **no `[projection]` block and no
> `RoundingRule`** - the no-projection-rule error applies to parameter tables
> (no `kind`), not to series. Fields: `id`, `kind`, `index_series` (the name
> tables resolve against), `series_id` (the publisher's id), `unit = "index"`,
> `base_period`, `periodicity`, `as_of` (the snapshot date of a revisable
> series), `verification`, `statutory_name`, `[window]` (`kind =
> "trailing-12-month-mean"`, `months`, `ends_month`, and the statutory source of
> the window), `[observations.<year>]` (one decimal **string** per month of that
> year's window, exactly as published), optional `[window_sum]` (a declared sum
> per year, which the loader recomputes and compares), and `[[source]]`. The
> projection ratio is the ratio of two window sums, so no mean is ever rounded.

**Proposed replacement in `ENGINE-SPEC.md` §1.3** for the clause "Each indexed
table carries `projection{...}`":

> Each indexed table carries `projection{rule, index, index_series, lag_years,
> base_year | base_year_by_edge, first_adjusted_year?, base_values[status] (one
> amount, or one per edge), rounding{increment | increment_by_key, direction,
> basis}}` (`DOMAIN-MODEL.md` §15 defines each field): the uprated amount is
> `base_value x (index_(t - lag_years) / index_(base year of that threshold))`
> ...

(the rest of the paragraph unchanged).

**Proposed replacement in `TESTING.md` §3.1**, last paragraph, for the sentence
naming the shape: the same field list, with "`base_values` broken down per
filing status, and per edge in an edged table, exactly as `values` is".

**Proposed replacement for `TESTING.md` §11.2 gate 9:**

> 9. A new parameter table has a projection rule and a `RoundingRule` (a table
> without a projection rule is a CI error), a primary source, an as-of date, a
> `verification` status and an archived checksum - and, where the rule is
> `index`, `index`, `lag_years`, per-breakdown `base_values`, exactly one of
> `base_year` and `base_year_by_edge`, exactly one of `rounding.increment` and
> `rounding.increment_by_key`, and a named `index_series` that **resolves to an
> archived `kind = "index-series"` table** rather than being fetched (§3.1). An
> index-series table is exempt from the projection-rule requirement and carries
> none. Every `values`, `base_values` and `increment_by_key` key is a wire form
> of the table's declared breakdown enum ... (unchanged to the end).

**The gate follows automatically.** `cargo xtask data-hygiene` rule 8 drives
the `pfp-params` loader over every table and series, so it checks whatever
field names the loader reads; its unit test removes each gate-9 field in turn
and expects a failure. If the decision renames a field, the rename is made in
`crates/pfp-params/src/table.rs` and the gate follows with it.

**Not settled by this erratum, deliberately.**

- **B4 stays open.** That the tops of the 10 and 12 percent brackets take 2016
  and the other four edges 2017 from tax year 2026 is a *reading* of the words
  Public Law 119-21 §70101(b) inserted into 1(j)(3)(B)(i), not a quotation. E1
  adopts a field that can hold the reading; it does not adopt the reading. It
  remains on the `TESTING.md` §5.2 hand-verification list (checklist item
  `P-BY`), to be settled by reading the statute and **not** by observing which
  base year reproduces a published threshold.
- **B3** (`projection.derived`, the `mfj`/`qss` standard deduction as 200
  percent of the rounded `single` amount) is proposed by the loader and present
  in no shipped file. If adopted it needs its own row in the field table and the
  parameter file's owner adds it to `std_deduction.toml`.
- **B2** (a `basis` value for "round the total") and **B1** (the index vintage)
  are value questions and are untouched.

---

## E2 - the unit of `RoundingRule.increment` in a parameter file

**Sites.** `DOMAIN-MODEL.md` §15 (worked TOML and the field table of E1),
`ENGINE-SPEC.md` §1.1.

**The ambiguity.** `ENGINE-SPEC.md` §1.1 declares
`RoundingRule { increment: Cents, ... }`; §15's worked TOML writes
`increment = 50` inside a `unit = "USD"` table. Both are right, about two
different things, and neither says so. A table author reading §15 next to §1.1
could write `5000`, or - reading §1.1 alone - believe `50` means 50 cents; the
second silently rounds to half a dollar.

**What is implemented.** The in-memory `pfp_money::RoundingRule.increment` is
`Cents`. A parameter file writes the increment in the table's declared `unit`,
as the statute prints it (`unit = "USD"`: whole dollars), and the loader
converts (`50` becomes `Cents(5000)`); the conversion is tested.

**Proposed text for `DOMAIN-MODEL.md` §15**, as a comment on the worked TOML
line and as a sentence under it:

> `increment = 50   # in the table's unit: 50 DOLLARS here, as 26 USC
> 63(c)(7)(B)(ii) prints it`
>
> `rounding.increment` and `rounding.increment_by_key` are written **in the
> table's declared `unit`**, exactly as the statute prints them. The loader
> converts to the `Cents` of `RoundingRule.increment` (`ENGINE-SPEC.md` §1.1);
> no parameter file ever writes cents. A rule finer than the table's unit (the
> dime of the PIA) belongs to a table whose unit can express it as an integer,
> which is a question for the milestone that ships it (`rounding/table.json`,
> open question `increment-unit`).

**Proposed cross-reference in `ENGINE-SPEC.md` §1.1**, after the code block:
"`RoundingRule.increment` is `Cents` in memory; a parameter file states it in
the table's unit and the loader converts (`DOMAIN-MODEL.md` §15)."

---

## E3 - rounding rules that govern a computation, and the per-product rule

**Sites.** `DOMAIN-MODEL.md` §15 and `ARCHITECTURE.md` D3 (part 1);
`ENGINE-SPEC.md` §3.2 (part 2).

### Part 1 - where a named rule that governs a computation lives

D3 and §15 say a `RoundingRule` is "stored as data beside the parameter it
governs, never a global mode". `irs.whole_dollar` governs no parameter cell: it
governs the *sum* of the rate schedule. `money.cent_half_even` governs each
`rate x amount` product. Both are `const` values in
`crates/pfp-tax/src/schedule.rs`, because the vintage has no place for them and
no document says where such a rule lives.

Two branches. The implementer's recommendation is (a); either is small.

- **(a) In engine code, and the design says so.** Proposed sentence for §15,
  after "never a global mode": "A rule that governs a *computation* rather than
  a parameter cell - the whole-dollar rounding of a return line, the cent
  convention of a product - is a convention of the worksheet, not a fact of a
  vintage: it is a named constant (`RuleId`) in the engine crate that owns the
  worksheet, recorded on every `Line` it rounds (`rounded_by`), listed in
  `ENGINE-SPEC.md` beside the line it governs, and changed only with the
  engine's version. It is still never a global mode: each call names its rule."
  This keeps a vintage a statement of law and keeps the wasm engine free of a
  lookup that can fail. Cost: a whole-dollar convention that changes with a tax
  year could not be expressed as data; none is known.
- **(b) A named-rule table in the vintage** (`rules.toml`:
  `[rule."irs.whole_dollar"] increment, direction, basis`, plus `[[source]]`),
  resolved by `ParamView::rule(id)`. Cost: a new table kind under S1, a new
  failure mode in `schedule_tax`, and a source citation for a convention the IRS
  states in form instructions rather than in the archived documents.

### Part 2 - the rule for each product (before seam S3 freezes at M1)

§3.2 says "each product via `mul_ratio`, summed in cents, rounded once to the
whole dollar" and names no rule for the product, but `mul_ratio` cannot return
without rounding (D3). So the schedule has two rounding layers: each product to
the cent (half-even, as implemented), then the sum to the dollar (half-up).

**Harmless for every M0 fixture, and asserted so.** Taxable income is whole
dollars (a multiple of 100 cents) and every rate is a whole percent, so every
product is an exact integer number of cents and the cent layer is the identity.
`crates/pfp-tax/tests/properties.rs` checks the engine against an independent
exact-rational evaluation rounded once, over the whole income range.

**Not harmless in general.** With an income carrying cents, or a rate that is
not a whole percent, an exact sum of `x.4999...` dollars can have its products
rounded up to `x.50` and then be rounded up again, one dollar above the
single-rounding answer.

Branches for `ENGINE-SPEC.md` §3.2:

- **(a) Keep the cent layer and name it:** "each product via `mul_ratio` under
  `money.cent_half_even` (half-even at the cent, the D5 convention), summed in
  cents, and the sum rounded once to the whole dollar under `irs.whole_dollar`.
  `schedule` is defined on whole-dollar taxable income, which with whole-percent
  rates makes every product exact; an input carrying cents is rounded to the
  whole dollar under `irs.whole_dollar` before the schedule is entered." The
  second sentence is what makes two layers safe, and matches the return, where
  taxable income is a whole-dollar line.
- **(b) Sum exactly, round once:** the products are summed as exact rationals
  (`i128` numerator over the common denominator of the rates) and rounded once
  under `irs.whole_dollar`. Per-bracket `Line` values then need a display rule
  of their own, because they would no longer sum to the total by construction.

Neither branch changes an M0 fixture value.

---

## E4 - what "contents" means in `<name>@<sha256-of-contents>`

**Sites.** `DOMAIN-MODEL.md` §15 ("Vintages are immutable"), `DECISIONS.md`
ADR-010. The id is inside the text `PLAN.md` §3 freezes for seam S1.

**The gap.** No document defines "contents" for a vintage of several files (two
tables, a Markdown manifest, an index series stored outside the vintage
directory). Until one does, no `params/VINTAGES.lock` id can be recorded and
every fixture carries `paramVintage: null`.

**Proposed paragraph for `DOMAIN-MODEL.md` §15**, replacing the first sentence
of "Vintages are immutable" (ADR-010 to cite it):

> **Vintages are immutable.** `vintageId = "<name>@<hex sha256>"`, where the
> hash is taken over the vintage's **documents**: every parameter table in
> `params/vintages/<name>/` (`kind` `table`) and every archived index series one
> of them names in `index_series` (`kind` `index-series`). The hashed stream is
> the ASCII line `pfp-vintage-id/v1`, a newline, `<name>`, a newline, and then,
> for each document in ascending byte order of its `id`, the header line
> `<kind> <id> <byte length>` and a newline followed by the document's bytes
> exactly as stored - no canonicalisation, so a changed comment changes the id.
> The header carries the length so that no two different sets of documents
> produce the same stream. `VINTAGE.md` and other prose the engine never loads
> are outside the id and are pinned per file, with the tables and series, by
> `params/VINTAGES.lock`, which CI checks.

**What exists.** `pfp_params::Vintage::content_id` computes exactly this;
`Vintage::locked_as(id)` fails with `VintageIdMismatch` unless a human-recorded
id matches; `Vintage::id()` is `None` while unlocked; `cargo xtask data-hygiene`
prints the current value. Computing it claims nothing. Replacing the definition
changes the id, which is harmless before a lock exists.

**Consequences to record with the decision.** An index series shared by two
vintages is hashed into both, so re-snapshotting a revisable series is a new
vintage of every table that reads it - which is the point of 26 USC 1(f)(6)(A)
(B1). The `v1` tag lets the definition change later without ambiguity.

---

## E5 - a pending area in `TESTING.md` §2.2

**Site.** `TESTING.md` §2.2.

**The gap.** §2.2's loader rejects `pending-hand-verification` at tier 1, and
`docs/contributing.md` §3 forbids an AI-assisted session writing under
`fixtures/tier1/`. So fixtures authored before a person has read the source need
a place, and §2.2's layout has none. `fixtures/pending/` is that place; its
envelope differs from §2.2's in ways §2.2 should either admit or forbid.

**Proposed addition to the §2.2 layout block:**

```
  pending/          fixtures authored from a primary source and awaiting the §5.2 human gate; same
                    sub-directories as tier1/; ids `pending/<dir>/<name>`. Never counted as tier 1.
```

**Proposed rule, after the loader rules:**

> **The pending area.** A fixture under `fixtures/pending/` carries the tier-1
> envelope with these differences, and the loader enforces them:
> `"tier": "pending"` (a string; the integer tiers are 1, 2 and 3) with
> `"targetTier": 1`; `"promotesTo"`, the unique tier-1 id it will take;
> `"verification": "pending-hand-verification"` and nothing else;
> `"verificationNote"`, prose saying what a person must do; `"paramVintage":
> null` until the vintage it reads is locked. Optional keys:
> `"additionalSources"` (further `source` blocks, each complete),
> `"residuals"` (recorded differences between a computed and a published value -
> a finding, never a tolerance, and never asserted as an expected value), and
> `"openQuestions"` (cases with **no** expected value, awaiting a design
> decision; a fixture cannot promote while it has any). Promotion sets `id`
> from `promotesTo`, `tier` to `1`, a tier-1 `verification` value and the
> locked `paramVintage`, and removes `promotesTo` and `targetTier`; it changes
> nothing under `inputs` or `expect`. The validation report counts pending
> fixtures in its `unverified` block (§13), never beside tier 1.
>
> **Grids.** A fixture that checks one computation at many inputs wraps its
> cases as `expect.cases[]`, each with its own `inputs` and `lines[]`. The
> `LineId` rule applies at any depth: **every `id` or `lineId` under `expect`**
> must exist in the engine's `LineId` registry, so a renamed worksheet line
> breaks the build however the fixture nests it.

**Decide also:** one spelling of the line-id key. The pending grids write
`lineId` where §2.2's example writes `id`. Admitting both (as above) costs
nothing; choosing `id` is a fixture-author change to eleven files at promotion.

**What enforces the substance today.** There is no fixture loader crate at M0.
The `pfp-tax` fixture driver walks every `lineId` at any nesting depth, requires
each to exist in the worksheet and compares each with the engine's, so a renamed
line does fail the build - by a test, not yet by a loader. When the loader lands
the walk moves into it.

---

## E6 - `synthetic` means two things, and no pending fixture can take a tier-1 `verification` value as shaped

**Sites.** `TESTING.md` §2.2 (the two admissible tier-1 values), §3.1 (the
`chained-path-divergence` row).

**The collision.** §2.2 admits `hand-worked-reviewed` at tier 1 "**only** with
`synthetic: true`". `docs/contributing.md` §1.3 uses `synthetic` to mean *the
inputs describe no real person or household*. §2.2 therefore also uses it to
mean *computed by this project rather than transcribed*. The two come apart for
exactly the M0 uprating fixtures:

| Fixture | Inputs | Expected values | `synthetic` today | Value it must take |
|---|---|---|---|---|
| 11 schedule grids, `rounding/table`, `money/mul_ratio` | synthetic incomes and amounts (real published thresholds and rates as parameters) | computed by this project | `true` | `hand-worked-reviewed` - admissible as §2.2 stands, once `derivation` and `reviewedBy` are added |
| `uprating/2026-brackets`, `uprating/2026-stdded` | real statutory base amounts and the archived index | computed (`computedDollars`, `flooredIncreaseDollars`, `colaExact`) **and** transcribed (`publishedDollars`), with `residualDollars` between them | `false` | neither value fits as §2.2 stands |
| `uprating/chained-path-divergence` | the same | computed; no publication prints a chained value | `false` | §3.1 assigns it `hand-worked-reviewed` **in terms**, which §2.2 forbids without `synthetic: true` |
| `uprating/2025-brackets-control`, `published/*` | - | not promotable (no tier-1 id) | `false` | none needed |

Marking the uprating fixtures `synthetic: true` to get through the loader would
be a false statement about inputs that are public law. The fault is in §2.2.

**Proposed replacement for the `hand-worked-reviewed` bullet of §2.2:**

> - **`hand-worked-reviewed`** - the expected values are *computed by this
>   project* from statutory inputs or from the engine's own stated conventions,
>   where no publication prints the result. Admissible at tier 1 **only** with a
>   `derivation` string stating the arithmetic in full, a `source` block citing
>   the inputs the derivation consumes, and a recorded `reviewedBy` sign-off
>   under the review rule above. `synthetic` is independent of this value and
>   keeps the one meaning `docs/contributing.md` §1.3 gives it - *the inputs
>   describe no real person or household*: it is `true` for a hand-worked
>   household, ledger or income grid, and `false` where every input is public
>   law or a public statistic (a statutory base amount uprated by an archived
>   index). `synthetic: true` **or** a complete `source` block remains
>   mandatory; a `hand-worked-reviewed` fixture with `synthetic: false` must
>   have the complete `source` block.
> - A fixture that carries both computed and transcribed values (a statutory
>   pipeline set beside the published table) is `hand-worked-reviewed`: the
>   stricter value governs, its transcribed column is read back under §5.2 as
>   any transcription is, and the publisher-host rule applies to its `source`.

and, in the sentence introducing the second JSON example, replace "carries the
same envelope with `"synthetic": true` and two additional required fields" by
"carries the same envelope with two additional required fields"; in §14 item 9
replace "demands `synthetic: true`, a full derivation" by "demands a full
derivation".

**The `derivation` field - what exists and what is missing.** No fixture has a
top-level `derivation`. The arithmetic is nevertheless in the files: every case
of `rounding/table.json` and `money/mul_ratio.json` carries its own
`derivation` string, and every schedule grid carries `inputs.formula` plus the
per-bracket intermediates of every case. What §2.2 asks for is one top-level
string whose sha256 the sign-off records.

Adding it is a fixture edit. Fixtures are not the engine implementer's files
(`docs/contributing.md` §3: no session authors both a constant and its test;
the record of the one earlier exception is in `m0-hand-verification.md` §1.1),
so the strings below are **drafts for the fixtures' author or the maintainer to
place**, composed only from text already in each fixture. They add no number.

- *Schedule grids (11 files)*, top-level `derivation`: "For each case:
  tax = sum over brackets b of rate_b x max(0, min(x, top_b) - bottom_b), with
  bottom_0 = 0, bottom_b = top_(b-1), the last bracket unbounded, x the case's
  taxableIncomeCents, tops and rates those in `inputs`. Each product is exact in
  integer cents (x is a multiple of 100 cents and each rate a whole percent) and
  is the line's taxFromBracketCents; the sum of the seven products is rounded
  once, half-up, to the whole dollar. Each case's `lines` give amountTaxedCents
  and taxFromBracketCents per bracket; re-work them from `inputs` alone."
- *`rounding/table.json`*, top-level `derivation`: "Each case applies its rule
  `{incrementCents, direction, basis}` once. basis Amount: round the amount to a
  multiple of the increment in the stated direction. basis IncreaseOverBase:
  increase = base x (factor - 1) as an exact fraction; round the increase; add
  the base. down is toward negative infinity. The arithmetic of each case is
  written out in that case's own `derivation`; the cases under `openQuestions`
  have no expected value."
- *`money/mul_ratio.json`*, top-level `derivation`: "Each case computes
  cents x num / den exactly in a 128-bit intermediate and rounds once under the
  case's rule; the longhand product, quotient and remainder are in that case's
  own `derivation`."

**Why this is on the critical path.** With a single maintainer the admissible
review is `cooling-off-re-review`: a blind re-derivation **no sooner than seven
days after the original derivation**, whose hash must equal the hash signed
off. The clock runs from a derivation text that does not yet exist at top
level. Placing the text (and deciding E6) this week rather than at promotion
saves a week at promotion. The AI assistant is never the reviewing party.

---

## E7 - projection "under an editable inflation assumption" and the engine slice

**Sites.** `PLAN.md` §4.1 ("What a user can do now"), `ENGINE-SPEC.md` §1.3.

**The gap.** §4.1 promises the M0 user can "see how thresholds are projected
forward under an editable inflation assumption". §1.3 names the entry point,
`ParamView::for_year(t, &inflation_index)`, and the knob,
`assumptions.chained_cpi_wedge`. Neither exists. `pfp_params::ParamView`
exposes `published`, `value`, `project` and `rates`; `project` multiplies a base
amount by the ratio of two **archived** window sums. The archived series table
carries the windows for calendar years 2016, 2017, 2024 and 2025, so with
`lag_years = 1` the only projectable tax years are 2025 and 2026 - both of
which are published anyway - and a later year is `IndexUnavailable` rather than
a projection. There is no engine route to a projected 2030 threshold.

**Why the engine slice stopped there.** An inflation path is an `AssumptionSet`
field. `DOMAIN-MODEL.md` §15 requires that type to carry fields (capital-market
classes, correlations, market data, discount rates, a mortality table id) for
which M0 archives no source, and forbids bundling a third-party set. Inventing
a cut-down `AssumptionSet` inside `pfp-params` would freeze a second shape under
S1 that no document describes - the failure E1 exists to repair.

**Branches.**

- **(a) Recommended: say where it lands.** Proposed replacement for the last
  clause of §4.1's "What a user can do now": "...; see each threshold's
  projection rule, base year, base amount, index series and rounding rule, and
  the statutory projection for every year the archived index reaches. Projection
  **beyond the archive**, under an editable inflation assumption
  (`ParamView::for_year(t, &inflation_index)`, `chained_cpi_wedge`), lands with
  the Assumptions Registry screen and its `AssumptionSet`, not with the engine
  slice." And in `ENGINE-SPEC.md` §1.3, after the first sentence: "At M0
  `ParamView` projects only over archived index values (`project`); `for_year`
  and the inflation argument arrive with the `AssumptionSet` that supplies
  them."
- **(b) A minimal inflation-only input now.** `ParamView::for_year(t,
  &InflationIndex)` in `pfp-params`, where `InflationIndex` extends the archived
  series past its last window by a caller-supplied annual `Ratio` and the wedge:
  `index_t = index_last x ((1 + pi) x (1 - wedge))^(t - last)`. It is exact in
  `Ratio` and adds no float. It costs a new public type under S1, a decision on
  how a window *sum* is extended by an annual rate, and fixtures that someone
  other than the implementer must author first. It was not built in this round
  for that last reason: `docs/contributing.md` §3 puts the fixture before the
  code and with a different author.

Either way no value changes. The screen itself is outside this pull request.
