# Licence watchlist

*The restrictions that `cargo-deny` and the npm licence check pass silently, each row
date-stamped with the day it was last checked (`PLAN.md` §4.1; `TESTING.md` §2.3 and
§14 item 7; `SECURITY.md` §11.1; `docs/contributing.md` §5). This is the one
**non-mechanical** licence control: every project below is either outside the dependency
graph (a reference, an oracle, a fixture source, a data publisher), so no scanner ever
sees it, or is reported by GitHub as `NOASSERTION`, which the scanners accept. The release
gate requires a human to re-confirm every row and sign it off in the release pull
request (`PLAN.md` §4.13 item 11).*

**How a row is checked.** For a repository: the GitHub REST API record of the repository
(`license.spdx_id`, `pushed_at`) and the licence file in its default branch. For a page:
the page as served, read for a licence, terms-of-use or reproduction statement. A row
that could not be checked says **not confirmed on the date**, never a guess. A check is
a reading of what was seen on that day; it is not legal advice, and it grants nothing.

**Permitted uses** are the classes of ADR-004 and `docs/contributing.md` §5: *port with
attribution* (MIT/Apache/CC0), *vendor as data* (public-domain documents and CC0 data,
with provenance), *out-of-process oracle only* (GPL/AGPL), *read-only reference* (copy no
code, tests or data), *not bundled until terms are confirmed*.

## 1. Read-only references whose terms the scanners cannot see

