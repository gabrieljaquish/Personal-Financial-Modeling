# M0 hand-verification checklist: the `federal-2026` vintage and the M0 fixtures

This is the maintainer checklist for the **human gate** of the M0 engine slice
(`TESTING.md` §5.2, `docs/contributing.md` §3.2, ADR-022). It lists every value a
person has to read in a primary document before

- `fixtures/pending/**` may move to `fixtures/tier1/**`, and
- the `federal-2026` parameter vintage may be locked in `params/VINTAGES.lock`.

Every number below was transcribed or computed by an AI-assisted session. **None
of it is verified.** Ticking a box means: *I opened the archived document named
on that line, at the place named on that line, and read this value there myself.*
It never means "the tests pass" - the tests were written against the same
transcription and cannot catch a transcription error.

Rules for whoever works through this file:

1. An AI assistant may not tick a box, move a fixture under `fixtures/tier1/`, or
   add an entry to `params/VINTAGES.lock`.
2. If a value here disagrees with the document, **the document wins**. Do not
   tick; open an issue, correct the parameter file or fixture from the document
   by hand, and re-run the gates. Never adjust an index value, a base amount or a
   rounding rule so that a published number is reproduced.
3. The archived `.pdf` and `.html` files are the authority. The `.txt` extractions
   beside them are a convenience for searching; two of them carry a render-date
   banner, so their digests are not stable and are not what a checksum pins.
4. This file names no person. A sign-off is recorded where the design puts it:
   in the fixture's `reviewedBy` block (`TESTING.md` §2.2) and in the history of
   the change that promotes it.
5. Every amount in this file is public law or a public statistic, or a synthetic
   test input. None describes a person or a household.

The file was generated from the parameter files and fixtures named in it, so the
values printed here are the values in those files. If a file changes, regenerate
or re-read; do not trust a stale line.

---

## 0. Decisions that block promotion

These are not read-back items. Each needs a recorded decision (an ADR or an
erratum to the design documents) **before** anything in sections 3 and 4 is
promoted, because each changes what "verified" would mean.

Where a decision is an erratum to a design document, the wording is drafted,
ready to paste, in `docs/verification/m0-design-errata.md` (E1-E7). Seam S1
freezes with the change that carries this file, so E1-E4 belong **in that
change**: afterwards the definition sites are permanently wrong. Those drafts
are proposals; an AI-assisted session does not edit the design documents.

- [ ] **B1 - M0 acceptance, first bullet, is not met from the archive.**
  `PLAN.md` §4.1 requires uprating to reproduce *every* published 2026 threshold
  and standard deduction from the statutory base and the archived index series.
  Computed exactly, it reproduces 6 of 30 bracket thresholds for 2026 (residuals
  +25 to +400 dollars, computed above published), 1 of 30 for the 2025 control
  (residuals -25 to -650) and 4 of 5 standard deductions when `mfj`/`qss` are
  derived as the statute directs (2 of 5 as the table is shipped; see B3). The
  bracket residuals are
  attributed to the index vintage: 26 USC 1(f)(6)(A) fixes the C-CPI-U values at
  those published when the initial August value for the preceding year came
  out, the archived series is a later snapshot, and the archived August 2025
  release prints the C-CPI-U only as percent changes. Decide: source the frozen
  vintage, or restate the acceptance target. Do not promote anything under
  `fixtures/pending/uprating/` until this is recorded.
- [ ] **B2 - the 2026 head-of-household standard deduction, residual 25 dollars.**
  26 USC 63(c)(7)(B)(ii) rounds the *increase* down to a multiple of 50 dollars,
  which gives 24,175; Rev. Proc. 2025-32 §4.14(1) prints 24,150, which is what
  rounding the *total* gives. `RoundingRule.basis` has no value for the second
  reading. Decide before seam S1 freezes.
