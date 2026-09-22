// Session state of the shell, and the one-click recovery from a displaced cookie
// (SECURITY.md §7.1, test id S-28), as a pure reducer.
//
// The UI adds no timer, no retry loop and no automatic click: one click is one
// request, and the server's interval and cap stay the only policy.

import type { Announcements } from '../announce.ts';
import type { Outcome, SessionLoss } from '../api/client.ts';
import { failureText } from '../api/errors.ts';
import type { ApiFailure } from '../api/errors.ts';
import { clearProof } from './api.ts';
import type { ApiEnv } from './api.ts';
import { describeSession } from './handshake.ts';
import type { SessionState } from './handshake.ts';

export type RelaunchState =
  | { kind: 'idle' }
  | { kind: 'requesting' }
  /** 202: the application opened a new tab; this tab's proof belongs to the replaced session. */
  | { kind: 'reopened' }
  | { kind: 'throttled' }
  | { kind: 'exhausted' }
  | { kind: 'unavailable' }
  | { kind: 'failed'; failure: ApiFailure };

export interface ShellSession {
  readonly session: SessionState['kind'];
  /** Meaningful only while `session` is `displaced`. */
  readonly relaunch: RelaunchState;
}

export type SessionEvent =
  | { type: 'session-lost'; loss: SessionLoss }
  | { type: 'relaunch-requested' }
  | { type: 'relaunch-result'; outcome: Outcome<unknown> };

/**
 * What the API client calls when a screen's request finds the session gone. The
 * reducer stays pure; the one side effect lives here: a session that ENDED (401)
 * leaves a dead proof behind, and it is removed at once so the stored key is only
 * ever a live credential. A DISPLACED cookie (409) keeps its proof - the proof-only
 * relaunch recovery is built on it.
 */
export function sessionLossHandler(api: ApiEnv, dispatch: (event: SessionEvent) => void): (loss: SessionLoss) => void {
  return (loss) => {
    if (loss === 'session-ended') {
      clearProof(api);
    }
    dispatch({ type: 'session-lost', loss });
  };
}

export function initialShellSession(session: SessionState): ShellSession {
  return { session: session.kind, relaunch: { kind: 'idle' } };
}

/** Whether the re-open button exists. In a terminal state it is absent, not disabled. */
export function relaunchOffered(state: ShellSession): boolean {
  if (state.session !== 'displaced') {
    return false;
  }
  const { relaunch } = state;
  switch (relaunch.kind) {
    case 'idle':
    case 'requesting':
    case 'throttled':
      return true;
    case 'failed':
      return relaunch.failure.kind === 'unreachable';
    case 'reopened':
    case 'exhausted':
    case 'unavailable':
      return false;
  }
}

function relaunchFrom(outcome: Outcome<unknown>): RelaunchState {
  if (outcome.ok) {
    return { kind: 'reopened' };
  }
  switch (outcome.failure.kind) {
    case 'relaunch-throttled':
      return { kind: 'throttled' };
    case 'relaunch-exhausted':
      return { kind: 'exhausted' };
    case 'open-unavailable':
      return { kind: 'unavailable' };
    default:
      return { kind: 'failed', failure: outcome.failure };
  }
}

export function sessionReducer(state: ShellSession, event: SessionEvent): ShellSession {
  switch (event.type) {
    case 'session-lost':
      if (event.loss === 'displaced') {
        // A second 409 while already displaced must not reset a recovery in progress.
        return state.session === 'displaced' ? state : { session: 'displaced', relaunch: { kind: 'idle' } };
      }
      return { session: 'session-ended', relaunch: { kind: 'idle' } };
    case 'relaunch-requested':
      return relaunchOffered(state) && state.relaunch.kind !== 'requesting' ? { ...state, relaunch: { kind: 'requesting' } } : state;
    case 'relaunch-result':
      return state.session === 'displaced' && state.relaunch.kind === 'requesting' ? { ...state, relaunch: relaunchFrom(event.outcome) } : state;
  }
}

/** The heading of the session notice: a script-only focus target, never a live region. */
export const NOTICE_HEADING_ID = 'session-notice-heading';

/**
 * Where focus must go after `previous` became `next`, or `null` to leave it alone
 * (WCAG 2.4.3). Focus is moved only when the element that may hold it is about to
 * be unmounted, because focus on a removed element falls to <body> and the next
 * Tab restarts at the top of the document:
 *
 *   * the notice replaces a screen (connected -> anything else): every control of
 *     the screen is gone;
 *   * the re-open button is withdrawn (a terminal relaunch state): the button the
 *     person just activated is gone.
 *
 * While the button survives (requesting, throttled, unreachable) focus stays on
 * it, and the alert alone speaks. Pure, so the container only obeys it.
 */
export function sessionFocus(previous: ShellSession, next: ShellSession): string | null {
  if (next.session === 'connected') {
    return null;
  }
  const replacedScreen = previous.session === 'connected';
  const buttonWithdrawn = relaunchOffered(previous) && !relaunchOffered(next);
  return replacedScreen || buttonWithdrawn ? NOTICE_HEADING_ID : null;
}

/** The sentence of the session notice. `null` when connected: there is no notice. */
export function noticeText(state: ShellSession): string | null {
  if (state.session === 'connected') {
    return null;
  }
  if (state.session !== 'displaced') {
    return describeSession({ kind: state.session });
  }
  const { relaunch } = state;
  switch (relaunch.kind) {
    case 'idle':
      return 'This browser’s session was displaced by another local site. Your work in the application is not lost.';
    case 'requesting':
      return 'Asking the application to re-open…';
    case 'reopened':
      return 'The application opened a new tab. Continue there; this tab can be closed.';
    case 'throttled':
      return failureText({ kind: 'relaunch-throttled' });
    case 'exhausted':
      return failureText({ kind: 'relaunch-exhausted' });
    case 'unavailable':
      return failureText({ kind: 'open-unavailable' });
    case 'failed':
      return failureText(relaunch.failure);
  }
}

/** Connected is polite-only; every other state is alert-only; none is both. */
export function sessionAnnouncements(state: ShellSession): Announcements {
  const alert = noticeText(state);
  return alert === null ? { polite: describeSession({ kind: 'connected' }) } : { alert };
}
