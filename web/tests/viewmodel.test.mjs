// View-models: DTO -> display strings. They format and look up; they never compute.

import assert from 'node:assert/strict';
import { test } from 'node:test';

import { paramRefVm, worksheetVm } from '../src/viewmodel/lines.ts';
import { registryVm, shortVintageId } from '../src/viewmodel/registry.ts';
import { aboutVm, badgeFor, bannerSentences, STATUS_READING, STATUS_UNREAD, unverifiedNotice, vintageStatusSentence } from '../src/viewmodel/status.ts';
import * as golden from './support/golden.mjs';

const VERIFICATIONS = ['pending-hand-verification', 'primary-source-confirmed', 'hand-worked-reviewed', 'unstated'];

/** "verified" as a word of its own, not inside "unverified" or "verification". */
const claimsVerified = (text) => /(?<!un)\bverified\b/.test(text.replaceAll('treat as unverified', ''));

test('worksheet: seventeen lines in the server order, the result marked, amounts formatted', () => {
  const body = golden.rateSchedule();
  const vm = worksheetVm(body);
  assert.equal(vm.lines.length, 17);
  assert.deepEqual(vm.lines.map((l) => l.id), body.lines.map((l) => l.id));
  assert.deepEqual(vm.lines.map((l) => l.no), body.lines.map((_, i) => i + 1));
  assert.equal(vm.tax, '$11,504.00');
  assert.equal(vm.taxableIncome, '$100,000.00');
  assert.equal(vm.year, '2026');
  assert.equal(vm.filingStatus, 'Married filing jointly');
  assert.equal(vm.caption, 'Rate schedule worksheet, 2026, Married filing jointly');
  assert.deepEqual(vm.lines.filter((l) => l.isResult).map((l) => l.id), [body.rootLineId]);
  assert.equal(vm.lines.at(-1).amount, '$11,504.00');
  assert.equal(vm.announcement, 'Rate schedule calculated. Tax $11,504.00. 17 lines.');
});

test('worksheet: the headline is the server tax, not a sum of lines', () => {
  const body = { ...golden.rateSchedule(), tax: 12345 };
  assert.equal(worksheetVm(body).tax, '$123.45');
  assert.equal(worksheetVm(body).lines.at(-1).amount, '$11,504.00');
});

test('worksheet: inputs resolve to line numbers and labels; an unresolvable id is shown, never dropped', () => {
  const vm = worksheetVm(golden.rateSchedule());
  assert.deepEqual(vm.lines[0].from, []);
  assert.deepEqual(vm.lines[1].from, ['Line 1 - Taxable income']);
  assert.deepEqual(vm.lines[2].from, ['Line 2 - Taxable income taxed at 10%']);
  const body = golden.rateSchedule();
  body.lines[1].inputs = ['sched.missing'];
  assert.deepEqual(worksheetVm(body).lines[1].from, ['sched.missing (not in this worksheet)']);
});

test('parameter references carry the id in their text; an unaddressable id gets no href', () => {
  assert.deepEqual(paramRefVm({ paramId: 'irs.ordinary_brackets', year: 2026, breakdownKey: 'mfj', element: 'top_of_10' }), {
    text: 'irs.ordinary_brackets - 2026 - mfj - top_of_10',
    href: '#/assumptions/irs.ordinary_brackets',
  });
  assert.deepEqual(paramRefVm({ paramId: 'irs.ordinary_brackets', year: 2026, element: 'rates.0' }).text, 'irs.ordinary_brackets - 2026 - rates.0');
  for (const id of ['has space', 'a/b', 'x'.repeat(81)]) {
    const vm = paramRefVm({ paramId: id });
    assert.equal(vm.href, null);
    assert.equal(vm.text, id);
  }
  const refs = worksheetVm(golden.rateSchedule()).lines.flatMap((l) => l.params);
  assert.ok(refs.length >= 14);
  assert.ok(refs.every((ref) => ref.href === '#/assumptions/irs.ordinary_brackets'));
});

test('worksheet: the unverified notice names the vintage and is withheld only for verified and locked', () => {
  const body = golden.rateSchedule();
  const notice = worksheetVm(body).notice;
  assert.equal(notice, `Computed from parameter vintage ${body.vintage.contentId}, which is unlocked and pending human verification. Not for decisions.`);
  assert.ok(!claimsVerified(notice));
  const settled = { ...body, verified: true, vintage: { ...body.vintage, verified: true, lockedId: 'federal-2026@locked' } };
  assert.equal(worksheetVm(settled).notice, null);
  // Fail closed: each of these keeps the notice.
  for (const partial of [
    { ...settled, verified: false },
    { ...settled, verified: 'true' },
    { ...settled, vintage: { ...settled.vintage, verified: false } },
    { ...settled, vintage: { ...settled.vintage, lockedId: undefined } },
  ]) {
    assert.ok(worksheetVm(partial).notice !== null);
  }
  assert.ok(!claimsVerified(worksheetVm({ ...settled, verified: false }).notice));
});

