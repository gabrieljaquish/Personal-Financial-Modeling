# SECURITY

*Security specification. Date: 2026-09-17. Companion to `PLAN.md` (what and when), `ARCHITECTURE.md` (how) and `DECISIONS.md` (why). Binding on implementation. Every example is synthetic or a published third-party worked example; nothing here describes any real household.*

**Terminology (standing definition).** "The binary" or "the release" always means one thing: a **local web application for macOS**, shipped as a single self-contained executable (universal: Apple Silicon + Intel). Launching it starts a web server bound to loopback, serves the embedded web app over TLS and opens the default browser. The UI is a web app in the browser. It is not a native desktop GUI and not an Electron/Tauri window, and nothing else has to be installed. The optional `.app` wrapper is a notarization container and Finder launcher around the byte-identical Mach-O (`Info.plist` with `LSUIElement`, an icon, the executable — no second executable, no runtime, no WebView, no window, no Dock icon). Electron, Tauri v2 and any WebView host are refused (ADR-003).

**That definition is load-bearing for this document.** Because the UI is a page in the user's own browser rather than a window the application owns, the browser is a trust boundary the application does not control: shared with every other tab, every other loopback server and every installed extension, and equipped with persistent storage the application must refuse to use. Threats T2 (hostile page in the same browser), T3 (port squatting) and T7 (browser residue) exist because of that one fact, and so does every control in sections 6 and 7 — loopback TLS, DNS-rebinding defence, `Host`/`Origin` checks, the dual cookie + proof-token credential, the strict CSP, and "nothing in browser storage but the proof token". An Electron or Tauri shell would have removed T2, T3 and T7 and added a bundled runtime, a second signing surface and a second execution environment; that trade was refused by requirement.

---

## 0. Sourcing note

No researched source exists for the encrypted store. The project's internal research review (unpublished) covered no file format or cipher, key derivation or key management, decrypt boundary between the web UI and the backend, guarantee against plaintext browser persistence, transport security to a localhost backend, or backup and recovery, and makes **no** recommendation on those points.

Therefore every cryptographic, transport, session and process number below is taken from the **spine** — ADR-005, ADR-006, ADR-015, ADR-020, ADR-022/023 and the M0/M1/M3 acceptance lists — not from a researched source; the named RFCs (9106, 8785, 6902, 5869) are named by the spine. Items that **do** rest on an external source carry its URL: the licence tiers and the credential mechanics of sections 10 and 11 cite the repositories and vendor pages directly; the repo-safe vs encrypted-only split of section 1 is this document's own classification. Anything no primary document confirms is marked **(unverified)**. The compensating controls for designing a container without a research basis are in 3.9 (risk R12).

---

## 1. Assets and data classification

`docs/data-classification.md` (M0) is generated from this table; the data-hygiene linter (13.3) enforces it mechanically.

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

**The blast radius of the plan file is its whole history, not its current state (A3).** Because the change log is append-only and deletion is tombstoning (`DOMAIN-MODEL.md` §13), a mistyped earnings record, a former employer's plan detail, a balance the user explicitly deleted and the entire history of a person removed by `removePerson` all remain inside the file. T1 therefore recovers the union of everything ever entered. The control is the purge operation specified in 3.5, and the honest limit is that purge cannot reach copies already made.

Stated from the domain side, the same split reads: balances, debts, basis, loan terms, cross-year tax state, earnings records, insurance specifics, seeds, stored results and every credential are encrypted-store only, while tax tables, SSA constants, IRMAA tiers, capital-market assumptions, life tables and age-rating curves are repo-safe public data with source URL, as-of date and projection rule.

---

## 2. Threat model

### 2.1 Trust boundaries

```
 ┌── user's browser (NOT controlled by the app) ────────────────┐
 │  our origin https://127.0.0.1:<port>   other tabs/origins    │
 │  view-models in JS memory              extensions            │
 │  sessionStorage: proof token only      cookie jar is shared  │
 └──────────────▲───────────────────────────────────────────────┘
       TLS 1.3  │  B1: loopback socket (cookie + X-PFP-Proof + Host/Origin)
 ┌──────────────┴───────────────────────────────────────────────┐
 │ one process: pfp (universal Mach-O, hardened runtime)        │
 │  pfp-server ─ pfp-vault ─ pfp-import ─ pure engine crates    │
 │  plaintext plan lives ONLY here, only while unlocked         │
 └───▲──────────────▲────────────────────▲─────────────────────┘
     │ B2: file     │ B3: Keychain ACL   │ B4: hostile input files
 ┌───┴────┐   ┌─────┴────────┐      ┌────┴──────────────────────┐
 │*.pfplan│   │login keychain│      │CSV / OFX / QIF chosen by  │
 │+backups│   │(opt-in slot) │      │the user                   │
 └────────┘   └──────────────┘      └───────────────────────────┘
     ▲ B5: supply chain (deps, build, signing)    ▲ B6: public repository
```

### 2.2 Threats T1–T7 (frozen in `ARCHITECTURE.md` §8; `docs/threat-model.md` ships at M0)

| # | Threat | Principal controls | §|
|---|---|---|---|
| T1 | **Holder of a copy of the plan file** — stolen laptop, backup disk, sync folder, forwarded file | Argon2id KEK at a calibrated ~1 s cost with a hard floor; 256-bit random DEK; XChaCha20-Poly1305 STREAM; header authenticated as AAD and verified before parsing; bucket padding; no plaintext temp/WAL files; `rekey --rotate-dek` | 3 |
| T2 | **Malicious web page in the same browser** — CSRF, DNS rebinding, cross-origin read, loopback port scan | Exact `Host` allowlist (421); exact `Origin` + `Sec-Fetch-Site` (403); dual cookie + `X-PFP-Proof`; JSON content-type enforced; a custom header forcing a preflight that is never approved; no CORS/PNA headers ever; strict CSP; `no-store` | 7 |
| T3 | **Other local unprivileged process or user, including port squatting** | Loopback-only bind with a startup self-check; a name-constrained CA whose key is destroyed after signing one leaf, so a squatter cannot present a trusted certificate and cannot phish the passphrase; a mandatory warning when the preferred port is occupied (6.3); single-instance lock; 0600 modes; Keychain ACL bound to the designated requirement | 6, 4 |
| T3-s | **T3 reaching for session credentials** — a scraped launch token, or another loopback origin writing our cookie | Launch token passed only through an in-process URL-open API — never argv, environment, file or clipboard (7.1); the decrypted plan bound to **one** session id, concurrent sessions refused; passphrase never a CLI flag or environment variable (3.7). Two honest limits: the `Host`/`Origin`/`Sec-Fetch-Site` triad is browser-supplied and therefore a **T2 control only** (a non-browser local client sets those headers freely), so it is the session pair that stops T3; and the `__Host-` cookie is **not integrity-protected against other loopback origins**, which the port-scoped proof token reduces to an availability problem with a defined recovery path (7.1) | 7, 6 |
| T4 | **Supply chain** — malicious crate, npm package, build step or artifact | Lockfiles and pinned toolchain; `cargo-deny`; `cargo-audit`; `npm ci --ignore-scripts`; `npm audit signatures`; licence checks; Renovate 7-day release age; no CDN/fonts/analytics; reproducible double-build; SBOM no-copyleft assertion; signed `SHA256SUMS` | 11, 12 |
| T5 | **Developer or contributor error**, including real data in an AI-assisted session | gitleaks (pre-commit, CI, nightly full history) with custom rules; magic-byte commit hook; data-hygiene linter; synthetic-fixture policy; protected-path check; dollar-literal lint; redacted diagnostics bundle | 13 |
| T6 | **Hostile input file** handed to the importer while a plan is unlocked | From-scratch bounded parsers; no entity expansion, external references or network; wall-clock budget; `forbid(unsafe_code)`; 1 CPU-hour fuzz per parser per release; importers produce a reviewed `SnapshotDraft` and never write facts | 9 |
| T7 | **Browser residue** | No service worker, `localStorage`, IndexedDB, Cache API or WASM; `sessionStorage` holds only the proof token; `no-store`; opaque ids in URLs; launch token in the fragment, cleared with `history.replaceState`; asserted by Playwright each release | 7 |

### 2.3 Explicitly out of scope (stated to users in `docs/threat-model.md` and the About page)

| Out of scope | What the user is told |
|---|---|
| Malware running as the same user, or as root | It can read process memory, keystrokes and the decrypted plan; no user-space design defeats it |
| A malicious browser extension with all-sites access | It runs inside our origin and can read view-models (R20). Use a dedicated clean browser profile — said in the first-run flow |
| Memory forensics of an unlocked process; swap or hibernation images | `mlock` and `zeroize` reduce but do not eliminate exposure; auto-lock and exit-after-lock shorten the window (R14) |
| Coercion | Out of scope by construction |
| An attacker with **write** access rolling the file back to an older authentic version | Each version's authenticity is guaranteed; freshness is not. `generation` catches *accidental* stale copies only |
| File-size traffic analysis beyond the 64 KiB bucket | Padding hides small differences, not order-of-magnitude plan size |
| **macOS CrashReporter** | On any abnormal termination macOS writes an `.ips` report to `~/Library/Logs/DiagnosticReports` — backtraces, register state, loaded images, abort message — which the application cannot suppress, and Apple receives it when Analytics sharing is on. The controls are to hold no secret in a recoverable state at abort time (5) and to say this plainly: turn Analytics sharing off if that matters to you |
| **Residual metadata of the launch** | Opening the browser goes through LaunchServices, which records the event in the unified log, and the URL reaches browser history for the instant before `history.replaceState` clears the fragment. The token is single-use with a 60-second TTL, so what survives is the fact and time of a launch, not a usable credential |
| **Copies already made**, and purge | `vault compact --purge-before` and `rekey --rotate-dek` reach the plan file and the backups beside it — never a Time Machine snapshot, a cloud-sync version history, or a file already forwarded. On APFS neither can guarantee that overwritten blocks are unrecoverable |
| **Decline mode** (the user refuses the trust anchor, 6.2) | Without the trust anchor the application cannot prove it is the program answering on this port, so the T3 port-squatting protection rests **entirely** on the user comparing the fingerprint. The unlock screen makes that comparison a required step rather than an option |

---

## 3. The encrypted plan file (`*.pfplan`, container format v1)

Seam **S4**, frozen in M1. `formatVersion` (container) and `schemaVersion` (plan document) are independent.

### 3.1 File layout

```
headerLen : u32 little-endian
header    : RFC 8785 canonical JSON, UTF-8, ≤ 64 KiB          ← plaintext
section blobs in section-table order, each a STREAM of        ← ciphertext
64 KiB-plaintext chunks, each chunk carrying a 16-byte tag
```

