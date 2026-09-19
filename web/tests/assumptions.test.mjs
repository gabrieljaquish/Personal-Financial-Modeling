// The Assumptions Registry: every parameter with its source, as-of date, vintage,
// projection rule and rounding rule - and its verification status shown honestly.

import assert from 'node:assert/strict';
import { test } from 'node:test';
import { createElement } from 'react';

import { SortableTable } from '../src/components/SortableTable.tsx';
import { VerificationBadge } from '../src/components/VerificationBadge.tsx';
import { RETRY_FOCUS_ID, retryWithFocus } from '../src/screens/assumptions/AssumptionsScreen.tsx';
import { AssumptionsView } from '../src/screens/assumptions/AssumptionsView.tsx';
import { registryVm } from '../src/viewmodel/registry.ts';
import { badgeFor } from '../src/viewmodel/status.ts';
import { assertAccessible } from './support/a11y.mjs';
import { fakeApiEnv, fakeAppEnv } from './support/fakes.mjs';
import * as golden from './support/golden.mjs';
import { byAttr, byClass, byId, byTag, focusables, textOf } from './support/markup.mjs';
import { render } from './support/render.mjs';

const body = golden.assumptions();
const ready = { kind: 'ready', value: registryVm(body) };
const DETAIL_IDS = ready.value.ids.map((id) => `detail-${id}`);
const TARGETS = ['screen-heading', ...DETAIL_IDS];

const view = (registry, routedId = null) => render(createElement(AssumptionsView, { registry, routedId, onRetry() {}, onSorted() {} })).tree;
const claimsVerified = (text) => /(?<!un)\bverified\b/.test(text.replaceAll('treat as unverified', ''));

test('every state passes the checklist', () => {
  for (const registry of [{ kind: 'idle' }, { kind: 'loading' }, { kind: 'failed', failure: { kind: 'unreachable' } }, { kind: 'failed', failure: { kind: 'defect', code: 'malformed_response' } }, ready]) {
    assertAccessible(view(registry), { focusTargets: TARGETS });
  }
  assertAccessible(view(ready, 'irs.std_deduction'), { focusTargets: TARGETS });
});

test('loading is busy; a failure is an alert with Try again only where it can help', () => {
  assert.equal(byTag(view({ kind: 'loading' }), 'section')[0].attrs['aria-busy'], 'true');
  const failed = view({ kind: 'failed', failure: { kind: 'unreachable' } });
  assert.match(textOf(byAttr(failed, 'role', 'alert')[0]), /is not answering/);
  assert.deepEqual(byTag(failed, 'button').map((b) => textOf(b)), ['Try again']);
  assert.equal(byTag(view({ kind: 'failed', failure: { kind: 'defect', code: 'x' } }), 'button').length, 0);
});

test('summary: both shipped tables and the index series, with every column', () => {
  const tree = view(ready);
  const [tables, series] = byTag(tree, 'table');
  assert.equal(textOf(byTag(tables, 'caption')[0]), 'Parameter tables of vintage federal-2026');
  assert.deepEqual(
    byTag(byTag(tables, 'thead')[0], 'th').map((th) => textOf(th, { includeHidden: false }).replace(/, (not sorted|sorted \w+)$/, '')),
    ['Parameter id', 'Period', 'As of', 'Vintage', 'Verification', 'Projection', 'Rounding', 'Sources', 'Open items'],
  );
  const rows = byTag(byTag(tables, 'tbody')[0], 'tr');
  assert.deepEqual(rows.map((row) => textOf(byTag(row, 'th')[0])), ['irs.ordinary_brackets', 'irs.std_deduction']);
  const table0 = body.vintages[0].tables[0];
  const cells = byTag(rows[0], 'td').map((td) => textOf(td, { includeHidden: false }));
  assert.deepEqual(cells, [
    'year',
    table0.asOf,
    `${table0.vintageId.slice(0, 'federal-2026@'.length + 12)}… (Unlocked)`,
    'Pending hand verification - not for decisions',
    'index; cpi.chained; cpi.chained.aug12m; lag 1; base year by component',
    'increment varies by key; down; the increase over the statutory base amount',
    String(table0.sources.length),
    String(table0.openItems.length),
  ]);
  // The row header is the link to the entry.
  assert.equal(byTag(rows[0], 'a')[0].attrs.href, '#/assumptions/irs.ordinary_brackets');

  assert.equal(textOf(byTag(series, 'caption')[0]), 'Index series of vintage federal-2026');
  const s = body.vintages[0].indexSeries[0];
  const seriesRow = byTag(byTag(series, 'tbody')[0], 'tr')[0];
  assert.deepEqual(
    [textOf(byTag(seriesRow, 'th')[0]), ...byTag(seriesRow, 'td').map((td) => textOf(td, { includeHidden: false }))],
    [s.id, s.indexSeries, s.seriesId, s.asOf, `${s.years[0]} to ${s.years.at(-1)}`, 'Pending hand verification - not for decisions', String(s.sources.length), String(s.openItems.length)],
  );
});

