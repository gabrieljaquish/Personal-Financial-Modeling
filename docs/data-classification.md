# Data classification

*Every asset the application handles, its class, where it may exist and where it may
never exist. `SECURITY.md` §1 is the **source of record**; this document restates its
table for contributors, word for word, and a test in `xtask` (`cargo test -p xtask`,
run by every CI `rust` job) fails when the two tables differ, so the restatement cannot
drift. Where they ever differ anyway, `SECURITY.md` wins and this file is wrong. The
rows a linter can check today are checked by the data-hygiene linter (`SECURITY.md`
§13.3; `cargo xtask data-hygiene`); the rest are listed below as rules, not gates.
Shipped at M0 (`PLAN.md` §4.1; ADR-023).*

## The rule in one sentence

**The repository is the application only.** Household facts, derived results,
credentials and key material exist only inside the encrypted plan file or in the
application's memory while it is unlocked; public parameter tables with provenance,
synthetic fixtures and the application's own contracts are the only data the
repository holds (ADR-023; `docs/contributing.md` §1).

## Classes

| Class | Meaning |
|---|---|
| **Encrypted-only** | Exists in plaintext only inside the application's process while a plan is unlocked; at rest only inside the `*.pfplan` container. Never in the repository, a log, browser storage, a URL or telemetry |
| **Secret, memory-resident** | Key material and session credentials: held under `secrecy` / `zeroize` in memory, or in the keychain; never written to plaintext disk, a core dump or a log |
| **Secret, out-of-band** | Release signing material: in maintainer custody or a protected CI environment, never in the repository, a build log or the software bill of materials |
| **Repo-safe** | Public data with provenance, and the application's own contracts: may be committed and embedded in the binary |
| **Plaintext, user-initiated** | Artifacts the user deliberately writes out of the application, always behind a warning and a change-log entry |

## Assets (`SECURITY.md` §1)

The table is `SECURITY.md` §1 verbatim. Its abbreviations, for a contributor arriving
here first: DEK, the random data-encryption key of a plan file; KEK, the key-encryption
key derived from a passphrase or recovery code, one per slot; `.p8`, the Apple API key
file; SBOM, the software bill of materials; "(8)" points at `SECURITY.md` §8, which
specifies the diagnostics bundle; "SimpleFIN" is the likeliest first aggregator candidate
(`SECURITY.md` §10, post-1.0), and the row applies to any aggregator's token and access
URL.

| # | Asset | Class | May exist in | Never in |
|---|---|---|---|---|
| A1 | Household facts: persons, birth years, filing status, state, wages, balances, debts, basis and lots, property values, insurance face amounts, earnings records | Encrypted-only | Plan-file `plan` section; backend memory while unlocked | Repo, logs, browser storage, URLs, telemetry |
| A2 | Cross-year tax state: Roth ledger and five-year clocks, loss carryforward, two years of MAGI, coverage status | Encrypted-only | Plan-file `plan` | as A1 |
| A3 | Derived results: ledger rows, recommendations, explanations, snapshots, guardrail history, seeds, change log — **and every superseded fact**, because a change-log entry's `reverseDiff` carries the prior value of everything ever changed and snapshot bodies carry the facts as they stood | Encrypted-only | `results`, `snapshots`, `changelog` | as A1 |
| A4 | Future institution credentials (SimpleFIN setup token and Access URL, market-data keys) | Encrypted-only, highest sensitivity | `secrets` section (on-demand) **or** macOS Keychain | API responses, change log, network-log bodies, exports |
| A5 | Key material: DEK, slot KEKs, passphrase, recovery code, TLS leaf key, CA key (transient) | Secret, memory-resident | Memory under `secrecy`/`zeroize`; Keychain | Plaintext disk, core dumps, logs |
| A6 | Session credentials: launch token, `__Host-` cookie, `X-PFP-Proof` | Secret, ephemeral | Memory; cookie jar; `sessionStorage` (proof only) | Query strings, history, logs, `localStorage` |
| A7 | Public parameter tables with provenance, JSON Schema, OpenAPI document, format spec, synthetic fixtures | Repo-safe | Repository; embedded in the binary | — |
| A8 | Release signing material: Developer ID identity, Apple API `.p8`, Ed25519 release key | Secret, out-of-band | Protected CI environment / maintainer custody | Repo, build logs, SBOM |
| A9 | Exports and prints (CSV/JSON/PDF) | Plaintext, user-initiated | Wherever the user chooses | Created without a warning or a change-log entry |
| A10 | Redacted diagnostics bundle (8) | Plaintext, user-initiated | A user-chosen destination, mode 0600 | Created without a warning and a change-log entry; written to a default or hidden path; containing any value, identifier or path |

