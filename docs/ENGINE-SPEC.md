# ENGINE-SPEC

*How the deterministic engines compute: the annual ledger, the federal and state tax functions, the next-dollar allocation engine, Roth and conversion decisions, the debt engine, and the explanation contract every module returns. Date: 2026-09-17. Status: specification written against `PLAN.md`, `ARCHITECTURE.md` and `DECISIONS.md` (the spine). Where this file and the spine disagree, the spine wins and the disagreement is a defect in this file (section 14 lists the only known deviations). Every example is synthetic or a published third-party worked example. Nothing here describes any real household.*

**Terminology (standing definition, repeated from the spine).** "The binary" or "the release" always means a **local web application for macOS**: one self-contained universal executable (Apple Silicon + Intel) that starts a web server on loopback, serves the embedded web app over TLS and opens the default browser. The UI is a web app in the browser; it is not a native GUI and not an Electron/Tauri window. Everything specified in this document runs **inside that backend process**, in the pure engine crates. The browser never computes money, never applies patches and never sees the raw plan; it renders the view-models this engine produces.

**Number provenance.** Every statutory number below is cited by a source key; section 15 maps each key to a public primary document, or marks it as resting on the project's internal research review, which is not published ("the research", where this document says so, means that review). Numbers are shown here only to make fixtures and formulas checkable: **engine code contains no dollar literals** (lint, ADR-022); each value is read from a year-keyed parameter table through `ParamView`. Parameter ids in this document (`fed.brackets.ordinary`, ...) are indicative; the parameter catalogue is authoritative. Items no archived primary document confirms are marked **(unverified)** and must pass the M1 hand-verification gate before their vintage is locked.

---

## 0. Scope and milestone map

| Section | Content | Crate | Lands in |
|---|---|---|---|
| 1 | Conventions: types, time, uprating and rounding, year 0 / year-to-date, float fence | `pfp-money`, `pfp-params`, `pfp-explain` | M0 (money, `Line`, uprating), M1, M2 (year 0 / YTD) |
| 2 | Annual ledger, monthly sub-engines, surplus/deficit routing, growth, conservation | `pfp-ledger` | M2 (steps 1-6, one year), M3 (full loop) |
| 3 | Federal tax function, payroll, marginal/EMR, MAGI, RMDs, limits, rounding | `pfp-tax` | M0 (rate schedule), M1 (return), M2 (contribution side, AMT check), M3 (IRMAA), M8 (PTC) |
| 4 | State plug-in and effective-rate fallback | `pfp-tax` | M2 (trait, `NoIncomeTax`, `EffectiveRate`), M10 (`DeclarativeState`) |
| 5 | Next-dollar engine | `pfp-decide` | M2; distributional check M6 |
| 6 | Roth vs Traditional | `pfp-decide` | M2 (card v0: rate now + flip), M3 (verdict) |
| 7 | Roth-conversion planning | `pfp-decide` | M8 |
| 8 | Debt engine; debt vs invest at equal net worth | `pfp-ledger`, `pfp-decide` | M3 (amortization), M4 (planner, equal-net-worth test), M6 (on common random numbers) |
| 9 | Property | `pfp-ledger`, `pfp-tax` | **1.1** by decision (`PLAN.md` M10, §5); schema shape reserved M2 |
| 10 | Healthcare coverage state machine, HSA rules | `pfp-ledger` | **1.1** for the state machine (IRMAA M3, PTC M8, HSA limits M2) |
| 11 | First-death state machine; insurance sizing | `pfp-ledger`, `pfp-decide` | M3 (v1), M4 (full); **1.1** for `disable` and the grid |
| 12 | Explanation output contract for engine modules | `pfp-explain` | M2 onward |
| 13 | Validation hooks per section | fixtures | each milestone |

Social Security computation (`pfp-ss`: created M3 with the claim-age factors and the `BenefitAtAge` -> PIA back-solve, `ARCHITECTURE.md` §3; AIME/PIA, spousal, survivor and the earnings test M5), return generators and the scorecard (`pfp-sim`, M6-M7) and reallocation (M9) are specified elsewhere; this document only states how the ledger consumes them.

---

## 1. Conventions

### 1.1 Types

