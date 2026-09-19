# web — the Personal Financial Modeling front end

A Vite + React + TypeScript single-page application. `vite build` writes static
assets to `web/dist`, which `rust-embed` compiles into the `pfp` executable; the
Rust server serves them from the canonical loopback origin. Node is a build-time
tool and never ships (`ARCHITECTURE.md` §9, ADR-002).

At M0 this is two screens behind a session handshake:

- **Rate schedule** (`#/rate-schedule`): filing status, tax year and taxable
  income in; the rate-schedule tax out, with **every line explained** — the amount
  in each bracket, the tax from each bracket, the total, the rounding rule and the
  parameter cells each line read — exportable as CSV or JSON, and printable.
- **Assumptions Registry** (`#/assumptions`, `#/assumptions/<parameter id>`):
  every parameter with its source, as-of date, vintage, projection rule and
  rounding rule, and its verification status **shown honestly**.

The UI never computes money or tax. Every number on screen came from the API as
an integer number of cents and was only formatted (`src/format/money.ts`); the
tax shown is the server's `tax`, not a sum of lines.

## What is shown honestly

The shipped parameter vintage is **unlocked and pending human verification**, and
the UI says so in three places that cannot be dismissed: a persistent banner on
every route (kept on paper, and kept when the status cannot be read — then it
says "treat every figure as unverified"), a notice beside every computed result,
and a badge on every registry entry. The mapping fails closed
(`src/viewmodel/status.ts`): the word "verified" is produced only for
`verified === true`, "locked" only when a locked id is present, the two are
reported separately, and an unknown verification value is "treat as unverified".
Tests assert that the word "verified" appears nowhere on any screen rendered from
what the server really sends today.

## Session handshake

`src/session/handshake.ts` runs once, before the first render (`SECURITY.md` §7.1):

1. read the launch token from the URL **fragment** (`#t=<64 hex>`) — never from
   the query string;
2. clear the fragment with `history.replaceState`, before any request is sent;
3. `POST /api/v1/session/bootstrap { token }`; the server sets the `HttpOnly`
   `__Host-pfp` cookie and returns `{ proof }`;
4. store **only** the proof, in `sessionStorage["pfp.proof"]`. Every later call
   goes through `src/session/api.ts`, which sends it as `X-PFP-Proof`.

A reload of a tab that already holds a proof asks `session/status` instead.

The module takes `fetch`, `location`, `history` and `sessionStorage` as
arguments, so `npm test` exercises it under plain Node with recording fakes.

### Session states

| State | What the person sees | Control |
|---|---|---|
| connected | the screens; the polite region says "Connected…" | — |
| no launch token, token refused, session ended (401 mid-use), server unreachable | a "Not connected" notice in place of the screens | none: no button that cannot work |
| displaced (409: another local site overwrote the cookie) | the notice, and "Your work in the application is not lost" | **Re-open from the application** |
| …re-open accepted (202) | "The application opened a new tab. Continue there" | none |
| …throttled (429 `relaunch_throttled`) | "Wait a few seconds and try again" | button stays |
| …exhausted (429 `relaunch_exhausted`), unavailable (503 `open_unavailable`) | "Quit the application and start it again from its launcher" | button **removed**, not disabled |

A 409 or 401 on *any* screen call flips the whole shell, not one screen
(`src/api/client.ts`). One click is one request: the UI adds no timer, retry loop
or automatic click. **Honest limit:** every build today answers 503 to a re-open,
because the in-process opener is not implemented on any platform yet; the button,
its wiring and all outcomes are implemented and tested against fakes, and the 202
leg becomes reachable when the macOS platform module lands.

## API types and the client

`src/api/schema.gen.ts` is **generated and committed**: `npm run gen:api` runs
`scripts/gen-api-types.mjs` over the server's OpenAPI snapshot
(`crates/pfp-server/tests/snapshots/openapi__openapi_v1.snap`). The chain is: a
Rust DTO change fails the insta snapshot → accepting it fails
`tests/api-types-drift.test.mjs` → regenerating changes the types → `tsc` fails
wherever the UI used the old shape. That is ADR-017's property ("a DTO change
breaks the front-end build, not a user session") without `openapi-typescript`,
which would multiply the installed tree that has to clear the licence gate. The
generator fails closed: a JSON Schema keyword it does not know **throws**.

