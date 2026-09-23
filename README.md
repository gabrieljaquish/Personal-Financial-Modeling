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

Design complete; implementation is inside milestone M0 ([PLAN.md](docs/PLAN.md) §4.1): the
scaffold, the rate-schedule engine slice, the local TLS server and the first two screens (rate
schedule, Assumptions Registry) exist. **No screen has been run in a real browser yet** — see
"What is not yet verified in a real browser" under Development. See the design documents below.

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
| `cargo-cyclonedx` | exactly 0.5.9 | SBOM only: `cargo install --locked --version 0.5.9 cargo-cyclonedx`. `cargo xtask sbom` refuses any other version. `cargo-auditable` is deliberately **not** used: see [`packaging/README.md`](packaging/README.md) |

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
cargo xtask validation-report --check
                                     # docs/validation-report.md (and build/validation-report.json
                                     # if generated) equal a fresh build of the validation report
cargo xtask assumption-catalogue --check
                                     # docs/assumption-catalogue.md equals a fresh render of params/
```

`cargo xtask` with no arguments prints the full command list, including the build helpers
(`build-web`, `validation-report`, `assumption-catalogue` and the unsigned release commands
below are implemented; `schema` and `openapi` arrive with later steps of M0).

### Validation report and assumption catalogue

```sh
cargo xtask validation-report        # build/validation-report.json + docs/validation-report.md
cargo xtask assumption-catalogue     # docs/assumption-catalogue.md from params/
```

The validation report (`docs/TESTING.md` §13; `docs/PLAN.md` §4.13 item 8) is a pure
function of the repository: fixtures by tier, milestone and verification value; every
parameter vintage with its content id, lock state (verified against the files: a lock entry
counts only when the file still hashes to it, and a vintage is locked only when every file in
its directory is listed), verification status and archived-source
checksums; the property tests, contract snapshots and security test ids counted from the
source tree; and, for every corpus the design specifies but the tree does not hold yet, an
explicit "not yet introduced (milestone Mx)" entry. No clock, host name, path or person
enters it; a date appears only when given with `--generated-on YYYY-MM-DD`. The model is one
Rust file (`crates/pfp-server/src/api/report_dto.rs`) compiled into both `xtask` (the writer)
and `pfp-server` (the reader), and every struct refuses unknown fields.

The JSON is written under git-ignored `build/` because the data-hygiene gate reserves `.json`
documents for `fixtures/` and `params/`; `pfp-server` embeds it at compile time exactly as it
embeds `web/dist`:

- **debug build, no `build/validation-report.json`**: the build warns and
  `POST /api/v1/validation/report` answers `state: not-generated` with no report, which the
  About page says in its first sentence;
- **release build, report absent**: the build **fails** and says to run
  `cargo xtask validation-report`.

`docs/validation-report.md` is the committed rendering; `--check` (in CI's hygiene job and in
the `xtask` test suite) fails when it, or a generated JSON, differs from a fresh build. The
assumption catalogue is rendered from `params/` through the `pfp-params` loader, so it cannot
name a parameter id the engine would not load, and is checked the same way.

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
cd web && npm test                     # node:test: every view in every state, the client, the lints
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
| `--state-dir DIR` | Where the single-instance lock and the local certificate live (default `~/Library/Application Support/<application identifier>`, the placeholder identifier in `crates/pfp-app/src/identity.rs`); created `0700`, the key file `0600` |

`pfp --version`, `pfp --help` and `pfp openapi` (prints the OpenAPI document) start nothing.
`pfp trust remove --install-trust` removes the stored certificate and its trust setting.

What the front end does at this step of M0 (details in [`web/README.md`](web/README.md)):

- **Rate schedule** (`#/rate-schedule`): filing status, tax year and taxable income in whole
  dollars give the rate-schedule tax with every line explained — the amount in each bracket, the
  tax from each bracket, the total, what each line was computed from, the parameters it read and
  its rounding rule — as a worksheet table that prints (print CSS keeps the inputs, the result and
  the verification notice; it drops the form and the buttons). The browser computes nothing: every
  figure is an integer number of cents from the API, formatted as text.
