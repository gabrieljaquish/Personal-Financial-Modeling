// The About page: the honest headline first, computed from the report's counts;
// the build identity and licence; the validation report as accessible tables in
// which a section the tree does not hold yet says "not yet introduced (milestone
// Mx)" instead of disappearing; and the out-of-scope statement of SECURITY.md
// section 2.3. Rendered from the server's golden (a FUTURE-shaped report, so
// nothing on the page can be hard-coded to today's zeros) and from the M0 shape
// derived from it.

import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { test } from 'node:test';
import { createElement } from 'react';

import { AboutView, OUT_OF_SCOPE } from '../src/screens/about/AboutView.tsx';
import { headlineOf, reportVm, stateWords } from '../src/viewmodel/validation.ts';
import { assertAccessible } from './support/a11y.mjs';
import * as golden from './support/golden.mjs';
import { byAttr, byId, byTag, textOf } from './support/markup.mjs';
import { render } from './support/render.mjs';

const status = { kind: 'ready', value: golden.sessionStatus() };
const future = golden.validationReport();

/** Today's tree, derived from the golden rather than typed: zero tier-1, no plan fixture, pending only, an unlocked, unverified vintage. */
function m0Shape() {
  const report = structuredClone(future.report);
  const [tier1, tier2, tier3, personas, plans, pending] = report.fixtures.tiers;
  const emptied = { fileCount: 0, files: [], byVerification: [], byMilestone: [], byModule: [], byMilestoneAndVerification: [], reviewKinds: [] };
  Object.assign(tier1, { ...emptied, status: { ...tier1.status, state: 'empty' } });
  Object.assign(plans, { ...emptied, status: { ...plans.status, state: 'empty' } });
  Object.assign(pending, {
    fileCount: 21,
    byVerification: [{ name: 'pending-hand-verification', count: 21 }],
    byMilestoneAndVerification: [{ milestone: 'M0', verification: 'pending-hand-verification', count: 21 }],
  });
  report.fixtures.tiers = [tier1, tier2, tier3, personas, plans, pending];
  report.fixtures.invalid = [];
  report.fixtures.otherDirectories = [];
  const [vintage] = report.parameters.vintages;
  delete vintage.lockedId;
  Object.assign(vintage, { locked: false, verified: false, verification: [{ name: 'pending-hand-verification', count: 3 }] });
  report.parameters.lock = { present: false, entryCount: 0, lockedVintages: [], unverifiedVintages: [], checksumMismatchCount: 0, missingCount: 0, unlistedCount: 0, note: 'params/VINTAGES.lock does not exist; no vintage is locked.' };
  Object.assign(report.unverified, { tier1PrimarySourceConfirmedCount: 0, tier1HandWorkedReviewedCount: 0, pendingFixtureCount: 21, pendingParameterDocumentCount: 3 });
  report.pins.lockedParamVintageIds = [];
  return { state: 'generated', report };
}

const view = (report, statusLoadable = status) => render(createElement(AboutView, { status: statusLoadable, report, onRetry() {} })).tree;
/** "verified" as a word of its own, not inside "unverified" or "verification". */
const claimsVerified = (text) => /(?<!un)\bverified\b/.test(text.replaceAll('treat as unverified', ''));

const STATES = [
  { kind: 'idle' },
  { kind: 'loading' },
  { kind: 'failed', failure: { kind: 'unreachable' } },
  { kind: 'failed', failure: { kind: 'defect', code: 'malformed_response' } },
  { kind: 'ready', value: { state: 'not-generated' } },
  { kind: 'ready', value: future },
  { kind: 'ready', value: m0Shape() },
];

test('every state passes the checklist, with the heading as the one focus target', () => {
  for (const report of STATES) {
    for (const s of [status, { kind: 'idle' }, { kind: 'failed', failure: { kind: 'unreachable' } }]) {
      assertAccessible(view(report, s), { focusTargets: ['screen-heading'] });
    }
  }
});

