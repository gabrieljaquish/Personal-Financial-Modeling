# `fixtures/pending/` — fixtures awaiting human verification

Every file in this directory carries `"verification": "pending-hand-verification"`,
which `docs/TESTING.md` §2.2 says the **tier-1 loader rejects**: a tier-1 fixture
must be `primary-source-confirmed` or `hand-worked-reviewed`, and nothing else.

These fixtures were written from the archived primary documents under
`params/provenance/` **before any implementation existed**, which is what
`docs/contributing.md` §3.1 requires. They were written by an AI-assisted session,
which is why they are here and not in `fixtures/tier1/`: §3.1 also says the
assistant never authors both a constant and the test that checks it, so a human
reads every expected value back against the cited document before a file moves.

## Promotion

Each file names the tier-1 id it would be promoted to in its `promotesTo` field.
Promotion means: a human reads the expected values back against the archived
document at the stated locator, changes `verification` to
`primary-source-confirmed` (or, for a derived value with no publication printing
it, to `hand-worked-reviewed` with the `derivation` and `reviewedBy` fields
§2.2 requires), and moves the file under `fixtures/tier1/`. `fixtures/tier1/` is
protected by `cargo xtask protected-paths`.

## Directory map

| Here | Promotes to |
|---|---|
| `published/` | — (transcription inputs, not a promotable fixture) |
| `uprating/` | `t1/uprating/*` |
| `schedule/` | `t1/schedule/*` |
| `rounding/` | `t1/rounding/table` |
| `money/` | `t1/money/mul_ratio` |

## Read this before promoting anything under `uprating/`

The statutory uprating pipeline, computed exactly from the statutory base-year
amounts and the archived C-CPI-U series, **does not reproduce most published 2026
or 2025 bracket thresholds**. Each affected fixture carries `computed`,
`published` and `residual` as three separate fields, and none of them was tuned.
The diagnosis, the evidence that the pipeline *shape* is nonetheless correct, and
the decision a human has to make are in the authoring log. Do not promote an
uprating fixture, and do not lock a vintage, until that decision is recorded.

## `paramVintage` is `null` in every file, deliberately

`docs/TESTING.md` §2.2's envelope carries a `paramVintage` field. It is `null`
here because **no vintage exists yet**: seam S1 has not frozen, `params/VINTAGES.lock`
carries no entry, and inventing a vintage id would be exactly the kind of
plausible-but-unsourced value ADR-022 exists to prevent. Read it as "not yet
applicable", not as an omission. It is filled in at promotion, against the
vintage the fixture is then pinned to.

`uprating/2025-brackets-control.json` likewise has `promotesTo: null`. That is
deliberate and it is **not** a candidate for deletion: it is the positive control
that distinguishes a wrong statutory reading from a wrong index vintage, and it
is the reason the other uprating fixtures' residuals can be attributed at all.

## Synthetic data

Inputs marked `"synthetic": true` are invented for the test and describe no
person, household or real financial position (`docs/contributing.md` §1.3, §3.5).
Bracket tables, base amounts and index values are public law and public
statistics, cited to an archived primary document with its checksum.
