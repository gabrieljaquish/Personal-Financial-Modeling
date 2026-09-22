# Threat model

*The threats this application defends against, the controls that answer each, and what
is out of scope. `SECURITY.md` §2 is the **source of record**: this document restates it
for a reader who wants the table without the design, and points back into it. Where the
two ever differ, `SECURITY.md` wins and this file is wrong. Shipped at M0 (`PLAN.md`
§4.1); the seven threats are frozen in `ARCHITECTURE.md` §8. Every release reviews a
threat-model delta in its pull request (`PLAN.md` §4.13 item 6; `TESTING.md` §11.2 gate 8).*

## What is protected

One thing: the household's financial plan, held in a single encrypted file (`*.pfplan`)
and decrypted only inside the application's own process while it is unlocked. The full
asset list, with where each may and may never exist, is `docs/data-classification.md`
(from `SECURITY.md` §1).

## Trust boundaries

`SECURITY.md` §2.1 draws them; in words:

| Boundary | Between | Crossed by |
|---|---|---|
| B1 | The user's browser and the application process | A TLS 1.3 loopback socket carrying the session cookie, the proof header and the exact `Host` / `Origin` / `Sec-Fetch-Site` headers |
| B2 | The process and the plan file | Reads and atomic writes of the encrypted container and its backups |
| B3 | The process and the login keychain | The opt-in key slot, behind a keychain ACL |
| B4 | The process and files the user chooses to import | From-scratch bounded parsers |
| B5 | The build and its supply chain | Locked dependency graphs, pinned toolchain, signed and reproducible releases |
| B6 | The public repository and the world | Repository hygiene: nothing personal is ever committed |

## Threats T1–T7 and their principal controls

The table is `SECURITY.md` §2.2, condensed. The last column names the section of
`SECURITY.md` that specifies the controls; the milestone in which each control lands is
`SECURITY.md` §14–§15.

| # | Threat | Principal controls | `SECURITY.md` |
|---|---|---|---|
| T1 | **Holder of a copy of the plan file** (stolen laptop, backup disk, sync folder, forwarded file) | Argon2id-derived key-encryption key at a calibrated cost with a hard floor; a random 256-bit data key; XChaCha20-Poly1305 STREAM; the header authenticated before it is parsed; bucket padding; no plaintext temporary or write-ahead files; `rekey --rotate-dek` | §3 |
| T2 | **Malicious web page in the same browser** (CSRF, DNS rebinding, cross-origin reads, loopback port scans) | Exact `Host` allowlist (421); exact `Origin` and `Sec-Fetch-Site` (403); the cookie **and** the `X-PFP-Proof` header together; JSON content type enforced; a custom header that forces a preflight which is never approved; no CORS or private-network headers; strict Content Security Policy; `Cache-Control: no-store` | §7 |
| T3 | **Another local unprivileged process or user, including port squatting** | Loopback-only bind with a start-up self-check; a name-constrained local certificate authority whose key is destroyed after signing one leaf, so a squatter cannot present a trusted certificate; a mandatory warning when the preferred port is occupied; a single-instance lock; `0600` file modes; a keychain ACL bound to the code-signing requirement | §6, §4 |
| T3-s | **T3 reaching for the session credentials** | The launch token travels only through an in-process URL-open call, never argv, environment, file or clipboard; the decrypted plan is bound to one session; the passphrase is never a flag or an environment variable. Two limits are stated, not hidden: the header triad is browser-supplied and therefore a T2 control only, and the `__Host-` cookie is not integrity-protected against other loopback origins, which the port-scoped proof reduces to an availability problem with a defined recovery | §7, §6 |
| T4 | **Supply chain** (a malicious crate, npm package, build step or artifact) | Lockfiles and a pinned toolchain; `cargo-deny`; `cargo-audit`; `npm ci --ignore-scripts`; `npm audit signatures`; licence checks; Renovate with a seven-day release age; no CDN, fonts or analytics; a reproducible double build; a software bill of materials asserting no copyleft; a signed `SHA256SUMS` | §11, §12 |
| T5 | **Developer or contributor error**, including real data in an AI-assisted session | gitleaks pre-commit, in CI and nightly over full history; the container-magic hook and tree scan **with no allowlist, ever**; the data-hygiene linter; the synthetic-fixture policy; the protected-path check; the dollar-literal lint; the redacted diagnostics bundle | §13 |
| T6 | **Hostile input file** handed to the importer while a plan is unlocked | From-scratch bounded parsers; no entity expansion, external references or network; a wall-clock budget; `forbid(unsafe_code)`; one CPU-hour of fuzzing per parser per release; importers produce a reviewed draft and never write facts | §9 |
| T7 | **Browser residue** | No service worker, `localStorage`, IndexedDB, Cache API or WebAssembly; `sessionStorage` holds only the proof token; `no-store`; opaque ids in URLs; the launch token in the fragment, cleared at once; asserted by a browser suite at each release | §7 |