test('honesty: pending everywhere, unlocked, and the word "verified" appears nowhere for the shipped vintage', () => {
  const tree = view(ready);
  const text = textOf(tree);
  assert.match(text, /Parameter vintage federal-2026 is unlocked and pending human verification\. Figures computed from it are not for decisions\./);
  assert.ok(!claimsVerified(text));
  assert.ok(!/\bLocked as\b/.test(text));
  const badges = [...byClass(tree, 'badgeAttention'), ...byClass(tree, 'badgeConfirmed')];
  // Two per entry: the summary row and the detail.
  assert.equal(badges.length, 6);
  assert.ok(badges.every((b) => textOf(b, { includeHidden: false }) === 'Pending hand verification - not for decisions'));
});

test('honesty: a synthetic all-true body renders the positive text; every enum value and an unknown one are covered', () => {
  const vintage = body.vintages[0];
  const settled = {
    vintages: [
      {
        ...vintage,
        verified: true,
        lockedId: 'federal-2026@locked',
        tables: vintage.tables.map((t, i) => ({ ...t, verification: i === 0 ? 'primary-source-confirmed' : 'hand-worked-reviewed' })),
        indexSeries: vintage.indexSeries.map((s) => ({ ...s, verification: 'unstated' })),
      },
    ],
  };
  const tree = view({ kind: 'ready', value: registryVm(settled) });
  const text = textOf(tree);
  assert.match(text, /Parameter vintage federal-2026 is locked and verified\./);
  assert.match(text, /Locked as federal-2026@locked/);
  assert.match(text, /Confirmed against the primary source/);
  assert.match(text, /Hand-worked and reviewed/);
  assert.match(text, /Verification not stated - treat as unverified/);

  for (const [wire, words, cls] of [
    ['pending-hand-verification', 'Pending hand verification - not for decisions', 'badgeAttention'],
    ['primary-source-confirmed', 'Confirmed against the primary source', 'badgeConfirmed'],
    ['hand-worked-reviewed', 'Hand-worked and reviewed', 'badgeConfirmed'],
    ['unstated', 'Verification not stated - treat as unverified', 'badgeAttention'],
    ['something-new', 'Unknown status - treat as unverified', 'badgeAttention'],
  ]) {
    const badge = render(createElement(VerificationBadge, { badge: badgeFor(wire) })).tree;
    const [span] = byClass(badge, cls);
    // A9: the words carry the meaning; the glyph is decoration.
    assert.equal(textOf(span, { includeHidden: false }), words);
    assert.equal(byAttr(span, 'aria-hidden', 'true').length, 1);
  }
});

test('detail: every DTO field of an entry is rendered - sources, archive metadata, projection, rounding, values, open items', () => {
  const tree = view(ready);
  const table0 = body.vintages[0].tables[0];
  const detail = byId(tree, 'detail-irs.ordinary_brackets').parent;
  const text = textOf(detail);
  assert.equal(byId(tree, 'detail-irs.ordinary_brackets').tag, 'h4');
  for (const source of table0.sources) {
    for (const value of [source.title, source.publisher, source.url, source.retrieved, source.archive, source.sha256, source.locator, source.asOf]) {
      if (value !== undefined) {
        assert.ok(text.includes(value), `source field ${value}`);
      }
    }
  }
  for (const item of table0.openItems) {
    assert.ok(text.includes(item));
  }
  for (const value of [table0.vintageId, table0.asOf, 'Unlocked', 'index', 'cpi.chained', 'cpi.chained.aug12m', 'By component (table below)', '$24,800.00', '37%', ...table0.components]) {
    assert.ok(text.includes(value), value);
  }
  assert.match(text, /Questions a person still has to answer about this table/);
  // Source addresses are shown as text: the application opens nothing and links nowhere.
  assert.equal(byTag(detail, 'a').length, 0);
  assert.ok(byTag(detail, 'code').some((c) => textOf(c) === table0.sources[0].url));
  // The head-of-household residual the table file documents is shown in full.
  const deduction = textOf(byId(tree, 'detail-irs.std_deduction').parent);
  for (const item of body.vintages[0].tables[1].openItems) {
    assert.ok(deduction.includes(item));
  }
  // The series has a detail too, with its sources.
  const series = textOf(byId(tree, 'detail-bls.cpi.chained.suur0000sa0').parent);
  assert.ok(series.includes(body.vintages[0].indexSeries[0].sources[0].sha256));
});