Money is the opaque type `Cents = { readonly __cents: unique symbol }`, so
`+ - * /` on an amount is a compile error (`tests/cents-types.test.mjs` runs `tsc`
on a file that must not compile). TypeScript has **no** type that makes `a < b`
between two amounts an error, so comparison is closed by a source lint instead:
no relational operator and no `.sort(` in `src/viewmodel`, `src/screens` or
`src/components`, and the identifier `Cents` may be named only where an amount
becomes text. The one sorter (`src/table/sort.ts`) sorts ids, ISO dates and
counts, never amounts.

`src/session/api.ts` stays the one transport. A success body is read **once**, as
JSON or — for the two exports — as text (`expect: 'text'`), so the JSON export is
saved as the bytes the server sent, never parsed and re-serialised here.
`src/api/errors.ts` maps every server error code to the UI's own sentence; the
server's `message` is never rendered.

## Export

The exporter and its spreadsheet formula-injection escaping live in **Rust**
(`crates/pfp-server/src/api/export.rs`, `POST /api/v1/tax/rate-schedule/export`),
where a cell's type is known: amounts are bare numbers (`-12.34` stays a number),
text is always quoted and gets a leading apostrophe when it could be read as a
formula, JSON is untouched (`SECURITY.md` §9). The browser only downloads: `fetch`
→ `Blob` → a temporary `<a download>` attached to the document for the click. The
filename has **one source**: the `Content-Disposition` the server sent
(`rate-schedule.csv` / `rate-schedule.json`). `attachmentFilename` accepts it only
in the exact shape the server writes - a quoted lowercase `[a-z0-9-]` stem and the
extension of the format that was asked for - and otherwise uses the literal
`export.<ext>`; the client composes no name of its own. The plain warning stands
ahead of all three buttons, names a printed page as well as a downloaded file, and
describes each button (`aria-describedby`).
The buttons re-send the inputs of the **result on screen**, not whatever the form
holds now.

## Accessibility patterns established here

PLAN.md §4.1 puts these at M0 "where they are nearly free"; later screens reuse
them.

- **Forms**: every control has a `<label for>`; hints and errors are tied with
  `aria-describedby`; an invalid field has `aria-invalid`; text inputs with
  `inputmode="numeric"`, never `type="number"`. A failed submit moves focus to an
  error summary that is a named `role="group"`, **not** a live region, whose
  entries are buttons that focus their field. The group is `aria-describedby` its
  refusal sentence and its list, so that the focus announcement carries the errors
  and not only "There is a problem, group". **Unverified — see the blocking
  question below; do not copy this summary into another form until it is answered.**
- **Tables**: real `<table>`s with a `<caption>`, `<th scope>`, a row header in
  every row, inside a focusable scroll region named by the caption. A worksheet
  is a table in the server's computation order — the line structure is a graph,
  not a tree. Sortable headers carry `aria-sort` and contain a button whose text
  names the column and the direction in words. No ARIA grid, no custom arrow keys.
- **Live regions — one message, one channel** (`src/announce.ts`): exactly one
  polite `role="status"` region in the shell; failures that do not take focus are
  `role="alert"`; a container that receives focus is never a live region; no
  message goes to two channels. Every reducer state declares its announcements as
  data and the rule is tested for every state. The write to the polite region is
  authoritative (`politeText`): a state with no polite text writes the empty
  string, and the shell shows a message only on the screen that wrote it
  (`visiblePolite`), so no sentence outlives its state or follows the user to
  another screen. Leaving a screen also EMPTIES the store (`politeReducer`'s
  `route` event, and every screen container clears what it owns on unmount), so
  coming back never re-announces something said before leaving.
- **A session that ended (401) drops its proof at once** (`sessionLossHandler` →
  `clearProof`, through the injected storage): the one stored key is only ever a
  live credential. A displaced cookie (409) keeps the proof the recovery needs.
