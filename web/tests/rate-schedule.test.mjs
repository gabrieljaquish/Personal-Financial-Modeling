// The rate-schedule screen: validation, every state of the reducer rendered and
// run through the accessibility checklist, the explained worksheet from the golden
// body, and the export driven reducer -> effect -> reducer with a fake browser.

import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { test } from 'node:test';
import { createElement } from 'react';

import { politeOwner, politeReducer, politeText, visiblePolite } from '../src/announce.ts';
import { createClient } from '../src/api/client.ts';
import { EXPORT_WARNING } from '../src/components/ExportActions.tsx';
import { attachmentFilename, fallbackFilename, performCalculation, performExport } from '../src/screens/rate-schedule/effects.ts';
import { REFUSAL_GUIDANCE, RateScheduleView } from '../src/screens/rate-schedule/RateScheduleView.tsx';
import { ERROR_SUMMARY_ID, INITIAL_RS_STATE, RESULT_HEADING_ID, rsAnnouncements, rsReducer, validate } from '../src/screens/rate-schedule/state.ts';
import { assertAccessible, assertOneChannel } from './support/a11y.mjs';
import { errorResponse, fakeApiEnv, fakeAppEnv, fakeResponse } from './support/fakes.mjs';
import * as golden from './support/golden.mjs';
import { ancestors, byAttr, byClass, byId, byTag, elements, focusables, textOf } from './support/markup.mjs';
import { render } from './support/render.mjs';

const handlers = { onChange() {}, onSubmit() {}, onRetry() {}, onJump() {}, onExport() {}, onPrint() {} };
const TARGETS = ['screen-heading', ERROR_SUMMARY_ID, RESULT_HEADING_ID];
// SYNTHETIC inputs: the first grid point of docs/PLAN.md §4.1.
const FIELDS = { status: 'mfj', year: '2026', income: '100,000' };
const INPUTS = { year: 2026, filingStatus: 'mfj', taxableIncome: 10000000 };

const view = (state, publishedYears = ['2025', '2026']) => render(createElement(RateScheduleView, { state, publishedYears, handlers })).tree;
const run = (events, from = INITIAL_RS_STATE) => events.reduce(rsReducer, from);
const typed = (fields) => Object.entries(fields).map(([field, value]) => ({ type: 'change', field, value }));
const okResult = (request = 1) => ({ type: 'result', request, outcome: { ok: true, value: golden.rateSchedule() } });
const failedResult = (failure, request = 1) => ({ type: 'result', request, outcome: { ok: false, failure } });

const STATES = {
  idle: INITIAL_RS_STATE,
  invalid: run([{ type: 'submit' }]),
  'partly invalid': run([...typed({ ...FIELDS, income: '100.50' }), { type: 'submit' }]),
  loading: run([...typed(FIELDS), { type: 'submit' }]),
  ready: run([...typed(FIELDS), { type: 'submit' }, okResult()]),
  'refused by the server': run([...typed(FIELDS), { type: 'submit' }, failedResult({ kind: 'input-refused' })]),
  unreachable: run([...typed(FIELDS), { type: 'submit' }, failedResult({ kind: 'unreachable' })]),
  defect: run([...typed(FIELDS), { type: 'submit' }, failedResult({ kind: 'defect', code: 'request_invalid' })]),
  exporting: run([...typed(FIELDS), { type: 'submit' }, okResult(), { type: 'export-started', format: 'csv' }]),
  exported: run([...typed(FIELDS), { type: 'submit' }, okResult(), { type: 'export-started', format: 'csv' }, { type: 'export-result', outcome: { ok: true, value: 'f.csv' } }]),
  'export failed': run([
    ...typed(FIELDS),
    { type: 'submit' },
    okResult(),
    { type: 'export-started', format: 'json' },
    { type: 'export-result', outcome: { ok: false, failure: { kind: 'unreachable' } } },
  ]),
};

test('validation forms a request from strings only, or says what to type', () => {
  assert.deepEqual(validate(FIELDS), { inputs: INPUTS });
  assert.deepEqual(validate({ status: 'single', year: ' 2025 ', income: '$0' }), { inputs: { year: 2025, filingStatus: 'single', taxableIncome: 0 } });
  const all = validate({ status: '', year: '', income: '' });
  assert.deepEqual(all.errors.map((e) => e.field), ['status', 'year', 'income']);
  assert.deepEqual(validate({ ...FIELDS, status: 'married' }).errors.map((e) => e.field), ['status']);
  assert.deepEqual(validate({ ...FIELDS, year: '26' }).errors.map((e) => e.field), ['year']);
  for (const income of ['1e5', '100.50', '-1', '1,00', '90071992547410', 'abc']) {
    const { errors } = validate({ ...FIELDS, income });
    assert.deepEqual(errors.map((e) => e.field), ['income'], income);
    // A message says what to type and never echoes what was typed.
    assert.ok(!errors[0].message.includes(income), income);
  }
});