test('narrow viewports: what a hidden summary column says is also in that entry\'s detail', () => {
  const tree = view(ready);
  for (const table of byTag(tree, 'table').slice(0, 2)) {
    const head = byTag(byTag(table, 'thead')[0], 'th');
    const hidden = head.map((th, i) => ((th.attrs.class ?? '').split(' ').includes('secondaryCol') ? i : -1)).filter((i) => i !== -1);
    assert.ok(hidden.length >= 2);
    for (const row of byTag(byTag(table, 'tbody')[0], 'tr')) {
      const cells = row.children.filter((c) => c.tag !== undefined);
      const id = textOf(cells[0]);
      const detail = textOf(byId(tree, `detail-${id}`).parent);
      for (const i of hidden) {
        assert.ok((cells[i].attrs.class ?? '').split(' ').includes('secondaryCol'), 'header and body cells share the class');
        const parts = textOf(cells[i]).replace('… (Unlocked)', '').split('; ');
        for (const part of parts) {
          const needle = part === 'increment varies by key' ? 'Increment for' : part === 'base year by component' ? 'By component' : part.replace(/^lag |^base year /, '');
          assert.ok(detail.includes(needle), `${id}: "${part}" is in the detail`);
        }
      }
    }
  }
  assert.equal(byClass(tree, 'narrowNote').length, 1);
  assert.ok((byTag(tree, 'table')[0].parent.attrs.class ?? '').includes('scrollRegion'));
});

test('sorting: default id ascending; aria-sort in each state; buttons name the column and direction in words', () => {
  const vintage = ready.value.vintages[0];
  const columns = [
    { key: 'id', header: 'Parameter id', sortable: true, rowHeader: true, cell: (row) => row.id },
    { key: 'asOf', header: 'As of', sortable: true, cell: (row) => row.asOf },
    { key: 'openItems', header: 'Open items', sortable: true, cell: (row) => row.openItemCount },
    { key: 'period', header: 'Period', cell: (row) => row.period },
  ];
  const table = (initialSort) =>
    render(createElement(SortableTable, { caption: 'T', captionId: 'cap', columns, rows: vintage.tables, initialSort, secondaryClassName: 's', onSorted() {} })).tree;

  const asc = table({ key: 'id', direction: 'ascending' });
  assertAccessible(asc);
  assert.deepEqual(byTag(byTag(asc, 'thead')[0], 'th').map((th) => th.attrs['aria-sort']), ['ascending', 'none', 'none', undefined]);
  assert.deepEqual(byTag(asc, 'button').map((b) => textOf(b, { includeHidden: false })), ['Parameter id, sorted ascending', 'As of, not sorted', 'Open items, not sorted']);
  assert.deepEqual(byTag(byTag(asc, 'tbody')[0], 'th').map((th) => textOf(th)), ['irs.ordinary_brackets', 'irs.std_deduction']);

  const desc = table({ key: 'id', direction: 'descending' });
  assertAccessible(desc);
  assert.equal(byTag(byTag(desc, 'thead')[0], 'th')[0].attrs['aria-sort'], 'descending');
  assert.deepEqual(byTag(byTag(desc, 'tbody')[0], 'th').map((th) => textOf(th)), ['irs.std_deduction', 'irs.ordinary_brackets']);

  const byCount = table({ key: 'openItems', direction: 'descending' });
  assertAccessible(byCount);
  assert.equal(textOf(byTag(byCount, 'button')[2], { includeHidden: false }), 'Open items, sorted descending');
  const counts = byTag(byTag(byCount, 'tbody')[0], 'tr').map((row) => Number(textOf(byTag(row, 'td')[1])));
  assert.deepEqual(counts, counts.toSorted((a, b) => b - a));
});

