// Session states and the one-click recovery from a displaced cookie (S-28, the
// client half): the reducer's whole transition table, every row of the notice,
// and "one message, one channel" for every state.

import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { test } from 'node:test';
import { createElement } from 'react';

import { createClient } from '../src/api/client.ts';
import { clearProof, PROOF_KEY } from '../src/session/api.ts';
import { describeSession } from '../src/session/handshake.ts';
import { SessionNotice } from '../src/session/SessionNotice.tsx';
import { initialShellSession, NOTICE_HEADING_ID, sessionLossHandler, noticeText, relaunchOffered, sessionAnnouncements, sessionFocus, sessionReducer } from '../src/session/state.ts';
import { assertAccessible, assertOneChannel } from './support/a11y.mjs';
import { errorResponse, fakeApiEnv, fakeResponse } from './support/fakes.mjs';
import { byAttr, byId, byTag, elements, focusables, textOf } from './support/markup.mjs';
import { render } from './support/render.mjs';

const SESSIONS = ['connected', 'no-token', 'rejected', 'session-ended', 'displaced', 'unreachable'];
const RELAUNCHES = [
  { kind: 'idle' },
  { kind: 'requesting' },
  { kind: 'reopened' },
  { kind: 'throttled' },
  { kind: 'exhausted' },
  { kind: 'unavailable' },
  { kind: 'failed', failure: { kind: 'unreachable' } },
  { kind: 'failed', failure: { kind: 'session-ended' } },
  { kind: 'failed', failure: { kind: 'server-failed', code: 'internal_error' } },
];
const ALL_STATES = SESSIONS.flatMap((session) => (session === 'displaced' ? RELAUNCHES.map((relaunch) => ({ session, relaunch })) : [{ session, relaunch: { kind: 'idle' } }]));

const ok = { ok: true, value: {} };
const failed = (failure) => ({ ok: false, failure });
const displaced = (relaunch) => ({ session: 'displaced', relaunch });

test('the initial state is the handshake result', () => {
  for (const kind of SESSIONS) {
    assert.deepEqual(initialShellSession({ kind }), { session: kind, relaunch: { kind: 'idle' } });
  }
});

test('a 409 on any screen call moves the shell to displaced; a 401 ends the session', () => {
  for (const state of ALL_STATES) {
    const after = sessionReducer(state, { type: 'session-lost', loss: 'displaced' });
    assert.equal(after.session, 'displaced');
    // A second 409 must not reset a recovery already in progress.
    assert.deepEqual(after.relaunch, state.session === 'displaced' ? state.relaunch : { kind: 'idle' });
    assert.deepEqual(sessionReducer(state, { type: 'session-lost', loss: 'session-ended' }), { session: 'session-ended', relaunch: { kind: 'idle' } });
  }
});

test('relaunch-requested: only where the button exists, and never twice at once', () => {
  for (const state of ALL_STATES) {
    const after = sessionReducer(state, { type: 'relaunch-requested' });
    const expected = relaunchOffered(state) && state.relaunch.kind !== 'requesting' ? displaced({ kind: 'requesting' }) : state;
    assert.deepEqual(after, expected, JSON.stringify(state));
  }
});

test('relaunch-result: every outcome, and only while a request is in flight', () => {
  const outcomes = [
    [ok, { kind: 'reopened' }],
    [failed({ kind: 'relaunch-throttled' }), { kind: 'throttled' }],
    [failed({ kind: 'relaunch-exhausted' }), { kind: 'exhausted' }],
    [failed({ kind: 'open-unavailable' }), { kind: 'unavailable' }],
    [failed({ kind: 'unreachable' }), { kind: 'failed', failure: { kind: 'unreachable' } }],
    [failed({ kind: 'session-ended' }), { kind: 'failed', failure: { kind: 'session-ended' } }],
  ];
  for (const [outcome, relaunch] of outcomes) {
    assert.deepEqual(sessionReducer(displaced({ kind: 'requesting' }), { type: 'relaunch-result', outcome }), displaced(relaunch));
    for (const state of ALL_STATES.filter((s) => s.relaunch.kind !== 'requesting')) {
      assert.deepEqual(sessionReducer(state, { type: 'relaunch-result', outcome }), state);
    }
  }
});