test('every field starts empty: no default status, year or amount', () => {
  const tree = view(INITIAL_RS_STATE);
  assert.equal(byId(tree, 'rs-year').attrs.value, '');
  assert.equal(byId(tree, 'rs-income').attrs.value, '');
  const selected = byTag(byId(tree, 'rs-status'), 'option').filter((o) => 'selected' in o.attrs);
  assert.deepEqual(selected.map((o) => [o.attrs.value, textOf(o)]), [['', 'Choose a filing status']]);
  assert.equal(byTag(byId(tree, 'rs-status'), 'option').length, 6);
});

test('the controls are text inputs with a numeric keyboard, never type=number', () => {
  const tree = view(INITIAL_RS_STATE);
  for (const id of ['rs-year', 'rs-income']) {
    const input = byId(tree, id);
    assert.equal(input.attrs.type, 'text');
    assert.equal(input.attrs.inputMode ?? input.attrs.inputmode, 'numeric');
    assert.equal(input.attrs.autoComplete ?? input.attrs.autocomplete, 'off');
  }
  assert.equal(byId(tree, 'rs-year').attrs.maxLength ?? byId(tree, 'rs-year').attrs.maxlength, '4');
  assert.ok('noValidate' in byTag(tree, 'form')[0].attrs || 'novalidate' in byTag(tree, 'form')[0].attrs);
  assert.ok(!('action' in byTag(tree, 'form')[0].attrs));
});

test('the year hint is a lookup of what the registry publishes, and is omitted until it has loaded', () => {
  assert.equal(textOf(byId(view(INITIAL_RS_STATE), 'rs-year-hint')), 'This build publishes parameters for 2025 and 2026.');
  assert.equal(byId(view(INITIAL_RS_STATE), 'rs-year').attrs['aria-describedby'], 'rs-year-hint');
  const without = view(INITIAL_RS_STATE, []);
  assert.equal(byId(without, 'rs-year-hint'), undefined);
  assert.ok(!('aria-describedby' in byId(without, 'rs-year').attrs));
  assert.match(textOf(byId(without, 'rs-income-hint')), /Example: 100000 \(a synthetic figure\)/);
});

test('every state passes the checklist and keeps one message on one channel', () => {
  for (const [name, state] of Object.entries(STATES)) {
    const tree = view(state);
    assertAccessible(tree, { focusTargets: TARGETS });
    assertOneChannel(tree, rsAnnouncements(state));
    assert.equal(byTag(tree, 'h2').length, 1, name);
  }
});

test('declared announcements, state by state', () => {
  assert.deepEqual(rsAnnouncements(STATES.idle), {});
  assert.deepEqual(rsAnnouncements(STATES.invalid), { focus: ERROR_SUMMARY_ID });
  assert.deepEqual(rsAnnouncements(STATES['refused by the server']), { focus: ERROR_SUMMARY_ID });
  assert.deepEqual(rsAnnouncements(STATES.loading), { polite: 'Calculating…' });
  assert.deepEqual(rsAnnouncements(STATES.ready), { polite: 'Rate schedule calculated. Tax $11,504.00. 17 lines.' });
  assert.deepEqual(Object.keys(rsAnnouncements(STATES.unreachable)), ['alert']);
  assert.deepEqual(Object.keys(rsAnnouncements(STATES.exporting)), ['polite']);
  assert.deepEqual(rsAnnouncements(STATES.exported), { polite: 'The file f.csv was handed to your browser to save.' });
  assert.deepEqual(Object.keys(rsAnnouncements(STATES['export failed'])), ['alert']);
});

test('the polite write is authoritative: a state with no polite text writes the empty string', () => {
  // What the container hands to announce(), state after state.
  const written = (events) => {
    const out = [];
    events.reduce((state, event) => {
      const next = rsReducer(state, event);
      out.push(politeText(rsAnnouncements(next)));
      return next;
    }, INITIAL_RS_STATE);
    return out;
  };
  // loading -> failed: "Calculating…" must not stay above the alert.
  assert.deepEqual(written([...typed(FIELDS), { type: 'submit' }, failedResult({ kind: 'unreachable' })]).slice(-2), ['Calculating…', '']);
  // loading -> refused (422): the summary takes focus; the region is emptied.
  assert.deepEqual(written([...typed(FIELDS), { type: 'submit' }, failedResult({ kind: 'input-refused' })]).slice(-2), ['Calculating…', '']);
  // ready -> validation error: the old result sentence is cleared.
  const afterReady = written([...typed(FIELDS), { type: 'submit' }, okResult(), { type: 'change', field: 'income', value: 'abc' }, { type: 'submit' }]);
  assert.match(afterReady.at(-3), /^Rate schedule calculated/);
  assert.equal(afterReady.at(-1), '');
  // exported -> export failed.
  assert.equal(politeText(rsAnnouncements(STATES['export failed'])), '');
  for (const [name, state] of Object.entries(STATES)) {
    const { polite, alert } = rsAnnouncements(state);
    assert.equal(typeof politeText(rsAnnouncements(state)), 'string', name);
    if (alert !== undefined) {
      assert.equal(polite, undefined, name);
      assert.equal(politeText(rsAnnouncements(state)), '', `${name}: an alert state empties the polite region`);
    }
  }
  // Effects do not run under renderToStaticMarkup, so the wiring is pinned at the source.
  const container = readFileSync(join(import.meta.dirname, '..', 'src', 'screens', 'rate-schedule', 'RateScheduleScreen.tsx'), 'utf8');
  assert.match(container, /const polite = politeText\(rsAnnouncements\(state\)\);/);
  assert.match(container, /useEffect\(\(\) => \{\n(\s*\/\/.*\n)*\s*announce\(polite\);\n\s*\}, \[announce, polite\]\);/);
  assert.ok(!/polite !== undefined/.test(container), 'the write is never conditional');
});