```json
{
  "magic": "PFPLAN\u0000", "formatVersion": 1,
  "fileId": "<16 B, random at creation, stable for the life of the file>",
  "generation": 137, "createdAt": "…", "modifiedAt": "…",
  "kdf":  { "alg": "argon2id", "version": "1.3", "m": 262144, "t": 3, "p": 4 },
  "aead": { "alg": "xchacha20poly1305", "construction": "STREAM", "chunk": 65536 },
  "compression": "deflate", "padding": { "bucket": 65536 },
  "slots": [
    { "id": "s1", "type": "passphrase", "salt": "<32 B>", "wrapNonce": "<24 B>",
      "wrappedDek": "<48 B>", "createdAt": "…", "label": "passphrase" },
    { "id": "s2", "type": "recovery",   "salt": "<32 B>", "wrapNonce": "<24 B>", "wrappedDek": "<48 B>", … },
    { "id": "s3", "type": "keychain",   "keyId": "<uuid>", "wrapNonce": "<24 B>", "wrappedDek": "<48 B>", … }
  ],
  "sections": [
    { "label": "plan", "offset": 4096, "cipherLen": 131136, "noncePrefix": "<16 B>", "sectionEpoch": 91 },
    { "label": "changelog", … }, { "label": "snapshots", … },
    { "label": "results", … },   { "label": "secrets", … }
  ],
  "headerTag": "<32 B>"
}
```

The section table carries **ciphertext** lengths only; the true plaintext length is a `u64` little-endian prefix **inside** the first encrypted chunk, because a plaintext length in the header would defeat bucket padding. Unknown top-level header fields are rejected, not ignored.

**`sectionEpoch`** is a per-section counter that increments **only when that section is rewritten**, which is what makes a section's ciphertext portable across saves that do not touch it (3.2). **`headerTag`** is a 32-byte HMAC-SHA256 over the whole header, the file's tamper-evidence and stale-header check; it is the only header field excluded from its own input.

### 3.2 Key hierarchy

```
passphrase ──Argon2id(salt_s1, m,t,p → 32 B)─► KEK_s1 ─┐
recovery code Argon2id(salt_s2, m,t,p → 32 B)─► KEK_s2 ─┼─ XChaCha20-Poly1305 key wrap ─► DEK (256-bit random)
Keychain slot key (256-bit random) ───────────► KEK_s3 ─┘                                    │
                                                                                             ▼
   key_section = HKDF-SHA256(ikm = DEK, salt = fileId, info = "pfplan/v1/" + label)
                 for label ∈ {plan, changelog, snapshots, results, secrets}
```

- **DEK**: 256 bits from the OS CSPRNG at creation; unchanged by a passphrase change; changed only by `vault rekey --rotate-dek`.
- **Wrap**: `wrappedDek = XChaCha20Poly1305(KEK, wrapNonce, DEK, wrapAad)` → 32 B + 16 B tag.
- **`wrapAad`** = canonical JSON of `{magic, formatVersion, fileId, kdf, <this slot without wrappedDek>}` — deliberately save-stable (see Deviations D1).
- **`chunkAad(label, i)`** = `fileId ‖ u16be(formatVersion) ‖ label ‖ u64be(sectionEpoch) ‖ u32be(i) ‖ finalFlag`. Every field is either fixed for the life of the file or fixed for the life of that section's ciphertext, so **a section that was not rewritten stays valid byte-for-byte across any number of saves**. The nonce prefix is fresh random on each rewrite and `sectionEpoch` increments on each rewrite, so no key/nonce pair is ever reused.
- **`headerTag`** = `HMAC-SHA256(key_header, JCS(headerCore))`, where `key_header = HKDF-SHA256(ikm = DEK, salt = fileId, info = "pfplan/v1/header")`. **A MAC, not an AEAD:** `key_header` is fixed for the life of the file while `headerCore` changes on every save, so any nonce-bearing construction here would reuse one (key, nonce) pair across many distinct messages — with Poly1305 that leaks the authentication key. HMAC has no nonce and therefore no reuse condition. `headerCore` is the whole header with every `wrappedDek` **and `headerTag` itself** removed, and therefore includes `generation`, `modifiedAt`, `kdf`, `aead`, every slot descriptor and every section's `{label, offset, cipherLen, noncePrefix, sectionEpoch}`. It is verified **immediately after the DEK is unwrapped and before any section is decrypted**, so a reordered section table, an edited slot descriptor, a weakened KDF block or a body replayed under a stale header all fail before a single chunk is touched.
- **STREAM nonce** for chunk *i*: `noncePrefix (16 B, fresh random per section per rewrite) ‖ u56be(i) ‖ finalFlag (0x00, or 0x01 on the last chunk)` = 24 B. The final flag turns truncation into an authentication failure rather than a short read.

The AAD is deliberately split three ways — save-stable `wrapAad`, section-stable `chunkAad`, per-save `headerTag`, each recomputed only when it must be (Deviations **D1**).

### 3.3 Section pipeline

```
plaintext (canonical JSON; NDJSON for changelog)
  → DEFLATE (flate2, pure-Rust backend: no C dependency, ADR-005)
  → prepend u64le plaintext length
  → zero-pad to the next multiple of 65_536
  → split into 65_536-byte chunks → XChaCha20-Poly1305 STREAM under key_section
```

`secrets` is decrypted **only on demand** and re-zeroized immediately; unlocking a plan does not decrypt it — and, because `chunkAad` is section-scoped (3.2), saving a plan does not decrypt it either. A section is re-encrypted only when its plaintext changed; an unchanged section's blob is copied through the whole-file rewrite verbatim with its `sectionEpoch` and `noncePrefix` intact. That is what keeps `secrets` untouched for the life of the file and keeps carrying accumulated `snapshots` and `results` forward cheap (R17). The single exception is `rekey --rotate-dek`, which rewrites every section under a new DEK by design (3.7). **M1 test [S-30]:** perform N *ordinary* saves with a populated `secrets` section and assert its ciphertext bytes are unchanged and that no `secrets` decrypt occurred.

### 3.4 Argon2id parameters, calibration and unlock

| Parameter | Value | Source |
|---|---|---|
| Algorithm | Argon2id v1.3; RFC 9106 vectors in the M1 suite | ADR-005, M1 acceptance |
| Target cost | ≈ 1 s on the machine that created the file | ADR-005 |
| Floor (never below) | m = 256 MiB, t = 3, p = 4 | ADR-005 |
| Rejected on read | m > 4 GiB, t > 16, anything below the floor | ADR-005 |
| Output / salt | 32 bytes / 32 bytes fresh random per slot | this spec |

```
calibrate() -> (m, t, p):
    m, t, p = 262_144 KiB, 3, 4                    # the floor
    while m < 1_048_576 KiB and measure(m,t,p) < 950 ms:  m += 65_536   # 64 MiB steps
    while t < 8 and measure(m,t,p) < 950 ms:              t += 1
    return (m, t, p)
```

Calibration runs at creation and at `rekey`; chosen parameters live in the header so the file opens at its own cost anywhere. Bounds are enforced **on read, before any allocation**, so a tampered header can request neither a trivially weak KDF (downgrade) nor a 64 GiB allocation (denial of service) — the AAD catches the tampering regardless, but the bounds check runs first.

**Recovery code.** 128 CSPRNG bits as Crockford base32 in six groups of five with a check symbol, printed once and never stored, under the **same** Argon2id parameters: a 128-bit secret needs no stretching, but one KDF code path exercised by every test is worth more than the saved second.

**Passphrase entropy is the floor of the whole design.** The passphrase slot is mandatory and permanent (3.7), so against T1 the binding constraint is what the user types, not how the meter scores it: a human-chosen 12-character passphrase is within reach of a targeted offline attack at a ~1 s Argon2id cost. The first-run flow therefore offers **"generate one for me" first**, not behind an advanced toggle, reusing the recovery code's machinery — 128 CSPRNG bits as Crockford base32, six groups of five, check symbol — in the same transcribable form, with the instruction to store it in a password manager (which the stable preferred port, 6.3, lets the manager match to this origin). No new dependency, no wordlist, no second code path. Users who type their own keep the 12-character minimum and the heuristic display (Deviations **D5**).

**Unlock rate limiting.** Attempts 1–3 pay the KDF cost alone; attempt *n* > 3 waits `min(2^(n-3), 60)` seconds; ten consecutive failures require a restart. `{failedAttempts, lastAttemptAt}` **persists across restarts** in the plaintext sidecar preference already specified in 3.5, which holds no financial content: without that, the attacker this control exists for — a person at the keyboard — simply closes the browser, lets the process exit by design (5) and relaunches with the counter and the ten-failure ceiling at zero. An attacker who deletes the sidecar resets a usability control and nothing else, the same posture the `generation` heuristic takes. This bounds a person at the keyboard, **not** an attacker holding the file — that case rests on KDF cost and passphrase entropy, and the first-run flow says so in those words.

### 3.5 Atomic save, backups, crash safety

**Commit first, rotate after.** The plan file must never cease to exist, not even for an instant.

```
save(plan):
    tmp = same_directory(plan_path)/".pfplan.tmp.<random>"      # mode 0600
    write header(generation = g+1, fresh headerTag) and all section blobs,
      copying through any section whose plaintext did not change;  fsync(tmp)
    link(plan_path, ".pfplan.prev.<random>")     # a second name for the OLD content
    rename(tmp, plan_path); fsync(directory)     # POSIX-atomic replace: plan_path ALWAYS exists
    rotate: unlink bak.3; bak.2→bak.3; bak.1→bak.2; prev→bak.1
    fsync(directory)                             # the rotation is durable too
```

**Startup recovery** is specified, not left to chance. If `plan_path` is absent while a `.pfplan.prev.*` or `.pfplan.tmp.*` sits beside a `bak.1`, the application reports an interrupted save and **offers** to restore `bak.1`; it never silently promotes a backup, and it never deletes a temp or prev file it did not itself commit. A stray `.pfplan.prev.*` with `plan_path` present is a completed save whose rotation was interrupted, and is rotated on the next save.

Whole-file rewrite, always: no journal, no WAL, no plaintext temp, no autosave scratch file, ever. Monte Carlo paths are never stored — summaries plus the seed — which keeps whole-file rewrite affordable (R17). Three rolling encrypted backups sit beside the plan file, each a complete, independently openable container. Default location `~/Library/Application Support/<app>/plans/`, overridden by `--plan <path>`; ciphertext upload/download in the browser lets the user keep a copy anywhere. `generation` is compared with a plaintext sidecar preference (`{fileId: lastSeenGeneration, failedAttempts, lastAttemptAt}`, no financial content) to warn "this looks like an older copy" — a usability control, not a security control (2.3).

**Backups and rekey.** Because each backup is a complete container under the **old** DEK, a `rekey --rotate-dek` that only re-encrypted the plan file would leave up to three copies beside it still openable with the passphrase the user is rekeying away from — in the same directory and the same Time Machine snapshot. Rekey therefore takes both keys in hand and either re-encrypts **every sibling backup** under the new DEK, or, with `--discard-backups`, overwrites and unlinks them and starts rotation fresh. The operation reports which it did. The residual is stated in 2.3 and on the screen: a compromised passphrase is a reason to create a **new file**, not only to rekey.