test('the button exists exactly where a click can work', () => {
  const offered = ALL_STATES.filter(relaunchOffered).map((s) => `${s.relaunch.kind}${s.relaunch.failure ? `:${s.relaunch.failure.kind}` : ''}`);
  assert.deepEqual(offered, ['idle', 'requesting', 'throttled', 'failed:unreachable']);
});

test('every row of the notice, rendered', () => {
  const rows = [
    [{ session: 'connected', relaunch: { kind: 'idle' } }, null, false],
    [{ session: 'no-token', relaunch: { kind: 'idle' } }, describeSession({ kind: 'no-token' }), false],
    [{ session: 'rejected', relaunch: { kind: 'idle' } }, describeSession({ kind: 'rejected' }), false],
    [{ session: 'session-ended', relaunch: { kind: 'idle' } }, describeSession({ kind: 'session-ended' }), false],
    [{ session: 'unreachable', relaunch: { kind: 'idle' } }, describeSession({ kind: 'unreachable' }), false],
    [displaced({ kind: 'idle' }), 'This browser’s session was displaced by another local site. Your work in the application is not lost.', true],
    [displaced({ kind: 'requesting' }), 'Asking the application to re-open…', true],
    [displaced({ kind: 'reopened' }), 'The application opened a new tab. Continue there; this tab can be closed.', false],
    [displaced({ kind: 'throttled' }), 'Re-opening was asked for too often. Wait a few seconds and try again.', true],
    [displaced({ kind: 'exhausted' }), 'This session cannot re-open the application again. Quit the application and start it again from its launcher.', false],
    [displaced({ kind: 'unavailable' }), 'This build cannot re-open itself. Quit the application and start it again from its launcher.', false],
    [displaced({ kind: 'failed', failure: { kind: 'unreachable' } }), 'The application on this computer is not answering. If you quit it, start it again from its launcher.', true],
    [displaced({ kind: 'failed', failure: { kind: 'session-ended' } }), 'This tab’s session has ended. Open the page again from the application.', false],
  ];
  for (const [state, text, button] of rows) {
    const { tree, markup } = render(createElement(SessionNotice, { state, onRelaunch() {} }));
    if (text === null) {
      assert.equal(markup, '');
      continue;
    }
    assertAccessible(tree, { focusTargets: [NOTICE_HEADING_ID] });
    const [alert] = byAttr(tree, 'role', 'alert');
    assert.equal(textOf(alert, { includeHidden: false }), text);
    assert.ok(!('tabindex' in alert.attrs), 'the alert never takes focus');
    const buttons = byTag(tree, 'button');
    // Absent - not merely disabled - in a terminal state.
    assert.equal(buttons.length, button ? 1 : 0, JSON.stringify(state));
    if (button) {
      assert.equal(textOf(buttons[0]), 'Re-open from the application');
      assert.equal(buttons[0].attrs['aria-disabled'], state.relaunch.kind === 'requesting' ? 'true' : undefined);
      assert.ok(!byAttr(tree, 'role', 'alert').some((a) => byTag(a, 'button').length !== 0), 'the button is outside the alert');
    }
  }
});

test('one message, one channel: connected is polite-only, every other state alert-only', () => {
  for (const state of ALL_STATES) {
    const announcements = sessionAnnouncements(state);
    const { tree } = render(createElement(SessionNotice, { state, onRelaunch() {} }));
    assertOneChannel(tree, announcements);
    if (state.session === 'connected') {
      assert.deepEqual(announcements, { polite: 'Connected to the application on this computer.' });
    } else {
      assert.deepEqual(Object.keys(announcements), ['alert']);
      assert.equal(announcements.alert, noticeText(state));
    }
  }
});