test('the headline is the first content after the heading, and it is computed from the counts', () => {
  const tree = view({ kind: 'ready', value: m0Shape() });
  const section = byTag(tree, 'section')[0];
  const [heading, headline] = section.children.filter((c) => c.tag !== undefined);
  assert.equal(heading.attrs.id, 'screen-heading');
  assert.equal(textOf(heading), 'About this application');
  assert.equal(headline.tag, 'p');
  assert.equal(
    textOf(headline, { includeHidden: false }),
    '0 tier-1 fixtures; 21 fixtures pending human verification; parameter vintage federal-2026 unlocked and pending human verification.',
  );
  // A9: the glyph is decoration.
  assert.equal(byAttr(headline, 'aria-hidden', 'true').length, 1);
});

test('honesty: the M0 shape never says "verified", never says "locked", and counts pending as pending', () => {
  const tree = view({ kind: 'ready', value: m0Shape() });
  const text = textOf(tree);
  assert.ok(!claimsVerified(text), 'the word "verified" appears nowhere');
  assert.ok(!/\bLocked as\b/.test(text));
  const fixtures = byTag(tree, 'table')[0];
  const rows = byTag(byTag(fixtures, 'tbody')[0], 'tr');
  const cells = (row) => [textOf(byTag(row, 'th')[0]), ...byTag(row, 'td').map((td) => textOf(td))];
  assert.deepEqual(cells(rows[0]), ['tier1', 'Empty (specified for milestone M0)', '0', '0', '0', '0', '0', '0', '0']);
  assert.deepEqual(cells(rows[4]), ['plans', 'Empty (specified for milestone M0)', '0', '0', '0', '0', '0', '0', '0']);
  assert.deepEqual(cells(rows[5]), ['pending', 'Present (milestone M0)', '21', '0', '0', '21', '0', '0', '0']);
  // Nothing was unreadable and no unlisted directory exists: the table says so rather than vanishing.
  const unread = byTag(tree, 'table')[1];
  assert.deepEqual(cells(byTag(byTag(unread, 'tbody')[0], 'tr')[0]), ['none', 'every fixture file was read', 'no fixture directory outside those listed above exists']);
  assert.match(text, /0 primary-source-confirmed; 0 hand-worked-reviewed/);
});

test('honesty the other way: a future shape with promoted fixtures and a locked vintage renders its counts, so nothing is hard-coded', () => {
  const tree = view({ kind: 'ready', value: future });
  const text = textOf(tree);
  assert.equal(headlineOf(future.report), '3 tier-1 fixtures; 1 fixture pending human verification; parameter vintage federal-2026 locked and verified.');
  assert.ok(text.includes(headlineOf(future.report)));
  const fixtures = byTag(tree, 'table')[0];
  const first = byTag(byTag(fixtures, 'tbody')[0], 'tr')[0];
  assert.deepEqual([textOf(byTag(first, 'th')[0]), ...byTag(first, 'td').map((td) => textOf(td))], ['tier1', 'Present (milestone M0)', '3', '2', '1', '0', '0', '0', '0']);
  const vintages = byTag(tree, 'table')[3];
  const row = byTag(byTag(vintages, 'tbody')[0], 'tr')[0];
  assert.deepEqual(byTag(row, 'td').map((td) => textOf(td)).slice(0, 2), ['yes', 'yes']);
  assert.match(text, /2 primary-source-confirmed; 1 hand-worked-reviewed/);
  assert.match(text, /Locked parameter vintage ids/);
  // Plurals follow the counts.
  const one = structuredClone(future.report);
  one.fixtures.tiers[0].fileCount = 1;
  assert.match(headlineOf(one), /^1 tier-1 fixture; 1 fixture pending/);
  const none = structuredClone(future.report);
  none.parameters.vintages = [];
  assert.match(headlineOf(none), /; no parameter vintage\.$/);
});