test('verification badges: every wire value, and anything else fails closed', () => {
  assert.deepEqual(VERIFICATIONS.map((v) => badgeFor(v).text), [
    'Pending hand verification - not for decisions',
    'Confirmed against the primary source',
    'Hand-worked and reviewed',
    'Verification not stated - treat as unverified',
  ]);
  assert.deepEqual(VERIFICATIONS.map((v) => badgeFor(v).tone), ['attention', 'confirmed', 'confirmed', 'attention']);
  for (const unknown of ['verified', '', 'constructor', 'PENDING-HAND-VERIFICATION', undefined, null, true, 1]) {
    assert.deepEqual(badgeFor(unknown), { text: 'Unknown status - treat as unverified', tone: 'attention', rank: 2 });
  }
  // Pending leads a sort.
  assert.ok(badgeFor('pending-hand-verification').rank < badgeFor('primary-source-confirmed').rank);
});

test('honesty: the golden status says unlocked and pending human verification, and never "verified"', () => {
  const sentences = bannerSentences({ kind: 'ready', value: golden.sessionStatus() });
  assert.deepEqual(sentences, ['Parameter vintage federal-2026 is unlocked and pending human verification. Figures computed from it are not for decisions.']);
  assert.ok(!claimsVerified(sentences.join(' ')));
});

test('honesty: "verified" and "locked" are separate facts, each said only when true', () => {
  const v = (verified, lockedId) => vintageStatusSentence({ name: 'n', verified, lockedId });
  assert.equal(v(true, 'n@1'), 'Parameter vintage n is locked and verified.');
  assert.equal(v(true, undefined), 'Parameter vintage n is unlocked and verified. Figures computed from it are not for decisions.');
  assert.equal(v(false, 'n@1'), 'Parameter vintage n is locked and pending human verification. Figures computed from it are not for decisions.');
  for (const notTrue of [false, undefined, null, 'true', 1]) {
    assert.ok(!claimsVerified(v(notTrue, 'n@1')), String(notTrue));
  }
  for (const notLocked of [undefined, null, '']) {
    assert.match(v(true, notLocked), /is unlocked and/);
  }
  assert.match(unverifiedNotice('n@1', { name: 'n', verified: false }), /unlocked and pending human verification\. Not for decisions\./);
});

test('the banner never disappears on an error, and is empty only when everything is settled', () => {
  assert.deepEqual(bannerSentences({ kind: 'failed', failure: { kind: 'unreachable' } }), [STATUS_UNREAD]);
  assert.deepEqual(bannerSentences({ kind: 'idle' }), [STATUS_READING]);
  assert.deepEqual(bannerSentences({ kind: 'loading' }), [STATUS_READING]);
  const status = golden.sessionStatus();
  assert.deepEqual(bannerSentences({ kind: 'ready', value: { ...status, vintages: [] } }), [STATUS_UNREAD]);
  const settled = { ...status, vintages: [{ ...status.vintages[0], verified: true, lockedId: 'federal-2026@x' }] };
  assert.deepEqual(bannerSentences({ kind: 'ready', value: settled }), []);
  assert.match(STATUS_UNREAD, /treat every figure as unverified/);
});

test('about: build and vintage identity from session/status', () => {
  const entries = Object.fromEntries(aboutVm(golden.sessionStatus()).entries);
  assert.equal(entries['API version'], 'v1');
  assert.match(entries['Parameter vintage federal-2026'], /^federal-2026@[0-9a-f]{64} \(unlocked\)$/);
  assert.match(entries['Certificate trust'], /Not installed/);
});