- [ ] **B3 - `mfj`/`qss` standard deduction is 200 percent of the rounded `single`
  amount (26 USC 63(c)(2)(A)), and no field of the shipped table says so.** Read
  as shipped, the table projects `mfj`/`qss` independently (32,250 for 2026, 50
  dollars above the published and statutory 32,200). Published lookups are
  unaffected. Decide the field (`projection.derived` is the loader's proposal)
  and have the parameter file's owner add it.
- [ ] **B4 - `base_year_by_edge` is a reading, not a quotation.** That the tops of
  the 10 and 12 percent brackets fall back to calendar year 2016 from tax year
  2026, while the other four edges use 2017, is a reading of the words Public
  Law 119-21 §70101(b) inserted into 26 USC 1(j)(3)(B)(i). Read the words (item
  P-BY below) and record agreement or disagreement.
- [ ] **B5 - the identity of the index series.** 26 USC 1(f)(6)(A) names the
  "Chained Consumer Price Index for All Urban Consumers" by title only. That this
  is BLS series `SUUR0000SA0` rests on the BLS catalogue, not on statute
  (`TESTING.md` §5.2, the M0 gate item).
- [ ] **B6 - the denominator after a base-year substitution (26 USC 1(f)(3)(C)).**
  The pipeline takes the denominator to be the C-CPI-U for the substituted base
  year. The vintage stores no denominator, but every projected value depends on
  this reading.
- [ ] **B7 - two facts the statute states that no documented parameter field
  records.** The one-year lag of 26 USC 1(f)(3)(A)(i) ("the C-CPI-U for the
  preceding calendar year") and the binding of
  `index_series = "cpi.chained.aug12m"` to the archived table were constants in
  `crates/pfp-params/src/shipped.rs`. Both are now **data**: each indexed table
  carries `projection.lag_years = 1`, and the archived series table declares
  `index_series = "cpi.chained.aug12m"`, the name tables resolve against
  (`cargo xtask data-hygiene` fails a vintage whose `index_series` resolves to
  no archived series table). Confirm the lag against the statute (items
  `P-BR-LAG`, `P-SD-LAG`) and decide, under B12, whether these two fields are
  the shape S1 freezes.
- [ ] **B8 - the two rounding rules of the rate schedule are not parameters.**
  `irs.whole_dollar` (half-up to the whole dollar, `ENGINE-SPEC.md` §3.2) and
  `money.cent_half_even` (the rule each rate-times-amount product is taken under)
  are defined in `crates/pfp-tax/src/schedule.rs`, because the vintage carries no
  named-rule table. `ENGINE-SPEC.md` §3.2 does not name the cent rule at all.
  Neither can change a result for a whole-dollar income under whole-percent
  rates, which is every fixture point. Decide the cent rule and where named rules
  live before seam S3 freezes at M1. Both branches of each question, with
  wording, are in `m0-design-errata.md` E3.
- [ ] **B9 - how a vintage's content hash is formed.** ADR-010 and
  `DOMAIN-MODEL.md` §15 define the vintage id as `<name>@<sha256-of-contents>`
  but no document says what "contents" is for a vintage of several files. A
  definition is **proposed and implemented** so the decision can be made against
  running code (`pfp_params::VintageId`, `Vintage::content_id`):
  `sha256("pfp-vintage-id/v1\n" + name + "\n"`, then for each document in
  ascending byte order of its `id`: `kind + " " + id + " " + byte length + "\n"`
  followed by the document's bytes`)`, where the documents are every parameter
  table of the vintage (`kind` `table`) and every archived index series one of
  them reads (`kind` `index-series`), hashed exactly as stored, with no
  canonicalisation. `VINTAGE.md` is prose the engine never loads and is outside
  the id; `params/VINTAGES.lock` pins it per file. `cargo xtask data-hygiene`
  prints the id. Computing it claims nothing: `Vintage::id()` stays `None`, and
  every fixture keeps `paramVintage: null`, until a human records a lock and the
  embedding calls `Vintage::locked_as`. Decide: accept this definition as a
  one-paragraph erratum to `DOMAIN-MODEL.md` §15, or replace it (the id then
  changes, which is harmless before a lock). The paragraph is drafted in
  `m0-design-errata.md` E4.
- [ ] **B10 - two fixtures promoted to one tier-1 id (changed in fix round 1;
  confirm).** `schedule/mfj-2026-grid.json` and
  `schedule/mfj-2026-plan-acceptance.json` both carried
  `promotesTo: t1/schedule/mfj-2026-grid`. The acceptance file now carries
  `t1/schedule/mfj-2026-plan-acceptance` (envelope only; no input or expected
  value changed), every `promotesTo` is unique, and the `pfp-tax` fixture test
  asserts the new id. Confirm, or merge the three acceptance cases into the grid
  file instead.
- [ ] **B11 - open questions carried by `rounding/table.json`** (`nearest-exact-tie`,
  `down-versus-truncate`, `halfup-negative-tie`, `increment-unit`). They have no
  expected value by design; each needs a decision before M1 or M3, not before
  this promotion, but the file should not reach tier 1 with them unrecorded.
- [ ] **B12 - seam S1 is about to freeze in a shape no design document
  describes.** `DOMAIN-MODEL.md` §15, `ENGINE-SPEC.md` §1.3 and `TESTING.md`
  §3.1 and §11.2 gate 9 all specify
  `projection{rule, index, index_series, base_year, base_values, rounding}` with
  a scalar `base_year`, one `base_values` amount per filing status and a scalar
  `rounding.increment`. The shipped tables additionally carry
  `base_year_by_edge`, per-status **arrays** of `base_values`, `edges`,
  `rounding.increment_by_key`, `first_adjusted_year`, `lag_years`, a `[rates]`
  sub-table with its own `unit`, `verification`/`[hand_verification]`, and the
  archived series table (`kind = "index-series"`, `index_series`, `[window]`,
  `[observations.<year>]`, `[window_sum]`) has no documented shape at all
  (`VINTAGE.md` lists each with its statutory reason). Either amend those four
  definition sites to the shipped shape or change the shape to match them, **in
  the change that freezes S1**; afterwards the documents are permanently wrong.
  Two unit questions belong to the same erratum: (a) `ENGINE-SPEC.md` §1.1
  types `RoundingRule.increment` as `Cents`, while `DOMAIN-MODEL.md` §15's
  worked TOML writes `increment = 50` in a `unit = "USD"` table - the loader
  reads the TOML figure in the table's unit (dollars) and converts to cents, and
  §15 should say so beside the field, or a table author will write cents and
  silently round to 50 cents; (b) whether `unit` may be overridden per
  sub-table, as `[rates] unit = "ratio"` does. The gate follows the decision
  automatically: `data-hygiene` rule 8 drives the `pfp-params` loader, so it
  checks whatever field names the loader is changed to read. B4 stays a reading
  to be read, not settled by computing a threshold to see which base year
  reproduces it. Replacement text for all four sites, including a documented
  shape for the index-series table and the `verification` field of a parameter
  table, is drafted in `m0-design-errata.md` E1; the increment unit is E2.
- [ ] **B13 - the fixture envelope differs from `TESTING.md` §2.2.** All 21
  files carry `"tier": "pending"` (§2.2 types `tier` as an integer) with
  `"targetTier": 1`, and the schedule fixtures nest line ids at
  `expect.cases[].lines[].lineId` where §2.2's loader rule names
  `expect.lines[].id`. §2.2 defines no pending area, and its loader rejects
  `pending-hand-verification` at tier 1, so `fixtures/pending/` is the right
  place. The line-id guard is enforced today by the `pfp-tax` fixture driver
  (every `lineId` found anywhere in a case must exist in the worksheet, and each
  is compared with the engine's id), so a renamed line does break the build.
  Decide: add a pending-area rule to §2.2 that types `tier` and the `cases[]`
  wrapper, or have the fixtures' author conform the envelope. Promotion step 4
  already sets `"tier": 1`. The extra envelope keys (`promotesTo`, `targetTier`,
  `verificationNote`, `additionalSources`, `residuals`, `openQuestions`) are
  undefined in §2.2 as well; a pending-area rule covering all of it is drafted
  in `m0-design-errata.md` E5.
- [ ] **B14 - the negative control meets `PLAN.md` §4.1 but only half of
  `TESTING.md` §3.1, and the other half hangs on B1.**
  `uprating/chained-path-divergence.json` diverges from the statutory result on
  30 of 30 rows, and on none does the chained value equal the published one, so
  the `PLAN.md` clause holds. §3.1 adds "the statutory value is the published
  one", which holds only on the 6 zero-residual rows; the fixture asserts those
  6 (they cover all five statuses) and carries the other 24 as unasserted
  context. The fixture is not to be changed. Under B1 branch "source the frozen
  index vintage" the clause is restored in full and all 30 rows become
  assertable; under branch "restate the acceptance target" §3.1's clause must be
  restated with it, because it cannot hold on the 24 residual rows. Do not drop
  the clause silently at promotion.
- [ ] **B15 - the provenance catalogue.** `TESTING.md` §12 step 1 says
  `params/provenance/<year>/`; the archive is keyed by publisher, because a
  current-text statute has no single year (`params/provenance/INDEX.toml`
  header). `INDEX.toml` and `params/provenance/bls/MANIFEST.md` are two
  catalogues that should be merged or one made normative, and they spell the
  pending status differently (`pending-human-verification` against the
  `pending-hand-verification` of §2.2 and the parameter tables). Both now state
  the 17 USC 105 licence basis, and the root `NOTICE` scopes
  `params/provenance/` out of the Apache-2.0 grant.
- [ ] **B16 - as shaped, the uprating fixtures can take neither tier-1
  `verification` value, and no fixture has the top-level `derivation` the
  sign-off hashes.** `TESTING.md` §2.2 admits `hand-worked-reviewed` at tier 1
  only with `synthetic: true`, which overloads `synthetic` (`docs/contributing.md`
  §1.3: the inputs describe no real household) to also mean "computed, not
  transcribed". `uprating/2026-brackets.json`, `uprating/2026-stdded.json` and
  `uprating/chained-path-divergence.json` have computed expected columns
  (`computedDollars`, `flooredIncreaseDollars`, `colaExact`, `residualDollars`)
  from **real** statutory inputs, so they correctly carry `synthetic: false` -
  and §3.1 assigns the third `hand-worked-reviewed` in terms, which §2.2 then
  forbids. Do not flip `synthetic` to get through the loader: that would be a
  false statement. Decide the §2.2 wording (drafted in `m0-design-errata.md`
  E6, which decouples the two meanings). Separately, the 13 fixtures this
  project computed (11 schedule grids, `rounding/table.json`,
  `money/mul_ratio.json`) carry the arithmetic per case (`inputs.formula` and
  per-bracket intermediates; a `derivation` on every rounding and `mul_ratio`
  case) but no top-level `derivation` string; drafts composed only from text
  already in each file are in E6, for the fixtures' author or the maintainer to
  place - not the engine's implementer. **This is on the critical path:** with
  one maintainer the admissible review is the blind cooling-off re-derivation no
  sooner than seven days after the derivation it signs off (§2.2), so the text
  should exist a week before promotion is wanted.
- [ ] **B17 - `PLAN.md` §4.1 promises projection "under an editable inflation
  assumption"; no engine path provides it.** `ENGINE-SPEC.md` §1.3's
  `ParamView::for_year(t, &inflation_index)` and `chained_cpi_wedge` do not
  exist. `ParamView::project` reads archived index windows only (calendar years
  2016, 2017, 2024, 2025), so the projectable tax years are 2025 and 2026 and
  anything later is `IndexUnavailable`. An inflation path is an `AssumptionSet`
  field, and `DOMAIN-MODEL.md` §15's `AssumptionSet` needs fields M0 archives no
  source for. Decide: record in `PLAN.md` §4.1 that projection beyond the
  archive lands with the Assumptions Registry screen and its `AssumptionSet`
  (recommended; wording in `m0-design-errata.md` E7), or commission a minimal
  inflation-only input to `pfp-params` now - which needs fixtures authored first
  by someone other than the implementer.

---

## 1. Step zero: the archive is what it says it is

Run, from the repository root:

```sh
shasum -a 256 params/provenance/irs/*.pdf params/provenance/usc/*.html \
  params/provenance/congress/*.pdf params/provenance/bls/time-series/su.* \
  params/provenance/bls/cpi_09112025.htm \
  params/provenance/bls/chained-cpi-questions-and-answers.htm
```

and compare with the table. Then, for at least the two revenue procedures and
the two Code sections, **fetch the document again from the publisher URL in
`params/provenance/INDEX.toml`** and confirm it is the same document (the PDFs
should be byte-identical; the Code pages embed session state and will not be, so
compare the statutory text). A checksum only proves the file has not changed
since it was archived, not that it was the right file.

| Key | Archived file | sha256 | |
|---|---|---|---|
| `RP24-40` | `params/provenance/irs/rp-24-40.pdf` (Rev. Proc. 2024-40) | `4de9db6b6662b5c305a59100a027051628823b42fd5817ea4e383ad4c302af1f` | [ ] |
| `RP25-32` | `params/provenance/irs/rp-25-32.pdf` (Rev. Proc. 2025-32) | `e9ada115fb43a4af5ea326ca91edd40ab0bf29b9be01d2c13d7b8fa96214635b` | [ ] |
| `USC1` | `params/provenance/usc/usc26-s1.html` (26 USC 1, release point 20260911_119-108) | `9a23b4e2577dbab0b19b5498407338e0a0dc0f1f78b3d2f7e4b877f180686313` | [ ] |
| `USC63` | `params/provenance/usc/usc26-s63.html` (26 USC 63, release point 20260911_119-108) | `83efb2d828ddd0d80a1eb28caa7c863581f95b18da501aef09119922f88e0d57` | [ ] |
| `PL119-21` | `params/provenance/congress/PLAW-119publ21.pdf` (Public Law 119-21) | `42f86c3d408bccb1a663c11a51a90d207b3dbc25e689240da51726154be51c8b` | [ ] |
| `BLS-SU` | `params/provenance/bls/time-series/su.data.1.AllItems` (BLS C-CPI-U flat file, series SUUR0000SA0) | `c40304234a1e838cffe4ba88567a997b91595c223613f2584aa0cab3191f3f75` | [ ] |
| `BLS-SERIES` | `params/provenance/bls/time-series/su.series` (BLS su series catalogue) | `7da3ac42aef524b2cc21fe2cb8f6ab71124ae808db699f2b4d5919b306bafbcc` | [ ] |
| `BLS-FOOT` | `params/provenance/bls/time-series/su.footnote` (BLS su footnote codes) | `732d760a9e80af7637f7bcded8f4828dd0422b46faa98e03099e8fd3d792170f` | [ ] |
| `BLS-REL` | `params/provenance/bls/cpi_09112025.htm` (BLS CPI release USDL-25-1356 (August 2025)) | `f376b178ee4e16a7cc2ae6810e2164cad96aa8da156900623b2ff780a78f26bf` | [ ] |
| `BLS-FAQ` | `params/provenance/bls/chained-cpi-questions-and-answers.htm` (BLS chained-CPI questions and answers) | `7dddb81e1b25b28d9eb638a13d35117ba15a658bdfd980ee1702e43d124bde2e` | [ ] |

Source keys used below:

- **`RP24-40`** - §2.01, TABLES 1-4, printed pages 5-7: the 2025 rate tables. Its
  §2.15(1) standard deduction was **removed** by Rev. Proc. 2025-32 §3.01 and must
  not be used.
- **`RP25-32`** - §3.01, printed page 9: the 2025 standard deduction as amended;
  §4.01, TABLES 1-4, printed pages 10-12: the 2026 rate tables; §4.14(1), printed
  page 18: the 2026 standard deduction.
- **`USC1`** - §1(j)(2)(A)-(D): base amounts; §1(j)(3)(A),(B): base years and the
  25-dollar rule for `single`; §1(f)(3): the adjustment; §1(f)(6): the index and
  its averaging window; §1(f)(7): rounding.
- **`USC63`** - §63(c)(2): structure; §63(c)(7)(A): base amounts; §63(c)(7)(B)(ii):
  base year 2024, first adjusted year, and its own rounding sentence.
- **`PL119-21`** - §70101 (PDF page 88 = 139 Stat. 158) and §70102 (PDF pages
  88-89): the 2025 amendments and their effective dates.
- **`BLS-SU`** - rows of series `SUUR0000SA0`, one per month.

---

### 1.1 Change record: files edited after the value checks ran

The independent value checks of this slice - the archived sources (456 values),
the fixtures (2,953 assertions) and the parameter tables (216 values) - ran
against the files as their authors wrote them, and each reported zero
mismatches. Some value-bearing files were edited **afterwards** - after the
engine crates existed and after those reports - in the first fix round, by the
session that implemented the engine. `docs/contributing.md` §3 keeps the author
of a constant and the author of its test apart, so that is recorded here rather
than left to a reviewer's memory; the slice lands as one change, so the
repository history will not show it. Whoever works this checklist should know
that the checker reports alone do not cover the bytes on disk. What follows
does.

**Five fixtures**, one leaf each, none an input or an expected value. Four are
prose in which a reference to a git-ignored working note was replaced; one is
an envelope id:

| File | Leaf | Change |
|---|---|---|
| `fixtures/pending/money/mul_ratio.json` | `/notes` | "derived longhand in" a working note became "derived longhand in its own case's derivation field" |
| `fixtures/pending/uprating/2026-brackets.json` | `/notes` | "Diagnosis (see" a working note ")" became "Diagnosis (summarised as decision B1 in" this file ")" |
| `fixtures/pending/uprating/2026-stdded.json` | `/notes` | "The human decision (see" a working note ")" became "The human decision (decision B2 in" this file ")" |
| `fixtures/pending/rounding/table.json` | `/expect/cases[5]/note` | "see" a working note became "see decision B1 in" this file; it is a `note` on the case - the case's inputs and `expectCents` are unchanged |
| `fixtures/pending/schedule/mfj-2026-plan-acceptance.json` | `/promotesTo` | `t1/schedule/mfj-2026-grid` became `t1/schedule/mfj-2026-plan-acceptance` (decision B10) |

The other 16 fixtures and `fixtures/pending/README.md` are exactly what their
author's generator writes.

**Three parameter files** (`ordinary_brackets.toml`, `std_deduction.toml`,
`params/index-series/cpi-chained-suur0000sa0.toml`): `projection.lag_years = 1`
was added to the two tables and `index_series = "cpi.chained.aug12m"` to the
series table (decision B7 - two facts that had been constants in code), each
with an entry in the file's `[hand_verification] open` list and a line in this
checklist (`P-BR-LAG`, `P-SD-LAG`, `P-IX-NAME`); and comments pointing at
git-ignored working notes were repointed. `lag_years` is a new value-bearing
field, which is why it is on the read-back list. **No published amount, base
amount, base year, increment, rate or index observation was changed.** Both
provenance catalogues (`params/provenance/INDEX.toml`,
`params/provenance/bls/MANIFEST.md`), `VINTAGE.md` and this file were edited in
the same round (prose, the 17 USC 105 licence basis, and the digests below); no
archived document was touched.

**How this was established.** For the fixtures: the fixture author's generator,
untouched since before the first engine file existed, was re-run into a scratch
directory and every JSON leaf of all 21 files compared with the files on disk;
exactly the five leaves above differ, and the README is identical. This was done
by a reviewer and again, independently, when this record was written. The
fixture checker and the parameter audit were then re-run against the edited
tree: 2,953 and 341 assertions, no mismatch. For the parameter files there is
no earlier copy to diff against - nothing under `params/` has been committed -
so the statement above rests on the fix-round log and on the re-run audit, which
re-derives every parameter number from the archive; **the read-back in sections
2-4 is what actually verifies them.** The generator and both checkers are the
maintainer's git-ignored working files and are not part of the repository, so a
later reader cannot repeat this step; that is the reason it is written down.

**The digests printed in this file were regenerated after those edits.** All 21
fixture sha256 prefixes in section 5, the three parameter-file digests in
sections 2-4 and the ten archive digests in the table above were compared with
the files on disk when this record was written: 34 of 34 match. A digest that
does not match means a later edit; stop and find out what changed.

A later round that touches a value-bearing file adds a dated line here.

- Fix round 2 (2026-09-19): no file under `fixtures/` or `params/` was edited.

---

## 2. Parameters: `params/vintages/federal-2026/ordinary_brackets.toml`

File sha256 at generation: `9cbcadb2c8fe9abe14e96e7065d30946519f761b52ecabeee82e3ade34f64236`

### 2.1 Published thresholds, tax year 2025

Read in `RP24-40` §2.01, printed pp. 5-7, the left-hand "If Taxable Income Is" column of each table (not the cumulative-tax column beside it).

- [ ] `P-BR-2025-mfj-top_of_10` mfj top of 10% = **23,850** - `RP24-40` TABLE 1
- [ ] `P-BR-2025-mfj-top_of_12` mfj top of 12% = **96,950** - `RP24-40` TABLE 1
- [ ] `P-BR-2025-mfj-top_of_22` mfj top of 22% = **206,700** - `RP24-40` TABLE 1
- [ ] `P-BR-2025-mfj-top_of_24` mfj top of 24% = **394,600** - `RP24-40` TABLE 1
- [ ] `P-BR-2025-mfj-top_of_32` mfj top of 32% = **501,050** - `RP24-40` TABLE 1
- [ ] `P-BR-2025-mfj-top_of_35` mfj top of 35% = **751,600** - `RP24-40` TABLE 1
- [ ] `P-BR-2025-qss-top_of_10` qss top of 10% = **23,850** - `RP24-40` TABLE 1 (shared with mfj; no table of its own)
- [ ] `P-BR-2025-qss-top_of_12` qss top of 12% = **96,950** - `RP24-40` TABLE 1 (shared with mfj; no table of its own)
- [ ] `P-BR-2025-qss-top_of_22` qss top of 22% = **206,700** - `RP24-40` TABLE 1 (shared with mfj; no table of its own)
- [ ] `P-BR-2025-qss-top_of_24` qss top of 24% = **394,600** - `RP24-40` TABLE 1 (shared with mfj; no table of its own)
- [ ] `P-BR-2025-qss-top_of_32` qss top of 32% = **501,050** - `RP24-40` TABLE 1 (shared with mfj; no table of its own)
- [ ] `P-BR-2025-qss-top_of_35` qss top of 35% = **751,600** - `RP24-40` TABLE 1 (shared with mfj; no table of its own)
- [ ] `P-BR-2025-hoh-top_of_10` hoh top of 10% = **17,000** - `RP24-40` TABLE 2
- [ ] `P-BR-2025-hoh-top_of_12` hoh top of 12% = **64,850** - `RP24-40` TABLE 2
- [ ] `P-BR-2025-hoh-top_of_22` hoh top of 22% = **103,350** - `RP24-40` TABLE 2
- [ ] `P-BR-2025-hoh-top_of_24` hoh top of 24% = **197,300** - `RP24-40` TABLE 2
- [ ] `P-BR-2025-hoh-top_of_32` hoh top of 32% = **250,500** - `RP24-40` TABLE 2
- [ ] `P-BR-2025-hoh-top_of_35` hoh top of 35% = **626,350** - `RP24-40` TABLE 2
- [ ] `P-BR-2025-single-top_of_10` single top of 10% = **11,925** - `RP24-40` TABLE 3
- [ ] `P-BR-2025-single-top_of_12` single top of 12% = **48,475** - `RP24-40` TABLE 3
- [ ] `P-BR-2025-single-top_of_22` single top of 22% = **103,350** - `RP24-40` TABLE 3
- [ ] `P-BR-2025-single-top_of_24` single top of 24% = **197,300** - `RP24-40` TABLE 3
- [ ] `P-BR-2025-single-top_of_32` single top of 32% = **250,525** - `RP24-40` TABLE 3
- [ ] `P-BR-2025-single-top_of_35` single top of 35% = **626,350** - `RP24-40` TABLE 3
- [ ] `P-BR-2025-mfs-top_of_10` mfs top of 10% = **11,925** - `RP24-40` TABLE 4
- [ ] `P-BR-2025-mfs-top_of_12` mfs top of 12% = **48,475** - `RP24-40` TABLE 4
- [ ] `P-BR-2025-mfs-top_of_22` mfs top of 22% = **103,350** - `RP24-40` TABLE 4
- [ ] `P-BR-2025-mfs-top_of_24` mfs top of 24% = **197,300** - `RP24-40` TABLE 4
- [ ] `P-BR-2025-mfs-top_of_32` mfs top of 32% = **250,525** - `RP24-40` TABLE 4
- [ ] `P-BR-2025-mfs-top_of_35` mfs top of 35% = **375,800** - `RP24-40` TABLE 4

### 2.2 Published thresholds, tax year 2026

Read in `RP25-32` §4.01, printed pp. 10-12, the left-hand "If Taxable Income Is" column of each table (not the cumulative-tax column beside it).

- [ ] `P-BR-2026-mfj-top_of_10` mfj top of 10% = **24,800** - `RP25-32` TABLE 1
- [ ] `P-BR-2026-mfj-top_of_12` mfj top of 12% = **100,800** - `RP25-32` TABLE 1
- [ ] `P-BR-2026-mfj-top_of_22` mfj top of 22% = **211,400** - `RP25-32` TABLE 1
- [ ] `P-BR-2026-mfj-top_of_24` mfj top of 24% = **403,550** - `RP25-32` TABLE 1
- [ ] `P-BR-2026-mfj-top_of_32` mfj top of 32% = **512,450** - `RP25-32` TABLE 1
- [ ] `P-BR-2026-mfj-top_of_35` mfj top of 35% = **768,700** - `RP25-32` TABLE 1
- [ ] `P-BR-2026-qss-top_of_10` qss top of 10% = **24,800** - `RP25-32` TABLE 1 (shared with mfj; no table of its own)
- [ ] `P-BR-2026-qss-top_of_12` qss top of 12% = **100,800** - `RP25-32` TABLE 1 (shared with mfj; no table of its own)
- [ ] `P-BR-2026-qss-top_of_22` qss top of 22% = **211,400** - `RP25-32` TABLE 1 (shared with mfj; no table of its own)
- [ ] `P-BR-2026-qss-top_of_24` qss top of 24% = **403,550** - `RP25-32` TABLE 1 (shared with mfj; no table of its own)
- [ ] `P-BR-2026-qss-top_of_32` qss top of 32% = **512,450** - `RP25-32` TABLE 1 (shared with mfj; no table of its own)
- [ ] `P-BR-2026-qss-top_of_35` qss top of 35% = **768,700** - `RP25-32` TABLE 1 (shared with mfj; no table of its own)
- [ ] `P-BR-2026-hoh-top_of_10` hoh top of 10% = **17,700** - `RP25-32` TABLE 2
- [ ] `P-BR-2026-hoh-top_of_12` hoh top of 12% = **67,450** - `RP25-32` TABLE 2
- [ ] `P-BR-2026-hoh-top_of_22` hoh top of 22% = **105,700** - `RP25-32` TABLE 2
- [ ] `P-BR-2026-hoh-top_of_24` hoh top of 24% = **201,750** - `RP25-32` TABLE 2
- [ ] `P-BR-2026-hoh-top_of_32` hoh top of 32% = **256,200** - `RP25-32` TABLE 2
- [ ] `P-BR-2026-hoh-top_of_35` hoh top of 35% = **640,600** - `RP25-32` TABLE 2
- [ ] `P-BR-2026-single-top_of_10` single top of 10% = **12,400** - `RP25-32` TABLE 3
- [ ] `P-BR-2026-single-top_of_12` single top of 12% = **50,400** - `RP25-32` TABLE 3
- [ ] `P-BR-2026-single-top_of_22` single top of 22% = **105,700** - `RP25-32` TABLE 3
- [ ] `P-BR-2026-single-top_of_24` single top of 24% = **201,775** - `RP25-32` TABLE 3
- [ ] `P-BR-2026-single-top_of_32` single top of 32% = **256,225** - `RP25-32` TABLE 3
- [ ] `P-BR-2026-single-top_of_35` single top of 35% = **640,600** - `RP25-32` TABLE 3
- [ ] `P-BR-2026-mfs-top_of_10` mfs top of 10% = **12,400** - `RP25-32` TABLE 4
- [ ] `P-BR-2026-mfs-top_of_12` mfs top of 12% = **50,400** - `RP25-32` TABLE 4
- [ ] `P-BR-2026-mfs-top_of_22` mfs top of 22% = **105,700** - `RP25-32` TABLE 4
- [ ] `P-BR-2026-mfs-top_of_24` mfs top of 24% = **201,775** - `RP25-32` TABLE 4
- [ ] `P-BR-2026-mfs-top_of_32` mfs top of 32% = **256,225** - `RP25-32` TABLE 4
- [ ] `P-BR-2026-mfs-top_of_35` mfs top of 35% = **384,350** - `RP25-32` TABLE 4

### 2.3 Rate ladder

- [ ] `P-RATES-2025` rates for 2025 = **0.10, 0.12, 0.22, 0.24, 0.32, 0.35, 0.37** in every one of TABLES 1-4 - `RP24-40` §2.01; the rates themselves are 26 USC 1(j)(2) (`USC1`)
- [ ] `P-RATES-2026` rates for 2026 = **0.10, 0.12, 0.22, 0.24, 0.32, 0.35, 0.37** in every one of TABLES 1-4 - `RP25-32` §4.01; the rates themselves are 26 USC 1(j)(2) (`USC1`)
- [ ] `P-RATES-FLAT` the ladder is not indexed (`rule = "flat"`): no provision of 26 USC 1(f) adjusts a rate - `USC1` §1(f)(2)

### 2.4 Statutory base amounts (`projection.base_values`)

Read in `USC1` §1(j)(2), the table in each subparagraph. These are the tables for taxable year 2018 (§1(j)(3)(A)).

- [ ] `P-BASE-mfj-top_of_10` mfj top of 10% base = **19,050** - `USC1` §1(j)(2)(A)
- [ ] `P-BASE-mfj-top_of_12` mfj top of 12% base = **77,400** - `USC1` §1(j)(2)(A)
- [ ] `P-BASE-mfj-top_of_22` mfj top of 22% base = **165,000** - `USC1` §1(j)(2)(A)
- [ ] `P-BASE-mfj-top_of_24` mfj top of 24% base = **315,000** - `USC1` §1(j)(2)(A)
- [ ] `P-BASE-mfj-top_of_32` mfj top of 32% base = **400,000** - `USC1` §1(j)(2)(A)
- [ ] `P-BASE-mfj-top_of_35` mfj top of 35% base = **600,000** - `USC1` §1(j)(2)(A)
- [ ] `P-BASE-qss-top_of_10` qss top of 10% base = **19,050** - `USC1` §1(j)(2)(A)
- [ ] `P-BASE-qss-top_of_12` qss top of 12% base = **77,400** - `USC1` §1(j)(2)(A)
- [ ] `P-BASE-qss-top_of_22` qss top of 22% base = **165,000** - `USC1` §1(j)(2)(A)
- [ ] `P-BASE-qss-top_of_24` qss top of 24% base = **315,000** - `USC1` §1(j)(2)(A)
- [ ] `P-BASE-qss-top_of_32` qss top of 32% base = **400,000** - `USC1` §1(j)(2)(A)
- [ ] `P-BASE-qss-top_of_35` qss top of 35% base = **600,000** - `USC1` §1(j)(2)(A)
- [ ] `P-BASE-hoh-top_of_10` hoh top of 10% base = **13,600** - `USC1` §1(j)(2)(B)
- [ ] `P-BASE-hoh-top_of_12` hoh top of 12% base = **51,800** - `USC1` §1(j)(2)(B)
- [ ] `P-BASE-hoh-top_of_22` hoh top of 22% base = **82,500** - `USC1` §1(j)(2)(B)
- [ ] `P-BASE-hoh-top_of_24` hoh top of 24% base = **157,500** - `USC1` §1(j)(2)(B)
- [ ] `P-BASE-hoh-top_of_32` hoh top of 32% base = **200,000** - `USC1` §1(j)(2)(B)
- [ ] `P-BASE-hoh-top_of_35` hoh top of 35% base = **500,000** - `USC1` §1(j)(2)(B)
- [ ] `P-BASE-single-top_of_10` single top of 10% base = **9,525** - `USC1` §1(j)(2)(C)
- [ ] `P-BASE-single-top_of_12` single top of 12% base = **38,700** - `USC1` §1(j)(2)(C)
- [ ] `P-BASE-single-top_of_22` single top of 22% base = **82,500** - `USC1` §1(j)(2)(C)
- [ ] `P-BASE-single-top_of_24` single top of 24% base = **157,500** - `USC1` §1(j)(2)(C)
- [ ] `P-BASE-single-top_of_32` single top of 32% base = **200,000** - `USC1` §1(j)(2)(C)
- [ ] `P-BASE-single-top_of_35` single top of 35% base = **500,000** - `USC1` §1(j)(2)(C)
- [ ] `P-BASE-mfs-top_of_10` mfs top of 10% base = **9,525** - `USC1` §1(j)(2)(D)
- [ ] `P-BASE-mfs-top_of_12` mfs top of 12% base = **38,700** - `USC1` §1(j)(2)(D)
- [ ] `P-BASE-mfs-top_of_22` mfs top of 22% base = **82,500** - `USC1` §1(j)(2)(D)
- [ ] `P-BASE-mfs-top_of_24` mfs top of 24% base = **157,500** - `USC1` §1(j)(2)(D)
- [ ] `P-BASE-mfs-top_of_32` mfs top of 32% base = **200,000** - `USC1` §1(j)(2)(D)
- [ ] `P-BASE-mfs-top_of_35` mfs top of 35% base = **300,000** - `USC1` §1(j)(2)(D)

### 2.5 Base years (`projection.base_year_by_edge`)

- [ ] `P-BY` Read 26 USC 1(j)(3)(B) in full in `USC1`, then Public Law 119-21 §70101(b) and (c) in `PL119-21` (PDF page 88). Confirm: (1) clause (i) substitutes 'calendar year 2017' for 'calendar year 2016'; (2) the words restricting clause (i) to "any rate bracket higher than 12 percent ends" and "any rate bracket higher than 22 percent begins" were *inserted* by §70101(b); (3) §70101(c) applies the insertion to taxable years beginning after 2025-12-31. (Decision B4 covers the mapping onto named edges.)
- [ ] `P-BY-2025-top_of_10` tax year 2025, top of 10%: base year **2017** - `USC1` §1(j)(3)(B)(i) with §1(f)(3)(A)(ii)
- [ ] `P-BY-2025-top_of_12` tax year 2025, top of 12%: base year **2017** - `USC1` §1(j)(3)(B)(i) with §1(f)(3)(A)(ii)
- [ ] `P-BY-2025-top_of_22` tax year 2025, top of 22%: base year **2017** - `USC1` §1(j)(3)(B)(i) with §1(f)(3)(A)(ii)
- [ ] `P-BY-2025-top_of_24` tax year 2025, top of 24%: base year **2017** - `USC1` §1(j)(3)(B)(i) with §1(f)(3)(A)(ii)
- [ ] `P-BY-2025-top_of_32` tax year 2025, top of 32%: base year **2017** - `USC1` §1(j)(3)(B)(i) with §1(f)(3)(A)(ii)
- [ ] `P-BY-2025-top_of_35` tax year 2025, top of 35%: base year **2017** - `USC1` §1(j)(3)(B)(i) with §1(f)(3)(A)(ii)
- [ ] `P-BY-2026-top_of_10` tax year 2026, top of 10%: base year **2016** - `USC1` §1(j)(3)(B)(i) with §1(f)(3)(A)(ii)
- [ ] `P-BY-2026-top_of_12` tax year 2026, top of 12%: base year **2016** - `USC1` §1(j)(3)(B)(i) with §1(f)(3)(A)(ii)
- [ ] `P-BY-2026-top_of_22` tax year 2026, top of 22%: base year **2017** - `USC1` §1(j)(3)(B)(i) with §1(f)(3)(A)(ii)
- [ ] `P-BY-2026-top_of_24` tax year 2026, top of 24%: base year **2017** - `USC1` §1(j)(3)(B)(i) with §1(f)(3)(A)(ii)
- [ ] `P-BY-2026-top_of_32` tax year 2026, top of 32%: base year **2017** - `USC1` §1(j)(3)(B)(i) with §1(f)(3)(A)(ii)
- [ ] `P-BY-2026-top_of_35` tax year 2026, top of 35%: base year **2017** - `USC1` §1(j)(3)(B)(i) with §1(f)(3)(A)(ii)

### 2.6 Projection rule and rounding

- [ ] `P-BR-RULE` rule = `index`, index = `cpi.chained` (C-CPI-U) - `USC1` §1(f)(3), §1(f)(6)(A)
- [ ] `P-BR-DIR` direction = `down` ("rounded to the next lowest multiple") - `USC1` §1(f)(7)(A)
- [ ] `P-BR-BASIS` basis = `IncreaseOverBase` (the *increase* is rounded, not the total) - `USC1` §1(f)(7)(A)
- [ ] `P-BR-INC-mfj` mfj increment = **50** dollars - `USC1` §1(f)(7)(A)
- [ ] `P-BR-INC-qss` qss increment = **50** dollars - `USC1` §1(f)(7)(A)
- [ ] `P-BR-INC-hoh` hoh increment = **50** dollars - `USC1` §1(f)(7)(A)
- [ ] `P-BR-INC-single` single increment = **25** dollars - `USC1` §1(f)(7)(B) by way of §1(j)(3)(B)(ii)
- [ ] `P-BR-INC-mfs` mfs increment = **25** dollars - `USC1` §1(f)(7)(B)
- [ ] `P-BR-LAG` lag_years = **1**: the adjustment for tax year Y reads "the C-CPI-U for the preceding calendar year", i.e. calendar year Y - 1 - `USC1` §1(f)(3)(A)(i), with §1(f)(1) and §1(f)(2)(A) for which calendar year a table is prescribed for (decision B7)
- [ ] `P-BR-ASOF` as_of = `2025-10-09` - `RP25-32` §1 (the 2025 row's own as-of is 2024-10-22, `RP24-40` §1)

---

## 3. Parameters: `params/vintages/federal-2026/std_deduction.toml`

File sha256 at generation: `fcaf03cb7d733ce753968e289534e112af22eb0ee604defd6e64fced0862f152`

### 3.1 Published amounts

Both years are read in `RP25-32`. The 2025 row is §3.01 (printed page 9), which removes Rev. Proc. 2024-40 §2.15(1); confirm the removal sentence while there.

- [ ] `P-SD-2025-mfj` mfj 2025 = **31,500** - `RP25-32` §3.01, printed p. 9
- [ ] `P-SD-2025-qss` qss 2025 = **31,500** (shares the joint-return amount) - `RP25-32` §3.01, printed p. 9
- [ ] `P-SD-2025-hoh` hoh 2025 = **23,625** - `RP25-32` §3.01, printed p. 9
- [ ] `P-SD-2025-single` single 2025 = **15,750** - `RP25-32` §3.01, printed p. 9
- [ ] `P-SD-2025-mfs` mfs 2025 = **15,750** - `RP25-32` §3.01, printed p. 9
- [ ] `P-SD-2026-mfj` mfj 2026 = **32,200** - `RP25-32` §4.14(1), printed p. 18
- [ ] `P-SD-2026-qss` qss 2026 = **32,200** (shares the joint-return amount) - `RP25-32` §4.14(1), printed p. 18
- [ ] `P-SD-2026-hoh` hoh 2026 = **24,150** - `RP25-32` §4.14(1), printed p. 18
- [ ] `P-SD-2026-single` single 2026 = **16,100** - `RP25-32` §4.14(1), printed p. 18
- [ ] `P-SD-2026-mfs` mfs 2026 = **16,100** - `RP25-32` §4.14(1), printed p. 18
- [ ] `P-SD-SUPERSEDED` the withdrawn 2025 figures of `RP24-40` §2.15(1) (printed p. 12) appear nowhere in the vintage

### 3.2 Base amounts, base year, rounding

- [ ] `P-SD-BASE-mfj` mfj base = **31,500** - `USC63` DERIVED: §63(c)(2)(A), 200 percent of the amount under (C); not printed in the statute
- [ ] `P-SD-BASE-qss` qss base = **31,500** - `USC63` DERIVED: §63(c)(2)(A), as mfj
- [ ] `P-SD-BASE-hoh` hoh base = **23,625** - `USC63` §63(c)(2)(B) as substituted by §63(c)(7)(A)(i)
- [ ] `P-SD-BASE-single` single base = **15,750** - `USC63` §63(c)(2)(C) as substituted by §63(c)(7)(A)(ii)
- [ ] `P-SD-BASE-mfs` mfs base = **15,750** - `USC63` §63(c)(2)(C) ("in any other case") as substituted by §63(c)(7)(A)(ii)
- [ ] `P-SD-BY` base_year = **2024** ("by substituting '2024' for '2016'") - `USC63` §63(c)(7)(B)(ii)(II)
- [ ] `P-SD-FIRST` first_adjusted_year = **2026** ("a taxable year beginning after 2025") - `USC63` §63(c)(7)(B)(ii)
- [ ] `P-SD-INC` increment = **50** dollars, direction `down`, basis `IncreaseOverBase` - `USC63` §63(c)(7)(B)(ii), the flush sentence after subclause (II) (not §1(f)(7)(A), which `DOMAIN-MODEL.md` §15 cites)
- [ ] `P-SD-LAG` lag_years = **1** - `USC63` §63(c)(7)(B)(ii)(II) ("the cost-of-living adjustment determined under section 1(f)(3) for the calendar year in which the taxable year begins"), then `USC1` §1(f)(3)(A)(i) ("the C-CPI-U for the preceding calendar year") (decision B7)
- [ ] `P-SD-EFF` the §63(c) amendments apply to taxable years beginning after 2024-12-31 - `PL119-21` §70102(c), PDF pages 88-89
- [ ] `P-SD-ASOF` as_of = `2025-10-09` - `RP25-32` §1

---

## 4. Parameters: `params/index-series/cpi-chained-suur0000sa0.toml`

File sha256 at generation: `8a0b4fd2b55fa6af4cab93cb65bcdbff312f13587a342719e29bebad776a133e`

### 4.1 The averaging window

- [ ] `P-IX-NAME` the table declares `index_series = "cpi.chained.aug12m"`, the name both parameter tables read it under: the C-CPI-U (`cpi.chained`) averaged over the 12 months ending August - `USC1` §1(f)(6)(A),(B) (decisions B5, B7)
- [ ] `P-IX-WINDOW` 12-month period ending 8/31 (August 31), mean of the monthly values - `USC1` §1(f)(6)(B)
- [ ] `P-IX-LAG` the adjustment for a calendar year reads the C-CPI-U "for the preceding calendar year" - `USC1` §1(f)(3)(A)(i) (decision B7)
- [ ] `P-IX-SERIES` series id `SUUR0000SA0`, base period "DECEMBER 1999=100", not seasonally adjusted, all items, U.S. city average - `BLS-SERIES` row `SUUR0000SA0`; `BLS-FAQ` table 1 (decision B5)
- [ ] `P-IX-FINAL` every month in the four windows carries no footnote code in `BLS-SU` (codes `I` initial and `U` interim are defined in `BLS-FOOT`); the revision schedule is in `BLS-REL`, Technical Note and Table 5 footnote 1

### 4.2 Monthly observations

Read each in `BLS-SU`: the row whose series id is `SUUR0000SA0`, with the year and period (`M01`..`M12`) shown.

- [ ] `P-IX-2015-M09` 2015-M09 = **135.837** - `BLS-SU` (window for calendar year 2016)
- [ ] `P-IX-2015-M10` 2015-M10 = **135.735** - `BLS-SU` (window for calendar year 2016)
- [ ] `P-IX-2015-M11` 2015-M11 = **135.393** - `BLS-SU` (window for calendar year 2016)
- [ ] `P-IX-2015-M12` 2015-M12 = **134.788** - `BLS-SU` (window for calendar year 2016)
- [ ] `P-IX-2016-M01` 2016-M01 = **134.966** - `BLS-SU` (window for calendar year 2016)
- [ ] `P-IX-2016-M02` 2016-M02 = **134.953** - `BLS-SU` (window for calendar year 2016)
- [ ] `P-IX-2016-M03` 2016-M03 = **135.655** - `BLS-SU` (window for calendar year 2016)
- [ ] `P-IX-2016-M04` 2016-M04 = **136.332** - `BLS-SU` (window for calendar year 2016)
- [ ] `P-IX-2016-M05` 2016-M05 = **136.895** - `BLS-SU` (window for calendar year 2016)
- [ ] `P-IX-2016-M06` 2016-M06 = **137.329** - `BLS-SU` (window for calendar year 2016)
- [ ] `P-IX-2016-M07` 2016-M07 = **137.007** - `BLS-SU` (window for calendar year 2016)
- [ ] `P-IX-2016-M08` 2016-M08 = **137.026** - `BLS-SU` (window for calendar year 2016)
- [ ] `P-IX-SUM-2016` window_sum 2016 = **1631.916** (re-add the twelve values by hand; regenerated sum 1631.916)
- [ ] `P-IX-2016-M09` 2016-M09 = **137.328** - `BLS-SU` (window for calendar year 2017)
- [ ] `P-IX-2016-M10` 2016-M10 = **137.536** - `BLS-SU` (window for calendar year 2017)
- [ ] `P-IX-2016-M11` 2016-M11 = **137.253** - `BLS-SU` (window for calendar year 2017)
- [ ] `P-IX-2016-M12` 2016-M12 = **137.221** - `BLS-SU` (window for calendar year 2017)
- [ ] `P-IX-2017-M01` 2017-M01 = **138.035** - `BLS-SU` (window for calendar year 2017)
- [ ] `P-IX-2017-M02` 2017-M02 = **138.403** - `BLS-SU` (window for calendar year 2017)
- [ ] `P-IX-2017-M03` 2017-M03 = **138.461** - `BLS-SU` (window for calendar year 2017)
- [ ] `P-IX-2017-M04` 2017-M04 = **138.810** - `BLS-SU` (window for calendar year 2017)
- [ ] `P-IX-2017-M05` 2017-M05 = **138.922** - `BLS-SU` (window for calendar year 2017)
- [ ] `P-IX-2017-M06` 2017-M06 = **138.989** - `BLS-SU` (window for calendar year 2017)
- [ ] `P-IX-2017-M07` 2017-M07 = **138.755** - `BLS-SU` (window for calendar year 2017)
- [ ] `P-IX-2017-M08` 2017-M08 = **139.128** - `BLS-SU` (window for calendar year 2017)
- [ ] `P-IX-SUM-2017` window_sum 2017 = **1658.841** (re-add the twelve values by hand; regenerated sum 1658.841)
- [ ] `P-IX-2023-M09` 2023-M09 = **171.490** - `BLS-SU` (window for calendar year 2024)
- [ ] `P-IX-2023-M10` 2023-M10 = **171.421** - `BLS-SU` (window for calendar year 2024)
- [ ] `P-IX-2023-M11` 2023-M11 = **170.946** - `BLS-SU` (window for calendar year 2024)
- [ ] `P-IX-2023-M12` 2023-M12 = **170.718** - `BLS-SU` (window for calendar year 2024)
- [ ] `P-IX-2024-M01` 2024-M01 = **171.649** - `BLS-SU` (window for calendar year 2024)
- [ ] `P-IX-2024-M02` 2024-M02 = **172.700** - `BLS-SU` (window for calendar year 2024)
- [ ] `P-IX-2024-M03` 2024-M03 = **173.796** - `BLS-SU` (window for calendar year 2024)
- [ ] `P-IX-2024-M04` 2024-M04 = **174.424** - `BLS-SU` (window for calendar year 2024)
- [ ] `P-IX-2024-M05` 2024-M05 = **174.685** - `BLS-SU` (window for calendar year 2024)
- [ ] `P-IX-2024-M06` 2024-M06 = **174.721** - `BLS-SU` (window for calendar year 2024)
- [ ] `P-IX-2024-M07` 2024-M07 = **174.792** - `BLS-SU` (window for calendar year 2024)
- [ ] `P-IX-2024-M08` 2024-M08 = **174.848** - `BLS-SU` (window for calendar year 2024)
- [ ] `P-IX-SUM-2024` window_sum 2024 = **2076.190** (re-add the twelve values by hand; regenerated sum 2076.190)
- [ ] `P-IX-2024-M09` 2024-M09 = **175.099** - `BLS-SU` (window for calendar year 2025)
- [ ] `P-IX-2024-M10` 2024-M10 = **175.346** - `BLS-SU` (window for calendar year 2025)
- [ ] `P-IX-2024-M11` 2024-M11 = **175.180** - `BLS-SU` (window for calendar year 2025)
- [ ] `P-IX-2024-M12` 2024-M12 = **175.219** - `BLS-SU` (window for calendar year 2025)
- [ ] `P-IX-2025-M01` 2025-M01 = **176.362** - `BLS-SU` (window for calendar year 2025)
- [ ] `P-IX-2025-M02` 2025-M02 = **177.134** - `BLS-SU` (window for calendar year 2025)
- [ ] `P-IX-2025-M03` 2025-M03 = **177.520** - `BLS-SU` (window for calendar year 2025)
- [ ] `P-IX-2025-M04` 2025-M04 = **178.081** - `BLS-SU` (window for calendar year 2025)
- [ ] `P-IX-2025-M05` 2025-M05 = **178.427** - `BLS-SU` (window for calendar year 2025)
- [ ] `P-IX-2025-M06` 2025-M06 = **178.991** - `BLS-SU` (window for calendar year 2025)
- [ ] `P-IX-2025-M07` 2025-M07 = **179.272** - `BLS-SU` (window for calendar year 2025)
- [ ] `P-IX-2025-M08` 2025-M08 = **179.839** - `BLS-SU` (window for calendar year 2025)
- [ ] `P-IX-SUM-2025` window_sum 2025 = **2126.470** (re-add the twelve values by hand; regenerated sum 2126.470)

---

## 5. Fixtures: every expected value

For each file: confirm its `source` block names the archived document and the
sha256 in section 1, then work the rows. A value that a publication prints is
*transcribed* and promotes as `primary-source-confirmed`. A value this project
computed is *derived* and promotes as `hand-worked-reviewed`, which needs the
`derivation` and `reviewedBy` fields of `TESTING.md` §2.2 - a second reviewer who
is not the author, or the blind cooling-off re-derivation no sooner than seven
days later. **Re-derive; do not re-read.** A calculator and the tables in
sections 2-4 are enough for every row here.

### 5.1 `fixtures/pending/published/` - transcription inputs, not promotable

- [ ] `F-PUB-brackets-2025` `fixtures/pending/published/brackets-2025.json` (sha256 `13eeda22c9626911...`) holds the same values as sections 2.1, 2.2, 3.1 - source `params/provenance/irs/rp-24-40.pdf`, section 2.01, Tables 1-4, printed pages 5-6. `crates/pfp-params/tests/fixtures.rs` asserts the equality cell by cell; the read-back is the one already done in sections 2 and 3.
- [ ] `F-PUB-brackets-2026` `fixtures/pending/published/brackets-2026.json` (sha256 `2cf2825cbceadd77...`) holds the same values as sections 2.1, 2.2, 3.1 - source `params/provenance/irs/rp-25-32.pdf`, section 4.01, Tables 1-4, printed pages 10-12. `crates/pfp-params/tests/fixtures.rs` asserts the equality cell by cell; the read-back is the one already done in sections 2 and 3.
- [ ] `F-PUB-stdded-2025` `fixtures/pending/published/stdded-2025.json` (sha256 `7a90e71b7909ac9d...`) holds the same values as sections 2.1, 2.2, 3.1 - source `params/provenance/irs/rp-25-32.pdf`, section 3.01, printed page 9. `crates/pfp-params/tests/fixtures.rs` asserts the equality cell by cell; the read-back is the one already done in sections 2 and 3.
- [ ] `F-PUB-stdded-2026` `fixtures/pending/published/stdded-2026.json` (sha256 `c824ee6501027225...`) holds the same values as sections 2.1, 2.2, 3.1 - source `params/provenance/irs/rp-25-32.pdf`, section 4.14(1), printed page 18. `crates/pfp-params/tests/fixtures.rs` asserts the equality cell by cell; the read-back is the one already done in sections 2 and 3.
- [ ] `F-PUB-brackets-2025-cum-mfj` printed cumulative-tax column, mfj: **0, 2385, 11157, 35302, 80398, 114462, 202154.50** - same document and pages, the "The Tax Is" column (a transcription cross-check only; the engine never reads it)
- [ ] `F-PUB-brackets-2025-cum-hoh` printed cumulative-tax column, hoh: **0, 1700, 7442, 15912, 38460, 55484, 187031.50** - same document and pages, the "The Tax Is" column (a transcription cross-check only; the engine never reads it)
- [ ] `F-PUB-brackets-2025-cum-single` printed cumulative-tax column, single: **0, 1192.50, 5578.50, 17651, 40199, 57231, 188769.75** - same document and pages, the "The Tax Is" column (a transcription cross-check only; the engine never reads it)
- [ ] `F-PUB-brackets-2025-cum-mfs` printed cumulative-tax column, mfs: **0, 1192.50, 5578.50, 17651, 40199, 57231, 101077.25** - same document and pages, the "The Tax Is" column (a transcription cross-check only; the engine never reads it)
- [ ] `F-PUB-brackets-2026-cum-mfj` printed cumulative-tax column, mfj: **0, 2480, 11600, 35932, 82048, 116896, 206583.50** - same document and pages, the "The Tax Is" column (a transcription cross-check only; the engine never reads it)
- [ ] `F-PUB-brackets-2026-cum-hoh` printed cumulative-tax column, hoh: **0, 1770, 7740, 16155, 39207, 56631, 191171** - same document and pages, the "The Tax Is" column (a transcription cross-check only; the engine never reads it)
- [ ] `F-PUB-brackets-2026-cum-single` printed cumulative-tax column, single: **0, 1240, 5800, 17966, 41024, 58448, 192979.25** - same document and pages, the "The Tax Is" column (a transcription cross-check only; the engine never reads it)
- [ ] `F-PUB-brackets-2026-cum-mfs` printed cumulative-tax column, mfs: **0, 1240, 5800, 17966, 41024, 58448, 103291.75** - same document and pages, the "The Tax Is" column (a transcription cross-check only; the engine never reads it)

### 5.2 `fixtures/pending/schedule/` - rate-schedule grids (derived)

Expected tax = sum over brackets of rate x (income in the bracket), in cents, rounded once half-up to the whole dollar. "On an edge" is *not over* it. Work each row from the thresholds in section 2.1 or 2.2. Per-bracket tax is listed lowest bracket first, in dollars.

**`fixtures/pending/schedule/hoh-2025-grid.json`** (sha256 `80d883d01134804f...`) - promotes to `t1/schedule/hoh-2025-grid`; thresholds from `params/provenance/irs/rp-24-40.pdf`, section 2.01, Tables 1-4, printed pages 5-6

- [ ] `F-SCH-hoh-2025-grid-zero` taxable income 0.00: 0.00 = 0.00 -> tax **0**
- [ ] `F-SCH-hoh-2025-grid-inside-first-bracket` taxable income 8,500.00: 850.00 = 850.00 -> tax **850**
- [ ] `F-SCH-hoh-2025-grid-on-first-bracket-edge` taxable income 17,000.00: 1,700.00 = 1,700.00 -> tax **1,700**
- [ ] `F-SCH-hoh-2025-grid-one-dollar-above-first-edge` taxable income 17,001.00: 1,700.00 + 0.12 = 1,700.12 -> tax **1,700**
- [ ] `F-SCH-hoh-2025-grid-on-mid-table-edge` taxable income 103,350.00: 1,700.00 + 5,742.00 + 8,470.00 = 15,912.00 -> tax **15,912**
- [ ] `F-SCH-hoh-2025-grid-one-dollar-above-mid-table-edge` taxable income 103,351.00: 1,700.00 + 5,742.00 + 8,470.00 + 0.24 = 15,912.24 -> tax **15,912**
- [ ] `F-SCH-hoh-2025-grid-mid-table` taxable income 223,900.00: 1,700.00 + 5,742.00 + 8,470.00 + 22,548.00 + 8,512.00 = 46,972.00 -> tax **46,972**
- [ ] `F-SCH-hoh-2025-grid-top-bracket` taxable income 726,350.00: 1,700.00 + 5,742.00 + 8,470.00 + 22,548.00 + 17,024.00 + 131,547.50 + 37,000.00 = 224,031.50 -> tax **224,032**

**`fixtures/pending/schedule/hoh-2026-grid.json`** (sha256 `82a3d7ab4770487d...`) - promotes to `t1/schedule/hoh-2026-grid`; thresholds from `params/provenance/irs/rp-25-32.pdf`, section 4.01, Tables 1-4, printed pages 10-12

- [ ] `F-SCH-hoh-2026-grid-zero` taxable income 0.00: 0.00 = 0.00 -> tax **0**
- [ ] `F-SCH-hoh-2026-grid-inside-first-bracket` taxable income 8,850.00: 885.00 = 885.00 -> tax **885**
- [ ] `F-SCH-hoh-2026-grid-on-first-bracket-edge` taxable income 17,700.00: 1,770.00 = 1,770.00 -> tax **1,770**
- [ ] `F-SCH-hoh-2026-grid-one-dollar-above-first-edge` taxable income 17,701.00: 1,770.00 + 0.12 = 1,770.12 -> tax **1,770**
- [ ] `F-SCH-hoh-2026-grid-on-mid-table-edge` taxable income 105,700.00: 1,770.00 + 5,970.00 + 8,415.00 = 16,155.00 -> tax **16,155**
- [ ] `F-SCH-hoh-2026-grid-one-dollar-above-mid-table-edge` taxable income 105,701.00: 1,770.00 + 5,970.00 + 8,415.00 + 0.24 = 16,155.24 -> tax **16,155**
- [ ] `F-SCH-hoh-2026-grid-mid-table` taxable income 228,975.00: 1,770.00 + 5,970.00 + 8,415.00 + 23,052.00 + 8,712.00 = 47,919.00 -> tax **47,919**
- [ ] `F-SCH-hoh-2026-grid-top-bracket` taxable income 740,600.00: 1,770.00 + 5,970.00 + 8,415.00 + 23,052.00 + 17,424.00 + 134,540.00 + 37,000.00 = 228,171.00 -> tax **228,171**

**`fixtures/pending/schedule/mfj-2025-grid.json`** (sha256 `49bbb51cdae65a83...`) - promotes to `t1/schedule/mfj-2025-grid`; thresholds from `params/provenance/irs/rp-24-40.pdf`, section 2.01, Tables 1-4, printed pages 5-6

- [ ] `F-SCH-mfj-2025-grid-zero` taxable income 0.00: 0.00 = 0.00 -> tax **0**
- [ ] `F-SCH-mfj-2025-grid-inside-first-bracket` taxable income 11,925.00: 1,192.50 = 1,192.50 -> tax **1,193**
- [ ] `F-SCH-mfj-2025-grid-on-first-bracket-edge` taxable income 23,850.00: 2,385.00 = 2,385.00 -> tax **2,385**
- [ ] `F-SCH-mfj-2025-grid-one-dollar-above-first-edge` taxable income 23,851.00: 2,385.00 + 0.12 = 2,385.12 -> tax **2,385**
- [ ] `F-SCH-mfj-2025-grid-on-mid-table-edge` taxable income 206,700.00: 2,385.00 + 8,772.00 + 24,145.00 = 35,302.00 -> tax **35,302**
- [ ] `F-SCH-mfj-2025-grid-one-dollar-above-mid-table-edge` taxable income 206,701.00: 2,385.00 + 8,772.00 + 24,145.00 + 0.24 = 35,302.24 -> tax **35,302**
- [ ] `F-SCH-mfj-2025-grid-mid-table` taxable income 447,825.00: 2,385.00 + 8,772.00 + 24,145.00 + 45,096.00 + 17,032.00 = 97,430.00 -> tax **97,430**
- [ ] `F-SCH-mfj-2025-grid-top-bracket` taxable income 851,600.00: 2,385.00 + 8,772.00 + 24,145.00 + 45,096.00 + 34,064.00 + 87,692.50 + 37,000.00 = 239,154.50 -> tax **239,155**

**`fixtures/pending/schedule/mfj-2026-grid.json`** (sha256 `18005b11693f2ea8...`) - promotes to `t1/schedule/mfj-2026-grid`; thresholds from `params/provenance/irs/rp-25-32.pdf`, section 4.01, Tables 1-4, printed pages 10-12

- [ ] `F-SCH-mfj-2026-grid-zero` taxable income 0.00: 0.00 = 0.00 -> tax **0**
- [ ] `F-SCH-mfj-2026-grid-inside-first-bracket` taxable income 12,400.00: 1,240.00 = 1,240.00 -> tax **1,240**
- [ ] `F-SCH-mfj-2026-grid-on-first-bracket-edge` taxable income 24,800.00: 2,480.00 = 2,480.00 -> tax **2,480**
- [ ] `F-SCH-mfj-2026-grid-one-dollar-above-first-edge` taxable income 24,801.00: 2,480.00 + 0.12 = 2,480.12 -> tax **2,480**
- [ ] `F-SCH-mfj-2026-grid-on-mid-table-edge` taxable income 211,400.00: 2,480.00 + 9,120.00 + 24,332.00 = 35,932.00 -> tax **35,932**
- [ ] `F-SCH-mfj-2026-grid-one-dollar-above-mid-table-edge` taxable income 211,401.00: 2,480.00 + 9,120.00 + 24,332.00 + 0.24 = 35,932.24 -> tax **35,932**
- [ ] `F-SCH-mfj-2026-grid-mid-table` taxable income 458,000.00: 2,480.00 + 9,120.00 + 24,332.00 + 46,116.00 + 17,424.00 = 99,472.00 -> tax **99,472**
- [ ] `F-SCH-mfj-2026-grid-top-bracket` taxable income 868,700.00: 2,480.00 + 9,120.00 + 24,332.00 + 46,116.00 + 34,848.00 + 89,687.50 + 37,000.00 = 243,583.50 -> tax **243,584**

**`fixtures/pending/schedule/mfj-2026-plan-acceptance.json`** (sha256 `091dd02293d25a94...`) - promotes to `t1/schedule/mfj-2026-plan-acceptance`; thresholds from `params/provenance/irs/rp-25-32.pdf`, section 4.01, Tables 1-4, printed pages 10-12

- [ ] `F-SCH-mfj-2026-plan-acceptance-plan-acceptance-1` taxable income 100,000.00: 2,480.00 + 9,024.00 = 11,504.00 -> tax **11,504**
- [ ] `F-SCH-mfj-2026-plan-acceptance-plan-acceptance-2` taxable income 150,000.00: 2,480.00 + 9,120.00 + 10,824.00 = 22,424.00 -> tax **22,424**
- [ ] `F-SCH-mfj-2026-plan-acceptance-plan-acceptance-3` taxable income 250,000.00: 2,480.00 + 9,120.00 + 24,332.00 + 9,264.00 = 45,196.00 -> tax **45,196**

**`fixtures/pending/schedule/mfs-2025-grid.json`** (sha256 `f7c599906a006df9...`) - promotes to `t1/schedule/mfs-2025-grid`; thresholds from `params/provenance/irs/rp-24-40.pdf`, section 2.01, Tables 1-4, printed pages 5-6

- [ ] `F-SCH-mfs-2025-grid-zero` taxable income 0.00: 0.00 = 0.00 -> tax **0**
- [ ] `F-SCH-mfs-2025-grid-inside-first-bracket` taxable income 5,962.00: 596.20 = 596.20 -> tax **596**
- [ ] `F-SCH-mfs-2025-grid-on-first-bracket-edge` taxable income 11,925.00: 1,192.50 = 1,192.50 -> tax **1,193**
- [ ] `F-SCH-mfs-2025-grid-one-dollar-above-first-edge` taxable income 11,926.00: 1,192.50 + 0.12 = 1,192.62 -> tax **1,193**
- [ ] `F-SCH-mfs-2025-grid-on-mid-table-edge` taxable income 103,350.00: 1,192.50 + 4,386.00 + 12,072.50 = 17,651.00 -> tax **17,651**
- [ ] `F-SCH-mfs-2025-grid-one-dollar-above-mid-table-edge` taxable income 103,351.00: 1,192.50 + 4,386.00 + 12,072.50 + 0.24 = 17,651.24 -> tax **17,651**
- [ ] `F-SCH-mfs-2025-grid-mid-table` taxable income 223,912.00: 1,192.50 + 4,386.00 + 12,072.50 + 22,548.00 + 8,515.84 = 48,714.84 -> tax **48,715**
- [ ] `F-SCH-mfs-2025-grid-top-bracket` taxable income 475,800.00: 1,192.50 + 4,386.00 + 12,072.50 + 22,548.00 + 17,032.00 + 43,846.25 + 37,000.00 = 138,077.25 -> tax **138,077**

**`fixtures/pending/schedule/mfs-2026-grid.json`** (sha256 `ce8a1b47e0a36b04...`) - promotes to `t1/schedule/mfs-2026-grid`; thresholds from `params/provenance/irs/rp-25-32.pdf`, section 4.01, Tables 1-4, printed pages 10-12

- [ ] `F-SCH-mfs-2026-grid-zero` taxable income 0.00: 0.00 = 0.00 -> tax **0**
- [ ] `F-SCH-mfs-2026-grid-inside-first-bracket` taxable income 6,200.00: 620.00 = 620.00 -> tax **620**
- [ ] `F-SCH-mfs-2026-grid-on-first-bracket-edge` taxable income 12,400.00: 1,240.00 = 1,240.00 -> tax **1,240**
- [ ] `F-SCH-mfs-2026-grid-one-dollar-above-first-edge` taxable income 12,401.00: 1,240.00 + 0.12 = 1,240.12 -> tax **1,240**
- [ ] `F-SCH-mfs-2026-grid-on-mid-table-edge` taxable income 105,700.00: 1,240.00 + 4,560.00 + 12,166.00 = 17,966.00 -> tax **17,966**
- [ ] `F-SCH-mfs-2026-grid-one-dollar-above-mid-table-edge` taxable income 105,701.00: 1,240.00 + 4,560.00 + 12,166.00 + 0.24 = 17,966.24 -> tax **17,966**
- [ ] `F-SCH-mfs-2026-grid-mid-table` taxable income 229,000.00: 1,240.00 + 4,560.00 + 12,166.00 + 23,058.00 + 8,712.00 = 49,736.00 -> tax **49,736**
- [ ] `F-SCH-mfs-2026-grid-top-bracket` taxable income 484,350.00: 1,240.00 + 4,560.00 + 12,166.00 + 23,058.00 + 17,424.00 + 44,843.75 + 37,000.00 = 140,291.75 -> tax **140,292**

**`fixtures/pending/schedule/qss-2025-grid.json`** (sha256 `adfee3ffaa9fc266...`) - promotes to `t1/schedule/qss-2025-grid`; thresholds from `params/provenance/irs/rp-24-40.pdf`, section 2.01, Tables 1-4, printed pages 5-6

- [ ] `F-SCH-qss-2025-grid-zero` taxable income 0.00: 0.00 = 0.00 -> tax **0**
- [ ] `F-SCH-qss-2025-grid-inside-first-bracket` taxable income 11,925.00: 1,192.50 = 1,192.50 -> tax **1,193**
- [ ] `F-SCH-qss-2025-grid-on-first-bracket-edge` taxable income 23,850.00: 2,385.00 = 2,385.00 -> tax **2,385**
- [ ] `F-SCH-qss-2025-grid-one-dollar-above-first-edge` taxable income 23,851.00: 2,385.00 + 0.12 = 2,385.12 -> tax **2,385**
- [ ] `F-SCH-qss-2025-grid-on-mid-table-edge` taxable income 206,700.00: 2,385.00 + 8,772.00 + 24,145.00 = 35,302.00 -> tax **35,302**
- [ ] `F-SCH-qss-2025-grid-one-dollar-above-mid-table-edge` taxable income 206,701.00: 2,385.00 + 8,772.00 + 24,145.00 + 0.24 = 35,302.24 -> tax **35,302**
- [ ] `F-SCH-qss-2025-grid-mid-table` taxable income 447,825.00: 2,385.00 + 8,772.00 + 24,145.00 + 45,096.00 + 17,032.00 = 97,430.00 -> tax **97,430**
- [ ] `F-SCH-qss-2025-grid-top-bracket` taxable income 851,600.00: 2,385.00 + 8,772.00 + 24,145.00 + 45,096.00 + 34,064.00 + 87,692.50 + 37,000.00 = 239,154.50 -> tax **239,155**

**`fixtures/pending/schedule/qss-2026-grid.json`** (sha256 `706ee70a5fa10553...`) - promotes to `t1/schedule/qss-2026-grid`; thresholds from `params/provenance/irs/rp-25-32.pdf`, section 4.01, Tables 1-4, printed pages 10-12

- [ ] `F-SCH-qss-2026-grid-zero` taxable income 0.00: 0.00 = 0.00 -> tax **0**
- [ ] `F-SCH-qss-2026-grid-inside-first-bracket` taxable income 12,400.00: 1,240.00 = 1,240.00 -> tax **1,240**
- [ ] `F-SCH-qss-2026-grid-on-first-bracket-edge` taxable income 24,800.00: 2,480.00 = 2,480.00 -> tax **2,480**
- [ ] `F-SCH-qss-2026-grid-one-dollar-above-first-edge` taxable income 24,801.00: 2,480.00 + 0.12 = 2,480.12 -> tax **2,480**
- [ ] `F-SCH-qss-2026-grid-on-mid-table-edge` taxable income 211,400.00: 2,480.00 + 9,120.00 + 24,332.00 = 35,932.00 -> tax **35,932**
- [ ] `F-SCH-qss-2026-grid-one-dollar-above-mid-table-edge` taxable income 211,401.00: 2,480.00 + 9,120.00 + 24,332.00 + 0.24 = 35,932.24 -> tax **35,932**
- [ ] `F-SCH-qss-2026-grid-mid-table` taxable income 458,000.00: 2,480.00 + 9,120.00 + 24,332.00 + 46,116.00 + 17,424.00 = 99,472.00 -> tax **99,472**
- [ ] `F-SCH-qss-2026-grid-top-bracket` taxable income 868,700.00: 2,480.00 + 9,120.00 + 24,332.00 + 46,116.00 + 34,848.00 + 89,687.50 + 37,000.00 = 243,583.50 -> tax **243,584**

**`fixtures/pending/schedule/single-2025-grid.json`** (sha256 `2e28aaf82d2cb99e...`) - promotes to `t1/schedule/single-2025-grid`; thresholds from `params/provenance/irs/rp-24-40.pdf`, section 2.01, Tables 1-4, printed pages 5-6

- [ ] `F-SCH-single-2025-grid-zero` taxable income 0.00: 0.00 = 0.00 -> tax **0**
- [ ] `F-SCH-single-2025-grid-inside-first-bracket` taxable income 5,962.00: 596.20 = 596.20 -> tax **596**
- [ ] `F-SCH-single-2025-grid-on-first-bracket-edge` taxable income 11,925.00: 1,192.50 = 1,192.50 -> tax **1,193**
- [ ] `F-SCH-single-2025-grid-one-dollar-above-first-edge` taxable income 11,926.00: 1,192.50 + 0.12 = 1,192.62 -> tax **1,193**
- [ ] `F-SCH-single-2025-grid-on-mid-table-edge` taxable income 103,350.00: 1,192.50 + 4,386.00 + 12,072.50 = 17,651.00 -> tax **17,651**
- [ ] `F-SCH-single-2025-grid-one-dollar-above-mid-table-edge` taxable income 103,351.00: 1,192.50 + 4,386.00 + 12,072.50 + 0.24 = 17,651.24 -> tax **17,651**
- [ ] `F-SCH-single-2025-grid-mid-table` taxable income 223,912.00: 1,192.50 + 4,386.00 + 12,072.50 + 22,548.00 + 8,515.84 = 48,714.84 -> tax **48,715**
- [ ] `F-SCH-single-2025-grid-top-bracket` taxable income 726,350.00: 1,192.50 + 4,386.00 + 12,072.50 + 22,548.00 + 17,032.00 + 131,538.75 + 37,000.00 = 225,769.75 -> tax **225,770**

**`fixtures/pending/schedule/single-2026-grid.json`** (sha256 `7393ab7adae8d8ff...`) - promotes to `t1/schedule/single-2026-grid`; thresholds from `params/provenance/irs/rp-25-32.pdf`, section 4.01, Tables 1-4, printed pages 10-12

- [ ] `F-SCH-single-2026-grid-zero` taxable income 0.00: 0.00 = 0.00 -> tax **0**
- [ ] `F-SCH-single-2026-grid-inside-first-bracket` taxable income 6,200.00: 620.00 = 620.00 -> tax **620**
- [ ] `F-SCH-single-2026-grid-on-first-bracket-edge` taxable income 12,400.00: 1,240.00 = 1,240.00 -> tax **1,240**
- [ ] `F-SCH-single-2026-grid-one-dollar-above-first-edge` taxable income 12,401.00: 1,240.00 + 0.12 = 1,240.12 -> tax **1,240**
- [ ] `F-SCH-single-2026-grid-on-mid-table-edge` taxable income 105,700.00: 1,240.00 + 4,560.00 + 12,166.00 = 17,966.00 -> tax **17,966**
- [ ] `F-SCH-single-2026-grid-one-dollar-above-mid-table-edge` taxable income 105,701.00: 1,240.00 + 4,560.00 + 12,166.00 + 0.24 = 17,966.24 -> tax **17,966**
- [ ] `F-SCH-single-2026-grid-mid-table` taxable income 229,000.00: 1,240.00 + 4,560.00 + 12,166.00 + 23,058.00 + 8,712.00 = 49,736.00 -> tax **49,736**
- [ ] `F-SCH-single-2026-grid-top-bracket` taxable income 740,600.00: 1,240.00 + 4,560.00 + 12,166.00 + 23,058.00 + 17,424.00 + 134,531.25 + 37,000.00 = 229,979.25 -> tax **229,979**

### 5.3 `fixtures/pending/uprating/` - statutory pipeline against the publication (derived and transcribed)

Blocked by decision B1 (and B2 for the standard deduction). For each row re-derive: factor = window_sum(tax year - 1) / window_sum(base year) from section 4; raw increase = base x (factor - 1); floor it to the increment; add the base. `published` is the value already read in section 2 or 3. The residual is recorded, never tuned.

**`fixtures/pending/uprating/2026-brackets.json`** (sha256 `b670182850221cc6...`) - base amounts `params/provenance/usc/usc26-s1.html`, 1(j)(2)(A)-(D) base amounts; 1(j)(3)(B) base years; 1(f)(3),(6),(7); reproduced 6 of 30

- [ ] `F-UP-2026-brackets-mfj-top10` base 19,050 (2016), factor - 1 = 247277/815958, raw increase 5773.124168, floored to 50: 5,750 -> computed **24,800**, published 24,800, residual **+0**
- [ ] `F-UP-2026-brackets-mfj-top12` base 77,400 (2016), factor - 1 = 247277/815958, raw increase 23456.158038, floored to 50: 23,450 -> computed **100,850**, published 100,800, residual **+50**
- [ ] `F-UP-2026-brackets-mfj-top22` base 165,000 (2017), factor - 1 = 467629/1658841, raw increase 46513.671292, floored to 50: 46,500 -> computed **211,500**, published 211,400, residual **+100**
- [ ] `F-UP-2026-brackets-mfj-top24` base 315,000 (2017), factor - 1 = 467629/1658841, raw increase 88798.827012, floored to 50: 88,750 -> computed **403,750**, published 403,550, residual **+200**
- [ ] `F-UP-2026-brackets-mfj-top32` base 400,000 (2017), factor - 1 = 467629/1658841, raw increase 112760.415254, floored to 50: 112,750 -> computed **512,750**, published 512,450, residual **+300**
- [ ] `F-UP-2026-brackets-mfj-top35` base 600,000 (2017), factor - 1 = 467629/1658841, raw increase 169140.622881, floored to 50: 169,100 -> computed **769,100**, published 768,700, residual **+400**
- [ ] `F-UP-2026-brackets-qss-top10` base 19,050 (2016), factor - 1 = 247277/815958, raw increase 5773.124168, floored to 50: 5,750 -> computed **24,800**, published 24,800, residual **+0**
- [ ] `F-UP-2026-brackets-qss-top12` base 77,400 (2016), factor - 1 = 247277/815958, raw increase 23456.158038, floored to 50: 23,450 -> computed **100,850**, published 100,800, residual **+50**
- [ ] `F-UP-2026-brackets-qss-top22` base 165,000 (2017), factor - 1 = 467629/1658841, raw increase 46513.671292, floored to 50: 46,500 -> computed **211,500**, published 211,400, residual **+100**
- [ ] `F-UP-2026-brackets-qss-top24` base 315,000 (2017), factor - 1 = 467629/1658841, raw increase 88798.827012, floored to 50: 88,750 -> computed **403,750**, published 403,550, residual **+200**
- [ ] `F-UP-2026-brackets-qss-top32` base 400,000 (2017), factor - 1 = 467629/1658841, raw increase 112760.415254, floored to 50: 112,750 -> computed **512,750**, published 512,450, residual **+300**
- [ ] `F-UP-2026-brackets-qss-top35` base 600,000 (2017), factor - 1 = 467629/1658841, raw increase 169140.622881, floored to 50: 169,100 -> computed **769,100**, published 768,700, residual **+400**
- [ ] `F-UP-2026-brackets-hoh-top10` base 13,600 (2016), factor - 1 = 247277/815958, raw increase 4121.495469, floored to 50: 4,100 -> computed **17,700**, published 17,700, residual **+0**
- [ ] `F-UP-2026-brackets-hoh-top12` base 51,800 (2016), factor - 1 = 247277/815958, raw increase 15698.048919, floored to 50: 15,650 -> computed **67,450**, published 67,450, residual **+0**
- [ ] `F-UP-2026-brackets-hoh-top22` base 82,500 (2017), factor - 1 = 467629/1658841, raw increase 23256.835646, floored to 50: 23,250 -> computed **105,750**, published 105,700, residual **+50**
- [ ] `F-UP-2026-brackets-hoh-top24` base 157,500 (2017), factor - 1 = 467629/1658841, raw increase 44399.413506, floored to 50: 44,350 -> computed **201,850**, published 201,750, residual **+100**
- [ ] `F-UP-2026-brackets-hoh-top32` base 200,000 (2017), factor - 1 = 467629/1658841, raw increase 56380.207627, floored to 50: 56,350 -> computed **256,350**, published 256,200, residual **+150**
- [ ] `F-UP-2026-brackets-hoh-top35` base 500,000 (2017), factor - 1 = 467629/1658841, raw increase 140950.519067, floored to 50: 140,950 -> computed **640,950**, published 640,600, residual **+350**
- [ ] `F-UP-2026-brackets-single-top10` base 9,525 (2016), factor - 1 = 247277/815958, raw increase 2886.562084, floored to 25: 2,875 -> computed **12,400**, published 12,400, residual **+0**
- [ ] `F-UP-2026-brackets-single-top12` base 38,700 (2016), factor - 1 = 247277/815958, raw increase 11728.079019, floored to 25: 11,725 -> computed **50,425**, published 50,400, residual **+25**
- [ ] `F-UP-2026-brackets-single-top22` base 82,500 (2017), factor - 1 = 467629/1658841, raw increase 23256.835646, floored to 25: 23,250 -> computed **105,750**, published 105,700, residual **+50**
- [ ] `F-UP-2026-brackets-single-top24` base 157,500 (2017), factor - 1 = 467629/1658841, raw increase 44399.413506, floored to 25: 44,375 -> computed **201,875**, published 201,775, residual **+100**
- [ ] `F-UP-2026-brackets-single-top32` base 200,000 (2017), factor - 1 = 467629/1658841, raw increase 56380.207627, floored to 25: 56,375 -> computed **256,375**, published 256,225, residual **+150**
- [ ] `F-UP-2026-brackets-single-top35` base 500,000 (2017), factor - 1 = 467629/1658841, raw increase 140950.519067, floored to 25: 140,950 -> computed **640,950**, published 640,600, residual **+350**
- [ ] `F-UP-2026-brackets-mfs-top10` base 9,525 (2016), factor - 1 = 247277/815958, raw increase 2886.562084, floored to 25: 2,875 -> computed **12,400**, published 12,400, residual **+0**
- [ ] `F-UP-2026-brackets-mfs-top12` base 38,700 (2016), factor - 1 = 247277/815958, raw increase 11728.079019, floored to 25: 11,725 -> computed **50,425**, published 50,400, residual **+25**
- [ ] `F-UP-2026-brackets-mfs-top22` base 82,500 (2017), factor - 1 = 467629/1658841, raw increase 23256.835646, floored to 25: 23,250 -> computed **105,750**, published 105,700, residual **+50**
- [ ] `F-UP-2026-brackets-mfs-top24` base 157,500 (2017), factor - 1 = 467629/1658841, raw increase 44399.413506, floored to 25: 44,375 -> computed **201,875**, published 201,775, residual **+100**
- [ ] `F-UP-2026-brackets-mfs-top32` base 200,000 (2017), factor - 1 = 467629/1658841, raw increase 56380.207627, floored to 25: 56,375 -> computed **256,375**, published 256,225, residual **+150**
- [ ] `F-UP-2026-brackets-mfs-top35` base 300,000 (2017), factor - 1 = 467629/1658841, raw increase 84570.311440, floored to 25: 84,550 -> computed **384,550**, published 384,350, residual **+200**

**`fixtures/pending/uprating/2025-brackets-control.json`** (sha256 `80b88495469ead82...`) - NOT promotable (`promotesTo` is null): the positive control; keep it - base amounts `params/provenance/usc/usc26-s1.html`, 1(j)(2)(A)-(D) base amounts; 1(j)(3)(B) base years; 1(f)(3),(6),(7); reproduced 1 of 30

- [ ] `F-UP-2025-brackets-control-mfj-top10` base 19,050 (2017), factor - 1 = 417349/1658841, raw increase 4792.803198, floored to 50: 4,750 -> computed **23,800**, published 23,850, residual **-50**
- [ ] `F-UP-2025-brackets-control-mfj-top12` base 77,400 (2017), factor - 1 = 417349/1658841, raw increase 19473.121655, floored to 50: 19,450 -> computed **96,850**, published 96,950, residual **-100**
- [ ] `F-UP-2025-brackets-control-mfj-top22` base 165,000 (2017), factor - 1 = 417349/1658841, raw increase 41512.468645, floored to 50: 41,500 -> computed **206,500**, published 206,700, residual **-200**
- [ ] `F-UP-2025-brackets-control-mfj-top24` base 315,000 (2017), factor - 1 = 417349/1658841, raw increase 79251.076505, floored to 50: 79,250 -> computed **394,250**, published 394,600, residual **-350**
- [ ] `F-UP-2025-brackets-control-mfj-top32` base 400,000 (2017), factor - 1 = 417349/1658841, raw increase 100636.287625, floored to 50: 100,600 -> computed **500,600**, published 501,050, residual **-450**
- [ ] `F-UP-2025-brackets-control-mfj-top35` base 600,000 (2017), factor - 1 = 417349/1658841, raw increase 150954.431437, floored to 50: 150,950 -> computed **750,950**, published 751,600, residual **-650**
- [ ] `F-UP-2025-brackets-control-qss-top10` base 19,050 (2017), factor - 1 = 417349/1658841, raw increase 4792.803198, floored to 50: 4,750 -> computed **23,800**, published 23,850, residual **-50**
- [ ] `F-UP-2025-brackets-control-qss-top12` base 77,400 (2017), factor - 1 = 417349/1658841, raw increase 19473.121655, floored to 50: 19,450 -> computed **96,850**, published 96,950, residual **-100**
- [ ] `F-UP-2025-brackets-control-qss-top22` base 165,000 (2017), factor - 1 = 417349/1658841, raw increase 41512.468645, floored to 50: 41,500 -> computed **206,500**, published 206,700, residual **-200**
- [ ] `F-UP-2025-brackets-control-qss-top24` base 315,000 (2017), factor - 1 = 417349/1658841, raw increase 79251.076505, floored to 50: 79,250 -> computed **394,250**, published 394,600, residual **-350**
- [ ] `F-UP-2025-brackets-control-qss-top32` base 400,000 (2017), factor - 1 = 417349/1658841, raw increase 100636.287625, floored to 50: 100,600 -> computed **500,600**, published 501,050, residual **-450**
- [ ] `F-UP-2025-brackets-control-qss-top35` base 600,000 (2017), factor - 1 = 417349/1658841, raw increase 150954.431437, floored to 50: 150,950 -> computed **750,950**, published 751,600, residual **-650**
- [ ] `F-UP-2025-brackets-control-hoh-top10` base 13,600 (2017), factor - 1 = 417349/1658841, raw increase 3421.633779, floored to 50: 3,400 -> computed **17,000**, published 17,000, residual **+0**
- [ ] `F-UP-2025-brackets-control-hoh-top12` base 51,800 (2017), factor - 1 = 417349/1658841, raw increase 13032.399247, floored to 50: 13,000 -> computed **64,800**, published 64,850, residual **-50**
- [ ] `F-UP-2025-brackets-control-hoh-top22` base 82,500 (2017), factor - 1 = 417349/1658841, raw increase 20756.234323, floored to 50: 20,750 -> computed **103,250**, published 103,350, residual **-100**
- [ ] `F-UP-2025-brackets-control-hoh-top24` base 157,500 (2017), factor - 1 = 417349/1658841, raw increase 39625.538252, floored to 50: 39,600 -> computed **197,100**, published 197,300, residual **-200**
- [ ] `F-UP-2025-brackets-control-hoh-top32` base 200,000 (2017), factor - 1 = 417349/1658841, raw increase 50318.143812, floored to 50: 50,300 -> computed **250,300**, published 250,500, residual **-200**
- [ ] `F-UP-2025-brackets-control-hoh-top35` base 500,000 (2017), factor - 1 = 417349/1658841, raw increase 125795.359531, floored to 50: 125,750 -> computed **625,750**, published 626,350, residual **-600**
- [ ] `F-UP-2025-brackets-control-single-top10` base 9,525 (2017), factor - 1 = 417349/1658841, raw increase 2396.401599, floored to 25: 2,375 -> computed **11,900**, published 11,925, residual **-25**
- [ ] `F-UP-2025-brackets-control-single-top12` base 38,700 (2017), factor - 1 = 417349/1658841, raw increase 9736.560828, floored to 25: 9,725 -> computed **48,425**, published 48,475, residual **-50**
- [ ] `F-UP-2025-brackets-control-single-top22` base 82,500 (2017), factor - 1 = 417349/1658841, raw increase 20756.234323, floored to 25: 20,750 -> computed **103,250**, published 103,350, residual **-100**
- [ ] `F-UP-2025-brackets-control-single-top24` base 157,500 (2017), factor - 1 = 417349/1658841, raw increase 39625.538252, floored to 25: 39,625 -> computed **197,125**, published 197,300, residual **-175**
- [ ] `F-UP-2025-brackets-control-single-top32` base 200,000 (2017), factor - 1 = 417349/1658841, raw increase 50318.143812, floored to 25: 50,300 -> computed **250,300**, published 250,525, residual **-225**
- [ ] `F-UP-2025-brackets-control-single-top35` base 500,000 (2017), factor - 1 = 417349/1658841, raw increase 125795.359531, floored to 25: 125,775 -> computed **625,775**, published 626,350, residual **-575**
- [ ] `F-UP-2025-brackets-control-mfs-top10` base 9,525 (2017), factor - 1 = 417349/1658841, raw increase 2396.401599, floored to 25: 2,375 -> computed **11,900**, published 11,925, residual **-25**
- [ ] `F-UP-2025-brackets-control-mfs-top12` base 38,700 (2017), factor - 1 = 417349/1658841, raw increase 9736.560828, floored to 25: 9,725 -> computed **48,425**, published 48,475, residual **-50**
- [ ] `F-UP-2025-brackets-control-mfs-top22` base 82,500 (2017), factor - 1 = 417349/1658841, raw increase 20756.234323, floored to 25: 20,750 -> computed **103,250**, published 103,350, residual **-100**
- [ ] `F-UP-2025-brackets-control-mfs-top24` base 157,500 (2017), factor - 1 = 417349/1658841, raw increase 39625.538252, floored to 25: 39,625 -> computed **197,125**, published 197,300, residual **-175**
- [ ] `F-UP-2025-brackets-control-mfs-top32` base 200,000 (2017), factor - 1 = 417349/1658841, raw increase 50318.143812, floored to 25: 50,300 -> computed **250,300**, published 250,525, residual **-225**
- [ ] `F-UP-2025-brackets-control-mfs-top35` base 300,000 (2017), factor - 1 = 417349/1658841, raw increase 75477.215719, floored to 25: 75,475 -> computed **375,475**, published 375,800, residual **-325**

**`fixtures/pending/uprating/2026-stdded.json`** (sha256 `caf5437e1d465bf0...`) - base amounts `params/provenance/usc/usc26-s63.html`, 63(c)(2), 63(c)(7)(A) base amounts, 63(c)(7)(B)(ii) base year and rounding; reproduced 4 of 5

- [ ] `F-UP-2026-stdded-single-amount` base 15,750 (2024), factor - 1 = 5028/207619, raw increase 381.424629, floored to 50: 350 -> computed **16,100**, published 16,100, residual **+0**; floor-the-total alternative 16,100
- [ ] `F-UP-2026-stdded-mfs-amount` base 15,750 (2024), factor - 1 = 5028/207619, raw increase 381.424629, floored to 50: 350 -> computed **16,100**, published 16,100, residual **+0**; floor-the-total alternative 16,100
- [ ] `F-UP-2026-stdded-hoh-amount` base 23,625 (2024), factor - 1 = 5028/207619, raw increase 572.136943, floored to 50: 550 -> computed **24,175**, published 24,150, residual **+25**; floor-the-total alternative 24,150
- [ ] `F-UP-2026-stdded-mfj-amount` derived, 200 percent of the rounded single result (26 USC 63(c)(2)(A)) -> computed **32,200**, published 32,200, residual **+0**
- [ ] `F-UP-2026-stdded-qss-amount` derived, 200 percent of the rounded single result (26 USC 63(c)(2)(A)) -> computed **32,200**, published 32,200, residual **+0**

**`fixtures/pending/uprating/chained-path-divergence.json`** (sha256 `c0102cd5230ddfc3...`) - the negative control: a chain from the 2025 *published* value must differ from the statutory result

- [ ] `F-CH-mfj-top10` (asserted) from 2025 published 23,850: chained **24,400**, statutory 24,800, published 24,800
- [ ] `F-CH-qss-top10` (asserted) from 2025 published 23,850: chained **24,400**, statutory 24,800, published 24,800
- [ ] `F-CH-hoh-top10` (asserted) from 2025 published 17,000: chained **17,400**, statutory 17,700, published 17,700
- [ ] `F-CH-hoh-top12` (asserted) from 2025 published 64,850: chained **66,400**, statutory 67,450, published 67,450
- [ ] `F-CH-single-top10` (asserted) from 2025 published 11,925: chained **12,200**, statutory 12,400, published 12,400
- [ ] `F-CH-mfs-top10` (asserted) from 2025 published 11,925: chained **12,200**, statutory 12,400, published 12,400
- [ ] `F-CH-mfj-top12` (context, not asserted) from 2025 published 96,950: chained **99,250**, statutory 100,850, published 100,800
- [ ] `F-CH-mfj-top22` (context, not asserted) from 2025 published 206,700: chained **211,700**, statutory 211,500, published 211,400
- [ ] `F-CH-mfj-top24` (context, not asserted) from 2025 published 394,600: chained **404,150**, statutory 403,750, published 403,550
- [ ] `F-CH-mfj-top32` (context, not asserted) from 2025 published 501,050: chained **513,150**, statutory 512,750, published 512,450
- [ ] `F-CH-mfj-top35` (context, not asserted) from 2025 published 751,600: chained **769,800**, statutory 769,100, published 768,700
- [ ] `F-CH-qss-top12` (context, not asserted) from 2025 published 96,950: chained **99,250**, statutory 100,850, published 100,800
- [ ] `F-CH-qss-top22` (context, not asserted) from 2025 published 206,700: chained **211,700**, statutory 211,500, published 211,400
- [ ] `F-CH-qss-top24` (context, not asserted) from 2025 published 394,600: chained **404,150**, statutory 403,750, published 403,550
- [ ] `F-CH-qss-top32` (context, not asserted) from 2025 published 501,050: chained **513,150**, statutory 512,750, published 512,450
- [ ] `F-CH-qss-top35` (context, not asserted) from 2025 published 751,600: chained **769,800**, statutory 769,100, published 768,700
- [ ] `F-CH-hoh-top22` (context, not asserted) from 2025 published 103,350: chained **105,850**, statutory 105,750, published 105,700
- [ ] `F-CH-hoh-top24` (context, not asserted) from 2025 published 197,300: chained **202,050**, statutory 201,850, published 201,750
- [ ] `F-CH-hoh-top32` (context, not asserted) from 2025 published 250,500: chained **256,550**, statutory 256,350, published 256,200
- [ ] `F-CH-hoh-top35` (context, not asserted) from 2025 published 626,350: chained **641,500**, statutory 640,950, published 640,600
- [ ] `F-CH-single-top12` (context, not asserted) from 2025 published 48,475: chained **49,625**, statutory 50,425, published 50,400
- [ ] `F-CH-single-top22` (context, not asserted) from 2025 published 103,350: chained **105,850**, statutory 105,750, published 105,700
- [ ] `F-CH-single-top24` (context, not asserted) from 2025 published 197,300: chained **202,075**, statutory 201,875, published 201,775
- [ ] `F-CH-single-top32` (context, not asserted) from 2025 published 250,525: chained **256,575**, statutory 256,375, published 256,225
- [ ] `F-CH-single-top35` (context, not asserted) from 2025 published 626,350: chained **641,500**, statutory 640,950, published 640,600
- [ ] `F-CH-mfs-top12` (context, not asserted) from 2025 published 48,475: chained **49,625**, statutory 50,425, published 50,400
- [ ] `F-CH-mfs-top22` (context, not asserted) from 2025 published 103,350: chained **105,850**, statutory 105,750, published 105,700
- [ ] `F-CH-mfs-top24` (context, not asserted) from 2025 published 197,300: chained **202,075**, statutory 201,875, published 201,775
- [ ] `F-CH-mfs-top32` (context, not asserted) from 2025 published 250,525: chained **256,575**, statutory 256,375, published 256,225
- [ ] `F-CH-mfs-top35` (context, not asserted) from 2025 published 375,800: chained **384,900**, statutory 384,550, published 384,350

### 5.4 `fixtures/pending/rounding/table.json` and `fixtures/pending/money/mul_ratio.json` (derived)

**`fixtures/pending/rounding/table.json`** (sha256 `ddeb8c061bcb7649...`) - promotes to `t1/rounding/table`. Each case carries its own `derivation`; re-work it. Amounts are cents.

- [ ] `F-RND-brackets-50-statutory` rule `usc26.1f7A.brackets`, increment 5000, `down`, `IncreaseOverBase` -> expect **21150000**
- [ ] `F-RND-brackets-50-exact-multiple` rule `usc26.1f7A.brackets`, increment 5000, `down`, `IncreaseOverBase` -> expect **21150000**
- [ ] `F-RND-brackets-50-zero-adjustment` rule `usc26.1f7A.brackets`, increment 5000, `down`, `IncreaseOverBase` -> expect **16500000**
- [ ] `F-RND-brackets-50-negative-increase` rule `usc26.1f7A.brackets`, increment 5000, `down`, `IncreaseOverBase` -> expect **16315000**
- [ ] `F-RND-mfs-25-statutory` rule `usc26.1f7B.mfs_and_single`, increment 2500, `down`, `IncreaseOverBase` -> expect **38455000**
- [ ] `F-RND-single-25-vs-50-discriminator` rule `usc26.1f7B.mfs_and_single`, increment 2500, `down`, `IncreaseOverBase` -> expect **5042500**
- [ ] `F-RND-stdded-50-single` rule `usc26.63c7Bii.stdded`, increment 5000, `down`, `IncreaseOverBase` -> expect **1610000**
- [ ] `F-RND-stdded-50-hoh-basis-discriminator` rule `usc26.63c7Bii.stdded`, increment 5000, `down`, `IncreaseOverBase` -> expect **2417500**
- [ ] `F-RND-stdded-50-hoh-floor-the-total` rule `usc26.63c7Bii.stdded`, increment 5000, `down`, `Amount` -> expect **2415000**
- [ ] `F-RND-dc-limit-1000-down` rule `usc26.415c.dc_limit`, increment 100000, `down`, `Amount` -> expect **7100000**
- [ ] `F-RND-dc-limit-1000-exact-multiple` rule `usc26.415c.dc_limit`, increment 100000, `down`, `Amount` -> expect **7000000**
- [ ] `F-RND-dc-limit-1000-negative` rule `usc26.415c.dc_limit`, increment 100000, `down`, `Amount` -> expect **-200000**
- [ ] `F-RND-db-limit-5000-down` rule `usc26.415b.db_limit`, increment 500000, `down`, `Amount` -> expect **28000000**
- [ ] `F-RND-db-limit-5000-exact-multiple` rule `usc26.415b.db_limit`, increment 500000, `down`, `Amount` -> expect **28500000**
- [ ] `F-RND-irmaa-nearest-down` rule `usc42.1395r_i.irmaa`, increment 100000, `nearest`, `Amount` -> expect **10900000**
- [ ] `F-RND-irmaa-nearest-up` rule `usc42.1395r_i.irmaa`, increment 100000, `nearest`, `Amount` -> expect **11000000**
- [ ] `F-RND-irmaa-nearest-exact-multiple` rule `usc42.1395r_i.irmaa`, increment 100000, `nearest`, `Amount` -> expect **10900000**
- [ ] `F-RND-ssa-pia-dime-down` rule `ssa.pia`, increment 10, `down`, `Amount` -> expect **123450**
- [ ] `F-RND-ssa-pia-dime-exact-multiple` rule `ssa.pia`, increment 10, `down`, `Amount` -> expect **123450**
- [ ] `F-RND-ssa-benefit-dollar-down` rule `ssa.payable_benefit`, increment 100, `down`, `Amount` -> expect **123400**
- [ ] `F-RND-ssa-benefit-dollar-exact-multiple` rule `ssa.payable_benefit`, increment 100, `down`, `Amount` -> expect **123400**
- [ ] `F-RND-direction-up-positive` rule `synthetic.direction`, increment 100, `up`, `Amount` -> expect **123500**
- [ ] `F-RND-direction-up-negative` rule `synthetic.direction`, increment 100, `up`, `Amount` -> expect **-123400**
- [ ] `F-RND-direction-down-negative-non-multiple` rule `synthetic.direction`, increment 5000, `down`, `Amount` -> expect **-130000**
- [ ] `F-RND-direction-halfup-tie` rule `synthetic.direction`, increment 100, `halfUp`, `Amount` -> expect **12400**
- [ ] `F-RND-direction-halfup-below` rule `synthetic.direction`, increment 100, `halfUp`, `Amount` -> expect **12300**
- [ ] `F-RND-direction-halfeven-tie-to-even` rule `synthetic.direction`, increment 100, `halfEven`, `Amount` -> expect **12400**
- [ ] `F-RND-direction-halfeven-tie-to-even-down` rule `synthetic.direction`, increment 100, `halfEven`, `Amount` -> expect **12400**
- [ ] `F-RND-basis-pair-synthetic-increase` rule `synthetic.basis`, increment 5000, `down`, `IncreaseOverBase` -> expect **112500**
- [ ] `F-RND-basis-pair-synthetic-total` rule `synthetic.basis`, increment 5000, `down`, `Amount` -> expect **110000**

**`fixtures/pending/money/mul_ratio.json`** (sha256 `39acb098ff49f5b4...`) - promotes to `t1/money/mul_ratio`. Each case carries its own `derivation`; re-work it. Amounts are cents.

- [ ] `F-MUL-i128-exceeds-i64` increment 1, `halfEven`, `Amount` -> expect **333330000000000** (forces the i128 path - hand-work this one)
- [ ] `F-MUL-i128-exact-tie-halfeven` increment 1, `halfEven`, `Amount` -> expect **1000000000000000** (forces the i128 path - hand-work this one)
- [ ] `F-MUL-i128-exact-tie-halfup` increment 1, `halfUp`, `Amount` -> expect **1000000000000001** (forces the i128 path - hand-work this one)
- [ ] `F-MUL-i128-negative` increment 1, `down`, `Amount` -> expect **-333330000000001** (forces the i128 path - hand-work this one)
- [ ] `F-MUL-double-rounding-guard` increment 1, `halfEven`, `Amount` -> expect **33333**
- [ ] `F-MUL-statutory-rate-22pct` increment 1, `halfEven`, `Amount` -> expect **2200000**
- [ ] `F-MUL-statutory-rate-niit-3.8pct` increment 1, `halfEven`, `Amount` -> expect **469136**
- [ ] `F-MUL-statutory-rate-addl-medicare-0.9pct` increment 1, `halfEven`, `Amount` -> expect **889**
- [ ] `F-MUL-composite-ratio-5-over-900` increment 1, `halfEven`, `Amount` -> expect **556**
- [ ] `F-MUL-cent-tie-halfeven-positive` increment 1, `halfEven`, `Amount` -> expect **6172**
- [ ] `F-MUL-cent-tie-halfup-positive` increment 1, `halfUp`, `Amount` -> expect **6173**
- [ ] `F-MUL-cent-tie-halfeven-negative` increment 1, `halfEven`, `Amount` -> expect **-6172**
- [ ] `F-MUL-zero-cents` increment 1, `halfEven`, `Amount` -> expect **0**
- [ ] `F-MUL-ratio-one` increment 1, `halfEven`, `Amount` -> expect **123456789**

Rules whose *existence and increment* are asserted by `rounding/table.json` but
whose primary documents are **not archived** in this vintage (415(c), 415(b), the
IRMAA "nearest 1,000", the two SSA rules): their cases use SYNTHETIC amounts and
test the arithmetic only. They verify nothing about the law; the increments
themselves are `TESTING.md` §5.2 items for M1, M3 and M5.

---

## 6. Promotion: the exact steps

Only after every box in sections 0-5 that applies is ticked.

1. **Record the decisions of section 0** in `DECISIONS.md` (or as design errata)
   and make whatever parameter-shape change they require. A change to a
   parameter file after this point means re-reading the affected lines above.
2. **Flip the parameter files.** In `ordinary_brackets.toml`, `std_deduction.toml`
   and `cpi-chained-suur0000sa0.toml` change
   `verification = "pending-hand-verification"` to
   `verification = "primary-source-confirmed"`, and remove from each
   `[hand_verification] open` list exactly the items section 0 closed. In
   `params/provenance/INDEX.toml` change `verification` likewise. Update the
   "Status" section of `params/vintages/federal-2026/VINTAGE.md`.
3. **Move each promotable fixture**, keeping its directory:

   ```sh
   mkdir -p fixtures/tier1/schedule fixtures/tier1/uprating fixtures/tier1/rounding fixtures/tier1/money
   git mv fixtures/pending/schedule/<status>-<year>-grid.json fixtures/tier1/schedule/
   git mv fixtures/pending/uprating/2026-brackets.json        fixtures/tier1/uprating/
   git mv fixtures/pending/uprating/2026-stdded.json          fixtures/tier1/uprating/
   git mv fixtures/pending/uprating/chained-path-divergence.json fixtures/tier1/uprating/
   git mv fixtures/pending/rounding/table.json                fixtures/tier1/rounding/
   git mv fixtures/pending/money/mul_ratio.json               fixtures/tier1/money/
   ```

   `published/*.json` and `uprating/2025-brackets-control.json` stay where they
   are: they have no tier-1 id. `schedule/mfj-2026-plan-acceptance.json` moves
   like the grids unless decision B10 merged it into `mfj-2026-grid.json`.
4. **Edit each moved file's envelope** (`TESTING.md` §2.2), and nothing inside
   `inputs` or `expect`:
   - `"id"`: the value of `"promotesTo"`; then delete `"promotesTo"` and `"targetTier"`.
   - `"tier"`: `1`.
   - `"verification"`: `"primary-source-confirmed"` where every expected value is
     printed by the cited publication; otherwise `"hand-worked-reviewed"`, adding
     `"derivation"` (the arithmetic in full) and `"reviewedBy"`
     (`reviewer`, `kind`: `second-reviewer` or `cooling-off-re-review`, `date`,
     `sha256` of the derivation text). Every file moved in step 3 computes its
     expected values, so every one of them is `hand-worked-reviewed`. The
     schedule grids, `rounding/table.json` and `money/mul_ratio.json` carry
     `"synthetic": true`; the three `uprating/` files carry `"synthetic": false`,
     correctly, because their inputs are statutory amounts - they cannot take
     the value until decision B16 amends §2.2, and `synthetic` is **not** to be
     flipped to make them pass. No file yet has the top-level `"derivation"`
     (B16).
   - `"verificationNote"`: replace the pending notice with what was done.
   - `"paramVintage"`: the vintage id from step 6.
5. **Point the tests at the new paths.** `crates/pfp-money/tests/fixtures.rs`,
   `crates/pfp-params/tests/fixtures.rs`, `crates/pfp-tax/tests/fixtures.rs` and
   `crates/pfp-tax/tests/properties.rs` `include_str!` the pending paths and
   assert `tier == "pending"`, `verification == "pending-hand-verification"` and
   `paramVintage == null`; change the paths and those three assertions, and
   nothing else. `pfp_params::Vintage::is_verified()` becomes `true` and
   `the_embedded_vintage_is_unlocked_and_unverified` must be rewritten to say so.
6. **Lock the vintage.** Create `params/VINTAGES.lock` with one line per file, in
   the form `cargo xtask data-hygiene` parses - `<sha256>  <repository-relative path>`:

   ```sh
   shasum -a 256 \
     params/vintages/federal-2026/ordinary_brackets.toml \
     params/vintages/federal-2026/std_deduction.toml \
     params/vintages/federal-2026/VINTAGE.md \
     params/index-series/cpi-chained-suur0000sa0.toml \
     > params/VINTAGES.lock
   ```

   The vintage id is `federal-2026@<content hash>` under decision B9:
   `cargo run -q -p xtask -- data-hygiene` prints it as
   `vintage content id federal-2026@...`. Record it where the embedding reads it
   (`crates/pfp-params/src/shipped.rs` then calls `Vintage::locked_as` with it,
   which fails if one byte of a table or series has changed), and write it into
   each promoted fixture's `paramVintage`. From this commit on the locked
   files are immutable: a correction is a new vintage
   (`docs/contributing.md` §1.4).
7. **Run every gate**, and expect `protected-paths` to name the new tier-1 files:
   that gate has no override, and a change it names is approved by the human
   review of the pull request that carries it.

   ```sh
   export PATH="$HOME/.cargo/bin:$PATH"
   cargo fmt --all -- --check
   cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
   cargo test --workspace
   cargo run -q -p xtask -- check-magic
   cargo run -q -p xtask -- lint-dollars
   cargo run -q -p xtask -- data-hygiene
   cargo deny check
   git diff --name-only --diff-filter=ACMR origin/main...HEAD | cargo run -q -p xtask -- protected-paths
   ```
8. **Clear the gate in the design.** Strike the M0 row of `TESTING.md` §5.2 with a
   pointer to the promoting change, and regenerate the validation report.
