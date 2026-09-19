// The shell on every route and in every session state: landmarks, one h1, one
// polite region, the navigation's current page, the persistent banner, and the
// order of focusable elements in the DOM.

import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { test } from 'node:test';
import { createElement } from 'react';

import { App, AppView } from '../src/App.tsx';
import { NotFoundScreen } from '../src/screens/not-found/NotFoundScreen.tsx';
import { assertAccessible } from './support/a11y.mjs';
import { fakeApiEnv, fakeAppEnv } from './support/fakes.mjs';
import * as golden from './support/golden.mjs';
import { byAttr, byClass, byId, byTag, elements, focusables, textOf } from './support/markup.mjs';
import { render } from './support/render.mjs';

const SHELL_TARGETS = ['main-content', 'screen-heading', 'rs-result-heading', 'session-notice-heading'];

function renderApp(hash, session = { kind: 'connected' }) {
  const api = fakeApiEnv(() => { throw new TypeError('no network in a render'); });
  const app = fakeAppEnv(api.env, { hash });
  return { ...render(createElement(App, { initialSession: session }), { env: app.env }), app, api };
}

test('every route renders one h1, each landmark once, one status region, and the right current page', () => {
  const routes = [
    ['', 'Rate schedule', 'Rate schedule'],
    ['#/rate-schedule', 'Rate schedule', 'Rate schedule'],
    ['#/assumptions', 'Assumptions Registry', 'Assumptions Registry'],
    ['#/assumptions/irs.std_deduction', 'Assumptions Registry', 'Assumptions Registry'],
    ['#/nope', 'Nothing at this address', null],
  ];
  for (const [hash, heading, current] of routes) {
    const { tree } = renderApp(hash);
    assertAccessible(tree, { shell: true, focusTargets: SHELL_TARGETS });
    assert.equal(textOf(byId(tree, 'screen-heading')), heading, hash);
    assert.equal(byId(tree, 'screen-heading').tag, 'h2');
    const currentLinks = byAttr(tree, 'aria-current', 'page');
    assert.deepEqual(currentLinks.map((a) => textOf(a)), current === null ? [] : [current], hash);
    assert.equal(textOf(byAttr(tree, 'role', 'status')[0]), 'Connected to the application on this computer.');
  }
});

test('the initial render runs no effect: no title set, no focus moved, nothing fetched', () => {
  const { app, api } = renderApp('#/assumptions/irs.std_deduction');
  assert.deepEqual(app.log, { titles: [], focus: [], prints: 0, files: [], replaced: [] });
  assert.deepEqual(api.calls, []);
});

test('DOM order of focusable elements: skip link first, then navigation, then the screen', () => {
  const { tree } = renderApp('#/rate-schedule');
  assert.deepEqual(focusables(tree), [
    'a:Skip to main content',
    'a:Rate schedule',
    'a:Assumptions Registry',
    'select:#rs-status',
    'input:#rs-year',
    'input:#rs-income',
    'button:Calculate',
  ]);
  const notFound = renderApp('#/nope').tree;
  assert.deepEqual(focusables(notFound), ['a:Skip to main content', 'a:Rate schedule', 'a:Assumptions Registry', 'a:Rate schedule', 'a:Assumptions Registry']);
});

test('landmarks are in the order the grid areas are laid out: header, main, aside, footer', () => {
  const { tree } = renderApp('');
  const shell = byClass(tree, 'shell')[0];
  assert.deepEqual(shell.children.map((c) => c.tag), ['header', 'main', 'aside', 'footer']);
  assert.equal(byTag(tree, 'header')[0].children[0].attrs.href, '#main-content', 'the skip link is the first element');
});

test('the verification banner is on every route, is a labelled region and not a live region', () => {
  for (const hash of ['', '#/assumptions', '#/nope']) {
    const { tree } = renderApp(hash);
    const [banner] = byAttr(tree, 'aria-label', 'Parameter verification status');
    assert.equal(banner.attrs.role, 'region');
    assert.ok(!('aria-live' in banner.attrs));
    assert.match(textOf(banner), /treat every figure as unverified/);
  }
});

function shellWith(status, session = 'connected') {
  return render(
    createElement(
      AppView,
      { shell: { session, relaunch: { kind: 'idle' } }, route: { screen: 'not-found' }, status, message: '', onSkip() {}, onRelaunch() {} },
      createElement(NotFoundScreen),
    ),
  ).tree;
}