test('registry: every table and series, with every summary field', () => {
  const body = golden.assumptions();
  const vm = registryVm(body);
  assert.equal(vm.vintages.length, 1);
  const [vintage] = vm.vintages;
  assert.deepEqual(vintage.tables.map((t) => t.id), ['irs.ordinary_brackets', 'irs.std_deduction']);
  assert.deepEqual(vm.ids, ['irs.ordinary_brackets', 'irs.std_deduction', 'bls.cpi.chained.suur0000sa0']);
  assert.deepEqual(vm.publishedYears, ['2025', '2026']);
  assert.match(vintage.statusLine, /unlocked and pending human verification/);
  assert.equal(vintage.settled, false);

  const [brackets, deduction] = vintage.tables;
  assert.equal(brackets.href, '#/assumptions/irs.ordinary_brackets');
  assert.equal(brackets.period, 'year');
  assert.equal(brackets.asOf, '2025-10-09');
  assert.match(brackets.vintageShort, /^federal-2026@[0-9a-f]{12}…$/);
  assert.equal(brackets.lockText, 'Unlocked');
  assert.equal(brackets.badge.text, 'Pending hand verification - not for decisions');
  assert.equal(brackets.projection, 'index; cpi.chained; cpi.chained.aug12m; lag 1; base year by component');
  assert.equal(deduction.projection, 'index; cpi.chained; cpi.chained.aug12m; lag 1; base year 2024');
  assert.equal(brackets.rounding, 'increment varies by key; down; the increase over the statutory base amount');
  assert.equal(deduction.rounding, '$50.00; down; the increase over the statutory base amount');
  // Counts are lengths.
  assert.equal(brackets.sourceCount, String(body.vintages[0].tables[0].sources.length));
  assert.equal(brackets.openItemCount, String(body.vintages[0].tables[0].openItems.length));
  assert.equal(brackets.sortKeys.openItems, body.vintages[0].tables[0].openItems.length);

  const [series] = vintage.series;
  assert.equal(series.id, 'bls.cpi.chained.suur0000sa0');
  assert.equal(series.name, 'cpi.chained.aug12m');
  assert.equal(series.publisherId, 'SUUR0000SA0');
  const years = body.vintages[0].indexSeries[0].years;
  assert.equal(series.yearsCovered, `${years[0]} to ${years.at(-1)}`);
});

test('registry detail: values, rates, base values, sources and open items, all of them', () => {
  const body = golden.assumptions();
  const table = body.vintages[0].tables[0];
  const detail = registryVm(body).vintages[0].details[0];
  assert.equal(detail.headingId, 'detail-irs.ordinary_brackets');
  const identity = Object.fromEntries(detail.identity);
  assert.equal(identity['Vintage'], table.vintageId);
  assert.equal(identity['Period'], 'year');
  assert.equal(identity['Lock'], 'Unlocked');

  const mfj = detail.grids.find((g) => g.caption === 'Published values of irs.ordinary_brackets for mfj, by tax year');
  assert.deepEqual(mfj.columns, table.components);
  assert.deepEqual(mfj.rows.map((r) => r.header), ['2025', '2026']);
  assert.equal(mfj.rows[1].cells[0], '$24,800.00');
  const rates = detail.grids.find((g) => g.caption.startsWith('Rates of'));
  assert.deepEqual(rates.rows[1].cells, ['10%', '12%', '22%', '24%', '32%', '35%', '37%']);
  assert.deepEqual(rates.columns, ['Rate 1', 'Rate 2', 'Rate 3', 'Rate 4', 'Rate 5', 'Rate 6', 'Rate 7']);
  assert.ok(detail.grids.some((g) => g.caption.startsWith('Statutory base values')));
  assert.ok(detail.grids.some((g) => g.caption.startsWith('Statutory base year')));
  assert.equal(detail.grids.length, Object.keys(table.values).length + 3);

  assert.equal(detail.sources.length, table.sources.length);
  assert.equal(detail.sources[0].url, table.sources[0].url);
  assert.equal(detail.sources[0].sha256, table.sources[0].sha256);
  assert.ok(detail.sources[0].entries.some(([label, value]) => label.startsWith('Archived copy') && value === table.sources[0].archive));
  assert.deepEqual(detail.openItems, table.openItems);
  assert.ok(Object.fromEntries(detail.rounding)['Increment for mfs'] === '$25.00');

  const single = registryVm(body).vintages[0].details[1];
  assert.deepEqual(single.grids[0].columns, ['Amount']);
  assert.equal(Object.fromEntries(single.rounding)['Increment'], '$50.00');
  assert.equal(Object.fromEntries(single.projection)['Base year'], '2024');
});

test('short vintage id keeps the name and twelve characters of the hash', () => {
  assert.equal(shortVintageId('federal-2026@0123456789abcdef'), 'federal-2026@0123456789ab…');
  assert.equal(shortVintageId('federal-2026@0123'), 'federal-2026@0123');
  assert.equal(shortVintageId('no-hash'), 'no-hash');
});
