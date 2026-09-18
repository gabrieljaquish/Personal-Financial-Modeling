# DECISIONS

*Architecture decision records. Date: 2026-09-17. Status of every record is **Accepted** unless marked otherwise. A record is superseded, never edited in place, once the milestone that implements it has shipped. All examples are synthetic or published third-party worked examples. Two sections follow the records: **Corrections of record** (C1-C5), where a wrong input or an unanswered flagged concern is fixed once and cited from the documents it touches, and **Open decisions**, which are not yet made.*

**Terminology used throughout (standing definition).** "The binary" or "the release" always means one thing: a **local web application for macOS**, shipped as a single self-contained executable (universal: Apple Silicon + Intel). Launching it starts a web server bound to loopback, serves the embedded web app over TLS, and opens the default browser. The user interface is a web app in the browser. It is not a native desktop GUI and not an Electron/Tauri window, and nothing else has to be installed.

| ADR | Title |
|---|---|
| 001 | Backend language: Rust |
| 002 | Front-end stack: Vite + React + TypeScript, Observable Plot |
| 003 | Release form: a local macOS web application |
| 004 | Licence (Apache-2.0) and the GPL/AGPL boundary |
| 005 | Plan-file format, encryption and unlock |
| 006 | TLS on loopback |
| 007 | Numeric representation and rounding |
| 008 | Annual ledger with monthly sub-engines |
| 009 | Scenarios as diffs over an immutable fact base |
| 010 | Parameter vintages and result pinning |
| 011 | State-tax plug-in model |
| 012 | In-house federal tax function with external oracles |
| 013 | Pluggable return generators and cross-architecture determinism |
| 014 | "Enough" is a scorecard |
| 015 | Browser session and localhost hardening |
| 016 | Explanation contract and stored workflow objects |
| 017 | API contract, type generation, and a front end that computes nothing |
| 018 | Milestone shape: a correctness-first engine on a vertical-slice schedule |
| 019 | Roth-vs-Traditional: the verdict is gated on the ledger that supports it |
| 020 | Network posture: zero outbound by default, one HTTP-capable crate |
| 021 | Extension model: compiled-in traits and data; connectors produce drafts |
| 022 | Validation corpus, mutation budget and the AI-assistance protocol |
| 023 | Repository data hygiene and user-agnostic documentation |
| 024 | Reports are print routes; no PDF library |

---

## ADR-001. Backend language: Rust

**Context.** The backend must be one of Rust, Go, C++ or Python. A release is one self-contained macOS executable (Apple Silicon and Intel), with no runtime to install, that runs a local TLS web server, parses untrusted import files and holds decrypted financial data in the same address space. The engine must run an exact, branchy, named-line tax function roughly 1,500,000 times per 10,000-path run (correction C2). The pinned toolchain is Rust (floor 1.84, `rust-toolchain.toml`), Node 26 for the web build and Python 3.13 fetched by `uv` for the oracle harness; Go is not part of the toolchain.

**Decision.** Rust, one Cargo workspace, toolchain pinned in `rust-toolchain.toml` (floor 1.84, bumped deliberately). Python survives only as the out-of-process oracle-harness language.

**Alternatives considered.**
- **Go.** Genuinely satisfies the release requirement (static binary, `embed`, trivial cross-compilation) and would ship a working product; it is the honest runner-up. It loses on the properties this domain needs most: (1) exhaustive sum types, since filing status, account tax type, `MonthRef`, rounding rule, coverage regime, income character and state fidelity are closed sets that change with law, and a missed arm is a silent wrong number; (2) a compiler-enforced money newtype where `Cents * Cents` does not compile; (3) thinner property-testing, snapshot, mutation and fuzz tooling, which is the backbone of the trust story; (4) a moving garbage collector means key material cannot be reliably wiped; (5) it would add a second toolchain to pin, sign and keep current beside Rust and Node.
- **Python.** Fails the release requirement in substance: one-file bundlers unpack an interpreter and many shared objects to a temporary directory at launch (slow start, every embedded library individually signed for notarization, a plaintext-adjacent temp directory), immutable `bytes`/`str` cannot be wiped, and making the per-year tax call fast needs NumPy vectorization that fights the named-line audit structs.
- **C++.** Produces the binary and offers nothing Rust does not, without memory safety on a listener that parses hostile files while holding secrets, and with the weakest package-audit, property-test and crypto ergonomics for a solo maintainer.

**Consequences.** One universal Mach-O via `lipo` of two targets. Pure-Rust TLS, AEAD and KDF mean no OpenSSL and no CMake in a two-target build. Slower compiles, a steeper language, and more compiler errors from AI assistants than Python or TypeScript would produce; this last cost is policy-level accepted as a feature, because the compiler rejects exactly the mistakes (unit confusion, missed enum arms) that a dynamic language accepts silently and a fixture suite may not catch. End users never install a toolchain.

---

## ADR-002. Front-end stack: Vite + React + TypeScript, Observable Plot

**Context.** The UI must build to static assets embedded in the binary. It is forms, tables and charts over server-owned state. The choice is genuinely close: Svelte wins on transitive dependency count and bundle size (a supply-chain argument), React on AI-assistance quality and ecosystem depth (a velocity argument).

**Decision.** Vite + React + TypeScript; TanStack Query; hash routing; plain CSS modules (no runtime CSS-in-JS, so the CSP can keep `style-src 'self'`); Observable Plot for charts; hand-built SVG for the next-dollar waterfall and the scenario diff; `d3-sankey` only if a cash-flow map is built. No client-side plan store, no persistence library, no service worker, no WASM.

