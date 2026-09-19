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
                                     # markers or citations; provenance; locked vintages;
                                     # and everything lint-server checks
cargo xtask lint-server              # std::fs only in pfp-vault/pfp-app and std::net only in
                                     # pfp-server, across every workspace crate; no
                                     # credential-shaped flag and no unlisted environment
                                     # read anywhere in pfp-app, tests included
cargo xtask protected-paths <path>…  # edits to fixtures/tier1/ or a locked params/ vintage
                                     # (paths as arguments, or one per line on stdin)
```

`cargo xtask` with no arguments prints the full command list, including the build helpers
(`build-web` is implemented; the others arrive with later steps of M0).

### Git hooks

```sh
lefthook install
```

Install once after cloning. The hook set is declared in `lefthook.yml`.

### Web build

```sh
cargo xtask build-web                  # npm ci --ignore-scripts, then npm run build
cargo xtask build-web --skip-install   # when web/node_modules is already in place
cd web && npm run build                # typecheck, npm test, vite build, record the input hash
cd web && npm test                     # the handshake tests, the storage lint, the stamp test
```

`--ignore-scripts` is not optional: npm lifecycle scripts do not run in this repository.
`npm run build` ends by recording a hash of the front-end inputs beside the bundle
(`web/dist/.dist-stamp`, written by `web/scripts/write-stamp.mjs`), so either command leaves a
tree that builds in release. `build-web` additionally checks that `web/dist` is `index.html` plus
content-hashed files under `assets/`, lists them in sorted order with their SHA-256, and
recomputes the input hash in Rust: it fails if the recorded one differs. `pfp-server` embeds
`web/dist` into the executable at compile time:

- **debug build, no `web/dist`**: the build warns and the server serves a built-in placeholder
  page, so every Rust test passes on a checkout with no Node installed;
- **release build, `web/dist` absent or older than its inputs** (`web/src`, `index.html`, the
  manifest and lockfile, the compiler and bundler configuration; running `vite build` by hand
  records no input hash): the build **fails** and says to run `cargo xtask build-web`.

### Run it

```sh
cargo xtask build-web --skip-install
cargo run -p pfp-app -- serve --port 0 --no-open --no-trust --state-dir "$(mktemp -d)"
```

The process prints one machine-readable line and then prose, and serves until Ctrl-C:

```text
PFP-READY origin=https://127.0.0.1:<port> fingerprint=SHA256:<AA:BB:…> trust=declined open=skipped
```

| Flag | Meaning |
|---|---|
| `--port N` | Prefer port `N` instead of the usual port, `47443`. Whichever is preferred, the occupied-port probe runs at every launch: if the port is taken the run warns first and only then continues on an OS-assigned port. `--port 0` asks for an OS-assigned port from the start, so nothing can be found occupied; every test and scripted run uses it |
| `--no-open` | Do not open a browser |
| `--no-trust` | Do not read or change any trust setting; the certificate fingerprint is printed for manual comparison. Wins over `--install-trust` |
| `--install-trust` | The explicit opt-in without which nothing is ever installed into, or removed from, the trust settings |
| `--state-dir DIR` | Where the single-instance lock and the local certificate live (default `~/Library/Application Support/pfp`); created `0700`, the key file `0600` |

`pfp --version`, `pfp --help` and `pfp openapi` (prints the OpenAPI document) start nothing.
`pfp trust remove --install-trust` removes the stored certificate and its trust setting.

What to expect at this step of M0:

- The listener is `127.0.0.1` only and TLS only; there is no plaintext mode in any profile.
- **No build in this repository opens a browser, shows an alert or touches a trust setting yet.**
  The platform seam exists as traits; the macOS implementation arrives with the M0 trust spike.
  Every run therefore behaves as `--no-open --no-trust`, the browser shows a certificate warning
  for the local certificate, and the fingerprint on the `PFP-READY` line is what to compare.
- The session is established from a launch token that the launcher hands to the browser inside
  the URL fragment, in-process. The token is never printed and cannot be passed by flag,
  environment variable or file, so a page opened by typing the address shows "Not connected".
  That is the designed behaviour, not a fault.

### The usual port

A run with no `--port` prefers `47443` (`PREFERRED_PORT` in `crates/pfp-app/src/cli.rs`). That is
the current decision recorded as open decision 9 in [DECISIONS.md](docs/DECISIONS.md) and the
proposed resolution in [SECURITY.md](docs/SECURITY.md) §6.3 and §16: a stable origin is what lets a
browser and a password manager recognise this application, and a probe at every launch is what
turns "another program took that address" into a warning instead of a silent move. The number
itself is this step's choice — in the registered range and below 49152, where the macOS ephemeral
range starts, so the operating system never assigns it to another program by itself — and it has
not been checked against the IANA registry; it is the maintainer's to ratify with open decision 9.
A second copy of this application started with another `--state-dir` finds the port occupied and
warns too; the warning is conservative by design.

### API methods

The five M0 operations are `POST`, and that is their contract rather than a placeholder. None of
them appears in [ARCHITECTURE.md](docs/ARCHITECTURE.md) §5 under another method; each is an
RPC-shaped operation; the one that takes financial input carries it in a body because URLs carry
opaque ids only; and [SECURITY.md](docs/SECURITY.md) §7.2 requires an exact `Origin` on every
`/api/**` request, which a browser sends on a `POST` and omits on a same-origin `GET`.

What remains is a disagreement about endpoints that do not exist yet: ARCHITECTURE.md §5 draws
the M2 run reads as `GET`, which §7.2 as written would refuse. [PLAN.md](docs/PLAN.md) ("Reading
order") settles which document gives way — the spine wins over a specification — so this is an
erratum against SECURITY.md §7.2's `Origin` row, to be amended by the maintainer in the pull
request that adds the first `GET`. That pull request flips the one constant `API_READ_RULE` in
`crates/pfp-server/src/admission.rs` to `SafeMethodsWithFetchMetadata` (already written and
unit-tested: still `Sec-Fetch-Site: same-origin`, still the session pair, still an exact `Origin`
whenever one is present) and adds `get` operations to the OpenAPI document. It does not move the
five operations that exist, so the snapshot and the generated front-end types for them are stable.

### Security test ids in this step

[TESTING.md](docs/TESTING.md) §8 numbers the security tests. For the server half of M0:

| Id | What it asserts | Where, or why not here |
|---|---|---|
| S-01 | `Host` 421, `Origin` 403, route-scoped `Sec-Fetch-Site` | `crates/pfp-server/src/admission.rs` (unit), `crates/pfp-server/tests/admission.rs` (over real TLS) |
| S-02 | Plaintext HTTP fails; no plaintext acceptor in any profile | `crates/pfp-server/tests/tls_only.rs`, `crates/pfp-server/tests/source_rules.rs` |
| S-03 | Loopback-only self-check | `crates/pfp-server/src/bind.rs`, `crates/pfp-server/tests/tls_only.rs` |
| S-04 | Exact response-header set | `crates/pfp-server/tests/headers.rs` and its snapshot. "Zero CSP violations across a journey" needs a browser: deferred to the Playwright suite |
| S-05 | Browser storage empty but for the proof token | Static half: `web/tests/no-storage.test.mjs`, which `SECURITY.md` §7.4 calls a first line and not the control. Runtime half: deferred to the Playwright suite, because only a real browser has the storage to inspect |
| S-06 | Zero third-party requests | Static half: `crates/pfp-server/src/csp.rs` and `crates/pfp-server/tests/assets.rs`. Runtime half: deferred to the Playwright suite (needs a browser's network log) |
| S-07 | Deny-outbound sandbox, offline first launch | `crates/pfp-app/tests/sandbox.rs` |
| S-08 | Launch token single-use and 60 s; cookie-only and proof-only rejected | `crates/pfp-server/src/session.rs`, `crates/pfp-server/tests/session.rs`, `web/tests/handshake.test.mjs` (the client half). Two clauses wait for M1, because they need a plan to exist: a stolen launch token reaches no plan endpoint, and a bootstrap while unlocked forces a re-unlock. The `on_session_replaced` hook is their seam |
| S-09 | Container magic-byte gate | Already in place: `cargo xtask check-magic` |
| S-10, S-11 | Supply-chain gates, Actions pinning, release key; double build, SBOM, signing | Not this step: the M0 release-pipeline step owns them |
| S-12 | Type-aware export escaping | Not this step: scheduled in M0 with the export helper, which is not part of the server |
| S-20 | `RLIMIT_CORE=0`, `Redacted<T>`, silent panic hook | `crates/pfp-app/src/harden.rs`, `crates/pfp-app/tests/panic_hook.rs`, `crates/pfp-server/src/redact.rs` |
| S-28 | Displaced `__Host-pfp` cookie gives the documented 409, not a 401 loop. **The one-click recovery leg is not reachable yet** — see the note below this table | `crates/pfp-server/src/session.rs`, `crates/pfp-server/tests/session.rs`, `crates/pfp-server/tests/headers.rs` |
| S-29 | Occupied preferred port warns before anything opens | `crates/pfp-app/src/launch.rs` (the warning, the order, and nothing opened when the warning cannot be shown), `crates/pfp-app/src/cli.rs` (a bare run prefers the usual port, so the probe runs at every launch). The tests occupy an OS-assigned port and prefer it with `--port`; none binds the usual port on the machine running them. **Known limitation:** "occupied" means *our bind failed*, so a program listening on the wildcard address (`0.0.0.0:<port>` with `SO_REUSEADDR`) raises no warning; it also receives no loopback connection while this application runs, which is why it is documented (`bind_loopback` in `crates/pfp-server/src/bind.rs`) rather than probed for |

**S-28, the recovery leg.** [SECURITY.md](docs/SECURITY.md) §7.1 specifies the displaced-cookie
answer as a distinguishable 409 *plus* a one-click path that asks the still-running backend for a
fresh launch token and a re-open. The 409, its stable code `session_cookie_displaced`, and the
server half of the recovery (`POST /api/v1/session/relaunch`, its interval throttle
`relaunch_throttled` and its separate per-session cap `relaunch_exhausted`) ship in this step and
are tested. The click itself does not exist yet, for two reasons: there is no screen to put the
button on until the UI pull request, and the in-process browser opener is
`Platform::unsupported()` in every build this step produces, so `relaunch` answers
503 `open_unavailable`. Until both land, a user who meets the 409 recovers by quitting the
application and starting it again from its launcher.

One [SECURITY.md](docs/SECURITY.md) control placed at M0 has no S-id and is **deferred**, so it is
recorded here rather than left silent:

| Control | State in this step | Why, and where it lands |
|---|---|---|
| §8: anything reaching the system log goes through `os_log` with every interpolated value `%{private}` | Not implemented. The in-memory ring buffer is the log (`crates/pfp-server/src/events.rs`); `warn` and `error` events are echoed to stderr, and the `PFP-READY` line and its prose go to stdout. What they can contain is fixed by type: event codes, route templates, status codes, the origin and the certificate fingerprint — never a token, a header value or a path as sent | `os_log` is a C interface, so it needs `unsafe` FFI or a vetted binding; `pfp-server` and `pfp-app` are both `#![forbid(unsafe_code)]`, and the place for it is the macOS platform module behind the `pfp-app` platform seam, which does not exist yet. The exposure §8 describes arises when `launchd` captures stdout and stderr in the `.app` channel, and that channel arrives with the signing block. It lands with the macOS platform module, before the first `.app` build; until then every run is from a terminal, where stderr is the terminal |

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