test('a fixture the generator could not read, and a directory the design does not name, are shown so the columns add up', () => {
  const tree = view({ kind: 'ready', value: future });
  const cells = (row) => [textOf(byTag(row, 'th')[0]), ...byTag(row, 'td').map((td) => textOf(td))];
  const fixtures = byTag(tree, 'table')[0];
  assert.match(textOf(byTag(fixtures, 'caption')[0]), /the last six columns add up to Documents/);
  const rows = byTag(byTag(fixtures, 'tbody')[0], 'tr');
  // The caption states an identity; make it self-enforcing on every row:
  // Documents (column 3) equals the sum of the six verification columns after it.
  for (const row of rows) {
    const c = cells(row);
    assert.equal(c.length, 9, `${c[0]}: nine cells`);
    const documents = Number(c[2]);
    const parts = c.slice(3).map(Number);
    assert.equal(parts.reduce((a, b) => a + b, 0), documents, `${c[0]}: the last six columns add up to Documents`);
  }
  assert.deepEqual(rows.map((row) => textOf(byTag(row, 'th')[0])), ['tier1', 'tier2', 'tier3', 'personas', 'plans', 'pending']);
  // plans: two documents, one read with an unstated verification value, one unreadable.
  assert.deepEqual(cells(rows[4]), ['plans', 'Present (milestone M0)', '2', '0', '0', '0', '1', '1', '0']);
  const unread = byTag(tree, 'table')[1];
  assert.match(textOf(byTag(unread, 'caption')[0]), /could not read/);
  assert.deepEqual(byTag(byTag(unread, 'tbody')[0], 'tr').map(cells), [
    ['fixtures/plans/broken.json', 'unreadable: not counted under any verification value', 'not JSON: synthetic parse error'],
    ['fixtures/scratch/', 'directory the design does not name: counted, not read', '1 file(s)'],
  ]);
  // A snapshot file is counted but never read as an envelope: the last column says so.
  const counted = structuredClone(future);
  const tier3 = counted.report.fixtures.tiers[2];
  Object.assign(tier3, { fileCount: 2, status: { ...tier3.status, state: 'present' } });
  const again = view({ kind: 'ready', value: counted });
  assert.deepEqual(cells(byTag(byTag(byTag(again, 'table')[0], 'tbody')[0], 'tr')[2]), ['tier3', 'Present (milestone M0)', '2', '0', '0', '0', '0', '0', '2']);
});

test('a section the tree does not hold yet says "not yet introduced (milestone Mx)" and is never omitted', () => {
  const tree = view({ kind: 'ready', value: future });
  const sections = byTag(tree, 'table')[4];
  assert.equal(textOf(byTag(sections, 'caption')[0]), 'Corpora by section: what exists at this commit, and what the design places later');
  const rows = byTag(byTag(sections, 'tbody')[0], 'tr');
  const byName = Object.fromEntries(rows.map((row) => [textOf(byTag(row, 'th')[0]), byTag(row, 'td').map((td) => textOf(td))]));
  assert.deepEqual(Object.keys(byName), [
    'Tier-1 fixtures',
    'Tier-2 pinned suites',
    'Tier-3 goldens',
    'Synthetic personas',
    'Plan fixtures (the demo plan, migration shapes)',
    'Recorded oracles',
    'Property-based invariants',
    'Contract snapshots',
    'Mutation score',
    'Fuzz corpora',
    'Security suite',
    'Browser end-to-end',
    'Performance budgets',
  ]);
  assert.equal(byName['Tier-2 pinned suites'][0], 'Not yet introduced (milestone M1)');
  assert.equal(byName['Tier-3 goldens'][0], 'Not yet introduced (milestone M3)');
  assert.equal(byName['Synthetic personas'][0], 'Not yet introduced (milestone M0)');
  assert.equal(byName['Plan fixtures (the demo plan, migration shapes)'][0], 'Present (milestone M0)');
  // The M0 shape: the demo plan is an M0 obligation the tree does not meet, said as empty, not dropped.
  const m0 = view({ kind: 'ready', value: m0Shape() });
  const m0Rows = byTag(byTag(byTag(m0, 'table')[4], 'tbody')[0], 'tr');
  const m0ByName = Object.fromEntries(m0Rows.map((row) => [textOf(byTag(row, 'th')[0]), textOf(byTag(row, 'td')[0])]));
  assert.equal(m0ByName['Plan fixtures (the demo plan, migration shapes)'], 'Empty (specified for milestone M0)');
  // A report that carries no entry for a named directory is shown as absent, never upgraded.
  const missing = structuredClone(future);
  missing.report.fixtures.tiers = missing.report.fixtures.tiers.filter((t) => t.tier !== 'personas');
  const withoutPersonas = view({ kind: 'ready', value: missing });
  const absentRow = byTag(byTag(byTag(withoutPersonas, 'table')[4], 'tbody')[0], 'tr').find((row) => textOf(byTag(row, 'th')[0]) === 'Synthetic personas');
  assert.deepEqual(byTag(absentRow, 'td').map((td) => textOf(td)), ['Not yet introduced (milestone not stated)', 'not in this report', 'This report carries no entry for fixtures/personas/; treat it as absent.']);
  assert.equal(byName['Fuzz corpora'][0], 'Not yet introduced (milestone M4)');
  assert.equal(byName['Mutation score'][0], 'Not yet introduced (milestone M1)');
  assert.equal(byName['Performance budgets'][0], 'Not yet introduced (milestone M0)');
  assert.equal(byName['Browser end-to-end'][0], 'Partly present (milestone M0)');
  assert.equal(byName['Contract snapshots'][0], 'Present (milestone M0)');
  // The note and the reference travel with the status.
  assert.equal(byName['Tier-2 pinned suites'][2], 'No suite has been transliterated.');
  assert.ok(byName['Tier-2 pinned suites'][1].startsWith('TESTING.md'));
  // Every state word, and an unknown one, is said honestly.
  for (const [state, words] of [
    ['not-yet-introduced', 'Not yet introduced (milestone M9)'],
    ['empty', 'Empty (specified for milestone M9)'],
    ['partial', 'Partly present (milestone M9)'],
    ['present', 'Present (milestone M9)'],
    ['something-new', 'Unknown state "something-new" - treat as not present (milestone M9)'],
  ]) {
    assert.equal(stateWords({ state, milestone: 'M9', reference: 'r', note: 'n' }), words);
  }
});