**The blast radius of the plan file is its whole history, not its current state (A3).**
Because the change log is append-only and deletion is tombstoning, everything ever
entered remains inside the file until purged; the purge operation and its honest limit
(it cannot reach copies already made) are `SECURITY.md` §3.5 and `docs/threat-model.md`.

## What the repository may hold (A7), stated from the domain side

| Repo-safe | Encrypted-only |
|---|---|
| Tax tables, Social Security constants, Medicare and IRMAA tiers, capital-market assumptions, life tables and age-rating curves, each with a source URL, an as-of date and a projection rule (`params/`) | Balances, debts, basis, loan terms, cross-year tax state, earnings records, insurance specifics, seeds, stored results and every credential |
| Synthetic fixtures carrying `"synthetic": true`, or published third-party worked examples with a citation (`fixtures/`) | Any figure that describes a person or a household |
| The application's contracts: the OpenAPI document, the plan JSON Schema, the container format specification | Anything derived from a real plan |

## How it is enforced

Policy without a linter fails silently, so every row that can be checked mechanically
today is (`SECURITY.md` §13; `docs/contributing.md` §1.5). A4, A8, A9 and A10 have no
mechanical check yet: each arrives with the feature it classifies (`SECURITY.md` §14: the
`Connector` / `SecretRef` seam at M10, the release signing pipeline in the M0
release-pipeline step, the export and print warning at M4, the diagnostics bundle at M4),
and until then the row is a rule a contributor follows, not one a gate enforces. The
checks that exist:

- `cargo xtask data-hygiene`: any `.json`, `.csv`, `.ofx`, `.qif`, `.yaml`, `.yml` or
  data `.toml` under version control must sit in `fixtures/` or `params/`; every fixture
  carries the synthetic marker or a citation; every parameter table carries provenance,
  an as-of date, a projection rule and a rounding rule; locked vintages are unchanged;
  no SSN-shaped string, plan-shaped JSON or `asOf`-stamped fact file outside its place;
  no credential-shaped flag or environment read in the launcher.
- `cargo xtask check-magic`: the `.pfplan` container magic anywhere in the working tree,
  or in a release archive, fails the build. **No allowlist, ever** (ADR-023).
- gitleaks, pre-commit, in CI and nightly over full history, with the project's custom
  rules (`.gitleaks.toml`).
- `cargo xtask protected-paths`: `fixtures/tier1/` and locked `params/` vintages are
  ground truth and never an assistant's to edit (ADR-022).
- `.gitignore` excludes `local/`, every `*.pfplan*`, exports, `.env*`, key and signing
  material (`SECURITY.md` §13.1).

A contributor who is unsure which class something is in should treat it as
encrypted-only and ask: the cost of a wrong "repo-safe" is permanent.

## Proposed errata to `SECURITY.md` §1

Two statements in the source of record do not match the tree. Neither is corrected here,
because a design document is a maintainer's to change (`docs/contributing.md` §3); each
is proposed as text to paste, in the form `docs/verification/m0-design-errata.md` uses,
and belongs in that file as its next entries.

1. **"is generated from this table."** No generator exists: `cargo xtask` has
   `validation-report` and `assumption-catalogue`, each with `--check`, and nothing for
   this document. What exists is a restatement held equal to the source by a test.
   Proposed wording for `SECURITY.md` §1, first sentence: "`docs/data-classification.md`
   (M0) restates this table, and a test fails when the two differ; the data-hygiene
   linter (13.3) enforces the rows it can."
2. **A7 does not name the validation report.** `docs/validation-report.md` is committed
   and `build/validation-report.json` is embedded in the binary (`PLAN.md` §4.1;
   `TESTING.md` §13), so it exists in exactly the two places A7 allows and nowhere else;
   it carries counts, never a household figure. Proposed wording for the A7 asset cell:
   "Public parameter tables with provenance, JSON Schema, OpenAPI document, format spec,
   synthetic fixtures, the validation report". Until a maintainer accepts it, this
   document does not add the report to A7 on its own authority, and an earlier draft of
   this file that did so was wrong by its own rule.
