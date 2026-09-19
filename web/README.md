# web — the Personal Financial Modeling front end

A Vite + React + TypeScript single-page application. `vite build` writes static
assets to `web/dist`, which `rust-embed` compiles into the `pfp` executable; the
Rust server serves them from the canonical loopback origin. Node is a build-time
tool and never ships (`ARCHITECTURE.md` §9, ADR-002).

At M0 this is the shell and the session handshake, and nothing else: landmarks, a
skip link, labelled regions and one live region that says whether this tab is
connected. There is no routing and no state beyond the session.

## Session handshake

`src/session/handshake.ts` runs once, before the first render (`SECURITY.md` §7.1):

1. read the launch token from the URL **fragment** (`#t=<64 hex>`) — never from
   the query string;
2. clear the fragment with `history.replaceState`, before any request is sent;
3. `POST /api/v1/session/bootstrap { token }`; the server sets the `HttpOnly`
   `__Host-pfp` cookie and returns `{ proof }`;
4. store **only** the proof, in `sessionStorage["pfp.proof"]`. Every later call
   goes through `src/session/api.ts`, which sends it as `X-PFP-Proof`.

A reload of a tab that already holds a proof asks `session/status` instead. The
live region then reads "Connected…" or one of four "Not connected…" messages (no
launch token, token refused, session displaced by another local site — the 409
recovery state — or server unreachable).

The module takes `fetch`, `location`, `history` and `sessionStorage` as
arguments, so `npm test` exercises it under plain Node with recording fakes.

## Commands

| Command | What it does |
|---|---|
| `npm ci --ignore-scripts` | Install exactly what `package-lock.json` pins, running no lifecycle scripts |
| `npm run dev` | Vite dev server on loopback, for front-end work alone |
| `npm run build` | `tsc --noEmit`, `npm test`, `vite build`, then `node scripts/write-stamp.mjs` — a type error or a failing test fails the build, and the last step records the front-end input hash in `dist/.dist-stamp`, which a **release** build of `pfp-server` requires (`vite build` empties `dist`, so the stamp has to be written after it) |
| `npm run typecheck` | `tsc --noEmit` |
| `npm test` | `node --test`: the handshake tests, the source lint (no storage API but the one `sessionStorage` key, no inline style, no absolute URL) and the build-stamp known-answer test. No test dependency: Node strips the TypeScript types itself |
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
tsconfig.json               strict; also type-checks vite.config.ts
.nvmrc                      the pinned Node version
scripts/check-licences.mjs  the per-package licence gate
src/
  main.tsx                  runs the handshake, then mounts the React root
  App.tsx                   the shell: landmarks, skip link, labelled regions
  session/handshake.ts      launch token -> cookie + proof; clears the fragment
  session/api.ts            same-origin POST carrying X-PFP-Proof; 409 -> displaced
  App.module.css            shell layout
  index.css                 document tokens and reset
  vite-env.d.ts             Vite client types
tests/
  handshake.test.mjs        the handshake against recording fakes
  no-storage.test.mjs       source lint over src/ and index.html
```

`node_modules/` and `dist/` are ignored by the repository-root `.gitignore`;
there is no `.gitignore` here.