- **Title**: the shell sets the document title from what is *on screen*
  (`documentTitle`): "Not connected" whenever the session notice replaces the
  screens - and the navigation then claims no `aria-current` - and "Not found" for
  a registry address whose id does not resolve. A registry address that does
  resolve marks its entry with `aria-current="true"`, the words "Linked entry" in
  its heading and a border style.
- **Lists inside table cells** carry `role="list"`: they are `list-style: none`,
  and WebKit drops the list role from such a `ul`.
- **A server refusal names no field, so neither does the UI**: the 422 summary is
  one general message with no per-field jump; jumps exist only for fields the
  client marked `aria-invalid` with a message of their own.
- **Focus**: focus moves to the
  screen heading only on changes *after* the first render, so the skip link stays
  the first thing a keyboard user meets. No positive `tabindex`, no `autofocus`.
- **Non-colour encoding**: every status, error and result marker is words, plus a
  border style; glyphs are `aria-hidden` decoration.
- **Target size**: in-cell and inline controls meet WCAG 2.2 SC 2.5.8 by a 24 CSS
  px minimum size, not by the spacing exception.
- **Reflow**: wide tables scroll inside their own region, never the page. Below
  `48rem` the registry summary hides its secondary columns and says so; every
  field of an entry is in its detail. The **worksheet keeps all five columns at
  every width** inside its scroll region: it has no detail view to fall back on.
- **Motion and print**: `prefers-reduced-motion` is honoured globally and nothing
  animates. Print keeps the result, the inputs, the worksheet, the banner and the
  unverified notice; it drops the navigation, the form and the buttons.

### What the tests do and do not prove

`npm test` renders every view in every state with `react-dom/server` and checks
the markup against the checklist in `tests/support/a11y.mjs`, and checks the CSS
**source** (`tests/css.test.mjs`). There is no browser, no jsdom and no axe-core
(axe-core is MPL-2.0 and fails the licence gate). So the tests prove markup,
references, DOM order of focusable elements, table structure, live-region
discipline and what the stylesheets say. They do **not** prove: computed contrast
in a real rendering; actual focus movement; **that focus order and visual order
match DOM order** (the CSS test removes the usual ways CSS separates them —
`order`, reversed flex, grid placement, positioning — but only a browser shows the
real tab sequence); rendered target sizes; what a screen reader announces;
zoom and reflow at 320 px; or **the file download itself** — `saveFile` is a fake
in every test, and the Blob/anchor mechanism first runs in the browser suite.
Those are the axe and Playwright gates of a later change.

**Blocking question for the assistive-technology pass, before M1 adds a form:**
when focus lands on the error summary, do VoiceOver, NVDA and JAWS speak the
error list (through `aria-describedby`), or only the group's name and role? If
only the name, change `FormErrorSummary` to the GOV.UK structure — an inner
`role="alert"` around the heading and list, focus on the outer `tabindex="-1"`
element — and amend rule A5a in `tests/support/a11y.mjs` and `src/announce.ts`,
which today forbid a live region inside a focused container. The markup tests
prove only that the description references resolve to the error text.

`tests/reflow.test.mjs` renders the views from the golden API bodies and requires
every element that shows an unbreakable run (a 64-hex `contentId`) to be covered
by an `overflow-wrap: anywhere` rule; that the browser then wraps is still a
browser question.

One thing about the **built** bundle is proven without a browser, on the Rust
side: `crates/pfp-app/tests/bundle.rs` starts the real binary, fetches
`index.html` and every asset over TLS and asserts that the shell has no inline
script, style or event handler, that every reference is a same-origin
content-hashed path, and that the exact policy headers are on each response. That
the bundle then *runs* without a CSP violation is still a browser question.

## Not here yet

- Threshold projection under an editable inflation assumption (PLAN.md §4.1): no
  API operation accepts an inflation assumption, so there is nothing to call. See
  erratum E7 in `docs/verification/m0-design-errata.md` for the proposed wording.
