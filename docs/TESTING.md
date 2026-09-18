# TESTING

*Validation strategy. Date: 2026-09-17. Companion to `PLAN.md` (what and when), `ARCHITECTURE.md` (how) and `DECISIONS.md` (why). Binding on every milestone's release gate (`PLAN.md` §4.13).*

**Terminology (standing definition).** "The binary" or "the release" always means one thing: a **local web application for macOS**, shipped as a single self-contained universal executable. Launching it starts a web server bound to loopback, serves the embedded web app over TLS and opens the default browser. The UI is a web app in the browser — not a native GUI, not an Electron/Tauri window. End-to-end tests therefore drive a **browser against the real binary**, never a native window harness.

**User-agnostic rule.** Every fixture, persona and example in this document and in `fixtures/` is either a published third-party worked example or clearly-labelled synthetic data. No household's figures ever enter the repository (ADR-023).

---

## 1. Why this document exists, and what it gates

Two capabilities in this product have **no external oracle at all**: the year-by-year household ledger and the next-dollar allocation engine (risk R6). Two more — the federal tax function and the Social Security module — have oracles that are either copyleft, slow, or both. The whole trust story therefore rests on a corpus that is automated, layered, and written from primary sources *before* the code it checks.

Three rules order everything below.

1. **Ground truth precedes implementation.** A fixture is transcribed from the primary source and committed before the function it tests is requested. The AI assistant never authors both a constant and the test that checks it; a lint bans dollar literals in engine crates outside tests; CI path rules block assistant edits to `fixtures/tier1/` and locked `params/` vintages (ADR-022).
2. **Where fixtures cannot reach, invariants must.** The ledger's primary control is not examples but the conservation identity, metamorphic properties and the reconciliation invariant — the last of which doubles as the decision engine's only test oracle (ADR-016).
3. **Every result prints its validation basis.** `tier-1 exact`, `oracle-checked`, `single-oracle`, `property-tested only`. A number whose basis is weak says so on screen, in the export and in the validation report. This is principle 8 of `PLAN.md` made mechanical.

### 1.1 Section-to-milestone map

| Milestone | What lands |
|---|---|
| M0 | §2 corpus structure, fixture schema, tier gates; §3.1 rounding/uprating and money properties; §8 server/browser/supply-chain/release suites **including the container-magic build gate**; §9 contract snapshots; §10 cold start; §11 CI stages and merge gates |
| M1 | §3.2 federal worksheets; §3.5 Tax-Calculator tier 2; §7 taxcalc oracle; §8 vault suite; §10 tax-kernel probe in **both trace modes**; §4 I25 trace-mode agreement; §12 update procedure |
| M2 | §3.4 next-dollar fixtures; §4 reconciliation invariant (I13, three parts), tax-cells-sum-once (I27), year-0 full-tax-year (I28) and patch/schema properties |
| M3 | §3.4 hand-worked ledgers and `t1/ledger/early-deficit-shortfall`; §4 conservation (I1), the net-worth identity (I26) and the metamorphic properties; §3.3 `ss_claim_factor_knife_edge`, because `pfp-ss` is created here with the claim-age factors and the `BenefitAtAge` → PIA back-solve (`PLAN.md` M3, `ARCHITECTURE.md` §3); persona goldens; Keychain test |
| M4 | §4 scenario and migration properties; `t1/ledger/bequest-conservation`; parser fuzzing; §8 `vault compact --purge-before` round-trip and the purge canary; print snapshots |
| M5 | §3.3 and §3.5 Social Security (the rest of `pfp-ss`: AIME/PIA, spousal, survivor, earnings test); the four survivor gates; AnyPIA fixtures |
| M6 | §6 Monte Carlo hygiene; cross-architecture bit-identity; §10 simulation budgets |
| M7 | §6.3 scorecard and spending rules; the probability-contract test |
| M8 | BETR, Form 8962, PV fixtures; the `SeparatedAt55{employer_plan_id}` and `Sepp72t{…}` opt-in rule fixtures; §7 Owl bounds |
| M9 | §4 review-attribution (I23), stale-pin honesty (I24), rebalance and wash-sale properties; lot-selection fixtures |
| M10 | §3.6 state modules against the recorded oracle; 529 fixtures; sandbox-connector test; review findings at zero high |
| 1.1 (by decision, `PLAN.md` M10) | `t1/tax/sec121` and the other housing fixtures; coverage state-machine fixtures; §10 insurance grid and I21 — the life modules' corpus, specified now so the reserved schema shapes are the right ones |

---

## 2. The three-tier corpus

### 2.1 Tiers and their gates

| Tier | Content | Gate | Change policy |
|---|---|---|---|
| **1** | Statute and publisher worked examples with **line-numbered intermediates**; hand-worked synthetic ledgers; the rounding table | Any mismatch fails CI. **No tolerance.** | Path-protected from assistant edits; a change needs a primary-source citation in the PR, or — for a `hand-worked-reviewed` fixture (§2.2) — the amended derivation plus a fresh sign-off under §2.2's review rule |
| **2** | (a) MIT/CC0/Apache suites pinned to a commit SHA and transliterated to data by `xtask` and committed under `fixtures/tier2/`; (b) separately, the AGPL-3.0 PolicyEngine-US YAML, an out-of-process data source consumed through a from-scratch adapter, fetched at a pinned commit into a git-ignored cache and **never committed** | CI; **skips reported with counts** in the validation report | Re-transliteration is a scripted, reviewed diff; a rise in the skip count fails the gate |
| **3** | `insta` golden snapshots (personas, migrations, report view-models, historical backtests) and **recorded** oracle outputs | Reviewed diff | A golden change requires an `engineVersion` bump (ADR-010) |

Tier 1 is where the product's claim to exactness lives, so it is also the smallest and most expensive tier. Tier 2 buys breadth cheaply. Tier 3 catches drift.

### 2.2 Directory layout and file schema

```
fixtures/
  tier1/            (fixture ids abbreviate the directory as `t1/`, e.g. `t1/tax/pub915-ws1-ex1`; `tier2/` is `t2/`)
    rounding/       uprating and RoundingRule cases
    uprating/       statutory base-year uprating reproductions and the chained-path negative control
    schedule/       rate-schedule grids with Line intermediates
    money/          Cents x Ratio rounding cases
    tax/            IRS worksheets, line by line
    ss/             statutory claim factors and PIA arithmetic
    ledger/         hand-worked 2- and 3-year synthetic ledgers
    decide/         next-dollar arithmetic with computed intermediates
    convert/        Roth-conversion closed forms (BETR)
    score/          scorecard contract cases (paired probability, funded-ratio strip)
  tier2/
    oss/            transliterated from open-social-security@<sha>
    ssatools/       transliterated from social-security-tools@<sha>
    taxcalc/        transliterated from Tax-Calculator@<sha>
    cfiresim/       spending-rule assertions from boknows/cFIREsim-open@<sha>
    spend/          spending-rule cases (dynamic spending, Guyton-Klinger, VPW) built on cfiresim/ and the formulas of §5.3
    policyengine/   adapter + recorded expectations only — upstream AGPL YAML is fetched at a pinned commit into a
                    git-ignored cache (.policyengine-cache/), never committed
  tier3/
    oracle/<tool>@<version>/   recorded JSONL request/response pairs
    snapshots/                 insta .snap files
  personas/         seeded synthetic households (generated, not hand-written)
  plans/plan.v{N}.json         frozen schema shapes for the migration chain
  plans/demo.plan.json         the synthetic demo plan, PLAINTEXT JSON, embedded in the binary (§8)
```

Every tier-1 and tier-2 fixture is one JSON document with this shape (money is **integer cents**, never a float, never a formatted string):

```json
{
  "id": "t1/tax/pub915-ws1-ex1",
  "tier": 1,
  "synthetic": false,
  "module": "pfp-tax",
  "milestone": "M1",
  "lawYear": 2025,
  "paramVintage": "federal-2025@<content-hash>",
  "source": {
    "publisher": "IRS",
    "title": "Publication 915, Worksheet 1, Example 1",
    "url": "https://www.irs.gov/publications/p915",
    "locator": "Worksheet 1, filled-in example 1",
    "retrieved": "2026-09-17",
    "archive": "params/provenance/irs/p915-2025.pdf",
    "sha256": "<sha256 of the archived copy>"
  },
  "inputs": { "filingStatus": "Single", "socialSecurityGross": 598000, "pension": 1860000,
              "wages": 940000, "taxableInterest": 99000, "taxExemptInterest": 0 },
  "expect": {
    "lines": [
      { "id": "pub915.ws1.l1", "label": "Net benefits",            "value": 598000 },
      { "id": "pub915.ws1.l2", "label": "One-half of line 1",      "value": 299000 },
      { "id": "pub915.ws1.l3", "label": "Other income",            "value": 2899000 },
      { "id": "pub915.ws1.l7", "label": "Combined income",         "value": 3198000 },
      { "id": "pub915.ws1.l9", "label": "Excess over base",        "value": 698000 },
      { "id": "pub915.ws1.l18","label": "Taxable benefits",        "value": 299000 }
    ]
  },
  "tolerance": { "kind": "exact" },
  "verification": "primary-source-confirmed",   // | hand-worked-reviewed | pending-hand-verification | derived
  "notes": "Single filer; combined income 31,980 is below the 34,000 adjusted base, so the 50% tier binds."
}
```

A `hand-worked-reviewed` fixture carries the same envelope with `"synthetic": true` and two additional required fields; its `source` block cites the inputs the derivation consumes rather than a publication that prints the answer:

```json
  "synthetic": true,
  "verification": "hand-worked-reviewed",
  "derivation": "Deduction 6,000/eligible person shrinks 6c per MAGI dollar above 75,000 (single).
                 EMR = bracket x (1 + 0.06). 22% -> 23.32%. MFJ, two eligible: 12,000 over
                 150,000-250,000 = 12c/dollar, EMR = bracket x 1.12 -> 24.64%.",
  "reviewedBy": { "reviewer": "<a named person; never the AI assistant>",
                  "kind": "second-reviewer",             // | cooling-off-re-review
                  "date": "2026-09-17", "sha256": "<hash of the derivation text signed off>" }
```

**The review rule, stated for a single-maintainer project** (`PLAN.md` R26). `kind: second-reviewer` is a named collaborator who is not the author, when one exists. When none exists, `kind: cooling-off-re-review` is admissible: the author re-derives the expected values **blind**, from the cited inputs, no sooner than seven days after the original derivation, and the sign-off records that second date and the hash of the re-derivation, which must equal the hash of the original; a mismatch reopens the fixture. Either way the AI assistant is never the reviewing party, the validation report prints the `kind` counts beside the `hand-worked-reviewed` counts (§13), and the fixture never claims a second human it did not have.

Rules the loader enforces (a violation is a CI failure, not a warning):

- `tolerance.kind` for tier 1 must be `exact`, or the **one** admitted alternative `{"kind":"publishedPrecision","decimals":N}` — exact after rounding to the precision the publisher actually prints, used only where the source publishes a derived rate rather than a statutory line (the EMR shapes and the BETR figures). It is never a band. Tiers 2 and 3 may additionally use `{"kind":"absoluteCents","value":500}`, `{"kind":"relative","ppm":N}` or `{"kind":"bounds","lo":…,"hi":…}`.
- `verification` is one of `primary-source-confirmed | hand-worked-reviewed | pending-hand-verification | derived`. A **tier-1** fixture must be one of the first two, and nothing else; `pending-hand-verification` and `derived` are rejected at tier 1 by the loader, which makes §5.2's gate table mechanical rather than remembered.
- The two admissible tier-1 values are not interchangeable, because the corpus has two genuinely different kinds of ground truth (§1, rule 1) and the validation report must show which one a number rests on:
  - **`primary-source-confirmed`** — the expected values are *transcribed from a publication*. It requires a complete `source` block, a **publisher-controlled** `source.url` (see §5.4) and a `source.sha256` matching the archived copy under `params/provenance/`. The publisher-host rule is enforced only on fixtures carrying this value, because it is the only value that claims a publication printed the number.
  - **`hand-worked-reviewed`** — the expected values are *computed by this project* from statutory inputs or from the engine's own stated conventions, where no publication prints the result. Admissible at tier 1 **only** with `synthetic: true`, a `derivation` string stating the arithmetic in full, a `source` block citing the statutory inputs the derivation consumes, and a recorded `reviewedBy` sign-off under the review rule above — a second reviewer who is not the author, or the cooling-off re-review — and never the AI assistant, per rule 1. The ledger, next-dollar and computed-EMR corpus lives here — the two capabilities §1 opens by saying have **no external oracle** could not otherwise reach tier 1 at all.
- The validation report's `unverified` block prints tier-1 `hand-worked-reviewed` counts beside the `primary-source-confirmed` counts, so the distinction is visible on the About page rather than buried in fixture files (§13).
- `synthetic: true` or a complete `source` block is mandatory — this is the data-hygiene linter's rule, shared with `params/`.
- `expect.lines[].id` must exist in the engine's `LineId` registry, so a renamed worksheet line breaks the build rather than silently skipping an assertion.
- Every fixture declares `milestone`; the validation report groups counts by tier **and** by milestone.