test('#/assumptions/<id> targets the right detail; an unknown id renders Not found', () => {
  const routed = view(ready, 'irs.std_deduction');
  const heading = byId(routed, 'detail-irs.std_deduction');
  assert.equal(heading.attrs.tabindex, '-1');
  assert.equal(textOf(heading), 'irs.std_deduction - Parameter table - Linked entry');
  assert.equal(textOf(byId(view(ready, 'bls.cpi.chained.suur0000sa0'), 'detail-bls.cpi.chained.suur0000sa0')), 'bls.cpi.chained.suur0000sa0 - Index series - Linked entry');

  const unknown = view(ready, 'no.such.table');
  assert.equal(textOf(byId(unknown, 'screen-heading')), 'Nothing at this address');
  assert.equal(byTag(unknown, 'table').length, 0);
  // Until the registry has loaded, an id cannot be judged: the screen loads, it does not say Not found.
  assert.equal(textOf(byId(view({ kind: 'loading' }, 'no.such.table'), 'screen-heading')), 'Assumptions Registry');
});

test('DOM order of focusable elements: each table region, its sort buttons, its row links, then the value tables', () => {
  const order = focusables(view(ready));
  assert.deepEqual(order.slice(0, 7), [
    'div:vintage-0-tables',
    'button:Parameter id, sorted ascending',
    'button:As of, not sorted',
    'button:Verification, not sorted',
    'button:Open items, not sorted',
    'a:irs.ordinary_brackets',
    'a:irs.std_deduction',
  ]);
  assert.equal(order[7], 'div:vintage-0-series');
  assert.ok(order.slice(13).every((entry) => entry.startsWith('div:detail-')), 'after the summaries, only the scrollable value tables');
});

test('an empty registry says so', () => {
  assert.match(textOf(view({ kind: 'ready', value: registryVm({ vintages: [] }) })), /carries no parameter vintage/);
});

// WCAG 2.4.3: the registry's "Try again" is unmounted by the loading state it starts.
test('Try again: focus moves to the screen heading before the button is unmounted', () => {
  const failed = view({ kind: 'failed', failure: { kind: 'unreachable' } });
  const loading = view({ kind: 'loading' });
  assert.deepEqual(focusables(failed), ['button:Try again']);
  assert.deepEqual(focusables(loading), [], 'the control that held focus is gone');
  // The target is in both renders, script-only, and not a live region nor around one.
  for (const tree of [failed, loading, view(ready)]) {
    const heading = byId(tree, RETRY_FOCUS_ID);
    assert.equal(heading.attrs.tabindex, '-1');
    assert.ok(!('role' in heading.attrs));
    assert.equal(byAttr(heading, 'role', 'alert').length, 0);
  }
  const app = fakeAppEnv(fakeApiEnv(() => { throw new TypeError('no network'); }).env);
  const order = [];
  const env = { focusById: (id) => (order.push(`focus:${id}`), app.env.focusById(id)) };
  retryWithFocus(env, () => order.push('retry'))();
  assert.deepEqual(order, [`focus:${RETRY_FOCUS_ID}`, 'retry'], 'focus first, while the button still exists; then one retry');
});

test('a deep link identifies its entry in the markup: the route is never byte-identical to the index', () => {
  const index = render(createElement(AssumptionsView, { registry: ready, routedId: null, onRetry() {}, onSorted() {} }));
  const routed = render(createElement(AssumptionsView, { registry: ready, routedId: 'irs.ordinary_brackets', onRetry() {}, onSorted() {} }));
  assert.notEqual(routed.markup, index.markup);

  // The index marks nothing.
  assert.deepEqual(byAttr(index.tree, 'aria-current', 'true'), []);
  assert.ok(!textOf(index.tree).includes('Linked entry'));

  // The routed render marks exactly one article: programmatically, in words, and with a border-style class.
  const current = byAttr(routed.tree, 'aria-current', 'true');
  assert.equal(current.length, 1);
  const [article] = current;
  assert.equal(article.tag, 'article');
  assert.equal(article.attrs['aria-labelledby'], 'detail-irs.ordinary_brackets');
  assert.ok(article.attrs.class.split(' ').includes('detailRouted'), 'a non-colour visual treatment');
  // The words are in the heading that a navigation focuses, so they are what is spoken.
  const heading = byId(routed.tree, 'detail-irs.ordinary_brackets');
  assert.equal(heading.attrs.tabindex, '-1');
  assert.match(textOf(heading, { includeHidden: false }), / - Linked entry$/);
  assert.equal(textOf(routed.tree).split('Linked entry').length - 1, 1, 'one entry is marked, not every entry');
  assertAccessible(routed.tree, { focusTargets: TARGETS });
});