test('a polite message is shown only on the screen that wrote it, and never without a session', () => {
  const sorted = { owner: 'assumptions', text: 'Sorted by Parameter id, descending.' };
  assert.equal(visiblePolite(sorted, 'assumptions', true), sorted.text);
  assert.equal(visiblePolite(sorted, politeOwner('assumption'), true), sorted.text, 'the list and a routed entry are one screen');
  assert.equal(visiblePolite(sorted, politeOwner('rate-schedule'), true), '', 'it does not survive navigation');
  assert.equal(visiblePolite(sorted, politeOwner('not-found'), true), '');
  assert.equal(visiblePolite(sorted, 'assumptions', false), '', 'a lost session empties the region');
  const app = readFileSync(join(import.meta.dirname, '..', 'src', 'App.tsx'), 'utf8');
  assert.match(app, /const polite = visiblePolite\(message, owner, connected\);/);
  assert.match(app, /dispatchPolite\(\{ type: 'write', owner, text \}\)/);
  assert.match(app, /<AnnounceContext value=\{announce\}>/);
});

test('no stale polite message: leaving the registry or the not-found route leaves nothing to re-announce (WCAG 4.1.3)', () => {
  const shown = (message, screen) => visiblePolite(message, politeOwner(screen), true);
  const write = (screen, text) => ({ type: 'write', owner: politeOwner(screen), text });
  const route = (screen) => ({ type: 'route', owner: politeOwner(screen) });
  const SORT = 'Sorted by As of, ascending.';

  // The reported sequence: sort the registry -> a bad hash -> back to the registry.
  // Driven by the shell's route event ALONE, so it holds even if a screen wrote nothing.
  let message = [write('assumptions', SORT)].reduce(politeReducer, { owner: 'assumptions', text: '' });
  assert.equal(shown(message, 'assumptions'), SORT);
  message = politeReducer(message, route('not-found'));
  assert.deepEqual(message, { owner: 'not-found', text: '' }, 'leaving the registry empties the store, not just the view of it');
  message = politeReducer(message, route('assumptions'));
  assert.equal(shown(message, 'assumptions'), '', 'the sort made before leaving is not announced again');

  // The same through the rate schedule, and out of the not-found route.
  message = [write('assumptions', SORT), route('rate-schedule'), route('assumptions')].reduce(politeReducer, message);
  assert.equal(shown(message, 'assumptions'), '');
  message = [write('not-found', 'anything'), route('assumptions'), route('not-found')].reduce(politeReducer, message);
  assert.equal(shown(message, 'not-found'), '', 'the not-found route leaves nothing behind either');

  // The list and a routed entry are one screen: opening an entry keeps the sort message.
  message = [write('assumptions', SORT), route('assumption')].reduce(politeReducer, message);
  assert.equal(shown(message, 'assumption'), SORT);

  // A route event for the owner that has just written (child effects run first) keeps its message.
  message = [route('rate-schedule'), write('rate-schedule', 'Calculating…'), route('rate-schedule')].reduce(politeReducer, message);
  assert.equal(shown(message, 'rate-schedule'), 'Calculating…');

  // Effects do not run under renderToStaticMarkup, so the wiring is pinned at the source:
  // the shell emits the route event, and EVERY screen container clears what it owns on leaving.
  const read = (...parts) => readFileSync(join(import.meta.dirname, '..', 'src', ...parts), 'utf8');
  assert.match(read('App.tsx'), /useEffect\(\(\) => dispatchPolite\(\{ type: 'route', owner \}\), \[owner\]\);/);
  assert.match(read('env.ts'), /useEffect\(\(\) => \{\n\s*announce\(''\);\n\s*return \(\) => announce\(''\);\n\s*\}, \[announce\]\);/);
  assert.match(read('screens', 'assumptions', 'AssumptionsScreen.tsx'), /\n\s*useClearedPolite\(\);/);
  assert.match(read('screens', 'not-found', 'NotFoundScreen.tsx'), /\n\s*useClearedPolite\(\);/);
  assert.match(read('screens', 'rate-schedule', 'RateScheduleScreen.tsx'), /useEffect\(\(\) => \(\) => announce\(''\), \[announce\]\);/);
});

