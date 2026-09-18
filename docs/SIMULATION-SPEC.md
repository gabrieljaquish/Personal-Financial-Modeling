# SIMULATION-SPEC

*Specification of the stochastic and retirement layer. Date: 2026-09-17. Companion to `PLAN.md` (what and when), `ARCHITECTURE.md` (how) and `DECISIONS.md` (why); where this document and the spine disagree, the spine wins and section 19 lists every known tension. All examples are synthetic or published third-party worked examples. Nothing here describes any real household.*

**Terminology (standing definition).** "The binary" or "the release" always means one thing: a **local web application for macOS**, shipped as a single self-contained executable (universal: Apple Silicon + Intel). Launching it starts a web server bound to loopback, serves the embedded web app over TLS and opens the default browser. The user interface is a web app in the browser; it is not a native desktop GUI and not an Electron/Tauri window. Consequence for this document: **every computation specified here runs in the Rust backend process of that local web application.** The browser never generates a random number, never computes a percentile and never sees a path; it sends typed actions, receives NDJSON progress over `fetch`, and renders view-models (ADR-015, ADR-017). There is no browser-side or WASM engine.

**Numbers policy.** Every numeric default below cites the public source it came from, or says that it rests on the project's internal research review, which is not published. Items no archived primary document confirms are tagged **(unverified)** and may not enter a locked vintage before the hand-verification gate (PLAN M1, ADR-010). Where a source supplies a sign or a range but no value, the field ships **uncalibrated** and says so on the assumptions sheet; this document does not invent a number.

---

## 1. Scope and milestone map

| Section | Content | Crate | Milestone |
|---|---|---|---|
| 2 | `ReturnGenerator`, `ReturnPath`, `simulate()` types; rule registry | `pfp-ledger` (traits), `pfp-sim` | Seam S6 frozen **M2**; generators **M6** |
| 3 | Capital-market-assumption (CMA) vintages | `params/assumptions/`, `pfp-params` | The vintage shape, loader and user-entry path ship at **M2**; the first entered vintage carries means, SDs **and** the correlation matrix, because the M2 deterministic path (4.1) needs the covariance to compute a median growth rate; no third-party set is bundled until its redistribution terms are confirmed (3.2) |
| 4.1 | Deterministic mean path | `pfp-ledger` | **M2** (one year), **M3** (full horizon) |
| 4.2-4.5 | Lognormal iid, circular block bootstrap, historical replay, named stresses | `pfp-sim` | **M6** |
| 4.6-4.7 | Two-phase regime, Student-t stress | `pfp-sim` | **After 1.0** (PLAN section 5); specified now so the interface needs no change |
| 5 | AR(1) inflation | `pfp-sim` | **M6**; the CPI refit is a **blocking** item for the M6 vintage lock |
| 6 | Planning age; stochastic mortality | `pfp-ledger`; `pfp-sim` | **M3**; **M7** |
| 7-9 | RNG, seeds, common random numbers, trial counts, Wilson, performance | `pfp-sim` | **M6** (performance probe **M1**) |
| 10-11 | Outcome metrics, paired probability type, "enough" scorecard, FI milestones | `pfp-sim`, `pfp-decide` | KPIs **M2/M3**; scorecard **M7** |
| 12 | Spending rules | `pfp-ledger` (constant real), `pfp-sim` | **M3**; rest **M7** |
| 13 | Withdrawal sequencing | `pfp-ledger` | Conventional **M3**; proportional, bracket-managed **M8** |
| 14 | Social Security | `pfp-ss` | Crate created **M3** with the claim-age factors, `ss.fra.retirement` and the `BenefitAtAge` → PIA back-solve for the user-entered estimate (ARCHITECTURE section 3); AIME/PIA, spousal, survivor, earnings test **M5**; claiming grid **M8** |
| 15 | Pensions and annuities | `pfp-model`, `pfp-ledger` | Streams **M3**, survivor scaling **M4**; pricing **after 1.0** |
| 16 | Allocation, glide paths, rebalancing, asset location, trade list | `pfp-decide`, `pfp-ledger` | Static allocation **M3**; everything else **M9** |
| 17 | Fan charts, percentile tables, assumptions sheet | `pfp-sim`, `pfp-report`, `web/` | **M6**, extended **M7** |
| 18 | Validation matrix | `fixtures/`, CI | Per milestone |

Cut rule (PLAN 4.12, five ranked tiers): two items in this document are on the list — the **monthly 62-70 claiming grid** (14.6) is tier 3 and the **trade-list generator** (16.5) is tier 4; nothing else in this document is cuttable. The 62 / FRA / 70 comparison, the review loop, the target-allocation choice and the rebalancing policy stay.

---

## 2. Core types and interfaces

### 2.1 Asset classes

