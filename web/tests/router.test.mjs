import assert from 'node:assert/strict';
import { test } from 'node:test';

import { documentTitle, hrefFor, INITIAL_ROUTER_STATE, needsRedirect, parseRoute, resolutionOf, routerReducer, titleFor } from '../src/router/hash.ts';
import { bootRouterState } from '../src/router/useRoute.ts';

const hash = (h) => ({ type: 'hash', hash: h });

test('routes parse, and everything else under #/ is Not found', () => {
  assert.deepEqual(parseRoute(''), { screen: 'rate-schedule' });
  assert.deepEqual(parseRoute('#'), { screen: 'rate-schedule' });
  assert.deepEqual(parseRoute('#/'), { screen: 'rate-schedule' });
  assert.deepEqual(parseRoute('#/rate-schedule'), { screen: 'rate-schedule' });
  assert.deepEqual(parseRoute('#/assumptions'), { screen: 'assumptions' });
  assert.deepEqual(parseRoute('#/assumptions/irs.ordinary_brackets'), { screen: 'assumption', id: 'irs.ordinary_brackets' });
  for (const bad of ['#/nope', '#/assumptions/', '#/assumptions/a b', '#/assumptions/a/b', `#/assumptions/${'a'.repeat(81)}`, '#/assumptions/%41', '#/rate-schedule?income=1', '#/Rate-Schedule']) {
    assert.deepEqual(parseRoute(bad), { screen: 'not-found' }, bad);
  }
});

test('a hash that is not a route is ignored: nothing in the page can knock the app off its route', () => {
  for (const foreign of ['#main-content', '#t=' + 'ab'.repeat(32), '#rs-year', '#x']) {
    assert.equal(parseRoute(foreign), null, foreign);
    const before = routerReducer(bootRouterState('#/assumptions'), hash('#/assumptions/irs.std_deduction'));
    const after = routerReducer(before, hash(foreign));
    assert.deepEqual(after.route, before.route);
    assert.deepEqual(after.effects, []);
  }
});

test('hrefFor and parseRoute round-trip every addressable route; an unaddressable id has no href', () => {
  for (const route of [{ screen: 'rate-schedule' }, { screen: 'assumptions' }, { screen: 'assumption', id: 'irs.std_deduction' }, { screen: 'assumption', id: 'A-z_0.9' }]) {
    assert.deepEqual(parseRoute(hrefFor(route)), route);
  }
  for (const id of ['has space', 'a/b', 'a'.repeat(81), '', 'é', 'a#b', 'a?b']) {
    assert.equal(hrefFor({ screen: 'assumption', id }), null, id);
  }
  assert.equal(hrefFor({ screen: 'not-found' }), null);
});

test('the first route event moves no focus - direct, or via the boot redirect', () => {
  for (const boot of ['', '#/', '#/assumptions', '#/assumptions/irs.std_deduction', '#/nope']) {
    const state = routerReducer(INITIAL_ROUTER_STATE, hash(boot));
    assert.equal(state.navigations, 'initial');
    assert.deepEqual(state.effects, [], boot);
  }
  // The boot redirect rewrites '' to '#/rate-schedule' with replaceState: the same
  // route arrives again and must still move no focus.
  const booted = bootRouterState('');
  const redirected = routerReducer(booted, hash('#/rate-schedule'));
  assert.deepEqual(redirected.effects, []);
  assert.equal(redirected.navigations, 'initial');
  assert.ok(needsRedirect('') && needsRedirect('#/') && !needsRedirect('#/assumptions'));
});

test('later route changes focus the right target; the title is not decided by the reducer', () => {
  let state = bootRouterState('#/rate-schedule');
  state = routerReducer(state, hash('#/assumptions'));
  assert.deepEqual(state.effects, [{ kind: 'focus', id: 'screen-heading' }]);
  state = routerReducer(state, hash('#/assumptions/irs.std_deduction'));
  assert.deepEqual(state.effects, [{ kind: 'focus', id: 'detail-irs.std_deduction' }]);
  state = routerReducer(state, hash('#/whatever'));
  assert.deepEqual(state.effects, [{ kind: 'focus', id: 'screen-heading' }]);
  assert.equal(titleFor({ screen: 'rate-schedule' }), 'Rate schedule - Personal Financial Modeling');
});

const APP = 'Personal Financial Modeling';
const ALL_ROUTES = [{ screen: 'rate-schedule' }, { screen: 'assumptions' }, { screen: 'assumption', id: 'irs.std_deduction' }, { screen: 'not-found' }];

test('the title describes what is on screen: a tab that is not connected says so on every route (WCAG 2.4.2)', () => {
  for (const route of ALL_ROUTES) {
    for (const resolution of ['unknown', 'resolved', 'missing']) {
      assert.equal(documentTitle(route, { connected: false, resolution }), `Not connected - ${APP}`, JSON.stringify(route));
    }
  }
  assert.equal(documentTitle({ screen: 'rate-schedule' }, { connected: true, resolution: 'unknown' }), `Rate schedule - ${APP}`);
  assert.equal(documentTitle({ screen: 'assumptions' }, { connected: true, resolution: 'unknown' }), `Assumptions Registry - ${APP}`);
  assert.equal(documentTitle({ screen: 'not-found' }, { connected: true, resolution: 'unknown' }), `Not found - ${APP}`);
});

test('a registry deep link is titled by its id only once the id resolves; an id that does not is "Not found"', () => {
  const ids = ['irs.std_deduction', 'irs.ordinary_brackets'];
  const present = { screen: 'assumption', id: 'irs.std_deduction' };
  const absent = { screen: 'assumption', id: 'irs.absent' };
  assert.equal(resolutionOf(present, ids), 'resolved');
  assert.equal(resolutionOf(absent, ids), 'missing');
  assert.equal(resolutionOf(absent, null), 'unknown', 'the registry has not loaded: nothing is claimed either way');
  assert.equal(resolutionOf({ screen: 'assumptions' }, ids), 'unknown');

  const title = (route, registryIds) => documentTitle(route, { connected: true, resolution: resolutionOf(route, registryIds) });
  assert.equal(title(present, ids), `irs.std_deduction - Assumptions Registry - ${APP}`);
  assert.equal(title(absent, ids), `Not found - ${APP}`);
  assert.ok(!title(absent, ids).includes('irs.absent'), 'the title never claims an id that does not exist');
  assert.equal(title(absent, null), `Assumptions Registry - ${APP}`);
  assert.ok(!title(absent, null).includes('irs.absent'));
});

test('no route carries anything but a parameter id: titles and hrefs are built from the id alone', () => {
  const href = hrefFor({ screen: 'assumption', id: 'irs.ordinary_brackets' });
  assert.equal(href, '#/assumptions/irs.ordinary_brackets');
  assert.ok(!href.includes('?') && !href.includes('='));
});