test('banner honesty: the golden status says unlocked and pending human verification, and never "verified"', () => {
  const tree = shellWith({ kind: 'ready', value: golden.sessionStatus() });
  assertAccessible(tree, { shell: true, focusTargets: SHELL_TARGETS });
  const banner = textOf(byAttr(tree, 'aria-label', 'Parameter verification status')[0], { includeHidden: false });
  assert.equal(banner, 'Parameter vintage federal-2026 is unlocked and pending human verification. Figures computed from it are not for decisions.');
  assert.ok(!/(?<!un)\bverified\b/.test(textOf(tree)), 'the word "verified" appears nowhere in the shell');
});

test('banner: a failed read keeps it; an all-true status renders the positive text in the aside and no banner', () => {
  const failed = shellWith({ kind: 'failed', failure: { kind: 'unreachable' } });
  assert.match(textOf(byAttr(failed, 'role', 'region')[0]), /could not be read - treat every figure as unverified/);
  assert.match(textOf(byTag(failed, 'aside')[0]), /Build identity could not be read/);

  const status = golden.sessionStatus();
  const settled = { ...status, vintages: [{ ...status.vintages[0], verified: true, lockedId: 'federal-2026@locked' }] };
  const tree = shellWith({ kind: 'ready', value: settled });
  assert.equal(byAttr(tree, 'aria-label', 'Parameter verification status').length, 0);
  assert.match(textOf(byTag(tree, 'aside')[0]), /locked as federal-2026@locked/);
});

test('the aside shows build identity from session/status', () => {
  const tree = shellWith({ kind: 'ready', value: golden.sessionStatus() });
  const aside = textOf(byTag(tree, 'aside')[0]);
  assert.match(aside, /Application version0\.0\.0-golden/);
  assert.match(aside, /API versionv1/);
  assert.match(aside, /federal-2026@[0-9a-f]{64} \(unlocked\)/);
});

test('a tab that is not connected shows the notice instead of the screens, and an empty polite region', () => {
  for (const kind of ['no-token', 'rejected', 'session-ended', 'unreachable', 'displaced']) {
    const { tree } = renderApp('#/rate-schedule', { kind });
    assertAccessible(tree, { shell: true, focusTargets: SHELL_TARGETS });
    assert.equal(byTag(tree, 'form').length, 0, kind);
    assert.equal(textOf(byAttr(tree, 'role', 'status')[0]), '', kind);
    assert.equal(byAttr(tree, 'role', 'alert').length, 1, kind);
    assert.equal(elements(tree).filter((e) => e.tag === 'h2').length, 2, 'the notice heading and the aside heading');
  }
});

test('not connected: the header claims no current page, on every route and in every session state (WCAG 2.4.2 / 4.1.2)', () => {
  for (const kind of ['no-token', 'rejected', 'session-ended', 'unreachable', 'displaced']) {
    for (const hash of ['#/rate-schedule', '#/assumptions', '#/assumptions/irs.std_deduction', '#/nope']) {
      const { tree } = renderApp(hash, { kind });
      assert.deepEqual(byAttr(tree, 'aria-current', 'page'), [], `${kind} ${hash}`);
      assert.equal(elements(tree).filter((e) => 'aria-current' in e.attrs).length, 0, `${kind} ${hash}`);
      // The links are still there: the navigation is not removed, it just claims nothing.
      assert.deepEqual(byTag(byTag(tree, 'nav')[0], 'a').map((a) => textOf(a)), ['Rate schedule', 'Assumptions Registry']);
      assert.equal(textOf(byId(tree, 'session-notice-heading')).length > 0, true);
    }
  }
  // Connected, the same routes do claim one.
  assert.equal(byAttr(renderApp('#/assumptions').tree, 'aria-current', 'page').length, 1);
});

test('the shell computes the title from what is on screen, and the router no longer sets one', () => {
  // Effects do not run under renderToStaticMarkup, so the wiring is pinned at the source;
  // `documentTitle` itself is covered case by case in router.test.mjs.
  const read = (...parts) => readFileSync(join(import.meta.dirname, '..', 'src', ...parts), 'utf8');
  const app = read('App.tsx');
  assert.match(app, /const title = documentTitle\(route, \{ connected, resolution: resolutionOf\(route, registry\.kind === 'ready' \? registry\.value\.ids : null\) \}\);/);
  assert.match(app, /useEffect\(\(\) => env\.setTitle\(title\), \[env, title\]\);/);
  assert.match(app, /<PrimaryNav route=\{shell\.session === 'connected' \? route : null\} \/>/);
  assert.ok(!read('router', 'useRoute.ts').includes('setTitle'), 'one writer of the title');
  assert.ok(!read('router', 'hash.ts').includes("'set-title'"));
});