A closed, ordered registry, **defined in `DOMAIN-MODEL.md` §2.1 (`pfp-domain`) and re-exported** — the block below is a verbatim reference copy, not a second declaration (ARCHITECTURE section 2's one-home CI check), and the same is true of `Seed` in 2.3. The index is part of the RNG addressing contract (section 7) and is never reordered; new classes are appended.

```rust
#[repr(u8)]
pub enum AssetClass { UsLarge=0, UsSmall=1, IntlDev=2, EmergingMkts=3,
                      UsAggBond=4, TreasuryInt=5, Tips=6, Cash=7, /* 8..=23 reserved */ }
pub const MAX_CLASSES: usize = 24;
```

An account's allocation is `BTreeMap<AssetClass, BasisPoints>` summing to 10,000. Weights are integer basis points so that rebalancing dollars are exact `mul_ratio` results (ADR-007); glide-path outputs are rounded to whole basis points with the remainder assigned to the largest class.

### 2.2 Return path (frozen with seam S6 in M2)

```rust
pub struct ReturnPath {
    pub returns:   Vec<[f64; MAX_CLASSES]>, // returns[t][class], simple annual nominal return, > -1.0
    pub inflation: Vec<f64>,                // inflation[t], annual CPI change
    pub death_year: [Option<Year>; 2],      // p1, p2; None = use the plan's planning age
    pub valuation: Option<Vec<f64>>,        // optional CAPE-like series for valuation rules (12.6, 16.2)
    pub provenance: PathProvenance,         // Mean{basis: MedianGrowth | ArithmeticMean}
                                            //   | Sampled{path_idx} | Window{start} | Stress{name}
}
```

`project()` never knows the source of a path (the project's internal research review (unpublished)). `death_year` and `valuation` must exist as optional fields **from M2** even though nothing fills them until M6/M7, so the S6 seam never changes shape.

### 2.3 Generator trait (ARCHITECTURE section 7 signature, plus `path_count`; see 19.4)

```rust
pub trait ReturnGenerator: Sync {
    fn generate(&self, seed: Seed, path_idx: u32, horizon: usize, a: &AssumptionSet) -> ReturnPath;
    fn describe(&self) -> GeneratorConfig;       // serialized into result pins and the assumptions sheet
    fn path_count(&self, horizon: usize) -> PathCount; // Unbounded | Exactly(n): replay and stresses are finite
}
pub struct Seed(pub [u8; 32]);  // reference copy of DOMAIN-MODEL 2.1; created by pfp-server (engine crates have no
                                // entropy, D1); stored on the Scenario; JSON: 64 lowercase hex characters
```

`generate` is a pure function of its arguments: calling it twice, on any thread, on either architecture, returns bit-identical output (D2).

### 2.4 Simulation entry point (ARCHITECTURE 4.1, verbatim) and options

```rust
pub fn simulate(plans: &[ResolvedPlan], assumptions: &AssumptionSet, params: &ParamView,
                generator: &dyn ReturnGenerator, seed: Seed, n_paths: u32, opts: &SimOpts) -> SimResult;

pub struct SimOpts { pub criterion: CriterionId, pub mortality: MortalityMode,  // PlanningAge | Sampled{table, multiplier}
                     pub percentiles: &'static [f64], pub keep_path_metrics: bool, pub rules: &'static RuleRegistry }
pub struct SimResult { pub per_plan: Vec<PlanSimSummary>, pub pairwise: Vec<PairedDelta>, pub sheet: AssumptionsSheet, pub pin: ResultPin }   // ResultPin: DOMAIN-MODEL 16
```

For each `path_idx`, `simulate` generates **one** `ReturnPath` and runs `project()` on it for **every** plan in the slice, so common random numbers are a property of the API, not of caller discipline (ADR-013). Paths are never stored; `PlanSimSummary` holds per-year percentile vectors, metrics and histograms only (ADR-005).

### 2.5 Rule registry and crate placement

`pfp-sim` depends on `pfp-ledger`, never the reverse. Therefore the traits `ReturnGenerator`, `SpendingRule`, `WithdrawalOrder`, `GlidePath` and `Criterion` are **defined in `pfp-ledger`** together with the M3 built-ins (mean path, constant real, conventional order, static allocation, never-below-zero). `pfp-sim` and `pfp-decide` contribute further implementations to a compiled-in static `RuleRegistry` that the caller passes through `ProjectOpts`/`SimOpts`. Policies stored in the plan (seam S7) reference rules by stable id plus parameters: `{"rule": "vanguard_dynamic", "ceiling": "0.05", "floor": "-0.025"}`. Unknown ids fail validation; nothing is loaded dynamically (ADR-021).

### 2.6 Float fence

This layer is where three of the four permitted `f64` uses live (D5): generation (a), statistics (c) and solvers (d). Spending-rule arithmetic (PMT, percentage-of-portfolio, guardrail bisection) is classified under (d).

There are exactly **two** `f64`-to-money entry points in `pfp-money`, and the count is auditable because both are named:

1. `Cents::grow(factor)`, the single permitted growth step. On a sampled, replayed or stress path `factor = sum_c w_c * (1 + R_c)`, computed in ascending class-index order (no `mul_add`, D6). On a `Mean{basis: MedianGrowth}` path it is the account's median growth factor of 4.1, computed by the same ascending-order reduction from the account's weights and the vintage covariance.
2. `Cents::from_f64_half_even(x)`, through which **every** spending rule and solver returns money. From there on the ledger is integer.

ARCHITECTURE D5(b) and ADR-007 name both entries, and the TESTING 6.1 lint exception list and TESTING 11.2 merge gate 7 carry the same pair (recorded as deviation 6 in section 19 when this document first needed the second constructor). No third boundary is added; a new one is a design change, not a refactor.

---

## 3. Capital-market-assumption vintages

### 3.1 File shape

Immutable, content-hashed TOML under `params/assumptions/`, locked by `VINTAGES.lock` (ADR-010). A plan references one by id; overrides live in the plan's separate override layer and are displayed beside the sourced value.

```toml
id = "user-entry-2026"       # vintage id becomes user-entry-2026@<sha256-prefix>
as_of = "YYYY-MM-DD"         # the date the entered figures are stated for
basis = "nominal"            # nominal | real  (real vintages require inflation.mean; the loader converts)
horizon_years = 10
mean_kind = "arithmetic"     # arithmetic | geometric (geometric is converted, 3.3)

[[class]]
id = "us_large"; mean = "0.065"; sd = "0.155"; geometric = "0.054"     # synthetic illustration values
# ... one block per AssetClass ...

[correlation]                # upper triangle, class order of section 2.1; row "inflation" optional
us_large = { us_small = "0.9", intl_dev = "0.8", em = "0.7", us_agg = "0.4", treasury_int = "0.2", cash = "0.0" }

[inflation]
mean = "0.027"; preset = "quantcalc"; phi = "0.4495"; sigma = "0.0080"   # section 5; a published third-party fit,
                                                                           # labelled as such until the refit gate
initial = "mean"; shock_corr_bonds = "uncalibrated"
chained_cpi_wedge = "0"                                                    # ENGINE-SPEC 1.3; DOMAIN-MODEL 15

[market_data]                # dated values; v1 never fetches them (ADR-020)
tips_real_yield_20y = { value = "...", as_of = "..." }
treasury_10y        = { value = "...", as_of = "..." }
cape                = { value = "40.52", as_of = "2026-09-16" }

[[source]]                   # who stated the figures: the user ("entered by the user from <publisher>, <edition>"),
                             # or, for a bundled vintage, the publisher-controlled URL, retrieval date and archive checksum
title = "..."; url = "..."; retrieved = "..."; sha256 = "..."
```

### 3.2 Vintages: the shape ships, the numbers do not

**Third-party capital-market-assumption sets are not bundled until their publisher's redistribution terms are confirmed and recorded in `docs/licence-watchlist.md`.** Published forecasts are proprietary research output; republishing their tables inside an Apache-2.0 repository is a right the project does not hold until the publisher grants it. v1 therefore ships the vintage file **shape** above, the **loader** (with the geometric-to-arithmetic conversion of 3.3 and the positive-definite repair of 3.4), published SHA-256 checksums for any file the user obtains, and **the user's own entry**: the assumptions review step of setup collects the per-class means, SDs and correlations (or a geometric mean the loader converts), and every number is displayed beside its `[[source]]` block. This is the same rule 4.4 applies to historical datasets, and `DOMAIN-MODEL.md` §15 and `ARCHITECTURE.md` §10.3 state it in the same words. A vintage whose publisher has granted permission ships as a named, immutable vintage with the permission recorded in the watchlist; none has at the date of this document.

Published sets the user may obtain and enter, named here as references only (no figure from any of them is reprinted in this document or in `params/`):

| Reference | What it publishes | Source |
|---|---|---|
| Verus Capital Market Assumptions (annual) | Geometric and arithmetic means, autocorrelation-adjusted SDs and a ten-year correlation matrix — the one set with everything the lognormal generator consumes | Publisher-controlled URL to be recorded at the gate; the only copy the project has located was hosted by a third party and, under `TESTING.md` §5.4, cannot be pinned as a source |
| J.P. Morgan Long-Term Capital Market Assumptions (annual) | Compound ten-to-fifteen-year returns | [J.P. Morgan LTCMA](https://am.jpmorgan.com/us/en/asset-management/institutional/insights/portfolio-insights/ltcma/) |
| AQR Capital Market Assumptions (annual) | Valuation-based real returns (`basis = "real"`) | [AQR](https://www.aqr.com/Insights/Research/Alternative-Thinking/2026-Capital-Market-Assumptions-for-Major-Asset-Classes) |
| Northern Trust Capital Market Assumptions (annual) | Geometric ten-year returns | [Northern Trust](https://ntam.northerntrust.com/content/dam/northerntrust/investment-management/global/en/documents/thought-leadership/2026/cma/2026-capital-market-assumptions-report.pdf) |
| Vanguard Capital Markets Model forecasts | Ten-year geometric ranges (the user enters a midpoint) | [Vanguard](https://corporate.vanguard.com/content/corporatesite/us/en/corp/vemo/vemo-return-forecasts) |

Several vintages side by side are shown on purpose: CMA choice moves results more than the simulation method. A set entered without SDs or correlations borrows them from another entered set, and the assumptions sheet says so; correlation entries the user has not entered are never inferred. Third-party names identify published methodologies and imply no affiliation, sponsorship or endorsement (ADR-004).

**Stock-bond correlation knob.** Published ten-year matrices put US large vs core bonds near 0.4 while the long-run value is near zero (-0.3 to +0.3). The plan-level override `stockBondCorrelation` defaults to **0.2** (range 0-0.4) and replaces every equity x nominal-bond entry; "use the vintage's matrix" is one click.

**Optimistic / pessimistic variants** are derived, not typed: +/-20% around each entered mean ([Boldin](https://help.boldin.com/en/articles/11049646-assumptions-for-inflation-and-appreciation) convention).

### 3.3 Geometric to arithmetic conversion

Generators consume **arithmetic** means. Feeding a CAGR into a draw double-counts volatility drag (about 1.1%/yr at 15% volatility, 5-10 points of 30-year success; [Praxion](https://www.praxionfinance.com/articles/monte-carlo-retirement-simulations/); Boldin corrected this in July 2025, [Boldin](https://www.boldin.com/retirement/understanding-boldins-monte-carlo-simulation-what-it-is-why-it-matters-and-whats-new/)). For a geometric input `g` with SD `s` the loader solves the exact lognormal identity rather than the `g + s^2/2` shortcut:

```
(1+g)^2 = (1+m)^2 / (1 + s^2/(1+m)^2)   =>   (1+m)^2 = [(1+g)^2 + sqrt((1+g)^4 + 4 s^2 (1+g)^2)] / 2
```

Golden check: a synthetic class with `g = 5.4%, s = 15.5%` converts to `m = 6.51%` (the 3.1 example's arithmetic 6.5% at that class's published precision). A vintage whose `mean_kind` is absent is a CI error.

### 3.4 Positive-definite repair

Before use, the matrix (with the knob applied) is tested by Cholesky. On failure: symmetric Jacobi eigen-decomposition (in-repo, `k <= 25`, `libm` `sqrt` only), clip eigenvalues at `1e-10`, recompose, rescale to unit diagonal, retry. `pd_repaired: true` and the largest absolute entry change are printed on the assumptions sheet.

---

## 4. Return generators

### 4.1 `MeanPath` (deterministic) - M2/M3

The path always carries `returns[t][c] = m_c` (the vintage's arithmetic mean, which is the correct expected return for anything that reads a class return directly) and `inflation[t] = pi_bar`. What differs between the two bases is the **growth factor the ledger compounds**, because a path compounding at the arithmetic mean sits above the median stochastic outcome by the volatility drag - about 1.1%/yr at 15% volatility (3.3), which is 35-40% of terminal wealth over 30 years at a high equity share. Every pre-M6 output is deterministic (FI dates, the M3 Roth-vs-Traditional verdict and its `t_future`, the conversion-planner baseline, the insurance grid, the funded ratio), so shipping the arithmetic path as the default would bias all of them one way: toward Roth, toward "on track" and toward lower insurance need.

**Default `basis: MedianGrowth`.** Per account and year, from that account's weights `w` and the vintage's arithmetic means `m` and covariance `Sigma[i][j] = rho_ij s_i s_j`:

```
m_p    = sum_c w_c * m_c                                  # portfolio arithmetic mean
s_p^2  = w' Sigma w                                        # portfolio variance of the simple return
factor = (1 + m_p) / sqrt( 1 + s_p^2/(1 + m_p)^2 )         # = 1 + g_p, the 3.3 identity at portfolio level
```

This is the exact lognormal geometric/arithmetic identity of 3.3 applied once to the portfolio, not per class: per-class geometric means do not combine linearly, which is why the phrase "the implied geometric mean" is not used anywhere in this document. `Sigma` is assembled by the vintage loader from the per-class `sd` values and the `[correlation]` upper triangle of 3.1, **after** the stock-bond knob of 3.2 is applied and **after** the positive-definite repair of 3.4 - the same matrix `LognormalIid` consumes, so the deterministic path, the iid generator and `v` in 12.8 cannot disagree about the same portfolio. `project()` receives `&AssumptionSet` (ARCHITECTURE 4.1), so `Sigma` and the account's current weights are both in hand at the growth step, which is where the factor is computed and memoized per `(account, year)` on first use. It is **not** precomputed in `PlanProgram::compile`: compilation runs once per plan, before any path exists, and so cannot know which basis - or which generator - will produce the path it is compiled for (9, step 1 precomputes only genuinely path-invariant work).

**`basis: ArithmeticMean`** keeps `factor = sum_c w_c (1 + m_c)` and is offered as a **labelled overlay only**, drawn beside the default and captioned "arithmetic mean, no volatility drag - above the median outcome". It is never the basis of a stored verdict.

Goldens:

- **Algebraic (Tier 1, exact).** A one-class account on the synthetic vintage of 3.1 (`m = 6.51%`, `s = 15.5%`) grows at the `g = 5.4%` of 3.3 to 1e-12; a zero-volatility vintage makes the two bases identical to the cent.
- **Statistical (Tier 3, committed interval).** For a no-cash-flow account at a fixed seed, `n = 10,000` `LognormalIid` paths over 30 years: the MeanPath-`MedianGrowth` terminal value lies inside the 95% bootstrap confidence interval of the sampled median (2,000 resamples). The interval is computed once and **committed with the fixture**, so the gate is a comparison against a pinned number rather than a re-derived statistic; a bare point equality would be a flaky gate. The same construction run against `ArithmeticMean` must fall outside the interval - that is the test that would have caught the old default.

### 4.2 `LognormalIid` - M6 (default stochastic generator)

Inputs: arithmetic means `m_i`, SDs `s_i`, correlation `rho_ij`. Multivariate moment match in log space, which reproduces the arithmetic means **and** covariances exactly:

```
Sigma_log[i][j] = ln( 1 + rho_ij * s_i * s_j / ((1+m_i)(1+m_j)) )      # diagonal: sigma_i^2 = ln(1 + s_i^2/(1+m_i)^2)
mu_i            = ln(1+m_i) - Sigma_log[i][i] / 2
L               = cholesky(Sigma_log)                                  # after 3.4
z[t]            = [ AS241(u(seed, path, t, slot=c)) for c in classes ] # section 7
R[t][i]         = libm::exp( mu_i + sum_{j<=i} L[i][j] * z[t][j] ) - 1
```

Mandatory golden check: `m = 7%, s = 15%` gives `sigma^2 = 0.019462`, `mu = 0.057928`, geometric `5.9638%` (the shortcut `m - s^2/2` gives 5.875% and must fail the test) to 1e-9. Second vector: `m = 6.7%, s = 15.5%` gives `mu = 0.0544`, `sigma = 0.1445`, geometric about 5.5% (the same identity applied).

One draw **per asset class per period**; each account's return is its allocation-weighted sum. Per-account independent draws and a single shock shared by all classes are both refused (the project's internal research review (unpublished)).

Known bias, printed beside results: at 70-90% success iid draws recommend 5-10% more income than historical simulation, and in the tails they produce paths worse than any in history, because iid ignores mean reversion ([Kitces 2022](https://www.kitces.com/blog/monte-carlo-simulation-historical-returns-sequence-risk-calculate-sustainable-spending-levels/)).

### 4.3 `CircularBlockBootstrap` - M6

Operates on a loaded `HistoricalDataset` (4.4) of joint rows `(class returns..., cpi)` so cross-correlation and inflation co-movement survive.

```
config: block_years in {1, 3, 5, 10} (default 5 = 60 months), length: Fixed | Geometric(mean = block_years),
        recenter: bool (default true), recenter_inflation: bool (default true)
t = 0
while t < horizon_periods:
    start = floor( u(seed, path, t, slot=BLOCK_START) * n_rows )
    len   = block (Fixed)  |  1 + floor( ln(u(.., slot=BLOCK_LEN)) / ln(1 - 1/block) )  (Geometric)
    for k in 0..len: row[t+k] = data[(start + k) mod n_rows]            # circular: sample ends are not under-weighted
    t += len
monthly datasets: 12 consecutive rows compound into one annual return; CPI likewise
```

**Re-centring, stated exactly** (the level and the reference mean are both choices worth tens of basis points a year, so neither is left to the implementation):

```
applied to the ANNUAL COMPOUNDED return, after the 12-row compounding - never to monthly rows
  R'[c] = (1 + R[c]) * (1 + m_c)   / (1 + mu_hist_c)   - 1
  pi'   = (1 + pi)   * (1 + pi_bar)/ (1 + mu_hist_cpi) - 1          # when recenter_inflation (default)
reference mean: mu_hist_c = arithmetic mean of ALL n_rows circular 12-month windows of class c,
  each window used exactly once - not the mean of the monthly rows, and not the mean of calendar years
```

The reference has to be the mean of the same population the sampler draws from, which is the circular window set. `recenter_inflation` defaults **on** because re-centring nominal returns while leaving the historical CPI series untouched (its mean is well above the long-run mean a current vintage carries, 2.7% in the synthetic example of 3.1) silently moves every class's **real** mean relative to `LognormalIid` on the same vintage - and the side-by-side display of section 17 labels the spread between generators "model uncertainty", a reading that is only honest if the generators share a real mean. A `recenter_real` variant, which re-centres real returns and lets nominal fall out, is offered with the same reference-mean rule.

Golden: running the full circular sample (every window drawn exactly once, `block_years = 1`, re-centring on), the arithmetic mean of each class's annual return and of CPI equals the vintage's `m_c` and `pi_bar` to 1e-6.

Single-year blocks are offered but labelled: they destroy autocorrelation and read more pessimistic than cohorts. Non-circular blocks are not offered.

### 4.4 `HistoricalReplay` and datasets - M6

**Datasets are not bundled** until redistribution terms are confirmed (PLAN R16; Shiller and Damodaran carry no explicit licence). v1 ships a loader, a documented CSV format and published SHA-256 checksums of the known-good derived files:

```
period,us_large,us_small,intl_dev,em,us_agg,treasury_int,tips,cash,cpi[,cape]
1928,0.4381,...          # annual "YYYY" or monthly "YYYY-MM"; simple returns; blank = class absent
```

A header comment records vendor, retrieval date and derivation (bond returns from Shiller data must be derived from yields). Classes absent from a dataset are mapped by the user to a present proxy or to the mean path; the mapping is on the assumptions sheet. Files enter through the import endpoint and are held in the plan file; v1 never fetches them (ADR-020).

Replay runs **every start with a full horizon** through the unchanged ledger: `path_count = Exactly(n_rows - H + 1)`. Where the horizon exceeds available data, the tail is spliced with the mean path and those windows are counted separately. Output type:

```rust
pub struct WindowCount { pub survived: u32, pub total: u32, pub spliced: u32, pub dataset: DatasetId,
                         pub horizon: u16, pub worst_start: Period, pub min_end_wealth: Cents, pub earliest_depletion: Option<Year> }
```

`WindowCount` has no probability field and no `Serialize` path that produces one: cohorts are cases, not probabilities. Only 56-61 rolling 30-year annual windows exist since 1926 ([McLean](https://www.mcleanam.com/monte-carlo-simulations-vs-historical-simulations/), [Retirement Researcher](https://retirementresearcher.com/advantages-monte-carlo-simulations/)), and long horizons have far fewer.

### 4.5 Named stresses - M6

Finite generators (`path_count = Exactly(1)` unless noted) rendered as labelled lines, never mixed into a probability.

| Stress | Definition | Source |
|---|---|---|
| `BadTiming` | Year 1 `m_c - 2 s_c`, year 2 `m_c - s_c` for the selected classes (default: equity classes); later years re-centred to `m_c + 3 s_c/(H-2)` so the horizon arithmetic mean is preserved. The assumed path is displayed because re-centred years can look unrealistically high | MoneyGuidePro stress menu ([glossary](https://www.guidestar.org/ViewEdoc.aspx?eDocId=11065603&approved=true)) |
| `BearMarketReplay` | Apply the Nov 2007-Feb 2009 S&P 500 total return of **-50.95%** to equity classes in year 1, mean path after. The bond leg needs a loaded dataset; without one bonds stay at mean and the sheet says so | same |
| `CohortPreset{1929, 1937, 1966, 1973, 2000, 2008}` | `HistoricalReplay` window starting at that year; requires a dataset | The project's internal research review (unpublished) |
| `WorstFirst{inner, k}` | Combinator: take any inner generator's path and move its `k` worst portfolio years to the front (default `k = 3`). Reported effect on a 1987-2018 bootstrap: 30-year success falls to 67% | [ERN guest post](https://earlyretirementnow.com/2019/07/10/monte-carlo-plan-for-retirement-guest-post-gasem/) |

"Fear sliders" (higher inflation, a Social Security cut, ten more years of life, returns -1%) are **scenario ops** of `kind: stress` (M4), not generators.

### 4.6 `TwoPhaseRegime` - after 1.0

Mean-preserving: lower returns in years 1-10, higher afterwards, constrained so the horizon mean and SD equal the iid baseline. Reference calibration for a 60/40 portfolio in **real** terms: 0.33%/month (SD 3.6%) for years 1-10, 0.57%/month (SD 2.8%) for years 11-30, against an iid baseline of 0.50%/month (SD 3.1%) ([Kitces 2022](https://www.kitces.com/blog/monte-carlo-simulation-historical-returns-sequence-risk-calculate-sustainable-spending-levels/)). The user sets the near-term haircut `d`; the generator solves the later mean from `(10(m-d) + (H-10) m') / H = m`. A fitted regime-switching model is a standing non-goal (its commonly quoted parameters are **(unverified)**).

### 4.7 `StudentT` stress - after 1.0

Standard `t_nu` scaled by `sqrt((nu-2)/nu)` (0.775 at `nu = 5`, 0.577 at `nu = 3`); one chi-square divisor per period so tails co-move; `nu <= 2` rejected. Always a labelled stress: a 95% plan reads 85-88% at `nu = 5` ([Retirement Lab](https://retirement-lab.com/learn/blog/fat-tails-retirement-planning/)). Sobol sampling is a non-goal.

---

## 5. Inflation model (M6)

```
pi_t = pi_bar + phi * (pi_{t-1} - pi_bar) + sigma_pi * eps_t,     eps_t = AS241(u(seed, path, t, slot=INFLATION))
unconditional SD of the process = sigma_pi / sqrt(1 - phi^2)     # what the 95% band is actually made of
pi_0 = pi_bar unless the vintage carries a dated initial value
```

**Two presets: the fitted process is the citable default, the unfitted heuristic is a labelled stress.**

| Preset | `phi` | `sigma_pi` | Unconditional SD | Basis |
|---|---|---|---|---|
| **`quantcalc` (default)** | 0.4495 | 0.80%/yr | **0.90%** | [QuantCalc methodology](https://quantcalc.app/methodology.html), an AR(1) fit on CPI from 1947 (its fitted mean is 2.53%; the mean is a separate assumption here). The citable default; PLAN M6 and TESTING `mc_inflation_ar1` carry the same numbers. Labelled **third-party fit, not reproduced by this project** until the refit gate below clears |
| `high-persistence-stress` | 0.65 | 1.75%/yr | **2.30%** | Midpoint of an unfitted heuristic range (phi 0.6-0.7, sigma 1.5-2%), whose unconditional SD spans 1.9-2.8%. Retained as a **labelled stress**, never a default; labelled **uncalibrated** wherever it affects a number |

The width of the two matters and is shown on the card: at the default's 0.90% unconditional SD the 95% band is roughly 0.9-4.5% around a 2.7% mean, so nominal pensions, fixed-rate mortgages, the never-indexed Social Security and NIIT thresholds and `fers_diet` COLAs (15) carry modest inflation risk in Monte Carlo, and the stress preset is the one-click way to see what a persistent inflation regime does to them. Neither preset is a fit this project has performed, so neither may be presented as one.

**Blocking gate for the M6 vintage lock (section 20; PLAN hand-verification list).** Before any vintage carrying inflation parameters is locked, an AR(1) is refit on CPI from 1913 by a **committed script** under `xtask/`, reading a checked-in CPI series with its source and SHA-256, and emitting `phi`, `sigma_pi` and the implied unconditional SD. The fitted unconditional SD is pinned as a golden so a later re-run that moves it fails CI, and the shipped default's label changes from "third-party fit" to "fitted" only if the refit reproduces it within the pinned tolerance. This is a release gate, not a recommendation: a third-party fit may ship as a labelled default, but it may not ship inside a locked vintage presented as this project's own.

The long-run mean `pi_bar` is a **separate, editable assumption** taken from the active CMA vintage so nominal returns and inflation stay coherent (2.7% in the synthetic example of 3.1); 2.40% (Trustees, [TR2026 V.B](https://www.ssa.gov/oact/TR/2026/V_B_econ.html)) and 2.5% are equally defensible one-click alternatives.

**Correlation with bonds.** The research prescribes a negative correlation between the inflation shock and the bond shock but supplies no magnitude. `inflation` is an optional row of the correlation matrix; v1 ships it **uncalibrated (0)** with that label, and a calibrated value enters only through a new vintage with a documented fit. Bootstrap and replay generators ignore the AR(1) and take CPI from the same historical row.

**Uses of the path.** (1) Real-dollar streams are inflated by the cumulative index. (2) `ParamView` uprates every indexed parameter per `(year, path)`, once per path, before the year loop. Each year is computed **directly from the parameter's statutory base-year amount times the cumulative index ratio to that year**, with the table's rounding rule applied **once** - to the increase over the base amount where the rule's `basis` says so (26 USC 1(f)(7)). The uprating pass is therefore a fan-out from the base year, never a year-over-year chain: chaining applies the rounding step once per year and diverges from the statutory amount. The index series is the published one through the last published year (the archived chained-CPI series each table's `index_series` names, relative to that table's statutory base year) and the path's own cumulative index beyond it — less the `chained_cpi_wedge` of ENGINE-SPEC 1.3, default 0 — which is what makes the result path-dependent. Never-indexed thresholds stay flat, which is what produces bracket creep honestly. PLAN seam S1 and ARCHITECTURE's parameter block own the field shape this requires (base year, base values per filing status, index series id); this document only states how the simulation layer consumes it. (3) Wage-indexed Social Security parameters grow at `pi_t + w`, with `w = 3.57% - 2.40% = 1.17%` real (Trustees ultimate assumptions, same source). (4) All displayed outputs are deflated **per path**. Component inflation for healthcare and housing belongs to the life-module specifications (ENGINE-SPEC 9-10, **1.1**).

---

## 6. Longevity

**Planning age (M3, deterministic).** `PlanningAgeMode::Computed` (the default) applies the FPA rule to each person's resolved table — `Person.mortality_table_id` if set, else the assumption set's `mortality_table_id`, which is not optional, so a table always resolves (DOMAIN-MODEL 4, 15): life expectancy + 5 for one person, longest life expectancy + 8 for two, on an annuitant table with improvement ([FPA 2021](https://www.financialplanningassociation.org/article/journal/AUG21-how-estimate-end-retirement)). `PlanningAgeMode::Fixed` offers **95 and 100 as one-click presets, 100 the conservative one**; amortization rules use 100 (a "95 when no table is chosen" fallback has no case to fire on here, because the assumption set always names a table). Every longevity-sensitive output states which convention produced it.

**Stochastic mortality (M7).** `MortalityMode::Sampled{table, multiplier}`: per path and person, `u = u(seed, path, 0, slot=MORTALITY_P1|P2)`; death age is the inverse CDF of cumulative survival built from `q_x * multiplier` (multiplier range 0.6-1.2). Default table: SSA cohort life tables (Alternative II), because period tables understate longevity for younger cohorts ([SSA cohort tables](https://www.ssa.gov/oact/HistEst/CohLifeTables/2025/CohLifeTables2025.html)); an annuitant-table option (2012 IAM, [SOA](https://mort.soa.org/)) serves healthy households. Deaths occur at year end and trigger the same first-death routine as deterministic runs (ADR-008). Returns and deaths are independent; the mildly positive correlation of spouses' mortality is ignored and stated. Paths are scored only while someone is alive. Reported per year: P(both alive), P(only p1), P(only p2), plus the funded / depleted / deceased band chart ([Rich, Broke or Dead](https://engaging-data.com/will-money-last-retire-early/)). Joint survival identity used in tests: `1 - (1 - p_A)(1 - p_B)`.

---

## 7. Randomness, seeds and common random numbers (M6)

**Generator.** `ChaCha8Rng::from_seed(seed.0)` with `set_stream(path_idx as u64)` (ADR-013). Within a path stream every uniform is **addressed**, not consumed in sequence:

```
const SLOTS: u64 = 32;   // 0..=23 asset classes (registry index), 24 INFLATION, 25 MORTALITY_P1, 26 MORTALITY_P2,
                         // 27 BLOCK_START, 28 BLOCK_LEN, 29..=31 reserved
fn u(seed, path, t, slot) -> f64 {
    rng.set_word_pos(((t as u128) * SLOTS as u128 + slot as u128) * 2);   // two 32-bit words per uniform
    ((rng.next_u64() >> 11) as f64 + 0.5) * (1.0 / 9007199254740992.0)    // open interval (0,1), 53 bits
}
```

Consequences: the shock for `(seed, path, year, class)` is the same number whatever the horizon, the set of classes in use, the plan being simulated, the thread that computes it, or whether mortality sampling is on. Two scenarios that must use **different** assumption sets are compared by two `simulate` calls with the same seed; the underlying normals are still identical. Adding a slot never perturbs existing results.

**Normal deviates.** `z = AS241(u)`: the in-repo Wichura AS241 (PPND16) inverse normal CDF with committed golden vectors; `rand_distr` is not used. `exp`, `ln`, `sqrt`, `pow` come from the pure-Rust `libm` crate (D6).

**Seeds.** `Scenario.seed` is created once by `pfp-server` and persisted in the plan file; compared scenarios run under the baseline's seed. The seed, generator configuration and `nPaths` are part of every result pin (ADR-010). "Re-roll" is an explicit user action that marks saved results stale.

**Aggregation.** Workers write per-path metrics into a preallocated slot indexed by `path_idx`; every floating-point reduction then runs sequentially in path-index order (D7). Percentiles use a full sort with `f64::total_cmp`. Result: bit-identical across thread counts and architectures.

**Paired comparisons.** For plans A and B in one slice, `PairedDelta` reports the per-path difference distribution (terminal real wealth 5/50/95, lifetime taxes, success flips A-only / B-only / both / neither). The next-dollar distributional check (M6) is this structure applied to a recommendation and its alternative.

---

## 8. Trial counts and intervals

| Use | `n_paths` | Standard error at p = 0.85 | Source |
|---|---|---|---|
| Ephemeral slider runs (never saved; mark saved results stale) | 1,000 | +/-1.1 pp | ARCHITECTURE section 5 |
| Saved runs | 5,000 | +/-0.5 pp | same |
| Solvers, tails, guardrails | 10,000+ | +/-0.36 pp | same |

1,000 paths are acceptable for sliders but not for A/B decisions, where the differences between allocation choices are smaller than +/-1.1 pp.

**Wilson 95% interval** on every rate, `z = 1.959964`:

```
centre = (p + z^2/2n) / (1 + z^2/n);   half = z * sqrt( p(1-p)/n + z^2/4n^2 ) / (1 + z^2/n)
golden: p = 0.90, n = 10,000 -> [0.8940, 0.9057];  n = 1,000 -> [0.8798, 0.9171]
```

Antithetic pairing is not used (it breaks the one-address-one-shock contract for little gain on success rates).

---

## 9. Performance targets and vectorization strategy

**Gates (CI `criterion`, Apple Silicon, ARCHITECTURE 4.3):** 1,000 x 60 under 250 ms; 10,000 x 60 under 2 s.

**Budget arithmetic.** 10,000 x 60 is 600,000 ledger-years in 16 core-seconds on eight cores, about **26.7 microseconds of core time per ledger-year**. A tax-kernel probe of 600,000 full `federal()` evaluations under 8 s single-threaded would be bound at **13.3 microseconds per evaluation**, and that number does not close with the evaluation budget below of two to three calls per ledger-year: tax alone would consume 27-40 microseconds of a 26.7-microsecond ledger-year, 100% to 148% of the budget before any other work. The per-year budget, not the per-evaluation figure, is therefore the gate (DECISIONS C2).

The probe is sized accordingly, and at M1 rather than M6 because that is the whole point of probing early (PLAN risk R5):

- The evaluation budget is **one contract, owned by ENGINE-SPEC 2.4**: **a mean of at most two** full `federal()` evaluations per **accumulation** ledger-year (the federal-state two-pass of ENGINE-SPEC 4.3, with its third pass skipped by the standard-deduction shortcut in the common case) and **a mean of at most three** per **retired** ledger-year (the two-pass plus one gross-up settle, 13.4), under the **hard per-year evaluation cap of six** that ENGINE-SPEC 2.4 states. "Accumulation years need one" would be false, because the two-pass costs two in any itemizing year and `TierFill` routing recomputes tax once per filled pre-tax option (ENGINE-SPEC 2.3).
- The M1 probe is therefore **sized to about 1,500,000 `federal()` evaluations at the same wall-clock limits** (2 s on eight cores, 8 s single-threaded), which is 600,000 ledger-years at the 2.5 average the contract implies and about **5.3 microseconds of core time per evaluation**. At that cost a retired ledger-year spends about 16 of its 26.7 microseconds on tax at its three-evaluation ceiling, and a ledger-year at the 2.5 mean about 13.3 — half the budget (ARCHITECTURE 4.3, DECISIONS C2) — so the remainder closes. PLAN M1 and TESTING 10 carry the gate; this document does not restate it in a second place. An equivalent and arguably better formulation, if the probe is rewritten rather than re-scaled, is to benchmark **one simulated ledger-year at 26.7 microseconds of core time including all tax calls** - the quantity the 2-second gate is actually made of.
- DECISIONS C2 records the same contract from the tax side: 1,500,000 evaluations, or equivalently one ledger-year in 27 microseconds.

If the probe fails, ADR-007's ladder is triggered at M1 with the tax kernel still unwritten, which is the outcome the early probe exists to buy.

**Strategy: parallel across paths, scalar exact-integer kernel within a path.** The tax kernel is branchy integer code returning named lines; SIMD across paths would fight that design, so none is attempted.

1. **Compile once per plan.** `PlanProgram::compile(&ResolvedPlan)` resolves every `MonthRef`, orders streams and goals, precomputes nominal fixed-rate amortization schedules (path-invariant), contribution-policy and withdrawal-policy rule lists, and RMD divisor lookups.
2. **Once per path.** Generate the `ReturnPath` (structure-of-arrays, one contiguous `f64` block); build the cumulative price index; uprate parameter tables for the whole horizon in one pass - each year computed from its base-year amount and the cumulative index ratio (section 5), so the pass is a fan-out and not a chain; compute Social Security AIME/PIA once (about 100 integer operations per person).
3. **Year loop** at `TraceLevel::None`: no allocation (one scratch arena per worker, reset per path), `BTreeMap` only outside the loop, account sleeves in flat arrays indexed by compiled ids. The trace level is an argument of `federal()` itself, not only of `ProjectOpts` - at `None` the function returns totals and typed accessors with the named-line map empty, which is what keeps up to 1.5 million line maps from being allocated per run. That is rung 0 of the ADR-007 ladder and the reason seam S3 must freeze the parameter at M1; adding it later means reopening a frozen seam.
4. **Tax-call budget:** the ENGINE-SPEC 2.4 contract above - a mean of at most two full `federal()` evaluations per accumulation ledger-year and three per retired ledger-year, under its hard per-year cap. This document states the budget in no other place.
5. **Fan-out** with `rayon` behind the `parallel` feature over contiguous `path_idx` chunks; the sequential core stays available for the `wasm32` purity build (D11).
6. **Progress:** one NDJSON line per 250 completed paths `{done, total, elapsedMs}`; cancellation is checked at the same cadence.

If a gate fails, ADR-007's ladder applies in order; an `f64` tax lane is the last resort and is never used for deterministic results.

---

## 10. Outcome metrics and success criteria (M6, extended M7)

Per path the runner records: terminal real wealth, first insolvency year, unmet real spending (sum and years), largest realized real spending cut versus plan, maximum drawdown, lifetime taxes and the real spending path. `trait Criterion` is selectable and **printed on every output**:

| Criterion | A path succeeds when | Note |
|---|---|---|
| `never_below_zero` (default) | no year leaves essential plus committed spending unmet | matches Boldin since Dec 2025 (the project's internal research review (unpublished)) |
| `goals_funded` | every goal at or above the chosen importance is funded | yields `PoS_needs` and `PoS_all` |
| `ends_with_one_dollar` | terminal investable wealth is positive | the insurance-grid criterion |

Aggregates: funding success, solvency `P(end > 0)`, goal completion `mean(funded / scheduled)`, shortfall magnitude among failures (mean and 10th-percentile PV of unmet real spending; years short), first-shortfall-age histogram, Confidence Age (greatest age `a` with success truncated at `a` at or above the threshold), survival-weighted failure, terminal-wealth percentiles. Display bands for a success rate, described as commonly cited benchmarks and never as pass/fail: 90% and above strong; 75-90% acceptable if willing to adjust; below 75% at risk; **100% is flagged as probable under-spending**; the label reads "probability of not needing to adjust" (the project's internal research review (unpublished)).

**Paired probability type (D12, ADR-014).**

```rust
pub struct PairedProbability {            // the ONLY serializable carrier of a success rate
    p: f64, n: u32, wilson95: (f64, f64), criterion: CriterionId,
    max_cut: f64,                         // max over retirement years of (spending - guaranteed income)/spending
    worst_decile_cut: f64,                // 90th percentile across paths of the largest realized real cut
    shortfall: ShortfallTiming,           // p10 / median first-shortfall age + histogram; None-safe when no path fails
}
```

Fields are private; the sole constructor takes all of them; a bare `f64` success rate has no DTO. The API-level test walks the OpenAPI snapshot and fails if any schema exposes a probability-named number outside this type. Worked magnitude example: if guaranteed income covers $5,500 of $6,000 monthly spending, depletion is an 8.3% cut ([Kitces](https://www.kitces.com/blog/monte-carlo-retirement-projection-probability-success-adjustment-minimum-odds/)).

---

## 11. The "enough" scorecard (M7) and FI milestones (M2/M3)

One screen, four panels plus labels. No panel renders alone, and nothing is coloured pass/fail.

### 11.1 Funded ratio with sensitivity strip

```
FR(d) = [ investable assets + PV_d(future planned savings) + PV_d(Social Security, pensions, annuities) ]
        / PV_d( planned spending including taxes )                    # real dollars, to the planning age
```

Two rows: **essential** (denominator from a deterministic projection with discretionary streams zeroed, so taxes reflect the smaller withdrawals) and **total** (baseline projection). Three columns: `d` = TIPS real yield, TIPS + 1%, portfolio return. The third column uses the **same basis as the deterministic projection that produced the numerator and denominator** - the 4.1 `MedianGrowth` rate, deflated - so the funded ratio and the projection cannot disagree about the same portfolio; the column header names the basis, and switching on the arithmetic overlay adds a fourth column rather than silently changing the third. The TIPS yield is a dated `market_data` value (user-overridable; fallback 1% real, the fallback Open Social Security uses). **Home equity is excluded** unless a monetization event is scheduled; human capital is never added. The discount rate dominates the answer (a 4% to 3% change cuts a sustainable budget by more than 15%, [ERN Part 33](https://earlyretirementnow.com/2019/12/18/safe-withdrawal-rate-without-simulations-swr-series-part-33/)), which is why the output is a strip; the often-quoted 1.0-1.2 comfort band is **(unverified)** and is not encoded.

### 11.2 Fail-safe rate by horizon

Household rate = first retirement year's portfolio withdrawal / portfolio at retirement (deterministic run). Shown against (a) **published reference rates, each with engine, data window, horizon, allocation and success definition printed**, and (b) a **computed** fail-safe when a dataset is loaded.

| Horizon | Published reference | Source |
|---|---|---|
| 30 years, forward-looking Monte Carlo, 90% success | 3.9% (3.4% with a 1% fee) | [Morningstar 2025](https://www.morningstar.com/lp/the-state-of-retirement-income) |
| 30 years, historical by allocation, worst cohort 1973 | 4.2 / 4.5 / 4.7 / 4.9% | [FPA 2023](https://www.financialplanningassociation.org/learning/publications/journal/NOV23-revisiting-william-bengens-safemax-portfolio-withdrawal-rate-OPEN) |
| 35 years | 3.5% | Morningstar 2025 |
| 50-60 years, 60-80% equities | **3.25-3.50%** | [ERN series](https://earlyretirementnow.com/safe-withdrawal-rate-series/) |

Computed: for each cohort with a full horizon, bisect the largest constant-real initial rate that survives (start-of-year withdrawal, annual rebalance, stated fee and terminal target); fail-safe = minimum over cohorts; displayed with the `WindowCount` at the household's own rate. The default FI-number rate is 3.5% for long horizons (DECISIONS open decision 8), not 4%.

### 11.3 Success probability, always paired

The `PairedProbability` of section 10 with its Wilson interval, the criterion, generator, vintage and `n`. Beside it: **spending at 95 / 90 / 80 / 70 / 50% success with the unspent-legacy distribution** (terminal real wealth p10/p50/p90 at each level). Card text: under annual re-planning a 95% and a 50% target give similar median spending, but the 95% plan leaves roughly three times the legacy goal unspent ([Kitces](https://www.kitces.com/blog/monte-carlo-retirement-projection-probability-success-adjustment-minimum-odds/)).

**Sustainable-spending solver.** Bisection on a multiplier of the discretionary spending schedule (essential first if the multiplier reaches zero) so that `p = target`. Default target **85%**, adjustable 50-95% (DECISIONS open decision 8). `n = 10,000`, fixed seed, bracket `[0, 3]`, stop at 0.1% of spending. Under common random numbers success is monotone in spending, so the bisection is deterministic and needs no noise handling.

### 11.4 Dollar guardrails

Expressed as portfolio values in today's dollars, stored with their pins, re-solved at each annual review (annual performs about as well as monthly in published backtests) and logged as an adjustment history.

```
S*     = solve spending : PoS(S, W0) = target
W_low  = solve lambda   : PoS(S_now, lambda * W0) = lower_pos ;  W_low  = lambda * W0
W_high = solve lambda   : PoS(S_now, lambda * W0) = upper_pos ;  W_high = lambda * W0
at W <= W_low : S_new = S_now - cut_fraction   * (S_now - S*(W))
at W >= W_high: S_new = S_now + raise_fraction * (S*(W) - S_now)
changes below dead_band are ignored
```

`lambda` scales every investable balance inside an internal transform (not a scenario). `upper_pos = 1.00` means "all simulated paths succeed".

| Preset | target / lower / upper PoS | Adjustment | Dead band | Source |
|---|---|---|---|---|
| **Product default** (`guardrails-80-25-100`) | 80% / 25% / 100% | cut 10% of the gap; raise 100% of the gap | 5% | Methodology per [Income Lab guide](https://incomelaboratory.com/retirement-income-guardrails-complete-guide/) |
| Tharp worked case | 95% / 80% / 99% | move so PoS travels 10% of the way back to target | - | [Kitces Mar 2021](https://www.kitces.com/blog/probability-of-success-driven-guardrails-advantages-monte-carlo-simulations-analysis-communication/) |
| Morningstar variant | cut 10% at PoS <= 75%; raise 10% at PoS >= 95%; cap 120% of initial real spending | - | - | [Morningstar 2025](https://www.morningstar.com/lp/the-state-of-retirement-income) |

The "$1M, $52,000, cut at $740,000" illustration in the literature does not disclose its parameters and is refused as a regression target (TESTING §5.3). Cost: three bisections of about eight iterations at `n = 5,000`, streamed with progress. A **preview** mode uses the closed form of 12.8 and says so on the card.

The guardrails on this panel are solved on the **full Monte Carlo runner against the actual plan**. The in-path guardrail rule of 12.7 must derive its `PoS` from the same construction; otherwise `worst_decile_cut` in `PairedProbability` (section 10) reports the realized cuts of a rule that differs from the one displayed here, and the two panels quietly describe different plans. 12.7 states the mechanism and the tolerance.

### 11.5 Labels and FI milestones (M2/M3)

Heuristics appear as labels only, never as verdicts: 25x (4%), 28.6-30.8x (3.25-3.5%), salary multiples. KPIs:

```
FI number   = annual spending / WR                       (WR default 3.5%, editable)
Coast FI    = annual spending / ( WR * (1 + r_real)^t )  (golden: $1M target, 20 years, 5% real -> $376,889)
Barista FI  = (spending - part-time income) / WR
Years to FI = ln(1 + r * (1/WR) * (1-s)/s) / ln(1+r)     (at 5% real, 4% WR: s=10% ~51 y, 25% ~32 y, 50% ~17 y)
```

Formulas per [Walletburst](https://walletburst.com/tools/coast-fire-calc/). Milestone dates are drawn on the net-worth chart. The FI number ignores taxes, pre-Medicare coverage and lumpy goals; the card says it is a KPI, not the plan.

---

## 12. Decumulation spending rules

`trait SpendingRule { fn discretionary_target(&self, s: &SpendState) -> f64 /* real dollars */ ; fn describe(&self) -> RuleConfig; }`. `SpendState` exposes prior real spending, portfolio value, initial rate, last portfolio return, age, years remaining, guaranteed income, valuation. Rules set **total** desired spending; the ledger applies cuts to discretionary streams first and never below the essential floor. All rules run after tax on the same paths and are compared on survival, median, minimum and volatility of real spending, not on success alone. Constant real ships in M3; the rest in M7.

| # | Rule | Mechanics and defaults | Source |
|---|---|---|---|
| 12.1 | Constant real | `W_t = W_{t-1} (1 + CPI_t)`; start-of-year withdrawal | [Pfau/Trinity](https://retirementresearcher.com/safe-withdrawal-rates-for-retirement-and-the-trinity-study/) |
| 12.2 | Dynamic spending (`vanguard_dynamic`; the published Vanguard Dynamic Spending rule) | `W_t = clamp(rate * P_{t-1}, W_{t-1}^{real} * 0.975, W_{t-1}^{real} * 1.05)`; the pair was chosen for survival above 85% over 35 years. Tier 3 fixture: 40,000 -> 42,000 -> 41,543 | [AAII](https://www.aaii.com/journal/article/vanguards-dynamic-spending-strategy-for-retirees) (secondary) |
| 12.3 | Guyton-Klinger | (1) inflation raise capped at 6%, no make-up; (2) skip the raise after a negative-return year when `W/P > IWR`; (3) if `W/P > 1.2 IWR` and more than 15 years remain, `W *= 0.90`; (4) if `W/P < 0.8 IWR`, `W *= 1.10`. Assertions transliterated from cFIREsim-open (Apache-2.0). Card note: historical cuts of 28/54/45/36% vs 3/32/8/0% for risk-based guardrails | [G-K 2006](https://www.financialplanningassociation.org/sites/default/files/2021-11/2006%20-%20Guyton%20and%20Klinger%20-%20Decision%20Rules%20and%20SWR%20(1).PDF), [Income Lab](https://incomelaboratory.com/risk-based-vs-guyton-klinger-guardrails/) |
| 12.4 | Amortization (ABW) with VPW and RMD presets | `W_t = PMT(r_adj, n_t, -TW_t, FV = legacy, type = 1)`; `TW_t = P_t + PV(future Social Security, pensions, savings at a bond-like rate)`; `n_t = planning_age - age_t`; tilt `g`: `r_adj = (1+r)/(1+g) - 1`; portfolio draw = `W_t` - current guaranteed income. **VPW preset:** `r = s * 5.0% + (1-s) * 1.9%` real, `n = 100 - age` (check: age 65 at 50/50 gives 4.8%; values generated from the formula, never vendored from the CC BY-SA table; parameter provenance unconfirmed). **RMD preset:** 0% real with a selectable IRS table. Never "fails"; spending SD about 6%/yr historically, worst cumulative cut -37% | [WCI](https://www.whitecoatinvestor.com/amortization-based-withdrawal-vs-safe-withdrawal-rates/), [finiki](https://www.finiki.org/wiki/Variable_percentage_withdrawal) |
| 12.5 | Ratchet | if `P_t / I_t > 1.5 * P_0` (**real**, `I_t` the path's cumulative price index) and at least 3 years since the last ratchet, `W *= 1.10`; never cut. The comparison is real because in a nominal ledger under stochastic inflation a nominal trigger fires on high-inflation paths where real wealth is flat, raising real spending exactly where it should not. The source does not state its convention, so `basis: real \| nominal` is a rule field, the default is real, and **the rule card prints which was used**; the convention is confirmed against the source at the M7 gate (section 20) | [Kitces](https://www.kitces.com/blog/the-ratcheting-safe-withdrawal-rate-a-more-dominant-version-of-the-4-rule/) |
| 12.6 | CAPE rule | `WR_t = 1.75% + 0.5 / CAPE_t` (3.60% at CAPE 27; about 2.98% at the 2026-09-16 CAPE of 40.52). Uses `ReturnPath.valuation` when the dataset carries CAPE. On parametric paths a **proxy** built from cumulative return *surprise* is used and flagged `proxy` on the sheet: `CAPE_t = CAPE_0 * exp( sum_{k<=t} [ ln(1 + R_eq_k) - ln(1 + E_k) ] )`, where `R_eq_k` is the realized equity-composite return and `E_k` is defined once, as **the expected value of `ln(1 + R_eq_k)` under the generator that produced this path**. That single definition gives the right thing in both cases and is the reason the proxy has no built-in drift: on a `Mean{basis}` path the generator is deterministic, so `E_k = ln(1 + R_eq_k)` and the surprise is identically zero; on a stochastic path `E[ln(1+R)] = ln(1 + g_eq)`, the equity composite's median growth rate from the 3.3 identity (4.1's machinery applied to the composite's own `m` and `s`), so the surprise is zero **in expectation** each period. Defining `E_k` as the arithmetic mean instead would make the proxy decay by the volatility drag on stochastic paths. Valuation then moves only when returns differ from what was assumed, which is the whole content of a valuation signal on a parametric path. A total-return index form such as `CAPE_0 * (real equity index_t)/(1.018)^t` is rejected: `returns[t][class]` are **total** returns, so the index grows at price growth plus dividend yield while the divisor grows at earnings growth only, and the proxy drifts up 1.5-2%/yr (1.6-1.8x over 30 years), which would decay the CAPE withdrawal rate systematically and pin the 16.2 valuation glide path at its 30% floor for life. A price-index form (`R_t` less a dividend-yield parameter) is the alternative, but it needs a dividend yield in the vintage, which no entered set is required to carry and which is not invented here. Property test: **on `MeanPath` the proxy CAPE is flat within 1% over the horizon**, on either basis | [ERN Part 54](https://earlyretirementnow.com/2022/10/12/dynamic-withdrawal-rates-based-on-the-shiller-cape-swr-series-part-54/), [multpl](https://www.multpl.com/shiller-pe) |
| 12.7 | Risk-based guardrails | The 11.4 presets applied inside paths. Nested simulation is still avoided, but not by feeding the closed form the raw withdrawal rate - see below | [Income Lab](https://incomelaboratory.com/retirement-income-guardrails-complete-guide/) |

**12.7 in full: what the in-path `PoS` is computed from.** The 12.8 closed form assumes a constant real withdrawal rate drawn from the portfolio for life, with no guaranteed income, no taxes and no bridge years. Handing it the raw `S/W` of the year violates every one of those assumptions where the rule matters most: a household retiring at 60 with Social Security starting at 70 draws two to three times its long-run rate during the bridge, so a naive `PoS(S/W)` reads catastrophic, fires spurious cuts, and then over-raises once the guaranteed income switches on. Two admissible constructions, both stated because the choice is a performance trade and both are exact about what is fed in:

```
(a) level-equivalent rate (default)
    S_level = PV_r( remaining planned net withdrawals after guaranteed income and estimated tax ) / a(n_t, r)
    PoS     = closed_form_12.8( s = S_level / W_t, age )       # a(n, r) = the ordinary annuity factor, r as below
(b) plan-specific pre-tabulation
    a coarse pre-pass Monte Carlo of the ACTUAL plan (n = 1,000, scenario seed) tabulates PoS(age, W/W_plan)
    on a grid; the in-path rule interpolates. Costs one extra pass; needs no closed-form assumption at all
```

Construction (b) is what makes the in-path rule and the displayed guardrails of 11.4 the same rule, so it is the one used whenever the guardrail panel is also being rendered, and a Tier-3 test asserts that the two agree on the trigger wealths within one grid step. (a) is the cheap path for slider runs and is labelled `level-equivalent` on the rule card. Feeding raw `S/W` to the closed form is not offered.

**12.8 Milevsky-Robinson closed form** (ported from the MIT-licensed R4GoodPersonalFinances mathematics, `NOTICE` entry): with the **continuously-compounded real drift** `r`, volatility `v`, hazard `l = ln 2 / median remaining life` and spending rate `s`,

```
alpha = (2r + 4l)/(v^2 + l) - 1;   beta = (v^2 + l)/2;   P(ruin) = GammaCDF(shape = alpha, x = s/beta)
golden (+/-0.02): r 0.05, v 0.15, median 20 y -> 7.14 / 13.87 / 22.07 / 31.01% at s = 3 / 4 / 5 / 6%
```

`r` is the **continuously-compounded** drift: substituting an arithmetic 5% for `r` is a specification error that biases the closed form against this project's own Monte Carlo, and TESTING `mc_ruin_closed_form` records the convention in the fixture. Conversion from the active vintage, at the portfolio real arithmetic mean `m_p` and SD `s_p` of 4.1, uses the same lognormal identity as 3.3 and 4.1:

```
v = sqrt( ln( 1 + s_p^2 / (1 + m_p)^2 ) )        # the 3.3 log variance
r = ln( 1 + m_p ) - v^2 / 2                       # = ln(1 + g_p), the continuously-compounded (median) drift
```

The parameterization is confirmed against the FAJ 2005 paper directly at the M7 gate rather than against a secondary transcription (section 20). The regularized incomplete gamma function is implemented in-repo (series below `x < alpha + 1`, continued fraction above; `libm::lgamma`). Source: [FAJ 2005](https://rpc.cfainstitute.org/research/financial-analysts-journal/2005/a-sustainable-spending-rate-without-simulation); values independently recomputed for this document.

**Overlay: spending smile.** Toggle beside constant real: `dAS = 0.00008 Age^2 - 0.0125 Age - 0.0066 ln(ExpTar) + 0.546` (about -1%/yr real; [Blanchett 2014](https://www.financialplanningassociation.org/sites/default/files/2020-09/MAY14%20JFP%20Blanchett_0.pdf)). Not stacked with a late-life medical-inflation line. The fixture is generated from the equation with its iteration convention documented; the folklore target "$74,146 at 84" is refused, as is "Bengen 4.15%" (PLAN M7).

**Not rules.** Buckets are a *view* ("years covered by cash + bonds") over a total-return portfolio, because static 50-70% stock allocations beat bucket rules across 21 countries ([Estrada 2019](https://blog.iese.edu/jestrada/files/2019/07/BucketApproach.pdf)). Income floors are `IncomeStream`s (section 15).

---

## 13. Withdrawal sequencing

`WithdrawalPolicy` is data (seam S7): an ordered rule list evaluated by `pfp-ledger` at step 6 when the year is in deficit.

**13.1 Conventional (M3).** The default `WithdrawalPolicy.order` of DOMAIN-MODEL 11, `[Cash, Taxable, Traditional, Roth, Hsa]` (`WithdrawalTier`), with `rmd_first: true`: (1) RMDs first, on the prior 12/31 balance and the person's divisor; an RMD beyond need is reinvested in taxable. (2) Cash above the spending-shock buffer, then taxable accounts: lots by the least-tax-first key of 16.5 (average basis until M9 lot import). (3) Traditional accounts of owners past 59.5. (4) Roth: contribution basis, then seasoned conversions, then earnings. (5) HSA last.

**59.5 guard - ENGINE-SPEC 2.4 is the single owner of this rule and this section restates nothing.** Before the penalty-free age the order is taxable, then Roth contribution basis, and there it **stops**: if the deficit is still uncovered the year is marked `SHORTFALL`. The engine never silently books a penalized withdrawal (PLAN principle 8: the plan does not quietly do something the household did not choose). Continuing the order into penalized sources would contradict ENGINE-SPEC 2.4 and would move success probability, the guardrails of 11.4 and the funded ratio for every pre-59.5 retiree, because a `SHORTFALL` year fails `never_below_zero`. Beside every such year the engine computes and displays the **penalized alternative** - the gross withdrawal that would have covered the deficit, with the early-distribution additional tax as its own named `Line` (`f5329.*`, ENGINE-SPEC 3.2) - so the choice is visible without being taken.

Penalized access is available only as an explicit, stored opt-in on `WithdrawalPolicy.early_access: Vec<EarlyAccessRule>` (DOMAIN-MODEL 11; empty by default, which is the SHORTFALL behaviour above), pinned with the result like any other policy choice: `EarlyAccessRule::Penalized{account_ids}` consents to booking the penalized alternative from the named accounts once penalty-free sources are exhausted, and is honoured from M3. Two further entries are reserved for the statutory penalty exceptions that a pre-59.5 retiree actually uses, `SeparatedAt55{employer_plan_id}` and `Sepp72t{account_id, started, method}`; both ship with user-entered parameters and are honoured from M8, once their statutory mechanics have been sourced and fixtured (section 20), because no archived source supplies the rule text for either exception.

**13.2 Proportional (M8).** Each year's need is drawn pro rata to balances across taxable / traditional / Roth after RMDs.

**13.3 Bracket-managed (M8).** Draw traditional up to the fill-to-threshold primitive's target (ordinary bracket top, LTCG boundary, IRMAA tier or FPL multiple; shared with the conversion planner), then taxable, then Roth. Magnitude printed on the comparison: bracket management extended portfolio life about 11% at $800k-$1M but about 2% at $3M ([GHH 2021](https://www.financialplanningassociation.org/article/journal/MAR21-comparison-tax-efficiency-decumulation-strategies)); tax ordering is second-order to the withdrawal rate. Vanguard-style ordering is expressible in the rule list but is not a shipped preset. An LP/MILP optimizer is a non-goal for v1.

**13.4 Gross-up solve.** A deficit `D` is an after-tax need; a traditional withdrawal `x` raises tax. Solve `x - [Tax(base + x) - Tax(base)] = D`. **ENGINE-SPEC 2.4 owns the algorithm and the per-year evaluation cap**; the shape, stated here once because the simulation budget depends on it:

```
x0 = the ENGINE-SPEC 2.4 closed-form segment solve, used as the INITIAL GUESS ONLY
     (walk the kinks returned by federal() at the pre-withdrawal base and solve segment by segment)
then secant on the REAL f(x) = x - dTax(x) - D, evaluating federal(), until |f| <= $1
     (deterministic runs: to the cent).  The tax booked is always the final real federal() evaluation.
```

The closed form cannot be the answer on its own: its kink list covers bracket edges, the Social Security phase-in, LTCG boundaries and the NIIT threshold, but not the senior deduction phase-out, IRMAA tiers, the premium-tax-credit cliff or any state rule, so a pre-65 ACA household or a senior household lands off the closed-form surface exactly where the error is largest. Using it as the initial guess keeps its speed and gives up none of its exactness. Typical convergence from that guess is one to two further evaluations. There is **one** per-year evaluation cap and **one** evaluation budget for this solve, and both live in ENGINE-SPEC 2.4; this document legislates neither. Policies are scored on lifetime taxes, after-tax terminal wealth `taxable (stepped-up) + Roth + traditional * (1 - heirs rate)` (default heirs rate 24%, editable) and solvency years.

---

## 14. Social Security (`pfp-ss`)

Ported with attribution from ssa.tools (earnings record to PIA) and Open Social Security (claiming), both MIT. Monthly arithmetic, integer cents, exact rationals (D3, D4). WEP and GPO do not exist (repealed; last applied December 2023) and are not ported. **M3** accepts an entered `SsEstimateEntry` per person — the PIA itself, or a statement's benefit-at-age that `pfp-ss` back-solves to a PIA, with the derivation shown — and is where `pfp-ss` is created, carrying the claim-age factors and `ss.fra.retirement` of 14.3 that the back-solve inverts and the ledger applies (PLAN M3, ARCHITECTURE section 3; the `ss_claim_factor_knife_edge` gate is green from then); **M5** computes the PIA from the earnings record; the plan stores the earnings record or a PIA, never a claimed benefit (DOMAIN-MODEL 4, R7).

### 14.1 Inputs

The stored shape is `Person.social_security: SocialSecurityInput` as DOMAIN-MODEL 4 declares it; this section reads it and adds nothing:

```
SocialSecurityInput { source: EnteredEstimate | EarningsRecord | NotClaiming,
                      claim_age: MonthRef, survivor_claim_age: Option<MonthRef>,
                      earnings_record: Option<EarningsRecord { rows: Record<Year, Cents>, as_of }>,   // M5
                      entered_estimate: Option<SsEstimateEntry { Pia { amount }
                                                               | BenefitAtAge { amount, at: MonthRef } }>,  // M3
                      future_earnings: Option<GrowthRule> }                                              // "if work stops at age X"
born_on_first: bool   -- derived from Person.dob at run time, never stored
```

`BenefitAtAge` is resolved to a PIA by inverting the claim-age factor of 14.3 at the age the statement assumes, so a figure that has already been reduced is not reduced twice. Earnings records live only in the plan file. Tests use synthetic records and AnyPIA-generated fixtures.

### 14.2 AIME and PIA

```
indexFactor[y] = AWI[year_turning_60] / AWI[y]  for y < year_turning_60, else 1
indexed[y]     = min(earnings[y], wageBase[y]) * indexFactor[y]          # cap BEFORE indexing
AIME           = floor_dollar( sum(top 35 indexed, zeros included) / 420 )
BP1, BP2       = round(180 * AWI[E-2] / 9779.44), round(1085 * AWI[E-2] / 9779.44)   # E = year turning 62; frozen there
PIA_62         = floor_dime( 0.90*min(AIME,BP1) + 0.32*clamp(AIME-BP1, 0, BP2-BP1) + 0.15*max(AIME-BP2, 0) )
PIA_y          = floor_dime( PIA_{y-1} * (1 + COLA_y) )   for every year from 62 on, whether or not claimed
payable        = floor_dollar( PIA * claim_factor )
```

2026 constants, all from one notice ([FR 2025-19763](https://www.govinfo.gov/content/pkg/FR-2025-11-03/pdf/2025-19763.pdf)): COLA 2.8%, AWI (2024) $69,846.57, wage base $184,500, bend points $1,286 / $7,749. **COLA key convention:** tables are keyed by the December-effective year (2025 -> 2.8%); a test asserts it, and any imported table using the "paid in" convention is converted. **Forward projection** (neither reference codebase does this): AWI grows at path inflation plus 1.17% real and COLA equals path inflation, from the Trustees ultimate assumptions of CPI 2.40% and covered-wage growth 3.57% ([TR2026 V.B](https://www.ssa.gov/oact/TR/2026/V_B_econ.html)). First-class output: **"PIA if work stops at age X"**, a sweep over `stopWork` re-running top-35 selection, shown in today's dollars.

### 14.3 Claim-age factors (one exact rational, applied once)

```
retirement FRA (ss.fra.retirement): born 1943-54 -> 66; 1955-59 -> 66 + 2 months per year; 1960+ -> 67.
Born on the 1st -> prior month's rules.
early   (m months before FRA): 1 - min(m,36) * 5/900  - max(m-36,0) * 5/1200
delayed (k months after FRA, to 70): 1 + k * 2/300          # credits earned in a year pay from the next January, except at 70
spousal top-up = max(0, PIA_worker/2 - PIA_own) * [1 - min(m,36) * 25/3600 - max(m-36,0) * 5/1200];  no delayed credits
deemed filing applies to retirement and spousal benefits, never to survivor benefits
```

**Survivor FRA is its own schedule (`ss.fra.survivor`), not the retirement table.** Survivor full retirement age for birth year `Y` equals the **retirement** FRA of birth year `Y - 2`:

| Birth year of the survivor | `ss.fra.survivor` | Months from 60 |
|---|---|---|
| 1945-1956 | 66 | 72 |
| 1957 | 66 y 2 m | **74** |
| 1958 | 66 y 4 m | 76 |
| 1959 | 66 y 6 m | 78 |
| 1960 | 66 y 8 m | 80 |
| 1961 | 66 y 10 m | 82 |
| 1962 and later | 67 | 84 |

The 74-month entry for a 1957 birth year (the 28.5% maximum reduction spread over the months between 60 and survivor FRA) is the anchor, and the `Y - 2` rule is the generalisation that reproduces it; the table is transcribed from [POMS RS 00615.301](https://secure.ssa.gov/poms.nsf/lnx/0300615301) at the hand-verification gate (section 20). The two tables disagree for birth years 1955-1961, so implementing `survivorFRA` from `ss.fra.retirement` gives wrong survivor benefits for a seven-year band - in the income stream ADR-019 says the Roth verdict depends on most. A Tier-1 fixture must pin a birth year where they differ; no existing case does.

**Survivor benefit, three branches.** The base depends on what the decedent had done, and only then is the survivor's own age reduction applied ([POMS RS 00615.320](https://secure.ssa.gov/poms.nsf/lnx/0300615320), the ordering Open Social Security implements):

```
age_factor = 1 - 0.285 * monthsEarly / monthsBetween(60, ss.fra.survivor)     # 0.715 at 60 -> 1.0 at survivor FRA
                                                                              # 0.715 also for disabled widow(er)s 50-59
(a) decedent FILED BEFORE their FRA  -> RIB-LIM:
      survivor = min( PIA_dec * age_factor, max(deceased_reduced_benefit, 0.825 * PIA_dec) )
      82.5% is a LIMIT, not a floor: a 60-year-old survivor of an early claimer gets 71.5% of PIA, not 0.825 * 0.715
(b) decedent FILED AT OR AFTER their FRA -> base is the actual benefit INCLUDING delayed credits:
      base     = deceased_benefit_incl_DRC
      survivor = min( base * age_factor, base )
(c) decedent DIED UNFILED:
      base     = PIA_dec                                   if death before their FRA
               = PIA_dec * (1 + k * 2/300), k = months FRA..month of death, capped at 70   otherwise
      survivor = min( base * age_factor, base )
the survivor receives max(own, survivor), never both; own retirement and survivor are independent claims
```

Branch (b) is the branch that matters most for claiming advice. A single RIB-LIM formula applied to every decedent would give a decedent who claimed at 70 (1.24 x PIA) `min(1,000, max(1,240, 825)) = 1,000` for a survivor at survivor FRA - the PIA, **19% below** the 1,240 actually payable, for the rest of the survivor's life. That would remove the main reason for the higher earner to delay, biasing the M5 comparison and the M8 grid toward early claiming and understating survivor-year income and therefore `t_future` in the Roth verdict and `F*` in the insurance grid. The 715-at-60 gate exercises branch (a) alone and cannot catch it; the four gates below cover every branch and the survivor-FRA table.

Sources: [POMS RS 00615](https://secure.ssa.gov/poms.nsf/lnx/0300615000), [POMS RS 00615.301](https://secure.ssa.gov/poms.nsf/lnx/0300615301) (widow reduction and the survivor FRA schedule), [POMS RS 00615.320](https://secure.ssa.gov/poms.nsf/lnx/0300615320), [benefit.service.ts](https://raw.githubusercontent.com/MikePiper/open-social-security/master/src/app/benefit.service.ts). CI gates on PIA 1,000, all four survivor cases at survivor FRA unless stated:

| Gate | Case | Expected |
|---|---|---|
| `ss_claim_factor_knife_edge` | own, 60 months early | **700** |
| same | own, +12 / +48 months | **1,080** / **1,320** |
| `ss_survivor_riblim_early` | branch (a): decedent claimed at 62 (700), survivor claims at 60 | **715** - discriminates limit from floor |
| `ss_survivor_delayed_decedent` | branch (b): decedent claimed at 70 (1,240), survivor at survivor FRA | **1,240** - the branch the single formula got wrong |
| `ss_survivor_unfiled_decedent` | branch (c): decedent FRA 67, died at 68 unfiled, survivor at survivor FRA | **1,080** |
| `ss_survivor_fra_table` | one birth year in 1955-1961, asserted against `ss.fra.retirement` for the same year | the two tables differ |

Ladder check for FRA 67: 70 / 75 / 80 / 86.7 / 93.3 / 100 / 108 / 116 / 124%. Branch (c)'s mechanics come from the MIT reference implementation rather than from a primary document; the 1,080 is this document's own delayed-credit formula applied, but the branch itself is **(unverified)** until confirmed against the Open Social Security and ssa.tools oracles at M5 (section 20).

### 14.4 Earnings test

2026: withhold $1 per $2 above $24,480 before the FRA year; $1 per $3 above $65,160 in the FRA year counting only pre-FRA months; none from FRA (same notice). Withholding removes whole monthly benefits from January until satisfied, in the 20 CFR 404.434 order; the grace-year monthly test applies in the first year with a non-service month. Each fully withheld month is credited back at FRA by reducing the count of early months. The published worked example for that credit (60 - 40 -> 20) is **(unverified)** and must be confirmed against POMS before it becomes a fixture; the mechanism is taken from the MIT source. Only wages and net self-employment income count.

### 14.5 Trust-fund haircut toggle

A year-keyed schedule `cut[y]` applied to own, spousal and survivor streams: `benefit = scheduled * (1 - cut[y])`, COLAs continuing on the reduced amount. Deterministic; its effect is reported as a success-rate delta between two runs on the same seed, never sampled.

| Preset | Schedule | Source |
|---|---|---|
| None | 0 | - |
| **Trustees (default on)** | 22% from 2032 (OASI depletion Q4 2032, 78% payable), **held flat thereafter as a labelled simplification** | [SSA TRSUM](https://www.ssa.gov/oact/TRSUM/index.html) |
| CBO two-step | 7% in 2032; 28% for 2033-2036 (published average), held at 28% afterwards as a **labelled extrapolation** | [CBO testimony](https://www.budget.senate.gov/imo/media/doc/drmollydahltestimonysenatebudgetcommittee1.pdf) |

**The flat 22% is a simplification, and it is labelled as one on the card and the assumptions sheet.** The payable percentage in the Trustees projection does not stay at 78% after depletion - it declines as the ratio of income to scheduled benefits falls - so a flat cut overstates late-life benefits relative to the cited source, in the years a long-horizon plan is most sensitive to. `cut[y]` is already a year-keyed schedule and needs no structural change; the year-by-year payable-percentage series is transcribed from the Trustees Report at the hand-verification gate and ships as the `Trustees (year-keyed)` preset, at which point the flat preset is retained only as a comparison. This document does not invent the intervening values (section 20). The no-cut comparison is always visible. An across-the-board cut does not by itself favour early claiming.

### 14.6 Claiming comparison (M5) and grid (M8)

M5 runs claim at 62 / FRA / 70 per person through the ledger. M8 adds the monthly grid (about 97 x 97 strategies for two people): monthly benefits to age 115 in four survival states, aggregated each December, probability-weighted and discounted at a **real** rate (dated 20-year TIPS yield, fallback 1%) so COLA cancels; mortality from the selected table. Because the PV surface is flat near its optimum and ignores tax, IRMAA, sequence risk and the insurance value of the survivor benefit, the top five candidates **plus 62 / FRA / 70** are re-ranked through the full ledger on after-tax terminal wealth and the scorecard, with sensitivity over the discount rate (0-3% real) and the mortality table. PV fixtures come from Open Social Security with the mortality table checked into `fixtures/`.

### 14.7 Out of scope, surfaced as `NotModelled`

Family maximum and child benefits, divorced-spouse benefits, disability conversion, voluntary suspension. The engine flags the household features that would trigger them rather than ignoring them.

### 14.8 Taxation hand-off

`pfp-ss` emits gross annual benefits per person. It never computes taxable amounts. The ledger places the total in `TaxInputs.social_security`; `pfp-tax` runs the Pub 915 worksheet (thresholds $25,000 / $34,000 single, $32,000 / $44,000 joint, never indexed; [Pub 915](https://www.irs.gov/publications/p915)) and returns named lines, which is what makes the torpedo visible in the effective-marginal-rate curve. Medicare premiums and IRMAA are ledger expenses on the t-2 MAGI clock, not netted from the benefit.

---

## 15. Pensions and annuities

One `IncomeStream` type covers every guaranteed source (the project's internal research review (unpublished)); kinds differ only in how the monthly amount is produced. **The stored shape is `DOMAIN-MODEL.md` §7's, declared there once**; the block below is a verbatim reference copy with the same spellings, not a second declaration (ARCHITECTURE section 2's one-home CI check, and the schema-identifier check of DOMAIN-MODEL §5 which covers this block):

```rust
pub struct IncomeStream {
    pub id: Id, pub label: String, pub owner: Owner,
    pub kind: IncomeKind,                 // Wages | SelfEmployment | Pension | SocialSecurity | Annuity (SPIA, DIA, QLAC)
                                          // | Rental | Interest | Dividends | CapitalGains | Other
    pub amount: Sourced<Cents>, pub period: Period,   // Annual | Monthly
    pub basis: Basis, pub growth: GrowthRule,         // Basis: Real | Nominal (DOMAIN-MODEL 2.4); growth = the COLA rule:
                                                      // Flat | FixedRate | Inflation | InflationCapped{cap} | FersDiet | ...
    pub start: MonthRef, pub end: MonthRef,           // Never = for life (joint life via survivor_fraction); Calendar; Event
    pub survivor_fraction: Ratio,                     // applies from the year after the stream's `owner` dies; 0 = life-only
    pub tax_character: TaxCharacter,                  // OrdinaryEarned | Ordinary | SsSection86 | Capital
                                                      // | ExclusionRatio{excluded} | Exempt
    pub payroll: Option<PayrollDetail>,               // wages only
    pub as_of: DateYmd,
}
```

`FersDiet`: CPI at or below 2% -> full; 2-3% -> 2%; above 3% -> CPI - 1 (the project's internal research review (unpublished)). On stochastic paths a `Flat` stream erodes with path inflation; the card shows real purchasing power at 10 and 20 years. At the pension holder's death the stream scales by `survivor_fraction` (M4 state machine; ENGINE-SPEC 11.1 row 4). Whether a stream counts for the earnings test (14.4) or for a MAGI (ENGINE-SPEC 3.4) is derived from `kind` and `tax_character`, never stored; a joint-and-survivor pop-up election is not modelled in v1 (DOMAIN-MODEL 7). **v1 takes amounts as entered.** A present value for display: `PV = 12 B [ sum_t v^t tp_x + s * sum_t v^t (1 - tp_x) tp_y ]` at the TIPS real rate on the selected table.

**After 1.0 (PLAN section 5):** annuity pricing `payout per $1 = (1 - load)/a_x`, `a_x = sum v^t tp_x` on the 2012 IAM annuitant table with a 10-15% load, **calibrated to a pasted quote and never to hard-coded payouts** (published 2026 benchmark payout tables are estimates, not carrier quotes); sizing `SPIA target = essential expenses - other guaranteed income`; the defined-benefit lump-sum evaluator on 417(e) segment rates; QLAC handling (2026 premium cap $210,000, [Notice 2025-67](https://www.irs.gov/pub/irs-drop/n-25-67.pdf)).

---

## 16. Portfolio allocation and tax-aware reallocation (M9; static allocation M3)

### 16.1 Target allocation: three inputs, never an optimizer

```
tolerance_equity : entered directly as a maximum equity share, beside the dollar loss it implies:
                   loss = equity_share * portfolio * drawdown, shown for drawdown = 30% and 40%
capacity_equity  : largest equity share on a 5-point grid for which the plan still meets its essential-spending criterion
                   under an immediate 40% equity drawdown stress (same seed). Shown with years_of_spending_covered =
                   (safe assets + PV guaranteed income) / annual essential spending, and recovery years
need_equity      : smallest equity share on the grid whose median path funds the goals
recommended      = min(tolerance_equity, capacity_equity);  if need_equity > recommended -> "change the goal, not the portfolio"
```

All three are plotted; high capacity never overrides low tolerance ([Kitces](https://www.kitces.com/blog/tolerisk-aligning-risk-tolerance-and-risk-capacity-on-two-dimensions/)). A scored questionnaire and optimizer-generated targets are non-goals. The split within equities and within bonds keeps the household's current proportions; the app selects no funds.

### 16.2 Glide paths (`trait GlidePath`: `(years_to_retirement, age, valuation?) -> weights`)

| Path | Definition | Source |
|---|---|---|
| **Default** | 90% equity until 25 years before retirement, linear to 50% at retirement, then **held at 50%** as a neutral placeholder (DECISIONS open decision 8). The early 90% plateau is read from a fund chart, not the cited paper | [Vanguard 2025](https://workplace.vanguard.com/content/dam/inst/iig-transformation/insights/pdf/2025/231657-02_TDF_PTDF_OTH-TRIGT-Research.pdf) |
| Continue to landing | as default, then linear to 30% seven years after retirement | same |
| Rising tent | 30% at retirement rising linearly to 60% (or 80%) over 30 years | [Pfau & Kitces 2014](https://www.financialplanningassociation.org/article/journal/JAN14-reducing-retirement-risk-rising-equity-glide-path) |
| 60 to 100 | 60% at retirement to 100% over about 10 years | [ERN Part 19](https://earlyretirementnow.com/2017/09/13/the-ultimate-guide-to-safe-withdrawal-rates-part-19-equity-glidepaths/) |
| Valuation rule | PE10 below 11.1 -> 60%; above 21.2 -> 30%; else 45%. Reads the same `ReturnPath.valuation` series as 12.6, so on parametric paths it depends on the return-surprise proxy: under a drifting proxy the rule would pin itself at the 30% floor for the whole horizon from any starting CAPE above 21.2 and stop being a rule at all | [Kitces & Pfau 2015](https://www.financialplanningassociation.org/article/journal/MAR15-retirement-risk-rising-equity-glide-paths-and-valuation-based-asset) |
| Lifecycle (Merton) | `w* = clamp(EP / (gamma * sigma^2), 0, 1)` on total wealth with future savings and Social Security counted as bond-like | The project's internal research review (unpublished) |

Every path is bounded by `min(tolerance, capacity)` and compared in the engine on spending outcomes (success with its companions, 5th-percentile terminal wealth), never on Sharpe. Rising paths add only 0.1-0.3 points of safe withdrawal rate.

### 16.3 Rebalancing policy as data

```rust
// reference copy of DOMAIN-MODEL 11 (the definition site), same spellings
pub struct RebalancingPolicy { pub look_frequency: LookFrequency,        // Annual | Quarterly | Monthly
                               pub band: BandSpec,                       // { absolute_pp: 5, relative_pct: 25, combine: Smaller }
                               pub destination: BandDestination,         // Target | Halfway | Edge
                               pub scope: RebalanceScope,                // BreachingOnly | All
                               pub location_priority: Vec<AssetClass> }  // asset-location order (16.4)
```

Default 5/25 ([WCI](https://www.whitecoatinvestor.com/rebalancing-the-525-rule/): 30% -> 25-35; 10% -> 7.5-12.5; 5% -> 3.75-6.25), destination `halfway`, scope `breaching_only`. The width is editable (one study found 20% relative best, [Daryanani 2008](https://www.financialplanningassociation.org/sites/default/files/2020-05/9%20Opportunistic%20Rebalancing%20A%20New%20Paradigm%20for%20Wealth%20Managers.pdf)). Inside the annual ledger the look is annual at step 7; M3 rebalances to target, M9 applies the bands. Order of operations everywhere: (1) contributions, dividends, RMDs to the most underweight class and withdrawals from the most overweight; (2) trades inside tax-advantaged accounts (worth +44 bps/yr after tax on their own, [Vanguard 2019](https://www.vanguardsouthamerica.com/content/dam/intl/americas/documents/latam/en/sa-2123766-getting-back-on-track.pdf)); (3) taxable sales. Every systematic policy lands within about 0.2%/yr of the others, so the do-nothing plan is priced honestly.

### 16.4 Asset location

`drag_i = E[R_i] * tau_i`, where `tau_i` is the household's effective annual rate on that class's return stream, **computed with `marginal()`** on the class's yield and distribution mix rather than from a bracket label. Greedy fill holding household weights at target, with employer-plan menus as hard constraints: tax-deferred space takes the highest-drag classes; Roth takes the highest expected-return equity remaining; taxable takes low-drag broad equity. Both the nominal and the after-tax view (`traditional * (1 - expected withdrawal rate)`) are shown. Reported value about 20-52 bps/yr (the project's internal research review (unpublished)).

### 16.5 Tax-aware trade list

Pure function `(HouseholdHoldings, TargetWeights, RebalancingPolicy, TaxContext, asOf) -> [TradePlan; 4]`. Lots arrive decrypted from the plan file; fixtures are synthetic.

```
Lot = { accountId, fundId, assetClass, qty, basisPerShare, acquired }     TaxContext = { rho_st, rho_lt (from marginal(), incl. NIIT and state), gainsBudget }
1. delta[c] = destination_dollars[c] - current_dollars[c]                  # destination per policy
2. apply scheduled cash flows (contributions, dividends, RMDs, withdrawals) against the largest deltas
3. satisfy remaining deltas inside tax-advantaged accounts, respecting menus and the 16.4 priority
4. residual taxable reductions: sort lots by T = rho * (1 - basis/price) ascending (least-tax-first: losses, then lowest tax
   per dollar; equals HIFO under one rate); sell until delta is met or gainsBudget is exhausted;
   skip gain lots within 30 days of long-term status; never realize short-term gains for band-level drift
5. WASH-SALE HARD BLOCK: a loss sale is refused if the same or a user-declared substantially identical fund was acquired in
   ANY household account (both people; IRAs, Roths, reinvested dividends) within the prior 30 days; after any loss sale the
   plan emits a 30-day do-not-buy constraint for that identity group and offers the class's designated substitute
6. if the budget binds, stop at the band edge and report residual drift
7. score: annual benefit = position * (delta expected return + delta expense ratio) vs cost = tax_paid * r;
   breakeven n = ln(V/(V - tax)) / ln((1 + r - e_new)/(1 + r - e_old))
```

Sources: [Moehle et al.](https://arxiv.org/abs/2008.04985) (least-tax-first optimality), [Rev. Rul. 2008-5](https://www.irs.gov/pub/irs-drop/rr-08-05.pdf) and [Pub 550](https://www.irs.gov/publications/p550) (61-day window; a repurchase inside an IRA or Roth forfeits the loss **permanently**, which is why this is a block and not a warning). The cost of realizing a gain is usually the lost return on the deferred tax (roughly 0.3-0.5%/yr of position value), not the tax itself. Unindexed NIIT thresholds change `rho_lt` and therefore lot order, which is why `rho` comes from the tax function.

**Four plans, always side by side:** full rebalance; partial to the band edge under the gains budget; cash-flow-only over N months; do nothing. Each shows realized short- and long-term gains, tax, post-trade drift, expected-return delta and the change in the paired success probability. Output is an explained list for the user to execute elsewhere; the app executes nothing. Properties: dollars conserved across every plan; no emitted list violates the 61-day window; fixture: a 62% equity position against a 200 bps trigger moves to 61.75% under a 175 bps destination ([Vanguard 2024](https://corporate.vanguard.com/content/dam/corp/research/pdf/the_rebalancing_edge_optimizing_target_date_fund_rebalancing_through_threshold_based_strategies.pdf)).

---

## 17. Result presentation (M6, extended M7)

**Fan chart.** Percentiles are computed **per period** by sorting that period's values across paths, in today's dollars deflated per path. Rank rule, matching the published construction (of 1,000 trials the 5-95% band is trials 51-950, 25-75% is 251-750, the median is trial 500; [RightCapital](https://help.rightcapital.com/knowledge-base/client-portal/retirement/analysis/confidence-tab-retirement-analysis)):

```
k(p, N) = floor(p*N) + 1  if p < 0.5;   ceil(p*N)  otherwise        (1-based rank in ascending order)
```

Bands 5-95, 10-90, 25-75 and the median, rendered with Observable Plot `areaY`; the deterministic path of 4.1 is always overlaid, labelled "Current assumptions, no volatility (median growth)", with the arithmetic-mean path available as a second, separately labelled overlay; named stresses are optional lines; a worst-decile overlay shows the median of the bottom 10% of paths by terminal wealth. The chart title carries generator, vintage, `n` and criterion.

```
FanVm = { years[], ages: {p1[], p2[]}, bands: [{p, realCents[]}], deterministic: realCents[], stresses: [{name, realCents[]}],
          worstDecile?: realCents[], pins }
PercentileTableVm = { rows: [{age, p5, p10, p25, p50, p75, p90, p95}], series: netWorth | spending | taxes }   # every 5 years + milestones
ScorecardVm = { fundedRatio: {essential: [3], total: [3], rates: [3]}, failSafe: {householdRate, references[], computed?: WindowCount},
                probability: PairedProbability, spendingLevels: [{target, spend, legacy: {p10,p50,p90}}], guardrails?: {...}, labels[] }
```

Statistics are `f64` inside the engine and are rounded to integer cents at the view-model boundary. When two or more generators have been run for a scenario they are shown side by side; the spread between them is labelled model uncertainty. A tornado chart and a savings-rate x equity-share heatmap reuse the runner at `n = 1,000` under the scenario seed.

**Path drill-down.** Any path can be recomputed at `TraceLevel::Full` from `(seed, path_idx)` and opened in the ledger view; the default offered is the path nearest the 10th-percentile terminal wealth. The endpoint is admitted in M6 with the screen that consumes it.

**Assumptions sheet (every run, stored with the result and printed in reports):** generator and configuration; the deterministic-path basis (`MedianGrowth` or `ArithmeticMean`, 4.1); vintage id and as-of date; means, SDs and the correlation matrix actually used, including `pd_repaired` and the stock-bond knob; inflation preset id and parameters with their "uncalibrated" flags (5); dataset id and SHA-256, block length, the re-centring reference mean and whether inflation was re-centred (4.3); mortality mode and table; criterion; spending rule with its real-or-nominal conventions, withdrawal policy and the `early_access` rules in force (13.1); the trust-fund schedule and whether it is the flat simplification (14.5); seed, `n`, time step (annual); proxies in force; validation basis (`property-tested only` for simulation outputs); and the standing notice that third-party names on presets and vintages identify published methodologies and imply no affiliation, sponsorship or endorsement, all trademarks belonging to their owners (ADR-004).

---

## 18. Validation matrix

| Milestone | Test | Tier / kind |
|---|---|---|
| M1 | Both `TraceLevel` modes agree to the cent on every typed accessor of `FederalReturn` (the property that makes the `None` fast path safe, section 9). The re-based tax-kernel probe itself is carried by PLAN M1 / TESTING 10, not restated here | Property |
| M2 | FI number and years-to-FI goldens; `ReturnPath` optional fields serialize stably; **`MedianGrowth` algebraic golden** (one-class account at `m = 6.51%`, `s = 15.5%` grows at `g = 5.4%` to 1e-12; zero-volatility vintage makes the two bases identical) | Tier 1 synthetic |
| M3 | Coast FI $376,889 (a user `r_real`, unaffected by 4.1); zero-return zero-inflation ledgers; planning-age convention table; constant-real path; `ss_claim_factor_knife_edge` (1,000 -> 700) with `pfp-ss`'s claim-age factors, and a `BenefitAtAge` entry that round-trips to its PIA through the inverted factor (14.1); **a pre-59.5 deficit marks `SHORTFALL` and the penalized alternative is reported beside it** (13.1), asserted with `early_access` empty and again with an `EarlyAccessRule::Penalized` entry, whose booked `f5329` line equals the printed alternative | Tier 1 |
| M5 | 1,000 -> 700 (already green from M3) / 1,080 / 1,320; **all four survivor gates (the three branches of 14.3 plus the survivor-FRA table): 715 (early-claiming decedent), 1,240 (decedent claimed at 70), 1,080 (died at 68 unfiled)**, and `ss.fra.survivor` differs from `ss.fra.retirement` on a 1955-1961 birth year; AnyPIA fixtures to the dime; COLA key convention; mutation budget on `pfp-ss` | Tier 1 / Tier 2 |
| M6 | Moment match to 1e-9; AS241 golden vectors; KS test of sampled marginals; **same seed bit-identical on aarch64 and x86_64 and across 1/2/8 threads**; addressing invariance (changing horizon or class set leaves shared shocks unchanged); CRN A/B variance below independent streams; zero-volatility generator equals `MeanPath` run; **`MedianGrowth` statistical golden against the committed bootstrap interval of the iid median, with `ArithmeticMean` falling outside it** (4.1); **full-circular-sample mean of every class and of CPI equals the vintage to 1e-6** (4.3); **fitted inflation unconditional SD golden from the committed refit script** (5, blocking for the vintage lock); Wilson goldens; PD-repair on a deliberately indefinite matrix; Bengen longevity smoke test as integer window counts on a declared dataset (4% never exhausted before 33 years; 4.25% could be in 28, [Bengen](https://www.financialplanningassociation.org/sites/default/files/2021-04/MAR04%20Determining%20Withdrawal%20Rates%20Using%20Historical%20Data.pdf)); performance gates | Property / Tier 3 / bench |
| M7 | Vanguard path 40,000 -> 42,000 -> 41,543; Guyton-Klinger transliterated assertions; Milevsky-Robinson four values; bisection monotonicity under CRN; **API-level paired-probability test**; **proxy CAPE flat within 1% on `MeanPath`** (12.6); **ratchet fires on the real comparison and not the nominal one on a high-inflation path** (12.5); **in-path guardrail trigger wealths match the displayed 11.4 guardrails within one grid step** (12.7); folklore targets refused in fixture notes; joint-survival identity | Tier 2 / Tier 3 / contract |
| M8 | Gross-up solve converges within the ENGINE-SPEC 2.4 budget on all personas, including a pre-65 ACA household and a senior household (the cases the closed form alone misses); policy comparison shares pins; Open Social Security PV fixtures | Property / Tier 2 |
| M9 | Dollars conserved across every rebalance; no trade list violates the 61-day window; least-tax-first ordering over mixed gain/loss lots; 62% -> 61.75% | Property / Tier 1 synthetic |

Published benchmark studies (Morningstar 3.9%, FPA 2023) are **comparison notes in the validation report, not gates**: reproducing them needs their proprietary assumptions.

---

## 19. Deviations from, and tensions with, the spine

1. **"Regime model" and Student-t are specified here but land after 1.0.** Capability (f) in `PLAN.md` §1.1 names a regime model among the generators; PLAN M6 scope-out and PLAN section 5 defer the two-phase regime and the t-stress, and a fitted regime-switching model is a standing non-goal. This document follows the spine.
2. **Timing of named stresses.** ARCHITECTURE section 7 places named stresses in M3-M4; PLAN M6 places them in M6. This document follows PLAN (M6). `BadTiming` has no data dependency and can be pulled into M4 at no seam cost if wanted.
3. **Trait placement.** ARCHITECTURE lists spending rules under `pfp-sim`, but `project()` in `pfp-ledger` must call them and `pfp-sim` depends on `pfp-ledger`. Resolution (2.5): traits and M3 built-ins in `pfp-ledger`; later implementations in `pfp-sim`, injected through a static `RuleRegistry` carried by `ProjectOpts`/`SimOpts`. No dependency arrow is reversed.
4. **`ReturnPath` and `ReturnGenerator` carry two fields and one method beyond the `ARCHITECTURE.md` §7 sketch.** `ReturnPath` carries optional `death_year` and `valuation`, and `ReturnGenerator` carries `path_count`, from the freeze of seam S6 at M2. These are additions to the sketch, not changes of meaning.
5. **RNG addressing.** ADR-013 fixes ChaCha8 with stream = path index. Section 7 adds word-position addressing inside each stream. It refines the decision and strengthens its guarantees; it does not alter it.
6. **Float fence: two named `f64`-to-money entry points.** Spending-rule arithmetic is classified under D5(d) "solvers", and every rule returns money through `Cents::from_f64_half_even`; allocation weights are integer basis points. ARCHITECTURE D5(b) and ADR-007 name `Cents::from_f64_half_even` beside `Cents::grow` as the two audited entries, and the TESTING 6.1 exception list and merge gate 7 carry the same pair. The count of boundaries is two and stays auditable; a third is a design change.
7. **Deterministic path compounds at the portfolio median growth rate (4.1), not the arithmetic mean.** Nothing in the spine fixes the basis, but every pre-M6 number in PLAN and ENGINE-SPEC is produced on this path, so the change moves FI dates, `t_future`, the conversion baseline, the insurance grid and the funded ratio. The arithmetic path survives as a labelled overlay. Consequence for seam S1/M2: the first assumption vintage must carry SDs and correlations, not means alone.
8. **The M1 tax-kernel probe is sized** to about 1,500,000 `federal()` evaluations at the stated wall-clock limits (section 9), and `federal()` takes a `TraceLevel` argument when seam S3 freezes at M1. PLAN M1 and TESTING 10 carry both, recorded as DECISIONS C2.
9. **ENGINE-SPEC 2.4 is the single owner of the 59.5 rule, the gross-up algorithm and the tax-evaluation budget.** This document's sections 9, 13.1 and 13.4 reference it and legislate none of the three.
10. **Research items deliberately not adopted:** an in-app Python optimisation or backtest harness (non-goal: optimizer-generated allocations); bundled historical datasets (R16); live yield fetches (ADR-020); a probability-weighted disability grid.
11. **Section 15's `IncomeStream` is `DOMAIN-MODEL.md` §7's, by reference.** A second shape here would fail the schema-identifier check, so the COLA variants live in `GrowthRule` (`InflationCapped`, `FersDiet`), `TaxCharacter` is enumerated once in DOMAIN-MODEL §7, `survivor_fraction` is a `Ratio` (the four common joint-and-survivor elections are entry presets), the two `countsFor*` flags are derived rather than stored, and the pop-up election waits for pension pricing after 1.0.

## 20. Register of unverified or uncalibrated inputs used above

Third-party capital-market-assumption sets (not bundled until redistribution terms are confirmed, 3.2; a publisher-controlled URL for each is recorded at the gate); inflation-bond shock correlation (uncalibrated, ships as 0); earnings-test credit worked example; CBO schedule beyond 2036 (extrapolation); VPW parameter provenance; Vanguard dynamic-spending primary source (secondary only); the early 90% plateau of the default glide path; the funded-ratio comfort band (not encoded); RSLN-2 parameters (not used); the CAPE proxy on parametric paths (this document's construct, the return-surprise form of 12.6, flagged at run time). Also unverified:

| Item | Status | Clears when |
|---|---|---|
| AR(1) inflation parameters (5) | Neither preset is this project's own fit: `quantcalc` (the default) is a third party's fit on a window this project has not reproduced; `high-persistence-stress` is the midpoint of an unfitted range | **Blocking for the M6 vintage lock.** A committed `xtask` refit on CPI from 1913, against a checked-in series with source and SHA-256, pins `phi`, `sigma_pi` and the unconditional SD as a golden |
| Milevsky-Robinson parameterization (12.8) | The continuously-compounded-drift convention and the `alpha`/`beta` expressions were recomputed from a secondary transcription, not from the paper's text | Confirmed against FAJ 61(6) directly at the M7 gate; the fixture records the convention (TESTING 6.2) |
| Survivor branch (c), decedent died unfiled (14.3) | Mechanics taken from the MIT Open Social Security port, not from a primary document. The 1,080 gate is this document's own delayed-credit formula applied, not an imported number | Confirmed against the Open Social Security and ssa.tools oracles at M5 |
| `ss.fra.survivor` derivation (14.3) | The `Y - 2` rule is this document's generalisation of the 74-month anchor for a 1957 birth year | Transcribed from POMS RS 00615.301 at the hand-verification gate |
| Ratchet real-vs-nominal convention (12.5) | The source states no basis. Default is real; the rule card prints which was used | Confirmed against the source at the M7 gate |
| Trustees payable-percentage schedule after 2032 (14.5) | The shipped 22% flat cut is a **labelled simplification**; the declining year-by-year series is not invented here | Year-keyed series transcribed from the Trustees Report; ships as the `Trustees (year-keyed)` preset |
| `SeparatedAt55{employer_plan_id}` and `Sepp72t{…}` (13.1) | Reserved opt-in rule ids only; no archived source supplies the rule text | Statutory mechanics sourced and fixtured before M8 |

Each is either excluded from locked vintages until verified or labelled wherever it affects a number.