**Alternatives considered.** Svelte 5 / SvelteKit static adapter (smaller SBOM, smaller bundles; rejected because the scarcest resource is one developer's time and the stated force multiplier is AI assistance, which is strongest in React/TypeScript, and because nothing in this UI rewards a smaller runtime). ECharts (kept only as the documented fallback if Plot cannot run under the strict CSP; decided by an M0 spike). A browser-side WASM engine (rejected: second execution environment, `'wasm-unsafe-eval'` in the CSP, and decrypted facts shipped to the browser for computation).

**Consequences.** The supply-chain cost of the larger npm tree is paid down mechanically: `npm ci --ignore-scripts`, lockfile lint, `npm audit signatures`, Renovate with a 7-day minimum release age, a per-package licence check (no front-end licence has been confirmed per package in advance), all assets local (no CDN, fonts or analytics). Node is a build-time tool only.

---

## ADR-003. Release form: a local macOS web application

**Status.** Accepted; one sub-point confirmed empirically in M0 and recorded here.

**Context.** A release is one self-contained macOS executable that the user launches; it runs a local web server, serves the web app on localhost over TLS and opens the default browser. It is not a native GUI and not an Electron/Tauri window. macOS is the v1 platform. Two facts complicate "one bare executable": a notarization ticket can be stapled to `.app`, `.dmg` and `.pkg` containers but not to a bare Mach-O, so an un-stapled first launch requires an online Gatekeeper check; and recent macOS removed the Control-click bypass for un-notarized software, so that check is a hard first-run failure on an offline or captive-portal machine. A bare Mach-O double-clicked in Finder also opens a Terminal window.

**Decision.**
1. The product is **one universal Mach-O** (`lipo` of `aarch64-apple-darwin` and `x86_64-apple-darwin`) with the web app, the public parameter vintages and **the synthetic demo plan as a plaintext JSON fixture** embedded (`fixtures/plans/demo.plan.json`; never a `.pfplan`, see ADR-023). Developer ID signed, hardened runtime with **no entitlements**, notarized.
2. It ships through **two channels built from the byte-identical executable**: (a) the default channel, a stapled DMG containing a **three-file `.app` wrapper** (`Info.plist` with `LSUIElement`, an icon, the executable); (b) a tarball of the bare executable for CLI and Homebrew users.
3. **Guard clause.** The wrapper is a notarization container and a Finder launcher. It contains no second executable, no bundled runtime, no WebView and no native window, and takes no Dock icon; the UI renders in the user's default browser. It must never be read as licence to add a native shell. Tauri v2, Electron and any WebView host are refused.
4. M0 verifies empirically whether `stapler staple` accepts a bare Mach-O and what Gatekeeper does on a network-isolated clean account. If stapling needs a container (expected), channel (a) is **mandatory**, not a convenience.
5. No Windows/Linux packaging, signing or key-store work in v1; portability is preserved by traits, `cfg`, and the engine's `wasm32` build.

**Alternatives considered.** Bare executable only (accepts a first-launch failure offline; rejected). A `.pkg` installer (heavier, writes outside the user's control). A full native app bundle with a menu-bar UI (a native GUI; rejected by requirement). Electron/Tauri (rejected by requirement).

**Consequences.** Apple Developer Program enrolment is a hard pre-M0 dependency. A Dock-less agent process is easy to forget while unlocked, so auto-lock, exit-after-lock and a prominent "Lock and quit" are required. The wording "one self-contained executable" is read as "one executable, optionally inside a minimal notarization container"; this reading is flagged as an open decision for the project maintainer to ratify.

---

## ADR-004. Licence (Apache-2.0) and the GPL/AGPL boundary

**Context.** The licence must be permissive (MIT or Apache-2.0). MIT/CC0/Apache sources may be ported with attribution. GPL/AGPL projects (Owl, PolicyEngine-US) may be used only as out-of-process, test-time oracles. TPAW Planner (PolyForm Noncommercial) is read-only; GitHub reported its licence as `NOASSERTION` as of 2026-09-17, so scanners miss it.

**Decision.** **Apache-2.0**, with a `NOTICE` file carrying an attribution entry — repository URL, licence, and the commit SHA pinned at the time of the port — for **every** permissive source the corpus ports or transliterates, and for vendored CC0 data (Tax-Calculator parameter values). The enumerated list, which `PLAN.md` §4.13 gate item 11 checks `NOTICE` against at every release:

| Source | Licence | What is taken | Pinned at |
|---|---|---|---|
| [Open Social Security](https://github.com/MikePiper/open-social-security) | MIT | Claim-age factors, the survivor ordering of `benefit.service.ts`, the 11 spec files, the PV fixtures | M3 (factors), M5 (suite), M8 (PV) |
| [ssa.tools](https://github.com/Gregable/social-security-tools) | MIT | Earnings record → AIME → PIA, `pia.test.ts`, `constants.ts` as a drift detector | M5 |
| [cFIREsim-open](https://github.com/boknows/cFIREsim-open) (the `boknows` repository; its `LICENSE` file is Apache-2.0, last push 2022-04-08, checked 2026-09-17) | Apache-2.0 | `test/spendingModule/` spending-rule assertions | M7 |
| [R4GoodPersonalFinances](https://cran.r-project.org/package=R4GoodPersonalFinances) | MIT (CRAN v1.2.0) | Milevsky-Robinson closed-form mathematics | M7 |
| [muirjc/retirement-planner](https://github.com/muirjc/retirement-planner) | MIT | The `InheritedIraDetails` shape mirrored by `InheritedDetails` | M2 |
| [Tax-Calculator](https://github.com/PSLmodels/Tax-Calculator) | CC0-1.0 | Parameter values and projections vendored into `params/`; `test_calcfunctions.py` transliterated | M0, M1 |

A new port or transliteration requires its `NOTICE` entry **in the same PR**; a release whose `NOTICE` is missing an entry for a pinned suite fails gate item 11. Beancount (GPL-2.0) and hledger (GPL-3.0) are **not** on this list: their booking-method semantics are read-only references, and the M9 lot-selection behavioural cases are written from scratch against documented semantics, with no test copied (`TESTING.md` §4, §14). **Third-party names identify published methodologies and imply no affiliation, sponsorship or endorsement; all trademarks belong to their owners** — this sentence appears in `NOTICE` and on the assumptions sheet, and a shipped preset whose name would otherwise be only a brand carries a descriptive id with the attribution in its provenance field (`ENGINE-SPEC.md` §5.4). The copyleft boundary is **architectural**: a subprocess and a file format, never an import. `oracles/` and `tools/` are not Cargo workspace members and are excluded from release archives; adapters are written from scratch; PolicyEngine YAML tests are consumed as data at a pinned commit from a git-ignored cache and are never committed; oracle outputs are **recorded** as goldens so ordinary CI installs no copyleft package. **Parameter-table values are never sourced from a downstream project**; a third-party default may be cited as a comparison with attribution, never adopted into `params/`.

**Alternatives considered.** MIT (simpler; no explicit patent grant and no standard attribution vehicle). Dual MIT/Apache-2.0 (Rust-ecosystem convention; adds contributor confusion for no benefit to an application). Linking PolicyEngine-US or Owl for richer state coverage or optimisation (forbidden; would relicense the product).

**Consequences.** Enforcement is mechanical: `cargo-deny` licence allowlist, per-package npm licence check, and an SBOM assertion at the release gate that no GPL/AGPL/PolyForm/Parity component reaches a shipped artifact. State results rest on one 2026-capable oracle and are labelled accordingly (ADR-011).

---

## ADR-005. Plan-file format, encryption and unlock

**Context.** All user data lives in **one** local encrypted plan file, encrypted at rest; a passphrase (Argon2id-derived key) always works and keeps the file portable; macOS Keychain storage of the key is opt-in. The project's internal research review (unpublished) classified the store as an unresearched first-order dependency, so every number here comes from this record and its companions rather than from a researched source. The plan is a versioned JSON document with RFC-6902 patches, a migration chain and golden fixtures, not a relational dataset.

**Decision.** A documented custom container, `*.pfplan`, format v1:
- Plaintext canonical-JSON header (magic, `formatVersion`, `fileId`, `generation`, KDF parameters, key slots, section table), **authenticated as AAD for every key wrap and every chunk**, so tampering with KDF parameters fails authentication rather than weakening the key. The container is authenticated before it is parsed.
- A random 256-bit DEK; **key slots** wrap it: passphrase (mandatory, permanent; Argon2id calibrated to about one second, floor m = 256 MiB, t = 3, p = 4, **bounds enforced on read**: reject m > 4 GiB, t > 16, below-floor downgrades); optional printed one-time recovery code; opt-in macOS Keychain slot (generic-password item, **`synchronizable = false`**, ACL bound to the code-signing designated requirement; an integration test asserts the attribute).
- Sections `plan | changelog | snapshots | results | secrets`, each under `HKDF-SHA256(DEK, "pfplan/v1/" + label)`, DEFLATE-compressed, **padded to 64 KiB buckets**, XChaCha20-Poly1305 in the chunked STREAM construction. `secrets` is decrypted only on demand.
- Whole-file atomic rewrite with three rolling encrypted backups; no plaintext temp, journal or cache files. Monte Carlo paths are never stored (summaries plus the seed).
- `vault rekey --rotate-dek` re-encrypts everything — the plan file **and every sibling backup** under the new DEK, or, with `--discard-backups`, overwrites and unlinks the backups and restarts rotation; the operation reports which it did. Each backup is a complete container under the old DEK, so a rekey that reached only the plan file would leave up to three copies beside it, in the same directory and the same Time Machine snapshot, still openable with the passphrase being rekeyed away (`SECURITY.md` §3.5, §3.7). The passphrase-change screen states: changing the passphrase does not protect older backups or synced copies; use Rekey to rotate the underlying key — and a compromised passphrase is a reason to create a new file, not only to rekey.
- A ~100-line **Python reference decryptor** in `tools/pfplan-ref/` (never in a release artifact) is the cross-implementation CI oracle, the proof of portability and the user's escape hatch. The format spec is published. Its test containers are **generated at test time from the plaintext JSON fixture and deleted**; no `.pfplan` byte sequence is ever committed or archived (ADR-023).
- **No container ever ships inside the product.** The embedded demo is plaintext synthetic JSON carrying `"synthetic": true` with its generator name and seed; the binary encrypts it into a `.pfplan` in the user's own plan directory (or a temporary file, for a Playwright run) at first use. This keeps the single control that would catch a real household file — the container-magic check — free of any allowlist.
- `formatVersion` and the plan's `schemaVersion` are independent.

**Alternatives considered.** SQLCipher (C dependency in a universal build, PBKDF2 default, journal/WAL side files, relational shape that fights scenario-as-patch and whole-document goldens). The `age` format (its passphrase recipient is scrypt; Argon2id is fixed by requirement). A single AEAD blob without sections (no on-demand secrets, no per-section keys, file size leaks plan complexity). zstd compression (C binding; DEFLATE via the pure-Rust backend keeps the build C-free and the Python decryptor dependency-light). CBOR header (adds a dependency to the reference decryptor for no benefit).

**Consequences.** "Rolling our own format" is answered with standard constructions only (key wrap, STREAM, HKDF), RFC test vectors, a byte-flip sweep, crash-during-save tests, fuzzing with a committed corpus, the independent decryptor and an external review before 1.0. A forgotten passphrase without a recovery code or Keychain slot is unrecoverable, and the setup flow says so before a passphrase is typed. Rollback by an attacker with write access is out of scope; `generation` detects accidental stale copies.

---

## ADR-006. TLS on loopback

**Context.** Browser-to-backend traffic must be encrypted in transit, on loopback only. Browsers already treat `http://localhost` as a secure context, so TLS here is not about web-platform features.

**Decision.** TLS 1.3 via `rustls` (`ring` provider). The release profile compiles **only** a TLS acceptor; a CI test connects over plaintext HTTP and asserts failure. The listener binds `127.0.0.1` only (a startup self-check refuses anything else); canonical origin is the IP literal `https://127.0.0.1:<port>` to avoid resolver and `/etc/hosts` tampering. First launch generates a per-install ECDSA P-256 CA with `pathLen = 0` and critical name constraints (`localhost`, `127.0.0.1`, `::1`), signs one leaf (SAN, `serverAuth`, 820 days), then **zeroizes the CA private key**, so the trusted anchor can never issue another certificate. After a one-paragraph native explanation — **shown from the application's own process** as a single alert panel (`CFUserNotificationDisplayAlert` or an in-process `NSAlert`; no GUI framework, no application window) — the CA is added to the **user** trust domain for the SSL policy via the system authorisation prompt. **Amended (ratifying `SECURITY.md` D7):** the mechanism first named here, `osascript display dialog`, is withdrawn. It drives Apple Events from a spawned interpreter, which the hardened runtime and TCC are built to restrict and which would want the very entitlements this design forbids; it attributes the dialog to "osascript" rather than to the application; and in the bare-executable channel there is no bundle identity to correct that. The text of the explanation is unchanged, and the terminology block is untouched: a native alert panel is not a native UI. The leaf key lives in the login keychain bound to the code signature (0600 file fallback). Declining keeps TLS-only service behind a browser warning with a printed fingerprint; there is no HTTP fallback. `pfp trust remove` uninstalls cleanly; renewal repeats the flow about every two years.

**Alternatives considered.** Plain HTTP on loopback (violates the hard rule; leaves port squatting open). An `mkcert`-style long-lived CA key on disk (a standing signing key that can mint certificates for anything within its constraints). A self-signed leaf without a CA (permanent browser warning). A publicly trusted certificate for a loopback-resolving hostname (requires distributing a private key in a public binary).

**Consequences.** The honest value statement is published: loopback TLS defends against loopback capture and, above all, **port squatting** (a process that binds the port first cannot present the trusted leaf, so it cannot phish the passphrase). Trust behaviour for user-added, name-constrained roots differs across Safari, Chrome and Firefox and drifts over time; it is an M0 spike and a standing release-gate Playwright test. **This record is where the M0 spike's outcome lands** (`PLAN.md` M0 scope, R25): the non-interactive CI trust provisioning — a pre-seeded runner trust store or `security add-trusted-cert -d` against an unlocked CI keychain, plus a Firefox profile with the root pre-imported — and the resulting declaration of which browsers run per PR, which run only at release, and any step that stays manual, which closes open decision 3. The stapling experiment is recorded separately, in ADR-003.

---

## ADR-007. Numeric representation and rounding

**Context.** IRS whole-dollar worksheets, SSA dime and dollar truncation, CMS nearest-$1,000 and statutory uprating rules coexist in one engine; a single global rounding mode silently breaks at least one. Statutory factors include repeating fractions (5/900, 5/1200, 2/3 of 1%) and values that land exactly on truncation boundaries: the canonical Open Social Security fixture, PIA 1,000 at 60 months early, computes to exactly 700.00, immediately above a dollar-truncation boundary, so any fixed-point rate passes or fails by the unstated sign of its representation error.

**Decision.**
- `Cents(i64)` for all money in facts, ledger, tax and Social Security paths; JSON carries integer cents.
- `Ratio{num: i64, den: i64}` for every statutory rate and factor, parsed from decimal strings or fractions. `cents.mul_ratio(r, rule)` computes in `i128` and rounds **exactly once**. Composite factors are reduced to one rational at point of use.
- `RoundingRule{increment, direction: Down|Up|HalfUp|HalfEven|Nearest, basis: Amount|IncreaseOverBase}` stored **as data beside the parameter it governs**. `basis: IncreaseOverBase` implements 26 USC 1(f)(7): the *increase over the statutory base year* rounds down to $50 ($25 for the MFS table). **It is computed from the statutory base-year amount times the index ratio, with the reduction applied exactly once**, never by chaining the prior year's published figure forward — a chain rounds twice and drifts off the published table (correction C3; ADR-010 carries the base-year amounts and the index series). Standard deduction $50 down; 415(c) $1,000 down; IRMAA thresholds nearest $1,000; PIA dime-truncated at each COLA; payable benefit dollar-truncated.
- The rate-schedule formula is the canonical tax computation; IRS Tax Table mode exists only in the validation adapter.
- `f64` is fenced to four places: return/inflation generation; the two named growth/constructor steps in `pfp-money` (`Cents::grow` and `Cents::from_f64_half_even`, both half-even at the cent — the only `f64`-to-money entry points, `ARCHITECTURE.md` D5(b), `SIMULATION-SPEC.md` §2.6; a third is a design change, not a refactor); statistics; solvers and spending-rule arithmetic, which return money through `from_f64_half_even`.
- **PIA 1,000 -> 700 is a named, mandatory CI gate** with a comment explaining why it is load-bearing.
- **No generic `Amount`/`Money` trait is threaded through the tax crate.** A performance probe runs at M1 at the **realistic load of about 1,500,000 `federal()` evaluations** (correction C2), reported separately for `TraceLevel::None` and `TraceLevel::Full`. Contingency ladder, in order, only if a gate fails: **(rung 0) suppress `Line` construction under `TraceLevel::None`**, which is the cheapest and largest win because the full line map is 40+ nodes with two `SmallVec`s each; then precompute path-invariant work and per-path uprated tables; an `i64` fast path when the product provably fits; denominator-specialised division (most rates have a power-of-ten denominator); and only last an `f64` lane, guarded by a lane-parity property test (every named line within $1) and never used for deterministic results.

**Alternatives considered.** Fixed-point rates in parts-per-million (too coarse for the Social Security path: about $0.12 of monthly benefit at a high PIA), parts-per-billion or 1e-8 (adequate precision but still a knife-edge on truncation boundaries and a thing to reason about forever). `rust_decimal` (decimal exactness does not represent 5/900). A generic money trait over `Cents` and `f64` from day one (speculative generality in the most correctness-critical crate; two answers for one product). Global `ROUND_HALF_EVEN` (breaks at least one convention).

**Consequences.** Exactness is a property of the types, not of reviewer vigilance. `i128` division costs some throughput, measured early. Parameter files carry slightly more structure. A lint forbids `f64` in `pfp-tax`, `pfp-ss` and `pfp-money` outside `grow` and `from_f64_half_even` (`ARCHITECTURE.md` D5(b); `TESTING.md` §6.1 D5 row and §11.2 gate 7 carry the same pair).

---

## ADR-008. Annual ledger with monthly sub-engines

**Context.** Every professional engine examined is a year-by-year ledger run in nominal dollars; some rules are inherently monthly (Social Security claiming and the earnings test, amortization, coverage switches at the 65th-birthday month). Goals-only planning structurally cannot answer Roth, bracket, asset-location or debt-vs-invest questions.

**Decision.** One cash-flow ledger as a pure function, `project(plan, assumptions, params, path, opts)`, implementing the nine-step annual loop; **nominal internally, real for display**; monthly sub-engines only where the rules are monthly, aggregated into the year. Contribution order and withdrawal order are symmetric strategy objects stored as data. **`ENGINE-SPEC.md` §2.4 is the single owner of deficit routing and of the gross-up contract**, and every other document references it rather than restating it: before 59.5 the default outcome of an uncovered deficit is `SHORTFALL` with a **computed "penalized alternative" shown beside it** (the engine never silently books the penalty), and the penalty-bearing routes are opt-in `WithdrawalPolicy.early_access` entries (`EarlyAccessRule::Penalized{account_ids}`, `SeparatedAt55{employer_plan_id}`, `Sepp72t{…}`, `DOMAIN-MODEL.md` §11), empty by default; the two statutory exceptions are honoured from M8 once their conditions clear the hand-verification gate. Gross-up solves by **secant on the real `federal()`**, with the closed-form segment walk used only as the initial guess, because the kink list (senior-deduction phase-out, IRMAA, the PTC cliff, state rules) is not closed. A good initial guess is what keeps the mean settle cost to one further evaluation inside the budget of correction C2; the hard per-year evaluation cap of `ENGINE-SPEC.md` §2.4 bounds the tail and falls through to `SHORTFALL`. The first-death state machine (MFJ through the death year, status flip in t+1, qualifying surviving spouse only with a dependent, re-ownership, step-up policy, joint MAGI in the IRMAA lookback for two more years) is built before Monte Carlo and runs identically in deterministic sweeps and stochastic paths. A goals view reads the same output.

**Alternatives considered.** A fully monthly ledger (12x the tax-timing complexity for little decision value; annual tax is annual). A goals-only mode alongside the ledger (two engines; rejected). Real-dollar internals (brackets, RMDs, fixed debt payments and the never-indexed thresholds are nominal facts).

**Consequences.** Intra-year timing is approximated by stated conventions (start-of-year withdrawals, end-of-year deaths) that are printed on the assumptions sheet. The ledger has no published fixtures anywhere, so its trust rests on the conservation identity (residual exactly zero), metamorphic properties, hand-worked Tier-1 ledgers and persona goldens (ADR-022).

---

## ADR-009. Scenarios as diffs over an immutable fact base

**Context.** Scenario-as-diff has converged across the industry under three names; copy-per-scenario is the documented anti-pattern because copies diverge the moment a fact changes. Git cannot version an encrypted, out-of-repo plan, so the household audit trail must live inside the plan file.

**Decision.** A scenario is `{id, name, parentId|null, kind, assumptionSetId, paramVintageIds[], seed, ops: RFC-6902[]}`; `resolve(s) = validate(applyPatch(parent ? resolve(parent) : base, s.ops))`. Every collection is `Record<id,item>` so patch paths use stable ids, never array indices. A **fact-path allowlist** rejects ops on fact paths unless `kind == hypotheticalFacts` (the hatch for death and disability what-ifs). The product spine is three scenarios: **Current Course** (baseline, no ops), **Proposed Plan** (*Adopt* appends ops and creates an action item), and **What-ifs**. Compared scenarios share seed, vintages and shock tensor. The audit trail is an in-file append-only change log of `{timestamp, typed action, reverseDiff}` plus immutable fact and result snapshots; undo is built on the same log. Migrations rewrite scenario patch paths as well as the base. The server applies patches; the front end only sends typed actions.

**Purge sits beside compaction, and is a confidentiality control, not a size valve.** A `reverseDiff` carries the prior value of every fact ever changed, and deletion is tombstoning wherever a snapshot may reference the row, so without purge a mistyped earnings record, a former employer's plan detail or a balance the user explicitly removed is retained for the life of the file. Therefore: `vault compact --purge-before <date>` drops `reverseDiff` payloads and snapshot bodies while **retaining metadata-only entries** (timestamp, action kind, actor) so the audit trail survives as a record of *that* something changed; `removePerson(p)` offers purge of that person's history as an explicit, separately confirmed step; purge forces a whole-file rewrite and rotates the backups. The UI and `docs/threat-model.md` state plainly that purge cannot reach older backups, exported copies or synced copies.

**Alternatives considered.** Copy-per-scenario (rejected above). A relational store with scenario tables (fights whole-document goldens and the migration chain). Client-side patch application (a second implementation of plan semantics and an unverified-licence dependency).

**Consequences.** The ops list doubles as the human-readable diff. Removing a person is a reference scrub that fails loudly on dangling `MonthRef`s. Change-log retention is unbounded by default with compaction and purge commands (open decision 4 on the default cap). **The blast radius of a captured plan file is therefore its full history, not its current state** — recorded against asset A3 in `SECURITY.md` §1, because a retained superseded fact is as sensitive as a live one.

---

## ADR-010. Parameter vintages and result pinning

**Context.** Every assumption and parameter must be visible, sourced, versioned and overridable, and every stored result pinned. Derived arithmetic has been observed to reproduce exactly while metadata rotted, and a downstream project has been observed to project a 2026 limit at $115,000 where the primary notice sets $111,000.

**Decision.**
- Public parameters live in `params/` as year-keyed TOML tables: `{id, unit, breakdown, values, projection{rule: index|wage|flat|zero|schedule, index, index_series, base_year, base_values, rounding}, [[source]]{title, url, retrieved, sha256}}`. **A table without a projection rule is a CI error** (it would silently freeze brackets and manufacture bracket creep). Never-indexed items carry `flat` explicitly. **`base_year` and `base_values` hold the statutory base-year amounts per filing status, and `index_series` names the archived price series the ratio is taken against**; a table whose rounding basis is `IncreaseOverBase` and which lacks either is a CI error, because the statutory figure is `base_value x index_ratio` with the reduction applied once and cannot be reconstructed from the prior year alone (correction C3, ADR-007). The index series itself is archived with a checksum like any other source. The applicable base years and series are **hand-verification-gate items**: the statute establishes the *rule* (the increase over the statutory base year, not the final amount), and no archived source states the base year of each table.
- **Immutable vintages.** Vintage id = `<name>@<content-hash>`; `VINTAGES.lock` holds a SHA-256 per file and CI fails if a locked file changes; corrections ship as a new vintage. Released vintages are path-protected from AI-assisted edits.
- **Primary sources only**, archived with checksums under `params/provenance/`; a constants-sync test checks each vintage against its vendored primary text; a hand-verification gate covers every unverified item (`TESTING.md` §5.2) before a vintage is locked.
- **Overrides are a separate layer** in the plan file, displayed beside the sourced value; vintages are never edited. Capital-market and inflation assumptions are immutable `AssumptionSet` vintages with as-of dates.
- **Result pins** on every stored or exported output: `{engineVersion (semver + commit), binaryDigest, schemaVersion, paramVintageIds[], assumptionSetId, marketDataAsOf, generator + config, seed, nPaths, criterion, inputsHash}`. CI refuses a golden-snapshot change without an `engineVersion` bump.
- **Honest limit, stated in the UI:** a release does not embed old engines. An old result is explained from its pins and re-run under the current engine with the difference shown; it is not bit-reproduced. The annual review attributes each KPI change to facts vs assumptions vs law (vintage) vs engine.

**Alternatives considered.** Parameters in code (unsourced, unversioned, invisible). Editable parameter files (destroys reproducibility). Embedding old engines for bit-reproduction (binary bloat and an unbounded maintenance surface). Live parameter fetches (violates the network posture and reproducibility).

**Consequences.** New law means a new vintage inside a new release, with a per-table delta shown before a plan adopts it. The Assumptions Registry page is generated from the same data.

---

## ADR-011. State-tax plug-in model

**Context.** States are pluggable: v1 ships the no-income-tax states, a user-entered effective-rate fallback for any state, and the plug-in interface; exact modules are added incrementally. PolicyEngine-US (AGPL, out of process) is the only 2026-capable state oracle and no state worked example exists anywhere.

**Decision.** `trait StateTax { compute(year, status, &FederalReturn, &StateInputs, &ParamView) -> StateReturn; fidelity() -> Exact | NoIncomeTax | EffectiveRate; validation_basis() }`, registered statically by state code. Most exact states are **data**: a generic `DeclarativeState` interpreter over `params/states/<code>.toml` (brackets or flat rate, deductions and exemptions, conformity starting point, SS-exclusion flag, age-gated retirement-income exclusions with caps, gains treatment). Hand-written implementations only where the data form cannot express a rule. `Fidelity` and the validation basis are printed on **every** state result. v1 ships `NoIncomeTax` (AK, FL, NV, NH, SD, TN, TX, WY), `EffectiveRate` for any state, and two reference `DeclarativeState` modules (one flat, one graduated) at M10.

**Washington is deliberately excluded from `NoIncomeTax`.** It levies no individual income tax, but it does levy an excise tax on long-term capital gains — 7% above an indexed standard deduction ($278,000 for 2025), with an additional 2.9% above $1,000,000 from 2025 per the Washington DOR (the Tax Foundation reports the top rate as 9%; the 2026 deduction amount is unverified), and with retirement-account distributions and real estate exempt. `NoIncomeTax` returns zero, which would silently misreport exactly the households this product serves: concentrated equity compensation and taxable-account decumulation. Washington therefore ships either as a `DeclarativeState` module whose only rule is a gains-threshold rate — a strong candidate for one of the two M10 reference modules (open decision 6) — or, until then, as `EffectiveRate` with a printed state-specific caveat naming the capital-gains excise tax, so that no result claims a zero state liability the engine has not computed. This sentence travels with the state roster in `ARCHITECTURE.md` §7, `PLAN.md` M2 and `ENGINE-SPEC.md` §4.1, and it is why `TESTING.md` §5.2 keeps Washington's 2026 deduction and top rate as an M10 hand-verification item.

**Alternatives considered.** Hand-written Rust per state (turns 40+ states into code contributions; does not scale for one maintainer). Calling PolicyEngine-US at runtime (AGPL; about 20 s import; forbidden). Dynamic plug-in loading (executes third-party code next to a decrypted plan). A flat effective rate only (misses SS exclusions and retirement-income rules that drive relocation and Roth decisions).

**Consequences.** Adding a state is a data contribution by pull request: one TOML module, fixtures, one recorded oracle run. Every exact module is single-oracle-validated and says so. User-confirmed state retirement-deduction values remain plan-level overrides.

---

## ADR-012. In-house federal tax function with external oracles

**Context.** The federal engine must be exact and must run inside 10,000 paths x 60 years. Open-source tax engines cannot: PolicyEngine-US is AGPL with a roughly 20-second import; Tax-Calculator is a Python microsimulation. The decision statistic is the effective marginal rate, computed by running the tax function twice.

**Decision.** Own a year-parameterized federal function in `pfp-tax` as **worksheets returning named lines**; one `Line` structure is the fixture intermediate, the UI audit trail and the EMR machinery. `TaxInputs` carries every income character from M1 (zeros until modelled). **The signature frozen at seam S3 carries the trace level explicitly** — `federal(year, status, inputs, params, trace: TraceLevel)`, and likewise `payroll()` and `marginal()` — so that the one credible performance escape hatch exists inside the frozen seam instead of requiring it to be reopened at M6. Under `TraceLevel::None` the function returns the totals and the typed accessors with `lines` empty and no `Line` allocated; a **tier-1 property asserts that the two modes agree to the cent on every typed accessor**. Later worksheets remain additions, never signature changes. Unsupported items are detected from inputs and surfaced as "not modelled", never silently ignored. External engines are **test oracles only**, out of process, with a **record/verify split**: Tax-Calculator (CC0) is the 2026 federal oracle (within $5 on a synthetic grid); PolicyEngine-US is the only 2026-capable state and ACA oracle; Owl's example cases bound the conversion planner; AnyPIA generates PIA fixtures; TAXSIM 35 and tenforty are back-year structural checks only. Tier-1 IRS worked examples with line-level intermediates gate hard.

**Alternatives considered.** Embedding or calling an open-source engine at runtime (licence and performance). Transpiling Tax-Calculator (couples the product to a project that is seeking maintainers). Bracket-table lookups for marginal rates (wrong in every phase-in and phase-out zone).

**Consequences.** Annual law maintenance is the project's own job, supported by vendored CC0 values, the Federal Register route around bot-walled pages, and nightly oracle re-recording as the early warning. "Exact federal" is a long tail; the coverage boundary is explicit and visible.

---

## ADR-013. Pluggable return generators and cross-architecture determinism

**Context.** Monte Carlo, historical replay and named stress tests must run through the unchanged ledger via pluggable generators. "Same seed, bit-identical on Apple Silicon and Intel" has three leaks: the PRNG, the platform math library, and the distribution sampler. Common random numbers are required for any meaningful A/B comparison; at N = 1,000 sampling noise swamps the differences between allocation choices.

**Decision.** `trait ReturnGenerator { generate(seed, path_idx, horizon, &AssumptionSet) -> ReturnPath }`; the interface exists from M2 with only the deterministic mean path behind it. Generators arrive in phases: named stresses; lognormal iid with **arithmetic-mean inputs** and Cholesky correlation (one draw per asset class per period; accounts mapped to allocations); circular block bootstrap of joint rows; historical replay reported as **integer window counts**, never a probability; AR(1) inflation. Determinism mechanism: `ChaCha8Rng` with **stream id = path index**; in-repo **Wichura AS241** inverse normal CDF with golden vectors (`rand_distr` does not promise value stability across versions); pure-Rust **`libm`** for `exp`/`ln`; float aggregation in path-index order. **CRN is structural:** `simulate(plans: &[ResolvedPlan], …)` shares one shock tensor across every plan compared. Run-size contract: ephemeral 1,000, saved 5,000, solvers and tails 10,000+; Wilson interval beside every rate.

**Alternatives considered.** PCG/Xoshiro (also portable; ChaCha8's stream addressing maps directly onto path indices). `rand_distr` normals (value stability not promised). Platform libm (differs across architectures). CRN by caller convention (one refactor breaks it). Per-account independent draws or a single shock across all asset classes (both known modelling errors).

**Consequences.** Results are independent of thread count and architecture, verified on both CI runners. Historical datasets without an explicit redistribution licence are not bundled; a loader with a documented CSV format and published checksums ships instead.

---

## ADR-014. "Enough" is a scorecard

**Context.** A lone probability of success hides the size and timing of the cut, reads 100% as good when it signals under-spending, and differs by vendor definition. Targeting 95% versus 50% with annual re-planning yields similar median spending but leaves several times the legacy unspent.

**Decision.** "Enough" is always a **scorecard**: essential and total funded ratio as a sensitivity strip (TIPS, TIPS + 1%, expected return; home equity excluded; no pass/fail band); a horizon-appropriate historical fail-safe rate with the data series, horizon, allocation and fee printed; Monte Carlo success probability **always paired** with `max_cut = (spending - guaranteed income) / spending`, worst-decile cut, shortfall timing and a Wilson interval; dollar guardrails re-solved at each review; spending at 95/90/80/70/50% with the unspent-legacy distribution. The success criterion is selectable and printed. Default solver target 85%, adjustable 50-95%. **Enforcement is at the contract layer:** the probability type cannot be constructed or serialized without its magnitude companions, verified by an API-level test (exports cannot bypass it), plus a component test. Folklore regression targets are refused.

**Alternatives considered.** A single gauge (the industry norm and the documented failure). UI-level enforcement only (bypassed by CSV/JSON export and by any new screen). A funded-ratio pass/fail band (the discount rate dominates the answer; the commonly quoted band is unverified).

**Consequences.** More to read on one screen, by design. Every `SimResult` DTO is larger. Heuristics (25x, salary multiples, Coast FI) appear as labels only.

---

## ADR-015. Browser session and localhost hardening

**Context.** Cookies on `127.0.0.1` are scoped by host, not port, so every local server on loopback shares one cookie jar; `sessionStorage` is scoped to the origin including the port. Hostile web pages can attack a localhost server (CSRF, DNS rebinding, cross-origin reads). No plan data may persist in the browser.

**Decision.** Launch token (256-bit, single use, 60-second TTL) in the **URL fragment** (never the query string, which enters history and logs), exchanged by POST, then cleared with `history.replaceState`. Session = **`__Host-` cookie (`Secure; HttpOnly; SameSite=Strict; Path=/`) AND an origin-scoped proof token in `sessionStorage` sent as `X-PFP-Proof`**; both are required on every API call. Exact `Host` allowlist (421) defeats DNS rebinding; exact `Origin` (403); `Sec-Fetch-Site` **scoped by route** — `same-origin` required on `/api/**`, where `none`, `same-site`, `cross-site` and an absent header are all 403, while document and static-asset routes accept `Sec-Fetch-Site: none` only with `Sec-Fetch-Mode: navigate` at path `/` (**amended, ratifying `SECURITY.md` D8:** the first request of every session is a top-level navigation that carries `none`, so the unqualified rule first written here would have 403'd the launch M0 acceptance requires; the scoped rule is strictly stronger on the API surface); JSON content type enforced; the custom header forces a preflight that is never approved; no CORS or Private Network Access headers ever. Strict CSP with zero third-party origins and no inline script; `no-store` on every API response and on `index.html`; no service worker, `localStorage`, IndexedDB or Cache API; `sessionStorage` holds **only** the proof token. Long runs stream **NDJSON over `fetch`** because `EventSource` and WebSocket cannot carry the proof header. File exports download via `fetch` + blob. Unlock attempts back off; auto-lock at 15 minutes idle; exit after lock with no heartbeat. The server tries a stable preferred port (so password managers can match the origin) and falls back to an OS-assigned port.

**Alternatives considered.** Cookie-only session (leaves the port-sharing hole open). Bearer token only, no cookie (eliminates CSRF by construction and was the base plan's design; the dual mechanism is preferred because a leaked proof token or a leaked cookie alone is useless, and `HttpOnly` keeps half the credential out of script reach). Banning `sessionStorage` outright (forecloses the origin-scoped factor). Server-Sent Events (cannot carry the header).

**Consequences.** Verified by Playwright against the real signed binary at every release: header exact-match snapshot, Host/Origin matrix, storage emptiness, zero third-party requests, zero CSP violations. A source lint on `web/` remains as a cheap first line but is not the control.

---

## ADR-016. Explanation contract and stored workflow objects

**Context.** The universal complaints about professional tools are black-box math, no audit trail and hidden defaults. The differentiator is an *auditable* next-dollar engine with no external oracle. A recommendation without its recorded assumption set is unverifiable a year later. This is the most expensive thing to retrofit and among the cheapest to design in.

**Decision.** Auditability is two data structures. (1) The `Line{id, label, value, inputs, params, rounding}` trace node on every computed tax, payroll and ledger value **whenever a trace is asked for**: deterministic runs and any re-run of a single path use `TraceLevel::Full`, Monte Carlo paths pass `TraceLevel::None`, in which the nodes are not constructed at all and only totals and typed accessors are produced (ADR-012). Auditability is never lost, because any path is re-runnable with `Full` from `(seed, path index)`, and a property test pins the two modes to the cent. (2) A typed `Explanation{claim, because[Reason{templateKey, values: [Bound{name, value, ref}]}], thresholds, alternatives[{option, r_u, whyNot}], flip[{input, currentValue, flipValue}], caveats, omissions}` on every `Recommendation`. **The reference travels with the value, not with the reason:** a `Reason` carries a list of named `Bound`s and each `Bound` carries its own `ref` (`LedgerRef | TaxLineRef | ParamRef | AssumptionRef`), because a realistic reason cites several cells at once and one shared `ref` could only be satisfied when every value was equal. **Reconciliation invariant**, in three parts, enforced by a property test over all personas and doubling as the decision code's test oracle: *agreement* — every `bound.value` equals the cell its `ref` resolves to in the same projection; *coverage* — every placeholder in the template named by `templateKey` has a `Bound` of that name; *reachability* — every `Bound` is rendered by its template, so a bound nothing displays fails exactly as a missing one does (`ARCHITECTURE.md` §4.2). **Flip values** by bisection turn each recommendation into a sensitivity statement. `Recommendation`, `Explanation`, `ActionItem`, `Review`, `FactSnapshot` and `ResultSnapshot` are **stored objects in the plan file from schema v1**, each carrying pins and a validation basis. The UI renders explanations from template keys; it never composes financial reasoning; no LLM participates at runtime.

**Alternatives considered.** Explanations as UI strings (drift from the math; untestable). A free-form trace dump (not checkable against references). Workflow artifacts as derived views (cannot answer "why did the plan change?" after facts move).

**Consequences.** Roughly 15-20% more work per decision module, accepted. Information architecture follows the planning workflow (Gather, Baseline, Analyze, Recommend, Plan, Monitor), and an engine feature is admitted only when a workflow step consumes it. The Annual Review and its attribution diff are scheduled after Monte Carlo and the scorecard, respecting the capability priority order.

---

## ADR-017. API contract, type generation, and a front end that computes nothing

**Context.** A Rust backend and a TypeScript front end can drift silently. "One engine language" must be literal.

**Decision.** JSON over HTTPS under `/api/v1`. DTOs derive `serde` and `utoipa::ToSchema`; the **OpenAPI document is generated at build time and snapshot-tested**; `openapi-typescript` generates the client types, so a Rust DTO change breaks the front-end build instead of a user session. The plan schema is additionally published as JSON Schema via `schemars` (a public repository artefact). The front end sends **typed actions** and renders **view-models**; it never applies RFC-6902 patches, never computes money and never receives the raw plan. An endpoint is admitted only when a screen in the current milestone consumes it.

**Alternatives considered.** `ts-rs` direct type emission (no contract document to snapshot or publish). GraphQL (schema tooling overhead with no benefit for one client). Hand-written client types (drift).

**Consequences.** One validator (the backend's); one source of truth for every shape; the contract changes only deliberately.

---

## ADR-018. Milestone shape: a correctness-first engine on a vertical-slice schedule

**Context.** Two approaches are defensible and complementary: one has the right numeric core, trace design and fixture discipline but delays visible value; the other has the right milestone shape and seam discipline but under-estimates by about 2x and ships a biased headline answer. Estimates of this kind are routinely optimistic. A release that computes nothing fails the "usable vertical slice" requirement.

**Decision.** (1) **Milestone-zero rule:** every milestone, including M0, produces a number a user can read, explain and export. (2) **Eight frozen seams** (parameter-table shape *including the projection block's base-year amounts and index series*, money types, `Line` and the tax signature *including its `TraceLevel` parameter*, container format, schema v1 with reserved shapes, `project()`/`simulate()` signatures, strategies-as-data plus the explanation contract, result pins) are decided slowly and early; everything else is allowed to be naive and is deepened in place. A seam is only useful if the escape hatches it will need are inside it before it freezes, which is why the two parenthetical additions above are part of the seam and not a later amendment. (3) Order: release spine and rate schedule (M0) -> exact single-year federal return plus vault (M1) -> this-year next-dollar waterfall (M2) -> lifetime ledger and the Roth-vs-Traditional verdict (M3) -> scenarios, full first-death, import, One-Page Plan (M4) -> Social Security (M5) -> Monte Carlo (M6) -> scorecard (M7) -> conversions, sequencing, claiming, ACA (M8) -> review loop and reallocation (M9) -> exact-state reference modules, 529 goals, connector freeze, review triage and hardening (M10), with a recurring two-week **parameter-vintage milestone (MV)** in each autumn publication window. **The life modules — `Property`, the per-person-year coverage state machine and the `disable` bundle with the insurance grid — are 1.1 by decision, not by contingency** (`PLAN.md` M10, §5, §7): each is a milestone's worth of work, and their schema shapes stay reserved in seam S5 so 1.1 is additive. (4) **Estimates are built per milestone as three separately counted classes — engine, UI, release-and-fixtures — and summed** (71 weeks nominal, published as a 71-85 week range), not corrected by a uniform multiplier, which was derived from engine-shaped estimates, made the front end invisible and was unfalsifiable. The **cut rule** is a ratio applied at every checkpoint (M2, M4, M6, M8 exit) at more than 20% cumulative overrun, re-baselining the remaining milestones and applying a **ranked five-tier cut list** (`PLAN.md` §4.12: OFX/QIF importers; the remaining progressive setup sections; the monthly claiming grid; the trade-list generator; the second exact-state module). Never cut: fixtures, the security suite, the release gate, pinning, the explanation invariant, **the current-year parameter vintage** and **the 529 option of capability (b)** (`PLAN.md` §1.1). (5) Stopping after any milestone leaves a coherent tool. (6) The vault rides alongside the first real domain milestone rather than occupying a standalone release. (7) Crates are created in the milestone that needs them.

**Alternatives considered.** Pure engine-first ordering (differentiator late; M0 with no number). A seven-week first milestone containing everything (not credible). Security-first ordering with eight weeks before any projection (violates the usable-slice requirement). Workflow-first with monitoring ahead of Monte Carlo (inverts the capability priority).

**Consequences.** The waterfall reaches users at about week 27 and the verdict at about week 34 (`PLAN.md` §4 marker dates); the committed core (M0-M8 plus both MV milestones) is 62 weeks. Signing and notarization exist from the first public release, so no household is ever asked to type its finances into an unsigned build.

---

## ADR-019. Roth-vs-Traditional: the verdict is gated on the ledger that supports it

**Context.** The decision is specified as running the tax function twice: marginal rate now versus the ledger's projected future marginal rate. The future base must include the Social Security torpedo, IRMAA, RMDs and the survivor's single-filer years. A future-rate estimate missing those is biased low in a knowable direction, which biases the recommendation toward Traditional invisibly. Shipping the answer early on an incomplete ledger, or behind a user slider, is the common failure; withholding, ranging and shipping-with-the-gap-printed are all arguable.

**Decision.** The **waterfall ships in M2** because match, statutory limits, hurdles, computed `deductible_fraction`, HSA payroll saving, liquidity classes and backdoor pro-rata need no future rates. In M2 the Roth-vs-Traditional card shows the **marginal rate now and the flip value** ("this reverses at a future marginal rate of X%") and **no verdict**; there is never a user slider presented as the answer. The **verdict ships in M3**, the same milestone in which RMDs, SS taxability, IRMAA on the t-2 clock and first-death survivor years all enter the ledger. The card shows the projected rate, the flip value, the with/without-survivor-years pair, and its **printed omissions** (ACA credits until M8; state at declared fidelity). Marginal is compared to marginal, never to average. Deepening in M4 (full first-death machine) and M5 (computed PIA) changes inputs, not the mechanism.

**Alternatives considered.** A computed verdict from an incomplete ledger with a disclaimer (a claim the engine cannot support). A user-set future-rate slider (not the specified mechanism). Delaying the whole next-dollar engine until the ledger is complete (delays the differentiator for no correctness gain).

**Consequences.** For one milestone the product's most-asked question gets a true sensitivity statement instead of an answer. That is principle 8 ("never claim more than the ledger can support") applied to the headline feature.

---

## ADR-020. Network posture: zero outbound by default, one HTTP-capable crate

**Context.** The privacy posture is a promise unless it is architectural. Future features (institution connectors, market-data refresh, update checks) need the network.

**Decision.** v1 makes **zero outbound connections**: no telemetry, update checks, fonts, CDNs or market-data calls; market-dependent defaults ship as dated values in assumption vintages and are user-overridable. **`pfp-net` is the only crate permitted an HTTP client**, enforced by `cargo-deny` bans everywhere else; every opt-in outbound feature is individually toggled, host-allowlisted and recorded in an **in-app network log**. An integration test runs the server under a deny-outbound sandbox profile. Future institution credentials resolve only through `SecretRef` to the macOS Keychain or the plan file's `secrets` section and never appear in API responses, logs or the change log.

**Alternatives considered.** Policy-only "we do not phone home" (unverifiable). Live yield fetches by default (breaks reproducibility and the posture). An embedded auto-updater (executes downloaded code; out of scope).

**Consequences.** No auto-update in v1; new law arrives as a new release. After 1.0, an opt-in signed update check and signed parameter packs go through `pfp-net`.

---

## ADR-021. Extension model: compiled-in traits and data; connectors produce drafts

**Context.** State modules, return generators, withdrawal rules, import formats and future connectors must be pluggable, inside a single signed executable that holds a decrypted plan.

**Decision.** Extension points are **compiled-in traits with a static registry, or declarative data** (`ContributionPolicy` rule lists, `DeclarativeState` modules, rebalancing policy). No dynamic loading, scripting or plug-in download. **Every connector, including the CSV/OFX/QIF file importers, produces a draft fact snapshot that the user reviews and merges; nothing writes facts directly.** The same seam serves file import in v1 and institution connectors later with no engine change. Parsers are bounded (size, depth, no XML entity expansion) and fuzzed with committed synthetic corpora.

**Alternatives considered.** Dynamic libraries or a scripting runtime (third-party code beside decrypted finances; breaks the single audit surface and notarization story). Importers writing facts directly (unreviewed data in the fact base; a different code path from future connectors).

**Consequences.** Contributions arrive by pull request through the same CI gates. The connector interface is exercised from the first import milestone and frozen at M10.

---

## ADR-022. Validation corpus, mutation budget and the AI-assistance protocol

**Context.** Trust comes from an automated validation corpus. The builder is a solo developer with AI assistance, whose dominant risk is plausible-but-wrong financial code and self-confirming tests. The ledger and the next-dollar engine have no published fixtures anywhere.

**Decision.**
- **Three tiers** (Tier 1 hard gate with line-level intermediates; Tier 2 pinned MIT/CC0/Apache suites transliterated by script; Tier 3 reviewed goldens and recorded oracle outputs, a change requiring an `engineVersion` bump).
- **Property and metamorphic invariants** as the ledger's primary control: conservation residual exactly zero, no-op patch, account split, row-order permutation, zero-volatility equals deterministic, homogeneity, tax monotone, uprating idempotent, CRN variance reduction, reconciliation invariant.
- **Mutation testing** (`cargo-mutants`) with a per-release surviving-mutant budget on `pfp-money`, `pfp-tax`, `pfp-ss`: a suite that does not notice `>=` becoming `>` at a bracket edge is not a suite.
- **Fuzzing** of the container header, patch/migration paths and every importer: committed synthetic corpora, 1 CPU-hour per parser per release.
- **Security suite** as a first-class corpus; **Playwright against the real binary** from M0 (security contract, six persona journeys, print snapshots, accessibility checks).
- **AI-assistance protocol**, addressing three distinct failure modes: (a) *self-confirming tests*: fixtures are written from the primary source before the implementation is requested; the assistant never authors both a constant and the test that checks it; a lint bans dollar literals in engine crates outside tests; (b) *silent ground-truth edits*: CODEOWNERS-style CI path checks block assistant edits to `fixtures/tier1/` and locked `params/` vintages; (c) *data exfiltration*: development and AI-assisted sessions use the synthetic demo plan only, and bug reports use a redacted diagnostics bundle.
- A **validation report** (fixture counts by tier, skips, oracle versions, mutation score, performance) is embedded in each release's About page. ASOP No. 56 is the per-release model-governance checklist. Folklore targets and the defective half of one published RMD example are refused in fixture notes.

**Alternatives considered.** Example-based tests only (cannot cover the ledger). Live oracle calls in CI (Python and copyleft installs on every run; flaky). Trusting review over mechanical controls (does not scale to AI-assisted volume).

**Consequences.** Fixture authoring is a standing cost in every milestone and is never cut. CI time grows; mutation and fuzz budgets run nightly and at release rather than per push.

---

## ADR-023. Repository data hygiene and user-agnostic documentation

**Context.** The repository is the application only: no user's information, profile, selections or results are ever committed. Only code, docs, public parameter tables with provenance and clearly labelled synthetic fixtures belong in it.

**Decision.** gitleaks pre-commit, in CI and nightly over full history, with custom rules (container magic bytes, access-URL and API-key patterns, Apple signing material); a commit hook rejecting any file that begins with the container magic; an **M0 release-gate scan under which a `.pfplan` container-magic byte sequence anywhere in the working tree or in a release archive fails the build**; a **data-hygiene linter** (any JSON/CSV/OFX/QIF/YAML or data TOML under version control must sit in `fixtures/` or `params/`; fixtures carry `synthetic: true` or a published-source citation; parameter files carry source, as-of date and checksum); `.gitignore` for `local/`, `*.pfplan*`, exports, `.env*`, signing material; persona fixtures generated from a seeded script so they are obviously not real; `docs/data-classification.md` derived from `SECURITY.md` §1's repo-safe vs encrypted-only table; logs never contain amounts, names or plan paths. **All documentation, fixtures, examples and commit messages are user-agnostic:** generic personas and synthetic figures only.

**The container-magic controls take no allowlist, ever.** The commit hook and the tree-and-archive scan are the one mechanical check that catches a real household file committed by accident, which is the whole of threat T5; a single allowlisted path blinds them permanently, and an allowlist is exactly what a committed demo container would force. So no `.pfplan` is committed, generated into the tree or shipped: the demo plan is **plaintext synthetic JSON** at `fixtures/plans/demo.plan.json` carrying `"synthetic": true` with its generator name and seed, the binary encrypts it into a container in the user's own directory at first use (ADR-005), and the reference decryptor's test containers in `tools/pfplan-ref/` are generated at test time into a temporary directory and deleted.

**Alternatives considered.** Policy without linters (fails silently). A private repository (does not remove the rule; removes the audience). Allowlisting one committed demo container so the build can embed it (rejected: it disables the only control that catches the failure it exists to catch, in exchange for saving one encryption call at first launch).

**Consequences.** Some friction adding fixtures; each needs a provenance block. The public artefacts that document the plan file are the schema, the JSON Schema, the golden migration fixtures and `demo.plan.json` — a plaintext JSON document, not a container.

---

## ADR-024. Reports are print routes; no PDF library

**Context.** The modern planning deliverable is a generated One-Page Plan regenerated at every review; printed output must be reproducible from pinned results.

**Decision.** `pfp-report` produces deterministic view-models pinned to a snapshot id; reports are routes with print stylesheets (`/report/one-page`, `/report/full`, `/report/review/:id`) printed to PDF by the browser. Exports and prints are recorded in the change log and carry their pins. Print snapshot tests run in CI.

**Alternatives considered.** An embedded PDF renderer (large dependency and licence surface inside the binary). Server-side HTML-to-PDF via a headless browser (a second runtime).

**Consequences.** Output fidelity depends on the user's browser print engine; snapshot tests pin the reference rendering. A re-rendered old snapshot reproduces identical numbers or reports the pin mismatch and shows the difference.

---

## Corrections of record

*Where an input to the design was wrong, or a flagged concern was left unanswered, the correction is recorded here once and the affected documents cite it. A correction is not an ADR: it changes no decision, it fixes a number or closes a question.*

**C1. The senior-deduction phase-out is a 6% rate, not "+6 points" of marginal rate.** The statutory rule (senior deduction 2025-2028, $6,000 per person 65+, 6% phase-out above MAGI $75,000 single / $150,000 MFJ; 26 USC 151(d)(5)) is easily read as raising the effective marginal rate by six percentage points. It does not. The deduction shrinks by 6 cents per dollar of MAGI per eligible person, so the EMR rises by `0.06 x n_eligible x bracket rate`. The corrected Tier-1 targets, which replace "senior phase-out +6 points" wherever it appeared:

| Case | Phase-out range (MAGI) | 22% bracket | 12% bracket |
|---|---|---|---|
| Single, one person 65+ | $75,000 - $175,000 | 22% -> **23.32%** | 12% -> **12.72%** |
| MFJ, two people 65+ | $150,000 - $250,000 | 22% -> **24.64%** | 12% -> **13.44%** |

The ranges are arithmetic on the values recorded in the project's internal research review (unpublished), not a new claim: `$6,000 / 0.06 = $100,000` of phase-out width per eligible person above the threshold, so a household with two eligible people sheds $12,000 over the same $100,000 at a combined 12%. Stacked with the Social Security torpedo the shape is `22% x 1.85 x 1.06` for one eligible person. The correction matters because a correct engine cannot reproduce a six-point step at all: the old target could only ever have been met by bending the engine to it, which is precisely the failure mode AI-assisted development makes cheap. The amounts and the 6% rate themselves are sourced to 26 USC 151(d)(5).

**C2. One tax-evaluation budget.** A single ledger-year gate can be stated three ways — one evaluation per ledger-year, a per-path-year maximum, a per-retired-year maximum — and a probe sized against the wrong one passes at M1 and fails at M6. The contract is recorded once:

- **A mean of at most two full `federal()` evaluations per accumulation ledger-year** (the federal-state two-pass, with the third pass skipped by the standard-deduction shortcut), and **a mean of at most three per retired ledger-year** (the two-pass plus one gross-up settle). These are means under a **hard per-year evaluation cap of six full `federal()` evaluations** — counting passes, secant steps, settles and outer cash-loop repeats alike, with the year marked `SHORTFALL` when the cap is reached — not per-year maxima; the cap is what bounds the tail, and the mean is what the gate measures. **`ENGINE-SPEC.md` §2.4 is the single owner of this contract** — the counts and the cap — and every other document cites it; `ARCHITECTURE.md` §4.3 owns only the wall-clock gates and the per-evaluation derivation below.
- The **binding gate is the ledger-year, not the kernel**: one simulated ledger-year in at most 27 microseconds of core time *including every tax call it makes*, benchmarked on the ledger-year. This is the 10,000 x 60 run in 2 s on eight cores, restated so that it cannot be met on paper by a kernel measured alone.
- The **M1 probe is re-based to about 1,500,000 `federal()` evaluations** (the budget above, averaged over a lifetime ledger) at the same wall-clock limits, reported separately for `TraceLevel::None` and `TraceLevel::Full`. Its role is diagnostic: it says which rung of ADR-007's ladder to pull.
- **Headroom, stated once.** At the probe's single-threaded limit (8 s over 1,500,000 evaluations, 5.33 microseconds each) the tax share of a ledger-year is **about 13.3 microseconds at the 2.5 mean — half of the 27-microsecond budget** — and about 16 microseconds in a retired year at its three-evaluation ceiling; the rest of the annual loop has the remainder (`ARCHITECTURE.md` §4.3, `TESTING.md` §10, `SIMULATION-SPEC.md` §9 carry the same arithmetic). A kernel that only just meets its number therefore passes, and the **ledger-year probe** (reported at M2, gating at M3) is the binding gate.
- **Rung 0 is a contingency, not scheduled M1 work.** Seam S3 fixes only what `TraceLevel::None` *returns* (`lines` empty, every typed accessor populated; `TESTING.md` I25). Whether the kernel internally builds and discards the 40-plus `Line` nodes on that path is what the two-mode probe measures, and rung 0 of ADR-007's ladder — making the `None` path construct none — is pulled **only if the M1 probe or the M3 ledger-year gate fails** (`PLAN.md` M1 and R5, `ARCHITECTURE.md` §4.3, `TESTING.md` §10), starting there because the `trace` parameter is in the S3 signature from M1 and pulling it reopens nothing. That is risk R5 behaving as designed: the ladder triggers at M1 or M3, while the kernel is still small enough to change, instead of at M6 with it written.
- The per-year budget, not the per-evaluation figure, is the gate: three evaluations at about 13 microseconds each against a 27-microsecond ledger-year would allot 148% of the budget to tax alone, which is why `SIMULATION-SPEC.md` §9 states the arithmetic in the per-year form.

**C3. Statutory uprating is computed from the base year, and is path-independent rather than associative.** `basis: IncreaseOverBase` rounds the *increase over the statutory base year*, once. Chaining a published figure forward year over year rounds twice and drifts off the published table, so the correct pipeline is `base_value x index_ratio` with the reduction applied a single time — which the parameter-table shape must be able to express **before seam S1 freezes at M0**, hence the `index_series`, `base_year` and `base_values` fields in ADR-010 and the addition of the index series and base-year amounts to M0 scope. The consequence for the test corpus: the invariant "uprating is associative across a two-step path" is **false** under this basis, and a correct implementation would fail it while a chained implementation passed. It is replaced by **path-independence** (every route from the base year to year N yields the same figure) plus a negative test asserting that a year-over-year chain *diverges* from the statutory result on at least one published 2026 threshold. The rule and the standard deduction's 2024 post-OBBBA base rest on 26 USC 1(f)(7) and 26 USC 63(c)(4); the bracket tables' 26 USC 1(f) base year and the identity of the index series are not stated by any source the project has archived, so those two are hand-verification-gate items read from statute before the M0 vintage locks (`TESTING.md` §5.2), never asserted from memory.

**C4. Section 121 applies the dollar limit to the eligible gain, not the nonqualified-use fraction to the capped amount** (`ENGINE-SPEC.md` §9). Under 26 USC 121(b)(5) the gain allocated to nonqualified use is ineligible and the $250,000 / $500,000 limit then applies to what remains; the project's internal research review (unpublished) transposed the two operations. Worked case: gain $800,000, MFJ, nonqualified-use fraction 0.2 -> eligible $640,000, exclusion **$500,000**, where the transposed order gives $400,000 and overstates taxable gain by $100,000. The module lands in 1.1 with `Property`; the correction is recorded now so the fixture is written the right way round.

**C5. Rule 1 of the debt comparison (term-matched Treasury after tax) binds for bondless households too** (`ENGINE-SPEC.md` §5.3). Rule 1 compares the after-tax debt rate with the term-matched Treasury yield after tax; the formulation "`E[r_equity] - ERP` when no bonds are held" dropped that comparison, and with the default vintage's 5% ERP it produced a risk-matched base of about 1.5% nominal — below the same vintage's own risk-free yields — so a 2.75% mortgage outranked a Treasury paying more after tax. `r_rm` therefore carries the after-tax safe yield as a **floor**, the `TaxableSafe` option sits inside the tiers 8-10 block, and the ERP default is vintage-implied (`E[r_equity] - y_10yr`) with the 5% preset `erp-5pct` (methodology per Kitces) (open decision 8).

---

## Open decisions (for the project maintainer to ratify)

1. **Requirement wording for the release form** (ADR-003): ratify that "one self-contained executable" permits the minimal `.app`-in-DMG notarization container around the byte-identical executable, pending the M0 stapling experiment.
2. **Apple Developer Program enrolment** before M0 starts, and custody of signing credentials and the Ed25519 release key.
3. **Firefox trust path** if it does not honour the user-domain root by default on current versions: documented manual step vs fingerprint-compare only. Closed by the M0 week-1 spike, whose outcome (including the CI trust-provisioning and per-PR / per-release browser split) is recorded in ADR-006 (`PLAN.md` M0, R25).
4. **Change-log retention default:** unbounded with compaction and purge (current decision) vs a capped undo history keeping the last change per day. **To be re-decided with data minimization as a stated input, not only plan-file growth:** every `reverseDiff` carries the prior value of every fact ever changed, so retention governs how much superseded financial history a captured file yields, not merely how large the file gets (ADR-009). Sub-questions: whether purge runs on a default schedule rather than only on demand, and whether a retention cap ships on by default.
5. **Which historical return datasets may be bundled**, after a redistribution-terms check; until then, loader only.
6. **Which two states become the reference `DeclarativeState` modules** at M10 (one flat, one graduated), chosen by contributor demand. **Washington is a third candidate on different grounds** (ADR-011): its rule is a single gains-threshold rate, so it is cheap to encode and it closes a real misreporting gap rather than demonstrating a shape; if it takes one of the two slots, the flat or graduated exemplar it displaces moves to 1.1.
7. **External security review:** reviewer, scope and budget. The review is **commissioned at M4 exit** (about nominal week 41), scoped to `pfp-vault` and `pfp-server`, with `docs/pfplan-format-spec.md` published at M1 as its input, so findings land in M6-M7 and M8-M10 are the remediation window; the zero-high gate stays at M10 (`PLAN.md` M4, R19). Because it is a third-party calendar dependency, this decision is closed **before M0 starts**, alongside decision 2.
8. **Default values that are preferences, not facts:** FI withdrawal-rate default (about 3.5% for long horizons), the equity-risk premium in the risk-matched test (**default vintage-implied, `E[r_equity] - y_10yr`, with the 5% preset `erp-5pct` (methodology per Kitces); editable 2-5%**, correction C5), the `r_u` horizon `H`, `min_survivor_years` on the Roth verdict (5), `chained_cpi_wedge` (0), heirs' tax rate (24%), survivor spending multipliers (0.70 core), solver success target (85%), post-retirement glide-path placeholder. All ship as visible, overridable assumptions; the defaults themselves may be revisited.
9. **Stable preferred port** vs always OS-assigned (password-manager origin matching vs predictability); current decision is preferred port with fallback.
10. **Cut-rule checkpoints** — ratify the rule as written in `PLAN.md` §4.12 and ADR-018: a 20% cumulative-overrun trigger evaluated at each of the M2, M4, M6 and M8 exits (a single check at M6 exit would observe nothing for nine months and then offer less relief than the overrun that fired it) and the five-tier ranked cut list. The life modules are **no longer part of this decision**: they move to 1.1 by decision (`PLAN.md` M10, ADR-018), so 1.0's definition of done does not depend on a cut firing.