The shared primitives are **defined once**, in `DOMAIN-MODEL.md` §2.1-§2.2 and §4 (`pfp-domain` / `pfp-model`), and re-exported; the block below is a verbatim reference copy with the same spellings, not a second declaration (`ARCHITECTURE.md` §2's one-home CI check fails a redefinition).

```rust
struct Cents(i64);                       // all money; JSON carries integer cents
struct Ratio { num: i64, den: i64 }      // every statutory rate/factor, parsed from "0.0765" or "5/900"
struct RoundingRule { increment: Cents, direction: Down|Up|HalfUp|HalfEven|Nearest,
                      basis: Amount|IncreaseOverBase }
fn Cents::mul_ratio(self, r: Ratio, rule: &RoundingRule) -> Cents;   // i128 inside, rounds once
fn Cents::grow(self, factor: f64) -> Cents;                          // one of the two named f64 -> Cents entries
                                                                     // (the other, Cents::from_f64_half_even, serves
                                                                     // solvers and spending rules; ARCHITECTURE D5)
type Year = i32;  type Month = MonthYm;   // MonthYm { y: i32, m: u8 }
enum Owner { P1, P2, Joint }
enum FilingStatus { Single, Mfj, Mfs, Hoh, Qss }   // wire forms "single" | "mfj" | "mfs" | "hoh" | "qss"
```

`Cents * Cents` does not compile. `f64` is fenced per D5, so **every application of a return, an inflation index or a half-year factor to money is a call to `grow`**, and every application of a statutory rate is `mul_ratio` with the rule stored beside the parameter. Prose in this document writes filing statuses as MFJ, MFS, HOH and QSS; the code identifiers are the `FilingStatus` variants above.

### 1.2 Time and timing conventions (printed on every assumptions sheet)

| Convention | Rule |
|---|---|
| Ledger step | Calendar year `t`. Year 0 is the plan's `asOf` year and is always a **full tax year** (1.4): tax, payroll, caps and phase-outs see year-to-date actuals plus the remaining-month flows, never the remaining months alone. |
| Ages | Age attained in year `t` = `t - birthYear` (RMD and catch-up rules use attained age); month precision only in monthly sub-engines. |
| Withdrawals and spending | Start of year (ADR-008). |
| Contributions and the routed surplus | Mid-year: new money receives `grow((1+r_t)^0.5)` in its first year (in year 0, `grow((1+r_t)^(m/24))` for `m` remaining months, the mid-point of the remaining window). Extra debt principal is spread evenly over the remaining monthly steps. The two are timed alike so that invest-vs-prepay comparisons carry no timing bias. |
| Deaths | End of year (ADR-008). |
| Tax settlement | Liability for year `t` is paid in year `t` (withholding and estimates assumed exact), except the PTC reconciliation delta, which settles in `t+1` [G5]. IRMAA is charged in the year it is paid and *attributed* to the income year only inside the EMR statistic. |
| Dollars | Nominal internally. A stream whose `basis` is `Real` (`DOMAIN-MODEL.md` §2.4) is converted with `amount0.grow(I_t)`, where `I_t` is the path's cumulative inflation index. Display values are deflated by `I_t` in `pfp-report`, never in the ledger. |

### 1.3 Parameter access

`ParamView::for_year(t, &inflation_index)` returns thresholds already uprated under each table's projection rule and `RoundingRule` (`index | wage | flat | zero | schedule`). The engine never uprates. Never-indexed items carry `flat` explicitly: NIIT and Additional Medicare thresholds, Social Security taxation bases, the $3,000 loss cap, the senior-deduction amounts and thresholds, the IRMAA top tier [TAXD].

**Uprating is computed from the statutory base year, never chained** (3.7). Each indexed table carries `projection{rule, index, index_series, base_year, base_values[status], rounding}`: the uprated amount is `base_value x (index_t / index_base_year)` with the `RoundingRule` applied **once** to the result, and for `basis: IncreaseOverBase` applied once to the increase over `base_value`. Chaining year over year (uprating year `t` from year `t-1`'s already-rounded amount) rounds repeatedly and diverges from the statutory result; a negative test asserts that divergence on at least one 2026 threshold. `index_series` names the series the statute uses, **not** the path's own price index. 26 USC 63(c)(4) [USC63] states the rule and the standard deduction's 2024 post-OBBBA base; the bracket tables' base year under 26 USC 1(f) [USC1] and the identity of the chained-CPI series are **read from statute at the M0 hand-verification gate** before the first vintage locks (`TESTING.md` §5.2, `DECISIONS.md` C3), not asserted here.

**`assumptions.chained_cpi_wedge`** (`DOMAIN-MODEL.md` §15 `AssumptionSet`; default 0, editable, printed on the assumptions sheet) is the annual amount by which the statutory chained index is assumed to run **below** the path's CPI: `index_t = cpi_t x (1 - wedge)^(t - base_year)`. No archived source quantifies the wedge, so the default leaves projected real bracket creep at zero and says so on the sheet rather than hiding the assumption.

### 1.4 Year 0 is a full tax year

The plan's `asOf` date falls inside a calendar year whose tax, payroll, caps and phase-outs are **calendar-year constructs**. Prorating income to the remaining months would put a two-earner household opened in September at a fraction of its actual wages, reading a lower bracket for `t_now`, the wrong position on the OASDI wage base, the wrong Roth IRA phase-out, and NIIT and Additional Medicare thresholds that never bind. Year 0 is therefore assembled from two parts:

```
plan.ytd: Option<YearToDate { as_of, wages[person], withholding[person], contributions[account],
                              employer_contrib[account], realized_gains }>   // DOMAIN-MODEL §3.1 owns the shape
tax base (year 0)  = ytd actuals + remaining-month projected flows       // a full calendar year
payroll() input    = full-year wages per person (ytd wages + remaining-month wages)
cap room           = statutory cap - ytd employee contributions; 415(c) room also nets ytd employer money
routable cash      = remaining-month surplus only        // ytd dollars are already spent or contributed
```

When a household has not entered YTD actuals (`plan.ytd` is `None`), the engine **annualizes** the entered current rates to a full year and prints the annualization as a caveat on every card that depends on it; it never falls back to a prorated stub year. Tier-1 case: the same synthetic household with `asOf` in January and with `asOf` in October yields an identical `t_now`, an identical wage-base position and identical phase-out placement; only `routable cash` and the growth exponent differ.

---

## 2. The annual ledger

### 2.1 Signature and state

`project(plan, assumptions, params, path, opts) -> Projection` (seam S6; signature in `ARCHITECTURE.md` 4.1). Internally:

```rust
struct LedgerState {                       // carried between years (step 8)
  people:   BTreeMap<PersonId, PersonState>,       // alive, age, employed, coverage, claimed SS, medicare
  accounts: BTreeMap<AccountId, AccountState>,     // balance, basis (avg-cost to M8, lots M9), allocation
  roth:     BTreeMap<AccountId, RothLedger>,       // contribution basis, conversions[(year, amount)], earnings
  roth_clocks: BTreeMap<RothClockKey, Year>,       // first funding year per clock group (2.4):
                                                   // RothClockKey = IraGroup(PersonId) | EmployerPlan(PersonId, PlanId)
  debts:    BTreeMap<DebtId, DebtState>,           // balance, rate, remaining term, promo end
  properties: BTreeMap<PropertyId, PropertyState>, // 1.1
  loss_carryforward: Cents,
  magi_history: [MagiPair; 2],                     // (t-1, t-2): {aca, irmaa, status}; seeded for plan years 0-1
                                                   // from the plan fact household.prior_magi (DOMAIN-MODEL 4; 3.4)
  ytd: YearToDate,                                 // year 0 only (1.4), from plan.ytd; zeroed from year 1 on
  ira_basis: BTreeMap<PersonId, Cents>,            // Form 8606 line 14 carry
  hsa_receipts: BTreeMap<PersonId, Cents>,
  ptc_reconcile_due: Cents, filing_status: FilingStatus, first_death_year: Option<Year>,
}
```

### 2.2 Order of operations within year `t`

```
1  PEOPLE      resolve alive, age, employed, coverage regime, filing status (section 11 sets status;
               the flip happens in t+1 after a death in t)
2  INCOME      wages, vest income (+withholding), SS (monthly engine aggregated; the entered SsEstimateEntry,
               resolved to a PIA, until M5), pensions/annuities x survivor fraction, rental NOI (1.1),
               RMD_p = prior-12/31 balance / divisor(age)   for each person past the start age,
               planned Roth conversions (M8), taxable-account yield (interest, QDI, turnover gains).
               In year 0 each figure is ytd actual + remaining-month projection (1.4); only the
               remaining-month part reaches step 6 as routable cash
3  PAYROLL     payroll(year, full-year wages) -> OASDI + regular Medicare only;
               mandatory pre-tax flows; employer match and nonelective contributions.
               Additional Medicare is NOT booked here: it is the f8959 line inside federal() (3.2)
4  OUTFLOWS    essential then discretionary spending; dated goals in priority order; healthcare
               regime cost with component inflation (1.1; a user-entered line before that);
               property carrying costs (1.1); insurance premiums;
               DEBT sub-engine: 12 monthly steps of required payments (section 8)
5  TAX (est.)  federal() two-pass with the state module (section 4.3) -- NIIT (f8960) and Additional
               Medicare (f8959) are lines INSIDE federal(), not separate addends;
               IRMAA surcharge from (magi_history[t-2].irmaa, magi_history[t-2].status) for each person
               on Medicare, + ptc_reconcile_due carried from t-1
6  ROUTE       net = cash inflows - outflows - tax
               net > 0 -> ContributionPolicy (section 5; Exact mode in year 0, TierFill later)
               net < 0 -> WithdrawalPolicy  (section 2.4)
               re-run step 5 on final flows; book the difference through the cash account (2.5)
7  GROW        per account: balance.grow(1 + sum_k w_k * r[t][k] - fee); new money gets the half-year factor;
               rebalance per policy (within-account only until M9; taxable rebalancing gains are an
               omission printed on results until then)
8  CARRY       basis, Roth ledger and five-year clocks, loss carryforward, MAGI pair, 8606 basis,
               HSA receipts, debt states, PTC reconcile due
9  EMIT        one LedgerRow (every cell has a LedgerRef) + FederalReturn + StateReturn + metrics
```

Monthly sub-engines run only where the rule is monthly and aggregate into the year: debt amortization (M3), Social Security (M5), coverage switches (1.1).

**One owner per tax.** Every tax is booked by exactly one function. `payroll()` owns OASDI and regular Medicare. `federal()` owns everything on the return, including NIIT (`f8960.*`) and Additional Medicare (`f8959.*`) — which is where the Additional Medicare threshold can be applied on the MFJ **combined** $250,000 basis the statute uses and a per-person `payroll()` cannot. The state module owns state tax; the IRMAA lookup and the PTC reconciliation are their own ledger lines. Conservation (2.5) cannot detect a tax booked twice, so a property test asserts that the tax cells of every `LedgerRow` sum to `total.* + state.total + irmaa + ptcReconcile` **exactly once**, over all personas and generated cases.

### 2.3 Surplus routing

In year 0 the surplus — the **remaining-month** surplus (1.4) — is routed by the next-dollar greedy loop in **Exact** mode with `Full` trace. In projection years the same `ContributionPolicy` is evaluated in **TierFill** mode: tiers are visited in policy order, each option is filled to `min(remaining surplus, cap)` in one step, `r_u` is evaluated once at the year's base for the tier-8-10 block test and for the optional order-by-`r_u` sort, and tax is recomputed once per filled pre-tax option. Both modes satisfy the same invariants (caps, liquidity floors, allocations sum to the surplus). Year 0 is path-invariant, so Monte Carlo computes it once.

**The pre-tax / Roth split of projected deferrals is a stored decision, never a tie-break.** `PlanPreTax` and `PlanRoth` have identical `r_u` until the `t_future` term enters at M3 (5.3), so the greedy loop's "ties by id" rule would otherwise make a silent, arbitrary Roth-vs-Traditional choice on the headline card while section 6's card is still saying "no verdict". Instead the split is the wage stream's stored `PayrollDetail.deferral_type` (`Traditional | Roth | Split{pct_roth}`, `DOMAIN-MODEL.md` §7), which is the current payroll election as entered at setup and is per person and per plan because the stream is. TierFill and the greedy loop route the recommended **amount** into the employer plan and divide it by that stored split; the Roth-vs-Traditional card is the only place that recommends **changing** it, and *Adopt* writes `/incomeStreams/*/payroll/deferralType` (`DOMAIN-MODEL.md` §12.1). This also closes the circularity: projection years use the stored split, so traditional balances, RMDs and therefore `t_future` are well defined; the verdict is computed on that baseline and, if the user adopts it, the projection is re-run **once** with the new split and the verdict recomputed — a documented two-pass fixed point, not an iteration to convergence, and the card prints which pass produced it.

### 2.4 Deficit routing

`WithdrawalPolicy` is an ordered rule list stored in the plan (S7). **This section is the single owner of deficit routing, the 59.5 guard and the gross-up solve;** `SIMULATION-SPEC.md` 13 states the policy presets and refers here for the mechanism. Default, "conventional" (M3):

```
1. RMDs are already in income (step 2); an RMD in excess of need is routed as surplus to taxable.
2. cash above the spending-shock buffer
3. taxable accounts (realized gain = withdrawal x (1 - basis/value); basis reduced pro rata)
4. traditional / employer pre-tax of any person aged 59.5 or older
5. Roth: contribution basis, then conversions older than five tax years (FIFO), then earnings if 59.5+ and
   the qualified-distribution clock for that group is met                                  [TAXD, Roth ordering]
6. HSA for banked receipts (tax-free), then as ordinary income after 65
59.5 guard: before 59.5, steps 4-5 may touch only penalty-free sources (Roth basis, seasoned conversions);
   if the deficit is still uncovered the year is marked SHORTFALL (spending unmet); the engine never
   silently books a penalized withdrawal.
```

**Roth clocks are per group, not per account.** `RothLedger` stays per account, because contribution basis and conversion rows are per account. The **qualified-distribution clock**, however, runs per `(person, Roth IRA group)` — all of a person's Roth IRAs share one clock, dated from the first tax year that person funded any Roth IRA, and conversion ordering is applied across them together — and separately per `(person, employer plan)`, because designated Roth accounts in an employer plan genuinely carry their own clock. `LedgerState.roth_clocks` (2.1) holds both. Without this, opening a new Roth IRA beside a fifteen-year-old one would make the household's withdrawal look unseasoned and book spurious taxable earnings or penalty. IRAs are never aggregated across spouses (5.6), but they are aggregated within a person. Tier-1 case: one person with two Roth IRAs of different ages takes a distribution; the older account's clock governs both.

**SHORTFALL is the default; the penalized alternative is computed beside it.** A year the 59.5 guard cannot cover is marked SHORTFALL, and the card also reports the **penalized alternative**: the gross pre-tax withdrawal that would cover the deficit, its ordinary tax, the early-distribution additional tax as its own named line (`f5329.*`, 3.2), and the resulting net worth, so the user sees what the guard is withholding rather than an unexplained unmet year. Taking it requires adding a rule to `WithdrawalPolicy.early_access` (`DOMAIN-MODEL.md` §11, empty by default), and the three entries are all explicit consent, never inference: `EarlyAccessRule::Penalized{account_ids}` books the penalized alternative from the named accounts once penalty-free sources are exhausted (honoured from M3, since it asserts no eligibility — only acceptance of a tax the engine already computes); `SeparatedAt55{employer_plan_id}` (a distribution from the employer plan of the year of separation at 55 or later, penalty-free, employer plans only — never an IRA); and `Sepp72t{account_id, started, method}` (substantially equal periodic payments, with the modification window and its retroactive-penalty consequence printed on the card). The last two are facts the user asserts whose statutory conditions are hand-verification-gate items, so the engine honours them from M8 (`PLAN.md` M3, `ARCHITECTURE.md` §7).

Proportional and bracket-managed policies arrive in M8 as additional rule lists; `trait WithdrawalOrder` is unchanged.

**Gross-up.** A withdrawal creates tax, which enlarges the deficit. Solve `x - [Tax(base + x) - Tax(base)] = D` by **secant iteration on the real `federal()`**, because the kink list is not closed in closed form: bracket edges, the Social Security phase-in, LTCG boundaries and the NIIT threshold are joined by the senior-deduction phase-out, the IRMAA tier boundary two years out, the PTC cliff and the state module's own kinks. The segment-by-segment closed form is retained **only as the initial guess** `x0`, which is what makes the iteration cheap:

```
x0        = closed-form segment solve from the lines returned by federal() at the pre-withdrawal base
            (fallback x0 = D / (1 - emr_prev) when no trace is available)
iterate   secant on f(x) = x - dTax(x) - D  until |f| <= $1; deterministic runs settle to the cent
```

The tax booked is the final `federal()` evaluation, never the estimate; any residual flows through the cash account, and only if cash would go below zero does the outer loop repeat. Every repeat spends further `federal()` evaluations against the **per-year evaluation cap** below — there is no separate iteration cap — so reaching the cap marks the year SHORTFALL however the evaluations were spent.

**Evaluation budget (the single contract, owned by this section; `DECISIONS.md` C2 records it, and C2, `PLAN.md` M1, `ARCHITECTURE.md` §4.3, `SIMULATION-SPEC.md` §9 and `TESTING.md` §10 cite it and carry the same numbers in the same terms — `ARCHITECTURE.md` §4.3 owns the wall-clock gates and the per-evaluation derivation, not the counts).** With `trace: TraceLevel::None`, per ledger-year:

| Year kind | Budget | Composition |
|---|---|---|
| Accumulation | **a mean of at most 2** | the federal-state two pass (4.3), whose pass 3 the standard-deduction shortcut skips in the common case; TierFill's per-option recompute reuses the same two evaluations at the tier's base |
| Retired (any gross-up) | **a mean of at most 3** | the two-pass plus one settle on the final flows; the closed-form seed means typical convergence is one secant step |
| Any | **hard cap 6** | the per-year evaluation cap: no single ledger-year makes more than six full `federal()` evaluations, counting passes, secant steps, settles and outer cash-loop repeats alike; when the cap is reached the year is marked SHORTFALL with the last evaluation's tax booked |

**These are means under the hard per-year evaluation cap, not per-year maxima.** The cap bounds the tail; the mean is what the ledger-year gate measures, and the benchmark counts and reports every year that spends more than its mean. At 10,000 paths x 60 years the means give about 1,500,000 evaluations, which is the figure the M1 probe is sized to at the wall-clock limits (2 s on eight cores, 8 s single-threaded), so the ADR-007 contingency ladder triggers at M1 on the realistic load rather than at M6 with the tax kernel already written. The equivalent statement of the same gate, and the one worth benchmarking directly, is **one simulated ledger-year in at most 27 microseconds of core time including all tax calls**.

### 2.5 Conservation identity (the ledger's primary control)

For every year, as a `debug_assert` and a CI property over all personas and generated cases:

```
sum(opening balances) + inflows + growth - taxes - spending - debt service
  - bequests out == sum(closing balances)          residual == Cents(0), exactly
```

`growth` here is the **net** growth that step 7 books, already net of the `fee` term inside `grow()`; the identity therefore carries no separate fees term, and the fee amount is reported as its own display line rather than a second subtraction. `inflows` includes employer contributions and insurance proceeds; `debt service` is interest plus principal (a second identity covers net worth including debts and property). Transfers between accounts (conversions, rollovers, re-ownership at death) net to zero by construction. PLAN M3's metamorphic properties are stated against this function.

### 2.6 Ledger row (export shape)

`LedgerRow{year, ages[], filingStatus, state, income{wages, vest, ss, pension, rental, rmd, conversion, interest, qdi, gains}, payroll{oasdi, medicare}, outflows{essential, discretionary, goals[], healthcare, housing, premiums, debtInterest, debtPrincipal}, tax{federal, state, niit, addlMedicare, irmaa, ptcReconcile}, routed{contributions[{accountId, option, cents}], withdrawals[{accountId, cents, realizedGain}]}, balances{byAccount, byTaxType}, debts{byDebt}, metrics{emrOrdinary, emrLtcg, savingsRate, magiAca, magiIrmaa, shortfall}, pins}`. Every numeric cell is addressable by `LedgerRef{year, path}`. `tax.niit` and `tax.addlMedicare` are **memo lines**: both are already included in `tax.federal` (they are `f8960.*` and `f8959.*` inside `federal()`), shown separately so a reader can see the surcharge, and excluded from any summation that also adds `tax.federal`.

---

## 3. The federal tax function

### 3.1 Signatures (seam S3)

```rust
pub fn federal(year: Year, status: FilingStatus, inputs: &TaxInputs, params: &ParamView,
               trace: TraceLevel) -> FederalReturn;
pub fn payroll(year: Year, person_wages: &[PersonWages], params: &ParamView,
               trace: TraceLevel) -> PayrollLines;                    // OASDI + regular Medicare only
pub fn marginal(year, status, base: &TaxInputs, delta: IncomeDelta, ctx: &CostCtx, params,
                trace: TraceLevel) -> MarginalRate;

struct TaxInputs {             // every income character from M1; zero until modelled
  persons: SmallVec<[TaxPerson; 2]>,      // birth year, blind, covered_by_employer_plan, on_medicare
  dependents: Dependents,                 // qualifying children, other dependents
  wages: PerPerson<Cents>, se_income: PerPerson<Cents>,
  interest: Cents, tax_exempt_interest: Cents, ordinary_dividends: Cents, qualified_dividends: Cents,
  st_gains: Cents, lt_gains: Cents, loss_carryforward_in: Cents,
  ira_distributions: Cents, ira_taxable_override: Option<Cents>,   // set by Form 8606 worksheet
  pensions: Cents, roth_conversions: Cents, social_security: Cents,
  rental_net: Cents, unrecaptured_1250: Cents,                      // populated 1.1; zero until then
  early_distributions: Cents,   // pre-59.5 amounts not covered by an exception; f5329 (3.2); non-zero only under
                                // an EarlyAccessRule::Penalized entry or the printed penalized alternative (2.4)
  vest_income: Cents, vest_withholding: Cents, iso_spread: Cents,   // iso_spread != 0 -> NotModelled
  above_line: AboveLine,  // pre-tax deferrals are already out of wages; HSA (non-payroll), trad-IRA
                          // contribution, student-loan interest, half SE tax (computed)
  itemizable: Itemizable, // state_income_tax, property_tax, mortgage_interest, acquisition_debt,
                          // charity, medical
  car_loan_interest: Cents,  // OBBBA 2025-2028: available to NON-itemizers, so not in Itemizable (3.2 line 4).
                             // A field ADDITION within S3, not a signature change: S3 freezes the shape
                             // "every income character, zeros until modelled", and this character was
                             // previously mis-filed inside Itemizable where no formula could reach it.
  qbi: Cents, passive_losses: Cents, foreign_income: Cents,         // non-zero -> NotModelled
}
enum IncomeDelta { Ordinary(Cents), Wages{person, cents}, LtGain(Cents), Qdi(Cents), IraDistribution(Cents),
                   RothConversion(Cents), PreTaxDeferral{person, cents}, HsaPayroll{person, cents},
                   MortgageInterest(Cents), StudentLoanInterest(Cents) }
struct MarginalRate { point: Ratio, block: Ratio, delta: Cents,
                      components: {federal, state, irmaa_pv, ptc_lost},      // sum to block x delta
                      memo:       {niit, addl_medicare},                     // already inside `federal`
                      lines_before: FederalReturn, lines_after: FederalReturn }  // empty under TraceLevel::None
```

`FederalReturn` is a `BTreeMap<LineId, Line>` plus typed accessors, a `not_modelled: Vec<NotModelled>` list and both MAGIs. Inputs the function does not model (EITC eligibility, education credits, QBI, passive losses, ISO/multi-year AMT, foreign income) **raise a flag; they are never silently ignored** (R8).

**`TraceLevel` is part of the frozen signature (S3), not of `ProjectOpts`.** Under `TraceLevel::None` the function computes and returns the totals and every typed accessor but leaves the `lines` map **empty**, constructing no `Line` nodes at all. This is the escape hatch the performance budget depends on: a full return is 40-plus `Line` nodes, each holding two `SmallVec`s, and Monte Carlo allocates one per ledger-year across up to 1.5 million evaluations (2.4). Putting `trace` on the signature at M1 means the hatch exists before the seam freezes; adding it later would mean reopening a frozen seam. `Summary` keeps the worksheet totals; `Full` keeps everything and is what deterministic runs and the Explain panel use. A Tier-1 property asserts that `None` and `Full` agree **to the cent on every typed accessor** for the whole fixture corpus, and the M1 probe reports the two modes separately. Suppressing `Line` construction under `TraceLevel::None` is rung 0 of the ADR-007 contingency ladder — before any arithmetic change.

### 3.2 Worksheet order and named lines

Line ids are stable strings; the prefix names the worksheet. Each line lists its inputs and parameters, which is what the Explain panel and the Tier-1 fixtures read.

| # | Worksheet (line prefix) | Computation | Parameters (2026 value, source) |
|---|---|---|---|
| 1 | `se.*` | `se_base = 0.9235 x SE`; `se_tax = 0.124 x min(se_base, max(0, wage_base - wages)) + 0.029 x se_base`; half is above-the-line | OASDI wage base $184,500; 15.3% on 92.35% [SSA-FR, T751, TAXD] |
| 2 | `agi.*` | `agi_ex_ss = wages + vest + SE net + interest + ordinary dividends + max(net gains, -loss cap) + IRA taxable + pensions + conversions + rental net - above-line`. Capital netting: ST and LT net against each other and the carry-in; net loss above the cap is carried out | Loss cap $3,000 ($1,500 MFS), flat [P550] |
| 3 | `pub915.ws1.*` | `agi_ex_ss_915 = agi_ex_ss + student_loan_interest_deduction` — the worksheet's provisional income does **not** subtract the student-loan interest deduction, while line 2's `agi_ex_ss` is net of every above-line item, so that one adjustment is added back explicitly. Then `PI = agi_ex_ss_915 + tax_exempt_interest + 0.5 x SS`; `taxable_ss = 0` if `PI <= base`; `min(0.5 SS, 0.5 (PI - base))` if `PI <= adj`; else `min(0.85 SS, 0.85 (PI - adj) + min(0.5 SS, 0.5 (adj - base)))`. Lines numbered as Pub 915 Worksheet 1; a fixture with a non-zero above-line adjustment covers the add-back | Base $25,000 single / $32,000 MFJ / $0 MFS living together; adjusted $34,000 / $44,000; flat [P915] |
| 4 | `ded.*` | `std = table[status] + n_aged_or_blind x addon`; `senior = sum over persons 65+ of max(0, amount - 0.06 x max(0, MAGI - threshold))` for dated years; `itemized_raw = min(SALT, salt_cap(year, MAGI)) + mortgage_interest x min(1, debt_cap / acquisition_debt) + max(0, charity - floor x AGI) + max(0, medical - 0.075 x AGI)`; `itemized = itemized_raw - cap37(itemized_raw, TI)` (the OBBBA limitation of itemized deductions to 35 cents per dollar for filers in the 37% bracket). `cap37` is evaluated against the **top marginal bracket implied by `TI` before preferential stacking**, not against the computed liability, so line 4 does not depend on line 9 and the worksheet stays acyclic; `deduction = max(std + nonitemizer_charity, itemized) + senior + car_loan_interest_allowed`; `TI = max(0, AGI - deduction)`. The senior deduction and the car-loan interest deduction are available whether or not the household itemizes, so **those two are added outside the `max`**; the non-itemizer charitable deduction is **forfeited on itemizing**, so it rides with `std` **inside** the `max` — placing it outside would hand it to an itemizing household too | Std $32,200 MFJ / $16,100 single, MFS / $24,150 HOH; add-on $1,650 married, $2,050 unmarried [RP25-32]. Senior $6,000 per person 65+, 6% phase-out above $150k MFJ / $75k single, tax years 2025-2028 [USC151]. Non-itemizer charity $1,000 / $2,000 MFJ from 2026; 35-cent cap for 37% filers [TAXD]. SALT cap $40,000 (2025), $40,400 (2026), +1%/yr to 2029, $10,000 from 2030; the phase-down **threshold has its own dated schedule** (`fed.ded.salt.phasedown_threshold`), not a fixed $500,000, because the cap is indexed 1%/yr while the reported threshold is a secondary-source figure; phase-down reported as 30% of the excess over the threshold, floored at $10,000 **(unverified, secondary source — both the threshold schedule and the rate go through the hand-verification gate)** [PL119-21, G1]. Acquisition debt cap $750,000 [P936]. Charitable floor 0.5% of AGI from 2026 [TAXD]. Car-loan interest up to $10,000, phase-out above $200k MFJ / $100k, 2025-2028 [PL119-21] |
| 5 | `sched.*`, `qdcg.*` | `pref = QDI + max(0, net LTCG)`; `ord_TI = max(0, TI - pref)`; `tax_ord = schedule(ord_TI)`; `zero_room = max(0, Z - ord_TI)`; `fifteen_room = max(0, F - max(ord_TI, Z))`; `tax_pref = 0.15 x min(max(0, pref - zero_room), fifteen_room) + 0.20 x max(0, pref - zero_room - fifteen_room)`; unrecaptured 1250 gain at `min(ordinary, 25%)` (1.1, with `Property`) | MFJ brackets top-of-band $24,800 / $100,800 / $211,400 / $403,550 / $512,450 / $768,700; single $12,400 / $50,400 / $105,700 / $201,775 / $256,225 / $640,600; Z/F $98,900 / $613,700 MFJ, $49,450 / $545,500 single, $66,200 / $579,600 HOH [RP25-32] |
| 6 | `f8960.*`, `f8959.*` | `NIIT = 0.038 x min(NII, max(0, MAGI - threshold))`, NII = interest + dividends + gains + rental net (conversions and IRA distributions are **not** NII); `addl_medicare = 0.009 x max(0, earned - threshold)` where `earned` is the **household** total of wages and SE income on a joint return, which is why this line lives here and not in `payroll()`: the $250,000 MFJ threshold is a combined-earnings threshold that a per-person payroll function cannot apply. Employer withholding of the surcharge is a credit against this line, not a second charge | $250k MFJ / $200k single / $125k MFS, flat [NIIT] |
| 7 | `amt.*` (sanity check, M2) | `AMTI = TI + (standard deduction if not itemizing, else SALT deducted) + preferences` — the standard deduction is not allowed against AMTI, so omitting the add-back under-reports in MFS and high-gain cases; `exemption = max(0, E - 0.5 x max(0, AMTI - P))`; `TMT = 0.26 x min(base, B) + 0.28 x max(0, base - B)` with preferential income keeping its LTCG rates; `AMT = max(0, TMT - regular)`. `iso_spread != 0` -> `NotModelled(MultiYearAmt)`; the return is still produced, labelled incomplete | E $140,200 MFJ / $90,100 single; P $1,000,000 / $500,000; B $244,500 [RP25-32] |
| 8 | `credits.*` | Child tax credit and credit for other dependents with their phase-outs (amounts and thresholds from `fed.credits.*`; CTC $2,200 [TAXD]); Saver's credit by AGI band. EITC, education credits -> `NotModelled` | [TAXD] |
| 8b | `f5329.*` | `early_dist_tax = rate x early_distributions` — the additional tax on early distributions not covered by an exception, its own named line so the penalized alternative of 2.4 and an `EarlyAccessRule::Penalized` withdrawal both show it separately from ordinary tax. Zero unless `early_distributions` is non-zero | `fed.early_dist.rate` and the exception list are **(unverified — not covered by the research)**; the line exists in the return shape from M3 and its parameters clear the hand-verification gate before any engine books it |
| 9 | `total.*` | `total = tax_ord + tax_pref + AMT + NIIT + addl_medicare + early_dist_tax + se_tax - credits`; `vest_trueup = Tax(with vest) - Tax(without) - vest_withholding` (informational line feeding step 4 cash flow) | Supplemental withholding 22% (37% above $1M) [DR 4.5] |
| 10 | `magi.*` | Both MAGIs (3.4) | |

The rate schedule is the canonical computation (D9): `schedule(x) = sum_b rate_b x max(0, min(x, top_b) - bottom_b)`, each product via `mul_ratio`, summed in cents, rounded once to the whole dollar under the `irs.whole_dollar` rule (half-up). Tier-1 check: 2026 MFJ taxable 100,000 -> 11,504; 150,000 -> 22,424; 250,000 -> 45,196, derived from the [RP25-32] brackets [DR 4.9].

### 3.3 Marginal rates and the EMR curve

```
marginal(base, delta):  a = Cost(base);  b = Cost(base + delta);  block = (b - a) / |delta|
Cost = federal total.* + state total
     + PV(IRMAA surcharge change in t+2, at assumptions.discount.nominalSafe, x persons on Medicare in t+2)
     + PTC lost in t (M8)
point = the same with delta = emr.step ($1,000 default)      # point EMRs at cliffs are reported as the block
```

`total.*` (3.2 line 9) **already contains NIIT, Additional Medicare, AMT, the early-distribution additional tax and the self-employment tax**; adding them again as separate terms would double count both surcharges in every EMR and in every `r_u` built on `Cost`, which bites hardest on exactly the high-wage two-earner households the surcharges exist for. `Cost` therefore names `total.*` once and adds only the two items that are not on the federal return: the IRMAA surcharge and the lost premium tax credit.

`emr_curve(year)` sweeps ordinary income and LTCG separately at `emr.step` from zero to the household's base plus a margin. Shapes that must reproduce (Tier 1 / M1 and M3 acceptance): Social Security torpedo 22% -> 40.7% (1.85x); LTCG bump zone 27%; 24% + 3.8% = 27.8%; first MFJ IRMAA tier = `2 x ($81.20 + $14.50) x 12 = $2,297`/yr [TAXD, CMS]. **Marginal is always compared with marginal, never with an average rate.**

**Senior-deduction phase-out: computed targets, not "+6 points".** The 6% in 26 USC 151(d)(5) is the **rate at which the deduction shrinks per dollar of MAGI, per eligible person** (3.2 line 4) — the deduction runs off over $100,000 of MAGI, $75k-$175k for one person and $150k-$250k for two, which only works at 6 cents per dollar per person. It is not 6 percentage points of marginal rate. EMR rises by `0.06 x n_eligible x bracket`, so the shapes are:

| Band | Bracket | EMR |
|---|---|---|
| Single, one person 65+, MAGI $75k-$175k | 22% | **23.32%** (12% -> **12.72%**) |
| MFJ, two persons 65+, MAGI $150k-$250k | 22% | **24.64%** (12% -> **13.44%**) |
| Inside the Social Security torpedo, one eligible person | 22% | `22% x 1.85 x 1.06` = **43.1%** (43.14%) — each ordinary dollar adds $1.85 of AGI, which shrinks the deduction by a further 6% of that |

Reading the phase-out as "+6 points" transposes the rate for a rate change; these computed targets are the only form in which the fixture is stated, and the correction is recorded in `DECISIONS.md` C1 (section 14). A correct engine cannot pass a "+6 points" gate, so such a gate would force either a bent engine or a stalled M1.

### 3.4 MAGI: two definitions, two clocks

| MAGI | Definition | Clock | Used by |
|---|---|---|---|
| `magi.aca` | AGI + tax-exempt interest + non-taxable Social Security + excluded foreign income | **Current year**, against the **prior year's** poverty guideline | PTC estimate in `t`, reconcile at filing, delta settles `t+1` (M8) [8962, G5] |
| `magi.irmaa` | AGI + tax-exempt interest | **Two years back** (`magi_history[t-2]`), household figure **with the filing status of that same return**; both spouses pay | IRMAA surcharge (M3) [USC1395r, CMS] |

Other phase-outs (Roth IRA, IRA deduction, senior deduction, student-loan interest, NIIT) use `MagiKind` variants whose add-backs are data in `fed.magi.<kind>`; v1 sets each to AGI plus the add-backs listed in that table. *The add-back lists for these variants were not covered by the research and are hand-verified at the M1/M2 gate.*

IRMAA is a **table lookup**, never a multiple of the base premium: published table for the current year, Trustees Table V.E3 for 2027-2035, ratios only beyond [G5]. 2026 MFJ thresholds $218k / $274k / $342k / $410k / $750k (single $109k / $137k / $171k / $205k / $500k); Part B add-ons $81.20 / $202.90 / $324.60 / $446.30 / $487.00; Part D add-ons $14.50 / $37.50 / $60.40 / $83.30 / $91.00; thresholds rounded to the nearest $1,000, top tier frozen through 2028 [CMS, G5].

```
irmaa_t = persons_on_medicare_t x 12 x (partB_addon + partD_addon)(magi_history[t-2].irmaa,
                                                                    magi_history[t-2].status)
```

**The lookback reads the lookback year's filing status, not the current year's.** SSA determines the surcharge from the return it actually has: the MAGI on the year `t-2` return **and the status that return was filed under** (the "two-year filing-status lookback" [TAXD]). Reading a joint MAGI against single-filer tiers is not a conservative approximation, it is a large one-directional error: a $250,000 joint MAGI lands in single tier 4 (above the $205k threshold: `12 x ($446.30 + $83.30)`, about $6,355/person/yr) instead of MFJ tier 1 (above $218k: `12 x ($81.20 + $14.50)`, about $1,148/person/yr; tiers numbered from the first surcharge tier, the base premium being tier 0), a spurious spike of roughly $5,000 per person for two years that then feeds the survivor-year EMRs and the conversion planner. `MagiPair` therefore stores `{aca, irmaa, status}` (2.1), and the plan carries `household.prior_magi: [Option<MagiPair>; 2]` — the `[t-1, t-2]` pairs, each with its filing status — as ordinary plan facts (`DOMAIN-MODEL.md` §4), because plan years 0 and 1 have no prior ledger year to read; an absent entry raises a `NotModelled(IrmaaLookback)` caveat for that year rather than a silent zero.

**SSA-44 life-changing events.** Death of a spouse, work stoppage, work reduction and a handful of other events let a beneficiary ask SSA to use the **current** year's MAGI and status instead of the lookback year's. `ssa44` is a per-event toggle, **default on at a modelled death event** and off elsewhere; when it applies, the lookup substitutes the current year's `magi.irmaa` and status for the lookback pair. Both the toggle and the substituted year are printed on the assumptions sheet, since the relief is a filing the household must actually make.

PTC (M8): `PTC = max(0, 12 x benchmark - applicable_pct(FPL%) x magi.aca)`, zero above 400% FPL (a dated, switchable parameter) and below the eligibility floor; applicable percentages 2.10% below 133% rising piecewise-linearly to 9.96% at 300-400% [RP25-25]; guideline for 2026 coverage $15,650 + $5,500 per additional person, so 400% for two = $84,600 [ASPE]; no repayment cap after 2025 [PTCQA].

### 3.5 RMDs

`RMD_t = balance(12/31, t-1) / divisor(age_t)` per person over all traditional/pre-tax accounts; start age 73 (born 1951-1959), 75 (born 1960+); Uniform Lifetime table, or the Joint table when the sole beneficiary is a spouse more than ten years younger [P590B]. No lifetime RMD on Roth accounts. The first-year April-1 deferral is not modelled (the RMD is taken in the year the age is attained). Divisors are a parameter table (73: 26.5, 74: 25.5, 75: 24.6, 80: 20.2, 85: 16.0, 90: 12.2). Tier 1: $100,000 at 75 -> $4,065; $3,953 on the Joint table; regression that 74 -> 25.5 (the publication's defective 2026 example half is not encoded) [P590B; DR 4.9].

### 3.6 Contribution limits and phase-outs (M2)

| Item (param id) | 2026 | Source |
|---|---|---|
| `fed.limits.elective_deferral`; catch-up 50+; ages 60-63 | $24,500; $8,000; $11,250 | [N25-67] |
| Catch-ups Roth-only when prior-year FICA wages from the sponsor exceed | $150,000 | [N25-67, FR-CU] |
| `fed.limits.415c` | $72,000 | [N25-67] |
| `fed.limits.ira`; catch-up | $7,500; $1,100 | [IRS-LIM] |
| Roth IRA phase-out | $242,000-$252,000 MFJ; $153,000-$168,000 single/HOH; $0-$10,000 MFS | [IRS-LIM] |
| Traditional IRA deduction phase-out | $129,000-$149,000 MFJ covered; $242,000-$252,000 non-covered spouse; $81,000-$91,000 single | [IRS-LIM] |
| `fed.limits.hsa` self / family; catch-up 55+ | $4,400 / $8,750; $1,000 | [RP25-19] |
| Student-loan interest | $2,500, phase-out $175k-$205k MFJ ($85k-$100k single) | [RP25-32 via DEBT] |
| Car-loan interest (2025-2028) | up to $10,000, phase-out above $200k MFJ / $100k | [PL119-21] |

Phase-outs are linear: `allowed = limit x clamp((top - MAGI) / (top - bottom), 0, 1)`, rounded under the table's `RoundingRule` (the statutory rounding increments for IRA phase-outs were not in the research and go through the hand-verification gate). Limits are **per person**; the family HSA limit is shared between spouses; HSA limits prorate by eligible months (section 10).

### 3.7 Statutory rounding and uprating (data, reproduced by a Tier-1 fixture)

Rounding is meaningless without the amount it rounds. Every indexed table therefore carries a `projection` block (1.3) holding the **statutory base year, the base-year amounts per filing status, and the index series the statute names**, and the M0 vintage archives both the base-year amounts and the chained-CPI index series alongside the thresholds. `uprate(year) = round(base_value x index_year / index_base_year)` with the rule applied once; for `basis: IncreaseOverBase` the rounding applies once to `(uprated - base_value)`. The 2026 acceptance target is computed from the base-year amounts and the index series, **not** from 2025's already-rounded values plus a year of CPI; uprating is therefore **path-independent** (any route from the base year to year `t` gives the same answer) but **not associative under chaining**, and a negative test asserts that a year-over-year chain diverges from the statutory result on at least one 2026 threshold.

| Item | Rule | Source |
|---|---|---|
| Bracket thresholds | Increase over the statutory base year rounds **down to $50** (`basis: IncreaseOverBase`); $25 for the MFS table. Index `cpi.chained`; the base year is a **hand-verification-gate item read from 26 USC 1(f) before the M0 vintage locks** (the research states the rule, not the year) | 26 USC 1(f)(7) [USC1; DR C13]; base year: M0 gate |
| Standard deduction | Down to $50 on the increase over the **2024 post-OBBBA base**; index `cpi.chained` | 26 USC 63(c)(4) [USC63; G3, DR C13] |
| 415(c) / 415(b) | Down to $1,000 / $5,000 | [DR C13] |
| IRMAA thresholds | Nearest $1,000 | [DR C13] |
| PIA / payable benefit | Dime-truncated at each COLA / dollar-truncated | [DR 4.6] |
| 402(g), 219(b)(5), 414(v) increments | $500 steps **(unverified)**: hand-verification gate | [DR C13] |
| ACA MOOP | `base x PAPI` down to $50 | [G5] |

---

## 4. State tax plug-in

### 4.1 Interface (ADR-011)

```rust
pub trait StateTax: Sync {
  fn compute(&self, year: Year, status: FilingStatus, fed: &FederalReturn,
             inputs: &StateInputs, params: &ParamView) -> StateReturn;
  fn fidelity(&self) -> Fidelity;                 // Exact | NoIncomeTax | EffectiveRate
  fn validation_basis(&self) -> ValidationBasis;  // e.g. SingleOracle
}
struct StateInputs { state: StateCode, ages: SmallVec<[u8;2]>, ss_gross: Cents, retirement_income: Cents,
                     public_pension: Cents, lt_gains: Cents, user_overrides: StateOverrides }
struct StateReturn { lines: BTreeMap<LineId, Line>, total: Cents, fidelity: Fidelity,
                     basis: ValidationBasis, not_modelled: Vec<NotModelled> }
```

Static registry by state code; no dynamic loading. `NoIncomeTax` (AK, FL, NV, NH, SD, TN, TX, WY [TF26]) returns zero with `Fidelity::NoIncomeTax`. Washington is deliberately not in that list: it levies an excise tax on long-term capital gains, so it ships as `EffectiveRate` with a printed caveat naming that tax, or as a `DeclarativeState` module whose only rule is the gains-threshold rate (ADR-011; its 2026 deduction and top rate are M10 hand-verification items, `TESTING.md` §5.2). `DeclarativeState` (M10) interprets `params/states/<code>.toml`: brackets or flat rate, standard deduction/exemptions, conformity starting point (federal AGI or taxable income), Social Security exclusion flag, age-gated retirement-income exclusions with caps, gains treatment. User-confirmed retirement-deduction values are plan-level overrides [DR C9].

### 4.2 Effective-rate fallback (any state, M2)

```
EffectiveRate { rate: Ratio,                       // user-entered; no default value ships
                base: FederalAgi | FederalTaxableIncome,   // default FederalAgi
                exclude_social_security: bool,     // default true
                exclude_retirement_income: bool }  // default false
state_tax = max(0, base_amount - exclusions).mul_ratio(rate, HalfUp to the dollar)
```

The marginal state rate used in `r_u` and EMR is obtained the same way as any other: by running the module twice. Every result carries `Fidelity::EffectiveRate` and the UI prints it next to the number. Known bias, printed as an omission: a flat effective rate understates marginal rates in graduated states and misses retirement-income exclusions that drive Roth and relocation answers.

### 4.3 Federal-state ordering

State tax is an input to the federal itemized deduction, and the state module reads the federal return. Resolution, two passes: (1) `federal()` with `itemizable.state_income_tax = 0`; (2) `state.compute(fed1)`; (3) `federal()` with the computed state tax. Because AGI does not depend on itemized deductions, AGI-based states are at a fixed point after pass 3. Pass 3 is **skipped** when itemizing is impossible: `other_itemized + min(salt_cap, property_tax + state_tax) <= standard deduction` (the common case, and the one that keeps Monte Carlo cheap). States that deduct federal tax cannot be expressed declaratively and need a hand-written module with its own iteration.

---

## 5. The next-dollar engine

`next_dollar(plan, proj, year, surplus, policy) -> Vec<Recommendation>` (seam S7). Original work with no external oracle (R6): its controls are the reconciliation invariant, the properties in 5.8 and PLAN M2's Tier-1 synthetic cases.

### 5.1 Option set

```rust
enum NextDollarOption {
  CashBuffer, EmergencyFund,
  EmployerMatch{person, plan},
  DebtPrepay{debt},
  HsaPayroll{person}, HsaDirect{person},
  IraTraditional{person}, IraRoth{person}, BackdoorRoth{person},
  PlanPreTax{person, plan}, PlanRoth{person, plan},          // 401(k)/403(b)/457; catch-up is a sub-cap
  MegaBackdoor{person, plan},
  Plan529{goal},                                              // active at M10; shape reserved from M2
  Taxable{account}, TaxableSafe{account},                     // TaxableSafe: T-bill / HYSA sleeve (5.4); adopts as
                                                              // Taxable does, a PlannedContribution (DOMAIN-MODEL 12.1)
}
```

Each option exposes `cap(state) -> Cents`, `cost(d) -> Cents` (take-home dollars needed to put `d` into the option), `r_u(state) -> Ratio` with its `Line` trace, `liquidity_class`, and `eligibility -> Result<(), WhyNot>`. The `Line` trace exists only under `Full` (or `Summary`) trace, which is Exact mode; under `TraceLevel::None`, TierFill on Monte Carlo paths computes the same `r_u` **value** with an empty trace, and any card built from a path re-runs that path with `Full` from `(seed, path index)` to obtain the trace (ARCHITECTURE 4.2).

### 5.2 Working in take-home dollars

The surplus `S0` is computed with **no elective contributions for the remainder of the year**; in year 0 the year-to-date contributions already made are part of the base and are netted out of every cap (1.4), not zeroed. For a candidate contribution `d`, by running the functions twice:

```
tax_saved(d)     = Cost(base) - Cost(base + delta(d))                 // federal + state, marginal()
payroll_saved(d) = payroll(wages) - payroll(wages - d)                // only for payroll-routed HSA
cost(d)          = d - tax_saved(d) - payroll_saved(d)
t_m              = tax_saved(d) / d          payroll_saving = payroll_saved(d) / d
```

Tier-1 cases: `payroll_saving` = 7.65% below the $184,500 wage base and 1.45% above it, and **nothing else** — the 0.9% Additional Medicare surcharge is an `f8959` line inside `federal()` (3.2 line 6), so it arrives through `t_m`, and adding it here as well would count it twice in the HSA-payroll `r_u`. Because year 0 feeds `payroll()` the **full-year** wages (1.4), the wage-base position is the household's real one rather than an artefact of the month the plan was opened. A flat 7.65% is a **labelled fallback** used only when wages are missing.

### 5.3 The after-tax, risk-matched return `r_u`

One basis for every option, as capability (b) (`PLAN.md` §1.1) demands: the **after-tax, risk-matched terminal value of one take-home dollar at a common horizon `H`**, reported as an annualized rate `r_u(H) = TV^(1/H) - 1`.

**Why not a first-year equivalent.** Counting one-time tax effects in full in year one mixes units: a one-time uplift and an annual debt rate are not the same quantity, and ranking on their sum compares a 10-point uplift with a 10%/yr loan as if they were equal. The algebra was wrong as well: costs are measured in take-home dollars (5.2), so a take-home dollar buys `1/(1 - t_m)` of pre-tax contribution: the one-time gain from deferring at `t_m` and withdrawing at `t_future` is `(t_m - t_future)/(1 - t_m)`, not `t_m - t_future` — at `t_m` 32% and `t_future` 22% that is 14.7%, not 10%. The first-year decomposition is kept, but **as explanation only**, printed beside `r_u(H)` so a reader can see how much of the answer is a one-time tax effect and how much is compounding.

```
H     = policy-level horizon, one value for every option, printed on every card.
        Default = years to the first projected traditional withdrawal or RMD; editable.
r_rm  (risk-matched base, pre-tax)
      = max( y_bond_sleeve                      if the target allocation holds bonds
             E[r_equity] - ERP                  otherwise ,
             y_safe_at )                        # floor, always applied
y_safe_at = term-matched Treasury or HYSA yield from the vintage's market_data, after tax [DEBT Rule 1]
ERP   default = vintage-implied (E[r_equity] - y_10yr); the 5% Kitces setting is a named preset.
        Both figures are printed side by side; editable range 2-5% [K2012, DEBT]
```

**The floor is not cosmetic.** Without it, a synthetic vintage of US large arithmetic 6.5%, 7-10y Treasury 4.4% and cash 3.7% together with a 5% ERP gives `r_rm` = 1.5% nominal for a household holding no bonds — below the same vintage's own risk-free yields, on the same screen. Every debt above about 1.5% after tax would then outrank taxable investing, so a 2.75% mortgage gets prepaid ahead of a Treasury or HYSA paying 2.9-3.4% after tax that strictly dominates prepayment. The floor restores Rule 1 of the debt comparison (compare the after-tax debt rate with the term-matched Treasury yield after tax; [DEBT]) for households that hold no bond sleeve, and the vintage-implied ERP default (6.5 - 4.4 = 2.1%, or 6.5 - 3.7 = 2.8% against cash, on that synthetic vintage) stops the shipped default from contradicting the vintage printed beside it.

| Option | Terminal value of one take-home dollar at `H` |
|---|---|
| Employer match | `(1 + r_rm)^H x (1 + match_rate x vesting_discount) x (1 - t_future) / (1 - t_m)` (5.5). Own tier; first-year form `match_rate / (1 - t_m)` |
| Debt prepayment | `(1 + r_debt_at)^H`, `r_debt_at = APR x (1 - t_m x deductible_fraction)`, with `deductible_fraction` **computed** by `marginal(.., MortgageInterest \| StudentLoanInterest)` (see below). Annualized `r_u` is `r_debt_at` and so is H-invariant; a debt maturing before `H` reinvests its freed payment at `r_rm` for the remainder, consistent with 5.8's freed-minimum handling within a year. Tier-1 `deductible_fraction`: 0 for a standard-deduction household, 1 for one itemizing through the SALT cap. Labelled fallback 0 only when itemizable inputs are missing. PSLF-track loans: 0 with a pay-the-minimum constraint [DR 4.3] |
| HSA via payroll / direct | `(1 + r_rm)^H / (1 - t_m - payroll_saving)` / `(1 + r_rm)^H / (1 - t_m)`, `payroll_saving` from 5.2 |
| Pre-tax plan, Traditional IRA (deductible) | `(1 + r_rm)^H x (1 - t_future) / (1 - t_m)`; until M3 `t_future` is unavailable, so the card prints `r_u` at `t_future = t_m` (the Roth-equal case) and lists the omission |
| Roth plan, Roth IRA, backdoor, mega-backdoor | `(1 + r_rm)^H` (backdoor: less `prorata_cost`, 5.6) |
| 529 | `(1 + r_rm)^H x (1 + state_deduction_rate)` (zero under `EffectiveRate`), capped by the goal's unfunded amount |
| Taxable | `(1 + r_rm - drag)^H` less the terminal tax on the embedded gain at `t_m_gain`; `drag = yield_ord x t_m_ord + yield_qdi x t_m_pref + turnover x t_m_gain`, each marginal rate from `marginal()`; `drag_avoided` in the first-year decomposition is this same figure (research range about 0.3-1.0%/yr [DR 4.3]) |
| Taxable, safe sleeve (`TaxableSafe`) | `(1 + y_safe_at)^H`. The T-bill / HYSA option that makes the 8-10 block test honest: prepaying a low-rate debt must beat holding the safe asset, not merely beat equities-minus-ERP |
| Cash / emergency fund | Filled by rule, not ranked. Vehicle hint: `HYSA y x (1 - t_fed - t_state)` vs `T-bill y x (1 - t_fed)` |

`t_m` in these denominators is always the **computed** marginal rate from 5.2, so the Tier-1 cases assert the `(1 - t_m)` denominator explicitly rather than the difference form; any worked example printed on a card states the `t_m` it was computed at. The card also prints `r_expected(H)` (with `E[r]` in place of `r_rm`). Tax-advantaged space is flagged **use-it-or-lose-it** (annual room is perishable).

**`deductible_fraction` is marginal, not average.** `[Cost(without the interest) - Cost(with it)] / (t_m x interest)` averages over *all* the interest, but a prepayment removes the *last* interest dollars; for a household whose mortgage interest straddles the standard deduction the average understates the marginal fraction. The candidate prepayment therefore evaluates `marginal(.., MortgageInterest(-delta))` with `delta` = the interest that prepayment actually avoids. The whole-interest figure is retained for full-payoff comparisons, where it is the right one, and both are labelled.

### 5.4 Default tier table (`ContributionPolicy` as data)

A policy is `ContributionPolicy { rules: Vec<ContributionRule { id, tier_label, label, option, test, capacity, r_u, liquidity_class, enabled }>, thresholds, step_cents }` stored in the plan (`DOMAIN-MODEL.md` §11); presets are data files. **The list order is the tier order.** `tier_label` is a display label carried from the research's numbering and decides nothing: the table below is written in list order, which is why `1b` follows `3`, and consecutive rules sharing a label form one tier. The default is the research's reconciliation of the Bogle Center's ten tiers with three documented modifications [BOGLE, DR 4.3, DR A7]; the combination is the research's construct, each ingredient is sourced:

| Tier | Rule | Thresholds (all editable, all printed on the card) |
|---|---|---|
| 1a | Spending-shock cash buffer | `max($2,000, 0.5 x monthly essential)` in safe-liquid assets [V2023] |
| 2 | Employer match, to the full match | vesting discount; Saver's Match joins from tax years after 2026 only under its MAGI phase-out [USC6433] |
| 3 | High-interest debt, avalanche by `r_u` | `hurdle_high` = **fixed 8% after tax** [WCI]; deferred-interest promos become zero-by-date required payments |
| 1b | Income-shock emergency fund | `3 + factors` months, clamped 3-12 (+1 each: single income, dependents, variable income, specialized job, weak insurance, low borrowing capacity; -1 each: dual stable incomes, strong severance) [V2023]. Toggle: "full emergency fund first" |
| 4-7 | HSA; IRA; employer plans to the limit; mega-backdoor Roth | Bogleheads order by default, `r_u` printed; toggle "order by `r_u`" |
| 7b | 529 toward a funded education goal (M10) | Position is this specification's choice (after retirement space, following [WCI]'s ordering); editable |
| 8-10 | Medium debt, taxable (risky **and** safe sleeves), low debt, **as one block** | `hurdle_low = y_10yr_Treasury + 3 pp` [BOGLE]; `y_10yr` is a dated value in the assumption vintage, user-overridable (section 14, D1) |

**Debt hurdle rule inside the 8-10 block.** For each debt with `r_u < hurdle_high`: prepay ahead of taxable investing when `r_u(debt) > max(r_u(Taxable), r_u(TaxableSafe))`. Because `TaxableSafe` is in the block, the test expands to "after-tax debt rate exceeds the after-tax bond-sleeve yield" when bonds are held, and, when they are not, to "after-tax debt rate exceeds both `E[r_equity] - ERP` and the after-tax term-matched safe yield" — the second half being Rule 1 of the debt comparison [DEBT], which a bondless household would otherwise lose. Otherwise the tier position stands and the debt is paid at minimums. Tilts: favour prepayment when retirement is ten years away or less or the household marks itself risk-averse [ERN21]; **variable-rate debt is evaluated at its rate-shock scenario rate**, not today's. The card prints the tier label, the test result and every rate that entered it, so an override is always visible. Warnings: bonds held against a mortgage; a mortgage carried past the retirement date.

Property: **no debt whose after-tax rate is below the after-tax safe yield may outrank every taxable option.** This is the regression that the old `r_rm` definition failed.

**Presets.** Preset ids are descriptive; the methodology's publisher is named in the preset's provenance field, and third-party names imply no affiliation or endorsement (ADR-004). *Fixed-hurdle* (`fixed-hurdle-8-4`, methodology per [WCI]): debt at 8%+ first, 4-7% after retirement accounts and before taxable, under 4% last. *Age-indexed* (`age-indexed-hurdle`, methodology per [MG]): one cutoff by decade of age (20s > 6%, 30s > 5%, 40s > 4%) above which debt is tier 3; debts below it fall into the block test [WCI, MG]. Acceptance (PLAN M2), two cases. **(i)** The research's worked placement of a fixed 6% non-deductible loan (Treasury 4.25%, no bonds, `E[r_equity]` 6.7%, ERP 5%) yields: default -> prepay before taxable after tiers 1a-7 (label "low-interest", override shown); fixed-hurdle -> before taxable, no override; age-indexed 30s -> tier 3; age-indexed 20s -> block test, before taxable [DR A7]. The `r_rm` floor does not disturb this case: 6% after tax still clears the 4.25% Treasury after tax. **(ii)** The case the floor exists for: a 3% fixed mortgage, no bonds, Treasury 4.25%, same vintage. `r_debt_at` is below `y_safe_at`, so `TaxableSafe` outranks prepayment and the debt stays at minimums — the answer the old definition inverted.

**Liquidity classes.** Tiers 1a/1b are measured against classes, not totals: *safe-liquid* (checking, money-market, T-bills) counts in full; *liquid-risky* (taxable investments, Roth contribution basis) counts toward 1b at a haircut (`liquidity.riskyHaircut`, research range 70-80%); *illiquid before 59.5* (pre-tax plans, HSA without banked receipts) counts zero [DR 4.3].

### 5.5 Employer match and vesting

The fields below are read by the spellings `DOMAIN-MODEL.md` §5 declares (the `lint:schema-identifiers` check of `TESTING.md` §9 resolves every stored-field identifier in this section, in 11.1, and in SIMULATION-SPEC 14.1, 15 and 16.3 against the published `plan.schema.json`):

```
EmployerPlan { match_formula: [MatchTier { employee_pct: Ratio, employer_pct: Ratio }],   // ordered tiers
               match_cap: Option<MatchCap { PctOfComp(Ratio) | Dollars(Cents) }>,
               true_up: bool, per_pay_period_match: bool,
               vesting: Immediate | Cliff { years } | Graded { pct_by_year: [Ratio] } }
match_dollars(deferral, pay) = min( sum over tiers of employer_pct x min(deferral_in_band, employee_pct x pay),
                                    match_cap resolved against pay )
vesting_discount = vested fraction at the person's expected_separation (plan fact, DOMAIN-MODEL 4; 1.0 when
                   already vested or when no separation date is entered, with a caveat)
capacity = deferral needed to reach the last tier's employee_pct x pay, minus ytd.contributions[account] for
           that plan's account (1.4)
```

Without `true_up`, a plan with `per_pay_period_match` loses match when the limit is reached early: the engine emits an `ActionItem` stating the per-period deferral percentage that keeps every period matched. Employer contributions count against 415(c) room (5.7).

### 5.6 Backdoor Roth and Form 8606 pro-rata

Offered when the direct Roth IRA cap phases to zero (3.6). Per person (IRAs are never aggregated across spouses):

```
f8606.l14 basis        = prior basis + this year's nondeductible contribution
f8606.denominator      = year-end value of all traditional, SEP and SIMPLE IRAs + distributions + conversions
ratio                  = basis / denominator                    (decimal rounded per the form's instruction)
nontaxable             = ratio x (distributions + conversions);  taxable = converted - nontaxable
prorata_cost           = taxable x (t_m - t_future*) / contribution   (BETR-adjusted, section 7)
```

**The pro-rata tax is a forced conversion, not a deadweight loss.** Charging the full `taxable x t_m` treats money that has merely moved from a pre-tax bucket to a Roth one as if it had been burned; in fact that slice is a Roth conversion, which extinguishes a future liability at `t_future`, and the unrecovered basis is not lost either — it carries forward on Form 8606 line 14. The backdoor is therefore evaluated as "a nondeductible contribution **plus** a forced conversion of the taxable slice" using the sections 6 and 7 machinery: the cost is the rate **difference** `t_m - t_future*`, where `t_future*` carries section 7's BETR adjustment for tax paid out of taxable cash. Charging the full `t_m` makes the backdoor look far worse than it is for any household holding a rollover IRA and can rank it below plain taxable investing. The card shows the carried basis alongside the cost.

Tier-1: basis 10,000 / value 90,000 / conversion 20,000 -> ratio 0.091, nontaxable $1,820 [F8606; DR 4.9]. When pre-tax IRA balances exist the card adds the caveat and, if `plan.accepts_roll_in`, an alternative: roll the pre-tax balance into the employer plan first. `BackdoorRoth` writes `ira_taxable_override` into `TaxInputs`.

### 5.7 Mega-backdoor Roth

```
eligible  = plan.after_tax_contributions_allowed
            && ( plan.in_plan_roth_conversion
                 || plan.in_service_withdrawal_age.is_some_and(|age| person_age >= age) )
room      = limit_415c - elective deferrals (excluding catch-up) - employer contributions   // at most $47,500 in 2026 (derived)
            both terms include year-to-date amounts (ytd.contributions, ytd.employer_contrib) in year 0
```

`in_service_withdrawal_age` is an `Option<u8>` in the schema, not a flag: `None` means no in-service path at all, and `Some(age)` opens the withdraw-and-roll route only once the person has reached that age, which is why eligibility is evaluated per year rather than read as a plan constant.
[N25-67; the derivation is labelled **(unverified)** — the component limits are primary-sourced but the worked number is not — and the catch-up exclusion is confirmed at the hand-verification gate.] If after-tax contributions are allowed but no conversion path exists, the option is returned as an `alternative` with `whyNot: NoConversionPath`. Plan features are facts collected at setup; the engine never assumes them.

### 5.8 The greedy loop (Exact mode)

```
fn next_dollar(plan, proj, year, surplus S0, policy):
  state <- base with no FURTHER elective contributions this year (year 0: ytd contributions stand,
           caps are already net of them, 1.4); remaining <- S0; out <- []
  loop while remaining > 0:
    tier  <- first tier in policy LIST ORDER having an eligible option with cap > 0   # never by tier_label (5.4)
    cands <- options in tier                      # default: policy order; "order by r_u": sort desc, ties by id
    opt   <- first(cands)
    d     <- largest d such that cost(d) <= min(remaining, policy.stepCents) and d <= opt.cap(state)
    apply d to state (caps deplete; debt payoff frees its minimum payment; tax base moves)
    remaining -= cost(d);  merge (opt, d) into out
    recompute t_m, r_u, eligibility (phase-outs move with MAGI)
  final step: d is solved so that remaining == 0 exactly; any unallocatable residue goes to Taxable or cash
  emit Recommendation per option with Explanation (section 12)
```

Constraints enforced on every step: statutory caps per person, **net of year-to-date contributions in year 0**; shared family HSA cap; HSA eligible months; 415(c) including employer money and year-to-date employer money; Roth-only catch-up above the wage threshold; IRA phase-outs at the *current* MAGI (pre-tax deferrals lower MAGI, so eligibility is re-evaluated each step); plan feature flags; PSLF-track loans pinned to minimums; promo balances as required payments; liquidity floors; no negative cash. `policy.stepCents` is data (default in the default-policy file, not a code literal). `PlanPreTax` and `PlanRoth` are never ranked against each other: the loop fills the employer plan's room as one option and divides it by the stored `deferral_type` split (2.3), so no tie-break can silently decide Roth versus Traditional.

Properties (PLAN M2): allocations sum to `S0`; no cap exceeded; raising a debt's APR never lowers its rank; no debt whose after-tax rate is below the after-tax safe yield outranks every taxable option (5.4); current-year conservation residual is zero; the tax cells sum to their total exactly once (2.2); the reconciliation invariant holds for every `Reason`; the same household at `asOf` January and `asOf` October produces the same `t_now` and the same option eligibility (1.4).

**Distributional check (M6).** The recommended allocation and its best alternative run through `simulate(&[planA, planB])` on one shock tensor; the card adds 5th/50th/95th percentile net worth for each. Until M6 the card lists this under `omissions`.

---

## 6. Roth vs Traditional: the tax function run twice

```
t_now    = marginal(base_t, PreTaxDeferral{person, x}).block        x = the contribution under consideration
t_future = future_marginal_rate(plan, proj, weighting)              ONE estimator, defined below
   Cost in both is section 3.3's Cost: federal total.* + state at declared fidelity
           + PV of the IRMAA change in y+2 (+ lost PTC from M8).  NIIT and Additional Medicare are
           inside total.* and are not added again.
```

**One future-rate estimator, cited by every card.** `future_marginal_rate(plan, proj, weighting)` is computed **once per projection** and exposed as a named `Line`, so the Roth-vs-Traditional card and the conversion planner (section 7) both cite the same `LedgerRef` and cannot disagree on the same plan and year:

```
future_marginal_rate(plan, proj, weighting) = aggregate_y  marginal(base_y, IraDistribution(x_y)).block
   weighting = { window, aggregate }                                   printed on the assumptions sheet
   default window   = W: the first projected traditional withdrawal or RMD to the plan end, INCLUDING
                      survivor years under single-filer tables after the assumed first death
   default aggregate = withdrawal-weighted mean, w_y = projected traditional withdrawals in y / their sum
                      over the window (the marginal dollar is assumed to leave on the same schedule as
                      the baseline's traditional dollars)
   x_y = x.grow(I_y / I_t)                                             (same real size)
```

Any caller needing a different view — section 7's planner uses an unweighted median over ages 75-90 — passes a different `weighting` to the **same** estimator and the card prints which one it used. Two unrelated formulas both called "the household's projected future marginal rate" is exactly the black-box behaviour ADR-016 exists to eliminate, and capability (c) (`PLAN.md` §1.1) names a single mechanism.

The future base is the ledger's own: pensions, the taxable share of Social Security, interest and dividends, projected RMDs, planned withdrawals, indexed brackets [DR 4.3]. Bracket labels mislead: a published "28% bracket" couple's true marginal rate was 32.36% [K2014].

**Flip value** (by bisection over `t_future`, reported on the card): not filling the limit -> `t_future* = t_now`. Filling the limit (a Roth dollar shelters more, the Traditional tax saving is invested in taxable) -> `t_future* = t_now x G_tax / G_ira`, with `G_tax = (1 + r - drag)^H`, `G_ira = (1 + r)^H`: the same closed form as the break-even tax rate in section 7.

**Card by milestone (ADR-019).** M2: `t_now`, the flip value, **no verdict**, no slider presented as an answer. M3: verdict in `{Traditional, Roth, Sensitive}`:

```
pair = (t_future without survivor years, t_future with survivor years)
bounds = pair re-run at survivor spending multipliers 0.60 and 0.74
guard:  if the assumed survivor window is shorter than policies.roth_verdict.min_survivor_years
        (DOMAIN-MODEL 11; default 5), withhold the verdict as `Sensitive` with the reason printed
Traditional if flip > max(pair, bounds);  Roth if flip < min(pair, bounds);  otherwise Sensitive
```

The guard matters because the whole verdict rests on the survivor years: ADR-019 gates the M3 verdict on them precisely because a future-rate estimate that omits them "is biased low in a knowable direction, which biases the recommendation toward Traditional invisibly". If the assumed first death sits at or near the plan end, the two elements of `pair` are near-identical, the bounds barely move, and the card would display a sensitivity comparison asserting robustness while shipping the bias. Computing a verdict from an uninformative pair is worse than withholding it, so below the minimum the engine prints the reason instead of a verdict. 11.1's default first-death convention is what keeps the window informative.

`Sensitive` prints the tie-breakers instead of a verdict: tax-bucket diversification; Roth when already filling limits; Traditional when the working-years state rate is high and the retirement state's is low [DR 4.3]. Printed omissions: ACA credits until M8; state at its declared fidelity. M4 (full first-death machine) and M5 (computed PIA) change inputs, not the mechanism.

**This card, and only this card, recommends changing the stored `deferral_type` split** (2.3). The next-dollar card recommends an amount into the employer plan; the split it is divided by is the stored decision. Adopting a new split re-projects once and recomputes the verdict on the new baseline, and the card states which pass it is showing.

---

## 7. Roth-conversion planning (M8)

**Primitive (fill to threshold) is a monotone root-find, never a subtraction.** `conv_t` = the largest `c >= 0` such that `measure(base_t + c) <= threshold_t`, found by bisection on `federal()` (the measure is monotone non-decreasing in `c`, so bisection is exact to the cent and terminates):

```
conv_t = max { c >= 0 : measure(base_t + c) <= threshold_t }        # bisection, not threshold - measure
```

Computing it as a difference assumes each converted dollar moves the measure by exactly one dollar, which is false in precisely the situations the feature exists for. Inside the Social Security phase-in a converted dollar adds up to $1.85 of taxable income; the senior-deduction phase-out adds another 6-12% on top (3.3); and a conversion pushes QDI and long-term gains across the 0% and 15% boundaries. A "fill the 12% bracket" computed as a difference therefore overfills by up to 85% and lands the household in the 40.7% torpedo zone the feature was meant to avoid. Tier-1 case: a mid-torpedo household filling to the 12% bracket top ends with ordinary taxable income **exactly at** the bracket top, not above it.

| Threshold kind | Measure |
|---|---|
| `BracketTop(rate)` | ordinary taxable income |
| `LtcgBoundary(Zero \| Fifteen)` | taxable income |
| `IrmaaTier(k)` | `magi.irmaa`, applies to conversions from the year two before any person's Medicare start (age 63 for entry at 65) |
| `FplMultiple(k)` | `magi.aca`, applies in years any person is on Marketplace coverage; 400% is the cliff [G5] |

When several apply the binding constraint is the minimum. The ACA and IRMAA constraints conflict across the two years before Medicare; the planner reports both candidate fills and their lifetime cost difference rather than silently picking one.

**Default planner: tax-equilibrium loop** [TAXD C]:

```
(a) baseline projection with zero conversions
(b) future_rate = future_marginal_rate(plan, proj, weighting{window: ages 75-90, aggregate: median})
    -- section 6's single estimator with the planner's own printed weighting, using single-filer tables
    from the assumed first-death year + 1 onward; the Roth card and this planner cite the same Line
(c) for each pre-RMD year: binary-search c such that block_rate(c) <= future_rate and no chosen cliff is crossed
(d) re-project (conversions shrink RMDs); repeat (b)-(d) 3-5 times or until max |delta c| < policy.stepCents
(e) compare policies {none, fill 12%, fill 22%, fill 24%, fill to IRMAA tier 1, equilibrium} on lifetime taxes
    and after-tax terminal wealth, under Monte Carlo on common random numbers
```

Rules: conversion tax is paid from taxable cash (if none, the conversion is reduced, never grossed up from the IRA before 59.5); each conversion starts its own five-year clock for the under-59.5 penalty and enters `RothLedger.conversions` FIFO; harvest 0% gains first, `harvest = min(unrealized, max(0, Z - TI_before))`; conversions are ordinary income, not earned income and not NII. `after_tax_terminal_wealth = taxable (stepped up) + Roth + traditional x (1 - heirs_rate)`, `heirs_rate` default 24%, editable [DR A6]; without the haircut no-conversion strategies look falsely good. **Survivor years:** after any `death(P, t)` the planner is re-run on the survivor's single brackets (section 11, step 10).

**Closed-form check (Tier 3).** `BETR = t_now x G_tax / G_ira` reproduces the published 35% / 30.1% / 23.5% / 14.1% by tax-payment source [V-BETR]. Two caveats carried into the explanation: the paper assumes no capital-gains tax when taxable assets are liquidated to pay the tax (the engine adds the realized-gain drag), and with IRA basis fraction `b` the rate on the taxed portion is `t_now x (1 - b)`. Owl example cases are recorded out-of-process **bounds**, not targets.

---

## 8. Debt engine; debt vs invest at equal net worth

**Monthly sub-engine (M3).** Per debt per month: `interest = balance.mul_ratio(APR/12)`; required payment by rule (`Fixed`, `max(pct x balance, floor)`, or level `PMT = P x i / (1 - (1+i)^-n)`); extra budget plus freed minimums go to ordered targets. Orders (M4): avalanche (after-tax APR descending, default), snowball (balance ascending), hybrid; non-default orders print their extra interest and months. Promo balances are forced to zero by `promoEnd`. Refinance is retire-old / open-new with `NPV(H)` over the expected holding period at the after-tax opportunity rate and a "new loan at the old payment" variant to neutralize the term reset [DEBT F]. Federal student-loan plan rules (RAP: 1-10% of AGI by $10,000 band, $10 floor, forgiveness at 360 payments, PSLF 120 [PL119-21]) are versioned parameters; PSLF-track loans have zero prepayment return. Acceptance: amortization equals closed-form PMT/IPMT/PPMT.

**Point rule.** Section 5.4's hurdle test.

**Equal-net-worth test (M4 deterministic, M6 stochastic).** Comparing at unequal net worth biases toward keeping the loan [G1]. Construct two `hypotheticalFacts` scenarios from the same base:

```
A (paid off): portfolio P - L, no loan          B (keep): portfolio P, loan L at r_m, fixed nominal P&I
   where L is drawn from the accounts the WithdrawalPolicy would actually liquidate (tax on that
   liquidation is booked in A, so the two start at equal AFTER-TAX net worth)
simulate(&[A, B]) on one shock tensor -> fail-safe spending, failure probability with max_cut, median
   terminal wealth, 5th/50th/95th net worth
```

The explanation flags four exceptions to paying off: embedded gains on the assets to be liquidated, money locked in tax-deferred accounts, bond yields above the loan rate, an expected sharp rate rise [ERN21]. Expected direction (not a regression target): at $1.0M net worth, 80/20 without a mortgage shows fail-safe 3.14% / median 5.31%; 100% stocks holding $1.2M against a $200k 3.875% mortgage shows 2.27% / 6.23% [ERN21]. Leverage raises the median and lowers the floor.

---

## 9. Property (1.1 by decision — `PLAN.md` M10 and §5; schema shape reserved in M2)

Sections 9, 10 and 11.2 specify the **life modules**, which `PLAN.md` moved out of M10 into 1.1 by decision rather than leaving them to the cut rule: each is a milestone's worth of work. They are specified now so that the reserved shapes in seam S5 (`properties{}`, `insurance{}`, `Person.coverage`) are the right shapes and 1.1 is additive. Nothing in this section is a 1.0 gate.

```
Property { id, kind: primary|second|rental|land, owners, acquired,
  value0, appreciation{meanNominal, sd, model}, 
  basis{purchasePrice, closingCosts, improvements[{date, amount, capitalize}], landPct,
        placedInService?, accumulatedDepreciation, conversion?{date, fmv}},
  loans[DebtId], carrying{propertyTax{mode: pctOfAssessed|amount, rate,
        assessedGrowth: market|cap(Ratio)|inflation, reassessOnSale}, insurance{amount, growth},
        maintenancePct, capexReservePct, hoa, utilities},
  rental?{grossRent, rentGrowth, vacancy, mgmtPct, otherOpex[], activeParticipation, suspendedLosses},
  residency[{from, to, use: residence|rental|vacant}],            // drives the Section 121 tests
  events[sell|downsize|buy|convertToRental|convertToPrimary|refinance|payoff|inheritStepUp],
  treatment{fundedRatio: exclude|eventOnly} }
```

Defaults ship with provenance and are overridable: nominal appreciation 4.3-4.4% with year-over-year SD 5.80% (about 1.5% real); rent growth CPI + 0.5 pp; selling cost about 8%, buying about 2% (medium confidence); maintenance plus capex about 1% of value (derived); insurance CPI + 1-2 pp [G1, FRED-CS].

**Annual loop.** `V_t = V_{t-1}.grow(1+g_t)`; assessed value per rule; `tax = rate x assessed`; carrying costs are **essential** outflows; capitalized improvements add to basis; loans amortize in the debt sub-engine. Home equity is in net worth and **never** in the withdrawal denominator or the essential funded ratio; it enters only through scheduled events [G1].

**Sale.** `amountRealized = price - sellingCosts`; `adjBasis = purchase + closing + capitalized improvements - depreciation allowed or allowable`; `gain = amountRealized - adjBasis`. Section 121: eligible when owned **and** used as a residence for at least 24 of the last 60 months and no exclusion in the prior 24 months; `exclusion = min( (gain - post-1997 depreciation) x (1 - nonqualifiedUseFraction), $250,000 | $500,000 MFJ )`; partial exclusion `= min(own, use, sinceLast days) / 730 x limit` for qualifying moves [P523]. **The order of the two operations is load-bearing.** Under 26 USC 121(b)(5) the gain allocated to nonqualified use is simply *ineligible*, and the dollar limit then applies to what remains; applying the limit first and the fraction second shrinks an exclusion that the statute would have left whole. Worked example: gain $800,000, MFJ, `nonqualifiedUseFraction` 0.2 -> eligible gain $640,000, exclusion **$500,000**. Taking the limit first gives $400,000 and overstates taxable gain by $100,000, with NIIT and an IRMAA lookback spike following it (`DECISIONS.md` C4; [USC121]); a Pub 523 worksheet fixture with gain above the limit **and** nonqualified use is added, verified at the hand-verification gate. Tax stack in order: unrecaptured Section 1250 gain `min(depreciation, gain)` at up to 25%, remaining LTCG through section 3.2 stacking, NIIT on gain above the exclusion, state. The gain flows through AGI, so Social Security taxability and the IRMAA lookback two years later see it; the card warns about the sale-year IRMAA cliff. Conversion to rental depreciates the lesser of FMV or adjusted basis.

**Rental.** Cash flow and taxable income are separate: `NOI = rent x (1 - vacancy) - opex`; `depreciation = nonLandBasis / 27.5`, mid-month first year `basis/27.5 x (12.5 - month)/12` [P946]; passive-loss allowance $25,000 phased out at 50 cents per dollar of MAGI from $100,000 to $150,000, remainder suspended and released on disposition [P925]. Until the allowance logic ships, non-zero rental losses raise `NotModelled(PassiveLoss)`.

**Rent vs buy.** User cost `u = r_f + w - tau x (r_m + w) + delta - g + gamma` with `tau = 0` unless the itemizing test says otherwise (computed, as in 5.3) [NYFED]; plus a horizon NPV reporting break-even years and, from M6, P(owning wins by year H). Section 1031 exchanges and HECM tables are out of scope (PLAN M10 Scope OUT).

---

## 10. Healthcare coverage state machine (1.1; the HSA limit rules below are M2, IRMAA M3, PTC M8)

One machine **per person**, stepped monthly at switch dates, sharing one household MAGI:

```
states: Employer | SpouseEmployer | Cobra | Marketplace{metal} | RetireeMedical
      | MedicareOriginal{medigap, partD} | MedicareAdvantage | Uninsured(warning)
transitions:
  employment ends        -> Cobra | Marketplace | SpouseEmployer      (scenario choice)
  Cobra, month 18        -> Marketplace (29 with disability, 36 other qualifying events)         [DOL]
  65th-birthday month    -> Medicare*, each person on their own date; the younger person stays on Marketplace
                            with a one-person coverage family and an unchanged FPL household size
  employer coverage ends after 65 -> Part B special enrolment within 8 months; COBRA and retiree coverage
                            do not extend it; late enrolment adds the permanent penalty line           [G5]
```

Costs: `Cobra = 1.02 x employer total premium`; Marketplace `member = rate21 x age_factor(age at effective date, state) x tobacco_factor`, benchmark from a user-entered county second-lowest-silver rate, fallback national benchmark $625/month at age 40 [KFF]; federal age curve 1.000 at 21-24, 1.278 at 40, 1.786 at 50, 2.714 at 60, 3.000 at 64+ [CMS-AGE]; PTC per section 3.4 with estimate-then-reconcile; expected out-of-pocket `min(MOOP, (1 - AV) x expected claims)`, 2026 MOOP $10,600 / $21,200 [HCGOV] (the catastrophic-year probability default is the research's own **unsourced** assumption and is labelled as such). Medicare stack per person: Part B ($202.90/month in 2026) + Part D + IRMAA lookup + Medigap (rating method is an input) or Advantage + uncovered care [CMS]. **Component inflation**, never one scalar: private premiums about 5%, Part B/D about 6% to 2031, Medigap 4.8%, out-of-pocket care CPI + 0.5-1.0 pt [G5].

**HSA rules.** Own account type. Eligible month = covered by an HDHP, or from 2026 enrolled in any Marketplace bronze or catastrophic plan, and not on Medicare; limit prorated by eligible months; eligibility ends six months before a post-65 Medicare application. Qualified for Medicare Part B/D/Advantage premiums and IRMAA after 65, not for Medigap or pre-65 Marketplace premiums; qualified withdrawals are capped at cumulative qualified expenses plus the banked-receipt ledger; after 65 other withdrawals are ordinary income [P969, IRS-HSA]. The policy layer (enhanced credits, stayed Marketplace provisions) is dated, switchable data.

---

## 11. First-death state machine and insurance sizing

### 11.1 `death(P, t)` event bundle

One function mutates a cloned, resolved scenario; it is invoked identically by deterministic sweeps, `hypotheticalFacts` scenarios and stochastic-mortality paths. Deaths occur at the end of year `t`.

| # | When | Effect | Version |
|---|---|---|---|
| 1 | `t` | Face amounts in force on P (individual and group) arrive tax-free in the survivor's taxable account; P's premiums stop | M4 (facts), 1.1 (sizing) |
| 2 | `t+1` | P's wages, deferrals and match end; employer benefits end; if P carried the household's coverage, a survivor health-premium line is added | M3 |
| 3 | `t+1` | P's own Social Security stops; the survivor stream is computed by `pfp-ss` under the **three-branch rule of SIMULATION-SPEC 14.3** — base by what the decedent had done ((a) filed before FRA: RIB-LIM `min(PIA x age_factor, max(deceased_reduced_benefit, 0.825 x PIA))`, where 0.825 is a **limit**, not a floor, so a 60-year-old survivor of an early claimer receives 71.5% of PIA; (b) filed at or after FRA: the actual benefit including delayed credits; (c) died unfiled), then the survivor's own age reduction against `ss.fra.survivor` — paid as `max(own, survivor)` [POMS]. Until M5 the decedent's entered `SsEstimateEntry`, resolved to a PIA, stands in for the record | M3 (estimate), M5 |
| 4 | `t+1` | P's pensions and annuities x their stream's `survivor_fraction` (`DOMAIN-MODEL.md` §7; 0 = life-only) | M3 |
| 5 | `t` | Accounts: traditional/Roth/HSA pass `beneficiary.spouseFraction` to the survivor (treated as own; RMDs on the survivor's age), remainder exits as a bequest valued at `1 - heirs_rate`; taxable re-owns with `stepUp: none \| half \| full` (full in community-property states, half for spousal joint tenancy in common-law states [P551]); joint accounts re-own with no tax event. P's RMD for year `t` is still taken. Inherited-IRA branches (remain beneficiary under 59.5; Uniform-Table election) are limited to spousal cases | M3 (balances pass whole), M4 (fractions, step-up, branches) |
| 6 | **`t+1`** | Filing status: MFJ through year `t` [P501]; then QSS for `t+1..t+2` **only** with a dependent child and an unmarried survivor, else Single (HOH with another dependent). Standard deduction, brackets, LTCG boundaries, IRMAA tiers, Social Security bases, NIIT threshold and SALT cap switch together because they are all keyed by status. A test asserts that flipping **in** year `t` fails | M3 (Single), M4 (QSS/HOH) |
| 7 | `t+1`.. | Spending x per-category survivor multiplier: default **0.70** core; **1.0** housing, dependents, education; explicit lines for services the deceased provided. 0.60 and 0.74 are shown as sensitivity bounds. No direct empirical anchor exists; it is a first-class user input [DR A10] | M3 |
| 8 | `t+1`, `t+2` | IRMAA lookback reads the **joint** MAGI of `t-1` and `t` **against the MFJ tiers**, because SSA uses the filing status of the lookback year's return, not the survivor's current status (3.4). Death of a spouse is an SSA-44 life-changing event, so the `ssa44` toggle defaults **on** at this event and substitutes the current year's MAGI and status; both the default and its effect are printed. Medicare premiums drop to one person | M4 |
| 9 | `t` | Optional debt payoff from proceeds (toggle) | M4 |
| 10 | `t+1`.. | Roth-conversion planner re-run on single brackets | M8 |

**The assumed first-death year (M3) and the plan end are two different conventions.** Planning age is a single household-level value (SIMULATION-SPEC 6: longest life expectancy + 8, stored once on `Household`), so it can only define **plan end** — using it for first death as well would place the death at or near the plan end and leave a same-age couple with roughly zero survivor years, which silently disables the survivor-year pair in section 6, the single-filer `future_rate` in section 7 and the widow's-penalty logic in rows 6-8 above.

In M3 the assumed first death is the **median joint-first-death year of the two persons under each person's resolved mortality table** — `Person.mortality_table_id` if set, else the assumption set's `mortality_table_id` (`DOMAIN-MODEL.md` §4, §15); the set's field is not optional, so a table always resolves and there is no "no table selected" branch (SIMULATION-SPEC 6 uses the same resolution). It is editable, and it is printed on the Roth card and the assumptions sheet beside the plan-end convention. Under the default table this leaves a same-age couple with a substantial survivor window — the regression asserts at least ten survivor years — rather than the zero the old wording produced. Section 6's minimum-survivor-year guard catches whatever the convention leaves too short. Section 11.2's criterion likewise runs to the **household** planning age; there is no per-person planning age, and no per-person field is introduced for one. From M7 the death year is sampled per path.

### 11.2 Insurance sizing inside the ledger (1.1, with the `disable` bundle)

```
for P in persons, for t in event years, for kind in {death, disable}:
   s  <- clone(baseline) + bundle(kind, P, t) + coverage(F)
   F*(P,t) = min F such that criterion(project(s)) holds        // bisection on F, about 12 steps
criterion default: the survivor's plan never falls below zero to the household planning age (11.1)
                   (alternatives: goals funded; ends with at least $1)
```

Grid rows {A dies, B dies, A disabled, B disabled} x event year; about `4 x 30 x 12 = 1,440` deterministic runs per refresh, gate under 5 s (`trace: None`, parallel over cells) [G6]. Output: pass/fail grid against in-force coverage, `F*` curve, `max_t F*` as suggested term and ladder, and the same grid at multipliers 0.60 / 0.74. Property: `F*` never rises when assets rise. The grid is **not** probability-weighted. `disable(P, t)`: wages stop; group LTD `min(pct x salary, cap)` after the elimination period, taxable when employer-paid, own-occupation to any-occupation toggle at 24 months, SSDI offset; individual DI tax-free when premiums are after-tax; SSDI is a toggle, never base case; deferrals and match stop [G6]. Group-plan specifics are user facts (the typical design rests on secondary sources). Rules of thumb appear as labelled sanity panels only; human capital is a hedging quantity, never spendable net worth.

---

## 12. Explanation output (contract for every module above)

Every decision function returns `Recommendation{id, kind, scenarioId, year, action, dollars, tier, r_u, explanation, pins, basis, status}` (ADR-016). Engine-side requirements:

| Field | Rule |
|---|---|
| `because[]` | `Reason{templateKey, values: [Bound{name, value, ref}]}` — **the reference travels with the value**, one `ref: LedgerRef \| TaxLineRef \| ParamRef \| AssumptionRef` per `Bound` (`ARCHITECTURE.md` 4.2). Reconciliation invariant, property-tested over all personas, in three parts: **agreement** (every `bound.value` equals the cell its own `ref` resolves to in the same projection), **coverage** (every placeholder in the template named by `templateKey` has a `Bound` of that name) and **reachability** (every `Bound` is rendered by its template; an unused bound fails as a missing one does). Template keys are an enum; the UI owns the wording, never the numbers |
| `thresholds[]` | Every threshold in force: `hurdle_high`, `hurdle_low` and its Treasury input with as-of date, ERP, emergency-fund months, liquidity haircut, step size |
| `alternatives[]` | `{option, r_u, whyNot}` for the next-best option in the tier and for every ineligible option (`OverIncomeLimit`, `NoConversionPath`, `PlanFlagMissing`, `PslfTrack`, `CapReached`) |
| `flip[]` | By bisection on one named input with all else fixed, **always on the ranked quantity `r_u(H)` of 5.3**, never on a first-year decomposition: `t_future` (Roth vs Traditional), `ERP`, `E[r_equity]` and the safe-yield floor (debt vs taxable), debt APR, `hurdle_high`, `H` itself. `{input, currentValue, flipValue}`; `None` when no reversal exists in the input's valid range. Section 6's closed forms (`t_future* = t_now`; `t_future* = t_now x G_tax / G_ira`) are unchanged by the horizon basis and remain the checkable case |
| `caveats[]`, `omissions[]` | Fallbacks used (flat 7.65%, `deductible_fraction` 0), `NotModelled` flags from the tax return, state fidelity, modules not yet shipped (distributional check, PTC) |
| `basis` | `Tier1Exact \| OracleChecked \| SingleOracle \| PropertyTestedOnly`; next-dollar, ledger and sizing results are `PropertyTestedOnly` on top of a `Tier1Exact`/`OracleChecked` tax layer, and say so |

Template keys are namespaced by module: `nd.*` (next-dollar), `rvt.*` (Roth vs Traditional), `conv.*`, `debt.*`, `ins.*`.

---

## 13. Validation hooks

Acceptance values are those in `PLAN.md` per milestone; this table only states which control carries each section.

| Section | Primary control | External oracle |
|---|---|---|
| 3 | Tier 1 line-for-line (Pub 915 including the student-loan add-back, Pub 590-B, Form 8960, rounding and uprating table with the chained-vs-base-year negative test, 2026 grid, computed EMR shapes of 3.3); `TraceLevel::None` equals `Full` to the cent on every typed accessor; tax monotone; mutation budget | Tax-Calculator recorded grid, within $5 |
| 2, 8 | Conservation residual 0; tax cells sum to their total exactly once (2.2); `asOf` January equals `asOf` October for `t_now` (1.4); metamorphic properties; hand-worked ledgers; PMT closed form; persona goldens | none exists (R6) |
| 4 | Fidelity and validation basis printed on every result | PolicyEngine-US recorded, single-oracle (M10) |
| 5, 6 | Tier-1 synthetic cases (5.2, 5.3 with the `(1 - t_m)` denominator asserted, 5.6, 5.7, 5.4's two loan placements); properties in 5.8; reconciliation invariant | none (R6) |
| 7 | Form 8962 examples; BETR closed form; fill-to-threshold lands **exactly at** the threshold for a mid-torpedo household | Owl cases as recorded bounds |
| 11.1 (M3-M4) | "flip in year `t` fails"; the survivor-year IRMAA case (joint lookback MAGI charged against **MFJ** tiers, run with and without `ssa44`, both asserted against the published tables); a same-age couple has at least ten survivor years under the default table; `EarlyAccessRule::Penalized` books the `f5329` line and a SHORTFALL year without it prints the same figure as the alternative | none |
| 9, 10, 11.2 (1.1) | Pub 523 examples **including gain above the limit with nonqualified use** (correction C4); ACA cliff reconciliation case; grid monotonicity and the 5 s gate — these travel with the life modules to 1.1 (`PLAN.md` M10) | none |

---

## 14. Deviations and flagged concerns

**Deviations from the spine: none.** Signatures, seams, milestone placement and crate boundaries follow `PLAN.md`, `ARCHITECTURE.md` and `DECISIONS.md`.

**Three artefacts this document specifies are also owned by the spine, which carries the same text.** (i) The tax signatures carry `trace: TraceLevel` (3.1), so seam S3 freezes at M1 **with** the performance escape hatch; `ARCHITECTURE.md` §4.1 and ADR-007's ladder carry the matching text. (ii) Parameter tables carry the `projection{rule, index, index_series, base_year, base_values, rounding}` block (1.3, 3.7) from the moment seam S1 freezes at M0, because the uprating rule M0 accepts cannot be expressed without the statutory base-year amounts; M0 scope includes the base-year amounts and the archived chained-CPI index series. (iii) The tax-evaluation budget is stated once here (2.4) and the M1 probe is sized to 1,500,000 evaluations at the stated wall-clock limits; `PLAN.md` M1, `ARCHITECTURE.md` §4.3, `TESTING.md` §10, `SIMULATION-SPEC.md` §9 and `DECISIONS.md` C2 carry the same figure in the same terms (three evaluations at about 13 microseconds each would be 148% of a 27-microsecond ledger-year, so the per-year budget, not the per-evaluation figure, is the gate).

**Deviations from the research, forced by the spine:**

- **D1. `hurdle_low` is not fed by live yields.** The research sets the medium/low debt boundary at the 10-year Treasury yield + 3 pp "from live yields". ADR-020 forbids outbound connections by default, so the yield is a dated value in the assumption vintage, shown with its as-of date, user-overridable, and refreshed only by a new vintage or the post-1.0 opt-in market-data feature. The same applies to every market-dependent default in section 5.
- **D2. No user-facing future-rate slider as the Roth answer.** The research suggests one; ADR-019 replaces it with the flip value (M2) and the ledger-derived verdict (M3).

**Corrections of record this specification carries** (each is an arithmetic or statutory point on which a plausible reading of the inputs is wrong; each is **recorded in `DECISIONS.md`'s corrections of record** so the settled reading is stated once — the numbering below is DECISIONS' numbering):

- **C1. The senior-deduction phase-out is 6 cents per dollar per eligible person, not "+6 points" of marginal rate** (3.3). The statutory endpoints require it: $6,000 running off over $75k-$175k for one person, $12,000 over $150k-$250k for two, only works at 6%.
- **C4. Section 121 applies the dollar limit to the eligible gain, not the fraction to the capped amount** (9); the transposed order overstates taxable gain.
- **C5. Rule 1 of the debt comparison (term-matched Treasury after tax) binds for bondless households too** (5.3), which an "`E[r_equity] - ERP` otherwise" formulation would drop.

**Additions by this specification (not in the spine; each is editable data or a labelled convention, and every stored field named here has its home in `DOMAIN-MODEL.md`):** mid-year contributions (1.2); year 0 as a full tax year with YTD actuals (1.4; `Plan.ytd`, DOMAIN-MODEL 3.1); the `chained_cpi_wedge` assumption, default 0 (1.3; `AssumptionSet`, DOMAIN-MODEL 15); the SHORTFALL rule with its computed penalized alternative and the `Penalized` / `SeparatedAt55` / `Sepp72t` opt-in rules (2.4; `WithdrawalPolicy.early_access`, DOMAIN-MODEL 11) with the `f5329.*` line (3.2); the stored `deferral_type` split and its two-pass fixed point (2.3, 6; `PayrollDetail`, DOMAIN-MODEL 7); `household.prior_magi` (3.4; DOMAIN-MODEL 4); two-pass federal-state ordering (4.3); `r_u(H)` on the risk-matched base with the safe-yield floor, the `TaxableSafe` option and the vintage-implied ERP default (5.3, 5.4); the 529 tier position (5.4); the `Sensitive` verdict and its `min_survivor_years` guard (6; `RothVerdictPolicy`, DOMAIN-MODEL 11); the `ssa44` toggle (3.4).

**Open items:**

- **F2. Unverified inputs on the critical path** (must clear the M1 hand-verification gate): SALT phase-down threshold schedule and rate; 402(g)/219(b)(5)/414(v) rounding increments; mega-backdoor derivation and catch-up exclusion from 415(c); the `MagiKind` add-back lists; OBBBA ACA sections; IRA phase-out rounding; the bond-sleeve comparison's source page; the non-itemizer charitable deduction and the 35-cent itemized cap; the early-distribution additional-tax rate and exception list behind `f5329.*` (3.2); and, at the **M0** gate, the 26 USC 1(f) bracket base year and the identity of the chained-CPI index series (1.3).
- **F3. Preference defaults** shipped as visible assumptions and listed in DECISIONS open decision 8: ERP (default vintage-implied, with the 5% preset `erp-5pct` per [K2012] — both printed), the `r_u` horizon `H`, `hurdle_high` 8%, heirs rate 24%, survivor multiplier 0.70, `roth_verdict.min_survivor_years`, `chained_cpi_wedge`, liquidity haircut, emergency-fund factors.

---

## 15. Source keys

Two kinds of key appear in this document.

**Internal-review keys.** `DR x.y`, `DR Cn`, `DR An`, `TAXD`, `DEBT`, `G1`, `G3`, `G5` and `G6` cite sections of **the project's internal research review, which is not published**. A number carrying only an internal-review key, with no primary key beside it, is **unsourced in the published set**: it is treated as **(unverified)** and may not enter a locked vintage until a primary document has been archived under `params/provenance/` at the hand-verification gate (`TESTING.md` §5.2, §12). Where a primary key accompanies an internal key, the primary document is the authority and the internal key records only where the derivation was first worked.

**Primary keys** resolve to a public document:

| Key | Source |
|---|---|
| USC1 | 26 USC 1(f), https://www.law.cornell.edu/uscode/text/26/1 |
| USC63 | 26 USC 63(c)(4), https://www.law.cornell.edu/uscode/text/26/63 |
| USC121 | 26 USC 121, https://www.law.cornell.edu/uscode/text/26/121 |
| RP25-32 | Rev. Proc. 2025-32, https://www.irs.gov/pub/irs-drop/rp-25-32.pdf |
| N25-67 | Notice 2025-67, https://www.irs.gov/pub/irs-drop/n-25-67.pdf |
| RP25-19 | Rev. Proc. 2025-19, https://www.irs.gov/pub/irs-drop/rp-25-19.pdf |
| RP25-25 | Rev. Proc. 2025-25, https://www.irs.gov/pub/irs-drop/rp-25-25.pdf |
| IRS-LIM | https://www.irs.gov/newsroom/401k-limit-increases-to-24500-for-2026-ira-limit-increases-to-7500 |
| FR-CU | Catch-up final rule, https://www.federalregister.gov/documents/2025/09/16/2025-17865/catch-up-contributions |
| USC151 | 26 USC 151(d)(5), https://www.law.cornell.edu/uscode/text/26/151 |
| USC6433 | 26 USC 6433, https://www.law.cornell.edu/uscode/text/26/6433 |
| USC1395r | 42 USC 1395r(i), https://www.law.cornell.edu/uscode/text/42/1395r |
| PL119-21 | Public Law 119-21, https://www.congress.gov/119/plaws/publ21/PLAW-119publ21.pdf |
| NIIT | https://www.irs.gov/individuals/net-investment-income-tax |
| SSA-FR | SSA 2026 constants notice, https://www.govinfo.gov/content/pkg/FR-2025-11-03/pdf/2025-19763.pdf |
| T751 | IRS Topic 751, https://www.irs.gov/taxtopics/tc751 |
| P915 / P590B / P550 / P936 / P523 / P925 / P946 / P969 / P501 / P551 | https://www.irs.gov/publications/p915 ; https://www.irs.gov/pub/irs-pdf/p590b.pdf ; https://www.irs.gov/publications/p550 ; https://www.irs.gov/publications/p936 ; https://www.irs.gov/publications/p523 ; https://www.irs.gov/publications/p925 ; https://www.irs.gov/pub/irs-pdf/p946.pdf ; https://www.irs.gov/publications/p969 ; https://www.irs.gov/publications/p501 ; https://www.irs.gov/publications/p551 |
| F8606 | Form 8606, https://www.irs.gov/pub/irs-pdf/f8606.pdf |
| 8962 / PTCQA | https://www.irs.gov/instructions/i8962 ; https://www.irs.gov/affordable-care-act/individuals-and-families/questions-and-answers-on-the-premium-tax-credit |
| IRS-HSA | https://www.irs.gov/newsroom/treasury-irs-provide-guidance-on-new-tax-benefits-for-health-savings-account-participants-under-the-one-big-beautiful-bill |
| CMS | 2026 Part B/IRMAA fact sheet, https://www.cms.gov/newsroom/fact-sheets/2026-medicare-parts-b-premiums-deductibles ; Trustees report https://www.cms.gov/oact/tr/2026 |
| CMS-AGE | https://www.cms.gov/CCIIO/Programs-and-Initiatives/Health-Insurance-Market-Reforms/Downloads/StateSpecAgeCrv053117.pdf |
| HCGOV | https://www.healthcare.gov/glossary/out-of-pocket-maximum-limit/ |
| KFF | https://www.kff.org/affordable-care-act/state-indicator/average-marketplace-premiums-by-metal-tier/ |
| ASPE | https://aspe.hhs.gov/topics/poverty-economic-mobility/poverty-guidelines |
| DOL | https://www.dol.gov/sites/dolgov/files/ebsa/about-ebsa/our-activities/resource-center/publications/an-employees-guide-health-benefits-under-cobra-2022.pdf |
| POMS | RS 00615.320, https://secure.ssa.gov/poms.nsf/lnx/0300615320 |
| TF26 | https://taxfoundation.org/data/all/state/state-income-tax-rates-2026/ |
| BOGLE | https://boglecenter.net/bogleheads-chapter-series-prioritizing-investments/ |
| WCI | https://www.whitecoatinvestor.com/invest-or-pay-off-debt/ |
| MG | https://moneyguy.com/article/is-a-5-student-loan-debt-considered-high-interest-pay-it-off-asap/ |
| V2023 | https://corporate.vanguard.com/content/dam/corp/research/pdf/in_case_of_emergency_break_glass.pdf |
| K2012 / K2014 | https://www.kitces.com/blog/why-keeping-a-mortgage-and-a-portfolio-may-not-be-worth-the-risk/ ; https://www.kitces.com/blog/how-to-evaluate-your-clients-current-and-future-marginal-tax-rate/ |
| ERN21 | https://earlyretirementnow.com/2017/10/11/the-ultimate-guide-to-safe-withdrawal-rates-part-21-mortgage-in-retirement/ |
| V-BETR | https://corporate.vanguard.com/content/dam/corp/research/pdf/a_betr_approach_to_roth_conversions_072025.pdf |
| NYFED | https://www.newyorkfed.org/medialibrary/media/research/staff_reports/sr218.pdf |
| FRED-CS | https://fred.stlouisfed.org/graph/fredgraph.csv?id=CSUSHPINSA |