**Purge, beside compaction.** Compaction is a size valve; purge is the data-minimization control that A3's blast radius requires. `vault compact --purge-before <date>` drops `reverseDiff` payloads and snapshot bodies older than the date while **retaining metadata-only entries** (timestamp, action kind), so the audit trail survives without the values. `removePerson` offers a purge of that person's history as an explicit, separately confirmed step rather than doing it implicitly. Purge forces a whole-file rewrite (every affected section's `sectionEpoch` advances) and rotates backups, and the UI says plainly that it cannot reach older backups or copies already made (2.3).

### 3.6 Integrity and tamper behaviour

| Manipulation | Result |
|---|---|
| Any byte flipped anywhere | Authentication error on the first affected operation — **never** a parse error, partial load or silent default (M1 byte-flip sweep) |
| KDF parameters weakened | Rejected by the bounds check; would fail `headerTag` verification regardless |
| Any header field edited — slot descriptor, `kdf`, `aead`, an `offset`, a `cipherLen`, a `sectionEpoch` | **`headerTag`** mismatch → authentication error, before any section is decrypted |
| Section table reordered | `headerTag` mismatch (the table is inside `headerCore`) → authentication error |
| A section truncated, or chunks reordered within a section | Final-flag and `u32be(i)` mismatch in `chunkAad` → authentication error |
| A section body replaced with one from another file, another section, or an earlier rewrite of the same section | `fileId`, `label` or `sectionEpoch` mismatch in `chunkAad`, and the `noncePrefix` recorded in the header no longer matches → authentication error |
| A whole *header* replayed over current bodies, or a current header over stale bodies | `headerTag` binds `generation`, `modifiedAt` and every section's `{offset, cipherLen, noncePrefix, sectionEpoch}` → authentication error |
| Whole file replaced with an older authentic file *in its entirety* | Accepted; `generation` heuristic warns (out of scope, 2.3) |
| Header > 64 KiB, unknown fields, > 8 slots | Rejected before allocation |

**The container is authenticated before it is parsed** (ADR-005), in this order: bounds-check the header; open a key slot to unwrap the DEK; **verify `headerTag`**; then authenticate every chunk as it is decrypted; and only then parse JSON. No serde structure is ever built from unauthenticated bytes, and no section key is ever used against bytes the header has not vouched for.

### 3.7 Key-slot lifecycle

| Operation | Requires | Effect |
|---|---|---|
| Create file | new passphrase | DEK generated; passphrase slot written; recovery code offered and printed once |
| Add recovery slot | unlocked plan | New 128-bit code printed; slot appended |
| Add Keychain slot (opt-in) | unlocked plan + Keychain authorisation | 256-bit slot key stored in the login keychain; slot appended |
| Remove a slot | unlocked plan | Slot removed. The **passphrase slot is mandatory and permanent** |
| Change passphrase | old passphrase | Re-wraps the passphrase slot only. **The DEK does not change**, so older copies and backups stay readable with the old passphrase; the screen says exactly that and links to Rekey (risk R13) |
| `vault rekey --rotate-dek` | passphrase | New DEK and section keys, every slot re-wrapped, **every section rewritten** (including `secrets`, the one operation that does decrypt it) and every sibling backup re-encrypted under the new DEK — or, with `--discard-backups`, overwritten, unlinked and rotation restarted (3.5). The operation reports what it did. It is the only operation that renders a copy **in that directory** useless; copies elsewhere are beyond its reach (2.3) |
| `vault verify` | passphrase or Keychain | Authenticates the file and every backup without writing; reports `formatVersion`, `schemaVersion`, `generation`, slots, section sizes and each `sectionEpoch` |

**How a passphrase reaches the CLI.** From the controlling terminal with echo disabled, or from a pipe on stdin when stdin is not a TTY. **There is no `--passphrase` flag and no `PFP_PASSPHRASE` environment fallback in any build profile, including test and debug builds.** Both conveniences put the file's master credential where it does not belong — a command line visible in a same-uid process listing, a shell history file, an environment block inherited by every child process, and CI logs — and once added for a scripted test they never come back out. A CI check in the 13 lint set greps `pfp-app` for a passphrase-shaped CLI flag or `env::var` lookup, so the convenience cannot be reintroduced quietly.

### 3.8 Passphrase loss — what actually happens

| Situation | Outcome |
|---|---|
| Passphrase lost, recovery code held | Unlock with the code, set a new passphrase, then offer `rekey --rotate-dek` |
| Passphrase lost, Keychain slot enabled, same Mac and same signed binary | Unlock via Keychain, set a new passphrase |
| Passphrase lost, no recovery code, no Keychain slot | **Unrecoverable.** No backdoor, no escrow, no reset. The first-run flow states this *before* the first passphrase is typed and requires an explicit acknowledgement (ADR-005) |
| Keychain item lost (new Mac, wiped keychain, or a binary signed by a different identity) | The passphrase slot still opens the file — precisely why it is mandatory and permanent. A routine update re-signed with the **same** Developer ID keeps working, by construction (4) |
| Recovery code lost while the passphrase works | Remove the old recovery slot, add a new one, print it |

### 3.9 Why a custom container is acceptable (risk R12)

ADR-005 rejected SQLCipher (a C dependency in a universal build, PBKDF2 by default, journal/WAL side files, a relational shape that fights scenario-as-patch), the `age` format (scrypt passphrase recipient; Argon2id is fixed by requirement), a single AEAD blob (no on-demand `secrets`, no per-section keys, size leaks plan complexity) and zstd (a C binding). The answer to "you rolled your own format" is a list of CI gates: **standard constructions only** (AEAD key wrap, HKDF-SHA256, HMAC-SHA256, STREAM, RFC 9106 Argon2id — no novel primitive or mode); RFC vectors; a byte-flip sweep; 1 CPU-hour header fuzzing; a crash-during-save test; a canary-string disk scan; the ~100-line **Python reference decryptor** in `tools/pfplan-ref/` as a cross-implementation oracle (never shipped, and the user's escape hatch); a **published format spec** (`docs/pfplan-format-spec.md`, published at **M1** with the container, because it is the input an external reviewer of a custom AEAD container needs); and an **external review** of `pfp-vault` and `pfp-server` commissioned at **M4 exit** (about nominal week 41), so findings land in M6-M7 and M8-M10 are the remediation window, with the gate requiring zero high findings at M10 (R19; `PLAN.md` M1, M4, M10; open decision 7 is closed before M0 starts because the reviewer is a calendar dependency).

---

## 4. Key stores: the opt-in macOS Keychain slot, and the seam for other systems

The seam already exists (`ARCHITECTURE.md` §7) and is not re-invented:

```rust
pub trait KeyStore {
    fn get(&self, key_id: &KeyId) -> Result<Option<Secret<[u8; 32]>>, KeyStoreError>;
    fn set(&self, key_id: &KeyId, key: &Secret<[u8; 32]>) -> Result<(), KeyStoreError>;
    fn delete(&self, key_id: &KeyId) -> Result<(), KeyStoreError>;
}
```

`#[cfg(target_os = "macos")] MacKeychain` uses `security-framework`; every other target returns `KeyStoreError::Unsupported`, rendered as "a system keystore is not available on this platform; your passphrase still works". A Windows DPAPI or Linux Secret-Service implementation is a later packaging project, not a redesign.

The macOS item is a generic-password item in the **file-based login keychain** — the same store §6.1 uses for the TLS leaf key — with these constants, each of which is a decision rather than a default:

| Attribute | Value | Why this and not the obvious alternative |
|---|---|---|
| Keychain | The user's **login keychain** | It is the store that supports a per-item ACL bound to code identity. The data-protection keychain's access control is keychain **access groups**, which need a `keychain-access-groups` entitlement — and 5 and 12 make an **empty entitlements set** binding. Empty entitlements wins; see the M0 spike below |
| `kSecAttrService` | A fixed reverse-DNS string **compiled into the binary**, identical in both release channels | A bundle identifier is undefined for the bare-executable tarball (ADR-003 ships both channels from the same bytes), so the item must not depend on one. Both channels address the same item |
| `kSecAttrAccount` | `pfplan-dek-wrap/<fileId>` | One item per file; deleting a plan's slot never touches another's |
| `kSecAttrSynchronizable` | **`false`** | Never iCloud Keychain |
| Accessibility | The item must be readable **only while this Mac is unlocked** and must **never leave this device** — no backup inclusion, no migration to a new Mac. Where the API expresses this as an accessibility class, that class is `kSecAttrAccessibleWhenUnlockedThisDeviceOnly` | The `…WhenUnlocked` class without `ThisDeviceOnly` is included in encrypted backups and migrates, which would silently contradict 3.8's premise that a new Mac means the passphrase is the way in |
| ACL | The code-signing **designated requirement**, expressed as `identifier "<id>"` **and** `certificate leaf[subject.OU] = "<TeamID>"` — **never** a `cdhash` and never a path | A cdhash- or path-pinned requirement breaks on **every signed update**, turning 3.8's stated edge case into a routine event. Identity, not a specific build, is what should gate the item. The exact requirement string is recorded under `packaging/` and reviewed like any other release constant |

**M0 spike (before the release form is frozen):** confirm that an item with this accessibility intent, this ACL and an empty entitlements set is creatable and readable in both channels, and that the login keychain's ACL prompt behaviour is acceptable. If the spike shows that the required accessibility guarantee is only available in the data-protection keychain, the entitlement question is a genuine trade against 12 and needs an **ADR** — it is not a detail to settle in an implementation PR.

The slot is **opt-in, never default**. The consent screen says plainly that anyone who can unlock this Mac account can then open the plan file without the passphrase, that the item does not travel to other devices or into a backup, and that a binary signed by a different identity cannot read it — the passphrase being the way in. The **TLS leaf private key** uses the same store with a 0600 file fallback (ADR-006).

---

## 5. Memory hygiene and process hardening

| Control | Specification | Milestone |
|---|---|---|
| Secret types | DEK, section keys, KEKs, passphrase and recovery bytes and decrypted `secrets` payloads are `Secret<T>` (`secrecy`) over buffers implementing `Zeroize` + `ZeroizeOnDrop` | M1 |
| `Redacted<T>` | `Debug`/`Display` print `<redacted>`; plan facts crossing a logging boundary are wrapped | M0 |
| Locking | Best-effort `mlock` on key buffers; failure is a logged capability note, never fatal. Swap and hibernation images remain outside the app's control (2.3) | M1 |
| Core dumps | `RLIMIT_CORE = 0` set at startup, before any secret exists. This suppresses BSD core files **only** — it does not stop macOS CrashReporter (2.3) | M0 |
| Abnormal termination | Handlers for `SIGSEGV`, `SIGBUS`, `SIGILL`, `SIGABRT` and `SIGTRAP` whose **first and only** action is to zeroize the key registry and `munlock`, then re-raise with the default handler. Best-effort and documented as such: the goal is that whatever macOS's own reporter captures contains no key in a recoverable state, since `panic = abort` is an ordinary Rust occurrence and a DEK lives in registers and on the stack | M1 |
| Panics | Hook prints file, line and a stable code — never payloads, values or plan paths — and **zeroizes before abort**. Engine crates `#![forbid(unsafe_code)]` | M0 |
| Auto-lock | 15 minutes idle → keys zeroized, session invalidated, in-memory plan dropped; after lock with no browser heartbeat the process exits; a prominent "Lock and quit" is always visible (risk R14) | M0 plumbing, M1 real lock |
| What "idle" means | **No user-interaction event reported by the front end** — pointer, keyboard or visibility change. Explicitly **not** network activity: the lock heartbeat, background refetch and an open NDJSON stream do **not** reset the timer. Measuring idle from the last HTTP request would let one forgotten open tab hold the plan unlocked indefinitely, which is the R14 scenario the control exists to close. **M1 test:** periodic API polling with no user interaction still locks at 15 minutes | M1 |
| Single instance | Advisory lock file plus the loopback bind | M0 |
| Hardened runtime | Signed `--options runtime` with **no entitlements**: no JIT, no unsigned-memory execution, no library-validation bypass, no debugger attachment | M0 |
| Engine purity | Engine crates have no filesystem, network, clock, environment or entropy access (contract D1), enforced by the `wasm32-unknown-unknown` CI build, so no engine path can leak a fact off-process | M0 onward |

---

## 6. TLS on loopback

ADR-006 is binding, and its honest value statement is published rather than implied: browsers already treat `http://localhost` as a secure context, so loopback TLS is **not** about web-platform features. It defends against loopback capture and, above all, **port squatting** — a process that grabs the port first cannot present the trusted leaf, so it cannot render a convincing passphrase prompt.

### 6.1 Certificate generation (first launch)

```
ca_key  = ECDSA P-256 (CSPRNG)
ca_cert = self-signed; basicConstraints CA:TRUE pathLen:0 (critical);
          nameConstraints (critical) permitted: dNSName "localhost",
          iPAddress 127.0.0.1/32, iPAddress ::1/128; excluded: everything else;
          keyUsage keyCertSign|cRLSign; validity 825 days
leaf    = ECDSA P-256; SAN dNSName "localhost" + 127.0.0.1 + ::1;
          extendedKeyUsage serverAuth; validity 820 days; signed by ca_key
zeroize(ca_key)                     # the anchor can never issue a second certificate
store leaf private key in the login keychain bound to the code signature (0600 file fallback)
```

Generated with `rcgen`, served by `rustls` (`ring` provider), TLS 1.3. **The release profile compiles only a TLS acceptor**, and a CI test connects over plaintext HTTP and asserts failure.

### 6.2 Trust UX

1. A short native explanation — **shown from the application's own process** with `CFUserNotificationDisplayAlert` or an in-process `NSAlert`, activating the app so the alert comes to the front. Content unchanged: what a local certificate is, that a critical name constraint limits it to `localhost`/`127.0.0.1`/`::1`, that its signing key was destroyed after issuing one certificate, and that `pfp trust remove` undoes it. **This is the entire informed-consent step before the user installs a trust anchor, and it must never be shown by a spawned interpreter** — not `osascript`, not any other (Deviations **D7**). It is one native alert panel, not an application window: the UI remains a page in the user's browser.
2. The system authorisation prompt adds the CA to the **user** trust domain for the SSL policy only — never the admin or system domain.
3. **Declining is supported, and its cost is stated where it is chosen** — "supported" is not "equally safe". The decline screen says, in the same plain language as the rest of this section, that without the trust anchor the application **cannot prove it is the program answering on this port**, and that the leaf SHA-256 fingerprint printed in the status console must be compared **before** the passphrase is typed. In decline mode the comparison is a **step, not an option**: the first unlock screen displays the fingerprint and asks the user to confirm it matches the console, and the passphrase field stays disabled until they do. The service stays TLS-only and there is **no HTTP fallback** at any point. The residual — that T3 port-squatting protection now rests entirely on that comparison — is in 2.3 and `docs/threat-model.md`.
4. `pfp trust remove` deletes the CA from the trust domain and the leaf key from the keychain; renewal repeats the flow about every two years.
5. Trust behaviour for user-added, name-constrained roots differs across Safari, Chrome and Firefox and drifts (R3): an M0 spike covers all three and a release-gate Playwright test asserts a warning-free load after one authorisation. The same spike asserts, on a **clean account in both channels**, that the explanation appears **in front**, is attributed to the application rather than to an interpreter, and triggers **no TCC prompt**. The Firefox path, which uses its own root store, is **open decision #3**.

### 6.3 Binding

Bind `127.0.0.1` only; a startup self-check enumerates the listener's local address and **refuses to start** on anything else. The canonical origin is the IP literal `https://127.0.0.1:<port>` — never a hostname — so a tampered `/etc/hosts` or hostile resolver cannot move the origin. Port selection tries a stable preferred port (so password managers can match the origin) and falls back to an OS-assigned port (open decision #9). `::1` appears in SANs and name constraints for completeness but is not bound in v1.

**The fallback is loud, never silent.** The preferred port exists so the browser and password manager recognise `https://127.0.0.1:<preferred>` as this application and offer the passphrase there — so a process that binds it first inherits that recognition while the real application moves elsewhere. The browser warning alone is not enough, because 6.2 point 3 has told some users a warning is a state they may click through. The **probe therefore runs at every launch**, and an occupied preferred port does **not** open the browser silently: the same in-process alert as 6.2 says that another program is using this application's usual address, that any page appearing there is **not** this application, and that the passphrase must not be typed into it. Only then does it continue on an OS-assigned port, whose full origin is shown in the status console. This is the argument for **preferred-port-with-mandatory-warning** in open decision #9.

---

## 7. Localhost web-server defences

ADR-015 is binding; the header set in 7.3 is **exact-match snapshot-tested** at M0, so nothing may be added without updating the snapshot and this section together.

### 7.1 Session establishment

```
pfp-app → browser : https://127.0.0.1:<port>/#t=<launch token>    (fragment, never query string)
browser → server  : POST /api/v1/session/bootstrap { token }
server  → browser : Set-Cookie: __Host-pfp=<opaque>; Secure; HttpOnly; SameSite=Strict; Path=/
                    body { proof: <opaque> }
browser           : sessionStorage["pfp.proof"] = proof; history.replaceState(clears fragment)
browser → server  : every later call carries the cookie AND X-PFP-Proof
```

| Credential | Size / TTL | Why it exists |
|---|---|---|
| Launch token | 256 bits, single use, 60 s, in the **fragment** (never the query string, which enters history, logs and referrers) | Binds this browser session to this launch |
| `__Host-` cookie | 256 bits, session-scoped, `Secure; HttpOnly; SameSite=Strict; Path=/` | `HttpOnly` keeps half the credential out of script reach |
| `X-PFP-Proof` | 256 bits in `sessionStorage`, which is scoped to origin **including port** | Cookies on `127.0.0.1` are scoped by host and **not** by port, so every loopback server shares one jar; the port-scoped factor closes that hole, and a custom header forces a preflight that is never approved |

Both factors are required on every API call: a leaked cookie alone, or a leaked proof token alone, is useless.

**How the launch token reaches the browser.** The URL is opened by calling the `NSWorkspace` / `LSOpenCFURLRef` API **from inside the process** — never by spawning `open(1)` with the URL as an argument, and never through a file, the clipboard or an environment variable. **The token must not appear in any argv, environment block or shell history**: a command line is visible to a same-uid process listing and lands in history files, and an environment block is inherited by every child. The same rule governs the passphrase (3.7). The residual — a LaunchServices entry in the unified log, and browser history for the instant before `history.replaceState` runs — is recorded in 2.3; the lever if that becomes unacceptable is a 10 s TTL, not a different transport.

**One session per unlocked plan.** A decrypted plan is bound to exactly **one** session id. A `bootstrap` presented while a plan is unlocked does not mint a second session with access to it: it **forces re-unlock**, and concurrent sessions are refused. Otherwise a scraped launch token would yield a session that reads the whole plan the moment the legitimate user unlocks.

**When another loopback origin displaces the cookie.** Cookies on `127.0.0.1` are host-scoped and `127.0.0.1` is a secure context, so any other loopback origin — a squatter, or an innocent dev server — can overwrite `__Host-pfp` or flood the jar until it is evicted. The port-scoped proof token makes that an **availability** problem rather than session fixation, but the launch token was single-use and has expired, so with no path back the user meets a 401 loop, concludes the app is broken, and learns to relaunch and click through warnings. Therefore: when the cookie is missing or does not match the live session while the request is otherwise admissible, the server returns a **distinguishable `409` with a stable code**, not a bare 401. The SPA renders "this browser's session was displaced by another local site; re-open from the app" with a one-click path asking the still-running backend for a fresh launch token and a re-open (the launch capability already exists, 6.3).

### 7.2 Request admission

**The `Sec-Fetch-Site` rule is scoped by route**, because the first request of every session is a top-level navigation carrying `Sec-Fetch-Site: none` with `Sec-Fetch-Mode: navigate` — *not* `same-origin`. An unqualified rule would 403 the very load M0 acceptance requires to succeed on three browsers, and the repair reached for under that pressure is to relax the check globally, discarding the cheapest strong defence against T2 (Deviations **D8**).

| Check | Scope | Failure |
|---|---|---|
| `Host` exactly `127.0.0.1:<port>` (allowlist, not a pattern) | everything | **421 Misdirected Request** — the DNS-rebinding defence: a rebound name arrives with a foreign `Host` |
| `Origin` exactly `https://127.0.0.1:<port>` | required on every `/api/**` request; on other routes checked on every state-changing method and on GET where present | 403 |
| `Sec-Fetch-Site: same-origin` — `none`, `cross-site`, `same-site` and **absent** all rejected | `/api/**` only | 403 |
| `Sec-Fetch-Site: none` accepted **only** with `Sec-Fetch-Mode: navigate` and path `/`; cross-site and same-site navigations rejected | document and static-asset routes | 403 |
| `Content-Type: application/json` on bodies | everything | 415 |
| Cookie **and** `X-PFP-Proof` present and matching the live session | `/api/**` | 401, or **409** where the cookie was displaced (7.1) |
| Body size cap (1 MiB for actions; higher only for ciphertext upload) | everything | 413 |
| `Access-Control-Allow-*`, `Access-Control-Allow-Private-Network` | everything | **never emitted, under any condition** |

**The front end sends its API requests with `referrerPolicy: 'strict-origin'`, not the document's `no-referrer`.** Fetch's "append a request Origin header" step serialises `Origin` as `null` for a non-CORS-mode request that is not `GET`/`HEAD` under a `no-referrer` policy, and Firefox implements the step as written, so a same-origin `POST` made under the document's own policy arrives as `Origin: null` and the exact-`Origin` rule above refuses it (403, `origin_forbidden`) before the launch token is read; Chrome sends the real origin either way. Under `strict-origin` the step nulls `Origin` only on an https-to-http downgrade, impossible on one loopback origin, and the `Referer` it permits is exactly `https://127.0.0.1:<port>/` - the application's own origin. The rule itself is unchanged: `null` stays refused, because it is also what a sandboxed frame and a cross-site redirect send.

`frame-ancestors 'none'` (7.3) holds on every route regardless. **`Host`, `Origin` and `Sec-Fetch-*` are browser-supplied, so this triad is a T2 control only** — a local non-browser client sets all of them freely. Against T3 the control is the session pair plus the one-session binding (7.1), never these headers.

### 7.3 Response headers (exact set)

```
Content-Security-Policy: default-src 'none'; script-src 'self'; style-src 'self';
                         img-src 'self' data:; connect-src 'self'; font-src 'self';
                         frame-ancestors 'none'; base-uri 'none'; form-action 'none'
Cross-Origin-Opener-Policy: same-origin
Cross-Origin-Resource-Policy: same-origin
X-Content-Type-Options: nosniff
Referrer-Policy: no-referrer
Cache-Control: no-store                               (index.html and every /api/ response)
Cache-Control: public, max-age=31536000, immutable    (content-hashed static assets only)
```

No inline script and no inline style: plain CSS modules and no runtime CSS-in-JS keep `style-src 'self'` (ADR-002), and charts render SVG without `eval` (Observable Plot; ECharts is the documented fallback if the M0 CSP spike fails). No `'unsafe-inline'`, `'unsafe-eval'` or `'wasm-unsafe-eval'`, and no `report-uri` — a report endpoint would be an outbound connection. The CSP is generated from the build-time asset-hash manifest, so it cannot drift from what is embedded.

### 7.4 Browser-side prohibitions (T7)

No service worker, `localStorage`, IndexedDB, Cache API or WASM. `sessionStorage` holds exactly one key, the proof token. URLs carry opaque ids only. The browser receives **view-models**, never the raw plan. Long runs stream **NDJSON over `fetch`** rather than `EventSource` or WebSocket, neither of which can carry the proof header; exports download via `fetch` + blob. A source lint over `web/` bans the storage APIs as a cheap first line; the control is the Playwright assertion against the real signed binary at every release.

### 7.5 Offline by default (ADR-020)

v1 makes **zero outbound connections**: no telemetry, crash upload, update check, fonts, CDN, analytics or market-data calls; all assets are local; market-dependent defaults ship as dated values in assumption vintages and are overridable. `pfp-net` is the **only** crate permitted an HTTP client, enforced by `cargo-deny` bans on every other workspace member, and it does not exist until the first opt-in outbound feature does. An integration test runs the server under a deny-outbound sandbox profile, and M0 acceptance requires first launch to succeed **with networking disabled**.

---

## 8. Logging and diagnostics

**Rule: logs describe control flow, never content.**

| Allowed | Forbidden |
|---|---|
| Stable event codes, method and route template (`PATCH /api/v1/plan`), status codes, durations, counts, engine/schema versions, parameter vintage ids, seeds, run ids, opaque object ids | Any amount, name, date of birth, account or institution identifier; any plan or export file path; any passphrase, token, header or cookie value even truncated; any import-file content or filename; any change-log payload |

**Destination, stated accurately.** The application writes **no log file of its own**, ships nothing, and **ships no crash reporter and makes no crash upload**; macOS's own reporter is outside its control (2.3). But "stderr only" is not "nothing persists": in the `.app` channel the process is launched by `launchd`, which captures stdout and stderr into the **unified logging system** — written to disk, readable by the same user, and swept into `sysdiagnose` archives users routinely send to support. The content rules above bound the damage, but seeds, run ids, route templates, collection counts and timings are a real metadata trail. The specified behaviour is therefore an **in-memory ring buffer** as the real log, surfaced in the status console and used to build the diagnostics bundle, with **nothing above `warn` going to stderr by default**; anything that does reach the system log goes through `os_log` with **every interpolated value marked `%{private}`**, so redaction is enforced by the logging system as well as by `Redacted<T>`. `--log-level debug` raises ring-buffer verbosity but cannot unlock content, because redaction is a type property, not a formatting choice.

The **network log** (meaningful only once `pfp-net` exists) records timestamp, feature, method, host, path, status and byte counts — never bodies, never credentials.

The **redacted diagnostics bundle** (asset **A10**) is a plaintext artifact written deliberately to be sent to a stranger, so it is treated as one: **user-initiated only**, preceded by the same plain warning and change-log entry that govern exports (A9), written to a **destination the user chooses** at mode **0600**, never to a default or hidden path. It carries app version and binary digest, OS and architecture, engine/schema/`formatVersion`, vintage ids, feature toggles, collection counts ("4 accounts, 2 debts"), the last 200 ring-buffer lines and the validation report — no values, no identifiers, no paths. The canary test [S-25] stands: a CI test generates it from a synthetic plan containing a canary and asserts zero canary hits.

---

## 9. Import-file handling (T6)

Every importer is a `Connector` producing a `SnapshotDraft` the user reviews and merges; **nothing writes facts directly** (ADR-021). Parsers are written from scratch in Rust: the mature Python OFX library `ofxtools` is **GPL-3.0-only** and cannot be linked or vendored, and `ofx-js` is MIT but is a JavaScript library that would have to run in the browser, where no decrypted data may be computed.

```rust
pub trait ImportFormat {
    fn sniff(bytes: &[u8]) -> bool;
    fn parse(bytes: &[u8], limits: &ImportLimits) -> Result<SnapshotDraft, ImportError>;
}
```

| Limit | v1 default | Reason |
|---|---|---|
| File size | 32 MiB | A balance/holdings snapshot is kilobytes; three orders of magnitude of headroom |
| Records | 250,000 | Bounds worst-case allocation |
| Field length / columns per record | 64 KiB / 512 | Bounds a pathological cell and header explosion |
| Nesting depth (OFX/SGML) | 64 | Bounds recursion; parsers iterative where practical |
| Entity/DTD expansion, external references, network fetch | **Disabled entirely** | No billion-laughs, no XXE; an import never causes an outbound connection |
| Wall clock | 10 s, then abort | Bounds algorithmic-complexity attacks |
| Encoding | Strict UTF-8 with a documented single-byte fallback; invalid sequences are an error, never silent replacement | Prevents smuggling through decoding differences |

Parsers live in `pfp-import`, `#![forbid(unsafe_code)]`, with no filesystem access beyond the file handed to them and no network capability. **Fuzzing**: one `cargo-fuzz` target per format with a committed synthetic corpus, 1 CPU-hour per parser per release; acceptance is "never panics and never writes facts without review" (M4). **Export escaping is type-aware, and CSV-only (M0, before any user data exists).** Formula injection is a CSV-*consumption* problem, so the rule is written against the one format that has it:

| Case | Rule |
|---|---|
| CSV cells the exporter knows to be **numeric or date-typed** — every `Cents`, `Ratio` and date field, which the exporter can tell from the schema | Emitted **bare and never escaped** |
| CSV **free-text** cells — labels, notes, reasons, explanation strings | Emitted quoted; when the text begins with `=`, `+`, `-`, `@`, tab or CR, a leading apostrophe is added **inside** the quotes |
| **JSON export** | **Exempt.** Values are emitted verbatim; a leading-character escape corrupts strings for every consumer, protects nothing, and applied to a number produces invalid JSON |

A blanket leading-character rule would be a security control that produces wrong money in the product's core artifact, because `-` begins every negative amount: every withdrawal, loss and delta would become text, and a user who exports a ledger and sums a column would get a silently wrong total. [S-12] is stated against this rule, not the blanket one: a negative `Cents` round-trips as a **number**, a label beginning with `=` as **inert text**, and the JSON export is **byte-identical** to the underlying values.

Exports and prints are the one place plaintext leaves the store: user-initiated, preceded by a plain warning, recorded in the change log with their pins. Import provenance (format, file hash, row counts, timestamp — never the file, never its path) is recorded in the `importProvenance{}` collection inside the encrypted plan (`DOMAIN-MODEL.md` §17).

---

## 10. Future connector credentials (later phase; seam frozen at M10)

Institution connections are a committed later phase, not v1. The security contract is fixed now so nothing is retrofitted:

```rust
pub enum SecretRef { Keychain { key_id: KeyId }, PlanSecrets { id: SecretId } }

pub trait Connector {
    fn describe(&self) -> ConnectorInfo;
    fn fetch(&self, secrets: &SecretRef, since: Option<Date>) -> Result<Vec<SnapshotDraft>, ConnectorError>;
}
```

| Rule | Detail |
|---|---|
| Storage | A credential resolves **only** from the macOS Keychain or the plan file's `secrets` section; nowhere else exists |
| On-demand | `secrets` is decrypted for the duration of one fetch and zeroized immediately |
| Never exposed | No credential appears in an API response, log line, network log, change log, export, diagnostics bundle or stored `Recommendation`; a serialization test asserts no DTO can carry a `Secret` |
| Network confinement | Network connectors run only inside `pfp-net`, individually toggled, host-allowlisted, every request in the in-app network log (ADR-020) |
| Drafts only | Connectors produce `SnapshotDraft`s for review — the same path as file import, so review and merge are shared and tested from M4 |
| Proof first | A **sandbox connector** exercises the Keychain-held-secret path end to end at M10, before any real institution connector exists |

Research-sourced mechanics for the likeliest first candidate: SimpleFIN Bridge issues a **setup token** POSTed once to a claim URL, returning an **Access URL** carrying HTTP Basic credentials; the bridge is $15/yr, ≤25 institutions and ≤25 apps, **≤24 requests/day**, 90-day windows ([SimpleFIN](https://beta-bridge.simplefin.org/info/developers)). The Access URL *is* the credential — embedded userinfo makes it a bearer secret, never logged, never shown in a URL bar, stored as a `SecretRef` like any other. Holdings are undocumented in the protocol, so brokerage positions stay CSV/OFX. SimpleFIN's reported MX backing is **(unverified)**. Market-data keys carry their own terms: FRED requires a **distinct key per application** and the display of its non-endorsement notice ([FRED](https://fred.stlouisfed.org/docs/api/fred/)).

---

## 11. Dependency and supply-chain policy (T4)

| Control | Mechanism | Where | Failure |
|---|---|---|---|
| Pinned toolchain and locked graphs | `rust-toolchain.toml` (floor 1.84) + pinned runner image; `Cargo.lock` with `--locked`; `package-lock.json` with `npm ci` | CI, local | Build refuses |
| No install scripts; package signatures | `npm ci --ignore-scripts`; `npm audit signatures` | CI | Gate fails |
| Vulnerabilities | `cargo-audit`, `cargo-deny advisories` | CI + nightly | Gate fails |
| Licence **allowlist** | `cargo-deny`: MIT, Apache-2.0, BSD-2/3-Clause, ISC, CC0-1.0, Unicode-3.0, Zlib; **extending the list needs an ADR**. Per-package `license-checker` for npm, because no front-end licence has been confirmed per package in advance | CI | Gate fails |
| Licence **clarifications** (the second, separate mechanism) | A committed table asserting an SPDX expression against a crate's **actual licence text**, each entry carrying a one-line justification and the **file hash** of that text, reviewed like a fixture. This is the instrument for a licence-*metadata* gap, which an ADR is the wrong tool for: `ring` — fixed by ADR-006 as the `rustls` provider — has no clean SPDX expression (an ISC-style grant for the original work plus OpenSSL/BoringSSL-derived terms for inherited assembly and C), and `cargo-deny` reports it as unknown, as it does for parts of the `webpki` family and for the `NOASSERTION` phenomenon already flagged for TPAW (11.1). **The `ring` clarification is pre-declared at M0** so the gate is green on day one. A clarification records **what a licence is, never what the project wishes it were**, and may **never** admit a copyleft-licensed crate; the SBOM's no-copyleft assertion at the release gate remains the independent check | CI | Gate fails |
| Source pinning | `cargo-deny sources`: crates.io only; no git dependencies without an ADR | CI | Build refuses |
| Capability bans | No HTTP client crate outside `pfp-net`; `std::fs` only in `pfp-vault`/`pfp-app`/`xtask`; `std::net` only in `pfp-server` | CI | Build refuses |
| Update cadence | Renovate, **7-day minimum release age** (a compromised release is usually yanked inside a week) | Repo | — |
| No remote runtime assets | No CDN, fonts, analytics or telemetry; everything embedded | Build + CSP + Playwright | Gate fails |
| SBOM | CycloneDX (`cargo-cyclonedx` + `cyclonedx-npm`) on every release, asserting **no GPL/AGPL/PolyForm/Parity component**; `cargo-auditable` embeds the manifest in the binary | Release | Gate fails |
| Reproducibility and integrity | Two independent CI builds produce an identical unsigned SHA-256, published as "given toolchain X"; `SHA256SUMS` signed with the Ed25519 release key | Release | Gate fails |
| **Release-key publication and rotation** | The Ed25519 **public** key is published **out of band in two independent locations**, and its fingerprint is printed on the app's **About page**, so an installed copy can be compared against the download page. A signature and a key served from the same release page authenticate nothing against the compromise that matters. **Rotation** = a new key signed by the old one, both published for one release cycle. The post-1.0 update-check key (12, ADR-020) ships as a **two-key trust store** (current + next), so rotation never requires a reinstall | Release; About page | Gate fails |
| **The build itself** (the other half of T4) | All GitHub Actions — including first-party ones — **pinned to full commit SHAs** and updated by Renovate under the same 7-day release age; `permissions: contents: read` as the workflow default with explicit, per-job escalation; **no `pull_request_target`** and no fork-triggered workflow that touches a release path; release workflows runnable **only** from a protected branch/tag ruleset, with the protected environment requiring a manual approval — **by a reviewer distinct from the author when the project has more than one maintainer; for a single maintainer, a manual approval no sooner than 24 hours after the tag**, recorded in the environment's deployment log, with that limit stated in `docs/threat-model.md` (`PLAN.md` R26) | CI config | Gate fails |

### 11.1 The copyleft boundary is also a supply-chain boundary

**No oracle code ever enters the process that holds a decrypted plan.**

| Class | Projects (research-sourced) | Permitted use |
|---|---|---|
| Port with attribution | [Open Social Security](https://github.com/MikePiper/open-social-security) (MIT), [ssa.tools](https://github.com/Gregable/social-security-tools) (MIT), [boknows/cFIREsim-open](https://github.com/boknows/cFIREsim-open) rule tests (Apache-2.0 per its `LICENSE` file; pinned as `boknows/cFIREsim-open@<sha>` at transliteration; last push 2022-04-08, checked 2026-09-17), [R4GoodPersonalFinances](https://cran.r-project.org/package=R4GoodPersonalFinances) (MIT), [muirjc/retirement-planner](https://github.com/muirjc/retirement-planner) (MIT) — the full enumeration with pinned commits is ADR-004's `NOTICE` list | Re-implement in Rust; transliterate tests; `NOTICE` entries |
| Vendor as data | [Tax-Calculator](https://github.com/PSLmodels/Tax-Calculator) parameters (**CC0-1.0**, not MIT); IRS/SSA/CMS primary documents; [NCHS life tables](https://www.cdc.gov/nchs/products/life_tables.htm) | Copied into `params/` with provenance and checksums |
| Out-of-process test-time oracle only | [Owl](https://github.com/mdlacasse/Owl) (GPL-3.0), [PolicyEngine-US](https://github.com/PolicyEngine/policyengine-us) (AGPL-3.0) | Subprocess + JSONL, outputs **recorded** as goldens; never linked, vendored, imported or shipped (PolicyEngine's YAML is fetched into a git-ignored cache, never committed); adapters from scratch; parameter-table values never sourced from them — a default may be cited as a comparison with attribution, never adopted (ADR-004) |
| Read-only reference | [TPAW](https://github.com/bengmathew/tpaw) (PolyForm Noncommercial 1.0.0 — GitHub reported `NOASSERTION` as of 2026-09-17, so scanners miss it), [prime-harvesting](https://github.com/hoostus/prime-harvesting) (Parity 7.0.0), [Beancount](https://github.com/beancount/beancount) (GPL-2.0) and [hledger](https://github.com/simonmichael/hledger) (GPL-3.0) booking semantics, and repositories with no `LICENSE` file as of 2026-09-17 ([ukaia/retirement-planner](https://github.com/ukaia/retirement-planner), [mjcrepeau/retirement-planner](https://github.com/mjcrepeau/retirement-planner)); every status claim in this row is a `docs/licence-watchlist.md` entry re-confirmed at each release with its check date | Read for design; re-derive equations; copy no code, tests or data |

`oracles/` and `tools/` are **not Cargo workspace members** and are excluded from release archives; each oracle is a separate `uv` project with its own lockfile and licence notice, Python pinned to 3.13 and fetched by `uv` rather than the system interpreter; `cargo xtask oracle record` runs them as subprocesses and stores JSONL goldens; ordinary CI runs **verify** only and installs no Python and no copyleft package.

---

## 12. macOS signing, notarization and Gatekeeper first-launch UX

| Step | Command / property |
|---|---|
| Build | `cargo build --release --locked` for `aarch64-apple-darwin` and `x86_64-apple-darwin`; `MACOSX_DEPLOYMENT_TARGET=12.0`; `--remap-path-prefix`; `codegen-units = 1`; `SOURCE_DATE_EPOCH`; `lipo -create` → one universal Mach-O |
| Sign | `codesign --options runtime --timestamp --sign "Developer ID Application: …"` with an **empty entitlements set** — no JIT, no unsigned-executable-memory, no library-validation bypass, no debugging entitlement |
| Notarize / staple | `xcrun notarytool submit --wait`; `xcrun stapler staple` on the DMG and the `.app` inside it |
| Verify | `codesign --verify --strict`; `spctl -a -vv` reports *Notarized Developer ID*; `stapler validate` |
| Channels | (a) default: a stapled DMG holding the three-file `.app` wrapper; (b) a tarball of the **byte-identical** bare executable for CLI/Homebrew users |
| Credentials | Signing identity, Apple API key and the Ed25519 release key live in a protected CI environment that runs only on signed tags, from a protected tag ruleset, with manual approval **by a reviewer distinct from the author, or — for a single maintainer — after the 24-hour cooling-off period of section 11** (asset A8; `PLAN.md` R26). Release-key custody, publication and rotation are specified in 11 and feed open decision #2 |

**Why the wrapper exists.** A notarization ticket staples to `.app`, `.dmg` and `.pkg` but **not** to a bare Mach-O, and recent macOS removed the Control-click bypass for un-notarized software — so without a stapled container the first launch performs an online Gatekeeper check that fails hard on an offline or captive-portal machine (R2, ADR-003). M0 settles this on a network-isolated clean account; if stapling a bare Mach-O is rejected (expected), channel (a) is mandatory rather than a convenience.

**First launch, honestly described.** The user drags the `.app` to Applications (or extracts the tarball). First open shows **only the standard notarized-download dialog** — no "unidentified developer" wall, no right-click workaround — **and it works with networking disabled** (M0 acceptance). The process starts with no Dock icon, binds loopback — warning first if the preferred port is occupied (6.3) — and, on the very first run, shows the certificate-trust explanation and system authorisation from its own process (6.2); it then opens the default browser at `https://127.0.0.1:<port>/#t=…` **through the in-process `NSWorkspace`/`LSOpenCFURLRef` API, never by spawning `open(1)` with the URL as an argument** (7.1). Launched bare from Finder, the executable opens a Terminal window as its status console (fingerprint, port, "Lock and quit"). There is **no auto-update and no phone-home** in v1 (ADR-020): new law arrives as a new parameter vintage in a new release, with a per-table delta shown before a plan adopts it; an opt-in signed update check through `pfp-net` against a **compiled-in two-key Ed25519 trust store** (current + next, so the key can rotate without a reinstall — 11) is a post-1.0 item, and self-replacing updates are out of scope. Source builders get documented instructions for an unsigned local build and what that costs them (R4).

---

## 13. Repository hygiene — enforcing "the repository is the application only"

The repository contains **only** code, docs, public parameter tables with provenance and clearly labelled synthetic fixtures (ADR-023). All of the following is mechanical, because policy without a linter fails silently.

### 13.1 `.gitignore` (minimum)

```
local/                 # developer scratch; never published, no exceptions
*.pfplan  *.pfplan.bak*  *.pfplan.tmp*
exports/  *.export.csv  *.export.json
.env  .env.*
*.p8  *.p12  *.cer  *.mobileprovision      # Apple signing material
*.pem  *.key                               # never a private key in the repo
oracles/**/.venv/  .uv-cache/  .policyengine-cache/
```

### 13.2 gitleaks (pre-commit via `lefthook`, CI, and nightly over full history)

| Custom rule | Pattern (indicative) | Rationale |
|---|---|---|
| Plan-file magic | leading bytes `PFPLAN\0` | An encrypted plan is still user data |
| SimpleFIN Access URL | `https://[\w.%-]+:[\w.%-]+@[\w.-]+/simplefin` | Basic-auth userinfo *is* the credential |
| FRED-style key | `\b[0-9a-f]{32}\b` near `fred`/`api_key` | Distinct key per application; never in the repo |
| Apple signing material | The PEM header for a private key (the five-dash `BEGIN` marker followed by `PRIVATE KEY` and five dashes — split here so this document does not match its own rule), `.p8` and `.p12` blobs, `AuthKey_[A-Z0-9]{10}` | Release-key custody |
| Ed25519 release key | The OpenSSH private-key PEM header (`BEGIN` … `OPENSSH PRIVATE KEY`, split as above) | Release-key custody |

The literal regular expressions live in `.gitleaks.toml` under the rule ids `pfp-plan-magic`, `pfp-simplefin-url`, `pfp-fred-key`, `pfp-apple-signing` and `pfp-release-key`; this table describes them and deliberately does not reproduce a string that would itself trigger them.

A separate commit hook rejects any file whose **first bytes are the container magic**, independent of gitleaks, because that one check catches the likeliest accident.

**The magic-byte hook and the 13.4 "container magic anywhere" rule take no allowlist, ever.** No path, no fixture directory, no exception for a demo file. That rule is the single control standing between a real household's `.pfplan` and a public repository (the T5 failure mode), and one allowlisted path blinds it permanently.

That makes the shipped demo plan a conflict to resolve rather than to exempt (Deviations **D9**). **The demo never exists as a `.pfplan` in the repository.** It ships as **plaintext synthetic JSON at `fixtures/plans/demo.plan.json`** carrying `"synthetic": true` plus generator name and seed per 13.3(2); `rust-embed` embeds the **JSON**, and the binary encrypts it into a `.pfplan` in the user's own directory — or a temporary plan for Playwright — at first use. `tools/pfplan-ref/` likewise holds **no** container test data; its fixtures are generated at test time.

**M0 gate:** a `.pfplan` byte sequence **anywhere** in the tree or in a release archive fails the build.

### 13.3 Data-hygiene linter (pre-commit and CI)

1. Any `.json`, `.csv`, `.ofx`, `.qif`, `.yaml`, `.yml` or data `.toml` under version control must live in `fixtures/` or `params/` — and, for `.yaml`/`.yml`, must carry `synthetic: true` or a citation block like any fixture, so an upstream AGPL corpus (PolicyEngine-US) cannot be committed by accident; it is fetched into the git-ignored `.policyengine-cache/` (13.1).
2. Every `fixtures/` file carries **either** `"synthetic": true` with generator name and seed, **or** a citation block `{title, publisher, url, page, retrieved, sha256}`.
3. Every `params/` file carries `[[source]] {title, url, retrieved, sha256}`, an as-of date, a projection rule and a `RoundingRule`; a table without a projection rule is a CI error.
4. Locked vintages are immutable: `VINTAGES.lock` holds a SHA-256 per file and a changed hash fails CI; corrections ship as a **new** vintage.
5. Persona fixtures are generated by a seeded `xtask` script, so they are obviously synthetic and regenerable.

### 13.4 User-data and credential-handling pattern check (CI on the diff, nightly over full history)

| Pattern | Action |
|---|---|
| Container magic anywhere — **no allowlist, ever** (13.2), including release archives | Fail |
| SSN-shaped `\b\d{3}-\d{2}-\d{4}\b` outside a documented fixture allowlist | Fail |
| A JSON document containing `"schemaVersion"` together with `"accounts"` or `"persons"` outside `fixtures/plans/` | Fail |
| A fixture with neither `"synthetic": true` nor a source citation | Fail |
| Real-looking email or street addresses in fixtures | Fail |
| An `asOf`-stamped fact file outside `fixtures/` | Fail |
| A passphrase-shaped CLI flag or an `env::var` passphrase lookup anywhere in `pfp-app`, in **any** build profile (3.7) | Fail |

### 13.5 AI-assistance protocol (T5)

Three failure modes, three controls (ADR-022): *self-confirming tests* — fixtures are written from the primary source **before** the implementation is requested, the assistant never authors both a constant and the test that checks it, and a lint bans dollar literals in engine crates outside tests; *silent ground-truth edits* — CODEOWNERS-style CI path checks block edits to `fixtures/tier1/` and locked `params/` vintages; *data exfiltration* — development and AI-assisted sessions use the **synthetic demo plan only** (R15), meaning `fixtures/plans/demo.plan.json` encrypted locally at first use (13.2), and bug reports use the redacted diagnostics bundle (A10). All documentation, fixtures, examples and commit messages are user-agnostic: generic personas ("an early-career single filer with student debt", "a two-earner couple with a mega-backdoor-capable plan") and synthetic figures only.

---

## 14. Milestone map — where every part of this document lands

| § | Control | Milestone | Proof in `PLAN.md` |
|---|---|---|---|
| 1 | `docs/data-classification.md`; data-hygiene linter | M0 | M0 scope |
| 2 | `docs/threat-model.md` with T1–T7 and the out-of-scope statement | M0 | M0 scope; gate item 6 (threat-model delta in every release PR) |
| 3.1–3.8 | Container v1: `headerTag` over `headerCore`, section-scoped `chunkAad` with `sectionEpoch`, passphrase and recovery slots, per-section HKDF subkeys, bucket padding, generation counter, commit-first atomic save with startup recovery, three backups, `vault verify`, `rekey --rotate-dek` including backup re-encryption | **M1** (seam S4) | M1 scope and acceptance (RFC vectors, byte-flip sweep, bounds, crash-during-save, secrets-untouched, canary scan, fuzz) |
| 3.5 | `vault compact --purge-before <date>` and the `removePerson` purge offer (3.5): the payloads purge drops exist only from M2 (`reverseDiff` entries) and M4 (snapshot bodies), so purge lands with them | **M4** | M4 scope and acceptance (purge round-trip read back with the reference decryptor; purge canary, `TESTING.md` §8); R17 |
| 3.9 | Python reference decryptor as a CI oracle; `docs/pfplan-format-spec.md` published with the container | M1 | M1 scope and acceptance |
| 3.9 | External review of `pfp-vault` and `pfp-server`: commissioned at **M4 exit**, findings landing M6-M7, remediation M8-M10, **zero high findings gated at M10** (R19) | **M4 exit** (commission), **M10** (gate) | M4 and M10 scope; open decision 7 closed pre-M0 |
| 4 | Keychain/entitlement spike: accessibility class, login-keychain ACL and empty entitlements together, both channels | **M0** (before the release form is frozen) | M0 scope; ADR if the spike forces the data-protection keychain |
| 4 | `KeyStore` trait; opt-in Keychain slot with `synchronizable = false`, a fixed compiled-in service string and an ACL bound to `identifier` + leaf `subject.OU`; DEK-rotation warning | **M3** | M3 scope and acceptance |
| 5 | `RLIMIT_CORE = 0`, `Redacted<T>`, panic hook, single-instance lock, auto-lock plumbing, hardened runtime | M0 | M0 scope |
| 5 | Real auto-lock; `secrecy`/`zeroize`/`mlock` on live keys | M1 | M1 scope |
| 6 | Name-constrained CA with a discarded key, trusted leaf, canonical origin, TLS-only listener, in-process trust alert, decline-mode fingerprint confirmation, occupied-preferred-port warning, `pfp trust remove` | M0 | M0 scope and acceptance (three browsers; plaintext fails; loopback only; no TCC prompt) |
| 7.1–7.4 | Launch token via the in-process URL-open API, one-session binding, cookie-displacement recovery, `__Host-` cookie + `X-PFP-Proof`, route-scoped Host/Origin/`Sec-Fetch-Site` guards, exact headers, CSP, browser-storage prohibitions | M0 | M0 acceptance (421/403, navigation accepted, header snapshot, storage emptiness, zero CSP violations) |
| 7.5 | Zero outbound by default; deny-outbound sandbox; offline first launch | M0 | M0 acceptance |
| 7.5 | `pfp-net` confinement, per-feature toggles, network log | first opt-in outbound feature (post-1.0) | ADR-020; `PLAN.md` §5 |
| 8 | Logging rules, ring buffer and `os_log` `%{private}`, panic hook; diagnostics bundle (A10) exercised from M4 | M0 / M4 | ADR-022/023, R15 |
| 9 | Bounded parsers, no entity expansion, fuzzing, draft-only import | **M4** | M4 acceptance |
| 9 | Export escaping (M0); export/print warning and change-log entry (M4) | M0 / M4 | M0 and M4 scope |
| 10 | `Connector` + `SecretRef` frozen; sandbox connector | **M10** | M10 scope |
| 11 | Lockfiles, `cargo-deny`, `cargo-audit`, npm controls, Renovate, SBOM assertion, `cargo-auditable`, double-build, signed `SHA256SUMS` | M0, then every gate | M0 scope; gate items 3 and 5 |
| 12 | Universal build, Developer ID signing, hardened runtime, notarization, stapling experiment, DMG and tarball channels | M0 | M0 scope and acceptance; ADR-003 records the result |
| 13 | `.gitignore`, gitleaks rules, allowlist-free magic-byte hook and tree/archive gate, plaintext JSON demo plan, hygiene linter, user-data and credential-handling check, passphrase-flag lint, protected-path check, dollar-literal lint, seeded fixtures | M0 | M0 scope |
| 15 | The security suite as a first-class corpus | M0 onward | Gate item 2 |

---

## 15. Security checklist per milestone

Every milestone runs the full **release gate** (`PLAN.md` §4.13), of which items 2–6 are security items: security integration suite green; reproducible double-build with a published digest; `codesign --verify --strict` / `spctl` / `stapler validate` pass with a hardened runtime and no entitlements; CycloneDX SBOM with the no-copyleft assertion plus clean `cargo-deny` and `npm audit signatures`; and a **threat-model delta reviewed in the release PR**. The rows below are the *additional*, milestone-specific items; the id in brackets is the test's name in the security suite, and its kind follows.

| Milestone | Additional checklist |
|---|---|
| **M0** Trust rails | T1–T7 threat model and data classification published · foreign `Host` → 421, foreign `Origin` → 403; **a navigation with `Sec-Fetch-Site: none` to `/` is served**; the same header on any `/api/` route is **403**; a cross-site navigation is rejected [S-01, Playwright] · plaintext HTTP fails, no plaintext acceptor in the release profile [S-02] · loopback-only self-check [S-03] · header exact-match snapshot, zero CSP violations across a journey [S-04] · browser storage empty but for the proof token [S-05] · zero third-party requests [S-06] · deny-outbound sandbox and offline first launch [S-07] · launch token single-use and 60 s, cookie-only and proof-only rejected, **a non-browser client holding a stolen launch token cannot reach any plan endpoint**, and a bootstrap while unlocked forces re-unlock [S-08] · a second loopback origin overwriting `__Host-pfp` produces the documented 409 recovery path, not an unrecoverable 401 loop [S-28] · with the preferred port occupied, launch warns and the browser opens only at the fallback origin [S-29] · CA key zeroized, trust flow verified in three browsers, the in-process alert comes to the front with no TCC prompt, decline path prints a fingerprint **and requires the confirmation step** · Keychain/entitlement spike settled (4) · `RLIMIT_CORE = 0`, `Redacted<T>`, silent panic hook [S-20] · repo controls live **before** any user data can exist, and no `.pfplan` byte sequence exists in the tree or a release archive [S-09] · supply-chain gates, Actions pinned to commit SHAs, release key published out of band and its fingerprint shown on the About page [S-10] · double-build, SBOM, signing chain [S-11] · type-aware export escaping: a negative `Cents` round-trips as a number, a label beginning with `=` as inert text, JSON byte-identical [S-12] |
| **M1** Federal return + vault | Container v1 frozen as seam S4 · `docs/pfplan-format-spec.md` published with it (3.9) · RFC 9106 and XChaCha20-Poly1305 vectors [S-13] · byte-flip sweep never parses [S-14, property] · `headerTag` verified before any section decrypt; an edited slot descriptor, `sectionEpoch` or section table fails there [S-14] · KDF bounds on read [S-15] · calibration with the floor enforced · **N ordinary saves with a populated `secrets` section: its ciphertext is unchanged and it was never decrypted** — `rekey --rotate-dek`, which rewrites every section by design, is excluded (3.7) [S-30] · recovery-code print flow, the generated-passphrase option offered first, and the "no recovery is possible" acknowledgement · Python reference decryptor opens Rust-written files from test-time-generated fixtures [S-16] · crash-during-save fault-injected **inside each window of the commit-first sequence**, asserting `plan_path` always opens and that the "absent `plan_path`, present `bak.1`" recovery path is taken rather than a silent promotion [S-17] · canary disk scan [S-18] · 1 CPU-hour header fuzz [S-19] · real auto-lock over decrypted data, including **periodic API polling with no user interaction still locking at 15 minutes** · zeroizing signal and panic paths · unlock back-off survives a restart · mutation budget on touched crates |
| **M2** Next dollar | Patch and migration fuzz target [S-24] · `secrets` shape reserved, never decrypted on unlock · no plan facts in any log from new endpoints · every new endpoint carries both session factors and all admission checks · result pins stored, nothing recomputed silently |
| **M3** Lifetime ledger | `kSecAttrSynchronizable == false` asserted on the created item; the item **absent from backup and export paths**; **survives a re-sign with the same Developer ID**; **refused to a differently-signed binary**; the passphrase slot still opens the file after the item is deleted; item absent after opt-out [S-21, integration] · ACL bound to the designated requirement as `identifier` + leaf `subject.OU` · passphrase-change screen carries the DEK-rotation warning · `rekey --rotate-dek` verified to orphan an old-DEK copy **and to leave no old-DEK backup beside the plan file**, under both the re-encrypt and `--discard-backups` paths [S-22] |
| **M4** Scenarios + import | 1 CPU-hour fuzz per parser with committed corpora and no fact written without review [S-23] · importer limits table enforced and unit-tested · `vault compact --purge-before` round-trip read back with the reference decryptor, and the purge canary absent from the rewritten file and the rotated backup set (3.5; `TESTING.md` §8) · export/print warning plus change-log entry · diagnostics bundle written only to a user-chosen destination at mode 0600, with the same warning and change-log entry, plus the canary test [S-25] · **external review of `pfp-vault` and `pfp-server` commissioned at exit** (R19), handed the M1 format spec |
| **M5** Social Security | Earnings records confirmed encrypted-only — never a fixture, sample scenario or committed JSON · no new outbound dependency introduced by parameter sourcing |
| **M6** Monte Carlo | Paths never stored (summaries + seed only) · seeds stored inside the encrypted file · NDJSON streaming preserves the proof-header requirement · no nondeterminism that would make a pin unverifiable |
| **M7** Scorecard | No new persisted surface; the probability-without-magnitude serialization test covers exports as well as the API |
| **M6-M7** Uncertainty / scorecard | Review findings received and scheduled; anything high is remediated inside M8-M10 (R19) |
| **M8** Conversions / ACA | ACA and IRMAA inputs confirmed encrypted-only · review remediation in progress |
| **M9** Review loop | Snapshot diffs and review reports carry pins, never raw facts, to the browser · print routes obey the same CSP and `no-store` rules |
| **M10** State modules, connector seam, hardening | `Connector`/`SecretRef` frozen · sandbox connector proves the Keychain-secret path and no DTO can carry a `Secret` [S-26, type + integration] · reviewer findings triaged to **zero high** [S-27] · the whole suite green against the signed artifact. (The life modules are 1.1 by decision, `PLAN.md` M10; they add no new persisted surface beyond the already-reserved `properties{}` / `insurance{}` shapes) |

---

## 16. Open decisions carried from `DECISIONS.md` (not resolved here)

| # | Decision | Status |
|---|---|---|
| 1 | Ratify that "one self-contained executable" permits the minimal `.app`-in-DMG notarization container around the byte-identical executable | Open, pending the M0 stapling experiment |
| 3 | Firefox trust path if it does not honour the user-domain root: documented manual import versus fingerprint-compare only | Open; the M0 spike informs it |
| 2 | Release-key custody | Open. **Proposed resolution, pending ratification in `DECISIONS.md`:** the controls in 11 and 12 — out-of-band publication in two locations, fingerprint on the About page, rotation as new-key-signed-by-old, a two-key trust store, protected tag ruleset with a reviewer distinct from the author |
| 4 | Change-log retention: unbounded with compaction versus a capped undo history | Open. This is a confidentiality question, not only a growth question: `reverseDiff` payloads and snapshot bodies are the blast radius of T1 (1, A3). **Proposed resolution, pending ratification:** re-decide with **data minimization as a stated input**, with the purge operation in 3.5 available regardless of the retention default |
| 7 | External security reviewer, scope and budget | Open, **to be closed before M0 starts** (a third-party calendar dependency, R19); the timing itself is decided: commissioned at **M4 exit**, scoped to `pfp-vault` and `pfp-server`, handed the M1 format spec, zero-high gate at M10 |
| 9 | Stable preferred port versus always OS-assigned | Open. **Proposed resolution, pending ratification:** **preferred port with a mandatory warning** — the probe runs at every launch and an occupied preferred port produces the in-process alert of 6.3 before any fallback |

---

## 17. Deviations from the spine

Everything above follows `PLAN.md`, `ARCHITECTURE.md` and `DECISIONS.md`. The items below are detail added where the spine was silent, or a clarification where a literal reading is not implementable. None changes a spine decision; three (D7, D8, D9) concern a mechanism the spine names by name and are recorded here so the reasoning is stated once — each is ratified in the spine (ADR-006, ADR-015, ADR-003/005/023 respectively), so none is an open conflict.

- **D1 — AAD scope is split three ways: save-stable, section-stable and per-save.** ADR-005 says the header is authenticated as AAD "for every key wrap and every chunk". That is not literally implementable at either level. Re-wrapping every key slot on every save would need the passphrase *and* the recovery code, which is not held. And a `chunkAad` carrying `generation` or `modifiedAt` would invalidate every existing chunk on every save, forcing `secrets` — the highest-sensitivity asset (A4) — to be decrypted and re-encrypted whenever any figure changes, contradicting 3.3 and 10. This document therefore defines **`wrapAad`** over a save-stable subset, **`chunkAad`** over values fixed for the life of that section's ciphertext, and **`headerTag`** over the whole per-save header core. Every property ADR-005 asks for survives, now located precisely: KDF and slot tamper-evidence, section-table reordering and stale-header replay in `headerTag`; truncation, reordering and cross-section or cross-epoch substitution in `chunkAad`. Clarification, not a change of intent.
- **D2 — Plaintext length lives inside the first encrypted chunk**, not in the section table: a plaintext length in the header would defeat the bucket padding the spine requires.
- **D3 — Concrete numbers chosen here**: the STREAM nonce layout and the importer limits (32 MiB, 250,000 records, 64 KiB field, 512 columns, depth 64, 10 s). The spine fixes the constructions and the requirement to bound parsers, not the values; these are defaults recorded in the format spec.
- **D4 — Unlock back-off curve** (`min(2^(n-3), 60)` seconds after three attempts, restart after ten); ADR-015 requires back-off without stating the curve. **The counter persists across restarts** in the plaintext sidecar preference (3.4), because the process exits after auto-lock by design and an in-process-only counter would reset for exactly the attacker it targets.
- **D5 — Passphrase quality: a 12-character minimum and a heuristic strength display for typed passphrases, plus a generated high-entropy option offered first.** The original entry declined a wordlist estimator — a data dependency with its own licence and size cost, needing an ADR — and that still stands. **Amended**, because it answered the wrong question: against T1 the binding constraint is the entropy of the credential, not the quality of the meter, and the passphrase slot is the permanent floor of the design. The amendment adds no dependency, reusing the recovery code's machinery (128 CSPRNG bits, Crockford base32, six groups of five, check symbol) for the primary credential (3.4). The compensating control — the pre-passphrase statement that it is the only guaranteed way back into the file — is unchanged.
- **D6 — Not a deviation; recorded so the question is not reopened.** `Cross-Origin-Embedder-Policy: require-corp` would be a reasonable addition to 7.3, but the header set is exact-match snapshot-tested at M0 and `ARCHITECTURE.md` §5 names only the COOP/CORP pair. It is therefore **not** included, and 7.3 matches the spine exactly. Adding it later must update the M0 snapshot and this section in the same change.
- **D7 — The trust explanation is shown in-process, never by a spawned `osascript display dialog`.** That mechanism drives Apple Events from a spawned interpreter, which is what the hardened runtime and TCC are built to restrict, and it would want the very entitlements 12 forbids; it also attributes the dialog to "osascript" rather than to the application, and in the bare-executable channel there is no bundle identity to fix that. 6.2 therefore specifies `CFUserNotificationDisplayAlert` or an in-process `NSAlert`; ADR-006 (as amended) and `ARCHITECTURE.md` §8 carry the same mechanism. It does not touch the terminology block: a native alert panel is not an application window, and the UI remains a page in the user's browser.
- **D8 — `Sec-Fetch-Site: same-origin` is scoped by route.** ADR-015 (as amended) and `ARCHITECTURE.md` §5 carry the route-scoped rule, and `PLAN.md` M0 acceptance and `TESTING.md` §8 assert the navigation case. An unqualified rule is not implementable: a top-level navigation — from the address bar, a bookmark, or the application's own launch — carries `Sec-Fetch-Site: none`, so an unqualified rule would 403 the first request of every session, including the launch M0 acceptance requires to work in three browsers. 7.2 therefore requires `same-origin` on `/api/**` and accepts `none` on document routes only with `Sec-Fetch-Mode: navigate` at path `/`. The defence against T2 is strengthened rather than relaxed, because the `/api/**` rule also rejects `same-site` and an absent header. Recorded because the spine names the rule.
- **D9 — The demo plan ships as plaintext synthetic JSON, not as a `.pfplan`.** `ARCHITECTURE.md` §9 item 2, ADR-003 and `DOMAIN-MODEL.md` R9 embed the synthetic demo plan as `fixtures/plans/demo.plan.json`. A committed `.pfplan` could not coexist with the allowlist-free magic-byte controls in 13.2 and 13.4, and must not, since allowlisting that one path would blind the only control that catches a real household file committed by accident. 13.2 therefore specifies the plaintext fixture, encrypted by the binary at first use, and ADR-005/ADR-023 record the same rule.