test('one click is one request: the recovery posts to relaunch with the proof and nothing else', async () => {
  const answers = [
    [() => fakeResponse(202, '{}'), { kind: 'reopened' }],
    [() => errorResponse(429, 'relaunch_throttled'), { kind: 'throttled' }],
    [() => errorResponse(429, 'relaunch_exhausted'), { kind: 'exhausted' }],
    [() => errorResponse(503, 'open_unavailable'), { kind: 'unavailable' }],
    [() => errorResponse(401, 'session_required'), { kind: 'failed', failure: { kind: 'session-ended' } }],
    [() => { throw new TypeError('network'); }, { kind: 'failed', failure: { kind: 'unreachable' } }],
  ];
  for (const [respond, relaunch] of answers) {
    const api = fakeApiEnv(respond);
    const lost = [];
    const client = createClient(api.env, (loss) => lost.push(loss));
    let state = sessionReducer(displaced({ kind: 'idle' }), { type: 'relaunch-requested' });
    state = sessionReducer(state, { type: 'relaunch-result', outcome: await client.relaunch() });
    assert.deepEqual(state, displaced(relaunch));
    assert.equal(api.calls.length, 1);
    assert.equal(api.calls[0].input, '/api/v1/session/relaunch');
    assert.equal(api.calls[0].init.body, undefined);
    assert.equal(api.calls[0].init.headers['X-PFP-Proof'], 'cd'.repeat(32));
    assert.deepEqual(lost, [], 'a relaunch outcome never replaces the notice that offers the recovery');
  }
});

// WCAG 2.4.3. Focus on an unmounted control falls to <body>, and the next Tab
// restarts at the top of the document. Whenever a session transition removes a
// control that may hold focus, the shell must be told where focus goes instead.

const notice = (state) => render(createElement(SessionNotice, { state, onRelaunch() {} })).tree;

test('the notice heading is a script-only focus target outside every live region', () => {
  for (const state of ALL_STATES.filter((s) => s.session !== 'connected')) {
    const tree = notice(state);
    const heading = byId(tree, NOTICE_HEADING_ID);
    assert.equal(heading.tag, 'h2');
    assert.equal(heading.attrs.tabindex, '-1', 'focusable by script, never in the tab order');
    assert.ok(!('role' in heading.attrs) && !('aria-live' in heading.attrs));
    assert.equal(elements(heading).filter((e) => e.attrs.role === 'alert').length, 0);
    assert.ok(!focusables(tree).some((entry) => entry.startsWith('h2')));
    // The focused heading does not repeat the alert: one message, one channel.
    assert.ok(!textOf(heading).includes(noticeText(state)));
  }
});

test('focus: every transition that withdraws the re-open button names the heading; one that keeps it leaves focus alone', () => {
  const events = [
    { type: 'relaunch-requested' },
    { type: 'session-lost', loss: 'displaced' },
    { type: 'session-lost', loss: 'session-ended' },
    ...[ok, failed({ kind: 'relaunch-throttled' }), failed({ kind: 'relaunch-exhausted' }), failed({ kind: 'open-unavailable' }), failed({ kind: 'unreachable' }), failed({ kind: 'session-ended' }), failed({ kind: 'server-failed', code: 'internal_error' })].map(
      (outcome) => ({ type: 'relaunch-result', outcome }),
    ),
  ];
  let withdrawn = 0;
  for (const before of ALL_STATES) {
    for (const event of events) {
      const after = sessionReducer(before, event);
      const target = sessionFocus(before, after);
      const label = `${JSON.stringify(before)} + ${JSON.stringify(event)}`;
      if (after.session === 'connected') {
        assert.equal(target, null, label);
        continue;
      }
      // The property, stated on the markup: a control present before and absent
      // after means focus may have been on it, so a target is required - and the
      // target is rendered, with tabindex="-1", in the state that follows.
      const lost = focusables(before.session === 'connected' ? { children: [] } : notice(before)).filter((entry) => !focusables(notice(after)).includes(entry));
      const replacedScreen = before.session === 'connected';
      if (lost.length !== 0 || replacedScreen) {
        withdrawn += 1;
        assert.equal(target, NOTICE_HEADING_ID, label);
        assert.equal(byId(notice(after), target).attrs.tabindex, '-1', label);
      } else {
        assert.equal(target, null, `focus is left where it is: ${label}`);
      }
    }
  }
  assert.ok(withdrawn !== 0);
});