test('idle: the form and an empty-state sentence, no table', () => {
  const tree = view(STATES.idle);
  assert.equal(byTag(tree, 'table').length, 0);
  assert.match(textOf(byClass(tree, 'result')[0]), /No result yet/);
  assert.deepEqual(focusables(tree), ['select:#rs-status', 'input:#rs-year', 'input:#rs-income', 'button:Calculate']);
});

test('invalid: A3 - each field is invalid and described by its hint then its error; the summary is a focus target, not a live region', () => {
  const state = STATES.invalid;
  assert.equal(state.focusSeq, 1, 'the reducer asks for focus on the summary');
  assert.equal(state.phase.kind, 'idle');
  const tree = view(state);
  for (const [id, describedBy] of [['rs-status', 'rs-status-error'], ['rs-year', 'rs-year-hint rs-year-error'], ['rs-income', 'rs-income-hint rs-income-error']]) {
    const field = byId(tree, id);
    assert.equal(field.attrs['aria-invalid'], 'true');
    assert.equal(field.attrs['aria-describedby'], describedBy);
    assert.ok(textOf(byId(tree, `${id}-error`), { includeHidden: false }).length > 10);
  }
  const summary = byId(tree, ERROR_SUMMARY_ID);
  assert.equal(summary.attrs.role, 'group');
  assert.equal(summary.attrs.tabindex, '-1');
  assert.equal(textOf(byId(tree, summary.attrs['aria-labelledby']), { includeHidden: false }), 'There is a problem');
  // Focus on a bare group speaks only "There is a problem, group": the errors are its description.
  const described = summary.attrs['aria-describedby'].split(' ').map((id) => byId(tree, id));
  assert.ok(described.every((e) => e !== undefined && ancestors(e).includes(summary)), 'the description is inside the summary');
  assert.equal(described.map((e) => textOf(e)).join(' '), state.errors.map((e) => e.message).join(''));
  const refused = view(STATES['refused by the server']);
  const refusedSummary = byId(refused, ERROR_SUMMARY_ID);
  const refusedIds = refusedSummary.attrs['aria-describedby'].split(' ');
  assert.equal(refusedIds.length, 1, 'the refusal is one general message: there is no list to describe it');
  assert.equal(textOf(byId(refused, refusedIds[0])), `${STATES['refused by the server'].refusal} ${REFUSAL_GUIDANCE}`);
  assert.ok(!('aria-live' in summary.attrs));
  assert.equal(byAttr(tree, 'role', 'alert').length, 0, 'a failed submit is announced by focus alone');
  // Entries are buttons that focus a field - never #id anchors, which would collide with hash routing.
  assert.equal(byTag(summary, 'a').length, 0);
  assert.deepEqual(byTag(summary, 'button').map((b) => textOf(b)), state.errors.map((e) => e.message));
  assert.deepEqual(focusables(tree).slice(0, 3), state.errors.map((e) => `button:${e.message}`));
  // The summary comes before the form in the DOM.
  const order = elements(tree).filter((e) => e.attrs.id === ERROR_SUMMARY_ID || e.tag === 'form').map((e) => e.tag);
  assert.deepEqual(order, ['div', 'form']);
});

test('partly invalid: only the wrong field is marked', () => {
  const tree = view(STATES['partly invalid']);
  assert.equal(byId(tree, 'rs-income').attrs['aria-invalid'], 'true');
  assert.ok(!('aria-invalid' in byId(tree, 'rs-year').attrs));
  assert.ok(!('aria-invalid' in byId(tree, 'rs-status').attrs));
  assert.equal(byId(tree, 'rs-income').attrs.value, '100.50', 'what was typed stays in the field');
});

test('loading: the result region is busy and the submit is aria-disabled, not disabled; a second submit is ignored', () => {
  const state = STATES.loading;
  assert.deepEqual(state.pending, INPUTS);
  const tree = view(state);
  assert.equal(byClass(tree, 'result')[0].attrs['aria-busy'], 'true');
  const submit = byTag(tree, 'button').find((b) => b.attrs.type === 'submit');
  assert.equal(submit.attrs['aria-disabled'], 'true');
  assert.ok(!('disabled' in submit.attrs));
  assert.equal(rsReducer(state, { type: 'submit' }), state);
});

test('a newer submit supersedes an older answer', () => {
  const second = run([failedResult({ kind: 'unreachable' }), { type: 'retry' }], STATES.loading);
  assert.equal(second.request, 2);
  assert.equal(rsReducer(second, okResult(1)), second, 'the answer to request 1 is dropped');
  assert.equal(rsReducer(second, okResult(2)).phase.kind, 'ready');
});

