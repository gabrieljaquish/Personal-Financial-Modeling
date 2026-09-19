# `federal-2026` — the first parameter vintage

**Scope:** the M0 engine slice (`PLAN.md` §4.1). Ordinary income-tax rate
schedule (bracket edges and the rate ladder) and the basic standard deduction,
published values for tax years **2025 and 2026**, every filing status; the
statutory base-year amounts and base years each projection rule reads; and the
archived price-index series the rule is taken against.

**Not in this vintage:** the additional (aged/blind) and dependent-limited
standard deductions, capital-gain breakpoints, AMT, the estates-and-trusts rate
table (26 USC 1(j)(2)(E) is not a `FilingStatus`), and every contribution,
Social Security, Medicare and ACA parameter. Those arrive with the milestone
that first needs them.

## Status: pending hand verification — this vintage is not locked

Every value in this directory was transcribed by an AI-assisted session from
the archived primary documents under `params/provenance/`. Under
`docs/contributing.md` §3.2 and ADR-022 a human must read each value back
against the primary document before the vintage locks, and the `TESTING.md`
§5.2 items listed in each table's `[hand_verification]` block must be cleared.

There is deliberately **no entry for this vintage in `params/VINTAGES.lock`**:
locking is `TESTING.md` §12 step 8 and happens only after that human pass.
Until then the vintage is readable and reviewable but carries no immutability
claim, and no `vintage_id` is assigned — a `<name>@<content-hash>` (ADR-010)
over files that are still expected to change would assert an immutability this
vintage does not have. The loader can *compute* the content hash of whatever
documents it is given (`pfp_params::Vintage::content_id`, printed by
`cargo xtask data-hygiene`), under a proposed definition of "contents" that is
decision B9 of `docs/verification/m0-hand-verification.md`; computing it
asserts nothing, and `Vintage::id` stays empty until a human records a lock.

Nothing here was back-solved, tuned or reconciled. **One published figure is
not reproduced by the statutory rule this vintage encodes** — the 2026
head-of-household standard deduction, residual **$25**. It is recorded in
`std_deduction.toml` and as decision B2 of
`docs/verification/m0-hand-verification.md`, not fixed.

## Files

| Table id | File | What it holds |
|---|---|---|
| `irs.ordinary_brackets` | `ordinary_brackets.toml` | Six indexed dollar edges per filing status, per year, plus the seven-rate ladder |
| `irs.std_deduction` | `std_deduction.toml` | Basic standard deduction per filing status, per year |
| `bls.cpi.chained.suur0000sa0` | `../../index-series/cpi-chained-suur0000sa0.toml` | The C-CPI-U observations and window sums the projections read |

The index series sits outside the vintage directory because it is an archived
data snapshot rather than a parameter table, and because the data-hygiene
linter treats every `.toml` under `params/vintages/` as a parameter table
requiring a projection rule and a `RoundingRule` (deviation 4).

## Shape deviations from the design documents

Seam S1 freezes the parameter-table shape at M0 (`PLAN.md` §3). Seven things the
statute says cannot be written in the shape `DOMAIN-MODEL.md` §15,
`ARCHITECTURE.md` §6 and ADR-010 specify. Each is carried in the table that
needs it, in a field whose name says it is an extension, and each is reported
as an erratum rather than worked around. A loader written to the documented
shape will not read these files until the shape settles — which is the point:
**the shape has to settle before S1 freezes, not after.**

1. **`projection.rounding.increment` is a scalar, but the bracket increment is
   per filing status**: $25 for `single` (26 USC 1(j)(3)(B)(ii)) and `mfs`
   (1(f)(7)(B)), $50 otherwise. Written as `increment_by_key`.
2. **`projection.base_year` is a scalar, but from tax year 2026 one bracket
   table has two base years** — 2016 for the tops of the 10% and 12% brackets,
   2017 for the rest (1(j)(3)(B)(i) as amended by PL 119-21 §70101(b)) — and
   the split does not exist in tax year 2025. So the base year varies by
   threshold *within* a table and by year *within* a table. Written as
   `base_year_by_edge`, keyed by tax year.
3. **`projection.base_values` holds one amount per breakdown key**, but a
   bracket table needs six per key. Written as an ordered array matching
   `edges`. Separately, the `mfj`/`qss` standard deduction is *derived* (200%
   of the **rounded** `single` result, 26 USC 63(c)(2)(A)), which no field
   expresses.
4. **No documented shape exists for an archived index-series table**, and
   `TESTING.md` §11.2 gate 9's "a table without a projection rule is a CI
   error" would force a meaningless rule onto one. The series table declares
   `kind = "index-series"`, carries no projection block, and lives under
   `params/index-series/`.
5. **`unit` is a scalar per table, but a rate schedule has two kinds of
   value** — dollar edges and ratios. They are kept in one table because the
   edges are the only part that is projected, so the table has exactly one
   projection rule and one rounding rule, both governing the edges; `[rates]`
   declares its own unit. A separate never-indexed rates table would have to
   carry a `RoundingRule` it does not have, because no statutory rule rounds a
   tax rate — and gate 9 and the hygiene linter both require one on every
   parameter table.

6. **No field says when an indexed amount is first adjusted.** 26 USC
   63(c)(7)(B)(ii) applies only "In the case of a taxable year beginning after
   2025", so the 2025 standard deduction is the unadjusted base and the 2026
   row is the first projected one. Written as
   `projection.first_adjusted_year`. Without it a loader cannot tell a base
   year that has been reached from one that has not, and would try to project
   a year the statute leaves alone.

7. **Nothing bound a table to the archived series it reads, and no field
   recorded which year of it.** `index_series = "cpi.chained.aug12m"` is the
   spelling `DOMAIN-MODEL.md` §15 prescribes, but the archived series' id is its
   publisher's, and the one-year lag of 26 USC 1(f)(3)(A)(i) ("the C-CPI-U for
   the preceding calendar year") appeared in no field. The series table now
   declares the `index_series` name it answers to, and each indexed table
   carries `projection.lag_years`, which is on the §5.2 hand-verification list
   of both tables. `cargo xtask data-hygiene` fails a vintage whose
   `index_series` resolves to no archived series table.

A smaller one: **no design document specifies where a `params/` table
records that it is pending hand verification**, though `TESTING.md` §5.2 and
§11.2 gate 9 presuppose the concept; `verification` is defined for *fixtures*
(§2.2). Each table here carries `verification = "pending-hand-verification"`,
the §2.2 spelling. Note that `params/provenance/INDEX.toml` spells the same
idea `pending-human-verification`; the two should be reconciled before lock.
And no design document names a vintage-level manifest — this file is one, in
Markdown rather than TOML for the reason given under **Files**, following the
precedent of `params/provenance/bls/MANIFEST.md`.

Every value with its source reference, as a read-back checklist, and the
decisions that block locking are in `docs/verification/m0-hand-verification.md`.