test('no report: a build without one says so first; an unread or unreadable report says so; retry only where it helps', () => {
  const absent = view({ kind: 'ready', value: { state: 'not-generated' } });
  assert.match(textOf(byTag(absent, 'p')[0]), /^△ This build carries no validation report/);
  assert.match(textOf(byTag(absent, 'p')[0]), /nothing on this page is validated/);
  assert.equal(byTag(absent, 'table').length, 0);
  assert.equal(byTag(absent, 'button').length, 0);

  const unread = view({ kind: 'loading' });
  assert.match(textOf(byTag(unread, 'p')[0]), /has not been read yet/);
  assert.equal(byTag(unread, 'section')[0].attrs['aria-busy'], 'true');

  const failed = view({ kind: 'failed', failure: { kind: 'unreachable' } });
  assert.match(textOf(byTag(failed, 'p')[0]), /could not be read/);
  assert.match(textOf(byAttr(failed, 'role', 'alert')[0]), /is not answering/);
  assert.deepEqual(byTag(failed, 'button').map((b) => textOf(b)), ['Try again']);
  assert.equal(byTag(view({ kind: 'failed', failure: { kind: 'defect', code: 'x' } }), 'button').length, 0);
  assert.equal(reportVm({ kind: 'idle' }).tables.length, 0);
});

test('every table has a caption, column headers and a row header per row', () => {
  const tree = view({ kind: 'ready', value: future });
  const tables = byTag(tree, 'table');
  assert.equal(tables.length, 7);
  for (const table of tables) {
    assert.ok(textOf(byTag(table, 'caption')[0]) !== '');
    const columns = byTag(byTag(table, 'thead')[0], 'th');
    assert.ok(columns.length >= 2);
    for (const row of byTag(byTag(table, 'tbody')[0], 'tr')) {
      assert.equal(byTag(row, 'th').length, 1);
      assert.equal(byTag(row, 'td').length + 1, columns.length);
    }
    const region = table.parent;
    assert.equal(region.attrs.role, 'region');
    assert.equal(region.attrs['aria-labelledby'], byTag(table, 'caption')[0].attrs.id);
  }
  // The unverified block is printed, not hidden, with its milestone.
  const unverified = tables[5];
  assert.match(textOf(byTag(unverified, 'caption')[0]), /Still unverified/);
  assert.deepEqual(byTag(byTag(unverified, 'tbody')[0], 'td').map((td) => textOf(td)), ['M1, before a synthetic lock', 'M1']);
});