- **Export**: "Download CSV" and "Download JSON" call `POST /api/v1/tax/rate-schedule/export`. The
  exporter and its spreadsheet formula-injection escaping are in Rust
  (`crates/pfp-server/src/api/export.rs`): money, integers and dates are bare (`-12.34` stays a
  number); text is always quoted, `"` doubled, and prefixed with one apostrophe inside the quotes
  when its first character — looking past leading whitespace and U+FEFF — is `=`, `+`, `-`, `@`, tab
  or carriage return, or when it already begins with an apostrophe (so the inverse is exact);
  records end in CRLF, UTF-8 without a byte-order mark; the JSON export carries the API's values
  verbatim, pretty-printed with one trailing newline, and is never escaped (the bytes differ from
  the compact API body; the values do not, which is what the tests compare). An export **fails
  closed**: a body that cannot be serialized is the API's standard 500 error body, never a 200
  with an empty attachment. The **filename has one source**: the server's `Content-Disposition`
  (`rate-schedule.csv`, `rate-schedule.json`). The front end saves the file under that name after
  validating the header against a strict pattern (lowercase `[a-z0-9-]` stem, the extension of the
  format it asked for), and falls back to the literal `export.<ext>` otherwise; it composes no name
  of its own. A **plain warning precedes both ways out** — the two downloads and Print — and says
  that a downloaded file or a printed page contains the taxable income that was entered
  (`ARCHITECTURE.md` §5, `SECURITY.md` §9). The Assumptions Registry is not exportable yet (it is nested, and it
  already exists as TOML under `params/`).
- **Assumptions Registry** (`#/assumptions`, `#/assumptions/<parameter id>`): every parameter table
  and index series with its sources (title, publisher, URL as text, retrieval date, locator,
  archived-copy path and SHA-256), as-of date, vintage, projection rule, rounding rule and open
  items. **Verification is shown as it is**: the shipped vintage is unlocked and pending human
  verification, a banner on every screen and beside every result says so ("not for decisions"),
  it prints, and it does not disappear when the status cannot be read — an unreadable status is
  reported as unverified.
- **Session states**: connected; not connected (no launch token, a used or expired one, a session
  that ended, an application that is not answering); and a **displaced session** — the 409
  `session_cookie_displaced` on any call replaces the screens with a notice and one button,
  "Re-open from the application", which posts once to `session/relaunch`. 202 tells the user to
  continue in the new tab; 429 `relaunch_throttled` says to wait and keeps the button; 429
  `relaunch_exhausted` and 503 `open_unavailable` say to quit and start again and **remove** the
  button. There is no timer, retry loop or automatic click.
- Threshold projection "under an editable inflation assumption" (PLAN.md §4.1) is **not built**: no
  API operation accepts an inflation assumption. The same clause is the subject of erratum **E7** in
  [docs/verification/m0-design-errata.md](docs/verification/m0-design-errata.md), which already
  carries proposed replacement wording for a maintainer to place; the deferral creates no build
  work beyond it. Charts are not built either; neither screen has one.

#### What is not yet verified in a real browser

By rule, nothing in this repository opens a browser, and the launch token cannot be handed to one
from outside the process, so **no screen has been exercised in Chromium, Firefox or WebKit**. The
Node tests render every view in every state with `react-dom/server` and check markup, references,
DOM order, table structure, live-region discipline and the CSS source; the Rust tests fetch the
real bundle from the real binary over TLS (`crates/pfp-app/tests/bundle.rs`: no inline script or
style, every asset same-origin and content-hashed, the exact header set on each). What that leaves
unproven, and waits for the Playwright and axe step:

- that the bundle **executes** with zero `securitypolicyviolation` events under the strict CSP;
- **the file download itself** (`fetch` → `Blob` → a temporary `<a download>`): `saveFile` is a fake
  in every test, so the download event, the filename, the bytes on disk and the absence of a CSP
  violation are unverified in all three engines;
- the session handshake end to end from a launcher-opened tab, the displaced-cookie notice after a
  real cookie displacement, and the 202 leg of the recovery (unreachable today — see S-28 below);
- real focus movement and tab order (the tests prove DOM order and forbid the CSS that separates
  it from visual order), screen-reader announcements and the absence of double announcements;
- computed colour contrast, rendered 24 px target sizes, zoom and reflow at 320 CSS px;
- the printed page (the tests assert the print CSS source, not a rendering);
- S-04 "zero CSP violations across a journey", S-05 "storage empty but for the proof token" at run
  time, and S-06 "zero third-party requests" from a browser's network log;
- Observable Plot under `style-src 'self'`: a desk audit exists, no source audit and no browser
  spike; see `docs/contributing.md` §8 item 7 for what it found.

What to expect from the launcher at this step of M0:

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

The seven M0 operations (`session/bootstrap`, `session/status`, `session/relaunch`,
`tax/rate-schedule`, `tax/rate-schedule/export`, `assumptions/list`, `validation/report`) are `POST`, and that is their contract rather than a placeholder. None of
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
seven operations that exist, so the snapshot and the generated front-end types for them are stable.

### Security test ids in this step

[TESTING.md](docs/TESTING.md) §8 numbers the security tests. For the server half of M0:

| Id | What it asserts | Where, or why not here |
|---|---|---|
| S-01 | `Host` 421, `Origin` 403, route-scoped `Sec-Fetch-Site` | `crates/pfp-server/src/admission.rs` (unit), `crates/pfp-server/tests/admission.rs` (over real TLS) |
| S-02 | Plaintext HTTP fails; no plaintext acceptor in any profile | `crates/pfp-server/tests/tls_only.rs`, `crates/pfp-server/tests/source_rules.rs` |
| S-03 | Loopback-only self-check | `crates/pfp-server/src/bind.rs`, `crates/pfp-server/tests/tls_only.rs` |
| S-04 | Exact response-header set | `crates/pfp-server/tests/headers.rs` and its snapshot; `crates/pfp-app/tests/bundle.rs` asserts the set on the shell and on every asset of the real bundle, served by the real binary, and that the shell has no inline script, style or event handler. "Zero CSP violations across a journey" needs a browser: deferred to the Playwright suite |
| S-05 | Browser storage empty but for the proof token | Static half: `web/tests/no-storage.test.mjs`, which `SECURITY.md` §7.4 calls a first line and not the control. Runtime half: deferred to the Playwright suite, because only a real browser has the storage to inspect |
| S-06 | Zero third-party requests | Static half: `crates/pfp-server/src/csp.rs`, `crates/pfp-server/tests/assets.rs` and `crates/pfp-app/tests/bundle.rs` (every reference in the served shell is a same-origin, content-hashed path; no asset names a further origin, a source map or an `@import`). Runtime half: deferred to the Playwright suite (needs a browser's network log) |
| S-07 | Deny-outbound sandbox, offline first launch | `crates/pfp-app/tests/sandbox.rs` |
| S-08 | Launch token single-use and 60 s; cookie-only and proof-only rejected | `crates/pfp-server/src/session.rs`, `crates/pfp-server/tests/session.rs`, `web/tests/handshake.test.mjs` (the client half). Two clauses wait for M1, because they need a plan to exist: a stolen launch token reaches no plan endpoint, and a bootstrap while unlocked forces a re-unlock. The `on_session_replaced` hook is their seam |
| S-09 | Container magic-byte gate | Already in place: `cargo xtask check-magic` |
| S-10, S-11 | Supply-chain gates, Actions pinning, release key; double build, SBOM, signing | Not this step: the M0 release-pipeline step owns them |
| S-12 | Type-aware export escaping | `crates/pfp-server/src/api/export.rs` (unit: each trigger character, the leading-whitespace and U+FEFF variants, a leading apostrophe, quote doubling, embedded separators and newlines, non-ASCII text, `Text("-5")` escaped while `Cents(-500)` is bare) and `crates/pfp-server/tests/export.rs` (a `proptest` round trip through an independent RFC 4180 reader; no emitted text cell begins with a trigger even after trimming; the JSON export equals the API values; the route is POST-only and session-gated). The escaping is in the server, where a cell's type is known; the browser only downloads. The rule is stronger than `SECURITY.md` §9 as written (it looks past leading whitespace, and escapes a leading apostrophe) — an erratum for that document. `Ratio` cells are not exported at M0: a bare `10/100` is read as a date by spreadsheets, so the textual form needs a decision first |
| S-20 | `RLIMIT_CORE=0`, `Redacted<T>`, silent panic hook | `crates/pfp-app/src/harden.rs`, `crates/pfp-app/tests/panic_hook.rs`, `crates/pfp-server/src/redact.rs` |
| S-28 | Displaced `__Host-pfp` cookie gives the documented 409, not a 401 loop. **The one-click recovery is built, but its success leg is not reachable yet** — see the note below this table | `crates/pfp-server/src/session.rs`, `crates/pfp-server/tests/session.rs`, `crates/pfp-server/tests/headers.rs`; the client half in `web/tests/session-ui.test.mjs` and `web/tests/client.test.mjs` |
| S-29 | Occupied preferred port warns before anything opens | `crates/pfp-app/src/launch.rs` (the warning, the order, and nothing opened when the warning cannot be shown), `crates/pfp-app/src/cli.rs` (a bare run prefers the usual port, so the probe runs at every launch). The tests occupy an OS-assigned port and prefer it with `--port`; none binds the usual port on the machine running them. **Known limitation:** "occupied" means *our bind failed*, so a program listening on the wildcard address (`0.0.0.0:<port>` with `SO_REUSEADDR`) raises no warning; it also receives no loopback connection while this application runs, which is why it is documented (`bind_loopback` in `crates/pfp-server/src/bind.rs`) rather than probed for |

**S-28, the recovery leg.** [SECURITY.md](docs/SECURITY.md) §7.1 specifies the displaced-cookie
answer as a distinguishable 409 *plus* a one-click path that asks the still-running backend for a
fresh launch token and a re-open. The 409, its stable code `session_cookie_displaced`, and the
server half of the recovery (`POST /api/v1/session/relaunch`, its interval throttle
`relaunch_throttled` and its separate per-session cap `relaunch_exhausted`) ship in this step and
are tested. The click exists: a 409 on any call replaces the screens with a notice and the
"Re-open from the application" button, and all five outcomes (202, the two 429 codes, 503, and a
failure) have their own sentence, tested against fakes in Node. **What a user meets today is the
503**: the in-process browser opener is `Platform::unsupported()` in every build this repository
produces, so `relaunch` answers `open_unavailable`, the notice says that this build cannot re-open
itself, and the user recovers by quitting the application and starting it again from its
launcher. The 202 leg becomes reachable when the macOS platform module lands; it is exercised
in-process, with the test standing in for the opener, in `crates/pfp-app/tests/e2e.rs`.

One [SECURITY.md](docs/SECURITY.md) control placed at M0 has no S-id and is **deferred**, so it is
recorded here rather than left silent:

| Control | State in this step | Why, and where it lands |
|---|---|---|
| §8: anything reaching the system log goes through `os_log` with every interpolated value `%{private}` | Not implemented. The in-memory ring buffer is the log (`crates/pfp-server/src/events.rs`); `warn` and `error` events are echoed to stderr, and the `PFP-READY` line and its prose go to stdout. What they can contain is fixed by type: event codes, route templates, status codes, the origin and the certificate fingerprint — never a token, a header value or a path as sent | `os_log` is a C interface, so it needs `unsafe` FFI or a vetted binding; `pfp-server` and `pfp-app` are both `#![forbid(unsafe_code)]`, and the place for it is the macOS platform module behind the `pfp-app` platform seam, which does not exist yet. The exposure §8 describes arises when `launchd` captures stdout and stderr in the `.app` channel, and that channel arrives with the signing block. It lands with the macOS platform module, before the first `.app` build; until then every run is from a terminal, where stderr is the terminal |

### Release build (unsigned, local)

The unsigned half of the release pipeline (`docs/PLAN.md` §4.1; `docs/ARCHITECTURE.md` §9;
[`packaging/README.md`](packaging/README.md)) runs on a developer machine as a **single-arch,
ad-hoc signed** build:

```sh
cargo xtask dist                     # validation report, npm ci --ignore-scripts + vite build,
                                     # cargo build --release --locked (host target, no rustc wrapper),
                                     # pre-codesign SHA-256, ONE ad-hoc signature, both channels,
                                     # check-magic, dist-manifest.json, SHA256SUMS
cargo xtask dist --compare <dirA> <dirB> --report report.md
                                     # WHERE two dist outputs differ (per Mach-O section, per
                                     # tar member, per web/dist file); reported, not gating
cargo xtask sbom                     # CycloneDX JSON + the no-copyleft assertion
cargo xtask sbom --check <file>…     # the assertion alone
```

The output is `target/dist/macos-<arch>-single-arch-adhoc/`: `work/` (the pre-codesign and
signed executables), `app/` (the `.app` wrapper with `LICENSE` and `NOTICE`), `bare/`, and
`artifacts/` (the two deterministic `.tar.gz` channels, `dist-manifest.json`, `SHA256SUMS`).
A one-target build is labelled **single-arch** in every file name and in the manifest; only a
`lipo` of both targets is called universal. `SOURCE_DATE_EPOCH` is the last commit time (or
`--source-date-epoch N`), never the clock. Building twice from one commit gives a
byte-identical executable and byte-identical tarballs **given the same** Rust toolchain, macOS
SDK and linker (Xcode), and `gzip`: the checkout path, the target directory and `$CARGO_HOME`
are remapped out of the binary, and no rustc wrapper runs (see
[`packaging/README.md`](packaging/README.md), "Reproducing a build"). Signing is `codesign --sign -` only: no identity, no keychain, no entitlements. Such
a build runs on the machine that made it; Gatekeeper blocks it anywhere else, as it should.

`cargo xtask sign-checksums` (the Ed25519 signature over `SHA256SUMS`) is a stub that always
fails. No step of this pipeline generates, reads or stores a key.

### What only CI can enforce

- **The toolchain pin.** `rust-toolchain.toml` is honoured wherever `rustup` is present. A
  Homebrew-only Rust ignores it, so a local build may use a different compiler; CI is
  authoritative for the version the release is built with.
- **The `wasm32-unknown-unknown` purity build**, which proves that engine crates touch no
  filesystem, network, clock, environment or entropy (determinism rule D1,
  [ARCHITECTURE.md](docs/ARCHITECTURE.md) §4.3). It needs a `rustup`-installed target, so it is
  a CI-only job.
- **The universal build, the DMG and the double build**
  (`.github/workflows/release.yml`, run by hand only: `workflow_dispatch`, no tag or push
  trigger). `x86_64-apple-darwin` and therefore `lipo` need `rustup` targets that a Homebrew
  Rust cannot add; the DMG is made with `hdiutil`, which never runs on a developer machine; and
  the reproducibility report compares two builds on two runners in two directories. The
  workflow uploads everything as workflow artifacts only: no GitHub Release, no tag, no
  publishing, no Developer ID signature, no notarization.

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
