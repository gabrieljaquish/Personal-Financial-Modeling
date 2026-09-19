// WCAG 1.4.10 Reflow, as far as markup and stylesheet source can show it: a
// contentId carries a 64-hex hash with no line-break opportunity, which is wider
// than a 320px viewport. Every element that renders one - found by rendering the
// real views from the golden API bodies, not from a list kept by hand - must be
// covered by a rule that declares `overflow-wrap: anywhere` (the property
// inherits, so a rule on the element or on an ancestor counts). That the browser
// then wraps it is for the browser suite.

import assert from 'node:assert/strict';
import { readdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { test } from 'node:test';
import { createElement } from 'react';

import { AppView } from '../src/App.tsx';
import { AssumptionsView } from '../src/screens/assumptions/AssumptionsView.tsx';
import { RateScheduleView } from '../src/screens/rate-schedule/RateScheduleView.tsx';
import { INITIAL_RS_STATE, rsReducer } from '../src/screens/rate-schedule/state.ts';
import { registryVm } from '../src/viewmodel/registry.ts';
import * as golden from './support/golden.mjs';
import { ancestors, elements } from './support/markup.mjs';
import { render } from './support/render.mjs';

const src = join(import.meta.dirname, '..', 'src');
const walk = (dir) => readdirSync(dir, { withFileTypes: true }).flatMap((e) => (e.isDirectory() ? walk(join(dir, e.name)) : [join(dir, e.name)]));

/** Class names that appear in the selector of a rule declaring `overflow-wrap: anywhere`. */
function wrappingClasses() {
  const out = new Set();
  for (const file of walk(src).filter((f) => f.endsWith('.css'))) {
    const css = readFileSync(file, 'utf8').replace(/\/\*[\s\S]*?\*\//g, '');
    for (const [, selectors, declarations] of css.matchAll(/([^{}]+)\{([^{}]*)\}/g)) {
      if (/overflow-wrap:\s*anywhere/.test(declarations)) {
        for (const [, name] of selectors.matchAll(/\.([A-Za-z_][\w-]*)/g)) {
          out.add(name);
        }
      }
    }
  }
  return out;
}

/** The longest run a browser cannot break: no white space, no hyphen, no slash. */
const UNBREAKABLE = /[^\s\-/]{30,}/;

function unwrapped(tree, wrapping) {
  const classesOf = (e) => (e.attrs?.class ?? '').split(/\s+/).filter(Boolean);
  return elements(tree)
    .filter((e) => e.children.some((c) => c.text !== undefined && UNBREAKABLE.test(c.text)))
    .filter((e) => ![e, ...ancestors(e)].some((n) => classesOf(n).some((c) => wrapping.has(c))))
    .map((e) => `<${e.tag} class="${e.attrs.class ?? ''}">`);
}

const handlers = { onChange() {}, onSubmit() {}, onRetry() {}, onJump() {}, onExport() {}, onPrint() {} };
const FIELDS = { status: 'mfj', year: '2026', income: '100,000' }; // synthetic
const ready = [
  ...Object.entries(FIELDS).map(([field, value]) => ({ type: 'change', field, value })),
  { type: 'submit' },
  { type: 'result', request: 1, outcome: { ok: true, value: golden.rateSchedule() } },
].reduce(rsReducer, INITIAL_RS_STATE);

test('reflow: the rules that must wrap a contentId do', () => {
  const wrapping = wrappingClasses();
  for (const name of ['unverifiedNotice', 'vintageBanner']) {
    assert.ok(wrapping.has(name), `.${name} declares overflow-wrap: anywhere`);
  }
});

test('reflow: every element that renders an unbreakable run is covered by overflow-wrap: anywhere', () => {
  const wrapping = wrappingClasses();
  const registry = { kind: 'ready', value: registryVm(golden.assumptions()) };
  const trees = {
    'rate schedule, ready': render(createElement(RateScheduleView, { state: ready, publishedYears: ['2026'], handlers })).tree,
    registry: render(createElement(AssumptionsView, { registry, routedId: null, onRetry() {}, onSorted() {} })).tree,
    'registry entry': render(createElement(AssumptionsView, { registry, routedId: 'irs.std_deduction', onRetry() {}, onSorted() {} })).tree,
    shell: render(
      createElement(AppView, {
        shell: { session: 'connected', relaunch: { kind: 'idle' } },
        route: { screen: 'not-found' },
        status: { kind: 'ready', value: golden.sessionStatus() },
        message: '',
        onSkip() {},
        onRelaunch() {},
      }),
    ).tree,
  };
  // The sample is real: the golden result does put a 64-hex hash on the screen.
  const notice = elements(trees['rate schedule, ready']).find((e) => (e.attrs.class ?? '').includes('unverifiedNotice'));
  assert.ok(notice.children.some((c) => /@[0-9a-f]{64}/.test(c.text ?? '')), 'the notice carries the full contentId');
  for (const [name, tree] of Object.entries(trees)) {
    assert.deepEqual(unwrapped(tree, wrapping), [], name);
  }
});

test('reflow: the detector itself finds an uncovered hash', () => {
  const tree = { tag: '#root', attrs: {}, parent: null, children: [] };
  const p = { tag: 'p', attrs: { class: 'plain' }, parent: tree, children: [{ text: `federal-2026@${'ab'.repeat(32)}, which` }] };
  tree.children.push(p);
  assert.deepEqual(unwrapped(tree, new Set(['other'])), ['<p class="plain">']);
  assert.deepEqual(unwrapped(tree, new Set(['plain'])), []);
});