### 2.3 What each tier may borrow (the licence boundary, under test)

Port with attribution (MIT/Apache-2.0: Open Social Security, ssa.tools, cFIREsim-open rule tests) — transliterate assertions by `xtask`, one `NOTICE` entry each. Vendor as data (CC0/public domain: Tax-Calculator parameter values, IRS/SSA/CMS documents) — copy into `params/` with an archived, checksummed primary text. GPL/AGPL projects are out-of-process oracles only (§7), and TPAW is read-only reference: re-derive equations, copy nothing. **Not vendored:** the finiki/Bogleheads VPW table (CC BY-SA) and any historical return series whose redistribution terms are unconfirmed — generate from the formula, or ship a loader plus a documented CSV format with published checksums.

Two restrictions need a **non-mechanical** control because the scanners pass them silently: **TPAW is PolyForm Noncommercial but GitHub reported `spdx_id: NOASSERTION` as of 2026-09-17** ([tpaw](https://github.com/bengmathew/tpaw)), and **AnyPIA-js has no LICENSE file and its last commit is dated 2021-03-11 (checked 2026-09-17)** ([anypia-js](https://github.com/codeforboston/anypia-js)). Both are rows in `docs/licence-watchlist.md`, each with the date it was last checked, which the release checklist requires a human to re-confirm (`PLAN.md` §4.13 item 11). Beancount (GPL-2.0) and hledger (GPL-3.0) are read-only references for booking semantics, never transliterated (§4, §14).

---

## 3. Fixture matrix

Money in the `Expected` column is written the way the publisher writes it; the fixture file stores integer cents. "Milestone" is where the fixture must be green.

### 3.1 Tier 1 — rounding, uprating and money (M0)

| Fixture | Source | Module | Inputs | Expected | Tol. | M |
|---|---|---|---|---|---|---|
| `t1/rounding/table` | 26 USC 1(f)(7); [Pub 590-B](https://www.irs.gov/pub/irs-pdf/p590b.pdf); [42 USC 1395r(i)](https://www.law.cornell.edu/uscode/text/42/1395r); [SSA FR 2025-19763](https://www.govinfo.gov/content/pkg/FR-2025-11-03/pdf/2025-19763.pdf) | `pfp-money` | The eight rules below | Each rule's `{increment, direction, basis}` round-trips a table of boundary amounts | exact | M0 |
| `t1/uprating/2026-brackets` | [Rev. Proc. 2025-32](https://www.irs.gov/pub/irs-drop/rp-25-32.pdf) Tables 1 & 3 | `pfp-params` | 26 USC 1(f)(7) **statutory base-year amounts per filing status** + the archived `cpi.chained` series, ratio taken base-year → 2026 | Every 2026 bracket top reproduced: MFJ 24,800 / 100,800 / 211,400 / 403,550 / 512,450 / 768,700; single 12,400 / 50,400 / 105,700 / 201,775 / 256,225 / 640,600 | exact | M0 |
| `t1/uprating/2026-stdded` | Rev. Proc. 2025-32 §.14 | `pfp-params` | **Post-OBBBA 2024 base-year amounts** + the archived `cpi.chained` series, `basis: IncreaseOverBase` | 16,100 single/MFS · 32,200 MFJ · 24,150 HOH; aged/blind add-on **1,650** married, **2,050** unmarried; dependent floor greater of **1,350** or earned + 450 | exact | M0 |
| `t1/uprating/chained-path-divergence` | Same base-year amounts and series | `pfp-params` | 2026 computed the statutory way vs computed by chaining 2025 → 2026 off an already-rounded 2025 value | The two **disagree** on at least one 2026 threshold; the statutory value is the published one. `hand-worked-reviewed`: a negative control for I7 | exact | M0 |
| `t1/schedule/mfj-2026-grid` | Derived from Rev. Proc. 2025-32 brackets (`ENGINE-SPEC.md` §3.2) | `pfp-tax` | MFJ taxable 100,000 / 150,000 / 250,000 | **11,504** · **22,424** · **45,196**, with `Line` intermediates per bracket slice | exact | M0 |
| `t1/money/mul_ratio` | ADR-007 | `pfp-money` | `Cents × Ratio` at every rounding direction and increment | One rounding step, `i128` intermediate, no double rounding | exact | M0 |

Rounding table as data (`{rule_id, increment, direction, basis}`):

| Rule | Increment | Direction | Basis |
|---|---|---|---|
| 26 USC 1(f)(7)(A) brackets | $50 | down | **increase over the statutory base year** |
| 1(f)(7)(B) MFS table | $25 | down | increase over base |
| 63(c)(4) standard deduction | $50 | down | increase over base (2024 base post-OBBBA) |
| 415(c) defined-contribution limit | $1,000 | down | amount |
| 415(b) defined-benefit limit | $5,000 | down | amount |
| 42 USC 1395r(i) IRMAA thresholds | $1,000 | **nearest** | amount |
| SSA PIA | $0.10 | down (truncate) | amount |
| SSA payable monthly benefit | $1 | down (truncate) | amount |

The `IncreaseOverBase` basis is load-bearing and is its own assertion: applying $50 rounding to the *final* figure reproduces most but not all published thresholds (the project's internal research review (unpublished)). A fixture that passes under both bases is not a test of this rule; the corpus keeps at least one threshold per filing status where the two bases disagree.

**The inputs to these three fixtures are base-year amounts, not last year's published table, and that has a consequence for seam S1.** `IncreaseOverBase` rounds the *increase over the statutory base year*, once. The only way to compute it is `base_value × (index_year / index_base_year)`, with the $50 (or $25) reduction applied a single time to the increase. Chaining — taking the published 2025 figure, applying one year of index, rounding again — rounds twice and drifts; that is what `t1/uprating/chained-path-divergence` pins. The parameter-table shape frozen at M0 (`PLAN.md` §3, seam S1) must therefore carry the inputs the correct pipeline needs, not only the projection rule: `projection{rule, index, index_series, base_year, base_values, rounding}`, with `base_values` broken down per filing status exactly as `values` is. A table that declares `basis: IncreaseOverBase` without `base_year` and `base_values` is a CI error, alongside the existing no-projection-rule error (§11.2 gate 9). M0 scope therefore also archives the `cpi.chained` series itself and the base-year amounts under `params/provenance/`, checksummed like any other primary text — an index series that is fetched rather than archived would make the M0 gate unreproducible. The §1(f) base year and its per-status base amounts are on the §5.2 hand-verification list until a human has read them in statute.

### 3.2 Tier 1 — federal tax worksheets (M1 unless noted)

| Fixture | Source | Module | Inputs | Expected (line-level) | Tol. | M |
|---|---|---|---|---|---|---|
| `t1/tax/pub915-ws1-ex1` | [Pub 915](https://www.irs.gov/publications/p915) | `pfp-tax` | Single; SS 5,980; pension 18,600; wages 9,400; interest 990 | half 2,990 · other 28,990 · combined 31,980 · over-base 6,980 · **taxable 2,990** | exact | M1 |
| `t1/tax/pub915-ws1-ex3` | Pub 915 | `pfp-tax` | MFJ; SSEB 10,000; pension 38,000; interest 2,300; tax-exempt 200 | combined 45,500 · over-adjusted 1,500 · tier-1 adder 5,000 · **taxable 6,275** | exact | M1 |
| `t1/tax/rmd-590b-75` | [Pub 590-B](https://www.irs.gov/pub/irs-pdf/p590b.pdf) | `pfp-tax` | Age 75, prior 12/31 balance 100,000, spouse 6 years younger | Uniform Lifetime divisor 24.6 → **4,065** | exact | M1 |
| `t1/tax/rmd-590b-joint` | Pub 590-B Table II | `pfp-tax` | Same balance, sole-beneficiary spouse 11 years younger | divisor 25.3 → **3,953** | exact | M1 |
| `t1/tax/rmd-age74-regression` | Pub 590-B Appendix B | `pfp-tax` | Age 74 | divisor **25.5**. Guards against the publication's own defective 2026 half (see §5.2) | exact | M1 |
| `t1/tax/ult-table` | Pub 590-B Appendix B | `pfp-params` | Ages 72–100 | 27.4, 26.5, 25.5, 24.6, 23.7, 22.9, 22.0, 21.1, 20.2, 19.4, 18.5, 17.7, 16.8, 16.0, 15.2, 14.4, 13.7, 12.9, 12.2, 11.5, 10.8, 10.1, 9.5, 8.9, 8.4, 7.8, 7.3, 6.8, 6.4 | exact | M1 |
| `t1/tax/slt-spots` | Pub 590-B Appendix B | `pfp-params` | Single Life Table spot checks | 60 → 27.1 · 65 → 22.9 · 70 → 18.8 · 75 → 14.8 · 80 → 11.2 | exact | M1 |
| `t1/tax/niit-8960` | IRS Form 8960 and its instructions (publisher-hosted; archive path recorded at authoring) | `pfp-tax` | The instructions' worked examples | Named NIIT lines; 3.8% on the lesser of NII or MAGI over 200k/250k/125k | exact | M1 |
| `t1/tax/ltcg-stacking` | Rev. Proc. 2025-32 breakpoints | `pfp-tax` | Ordinary + QDI/LTCG spanning the 0/15/20 boundaries | 0% room to 49,450 single / 98,900 MFJ; 15% to 545,500 / 613,700, stacked above ordinary | exact | M1 |
| `t1/tax/8606-prorata` | [Form 8606](https://www.irs.gov/pub/irs-pdf/f8606.pdf) line 10 | `pfp-tax` | basis 10,000; year-end value 90,000; conversion 20,000 | ratio **0.091** (≥3 dp, capped at 1.000) → nontaxable **1,820**; unrounded variant 1,818.18 agrees within $1 × conversions/1,000 | exact | M2 |
| `t1/tax/senior-deduction` | [26 USC 151(d)(5)](https://www.law.cornell.edu/uscode/text/26/151) | `pfp-tax` | 65+ filer, MAGI sweep | 6,000 per person; 6% phase-out above 75k single / 150k MFJ; zero at 175k / 250k; applied **after** taxable SS | exact | M1 |
| `t1/tax/payroll-2026` | [Topic 554/751](https://www.irs.gov/taxtopics/tc554); [Notice 2025-67](https://www.irs.gov/pub/irs-drop/n-25-67.pdf) | `pfp-tax` | Wages and SE profit across the wage base | `payroll()` books **OASDI to base 184,500 and regular Medicare 1.45% only**, plus SE on 92.35%; it never books Additional Medicare | exact | M1 |
| `t1/tax/addl-medicare-8959` | [Topic 554/751](https://www.irs.gov/taxtopics/tc554); [Form 8959](https://www.irs.gov/forms-pubs/about-form-8959) | `pfp-tax` | Two-earner MFJ household whose combined earnings cross 250k while neither person crosses it alone; single at 200k; MFS at 125k | Additional Medicare 0.9% is the `f8959.*` line **inside `federal()`** on the household combined basis (`ENGINE-SPEC` §3.2 line 6); employer withholding of the surcharge is a credit against that line, not a second charge; the MFJ case is the discriminator a per-person `payroll()` cannot pass | exact | M1 |
| `t1/tax/emr-shapes` | [Reichenstein & Meyer](https://www.financialplanningassociation.org/article/journal/JUL18-understanding-tax-torpedo-and-its-implications-various-retirees); [Kitces bump zone](https://www.kitces.com/blog/long-term-capital-gains-bump-zone-higher-marginal-tax-rate-phase-in-0-rate/) | `pfp-tax` | Income sweeps by character | The **published** shapes: SS torpedo 22% → **40.7%** (1.85×); LTCG bump zone **27%**; 24% + 3.8% = **27.8%** | `publishedPrecision`, 1 dp | M1 |
| `t1/tax/emr-senior-phaseout` | [26 USC 151(d)(5)](https://www.law.cornell.edu/uscode/text/26/151), via the derivation below | `pfp-tax` | MAGI sweep across the senior-deduction phase-out, one 65+ single filer and an MFJ couple both 65+ | Single, MAGI 75k–175k: 22% → **23.32%**, 12% → **12.72%**. MFJ two eligible, 150k–250k: 22% → **24.64%**, 12% → **13.44%**. Inside the torpedo, single: 22% × 1.85 × 1.06 | exact | M1 |
| `t1/tax/irmaa-step` | [SSA POMS HI 01101.020](https://secure.ssa.gov/poms.nsf/lnx/0601101020); [CMS 2026 fact sheet](https://www.cms.gov/newsroom/fact-sheets/2026-medicare-parts-b-premiums-deductibles) | `pfp-tax` | MFJ crossing the first tier, two covered people | 2 × (284.10 − 202.90 + 14.50) × 12 = **$2,297/yr** step in the EMR curve | exact | M3 |
| `t1/tax/ptc-8962` | IRS Form 8962 examples; [Rev. Proc. 2025-25](https://www.irs.gov/pub/irs-drop/rp-25-25.pdf); [ASPE guidelines](https://aspe.hhs.gov/poverty-guidelines) | `pfp-tax` | Household at 350% and 405% FPL | Applicable percentage band 9.96%; **zero credit above 400% FPL** (cliff live since 2026-01-01) | exact | M8 |
| `t1/tax/sec121` | IRS Pub 523 worked examples | `pfp-ledger` | Ownership/use tests, partial exclusion, **and gain above the limit with nonqualified use** | Named gain-exclusion lines and depreciation recapture; the dollar limit applies to the eligible gain after the nonqualified-use fraction, never the other way round (`DECISIONS.md` C4) | exact | **1.1** (with `Property`) |

**Why the senior phase-out is its own fixture, and why it is not "+6 points."** The statute states the rule as a **6% phase-out** — the deduction shrinks by 6 cents per dollar of MAGI above the threshold — and the attested endpoints are: $6,000 per eligible person, gone at $175k single and $250k MFJ ([26 USC 151(d)(5)](https://www.law.cornell.edu/uscode/text/26/151)). A phase-*out rate* is not a rate *increase*. Losing 6 cents of deduction per dollar adds 6 cents of taxable income per dollar, so the effective marginal rate rises by `phase_out_rate × bracket`, not by six points:

- **Single, one eligible person.** $6,000 over the attested $75k → $175k range is 6¢/dollar. EMR = bracket × 1.06 → 22% becomes **23.32%**, 12% becomes **12.72%**.
- **MFJ, two eligible people.** $12,000 over the attested $150k → $250k range is 12¢/dollar. EMR = bracket × 1.12 → 22% becomes **24.64%**, 12% becomes **13.44%**. The per-eligible-person reading is *inferred from* the attested endpoints rather than read off statute, so it carries a §5.2 hand-verification gate; a per-return reading would put the MFJ zero point elsewhere and the fixture would have to move with it.
- **Inside the torpedo.** The senior deduction applies **after** taxable Social Security and does not move the §86 thresholds, so a dollar of ordinary income inside the phase-in drags benefits into MAGI *and then* erodes the deduction. The two effects compound: 22% × 1.85 × 1.06 for a single filer in both zones at once.

The fixture is `verification: hand-worked-reviewed` with that derivation recorded verbatim, and tolerance **exact** — these are computed `Ratio` products, not figures a publication prints to one decimal, so they must not inherit the `publishedPrecision` tolerance that the transcribed shapes correctly use. A correct engine cannot pass a "+6 points" assertion, and an engine bent to pass one would be wrong at every senior MAGI in the range.

### 3.3 Tier 1 — Social Security (M5, except the claim-factor gate, which is M3)

| Fixture | Source | Module | Inputs | Expected | Tol. | M |
|---|---|---|---|---|---|---|
| `t1/ss/claim-factor-knife-edge` | Statutory factor per [POMS RS 00615](https://secure.ssa.gov/poms.nsf/lnx/0300615000) | `pfp-ss` | PIA 1,000, 60 months early | **700.00** exactly. Factor reduced to one rational at point of use: `1 − min(m,36)·5/900 − max(m−36,0)·5/1200`. **Named mandatory CI gate `ss_claim_factor_knife_edge`**; green from **M3**, where the claim-age factors and `ss.fra.retirement` land with the `BenefitAtAge` → PIA back-solve (`PLAN.md` M3, `ARCHITECTURE.md` §3), and a gate on every milestone after | exact | M3 |
| `t1/ss/pia-formula` | [SSA FR 2025-19763](https://www.govinfo.gov/content/pkg/FR-2025-11-03/pdf/2025-19763.pdf); [SSA familymax derivation](https://www.ssa.gov/oact/cola/familymax.html) | `pfp-ss` | AIME sweeps at 2026 bend points 1,286 / 7,749 | `0.90·min(AIME,b1) + 0.32·clamp(…) + 0.15·max(…)`, **dime-truncated**, then dime-truncated again after each COLA | exact | M5 |
| `t1/ss/aime-floor` | SSA indexing rules | `pfp-ss` | Synthetic 35-year earnings records, each year capped at its taxable maximum **before** indexing | `AIME = floor(Σ top-35 indexed / 420)`; zero years count | exact | M5 |
| `t1/ss/survivor-riblim-early` (gate `ss_survivor_riblim_early`) | [POMS RS 00615.320](https://secure.ssa.gov/poms.nsf/lnx/0300615320) | `pfp-ss` | Branch (a): decedent claimed at 62 (700 on PIA 1,000); survivor claims at 60 | **715** — `min(PIA × age_factor, max(deceased_reduced, 0.825 × PIA))`; the 82.5% figure is a **limit, not a floor** | exact | M5 |
| `t1/ss/survivor-delayed-decedent` (gate `ss_survivor_delayed_decedent`) | POMS RS 00615.320; Open Social Security `benefit.service.ts` | `pfp-ss` | Branch (b): decedent claimed at 70 (1,240); survivor at survivor FRA | **1,240** — the base is the actual benefit **including delayed credits**; the single RIB-LIM formula gives 1,000 here and is wrong by 19% for life (`SIMULATION-SPEC` §14.3) | exact | M5 |
| `t1/ss/survivor-unfiled-decedent` (gate `ss_survivor_unfiled_decedent`) | Open Social Security port; **(unverified)** until confirmed against the OSS and ssa.tools oracles at M5 | `pfp-ss` | Branch (c): decedent FRA 67, died at 68 unfiled; survivor at survivor FRA | **1,080** — the PIA with credits to the month of death | exact | M5 |
| `t1/ss/survivor-fra-table` (gate `ss_survivor_fra_table`) | [POMS RS 00615.301](https://secure.ssa.gov/poms.nsf/lnx/0300615301); the 74-month anchor for a 1957 birth year | `pfp-params` | One birth year in 1955-1961 | `ss.fra.survivor` **differs** from `ss.fra.retirement` for that year (its own schedule: retirement FRA of birth year Y − 2); implementing survivor FRA from the retirement table fails this fixture | exact | M5 |
| `t1/ss/cola-chain` | [SSA FR 2025-19763](https://www.govinfo.gov/content/pkg/FR-2025-11-03/pdf/2025-19763.pdf) | `pfp-ss` | 2026 COLA **2.8%** applied from the age-62 year regardless of claim date | Dime truncation at each step; delaying never forfeits a COLA | exact | M5 |
| `t1/ss/cola-year-convention` | ssa.tools `constants.ts` keying hazard | `pfp-ss` | Same COLA labelled by December-effective year vs paid-in year | A test **asserts the chosen convention** and converts on import. 2.8% effective Dec 2025 = paid in 2026 | exact | M5 |

### 3.4 Tier 1 — next-dollar arithmetic (M2) and hand-worked ledgers (M3)

| Fixture | Source | Module | Inputs | Expected | Tol. | M |
|---|---|---|---|---|---|---|
| `t1/decide/hsa-payroll-saving` | Derived from the 2026 wage base ([Notice 2025-67](https://www.irs.gov/pub/irs-drop/n-25-67.pdf), [Topic 751](https://www.irs.gov/taxtopics/tc751)) | `pfp-decide` | Synthetic wages below / above **184,500**, and above the 250k MFJ Additional-Medicare threshold | `payroll_saving` **7.65%** below the base, **1.45%** above it, and **still 1.45%** above the Additional-Medicare threshold — **computed by running `payroll()` twice**, never a literal. The 0.9% surcharge is an `f8959` line inside `federal()`, so above that threshold it appears in `t_m` via `marginal(.., HsaPayroll)` and the fixture asserts it **there**, once; a 2.35% payroll figure would mean `payroll()` had booked Additional Medicare and the HSA `r_u` counted it twice (`ENGINE-SPEC` §5.2, I27) | exact | M2 |
| `t1/decide/deductible-fraction` | Rev. Proc. 2025-32 standard deduction; SALT schedule | `pfp-decide` | Synthetic household taking the standard deduction vs one itemizing through the SALT cap | `deductible_fraction` **0** vs **1**, computed by running `federal()` twice | exact | M2 |
| `t1/decide/mega-backdoor-room` | 415(c) **72,000** and deferral **24,500** ([Notice 2025-67](https://www.irs.gov/pub/irs-drop/n-25-67.pdf)) | `pfp-decide` | Synthetic plan with employer contribution E and deferrals D | room = `72,000 − D − E`, clamped at 0, gated on the plan's after-tax and in-plan-conversion flags | exact | M2 |
| `t1/decide/debt-placement` | [Bogle Center ten-tier waterfall](https://boglecenter.net/bogleheads-chapter-series-prioritizing-investments/) | `pfp-decide` | A fixed 6% non-deductible loan under each named preset (Treasury 4.25%, no bonds, `E[r_equity]` 6.7%, ERP preset 5%) | Tier placement per preset (`ENGINE-SPEC` §5.4 case (i)); the medium/low boundary default is **10-year Treasury + 3 pp**, an editable assumption, *not* the stock-return assumption; every `r_u` row asserts the **`(1 − t_m)` denominator** explicitly, at the `t_m` the card prints | exact | M2 |
| `t1/decide/safe-yield-floor` | Rule 1 of the debt comparison (`ENGINE-SPEC.md` §5.3, `DECISIONS.md` C5) | `pfp-decide` | A 3% fixed mortgage, no bonds, Treasury 4.25%, same vintage (`ENGINE-SPEC` §5.4 case (ii)) | `r_debt_at` is below `y_safe_at`, so `TaxableSafe` outranks prepayment and the debt stays at minimums; the pre-floor `r_rm` definition inverted this. Paired with the property that no debt whose after-tax rate is below the after-tax safe yield outranks every taxable option (I4b) | exact | M2 |
| `t1/ledger/hand-2yr`, `t1/ledger/hand-3yr` | Hand-worked from the engine's own stated conventions, reviewed as tier 1 | `pfp-ledger` | Synthetic household; **zero return, zero inflation** | Every ledger cell, reproduced to the cent; conservation residual exactly 0 | exact | M3 |
| `t1/ledger/amortization` | Closed-form PMT/IPMT/PPMT | `pfp-ledger` | Synthetic loans, monthly sub-engine | Matches closed form to the cent over the full term | exact | M3 |
| `t1/ledger/death-year-status-guard` | First-death convention (ADR-008) | `pfp-ledger` | Synthetic couple with an assumed first-death year *t* | MFJ **through** year *t*, status flips in *t+1*. Written as an **expected-failure** case: a variant ledger that flips status *in* year *t* must fail the assertion, so the test proves the guard is live rather than merely present | exact | M3 |
| `t1/ledger/bequest-conservation` | `ENGINE-SPEC` §11.1 step 5 and §2.5; `DOMAIN-MODEL` §6 | `pfp-ledger` | **Generated** case: couple, `beneficiary.spouse_fraction < 1` on a traditional and a Roth account, `remainder: Charity`, first death in year *t* | The untransferred share leaves the plan as a bequest valued at `1 − heirs_rate`; I1's **bequests-out** term is non-zero and the residual is still exactly 0. Generated rather than a seventh persona, because all six personas set `spouseFraction` to 1 | exact | M4 |
| `t1/ledger/early-deficit-shortfall` | `ENGINE-SPEC` §2.4, which owns the rule | `pfp-ledger` | Synthetic household with a deficit before 59.5 and no penalty-free source left; `WithdrawalPolicy.early_access` empty | The year is marked **SHORTFALL** by default and the engine books no penalized withdrawal; the **computed penalized alternative** sits beside it with the early-distribution additional tax on its own `f5329` line, so the ledger shows what the user is declining. A variant with an `EarlyAccessRule::Penalized{account_ids}` entry books that same alternative and no SHORTFALL (M3); variants with `SeparatedAt55{employer_plan_id}` and `Sepp72t{…}` each resolve the deficit without the `f5329` line, each with its own fixture (M8, once the statutory conditions clear §5.2) | exact | M3 (SHORTFALL, `Penalized`), M8 (statutory exceptions) |

**Verification values across §3.1–§3.4.** These tier-1 fixtures are `hand-worked-reviewed` (§2.2) rather than `primary-source-confirmed`, because their expected values are computed by this project from statutory or conventional inputs that no publication combines: `t1/money/mul_ratio`, `t1/schedule/mfj-2026-grid`, `t1/uprating/chained-path-divergence`, `t1/tax/emr-senior-phaseout`, `t1/tax/addl-medicare-8959`, `t1/ss/survivor-unfiled-decedent`, all five `t1/decide/*` rows and every `t1/ledger/*` row. Each ships `synthetic: true`, its derivation in full, a `source` block citing the inputs the derivation consumes, and a sign-off under §2.2's review rule. Two of them would otherwise be unplaceable: `t1/decide/mega-backdoor-room` **must** ship the formula rather than the blog's number (§5.2), and `t1/decide/debt-placement` combines the Bogle Center tier list with a Treasury-anchored boundary, a construct of this project (`ENGINE-SPEC` §5.4). Everything else in §3.1–§3.3 is transcribed from a publication and stays `primary-source-confirmed` under the publisher-host rule. Without this second value the loader would reject the ledger and next-dollar corpus outright — the two capabilities §1 opens by saying have no external oracle — or §2.2's gate would have to be unenforced, and its claim to be mechanical would be false.

### 3.5 Tier 2 — pinned MIT/CC0/Apache suites

Transliterated by `cargo xtask fixtures transliterate <suite>` from a pinned commit SHA; the SHA and the tool version appear in the validation report.

| Suite (pinned) | Module | Representative assertions | Tol. | M |
|---|---|---|---|---|
| [Open Social Security](https://github.com/MikePiper/open-social-security) `benefit.service.spec.ts` (MIT; **11** spec files, not 17) | `pfp-ss` | PIA 1,000, FRA Aug 2030: 60 mo early **700**; 24 mo early **866.67**; +12 mo **1,080**; +48 mo **1,320**; suspension **885.33** / **1,186.67** | exact | M5 |
| Open Social Security survivor cases | `pfp-ss` | At 60: **715**. 24 mo early: **908** = 1000 − 1000 × (24/74) × 0.285 (spec line 269) — **see the rounding note below** | exact | M5 |
| [`earningstest.service.spec.ts`](https://github.com/MikePiper/open-social-security/blob/master/src/app/earningstest.service.spec.ts) | `pfp-ss` | 2018 exempt 17,040 / 45,360: (70,000 − 17,040)/2 = **26,480**; FRA year (70,000 − 45,360)/3 = **8,213.33**; 9 months → **14,880** | exact | M5 |
| `calculate-PV.service.spec.ts` | `pfp-ss` | Single male, SSA table, 1% real: PV **142,644**; retroactive at FRA **180,441**; file-and-suspend to 70 **119,370**. The mortality table is **checked into fixtures** | ±$1 | M8 |
| [ssa.tools `pia.test.ts`](https://github.com/Gregable/social-security-tools/blob/main/src/test/pia.test.ts) (MIT) | `pfp-ss` | Bend points 926 / 5,583; AIME 3,000 → **833.4** and **663.68**; COLA 1.7%: 1,515.9 → **1,541.6** | exact | M5 |
| ssa.tools `constants.ts` | `pfp-params` | **Drift detector only, never a source of truth.** Fails on divergence from the locked vintage; a failure opens a primary-source re-read, not a parameter edit | exact | M5 |
| [Tax-Calculator](https://github.com/PSLmodels/Tax-Calculator/tree/master/taxcalc/tests) `test_calcfunctions.py` (CC0) | `pfp-tax` | Function-level regressions, e.g. `StdDed` expected `[12000, 15800, 13800, 14400, 6000, 6000, 0, 1000, 1350]` (line 208); `DependentCare` **25196** (line 181) | exact | M1 |
| [boknows/cFIREsim-open](https://github.com/boknows/cFIREsim-open) `test/spendingModule/`, pinned as `boknows/cFIREsim-open@<sha>` at transliteration (Apache-2.0 per the repository's `LICENSE` file; **last push 2022-04-08, checked 2026-09-17**; the `boknows` repository is the one whose `test/spendingModule/` assertions are transliterated, and its owner is the copyright holder named in `NOTICE`, ADR-004) | `pfp-sim` | Guyton-Klinger rule assertions, percent-of-portfolio, variable spending, VPW. Rule **logic** only — the bundled Shiller series is stale and the 5.2–5.6% headline is CMA-dependent, not a data fixture | exact on rule outputs | M7 |
| [PolicyEngine-US](https://github.com/PolicyEngine/policyengine-us) YAML (AGPL-3.0; consumed as data through a from-scratch adapter — the upstream YAML is fetched at a pinned commit into a git-ignored cache, never committed; `fixtures/tier2/policyengine/` holds the adapter and recorded expectations only) | `pfp-tax` | **212** IRS baseline files, **3,327** state files, **43** with `period: 2026`. Tests setting parameter overrides are **skipped**, and unmapped variables are **skipped, not failed** | oracle tolerance | M10 |

**Two traps recorded in the fixture notes.** (a) PolicyEngine's frequently-cited `taxable_social_security.yaml` (`tax_unit_taxable_social_security: 17_000`) is *not* usable: it is titled "Test flat 85% SS taxation with minimal parametric reform", sets three parameter overrides and is `period: 2024`. Replace it with a baseline-law example and report how many IRS YAMLs survive the no-override filter. (b) The 43 `period: 2026` files are mostly OBBBA-era items (QBID, SALT, tip/overtime, auto-loan interest) — **they do not cover the core SS or bracket paths**.

**Survivor rounding note.** The OSS assertion **908** implies nearest-dollar rounding, while SSA's payable-benefit rule is dollar **truncation** (1000 − 92.43 = 907.57 → 907). The fixture records the source's convention; a companion tier-1 case pins the engine's convention against POMS at M5 (§5.2).

### 3.6 Tier 3 — goldens and recorded oracles

| Golden / oracle | What it covers | Basis label | Tol. | M |
|---|---|---|---|---|
| `tier3/oracle/taxcalc@<ver>` | Synthetic single/MFJ/HOH grid, **≥500 cases**, ages 35–85, wages/SS/IRA/LTCG/QDI, itemizing on and off | `oracle-checked` | **within $5** | M1 |
| `tier3/oracle/policyengine@<ver>` | State modules and ACA PTC; the **only 2026-capable** state and ACA oracle | `single-oracle` (printed on every state result) | declared per module | M10 |
| `tier3/oracle/owl@<ver>` | Conversion-planner example cases (`Case_*.toml`, 17 of them) | `oracle-checked`, **as bounds** | bounds, never equality | M8 |
| AnyPIA-generated PIA fixtures | Synthetic earnings records through SSA's own engine ([ssa.gov/OACT/ANYPIA](https://www.ssa.gov/OACT/ANYPIA/)) | `tier-1 exact` once transcribed | **to the dime** | M5 |
| `tier3/snapshots/personas/*` | Six seeded personas: ledger rows, KPIs, recommendations, explanations | `property-tested only` unless a tier-1 case covers the cell | exact snapshot | M3+ |
| `tier3/snapshots/reports/*` | One-Page Plan, Full, Review print view-models | — | exact snapshot | M4 |
| `tier3/snapshots/migrations/*` | `plan.v{N}.json` migrated forward, including scenario patch-path rewrites | — | exact snapshot | M2+ |
| `tier3/snapshots/openapi.json`, `plan.schema.json` | API and plan-schema contracts | — | exact snapshot | M0/M2 |

**An LP optimum and a heuristic will differ.** Owl's `Case_john+sally.toml` frames bracket-surfing against an LP optimum; its recorded outputs **bound** the conversion planner (conversions-by-year and terminal after-tax estate inside a declared band), and a fixture note says so, so that a future contributor does not "fix" the planner to match an optimizer the product deliberately does not have.

### 3.7 Personas (seeded, synthetic, shared by goldens and e2e)

Generated by `cargo xtask personas --seed <n>`, never hand-written, never derived from anyone's data. All six are used by the tier-3 goldens, the reconciliation-invariant property test and the Playwright journeys.

| Id | Shape | What it exercises |
|---|---|---|
| `P1` | Early-career single filer, student debt, employer match, HDHP | Match, high-interest debt hurdle, HSA, IRA phase-out edges, student-loan interest phase-out |
| `P2` | Two-earner couple, mortgage, mega-backdoor-capable plan | Itemizing vs standard, SALT cap, 415(c) room, backdoor pro-rata, two-person owner tagging |
| `P3` | Single-income couple with dependents | Child and other-dependent credits, one-earner payroll, spousal IRA |
| `P4` | Two-earner couple near retirement, mixed account types, one pension stream | RMDs, SS taxability, IRMAA on the t-2 clock, first-death and survivor single-filer years |
| `P5` | Single filer retiring before 65 on marketplace coverage | ACA PTC and the 400% FPL cliff, Roth conversions against the cliff, coverage switch at 65 |
| `P6` | Self-employed single filer, variable SE income, family HDHP | SE tax on 92.35%, no employer plan, HSA family limit, income volatility in the ledger |

---

## 4. Property-based and metamorphic invariants

`proptest` for generation, `insta` for goldens. The default per-push budget is 256 cases per property; nightly runs 10,000. Every property below names the crate it guards and the milestone it is introduced in; none is ever removed.

| # | Invariant | Statement | Crate | M |
|---|---|---|---|---|
| I1 | **Conservation of money** | `opening balances + inflows + growth − taxes − spending − debt service − bequests out == closing balances`, **residual exactly 0** in integer cents, every year, every path. Seven terms, stated exactly as `ENGINE-SPEC` §2.5 states them: `growth` is the **net** growth step 7 books, already net of the `fee` term inside `grow()`, so there is **no separate fees term** and the fee amount is a display line only (subtracting fees again would double-count them and could never reach residual 0); `inflows` includes **employer contributions and insurance proceeds**; `debt service` is interest plus principal; transfers between accounts net to zero by construction | `pfp-ledger` | M3 |
| I2 | Current-year conservation | Allocations sum exactly to the surplus; nothing is created or destroyed by the allocator | `pfp-decide` | M2 |
| I3 | No statutory cap exceeded | For any generated household, no recommendation exceeds a 402(g)/415(c)/IRA/HSA limit for that year | `pfp-decide` | M2 |
| I4 | Debt-rank monotonicity | Raising a debt's interest rate never lowers its rank in the waterfall | `pfp-decide` | M2 |
| I4b | Safe-yield floor | No debt whose after-tax rate is below the after-tax term-matched safe yield outranks every taxable option (`ENGINE-SPEC` §5.4, `DECISIONS.md` C5); the regression the pre-floor `r_rm` failed | `pfp-decide` | M2 |
| I5 | Tax monotone in income | `federal()` total tax is non-decreasing in each income character, holding others fixed | `pfp-tax` | M1 |
| I6 | Tax continuity | No discontinuity in total tax at any bracket, phase-out or worksheet boundary except where statute creates a genuine cliff; cliffs are **enumerated as data** and each is asserted to exist at exactly its parameter value | `pfp-tax` | M1 |
| I7 | Uprating idempotent and path-independent | Uprating an already-uprated table by a zero index leaves it unchanged. **Path-independence, not associativity:** the value for year *Y* depends only on `base_values`, `base_year` and the index ratio `index(Y)/index(base_year)`, so it is identical whichever intermediate years were computed first, or none. An associativity clause would be **false** under `basis: IncreaseOverBase` — a two-step path rounds the increase twice — so a correct engine would fail it and a chaining one pass. `t1/uprating/chained-path-divergence` is the negative control: the property is paired with an assertion that a year-over-year chain **does** diverge from the statutory result on at least one 2026 threshold, so neither half can be satisfied by an engine that has quietly collapsed the two computations into one | `pfp-params` | M0 |
| I8 | No negative balances | No account balance, tax line that cannot be negative, or spending line goes below zero; overdrafts surface as an explicit `Shortfall` event, never a negative cell | `pfp-ledger` | M3 |
| I9 | Account split | Splitting one account into two with the same total and allocation leaves every ledger row identical | `pfp-ledger` | M3 |
| I10 | Row-order permutation | Permuting the order of accounts, income streams or debts in the plan leaves outputs identical (guards hash-map iteration order, D8) | `pfp-ledger` | M3 |
| I11 | Homogeneity | Doubling every nominal amount *and* every dollar-denominated parameter doubles every output, up to the declared rounding | `pfp-ledger` | M3 |
| I12 | Zero-volatility equivalence | The stochastic runner with a zero-volatility generator reproduces the deterministic run **bit for bit** | `pfp-sim` | M6 |
| I13 | **Reconciliation invariant** | For every `Reason` in every `Explanation` across all six personas, three parts (`ARCHITECTURE.md` §4.2, ADR-016): **agreement** — every `Bound.value` **equals** the cell its own `Bound.ref` resolves to (ledger cell, tax line, parameter or assumption) in the same projection; **coverage** — every placeholder in the template named by `templateKey` has a `Bound` of that `name`; **reachability** — every `Bound` is rendered by its template, so an unused bound fails exactly as a missing one does. The reference travels with the value: there is no `Reason.ref` | `pfp-explain` | M2 |
| I14 | Patch / reverse-patch identity | `apply(reverse(apply(p, ops)), reverse_ops) == p` for generated plans and generated op lists | `pfp-model` | M2 |
| I15 | No-op patch stability | An empty or no-op patch leaves `inputsHash` (SHA-256 over RFC 8785 canonical JSON) unchanged | `pfp-model` | M2 |
| I16 | Scenario identity | A scenario with empty ops resolves equal to its parent; comparing identical inputs yields a zero diff | `pfp-model` | M4 |
| I17 | Migration totality | Every frozen `plan.v{N}.json` migrates to the current schema without loss; scenario patch paths are rewritten; the result validates | `pfp-model` | M2 |
| I18 | CRN variance reduction | An A/B comparison under one shared shock tensor has **lower variance** than the same comparison on independent streams | `pfp-sim` | M6 |
| I19 | Wash-sale safety | No generated trade list violates the 61-day window, across **both** people's accounts household-wide | `pfp-decide` | M9 |
| I20 | Rebalance conservation | Dollars are conserved across every rebalance, including tax withheld from a sale | `pfp-decide` | M9 |
| I21 | Insurance-grid monotonicity | Required face amount never rises when assets rise, holding everything else fixed | `pfp-decide` | **1.1** (with the `disable` bundle, `PLAN.md` M10) |
| I22 | Probability contract | A probability cannot be constructed or serialized without `max_cut`, worst-decile cut, shortfall timing and its Wilson interval. Asserted **at the API layer** (so exports cannot bypass it) and again as a component test | `pfp-sim`, `pfp-server` | M7 |
| I23 | Review attribution is exhaustive | Replaying a synthetic two-year persona history yields a review record attributing every KPI delta to **facts, assumptions, law (parameter vintage) or engine version**. The four causes are exhaustive by construction; a delta attributed to none of them, or double-counted, fails | `pfp-report` | M9 |
| I24 | Stale-pin honesty | Re-rendering a stored snapshot under a newer binary either reproduces identical numbers **or** reports the pin mismatch and shows the difference. Silently recomputing, and claiming bit-reproduction of an old engine, both fail (ADR-010) | `pfp-model`, `pfp-report` | M9 |
| I25 | **Trace-mode agreement** | `federal()`, `payroll()` and `marginal()` called with `TraceLevel::None` and with `Full` agree **to the cent on every typed accessor**, for every generated household. `None` returns totals and typed accessors with `lines` empty; `Full` additionally populates the `Line` map. The property is paired with a **sufficiency assertion**: the typed accessors are the complete in-engine interface to `FederalReturn`, so everything the engine's own arithmetic reads — including the bracket, §86 phase-in, LTCG and NIIT kink distances the gross-up uses, and the two evaluations behind `marginal()` — comes through an accessor, and `lines` is an audit and trace surface only, never read by engine code. A `lint:no-lines-in-engine` check enforces it. Without that half, `None` mode would be behaviourally incomplete and rung 0 unusable; with it, the mode the Monte Carlo runner uses is proven equal to the mode the audit trail uses rather than assumed equal (`PLAN.md` seam S3, ADR-007 rung 0) | `pfp-tax` | M1 |
| I26 | Net-worth identity | `ENGINE-SPEC` §2.5's **second** identity: net worth including debts and property reconciles year over year, so property values and debt principal cannot drift out of the balance sheet while I1 still balances on the cash-flow side. Stated over the same generated cases as I1 | `pfp-ledger` | M3 |
| I27 | **Tax cells sum once** | For every `LedgerRow`, the tax cells sum to `total.* + state.total + irmaa + ptcReconcile` **exactly once**: `tax.niit` and `tax.addlMedicare` are memo lines already inside `tax.federal` (`f8960.*`, `f8959.*`) and are never added again, `payroll()` books OASDI and regular Medicare only, and `MarginalRate::components` (four members) sums to `block × delta` with `memo.{niit, addl_medicare}` excluded. Conservation (I1) cannot detect a tax booked twice, which is why this is its own property (`ENGINE-SPEC` §2.2, §3.1, `PLAN.md` seam S3) | `pfp-ledger`, `pfp-tax` | M2 |
| I28 | **Year 0 is a full tax year** | The same generated household with `asOf` in January and with `asOf` in October yields an identical `t_now`, an identical OASDI wage-base position and identical phase-out placement; only `routable cash` and the mid-year growth exponent differ. Run with `plan.ytd` populated and with it `None` (annualization), and the caveat is asserted present in the second case (`ENGINE-SPEC` §1.4, `DOMAIN-MODEL` §3.1) | `pfp-ledger`, `pfp-decide` | M2 |

Pseudocode for I13, because it is the decision engine's only oracle:

```
for persona in personas:
    proj = project(persona, assumptions, params, mean_path, opts{trace: Full})
    for rec in next_dollar(persona, proj, year, surplus, policy):
        for reason in rec.explanation.because:
            template = templates[reason.templateKey]
            for bound in reason.values:                                   // Bound{name, value, ref}
                referenced = resolve(bound.ref, proj)                     // LedgerRef | TaxLineRef | ParamRef | AssumptionRef
                assert bound.value == referenced                          // agreement: exact Cents / Ratio equality, no epsilon
                assert template.placeholders.contains(bound.name)         // reachability: every bound is rendered
            for placeholder in template.placeholders:
                assert reason.values.any(|b| b.name == placeholder)       // coverage: every placeholder has a bound
        assert rec.explanation.flip.every(|f| sign(verdict_at(f.flipValue)) != sign(verdict_at(f.currentValue)))
```

**Rejected: inferred tolerance.** Beancount's *automatic tolerance inference* — a transaction balances if the residual is within a tolerance inferred from the written precision of the amounts — is incompatible with I1, which requires a residual of **exactly zero** in integer cents (ADR-008, `PLAN.md` M3 acceptance). The spine's rule is strictly stronger and is kept. Beancount and hledger remain useful for a different thing — the **behaviour** of booking methods and lot selection (FIFO/LIFO/specific-lot, balance assertions) — but both are GPL (Beancount GPL-2.0, hledger GPL-3.0), so they are read-only references: the M9 lot-selection cases are written from scratch against documented semantics as `hand-worked-reviewed` tier-1 fixtures, and no test file is copied. That addition is listed in §14.

---

## 5. Verification status: what is proven, what is not, and what is refused

### 5.1 Discharged by a second primary source

The 2026 Social Security constants and the whole 2026 Medicare/IRMAA dollar grid were first available to the project only through ssa.tools `constants.ts`, because `ssa.gov` and `cms.gov` returned 403/404 to automated fetching. A later re-read from primary documents supplies publisher-controlled URLs for most of them. Those items are therefore **discharged**, and the fixtures cite the primary document:

| Value | Discharged by |
|---|---|
| COLA **2.8%** (effective Dec 2025); AWI 2024 **$69,846.57**; taxable maximum **$184,500**; quarter of coverage **$1,890**; earnings-test exempt **$24,480 / $65,160** | [SSA FR Doc 2025-19763, 3 Nov 2025](https://www.govinfo.gov/content/pkg/FR-2025-11-03/pdf/2025-19763.pdf) |
| PIA bend points **$1,286 / $7,749**; family-max bend points **$1,643 / $2,371 / $3,093** | [SSA family maximum, with derivation](https://www.ssa.gov/oact/cola/familymax.html) + the same FR notice |
| Part B premium **$202.90**, Part B deductible **$283**, Part A deductible **$1,736**; IRMAA thresholds 109k/218k · 137k/274k · 171k/342k · 205k/410k · 500k/750k; Part B tier amounts **$284.10 / $405.80 / $527.50 / $649.20 / $689.90** | [CMS 2026 Parts A & B fact sheet](https://www.cms.gov/newsroom/fact-sheets/2026-medicare-parts-b-premiums-deductibles) |

### 5.2 Still unverified — one M0 gate, the M1 hand-verification gate, and later gates

No value below may be written into a locked `params/` vintage until a human has read it in the primary document and recorded the archive checksum. The gate list is shorter than the raw list of unfetched items because of §5.1, and longer where sources disagree.

| Item (why it is still open) | Gate |
|---|---|
| §402(g)(4) and §219(b)(5) **$500** uprating increments and the §414(v) catch-up rule — never fetched | **M1**, before the contribution-limit vintage locks |
| The 26 USC 1(f) **statutory base year and its per-filing-status base amounts**, and the identity of the index series behind `index = "cpi.chained"`. 26 USC 1(f)(7) establishes that rounding applies to the increase over the base year, but no archived source names the base year or the base amounts, and §3.1's pipeline cannot be written without them | **M0**, before the first `federal-2026` vintage locks — this is the one hand-verification item that gates a milestone earlier than M1, because seam S1 freezes at M0 |
| The senior-deduction phase-out **mechanic**: whether the 6% reduction applies per eligible person or per return. The attested endpoints ($175k single, $250k MFJ with two eligible) imply per person, which is what `t1/tax/emr-senior-phaseout` encodes; the statute text itself was not read | **M1**, with the senior-deduction fixtures. If it resolves the other way, the MFJ row moves to 23.32% and the endpoint moves with it |
| SALT cap phase-down schedule against statute — named by `PLAN.md` M1 | **M1** |
| Part D IRMAA adders (+$14.50 / $37.50 / $60.40 / $83.30 / $91.00) and Part D base $38.99 — sources disagree on whether the Part D figures are on the CMS fact sheet or secondary-sourced | **M3**, with the IRMAA table |
| Mega-backdoor room **derivation** — the component limits are primary-sourced, the worked number traces to a commercial blog | **M2** — ship the formula, not the blog's number |
| Roth catch-up mandate regulation dates (prior-year FICA wages > $150,000) | **M2** |
| 529 rules: $35,000 lifetime 529-to-Roth; $95,000 five-year gift election — secondary sources only | **M10** |
| 2027 ACA applicable percentages (2.15%–10.22%) | **M8**, or omit and project |
| Michigan retirement-deduction cap ($67,610 / $135,220); Washington's 2026 LTCG deduction and top rate — `michigan.gov` 403 on three routes, blogs only | **M10**, and only as a user-confirmable override |
| Survivor dollar-rounding at an early claim (OSS 908 vs truncation 907), §3.5 | **M5** |
| Vanguard backdoor-Roth BETRs (18.4% / 13.9%) — rasterized, absent from the PDF text layer | **M8**, not a fixture until hand-transcribed with a page citation |
| Hardy (2001) RSLN-2 parameters — paywalled; only the model form and log-likelihood ordering are verified | Out of v1 scope; refit locally if ever added |
| ARF worked example (60 early − 40 withheld → 20) — traces to CRS R41242, not fetchable | Confirm against POMS before it becomes a test |
| PolicyEngine's "within $100 of TAXSIM" and the NBER MOU | Never asserted; tolerances are declared here, not inherited |

Rasterized tables needing hand transcription with page citations before they can be fixtures: Guyton-Klinger Tables 6–7, Trinity 2011 Tables 1–2, Morningstar Exhibits 15–16 (including its guardrail parameters), Vanguard backdoor BETRs.

### 5.3 Refused targets — folklore, and numbers that are not data

A regression test against a number that does not exist in its cited source is worse than no test: it pins the engine to a typo. Each row below is refused **in the fixture notes**, so the refusal survives a future contributor's search.

| Refused target | Why, and what replaces it |
|---|---|
| Bengen "**4.15%**" | Appears **nowhere** in the 1994 paper (a text search returns zero hits). Encode what is in it: 4% never exhausted before **33 years**; 4.25% possible in **28**; 3% and ~3.5% never below **50**; 75/25 at 4% → **47** cohorts vs **40** for 50/50 ([FPA reprint](https://www.financialplanningassociation.org/sites/default/files/2021-04/MAR04%20Determining%20Withdrawal%20Rates%20Using%20Historical%20Data.pdf)) |
| Blanchett "**$74,146 at age 84**" | Not in the SOA paper; equation 1 yields **~$75,745**. Encode the equation: `ΔAS = 0.00008·Age² − 0.0125·Age − 0.0066·ln(ExpTar) + 54.6%` ([SOA](https://www.soa.org/globalassets/assets/files/resources/essays-monographs/2014-living-to-100/mono-li14-1a-blanchett.pdf)) |
| Historical success as a **percentage** | Denominators are tiny (55 overlapping 30-year windows) and the vendor series shifts results 1–2 windows. Assert **integer window counts**, name the series, encode the vendor spread as the tolerance (`PLAN.md` M6) |
| Guyton-Klinger **5.2–5.6%** headline | A study result resting on 1973–2004 multi-asset CMAs, not a data fixture. The **rule assertions** from cFIREsim-open are the fixture |
| Kitces guardrail triggers **$740k / $1.27M** | Outputs of one particular probability solver, not published constants |
| MoneyGuidePro "**75–90% Confidence Zone**" | Not found in vendor documentation as of 2026-09-17; only a third-party blog carries the band. Only the glossary definition (a user-selected target range) is verified; the row is in `docs/licence-watchlist.md` for re-confirmation at each release |
| A "**CBO 28% from 2032**" benefit cut | CBO's own testimony says **−7% in 2032**, then ~**28% per year 2033–2036** ([Dahl testimony](https://www.budget.senate.gov/imo/media/doc/drmollydahltestimonysenatebudgetcommittee1.pdf)). Shipped presets: none; **Trustees 22% from 2032** ([SSA TRSUM](https://www.ssa.gov/oact/TRSUM/index.html)); the CBO **two-step**. None is labelled "CBO 28% from 2032" |
| "High-interest debt = above the stock-return assumption" | The published boundary is Treasury-anchored: medium/low ≈ **10-year Treasury + 3 pp** ([Bogle Center](https://boglecenter.net/bogleheads-chapter-series-prioritizing-investments/)) |
| Pub 590-B's **2026** "Justin" half ($1,313 from $34,800 / 26.5) | Defective in the publication: the filer is 74 in 2026 and Appendix B gives **25.5** (→ $1,365). Encode the 2025 half plus the `t1/tax/rmd-age74-regression` guard |
| Vendoring the finiki/Bogleheads **VPW table** | CC BY-SA, and Bogleheads' own licence is unestablished — assume the stricter case. Generate from `pct(age,s) = r / ((1 − (1+r)^−(100−age))·(1+r))`, `r = s·0.05 + (1−s)·0.019`; keep three spot values: age 65 50/50 → **4.8%**, age 80 50/50 → **6.8%**, 1 year → **100%** ([finiki](https://www.finiki.org/wiki/Variable_percentage_withdrawal)) |
| Any RSLN-2 parameter set attributed to Hardy | The cited values are unconfirmed and the ZScore314/ESG defaults are an unrelated calibration; keep them physically separate from any Hardy citation (the project's internal research review (unpublished)) |

### 5.4 Provenance rule: re-anchor before pinning

Several canonical references were first located on **third-party file hosts** — Trinity 2011 via `desjansaar.com`, Morningstar 2025 via `static.twentyoverten.com`, Vanguard dynamic spending via `static1.squarespace.com`, Milevsky-Robinson via `rivershedge.blogspot.com`, and the one capital-market-assumption PDF the project has located, via `ipopif.org`. The loader **rejects** a `source.url` on any of those hosts, and the rule applies to `params/assumptions/` `[[source]]` blocks exactly as it applies to `fixtures/` and `params/vintages/`: a vintage citing a third-party host does not load. Each must be re-anchored to a publisher-controlled URL (e.g. Morningstar at [morningstar.com](https://www.morningstar.com/retirement/whats-safe-retirement-withdrawal-rate-2026); Milevsky-Robinson at [CFA Institute FAJ 61(6)](https://rpc.cfainstitute.org/research/financial-analysts-journal/2005/a-sustainable-spending-rate-without-simulation)) and archived with a checksum under `params/provenance/` before its values may be pinned.

---

## 6. Numerical hygiene and Monte Carlo tests

### 6.1 One test per determinism rule

`ARCHITECTURE.md` §4.3 states twelve rules. Each gets a named test and a milestone; none is enforced by review alone.

| Rule | Named test / mechanism | M |
|---|---|---|
| D1 purity (no I/O, clock, entropy, global state in engine crates) | `ci:wasm32-purity` — engine crates build for `wasm32-unknown-unknown`; `#![forbid(unsafe_code)]`; clippy `disallowed_types`/`disallowed_methods` | M0 |
| D2 cross-architecture bit-identity | `sim_bit_identity_cross_arch` — `SimResult` hash compared between `aarch64` and `x86_64` runners; plus a thread-count matrix (1, 2, 8) | M6 |
| D3 `Cents`/`Ratio`/`RoundingRule` | `money_mul_ratio_props` (round once, `i128` intermediate); `Cents * Cents` must not compile (`trybuild` compile-fail case); `t1/rounding/table` | M0 |
| D4 composite factors reduced to one rational | **`ss_claim_factor_knife_edge`** (PIA 1,000 → 700.00) | M3 |
| D5 `f64` fenced to four places | `lint:no-f64` over `pfp-tax`, `pfp-ss`, `pfp-money` (except the two named entries `Cents::grow` and `Cents::from_f64_half_even`, `SIMULATION-SPEC` §2.6) | M0 |
| D6 pure-Rust `libm`, in-repo AS241, no `mul_add` | `as241_golden_vectors`; clippy disallowed methods for platform math and `rand_distr` | M6 |
| D7 `ChaCha8Rng`, stream = path index, aggregation in path order | `sim_thread_count_independence` | M6 |
| D8 no hash-map iteration order dependence | clippy disallowed types (`HashMap`/`HashSet` in engine crates); property I10 | M0 |
| D9 rate-schedule formula is canonical | Tax-Table mode lives only under `fixtures/` tooling; a path lint fails if it appears in `pfp-tax` | M1 |
| D10 `inputsHash` = SHA-256 over RFC 8785 canonical JSON | Property I15 | M2 |
| D11 parallelism is a driver concern | CI feature matrix: `pfp-sim` with and without `parallel`, results compared | M6 |
| D12 probability carries its magnitudes | Property I22, at the API layer and as a component test | M7 |

### 6.2 Monte Carlo hygiene (M6)

| Test | Assertion |
|---|---|
| `mc_moment_match` | `σ² = ln(1 + s²/(1+m)²)`, `μ = ln(1+m) − σ²/2`. For m = 7%, s = 15%: **σ² = 0.019462**, σ = 0.139505, **μ = 0.057928**; `exp(μ) − 1 = 5.9638%` geometric vs the `m − s²/2` approximation 5.875%. Round-trip `E[R] = 0.07`, `Var = 0.0225` to **1e-9** ([log-normal identities](https://en.wikipedia.org/wiki/Log-normal_distribution)) |
| `mc_arithmetic_input_guard` | The generator's documented input is the **arithmetic** mean; a test feeds a geometric mean through the labelled conversion and asserts `E[R]`. Mislabelling these double-counts volatility drag |
| `mc_seed_reproducibility` | Same seed → identical paths across runs, thread counts and architectures (D2/D7) |
| `mc_distributional_ks` | Kolmogorov-Smirnov against the target lognormal — seed reproducibility alone does not prove the sampler correct |
| `mc_crn_variance` · `mc_zero_vol_equals_deterministic` | Properties I18 and I12 |
| `mc_wilson_interval` | Wilson 95% CI beside every rate. At p̂ = 0.90: **n = 10,000 → [0.8940, 0.9057]** (±0.59 pp); **n = 1,000 → [0.8798, 0.9171]** (±1.9 pp) ([Wilson score interval](https://en.wikipedia.org/wiki/Binomial_proportion_confidence_interval)) |
| `mc_convergence` | Run-size contract: interval width narrows monotonically across n = 1,000 (ephemeral), 5,000 (saved), 10,000+ (solvers, tails); a *saved* result at n < 5,000 is a serialization error |
| `mc_inflation_ar1` | `π_t = π̄ + φ(π_{t−1} − π̄) + σ_π ε_t` with the default `quantcalc` preset **φ = 0.4495, σ_π = 0.80%/yr** (unconditional SD 0.90%), mean editable; stationary moments recovered from a long path; the same test runs on the `high-persistence-stress` preset (φ 0.65, σ_π 1.75%, unconditional SD 2.30%) and asserts its label is `uncalibrated` on the assumptions sheet (`SIMULATION-SPEC` §5) |
| `mc_ruin_closed_form` | Milevsky-Robinson reciprocal-gamma cross-check: r = .05, v = .15, median remaining life 20 → α **3.1750**, β **0.028579**; ruin at 3/4/5/6% = **.0714 / .1387 / .2207 / .3101**, **tolerance ±0.02, never equality**. The fixture records that the numerator uses the **continuously-compounded** drift; substituting an arithmetic 5% is a specification error ([FAJ 61(6)](https://rpc.cfainstitute.org/research/financial-analysts-journal/2005/a-sustainable-spending-rate-without-simulation)) |
| `historical_replay_counts` | Bengen longevity smoke test on a **declared** series: 4% minimum longevity ≥ 33 years, 4.25% can exhaust in 28 — integer window counts, vendor named |

### 6.3 Scorecard, spending-rule and conversion fixtures (M7–M8)

| Fixture | Expected |
|---|---|
| `t2/spend/vanguard-dynamic` | $1M, 4%, ceiling +5%, floor −2.5%, inflation 3%, returns 10/5/5 → Y1 **40,000**; Y2 target 42,400 vs ceiling → **42,000**; Y3 **41,543** (ceiling 44,100, floor 40,950); real balances 1,060,000 / 1,038,582 / 1,017,206. The fixture cites the VCMM vintage and simulation count, because the rule's 5%/−2.5% choice was justified by >85% survival over 35 years in one specific edition |
| `t2/spend/guyton-klinger` | Assertions transliterated from cFIREsim-open (Apache-2.0): capital-preservation cut ×0.90 when `WR > 1.2·WR₀` and `years_left > 15`; prosperity raise ×1.10 when `WR < 0.8·WR₀`; inflation adjustment skipped after a negative-return year when `WR > WR₀` |
| `t2/spend/vpw` | Generated from the formula; three spot values only (§5.3) |
| `t1/convert/betr` (M8, `publishedPrecision` 1 dp) | Vanguard BETR closed form `BETR = t_now · G_tax / G_ira` reproduces **35% / 30.1% / 23.5% / 14.1%** by tax-payment source (IRA / tax-efficient taxable / tax-inefficient taxable / cash) at 6% return, 2% dividend yield, 18.8% on dividends and LTCG, 2% cash interest. Derivation checked in the fixture: `10,000·1.06²⁰ = 32,071.35`, `3,500·1.039²⁰ = 7,522`, `1 − (32,071 − 7,522)/32,071 = 23.45%` ([Vanguard, July 2025](https://corporate.vanguard.com/content/dam/corp/research/pdf/a_betr_approach_to_roth_conversions_072025.pdf)). The paper's no-capital-gains-on-liquidation assumption is recorded as a caveat, and the rasterized backdoor BETRs are **not** fixtures (§5.2) |
| `t1/score/probability-contract` | Property I22 |
| `t1/score/funded-ratio-strip` | Funded ratio rendered as a **sensitivity strip** (TIPS, TIPS + 1%, expected return), home equity excluded, and **never** as a pass/fail band. Asserted as a **view-model component test** (§9, layer 3), in the same place and the same way as the test that a success probability renders only alongside its magnitude companions — *not* at the API layer. The difference from I22 is deliberate and is a limitation, recorded in §14: D12 and ADR-014 give `PairedProbability` a private-fields, sole-constructor discipline, but `SIMULATION-SPEC` §11.1 specifies the funded-ratio strip only as a view-model shape (`fundedRatio: {essential: [3], total: [3], rates: [3]}`) with no constructor guarantee, so an API-layer assertion here would be testing a contract no specification defines. If a `SensitivityStrip<T>` type with that discipline later lands in `SIMULATION-SPEC` §11.1 and is listed in `ARCHITECTURE` D12, this assertion is **promoted to the API layer** and joins I22's mechanism |

---

## 7. The out-of-process oracle harness

The licence boundary is **a process and a file format, never an import** (ADR-004). The harness has two halves, and only one of them ever runs in ordinary CI.

```
cargo xtask oracle record <name>   # developer / nightly only
  1. generate a synthetic household grid (TAXSIM-style compact schema) -> requests.jsonl
  2. uv run --project oracles/<name> -- <entrypoint> requests.jsonl responses.jsonl
  3. store under fixtures/tier3/oracle/<tool>@<version>/ with a manifest
     {tool, version, lockfileHash, pythonVersion, recordedAt, gridId, requestCount}
  4. open a reviewed diff (nightly opens a PR; a changed cell is a law or oracle change)

cargo test -p pfp-tax --features oracle-verify   # every CI job, every local run
  compares engine output against the recorded JSONL. No Python. No copyleft install.
```

| Oracle | Licence | Role | Tolerance | Basis printed |
|---|---|---|---|---|
| `oracles/taxcalc` — [Tax-Calculator](https://github.com/PSLmodels/Tax-Calculator) | **CC0-1.0** (GitHub reports NOASSERTION) | The 2026 **federal** oracle | within **$5** on ≥500 synthetic cases | `oracle-checked` |
| `oracles/policyengine` — [PolicyEngine-US](https://github.com/PolicyEngine/policyengine-us), reached through the MIT [policyengine-taxsim](https://github.com/PolicyEngine/policyengine-taxsim) CLI | **AGPL-3.0** | The only 2026-capable **state and ACA** oracle | declared per state module | `single-oracle` |
| `oracles/owl` — [Owl](https://github.com/mdlacasse/Owl) | **GPL-3.0** | Conversion-planner example cases | **bounds, not targets** | `oracle-checked` (bounds) |
| SSA **AnyPIA** ([ssa.gov/OACT/ANYPIA](https://www.ssa.gov/OACT/ANYPIA/)) | US government work | PIA fixtures on synthetic earnings records | **to the dime** | `tier-1 exact` |

Mechanical rules, each a CI check:

- Each oracle is one `uv` project with its own lockfile and licence notice; **Python is pinned to 3.13 and fetched by `uv`**; the system interpreter is never used.
- `oracles/` and `tools/` are **not** Cargo workspace members and are excluded from release archives; a release-gate check greps the archive manifest for both paths. `cargo-deny`'s licence allowlist, the npm per-package check and the SBOM assert that **no GPL/AGPL/PolyForm/Parity component** reaches a shipped artifact.
- Adapters are written from scratch; **parameter-table values are never sourced from a downstream project** — a third-party default may be cited as a comparison with attribution, never adopted into `params/` (ADR-004). One downstream project projected a 2026 limit at $115,000 where the primary notice sets $111,000 — the standing reason for the constants-sync test against vendored primary text.
- TAXSIM 35 and `tenforty` are back-year **structural** checks only; neither can validate 2026 law, and neither is ever cited as a tolerance source.
- **Nightly re-record is the early warning for law changes.** A diff with no engine change means the oracle or the law moved; the triage note goes in the release PR.

---

## 8. Security test matrix

First-class corpus, green at every release gate. Playwright drives the **real binary** from M0.

| Area | Named tests |
|---|---|
| **Vault** (M1) | RFC 9106 Argon2id and XChaCha20-Poly1305 vectors; HKDF subkeys; header-as-AAD (mutating a header field fails authentication **before** parsing); byte-flip sweep always yields an *authentication* error, never a parse error or panic; out-of-bounds KDF parameters rejected on read (m > 4 GiB, t > 16, below-floor downgrade); fault-injected crash during save leaves a valid file; `vault verify` / `rekey --rotate-dek` round-trip; canary-string disk scan finds no plaintext and no WAL/journal file; the Python reference decryptor opens Rust-written files in CI **on a plan file generated at test time and never committed** (the same rule binds `tools/pfplan-ref/`: its test data is generated, so the repository never holds a container byte sequence for any reason) |
| **Keychain** (M3) | `kSecAttrSynchronizable == false`; ACL bound to the code-signing designated requirement |
| **Server and browser** (M0) | Loopback-only bind; plaintext HTTP **fails**; foreign `Host` → **421**; foreign `Origin` → **403**; `Sec-Fetch-Site` **route-scoped** (`SECURITY.md` §7.2, ADR-015): on `/api/**` a missing, `none`, `same-site` or `cross-site` value → **403**, while a top-level navigation to `/` with `Sec-Fetch-Site: none` and `Sec-Fetch-Mode: navigate` **is served** and a cross-site navigation is rejected [S-01]; exact header snapshot; auto-lock and back-off; `localStorage`/IndexedDB/Cache Storage **empty**, `sessionStorage` holding **only** the proof token; no service worker; zero third-party requests; **zero CSP violations**; trust flow green on Safari, Chrome and Firefox plus the decline/fingerprint path; full journey passes under a **deny-outbound sandbox profile** |
| **Vault, continued** (M4) | `vault compact --purge-before <date>` round-trip — metadata-only changelog entries (timestamp, action kind) survive, `reverseDiff` payloads and snapshot bodies are gone, the operation forces a whole-file rewrite and rotates backups, asserted by reading the file back with the reference decryptor; a **purge canary** writes a distinctive synthetic fact, purges it, and asserts the byte sequence is absent from the rewritten file and from the rotated backup set. Purge lands at **M4** (`PLAN.md` M4, R17), not with the M1 container, because the payloads it drops only exist from then: `reverseDiff` change-log entries arrive with the M2 action/patch chain and snapshot bodies with M4's `FactSnapshot`/`ResultSnapshot`, so at M1 there is nothing for purge to drop and nothing for the canary to find |
| **Parsers** (M4) | 1 CPU-hour `cargo-fuzz` per importer and on patch/migration per release; size and depth bounds; no XML entity expansion; a hostile file never panics and **never writes facts without review** |
| **Supply chain / repo** (M0) | `npm ci --ignore-scripts`, `npm audit signatures`, per-package licence check, `cargo-audit`, `cargo-deny`; no CDN/font/analytics origin in built assets; gitleaks pre-commit, CI and nightly full history; **container-magic gate** — the `PFPLAN\0` byte sequence *anywhere* in any file in the tree fails the commit hook, the CI diff check and the build, with **no allowlist, ever** (`SECURITY.md` §13.2 and §13.4); data-hygiene, protected-path and dollar-literal lints |
| **Release** (M0) | `codesign --verify --strict`, `spctl`, `stapler validate`; hardened runtime, no entitlements; reproducible double-build; SBOM no-copyleft assertion; signed `SHA256SUMS`; **container-magic scan over the unpacked release archive** — the embedded assets include the synthetic demo plan, and it must reach the archive as *plaintext JSON*, so a `PFPLAN\0` sequence in the archive fails the release; **first launch succeeds with networking disabled on a clean account** |

**Why the container-magic gate takes no allowlist.** The demo plan the binary embeds is `fixtures/plans/demo.plan.json` — plaintext synthetic JSON carrying `"synthetic": true` with its generator name and seed (`SECURITY.md` §13.3 rule 2). The binary encrypts it into a `.pfplan` in the user's own directory, or into a temporary plan for a Playwright run, at first use. Nothing ever commits an encrypted container, so the magic-byte check never needs an exception — and that matters, because an allowlist for *any* path would permanently blind the single control that catches a real household's plan file committed by accident, which is the T5 failure mode the control exists for. A test asserts the first-use conversion produces an openable `.pfplan` outside the tree, and the M0 build gate asserts the sequence is absent from both the tree and the archive.

---

## 9. Front-end and end-to-end tests

The front end sends typed actions and renders view-models. It **never computes money, never applies patches, persists nothing**. Three layers keep that true.

1. **Contract.** The OpenAPI document is generated from Rust DTOs by `utoipa` and **snapshot-tested**; `openapi-typescript` generates the client types, so a DTO change breaks the front-end build rather than a user session. The plan schema is published as JSON Schema via `schemars` and snapshot-tested too. Both snapshots are release-gate items. A third check rides on the second: **`lint:schema-identifiers`** resolves every stored-field identifier in the code blocks the specifications mark as reading the plan — `ENGINE-SPEC.md` §5 and §11.1, `SIMULATION-SPEC.md` §14.1, §15 and §16.3 — against `plan.schema.json`, so a field renamed in a specification but not in `DOMAIN-MODEL.md` (or the reverse) fails the build (`DOMAIN-MODEL.md` §5). It runs at M2 with the schema snapshot.
2. **Money cannot be arithmetic in TypeScript.** DTO money fields are annotated in the Rust schema (`x-money`) and a codegen post-step brands them as `Cents = number & { readonly __cents: unique symbol }`. Arithmetic on a branded value is a **type error**, so "just add these two cents fields in the component" does not compile. An ESLint rule additionally bans `localStorage`, `indexedDB`, `caches`, `navigator.serviceWorker` and any `Worker` construction in `web/src`.
3. **Component tests** (`vitest` + Testing Library): view-model rendering, explanation templates rendered from `templateKey` + values (never composed in the client), chart components fed golden view-models, and an accessibility pass. A component test asserts that a success probability renders **only** alongside its magnitude companions, and a second asserts the same for the funded ratio and its sensitivity strip (§6.3) — the probability is additionally guarded at the API layer by I22, the funded ratio is not, and §14 records why.

**Playwright, against the real binary, from M0.** Two suites:

- *Security contract* — the browser rows of §8, run against the debug binary on every PR and against the **signed artifact** at every release.
- *Persona journeys* — one journey per persona (§3.7), each exercising the milestone's new slice end to end: launch → trust → unlock → Quick Start → the milestone's number → explain panel → export → lock and reopen. `PLAN.md` states a **design intent** of about ten minutes for Quick Start and deliberately does not gate on it (§1.1, §7 item 6); the journey asserts the field count (at most the sixteen fields of `DOMAIN-MODEL.md` §18.1) and the resulting KPI set, not the wall clock.
- *Print snapshots* — `/report/one-page`, `/report/full`, `/report/review/:id` rendered under print media and snapshotted, because reports are print-CSS routes and there is no PDF library (ADR-024).
- *Accessibility* — `axe` on every journey screen; violations at serious or critical fail the gate.

---

## 10. Performance budgets

These are `criterion` gates in CI, not aspirations. Every number below comes from the spine; the tax-kernel probe count follows the **single tax-call contract** owned by `ENGINE-SPEC.md` §2.4 and recorded in `DECISIONS.md` C2 — `ARCHITECTURE.md` §4.3 owns the wall-clock gates and the per-evaluation derivation, not the counts — derived below so it can be checked rather than re-litigated.

| Budget | Threshold | Where | M |
|---|---|---|---|
| Tax-kernel probe, `TraceLevel::None` | **1,500,000** full `federal()` evaluations under **2 s** on eight Apple Silicon cores and under **8 s** single-threaded | `pfp-tax` bench | **M1** (deliberately early — risk R5) |
| Tax-kernel probe, `TraceLevel::Full` | Same 1,500,000 evaluations, **reported not gated**, so the cost of the audit trail is a measured number rather than a guess | `pfp-tax` bench | **M1** |
| Ledger-year probe | 600,000 `project()` path-years on the annual loop under **2 s** on eight cores — about **27 µs of core time per ledger-year including every tax call**; the **binding** gate | `pfp-ledger` bench | **M2** reported, **M3** gating |
| Monte Carlo, interactive | 1,000 paths × 60 years under **250 ms** on Apple Silicon | `pfp-sim` bench | M6 |
| Monte Carlo, saved/solver | 10,000 paths × 60 years under **2 s** | `pfp-sim` bench | M6 |
| Insurance grid | ~1,440 deterministic runs under **5 s** | `pfp-decide` bench | **1.1** (with the `disable` bundle) |
| Cold start | Launch to browser under **2 s** | Playwright | M0 |

**Where 1,500,000 comes from, and why it is not the gate that matters.** One `federal()` evaluation per ledger-year at 10,000 paths × 60 years would give 600,000, which is not the load the engine actually presents. The contract is fixed once (`ENGINE-SPEC.md` §2.4, `DECISIONS.md` C2): **a mean of at most 2 evaluations per accumulation ledger-year** — the federal-state two-pass, with the third pass skipped by `ENGINE-SPEC` §4.3's standard-deduction shortcut — and **a mean of at most 3 per retired ledger-year**, the same two-pass plus one gross-up settle, under a hard per-year evaluation cap of six. Averaged over a lifetime ledger that is about 2.5, so 600,000 ledger-years is **1,500,000** evaluations at the 2 s and 8 s wall-clock limits: 5.33 µs per evaluation single-threaded, 13.3 µs of tax per ledger-year, half of the 27 µs budget. Sizing the probe to the retired-year ceiling (1,800,000) on the argument that a decumulation persona is retired for most of its horizon is tempting; that concern is real, but it is answered by the **ledger-year probe**, not by inflating the kernel figure: the ledger-year gate is the normative unit, it is measured on the real call pattern of each persona including the retired ones, and it is what fails if the kernel only just meets its own number — a kernel that consumes 27 µs on tax alone leaves nothing for the other eight steps of the loop. So the kernel probe is **necessary but not sufficient** and diagnostic (it says which rung of ADR-007's ladder to pull), the ledger-year probe is **binding** from M3, and both are stated here against one contract with the same numbers as `PLAN.md` M1, `ARCHITECTURE.md` §4.3, `ENGINE-SPEC` §2.4 and `SIMULATION-SPEC` §9. An under-sized probe is a gate that passes at M1 and surfaces the shortfall at M6 with the tax kernel already written (risk R5), which is why the ledger-year probe is reported from M2 rather than waiting for M6.

If the M1 probe fails, ADR-007's contingency ladder is triggered **then**, in this order: **rung 0, suppress `Line` construction under `TraceLevel::None`** — the likeliest single win, since a full line map is 40-plus `Line` nodes each carrying two `SmallVec`s, allocated once per evaluation and discarded unread on every Monte Carlo path; then precompute path-invariant work and per-path uprated tables; an `i64` fast path where the product provably fits; denominator-specialised division; and only last an `f64` lane, guarded by a **lane-parity property test** (every named line within $1) and never used for deterministic results. Rung 0 is only available because the trace level is a parameter of `federal()` itself (seam S3: `federal(year, status, inputs, params, trace)`, likewise `payroll()` and `marginal()`) rather than a field on `ProjectOpts` that never reaches the tax function — and it is only *safe* because I25 proves the two modes agree to the cent.

---

## 11. CI pipeline, budgets and merge gates

### 11.1 Stages

**Stage 0 — pre-commit (lefthook, seconds).** `cargo fmt --check`; clippy on changed crates; gitleaks protect; data-hygiene linter; container-magic hook (**anywhere in the file, no allowlist**); dollar-literal and protected-path checks.

**Stage 1 — per push / PR (target ~15 minutes).** `fmt` · `clippy -D warnings` · `cargo-deny` · `cargo-audit` · workspace build · **tier 1 + 2 + 3 verify** (no Python, no network) · properties at 256 cases · `wasm32` purity build · `pfp-sim` feature matrix · OpenAPI and JSON-Schema snapshots · `npm ci --ignore-scripts`, `npm audit signatures`, licence check, `tsc`, `vitest` · Playwright security contract against a debug binary · `criterion` smoke run (reported, not gating).

**Stage 2 — nightly.** `cargo xtask oracle record` per oracle, opening a reviewed diff PR · `cargo-mutants` on `pfp-money`/`pfp-tax`/`pfp-ss` · `cargo-fuzz` 1 CPU-hour per target · gitleaks over **full history** · properties at 10,000 cases · cross-architecture determinism on both runners · `criterion` **gating** run against §10.

**Stage 3 — release (signed tag, manual approval, protected credentials).** Stages 1 and 2 · reproducible double-build with matching unsigned SHA-256 · sign, notarize, staple · SBOM, provenance, `cargo-auditable`, signed `SHA256SUMS` · Playwright persona journeys against the **signed artifact** · validation report regenerated into the About page.

### 11.2 Merge gates (a PR cannot land without all of these)

1. Stage 1 green.
2. **Tier 1 at 100%, no tolerance, no skips.**
3. Tier 2 skip count **not higher** than the baseline recorded in the validation report.
4. Any tier-3 golden change carries an `engineVersion` bump and a reviewed diff (ADR-010).
5. Any new engine behaviour arrives with its fixtures **in the same PR** (principle 1: no feature merges without its fixtures).
6. Touched money/tax/SS crates meet the surviving-mutant budget (§11.3); touched parsers meet the fuzz budget.
7. No new `f64` in `pfp-tax`/`pfp-ss`/`pfp-money` beyond the two named entries `Cents::grow` and `Cents::from_f64_half_even` (a third `f64`-to-money boundary is a design change needing an ADR, `SIMULATION-SPEC` §2.6), no new dollar literal in an engine crate, no new `std::fs`/`std::net`/HTTP-client dependency outside its permitted crate.
8. Threat-model delta reviewed if any row of §8 changes.
9. A new parameter table has a projection rule and a `RoundingRule` (a table without a projection rule is a CI error), a primary source, an as-of date and an archived checksum — and, where the rule's `basis` is `IncreaseOverBase`, a `base_year`, per-breakdown `base_values` and a named `index_series` that is archived rather than fetched (§3.1). Every `values` and `base_values` key is a wire form of the table's declared breakdown enum — for `filingStatus`, `single | mfj | mfs | hoh | qss` (`DOMAIN-MODEL.md` §4, §15) — so a key no `FilingStatus` deserializes fails here rather than at run time.
10. A changed public parameter value ships as a **new vintage**, never as an edit to a locked one.

### 11.3 Mutation and fuzz budgets

The spine specifies a **per-release surviving-mutant budget** on `pfp-money`, `pfp-tax` and `pfp-ss` but does not fix a number, and this document does not invent one. The **mechanism** is specified instead:

- The budget is declared per crate in `xtask/mutants.toml` as an integer count of surviving mutants, with a committed list of accepted survivors (each with a one-line justification).
- The budget **ratchets down only**. A run whose survivor count exceeds the declared budget fails the release gate; a run below it updates the budget in the same PR.
- The rationale is fixed even where the number is not: *a suite that does not notice `>=` becoming `>` at a bracket edge is not a suite.* Bracket, phase-out and threshold comparisons are the mutation operators that matter most, and a survivor there is never accepted.
- Initial values are a **project decision at M1 exit**, recorded in `DECISIONS.md`, not a researched figure.
- Fuzzing: **1 CPU-hour per target per release** (vault header, patch/migration, CSV, OFX, QIF), committed synthetic corpora, crashes triaged to zero before the tag.

---

## 12. Annual parameter update (a new tax year)

This is a **developer, release-time procedure**. v1 makes zero outbound connections (ADR-020): new law reaches a user only as a new immutable vintage inside a new release, and the app shows a per-table delta before a plan adopts it. Nothing below runs inside the shipped binary.

**Discovery (developer, each autumn).**

| Source | What it carries | Route |
|---|---|---|
| IRS Rev. Proc. (inflation adjustments) | Brackets, standard deduction, LTCG breakpoints, AMT, estate/gift | `irs.gov/pub/irs-drop/rp-YY-NN.pdf` — e.g. [Rev. Proc. 2025-32](https://www.irs.gov/pub/irs-drop/rp-25-32.pdf) |
| IRS Notice (plan limits) | 402(g), catch-ups, IRA, 415(c), compensation, HCE, QCD | e.g. [Notice 2025-67](https://www.irs.gov/pub/irs-drop/n-25-67.pdf) |
| IRS Rev. Proc. (HSA/HDHP; ACA percentages) | HSA limits; applicable percentage table | e.g. [Rev. Proc. 2025-19](https://www.irs.gov/pub/irs-drop/rp-25-19.pdf), [Rev. Proc. 2025-25](https://www.irs.gov/pub/irs-drop/rp-25-25.pdf) |
| SSA annual Federal Register notice | COLA, AWI, wage base, quarter of coverage, earnings-test exempt amounts, PIA and family-max bend points | [Federal Register API query for "Cost-of-Living Increase and Other Determinations"](https://www.federalregister.gov/api/v1/documents.json?conditions[term]=%22Cost-of-Living+Increase+and+Other+Determinations%22&conditions[agencies][]=social-security-administration), then the govinfo PDF |
| BLS CPI-W `CWUR0000SA0` | COLA inputs (Jul–Sep averages) | [api.bls.gov](https://api.bls.gov/publicAPI/v1/timeseries/data/CWUR0000SA0) |
| CMS Parts A & B fact sheet | Part B premium and deductible, Part A deductible, IRMAA tiers and amounts | [cms.gov](https://www.cms.gov/newsroom/fact-sheets/2026-medicare-parts-b-premiums-deductibles) |
| HHS/ASPE poverty guidelines | FPL for the PTC year | [aspe.hhs.gov](https://aspe.hhs.gov/poverty-guidelines) |
| Pub 590-B Appendix B | RMD tables, if changed | [irs.gov](https://www.irs.gov/pub/irs-pdf/p590b.pdf) |

**Checklist (each item is a PR-visible artifact).**

1. **Archive first.** Download each primary document to `params/provenance/<year>/`; record `sha256`, `retrieved` and the publisher URL. A third-party host is rejected (§5.4).
2. **Hand-verify every changed value** against the archived text. The assistant may transcribe; the human confirms; the assistant never authors both a constant and its test.
3. **New vintage directory** `params/vintages/federal-<year>/`. Never edit a locked vintage — a correction is also a new vintage.
4. **Declare projection and rounding per table.** No projection rule (`index | wage | flat | zero | schedule`) is a CI error; never-indexed items (NIIT thresholds, §86 base amounts, Additional Medicare thresholds, the $3,000 loss cap, the senior deduction's $6,000/$75,000/$150,000, the IRMAA top tier) carry `flat` explicitly.
5. **Constants-sync test** against text extracted from the archived document.
6. **Uprating reproduction test** — regenerate the new year's thresholds from the table's **`base_year` and `base_values`** and the archived index series, under the declared `RoundingRule`, and assert equality with the published values. Under `basis: IncreaseOverBase` the reduction is applied **once**, to the increase over the base year; regenerating from the *prior year's* published figure instead rounds twice and drifts (§3.1, I7). Archive the new index-series observations in the same PR as the documents. This is what catches a wrong rounding basis before anyone sees a wrong bracket.
7. **Per-table delta report** (`cargo xtask vintage diff <old> <new>`) reviewed in the PR: every changed cell, old → new, with its source line.
8. **Lock**: update `VINTAGES.lock`; CI fails if a previously locked file changes.
9. **Re-record the oracles**; triage the tier-3 diff — a change with no engine change means the law or the oracle moved.
10. **Golden review**: persona snapshots change; bump `engineVersion`.
11. **Hand-verification gate**: clear any §5.2 item this year's documents resolve; carry the rest forward with the reason.
12. **Regenerate the validation report** and the Assumptions Registry from the same data; release notes name every changed table and vintage id.
13. **In-app behaviour** (tested, not assumed): a result pinned to the old vintage shows as **stale** and is re-run only on request with the difference shown.

Law that is dated rather than indexed gets a scenario toggle, not a constant: the senior-deduction window (2025–2028), the SALT schedule and its 2030 step-down, the tips/overtime/car-loan deductions, and the ACA cliff — **live law as of 2026-01-01**, with a restoration bill that passed the House in January 2026 but is **not** law ([CRS R48290](https://www.congress.gov/crs-product/R48290)). Each toggle carries a fixture at both settings.

---

## 13. Validation report, governance, and what never gets cut

A **validation report** is generated at every release and embedded in the About page:

```
fixtureCounts: { tier1: {total, byModule, byMilestone, byVerification: {primarySourceConfirmed, handWorkedReviewed}},
                 tier2: {total, skipped, skipReasons}, tier3: {goldens, oracleSets} }
oracleVersions: [{tool, version, lockfileHash, recordedAt}]
mutationScore:  [{crate, surviving, budget}]
fuzz:           [{target, cpuHours, crashes}]
performance:    [{budget, measured, threshold, runner}]
properties:     [{id, cases, seedPolicy}]
unverified:     [{item, gate, milestone}]     // §5.2, printed, not hidden
refused:        [{target, reason, source}]    // §5.3
pins:           {engineVersion, schemaVersion, paramVintageIds, binaryDigest}
```

[ASOP No. 56 (Modeling)](https://www.actuarialstandardsboard.org/asops/modeling-3/) is the per-release model-governance checklist: intended purpose, reliance on third-party models (the oracles, named with versions), model risk mitigation (the three tiers, properties, mutation, fuzz), and documentation of known limitations (the `NotModelled` flags, the `Fidelity` labels, the printed omissions on the Roth card).

Per the cut rule (`PLAN.md` §4.12), **never cut**: fixtures, the security suite, the release gate, result pinning, the explanation invariant, **the current-year parameter vintage** (the MV milestone whose acceptance criteria are §12, so a cut cannot make the numbers on screen stale) and **the 529 option of capability (b)** (`PLAN.md` §1.1; so a cut cannot silently drop a named capability).

---

## 14. Deviations from the spine

Everything above is consistent with `PLAN.md`, `ARCHITECTURE.md` and `DECISIONS.md` except the following.

1. **Beancount tolerance inference is refused** (§4). Automatic tolerance inference for ledger balancing contradicts the conservation identity (residual **exactly 0** in integer cents; `PLAN.md` M3, ADR-008). The spine's rule is stronger and wins.
2. **Booking-method and lot-selection cases are added at M9** (§4), written from scratch against the documented semantics of Beancount (GPL-2.0) and hledger (GPL-3.0), which are read-only references under ADR-004 and `ARCHITECTURE.md` §10.3; no test file is copied and no `NOTICE` entry arises. This is the only corpus source introduced here that the spine does not name; it is additive.
3. **The mutation-budget number is left open** (§11.3). The spine specifies a budget without a figure and no research input supplies one; the mechanism is specified and the initial integers are a project decision at M1 exit.
4. **Unverified-item bookkeeping is reconciled across sources** (§5.1–§5.2). The 2026 SS constants and the CMS/IRMAA grid were first available only through a downstream project; a primary re-read supplies publisher-controlled URLs for most of them, so those items are **discharged** and the gate list is shorter than the raw list. Part D adders, where sources disagree, stay flagged.
5. **A survivor-benefit rounding conflict is surfaced, not resolved** (§3.5, §5.2): OSS's 908 implies nearest-dollar rounding where SSA's rule is truncation (907.57 → 907). Pinned against POMS at M5.
6. **The front-end money brand (`x-money` → branded `Cents`) is a new mechanism** (§9). The spine names a source lint and review; branding makes client-side money arithmetic a type error. A strengthening, not a change of contract.
7. **`docs/licence-watchlist.md` is a new artifact** (§2.3), because TPAW's PolyForm Noncommercial (reported `NOASSERTION`) and AnyPIA-js's absent licence are invisible to `cargo-deny` and `license-checker`.
8. **The funded-ratio strip is guarded at the view-model layer, not the API layer** (§6.3, §9). `ARCHITECTURE` D12 and ADR-014 give the paired probability a constructor-level discipline and I22 asserts it at the API boundary; `SIMULATION-SPEC` §11.1 specifies the funded-ratio strip only as a view-model shape, so there is no type whose construction could be constrained. The weaker guard is stated rather than pretended, and promotes to I22's mechanism if a `SensitivityStrip<T>` type is added to `SIMULATION-SPEC` §11.1 and listed in D12. This is the only TESTING-only choice among these corrections; the rest — the EMR senior-phase-out targets, the base-year uprating inputs, the `trace` parameter on `federal()`, the single tax-call budget, the seven-term conservation identity and the no-allowlist container-magic gate — are stated in every document at once and so are not deviations.
9. **A fourth `verification` value, `hand-worked-reviewed`, is added to the tier-1 fixture schema** (§2.2). The spine requires tier 1 to be primary-sourced, but the two capabilities with no external oracle — the ledger and the next-dollar engine — have no publication to transcribe. Rather than weaken the gate for everyone or exclude those corpora from tier 1, the loader admits a second, narrower value that demands `synthetic: true`, a full derivation and a recorded sign-off under §2.2's review rule (a second reviewer, or the cooling-off re-review for a single maintainer, `PLAN.md` R26), and the validation report prints the two counts separately.