- Charts. Neither M0 screen needs one, and no chart package is installed. A desk
  audit of Observable Plot under `style-src 'self'` (not a source audit, not a
  browser run) expects it to work with our own stylesheet in place of the
  `<style>` element it appends and without its legends — and expects both Plot
  (through `d3` → `robust-predicates`, Unlicense) and the ECharts fallback
  (`tslib`, 0BSD) to fail the licence gate below first. See
  `docs/contributing.md` §8 item 7.
- Hyperlinked source URLs: shown as text. The application links to no other origin.
- Registry export; the About page with the validation report.

## Commands

| Command | What it does |
|---|---|
| `npm ci --ignore-scripts` | Install exactly what `package-lock.json` pins, running no lifecycle scripts |
| `npm run dev` | Vite dev server on loopback, for front-end work alone |
| `npm run build` | `tsc --noEmit`, `npm test`, `vite build`, then `node scripts/write-stamp.mjs` — a type error or a failing test fails the build, and the last step records the front-end input hash in `dist/.dist-stamp`, which a **release** build of `pfp-server` requires (`vite build` empties `dist`, so the stamp has to be written after it) |
| `npm run typecheck` | `tsc --noEmit` |
| `npm test` | `node --test` with `tests/support/register.mjs` imported first: a synchronous module hook that transforms `.tsx` with esbuild (already in the tree as Vite's dependency, declared at the locked version) and stubs CSS modules, so `react-dom/server` renders the real components under plain Node. Suites: handshake, transport and client, API-type drift, error-code drift, formatting and validation, router, sorting, view-models, every view in every state against the accessibility checklist, CSS source rules, the source lint and the build stamp |
| `npm run gen:api` | Regenerates `src/api/schema.gen.ts` from the server's OpenAPI snapshot |
| `cargo xtask build-web` (repository root) | `npm ci --ignore-scripts` + `npm run build`, checks the emitted tree, and recomputes the input hash in Rust, failing if `npm run build` recorded a different one |
| `npm run licenses` | Per-package licence gate over the whole installed tree |

`npm install` is for changing dependencies. Everything else — local builds and
CI alike — uses `npm ci --ignore-scripts`.

## Constraints this directory is under

These are requirements from the design documents, not preferences. Each one has
a mechanical consequence if it is broken.

**No third-party origin, ever.** No CDN, no web font, no analytics, no error
reporter, no telemetry. Every byte the page loads comes from the loopback
origin, and `default-src 'none'` in the Content-Security-Policy makes anything
else a violation rather than a slow request (`SECURITY.md` §7.1).

**No browser storage and no service worker.** No `localStorage`, no
`IndexedDB`, no Cache Storage, no `navigator.serviceWorker`. M0 acceptance
asserts these are empty; `sessionStorage` holds exactly one value, the session
proof token. `tests/no-storage.test.mjs` is the source-level first line of that.

**No inline script and no inline style.** The CSP is `script-src 'self';
style-src 'self'` with no `'unsafe-inline'`, which blocks `style="…"`
attributes as well as `<style>` elements. So: class names from a CSS module,
never a `style={{ … }}` prop, and no runtime CSS-in-JS library (ADR-002).

**Deterministic output.** Two builds of one commit must produce byte-identical
`dist`, because the bundle is embedded in a binary whose unsigned SHA-256 is
published and compared across runners (`ARCHITECTURE.md` §9.4). Hence exact
version pins, a committed lockfile, a pinned Node version in `engines` and
`.nvmrc`, content-hashed file names and no sourcemaps. Anything that would put a
timestamp, a machine path or a build counter into `dist` is a defect.

**Minimal dependency tree.** Every package is a supply-chain liability and has
to clear the licence gate. Adding one is a decision, not a convenience.

## Why there is no `@vitejs/plugin-react`

JSX is transformed by esbuild through the `jsx: "react-jsx"` compiler option.
The plugin is absent for a specific, measured reason rather than by oversight.

`@vitejs/plugin-react` transforms React through Babel, which pulls in
`browserslist` and therefore `caniuse-lite`, licensed **CC-BY-4.0**. That is
outside the permitted licence set, so installing the plugin makes `npm run
licenses` fail. The plugin's only contribution to this application is React Fast
Refresh in `npm run dev`; without it a saved edit reloads the page instead of
hot-swapping the component, which costs a fraction of a second and no
correctness.

The same check is why **Vite is held at 7.x**. Vite 8 replaced its CSS pipeline
with `lightningcss`, a hard dependency licensed **MPL-2.0**, which is likewise
outside the permitted set. A dependency bot will offer the Vite 8 major; taking
it requires an ADR widening the permitted set first, not a change to the
checker. The hold is mechanical, not just prose: `renovate.json` at the
repository root disables the `vite` major update. Both deviations from ADR-002's
named stack, and the open question of whether the permitted set governs
build-time-only packages at all, are recorded as awaiting an ADR in
`docs/contributing.md` §8 item 2.

## The licence gate

`npm run licenses` walks every installed package — production and development,
at every depth — and requires an exact SPDX identifier from the permitted set in
`ARCHITECTURE.md` §8 and `SECURITY.md` §11: `MIT`, `Apache-2.0`,
`BSD-2-Clause`, `BSD-3-Clause`, `ISC`, `CC0-1.0`, `Unicode-3.0`, `Zlib`.
Widening that set needs an ADR (ADR-004).

The checker is a dependency-free Node script, `scripts/check-licences.mjs`, so
the licence gate does not itself widen the tree it polices. It deliberately
**fails** on an SPDX expression such as `(MIT OR CC0-1.0)`, on the legacy
multi-entry `licenses` array, and on a missing licence field. Teaching it to
accept the permitted half of a disjunction is how an allowlist widens quietly,
and a missing licence is exactly what scanners pass silently — which is why
`docs/licence-watchlist.md` exists as the human half of this control.

## Layout

```
index.html                  no inline script or style; the module entry only
vite.config.ts              deterministic build settings, each one commented
tsconfig.json               strict, erasable syntax only; also type-checks vite.config.ts
scripts/check-licences.mjs  the per-package licence gate
scripts/gen-api-types.mjs   OpenAPI snapshot -> src/api/schema.gen.ts; fails closed
scripts/write-stamp.mjs     the front-end input hash
src/
  main.tsx                  handshake, then the one real AppEnv (the only file naming window/document)
  env.ts                    AppEnv: every browser object, injected; the announce context
  announce.ts               "one message, one channel"
  App.tsx                   AppView (pure shell) and App (session, client, router, shared loads)
  useLoadable.ts            load once, retry, drop a superseded answer
  api/schema.gen.ts         GENERATED API types; Cents is opaque
  api/client.ts             typed client, shallow body guards, session-loss side effect
  api/errors.ts             error code -> failure -> the UI's own words
  session/                  handshake, transport, session reducer, the session notice
  router/                   parseRoute / hrefFor / reducer; the useRoute hook
  format/                   money, ratio, year, label tables: the only numeric conversions
  table/sort.ts             the one sorter; never sorts amounts
  viewmodel/                DTO -> display strings: worksheet, registry, verification status
  components/               LiveStatus, VintageStatusBanner, VerificationBadge, SortableTable,
                            WorksheetTable, ParamRefLink, FormErrorSummary, FailureAlert,
                            ExportActions, PrimaryNav, AboutBuild (+ CSS modules)
  screens/                  rate-schedule (state, effects, view, container), assumptions, not-found
  index.css                 document tokens, reset, base control sizes, print page
tests/
  support/register.mjs      .tsx and CSS-module hooks for node:test
  support/markup.mjs        tolerant tokenizer for React's static markup + queries
  support/a11y.mjs          the accessibility checklist, asserted on markup
  support/golden.mjs        the server's real bodies, read from its insta snapshots
  support/fakes.mjs         one fakeResponse that behaves like Response; fake ApiEnv/AppEnv
  types/cents-arith.ts      must NOT compile; cents-types.test.mjs asserts the diagnostics
  *.test.mjs                the suites listed under `npm test`
```

`node_modules/` and `dist/` are ignored by the repository-root `.gitignore`;
there is no `.gitignore` here.
