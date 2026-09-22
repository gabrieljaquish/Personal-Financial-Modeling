# Personal Financial Modeling

A self-hosted **financial planning application** that mirrors what professional
financial-planner software does. Download a release, enter your own information, and run it
locally. Nothing about any user ships with the code.

The focus is **planning, not tracking**. Budgeting and account-tracking tools already answer
"what do I have today?". This application answers forward-looking questions:

- Where should the next dollar of savings go — 401(k), Roth vs Traditional, IRA, HSA, taxable, or debt?
- How can lifetime taxes be minimized — bracket management, Roth conversions, asset location?
- Pay down debt or invest?
- How should investments be reallocated, and what does that cost in taxes?
- What does "enough saved" look like, and how confident can you be? (Monte Carlo / probabilistic modeling)

## Status

Design complete; implementation has started with the M0 scaffold ([PLAN.md](docs/PLAN.md) §4.1).
See the design documents below.

## Design documents

The build plan and specifications live in [`docs/`](docs/). Suggested reading order:

1. [PLAN.md](docs/PLAN.md) — vision, principles, milestone roadmap (M0–M10), risks, definition of done
2. [ARCHITECTURE.md](docs/ARCHITECTURE.md) — stack, components, repository layout, engine API, release pipeline
3. [DECISIONS.md](docs/DECISIONS.md) — architecture decision records
4. [DOMAIN-MODEL.md](docs/DOMAIN-MODEL.md) — plan-file schema, scenarios as diffs, parameter tables and vintages
5. [ENGINE-SPEC.md](docs/ENGINE-SPEC.md) — annual ledger, federal tax function, next-dollar engine, Roth decisions
6. [SIMULATION-SPEC.md](docs/SIMULATION-SPEC.md) — return generators, Monte Carlo, the "enough" scorecard, Social Security, portfolio
7. [SECURITY.md](docs/SECURITY.md) — threat model, encrypted plan file, TLS on loopback, repository hygiene
8. [TESTING.md](docs/TESTING.md) — validation corpus, oracles, invariants, CI gates

## How it is meant to work

1. Download a release (or build from source).
2. On first run, a setup flow collects your household, accounts, income, goals and assumptions.
3. Everything you enter is stored in an **encrypted file on your own machine**.
4. Explore projections, scenarios and recommendations in a browser UI served locally.

## Development

The design documents are the specification. [`docs/contributing.md`](docs/contributing.md)
collects the rules that apply before a first commit: synthetic data only, the no-allowlist
container-magic rule, the AI-assistance protocol and the licence boundary.

### Prerequisites

| Tool | Version | Notes |
|---|---|---|
| Rust | the channel pinned in [`rust-toolchain.toml`](rust-toolchain.toml) | With `rustup` the pin applies automatically and the `wasm32-unknown-unknown` target is installed with it. See "What only CI can enforce" below |
| Node and npm | the exact version in `web/.nvmrc` (CI uses it; `engines` in `web/package.json` is the accepted range) | A build-time tool only; no Node runtime ships in the binary |
| [lefthook](https://github.com/evilmartians/lefthook) | — | Runs the pre-commit gates |
| [gitleaks](https://github.com/gitleaks/gitleaks) | — | Secret scanning, pre-commit and in CI |
| `cargo-deny`, `cargo-audit` | — | `cargo install cargo-deny cargo-audit`; they land in `~/.cargo/bin`, which must be on `PATH` |

### Build and test

```sh
cargo build --workspace
cargo test --workspace
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo deny check            # advisories, licence allowlist, bans, sources
cargo audit
```

### Repository hygiene gates

Each gate exits 0 when clean and 1 on a violation. They run pre-commit and in CI; run them
directly at any time:

```sh
cargo xtask check-magic              # the .pfplan container magic anywhere in the working
                                     # tree, git-ignored files included (or in the given
                                     # files, directories or archives). No allowlist, ever.
cargo xtask check-magic --history    # the same, over every blob in the git history
cargo xtask lint-dollars             # dollar literals in engine crates outside tests
cargo xtask data-hygiene             # data files only under fixtures/ or params/; synthetic
                                     # markers or citations; provenance; locked vintages
cargo xtask protected-paths <path>…  # edits to fixtures/tier1/ or a locked params/ vintage
                                     # (paths as arguments, or one per line on stdin)
```

`cargo xtask` with no arguments prints the full command list, including the build helpers that
later steps of M0 implement.

### Git hooks

```sh
lefthook install
```

Install once after cloning. The hook set is declared in `lefthook.yml`.

### Web build

```sh
cd web
npm ci --ignore-scripts
npm run build
```

`--ignore-scripts` is not optional: npm lifecycle scripts do not run in this repository.
`cargo xtask build-web` will wrap these commands (with sorted, hashed output for the embedded
build) later in M0; until then, run them directly.

### What only CI can enforce

- **The toolchain pin.** `rust-toolchain.toml` is honoured wherever `rustup` is present. A
  Homebrew-only Rust ignores it, so a local build may use a different compiler; CI is
  authoritative for the version the release is built with.
- **The `wasm32-unknown-unknown` purity build**, which proves that engine crates touch no
  filesystem, network, clock, environment or entropy (determinism rule D1,
  [ARCHITECTURE.md](docs/ARCHITECTURE.md) §4.3). It needs a `rustup`-installed target, so it is
  a CI-only job.

## Design principles

**The repository is the application only.** No user's information, profile, selections or
results are ever committed. Everything personal is a runtime input.

**Privacy model (non-negotiable).**

- No financial information is ever stored in this repository — no balances, holdings,
  transactions or plan parameters, and no keys, tokens or passwords for financial institutions.
- User data lives only in a local encrypted store, encrypted **at rest and in transit**
  (including browser ↔ local backend).
- Institution credentials (for the later account-connection phase) live in the OS keychain or
  the encrypted local store — never in the repo, never in env files inside the repo tree.
- The repo contains only code, docs, and clearly-labeled **synthetic** fixtures.
- `.gitignore` blocks data directories, database/encrypted files, finance export formats and
  secret material. A secret scanner (gitleaks) is run before commits.

**Transparent assumptions.** Every rate, table and default is visible, sourced and overridable.

## Planned capabilities

| Area | Direction |
|---|---|
| Households | Single or two-person households; ages, filing status, retirement dates and survivor scenarios are user inputs |
| Taxes | US federal + state (user selects the state) |
| Accounts | 401(k)/403(b), Traditional and Roth IRA, HSA, 529, taxable, cash, debts |
| Modeling | Year-by-year projection, side-by-side scenarios, Monte Carlo |
| Data input | Manual entry and CSV/OFX import first; direct institution connections later through a connector interface |
| Front end | Web app in the browser (Vite + React + TypeScript), served by the local process |
| Backend | Rust — one self-contained executable that runs the local server and the modeling engine |
| Platform | macOS first; code kept portable to other operating systems |

> Nothing in this repository is financial advice.