test('focus: the named cases - relaunch outcomes, and a 409 or 401 in the middle of a screen', () => {
  const requesting = displaced({ kind: 'requesting' });
  const connected = { session: 'connected', relaunch: { kind: 'idle' } };
  for (const relaunch of [{ kind: 'reopened' }, { kind: 'exhausted' }, { kind: 'unavailable' }, { kind: 'failed', failure: { kind: 'session-ended' } }, { kind: 'failed', failure: { kind: 'server-failed', code: 'internal_error' } }]) {
    assert.equal(sessionFocus(requesting, displaced(relaunch)), NOTICE_HEADING_ID, relaunch.kind);
  }
  // The button survives: focus stays on it and the alert alone speaks.
  assert.equal(sessionFocus(displaced({ kind: 'idle' }), requesting), null);
  assert.equal(sessionFocus(requesting, displaced({ kind: 'throttled' })), null);
  assert.equal(sessionFocus(requesting, displaced({ kind: 'failed', failure: { kind: 'unreachable' } })), null);
  // A screen's controls are all unmounted when the notice replaces it.
  assert.equal(sessionFocus(connected, sessionReducer(connected, { type: 'session-lost', loss: 'displaced' })), NOTICE_HEADING_ID);
  assert.equal(sessionFocus(connected, sessionReducer(connected, { type: 'session-lost', loss: 'session-ended' })), NOTICE_HEADING_ID);
  // No transition, no move: a page that loads not connected keeps the document's natural start.
  for (const state of ALL_STATES) {
    assert.equal(sessionFocus(state, state), null);
  }
});

// Effects do not run under renderToStaticMarkup, so the containers' obedience to
// the pure focus decisions is pinned on their source: a cheap first line, as the
// storage lint is, in front of the browser test that will exercise real focus.
test('the containers obey the focus decisions', async () => {
  const { readFile } = await import('node:fs/promises');
  const source = (path) => readFile(new URL(`../src/${path}`, import.meta.url), 'utf8');
  const app = await source('App.tsx');
  assert.match(app, /const target = sessionFocus\(previousShell\.current, shell\);/);
  assert.match(app, /if \(target !== null\) \{\s*env\.focusById\(target\);/);
  const screen = await source('screens/rate-schedule/RateScheduleScreen.tsx');
  assert.match(screen, /if \(state\.resultFocusSeq !== 0\) \{\s*env\.focusById\(RESULT_HEADING_ID\);\s*\}\s*\}, \[env, state\.resultFocusSeq\]\);/);
  const registry = await source('screens/assumptions/AssumptionsScreen.tsx');
  assert.match(registry, /onRetry=\{retryWithFocus\(env, onRetry\)\}/);
});

test('a mid-use 401 removes the dead proof from the injected storage; a 409 keeps the proof the recovery needs', async () => {
  const cases = [
    [401, 'session_required', 'session-ended', false],
    [409, 'session_cookie_displaced', 'displaced', true],
  ];
  for (const [status, code, session, proofKept] of cases) {
    const api = fakeApiEnv(() => errorResponse(status, code));
    assert.ok(api.store.has(PROOF_KEY), 'the tab starts with a proof');
    let shell = { session: 'connected', relaunch: { kind: 'idle' } };
    const client = createClient(api.env, sessionLossHandler(api.env, (event) => { shell = sessionReducer(shell, event); }));
    await client.rateSchedule({ year: 2026, filingStatus: 'mfj', taxableIncome: 10000000 });
    assert.equal(shell.session, session);
    assert.equal(api.store.has(PROOF_KEY), proofKept, `${String(status)}: ${proofKept ? 'kept' : 'removed'}`);
    assert.equal(api.env.sessionStorage.getItem(PROOF_KEY) === null, !proofKept);
    // The next call therefore carries no dead credential.
    await client.status();
    assert.equal('X-PFP-Proof' in api.calls.at(-1).init.headers, proofKept);
  }

  // Through the injected storage and nothing else: no other key is touched.
  const api = fakeApiEnv(() => fakeResponse(200, '{}'));
  api.store.set('unrelated', 'kept');
  clearProof(api.env);
  assert.deepEqual([...api.store.keys()], ['unrelated']);

  // A 401 on the recovery itself is not reported to the shell (it must not replace the
  // displaced notice), so the handler is not what runs there.
  const app = readFileSync(join(import.meta.dirname, '..', 'src', 'App.tsx'), 'utf8');
  assert.match(app, /createClient\(env\.api, sessionLossHandler\(env\.api, dispatch\)\)/);
});