| Project | What GitHub reports | What the licence file says | Last push | Permitted use | Checked |
|---|---|---|---|---|---|
| [TPAW Planner](https://github.com/bengmathew/tpaw) | `spdx_id: NOASSERTION` ("Other") | `LICENSE.md` (226 bytes): "source-available under the PolyForm Noncommercial License 1.0.0" | 2026-09-15 | Read-only reference: re-derive equations, copy nothing. A PolyForm component may never reach a shipped artifact (`SECURITY.md` §11, SBOM assertion) | 2026-09-22 |
| [prime-harvesting](https://github.com/hoostus/prime-harvesting) | `NOASSERTION` ("Other") | `LICENSE.md` (3,509 bytes): "The Parity Public License 7.0.0", contributor Justus Pendleton | 2025-03-25 | Read-only reference; a Parity component may never reach a shipped artifact | 2026-09-22 |
| [Beancount](https://github.com/beancount/beancount) | `GPL-2.0` | GPL-2.0 | 2026-08-23 | Read-only reference for booking semantics; the M9 lot-selection cases are written from scratch, no test copied (`TESTING.md` §4, §14) | 2026-09-22 |
| [hledger](https://github.com/simonmichael/hledger) | `GPL-3.0` | GPL-3.0 | 2026-09-22 | As Beancount | 2026-09-22 |
| [ukaia/retirement-planner](https://github.com/ukaia/retirement-planner) | `license: null` (no licence file detected) | No licence file in the default branch | 2026-07-04 | Read-only reference: an unlicensed repository grants nothing; copy no code, tests or data | 2026-09-22 |
| [mjcrepeau/retirement-planner](https://github.com/mjcrepeau/retirement-planner) | `license: null` | No licence file in the default branch | 2026-07-29 | As above | 2026-09-22 |

## 2. AnyPIA and AnyPIA-js

| Project | What was seen | Permitted use | Checked |
|---|---|---|---|
| [AnyPIA-js](https://github.com/codeforboston/anypia-js) | GitHub reports `spdx_id: NOASSERTION` ("Other"). **A `LICENSE.md` (1,290 bytes) exists** in the default branch; its text is a waiver: "We waive copyright and related rights in the work worldwide through the CC0 1.0 Universal public domain dedication in an effort to make this useful in the United States Government if desired." GitHub cannot classify that wording, hence `NOASSERTION`. Last commit `d4d1d154` dated 2021-03-11; the repository is dormant. **The design documents say the repository has no `LICENSE` file (`TESTING.md` §2.3, `PLAN.md` §4.1); as of this check that is not what the repository shows.** Whether the waiver covers the vendored SSA C++ sources (`anypiab/`, `oactobjs32/`) that the JavaScript wraps is not stated in the file and is the question a human has to settle before anything is taken | Not a dependency and not transliterated. Until a human has read `LICENSE.md` and recorded what it grants and to which files, treat as read-only reference. The M5 PIA fixtures are generated **through SSA's own program** and transcribed as tier-1 data (`TESTING.md` §3.6), not taken from this repository | 2026-09-22 |
| [AnyPIA, the SSA Social Security Benefit Calculator page](https://www.ssa.gov/OACT/anypia/) | **Not confirmed on 2026-09-22**: the page answered HTTP 403 to the check. The design's statement stands unverified: the page names no licence, and the basis for using the program's output is the presumption that a work of the United States Government is not subject to copyright in the United States (17 USC 105), which is a presumption about the program, not a statement on the page | Generate fixtures with the program; record the presumption, not a licence, in each fixture's provenance | 2026-09-22 (not confirmed) |

## 3. Pinned tier-2 suites: licences and dormancy (`TESTING.md` §3.5; ADR-004)

A suite is transliterated at a pinned commit; the pin and the tool version go into the
validation report and the `NOTICE` entry lands in the same pull request. None is
transliterated yet (`docs/validation-report.md`: tier 2 not yet introduced). The dormancy
date matters because a dormant suite will not track a law change, and its assertions
are then evidence about the law of its last commit.

| Suite | What GitHub reports | Licence file | Last push | Note | Checked |
|---|---|---|---|---|---|
| [Open Social Security](https://github.com/MikePiper/open-social-security) | `MIT` | MIT | 2026-09-14 | Active | 2026-09-22 |
| [ssa.tools](https://github.com/Gregable/social-security-tools) | `MIT` | MIT | 2026-09-18 | Active | 2026-09-22 |
| [boknows/cFIREsim-open](https://github.com/boknows/cFIREsim-open) | `Apache-2.0` | Apache-2.0 (the `boknows` repository; not a fork) | **2022-04-08** | **Dormant for more than four years.** The rule assertions are still usable under Apache-2.0; the bundled return series is stale and is never a data fixture (`TESTING.md` §3.5) | 2026-09-22 |
| [Tax-Calculator](https://github.com/PSLmodels/Tax-Calculator) | `NOASSERTION` ("Other") | `LICENSE`: "The Tax-Calculator project is in the public domain within the United States. Additionally, we waive copyright and related rights in the work worldwide through the CC0 1.0 Universal public domain dedication." | 2026-09-21 | Active. The scanner would pass this silently because of the `NOASSERTION`; the licence file is the evidence (`TESTING.md` §7 already records the `NOASSERTION`) | 2026-09-22 |
| [muirjc/retirement-planner](https://github.com/muirjc/retirement-planner) | `MIT` | MIT | 2026-09-16 | Active; one type shape is mirrored (ADR-004) | 2026-09-22 |
| [R4GoodPersonalFinances](https://cran.r-project.org/package=R4GoodPersonalFinances) (CRAN) | Not a GitHub record | CRAN page: `License: MIT + file LICENSE`, version 1.2.0, published 2025-11-23 | 2025-11-23 (CRAN publication) | Active | 2026-09-22 |

## 4. Out-of-process oracles (never linked, vendored, imported or shipped)

| Project | What GitHub reports | Last push | Permitted use | Checked |
|---|---|---|---|---|
| [Owl](https://github.com/mdlacasse/Owl) | `GPL-3.0` | 2026-09-21 | Subprocess and file format only; outputs recorded as goldens (`TESTING.md` §7) | 2026-09-22 |
| [PolicyEngine-US](https://github.com/PolicyEngine/policyengine-us) | `AGPL-3.0` | 2026-09-22 | Its YAML is fetched at a pinned commit into a git-ignored cache and never committed; parameter values are never sourced from it (ADR-004) | 2026-09-22 |
| [policyengine-taxsim](https://github.com/PolicyEngine/policyengine-taxsim) | `MIT` | 2026-09-22 | The MIT command-line adapter through which the AGPL oracle is reached | 2026-09-22 |

## 5. Third-party capital-market-assumption sets and datasets: not bundled until terms are confirmed

`SIMULATION-SPEC.md` §3.2 and §4.4, `PLAN.md` R16 and `ARCHITECTURE.md` §10.3: a
vendor's published forecast is proprietary research output, and republishing its tables
inside an Apache-2.0 repository is a right the project does not hold until the publisher
grants it. v1 ships the vintage shape, the loader, published checksums and the user's own
entry. **No set below may be bundled**; a set whose publisher grants permission ships as a
named vintage with the permission recorded in this table.

| Set | What was seen on the publisher's page | Status | Checked |
|---|---|---|---|
| [J.P. Morgan Long-Term Capital Market Assumptions](https://am.jpmorgan.com/us/en/asset-management/institutional/insights/portfolio-insights/ltcma/) | No reproduction or terms-of-use statement found on the landing page as served | **Redistribution terms unconfirmed**; not bundled | 2026-09-22 |
| [AQR Capital Market Assumptions, 2026](https://www.aqr.com/Insights/Research/Alternative-Thinking/2026-Capital-Market-Assumptions-for-Major-Asset-Classes) | "This document is not to be reproduced or redistributed to any other person." and "©2026 AQR Capital Management, LLC. All rights reserved." | **Redistribution refused by the publisher**; never bundled; the user's own entry only | 2026-09-22 |
| [Northern Trust Capital Market Assumptions, 2026](https://ntam.northerntrust.com/content/dam/northerntrust/investment-management/global/en/documents/thought-leadership/2026/cma/2026-capital-market-assumptions-report.pdf) | The PDF was fetched but no readable terms could be extracted from it | **Not confirmed on 2026-09-22**; not bundled | 2026-09-22 (not confirmed) |
| [Vanguard Capital Markets Model forecasts](https://corporate.vanguard.com/content/corporatesite/us/en/corp/vemo/vemo-return-forecasts) | Investment disclaimers only; no reproduction or terms-of-use statement found on the page as served | **Redistribution terms unconfirmed**; not bundled | 2026-09-22 |
| Shiller and Damodaran historical return datasets (`SIMULATION-SPEC.md` §4.4: "carry no explicit licence") | Not checked: the design names no publisher URL to check | **Not confirmed on 2026-09-22**; loader and checksums only, never bundled | 2026-09-22 (not confirmed) |
| NCHS life tables (`SECURITY.md` §11.1, "vendor as data") | Not checked in this pass | A work of the United States Government by presumption (17 USC 105); to be confirmed against the publisher's page and archived with a checksum before any value is vendored | 2026-09-22 (not confirmed) |

## 6. Other restrictions the corpus refuses (`TESTING.md` §5.3)

| Item | What was seen | Status | Checked |
|---|---|---|---|
| [finiki, Variable percentage withdrawal](https://www.finiki.org/wiki/Variable_percentage_withdrawal) | Footer: content available under Creative Commons Attribution-ShareAlike 4.0; the VPW table is on the page | **Not vendored** (CC BY-SA). The table is generated from the formula, with three spot values as fixtures (`TESTING.md` §5.3) | 2026-09-22 |
| Bogleheads wiki VPW table | Not checked: `TESTING.md` §5.3 records the wiki's own licence as unestablished and assumes the stricter case | Not vendored | 2026-09-22 (not confirmed) |
| MoneyGuidePro "75–90% Confidence Zone" | A web search on the date found only third-party pages describing the band; no vendor documentation was located | **Not confirmed on 2026-09-22**; only the glossary definition (a user-selected target range) is verified, and the band is not a shipped constant (`TESTING.md` §5.3) | 2026-09-22 (not confirmed) |

## 7. Findings against the design documents from this check

Recorded here so that the next reviewer does not re-discover them; the design documents
are not edited by the check (`docs/contributing.md` §3).

1. `TESTING.md` §2.3 and `PLAN.md` §4.1 describe AnyPIA-js as having no `LICENSE` file.
   On 2026-09-22 the repository's default branch holds `LICENSE.md` with a CC0-style
   waiver, unrecognised by GitHub (`NOASSERTION`). The wording of both documents is stale
   and the row above records the file; what the waiver covers is still for a human to
   settle.
2. The SSA AnyPIA page could not be fetched (HTTP 403), so the design's "no stated
   licence" description of that page is neither confirmed nor contradicted by this
   check.

## 8. Re-checking at each release (`PLAN.md` §4.13 item 11)

1. For every repository row: fetch the API record and the licence file again; update
   "What GitHub reports", "Licence file", "Last push" and "Checked".
2. For every page row: read the page again; if it cannot be read, write "not confirmed
   on <date>" and leave the previous reading in place, dated.
3. Compare `NOTICE` against ADR-004's enumerated list: every source the release ports or
   transliterates has an entry with repository URL, licence and pinned commit.
4. Sign the review off in the release pull request. A row without a date is a gate
   failure; a date without a reading is too.