### What exists at M0

The controls for T2, T3, T3-s, T4, T5 and T7 that the server, the repository gates and the
front end can provide are built and tested in this repository (`README.md`, "Security
test ids in this step"). The container (T1), the keychain slot (T3, M3), the importers
(T6, M4) and the browser suite that asserts the T7 rows at run time do not exist yet; the
validation report on the About page says which corpora exist and which are not yet
introduced (`docs/validation-report.md`).

## Explicitly out of scope

Stated to users on the About page, exactly as `SECURITY.md` §2.3 states it. The
application does **not** defend against:

| Out of scope | What the user is told |
|---|---|
| Malware running as the same user, or as root | It can read process memory, keystrokes and the decrypted plan; no user-space design defeats it |
| A malicious browser extension with all-sites access | It runs inside the application's origin and can read what the screens show (R20). Use a dedicated clean browser profile, as the first-run flow says |
| Memory forensics of an unlocked process; swap or hibernation images | `mlock` and `zeroize` reduce but do not eliminate exposure; auto-lock and exit-after-lock shorten the window (R14) |
| Coercion | Out of scope by construction |
| An attacker with **write** access rolling the file back to an older authentic version | Each version's authenticity is guaranteed; freshness is not. `generation` catches *accidental* stale copies only |
| File-size traffic analysis beyond the 64 KiB bucket | Padding hides small differences, not order-of-magnitude plan size |
| **macOS CrashReporter** | On any abnormal termination macOS writes an `.ips` report to `~/Library/Logs/DiagnosticReports`, which the application cannot suppress, and Apple receives it when Analytics sharing is on. The controls are to hold no secret in a recoverable state at abort time (`SECURITY.md` §5) and to say so plainly: turn Analytics sharing off if that matters to you |
| **Residual metadata of the launch** | Opening the browser goes through LaunchServices, which records the event in the unified log, and the URL reaches browser history for the instant before `history.replaceState` clears the fragment. The token is single-use with a 60-second TTL, so what survives is the fact and time of a launch, not a usable credential |
| **Copies already made**, and purge | `vault compact --purge-before` and `rekey --rotate-dek` reach the plan file and the backups beside it, never a Time Machine snapshot, a cloud-sync version history or a file already forwarded. On APFS neither can guarantee that overwritten blocks are unrecoverable |
| **Decline mode** (the user refuses the trust anchor, `SECURITY.md` §6.2) | Without the trust anchor the application cannot prove it is the program answering on this port, so the T3 port-squatting protection rests **entirely** on the user comparing the fingerprint. The unlock screen makes that comparison a required step rather than an option |

## Release-process limits stated here by design

`SECURITY.md` §11 requires two statements to live in this document:

- **Release approval by a single maintainer.** Release workflows run only from a
  protected tag ruleset behind a protected environment. With more than one maintainer
  the manual approval is by a reviewer distinct from the author; with a single
  maintainer it is a manual approval no sooner than 24 hours after the tag, recorded in
  the environment's deployment log (`PLAN.md` R26).
- **Purge and re-key reach only what they can reach.** As in the table above: the plan
  file and its sibling backups, never copies already elsewhere (`DECISIONS.md` ADR-009).

## Where to read more

`SECURITY.md` §2 (this document's source), §3 (the container), §6–§7 (the loopback
server), §11 (supply chain), §13 (repository hygiene); `ARCHITECTURE.md` §8 (the
summary that freezes T1–T7); `TESTING.md` §8 (the security test matrix, green at every
release gate); `docs/data-classification.md` (the assets).