test('ready: the explained worksheet, from what the server really sends', () => {
  const body = golden.rateSchedule();
  const tree = view(STATES.ready);
  assert.equal(textOf(byClass(tree, 'headline')[0]), 'Tax: $11,504.00');

  const [table] = byTag(tree, 'table');
  assert.equal(textOf(byTag(table, 'caption')[0]), 'Rate schedule worksheet, 2026, Married filing jointly');
  assert.deepEqual(byTag(byTag(table, 'thead')[0], 'th').map((th) => textOf(th)), ['Line', 'Amount', 'Computed from', 'Parameters', 'Rounding']);
  const rows = byTag(byTag(table, 'tbody')[0], 'tr');
  assert.equal(rows.length, 17);
  // Server order, numbered, labelled.
  assert.deepEqual(rows.map((row) => textOf(byTag(row, 'th')[0]).replace(' - Result', '')), body.lines.map((line, i) => `${i + 1}. ${line.label}`));
  // The result is marked in words, in the row header, on exactly one row.
  assert.deepEqual(rows.filter((row) => textOf(byTag(row, 'th')[0]).endsWith(' - Result')).map((row) => rows.indexOf(row)), [16]);
  assert.equal(textOf(byTag(rows[16], 'td')[0]), '$11,504.00');
  assert.equal(textOf(byTag(rows[0], 'td')[0]), '$100,000.00');
  // "Computed from" is plain text: no button, no link, and no row is focusable.
  assert.equal(textOf(byTag(rows[2], 'td')[1]), 'Line 2 - Taxable income taxed at 10%');
  for (const row of rows) {
    assert.ok(!('tabindex' in row.attrs));
    assert.equal(byTag(byTag(row, 'td')[1], 'button').length + byTag(byTag(row, 'td')[1], 'a').length, 0);
  }
  // Every parameter reference is a link to its registry entry, and its text carries the id.
  const links = byTag(table, 'a');
  assert.equal(links.length, body.lines.flatMap((l) => l.params ?? []).length);
  for (const link of links) {
    assert.equal(link.attrs.href, '#/assumptions/irs.ordinary_brackets');
    assert.match(textOf(link), /^irs\.ordinary_brackets - 2026 - /);
  }
  assert.match(textOf(byTag(rows[2], 'td')[3]), /Rounded to the cent, half to even \(money\.cent_half_even\)/);
  // The table sits in a keyboard-scrollable region named by its caption.
  const region = table.parent;
  assert.deepEqual([region.attrs.role, region.attrs.tabindex, region.attrs['aria-labelledby']], ['region', '0', byTag(table, 'caption')[0].attrs.id]);
});

test('ready: the inputs as the server echoed them, and the unverified notice beside the number', () => {
  const body = golden.rateSchedule();
  const tree = view(STATES.ready);
  assert.equal(textOf(byClass(tree, 'inputs')[0]), 'Filing statusMarried filing jointlyTax year2026Taxable income$100,000.00');
  const notice = textOf(byClass(tree, 'unverifiedNotice')[0], { includeHidden: false });
  assert.equal(notice, `Computed from parameter vintage ${body.vintage.contentId}, which is unlocked and pending human verification. Not for decisions.`);
  assert.ok(!/(?<!un)\bverified\b/.test(textOf(tree)));
  assert.ok(!('role' in byClass(tree, 'unverifiedNotice')[0].attrs), 'the notice is content, not a live region');
});

test('ready: DOM order of focusable elements - form, worksheet region, parameter links, then the actions', () => {
  const order = focusables(view(STATES.ready));
  assert.deepEqual(order.slice(0, 5), ['select:#rs-status', 'input:#rs-year', 'input:#rs-income', 'button:Calculate', 'div:rs-worksheet-caption']);
  assert.deepEqual(order.slice(-3), ['button:Download CSV', 'button:Download JSON', 'button:Print']);
  assert.ok(order.slice(5, -3).every((entry) => entry.startsWith('a:irs.ordinary_brackets')));
  const note = textOf(byClass(view(STATES.ready), 'actions')[0]);
  assert.ok(note.startsWith(EXPORT_WARNING), 'the warning is the first thing in the block');
});

test('a parameter id that no route can address renders as text, never as a dead link', () => {
  const body = golden.rateSchedule();
  body.lines[1].params[0].paramId = 'not addressable/id';
  const state = run([...typed(FIELDS), { type: 'submit' }, { type: 'result', request: 1, outcome: { ok: true, value: body } }]);
  const tree = view(state);
  assert.ok(textOf(tree).includes('not addressable/id - 2026 - mfj - top_of_10'));
  assert.ok(byTag(tree, 'a').every((a) => a.attrs.href === '#/assumptions/irs.ordinary_brackets'));
});

