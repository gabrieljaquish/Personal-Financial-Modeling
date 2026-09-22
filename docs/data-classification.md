# Data classification

*Every asset the application handles, its class, where it may exist and where it may
never exist. `SECURITY.md` §1 is the **source of record**; this document restates its
table for contributors and is enforced mechanically by the data-hygiene linter
(`SECURITY.md` §13.3; `cargo xtask data-hygiene`). Where the two differ, `SECURITY.md`
wins and this file is wrong. Shipped at M0 (`PLAN.md` §4.1; ADR-023).*

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

| # | Asset | Class | May exist in | Never in |
|---|---|---|---|---|
| A1 | Household facts: persons, birth years, filing status, state, wages, balances, debts, basis and lots, property values, insurance face amounts, earnings records | Encrypted-only | The plan file's `plan` section; backend memory while unlocked | Repository, logs, browser storage, URLs, telemetry |
| A2 | Cross-year tax state: Roth ledger and five-year clocks, loss carry-forward, two years of MAGI, coverage status | Encrypted-only | The plan file's `plan` section | As A1 |
| A3 | Derived results: ledger rows, recommendations, explanations, snapshots, guardrail history, seeds, the change log, **and every superseded fact** (a change-log entry's reverse diff carries the prior value of everything ever changed) | Encrypted-only | `results`, `snapshots`, `changelog` sections | As A1 |
| A4 | Future institution credentials (an aggregator's setup token and access URL, market-data keys) | Encrypted-only, highest sensitivity | The `secrets` section, decrypted on demand, **or** the macOS keychain | API responses, the change log, network-log bodies, exports |
| A5 | Key material: the data key, slot keys, the passphrase, the recovery code, the TLS leaf key, the transient CA key | Secret, memory-resident | Memory under `secrecy` / `zeroize`; the keychain | Plaintext disk, core dumps, logs |
| A6 | Session credentials: the launch token, the `__Host-` cookie, `X-PFP-Proof` | Secret, ephemeral | Memory; the cookie jar; `sessionStorage` (the proof only) | Query strings, history, logs, `localStorage` |
| A7 | Public parameter tables with provenance, the JSON Schema, the OpenAPI document, the format specification, synthetic fixtures, the validation report | Repo-safe | The repository; embedded in the binary | — |
| A8 | Release signing material: the Developer ID identity, the Apple API key, the Ed25519 release key | Secret, out-of-band | A protected CI environment or maintainer custody | Repository, build logs, the software bill of materials |
| A9 | Exports and prints (CSV, JSON, PDF) | Plaintext, user-initiated | Wherever the user chooses | Created without a warning or a change-log entry |
| A10 | The redacted diagnostics bundle | Plaintext, user-initiated | A user-chosen destination, mode `0600` | Created without a warning and a change-log entry; written to a default or hidden path; containing any value, identifier or path |

**The blast radius of the plan file is its whole history, not its current state (A3).**
Because the change log is append-only and deletion is tombstoning, everything ever
entered remains inside the file until purged; the purge operation and its honest limit
(it cannot reach copies already made) are `SECURITY.md` §3.5 and `docs/threat-model.md`.

## What the repository may hold (A7), stated from the domain side

| Repo-safe | Encrypted-only |
|---|---|
| Tax tables, Social Security constants, Medicare and IRMAA tiers, capital-market assumptions, life tables and age-rating curves, each with a source URL, an as-of date and a projection rule (`params/`) | Balances, debts, basis, loan terms, cross-year tax state, earnings records, insurance specifics, seeds, stored results and every credential |
| Synthetic fixtures carrying `"synthetic": true`, or published third-party worked examples with a citation (`fixtures/`) | Any figure that describes a person or a household |
| The application's contracts: the OpenAPI document, the plan JSON Schema, the container format specification, the validation report | Anything derived from a real plan |

## How it is enforced

Policy without a linter fails silently, so every row above has a mechanical check
(`SECURITY.md` §13; `docs/contributing.md` §1.5):

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