test('the build identity, the licence and the link to the Assumptions Registry', () => {
  const tree = view({ kind: 'ready', value: future });
  const text = textOf(tree);
  assert.match(text, /Application version0\.0\.0-golden/);
  assert.match(text, /LicenceApache-2\.0/);
  const links = byTag(tree, 'a');
  assert.deepEqual(
    links.map((a) => [textOf(a), a.attrs.href]),
    [['Assumptions Registry', '#/assumptions']],
  );
  // Without the status the identity is said to be unread, never invented.
  assert.match(textOf(view({ kind: 'ready', value: future }, { kind: 'loading' })), /once the session status has been read/);
  // Pins come from the report, absent ones named as absent.
  assert.match(text, /Engine versionnone yet/);
  assert.match(text, /Binary digestnone yet/);
  assert.match(text, /Generated on2001-02-03/);
  const undated = structuredClone(future);
  delete undated.report.generatedOn;
  assert.match(textOf(view({ kind: 'ready', value: undated })), /no date: the generator reads no clock/);
});

/**
 * The first cell of every data row of the first Markdown table under the heading
 * that starts with `prefix`, with emphasis and code marks stripped and a trailing
 * parenthetical dropped (`**Decline mode** (…)` reads as `Decline mode`). The
 * same rule the xtask drift test applies to docs/threat-model.md.
 */
function firstCellsOf(markdown, prefix) {
  const lines = markdown.split('\n');
  const start = lines.findIndex((line) => line.startsWith(prefix));
  assert.notEqual(start, -1, `a heading starts with ${prefix}`);
  const cells = [];
  let inTable = false;
  for (const line of lines.slice(start + 1)) {
    if (/^#{1,3} /.test(line)) {
      break;
    }
    if (!line.startsWith('|')) {
      if (inTable) {
        break;
      }
      continue;
    }
    const first = line.slice(1).split('|')[0].trim();
    if (!inTable) {
      inTable = true;
      continue;
    }
    if (/^:?-+:?$/.test(first)) {
      continue;
    }
    cells.push(first.replaceAll('**', '').replaceAll('`', '').split(' (')[0].trim());
  }
  return cells;
}

test('the out-of-scope statement is SECURITY.md section 2.3, read from the document, row for row and in order', () => {
  const security = readFileSync(join(import.meta.dirname, '..', '..', 'docs', 'SECURITY.md'), 'utf8');
  const source = firstCellsOf(security, '### 2.3');
  assert.ok(source.length >= 10, `section 2.3 has its rows: ${String(source.length)}`);
  assert.deepEqual(
    OUT_OF_SCOPE.map(([threat]) => threat.split(' (')[0]),
    source,
    'OUT_OF_SCOPE in AboutView.tsx must carry every row of SECURITY.md section 2.3, in its order, with its first cell as the label',
  );
  // The extractor proves itself on a table it knows.
  assert.deepEqual(firstCellsOf('# t\n\n### 9.9 X\n\n| A | B |\n|---|---|\n| **one** (aside) | x |\n| `two` | y |\n\n### 9.10\n\n| C |\n|---|\n| three |\n', '### 9.9'), ['one', 'two']);

  const tree = view({ kind: 'ready', value: future });
  const list = byTag(tree, 'dl').at(-1);
  const terms = byTag(list, 'dt').map((dt) => textOf(dt));
  assert.deepEqual(
    terms,
    OUT_OF_SCOPE.map(([threat]) => threat),
  );
  assert.equal(byTag(list, 'dd').length, terms.length);
  assert.ok(byTag(list, 'dd').every((dd) => textOf(dd) !== ''));
  assert.match(textOf(tree), /SECURITY\.md, section 2/);
});