test('refused by the server: the UI\'s words go in the focused summary, with no alert', () => {
  const state = STATES['refused by the server'];
  assert.equal(state.focusSeq, 1);
  assert.equal(state.lastInputs, null);
  const tree = view(state);
  const summary = byId(tree, ERROR_SUMMARY_ID);
  assert.match(textOf(summary), /outside what this build’s parameters cover/);
  // The server names no field, so the UI attributes the refusal to none: one general
  // message, no "Check: <field>" jump, no list, and no field marked invalid.
  assert.match(textOf(summary), /does not say which of the three values it refused/);
  assert.ok(!textOf(tree).includes('Check:'));
  assert.equal(byTag(summary, 'button').length, 0);
  assert.equal(byTag(summary, 'ul').length, 0);
  assert.deepEqual(byAttr(tree, 'aria-invalid', 'true'), []);
  assert.equal(byClass(tree, 'fieldError').length, 0);
  assert.equal(byAttr(tree, 'role', 'alert').length, 0);
  assert.equal(byTag(tree, 'table').length, 0);
});

test('other failures: an alert that never takes focus, and Try again only where it can help', () => {
  const unreachable = view(STATES.unreachable);
  const [alert] = byAttr(unreachable, 'role', 'alert');
  assert.match(textOf(alert), /is not answering/);
  assert.ok(!('tabindex' in alert.attrs));
  assert.deepEqual(byTag(byClass(unreachable, 'result')[0], 'button').map((b) => textOf(b)), ['Try again']);
  assert.equal(byTag(alert, 'button').length, 0, 'the button is beside the alert, not inside it');

  const defect = view(STATES.defect);
  assert.match(textOf(byAttr(defect, 'role', 'alert')[0]), /This is a defect; the code is request_invalid\./);
  assert.equal(byTag(byClass(defect, 'result')[0], 'button').length, 0);
  assert.ok(!textOf(defect).includes('never rendered'), 'the server message is never shown');
});

test('exporting: the download buttons are aria-disabled and a second click is ignored; Print stays available', () => {
  const tree = view(STATES.exporting);
  const buttons = Object.fromEntries(byTag(byClass(tree, 'actions')[0], 'button').map((b) => [textOf(b), b.attrs['aria-disabled']]));
  assert.deepEqual(buttons, { 'Download CSV': 'true', 'Download JSON': 'true', Print: undefined });
  assert.equal(rsReducer(STATES.exporting, { type: 'export-started', format: 'json' }), STATES.exporting);
  assert.equal(rsReducer(STATES.idle, { type: 'export-started', format: 'csv' }), STATES.idle, 'nothing to export before a result');
});

test('export: the last successful inputs and the format are posted, and the server text reaches saveFile untouched', async () => {
  const bodies = { csv: golden.rateScheduleCsvExport(), json: golden.rateScheduleJsonExport() };
  const mimes = { csv: 'text/csv;charset=utf-8', json: 'application/json' };
  for (const format of ['csv', 'json']) {
    // The CSV leg runs through a body whose json() rejects.
    // The header is the server's literal (crates/pfp-server/src/api/export.rs, pinned by the header snapshot).
    const api = fakeApiEnv((path) =>
      path.endsWith('/export') ? fakeResponse(200, bodies[format], { 'Content-Disposition': `attachment; filename="rate-schedule.${format}"` }) : fakeResponse(200, JSON.stringify(golden.rateSchedule())),
    );
    const app = fakeAppEnv(api.env);
    const client = createClient(api.env, () => assert.fail('no session was lost'));

    let state = run([...typed(FIELDS), { type: 'submit' }]);
    state = rsReducer(state, await performCalculation(client, state.pending, state.request));
    assert.equal(state.phase.kind, 'ready');
    // The form is edited after the result: the export must NOT pick this up.
    state = run(typed({ status: 'single', year: '2025', income: '1' }), state);
    state = rsReducer(state, { type: 'export-started', format });
    state = rsReducer(state, await performExport(client, app.env.saveFile, state.lastInputs, state.exportPhase.format));

    assert.equal(api.calls[1].input, '/api/v1/tax/rate-schedule/export');
    assert.deepEqual(api.calls[1].body, { ...INPUTS, format });
    assert.deepEqual(api.responses[1].reads, { json: 0, text: 1 });
    assert.equal(app.log.files.length, 1);
    const [file] = app.log.files;
    assert.equal(file.text, bodies[format], 'the bytes the server sent, never parsed and re-serialised');
    assert.equal(file.mime, mimes[format]);
    assert.equal(file.filename, `rate-schedule.${format}`, 'the name is the one the server sent: one source of truth');
    assert.deepEqual(state.exportPhase, { kind: 'done', filename: file.filename });
  }
});

test('export: a failure saves nothing and is reported by an alert', async () => {
  const api = fakeApiEnv(() => errorResponse(500, 'internal_error'));
  const app = fakeAppEnv(api.env);
  const client = createClient(api.env, () => {});
  const event = await performExport(client, app.env.saveFile, INPUTS, 'csv');
  assert.deepEqual(app.log.files, []);
  const state = rsReducer(STATES.exporting, event);
  assert.deepEqual(state.exportPhase, { kind: 'failed', failure: { kind: 'server-failed', code: 'internal_error' } });
  assert.match(textOf(byAttr(view(state), 'role', 'alert')[0]), /code internal_error/);
});

test('one filename source: the server\'s Content-Disposition, accepted only in the exact shape the server writes', () => {
  assert.equal(attachmentFilename('attachment; filename="rate-schedule.csv"', 'csv'), 'rate-schedule.csv');
  assert.equal(attachmentFilename('attachment; filename="rate-schedule.json"', 'json'), 'rate-schedule.json');
  // The client composes no name of its own: the literals the server sends are the ones in its source.
  const serverSource = readFileSync(join(import.meta.dirname, '..', '..', 'crates', 'pfp-server', 'src', 'api', 'export.rs'), 'utf8');
  for (const format of ['csv', 'json']) {
    const literal = `attachment; filename=\\"rate-schedule.${format}\\"`;
    assert.ok(serverSource.includes(literal), `export.rs sends ${literal}`);
  }
  const source = readFileSync(join(import.meta.dirname, '..', 'src', 'screens', 'rate-schedule', 'state.ts'), 'utf8');
  assert.ok(!source.includes('pfp-rate-schedule-'), 'no second, client-composed filename');

  // A header is input. Anything but the strict shape falls back to a literal.
  const hostile = [
    null,
    '',
    'inline',
    'attachment',
    'attachment; filename=rate-schedule.csv',
    'attachment; filename="../rate-schedule.csv"',
    'attachment; filename="a/b.csv"',
    'attachment; filename="a\\\\b.csv"',
    'attachment; filename=".csv"',
    'attachment; filename=".hidden.csv"',
    'attachment; filename="-x.csv"',
    'attachment; filename="x-.csv"',
    'attachment; filename="Rate.csv"',
    'attachment; filename="a b.csv"',
    'attachment; filename="a%2Fb.csv"',
    'attachment; filename="a.csv.exe"',
    'attachment; filename="a.exe"',
    'attachment; filename="a.csv"; filename*=UTF-8\'\'evil.exe',
    ' attachment; filename="a.csv"',
    'attachment; filename="a.csv"\r\nX: y',
    `attachment; filename="${'a'.repeat(65)}.csv"`,
    // Well-formed, but not the format that was asked for.
    'attachment; filename="rate-schedule.json"',
  ];
  for (const header of hostile) {
    assert.equal(attachmentFilename(header, 'csv'), 'export.csv', String(header));
  }
  assert.equal(fallbackFilename('json'), 'export.json');
  assert.equal(attachmentFilename(`attachment; filename="${'a'.repeat(64)}.csv"`, 'csv'), `${'a'.repeat(64)}.csv`);
});

test('export: a response without a usable header still saves the server text, under the fallback name', async () => {
  const api = fakeApiEnv(() => fakeResponse(200, 'x', { 'Content-Disposition': 'attachment; filename="../../evil.csv"' }));
  const app = fakeAppEnv(api.env);
  const event = await performExport(createClient(api.env, () => {}), app.env.saveFile, INPUTS, 'csv');
  assert.deepEqual(app.log.files, [{ text: 'x', mime: 'text/csv;charset=utf-8', filename: 'export.csv' }]);
  assert.deepEqual(event, { type: 'export-result', outcome: { ok: true, value: 'export.csv' } });
});

test('the plain warning precedes BOTH ways out - the downloads and Print - and describes each button', () => {
  const tree = view(STATES.ready);
  const actions = byClass(tree, 'actions')[0];
  const inOrder = elements(actions).filter((e) => e.tag === 'p' || e.tag === 'button');
  assert.deepEqual(inOrder.map((e) => e.tag), ['p', 'button', 'button', 'button'], 'the warning is first in reading and focus order');
  const [warning, ...buttons] = inOrder;
  assert.equal(textOf(warning), EXPORT_WARNING);
  assert.deepEqual(buttons.map((b) => textOf(b)), ['Download CSV', 'Download JSON', 'Print']);
  for (const button of buttons) {
    assert.equal(button.attrs['aria-describedby'], warning.attrs.id, textOf(button));
  }
  // It is about a print as much as a download: a print is not "a file" and does not go "where downloads go".
  assert.match(EXPORT_WARNING, /printed page/);
  assert.match(EXPORT_WARNING, /downloaded file/);
  assert.match(EXPORT_WARNING, /taxable income you entered/);
  assert.match(EXPORT_WARNING, /PDF/);
  assert.ok(!/^The file\b/.test(EXPORT_WARNING), 'not worded as a download-only note');
});

test('in-cell lists keep list semantics under list-style: none (WebKit drops the role from a bare ul)', () => {
  const tree = view(STATES.ready);
  const table = byTag(tree, 'table')[0];
  const lists = byTag(table, 'ul');
  assert.ok(lists.length >= 2);
  const classes = new Set();
  for (const list of lists) {
    assert.equal(list.attrs.role, 'list', `ul.${list.attrs.class}`);
    classes.add(list.attrs.class);
    // The role on the ul is what restores `listitem` on its children: an li must not override it.
    for (const item of list.children) {
      assert.equal(item.tag, 'li');
      assert.ok(!('role' in item.attrs));
    }
  }
  assert.deepEqual([...classes].sort(), ['cellList', 'paramList']);
  // The row the finding named: the total is computed from several lines, each its own item.
  const widest = lists.filter((l) => l.attrs.class === 'cellList').map((l) => l.children.length).sort((a, b) => b - a)[0];
  assert.ok(widest >= 2, 'a "Computed from" cell with several items exists in the golden worksheet');
  // Both classes really are list-style: none - which is why the role is needed.
  const css = readFileSync(join(import.meta.dirname, '..', 'src', 'components', 'Table.module.css'), 'utf8');
  assert.match(css, /\.cellList,\s*\.paramList\s*\{[^}]*list-style:\s*none/);
});

test('the golden CSV export is what the design says: bare money, quoted text, CRLF, no BOM', () => {
  const csv = golden.rateScheduleCsvExport();
  assert.ok(!csv.startsWith('﻿'));
  const records = csv.split('\r\n');
  assert.equal(records.pop(), '');
  assert.equal(records.length, 18);
  assert.match(records[17], /^2026,"mfj",17,"sched\.tax","[^"]+",11504\.00,/);
  assert.ok(records.slice(1).every((r) => r.endsWith(',"no","no"')), 'every row says the vintage is unlocked and unverified');
});

// WCAG 2.4.3: "Try again" sits in the failure block, and the loading state unmounts
// that block while the button holds focus.

test('Try again: the button it unmounts hands focus to the result heading, which is never a live region', () => {
  const failedState = STATES.unreachable;
  assert.equal(failedState.resultFocusSeq, 0);
  const retried = rsReducer(failedState, { type: 'retry' });
  assert.equal(retried.phase.kind, 'loading');
  // The defect, on the markup: the control is in the tab order before and gone after.
  assert.ok(focusables(view(failedState)).includes('button:Try again'));
  assert.ok(!focusables(view(retried)).includes('button:Try again'));
  // The fix: the reducer asks for focus, and the target exists in the state that follows.
  assert.equal(retried.resultFocusSeq, 1);
  assert.equal(retried.focusSeq, failedState.focusSeq, 'the error summary is not asked for');
  for (const state of Object.values(STATES).concat(retried)) {
    const heading = byId(view(state), RESULT_HEADING_ID);
    assert.equal(heading.tag, 'h3');
    assert.equal(heading.attrs.tabindex, '-1');
    assert.ok(!('role' in heading.attrs) && !('aria-live' in heading.attrs));
    assert.equal(elements(heading).filter((e) => 'role' in e.attrs).length, 0);
  }
  assert.ok(!focusables(view(retried)).some((entry) => entry.startsWith('h3')), 'script-only: never a tab stop');
  // One message, one channel: the focused heading does not repeat the polite text.
  assert.ok(!textOf(byId(view(retried), RESULT_HEADING_ID)).includes(rsAnnouncements(retried).polite));
  // A second failure and a second retry ask again.
  const again = run([failedResult({ kind: 'unreachable' }, 2), { type: 'retry' }], retried);
  assert.equal(again.resultFocusSeq, 2);
});

test('Try again with a form that no longer validates: the summary takes focus, not the result heading', () => {
  const edited = run([{ type: 'change', field: 'year', value: '' }, { type: 'retry' }], STATES.unreachable);
  assert.equal(edited.phase.kind, 'failed', 'the alert and its button stay');
  assert.equal(edited.resultFocusSeq, 0);
  assert.equal(edited.focusSeq, STATES.unreachable.focusSeq + 1);
});

test('no other event moves focus to the result heading: their controls survive', () => {
  // Calculate (or Enter in a field) from a failed state: the focused control is in the form.
  const resubmitted = rsReducer(STATES.unreachable, { type: 'submit' });
  assert.equal(resubmitted.phase.kind, 'loading');
  assert.equal(resubmitted.resultFocusSeq, 0);
  for (const state of Object.values(STATES)) {
    assert.equal(state.resultFocusSeq, 0);
  }
  // A retry event in a state that offers no retry changes nothing about focus.
  assert.equal(rsReducer(STATES.loading, { type: 'retry' }).resultFocusSeq, 0);
});
