// Transport results -> one failure union -> the UI's own words.
//
// The server's `message` is never rendered: every sentence a person reads is
// written here, keyed by the server's stable `code`, so a future server string
// can never inject wording into a screen. tests/errors-drift.test.mjs reads every
// code named in the OpenAPI document and asserts each one has an entry below;
// an unknown code falls to its status class.
//
// This departs from ARCHITECTURE.md §5 ("Validation messages come from the backend
// validator"); the departure is recorded in docs/contributing.md §8 item 8.

import type { ApiFailureResult } from '../session/api.ts';

export type ApiFailure =
  | { kind: 'unreachable' }
  | { kind: 'session-ended' }
  | { kind: 'launch-rejected' }
  | { kind: 'displaced' }
  | { kind: 'input-refused' }
  | { kind: 'relaunch-throttled' }
  | { kind: 'relaunch-exhausted' }
  | { kind: 'open-unavailable' }
  | { kind: 'wrong-address' }
  /** The application could not read its own request, or its own answer. */
  | { kind: 'defect'; code: string }
  | { kind: 'server-failed'; code: string };

type CodedKind = Exclude<ApiFailure['kind'], 'unreachable'>;

/** Every code the server can send, and the failure it means. */
export const FAILURE_BY_CODE: Readonly<Record<string, CodedKind>> = {
  session_required: 'session-ended',
  launch_token_invalid: 'launch-rejected',
  session_cookie_displaced: 'displaced',
  schedule_input_invalid: 'input-refused',
  relaunch_throttled: 'relaunch-throttled',
  relaunch_exhausted: 'relaunch-exhausted',
  open_unavailable: 'open-unavailable',
  misdirected_host: 'wrong-address',
  origin_forbidden: 'wrong-address',
  fetch_site_forbidden: 'wrong-address',
  invalid_json: 'defect',
  request_invalid: 'defect',
  not_found: 'defect',
  method_not_allowed: 'defect',
  payload_too_large: 'defect',
  unsupported_media_type: 'defect',
  headers_too_large: 'server-failed',
  request_timeout: 'server-failed',
  internal_error: 'server-failed',
};

const DEFECT_STATUSES: readonly number[] = [400, 404, 405, 413, 415, 422];
const WRONG_ADDRESS_STATUSES: readonly number[] = [403, 421];

function withCode(kind: CodedKind, code: string): ApiFailure {
  return kind === 'defect' || kind === 'server-failed' ? { kind, code } : { kind };
}

/** The failure a transport result means. Codes win; the status class is the fallback. */
export function failureFrom(result: ApiFailureResult): ApiFailure {
  if (result.kind === 'unreachable') {
    return { kind: 'unreachable' };
  }
  const known = result.code !== undefined && Object.hasOwn(FAILURE_BY_CODE, result.code) ? FAILURE_BY_CODE[result.code] : undefined;
  if (known !== undefined && result.code !== undefined) {
    return withCode(known, result.code);
  }
  if (result.kind === 'displaced') {
    return { kind: 'displaced' };
  }
  if (result.kind === 'unauthenticated') {
    return { kind: 'session-ended' };
  }
  const code = result.code ?? `status_${String(result.status)}`;
  if (WRONG_ADDRESS_STATUSES.includes(result.status)) {
    return { kind: 'wrong-address' };
  }
  if (result.status === 429) {
    return { kind: 'relaunch-throttled' };
  }
  return DEFECT_STATUSES.includes(result.status) ? { kind: 'defect', code } : { kind: 'server-failed', code };
}

/** A success body that did not have the expected shape: a defect of the build. */
export const MALFORMED_BODY: ApiFailure = { kind: 'defect', code: 'malformed_response' };

/** What a person reads. */
export function failureText(failure: ApiFailure): string {
  switch (failure.kind) {
    case 'unreachable':
      return 'The application on this computer is not answering. If you quit it, start it again from its launcher.';
    case 'session-ended':
      return 'This tab’s session has ended. Open the page again from the application.';
    case 'launch-rejected':
      return 'The launch link was already used or has expired. Open the page again from the application.';
    case 'displaced':
      return 'This browser’s session was displaced by another local site.';
    case 'input-refused':
      return 'The year, filing status or taxable income is outside what this build’s parameters cover.';
    case 'relaunch-throttled':
      return 'Re-opening was asked for too often. Wait a few seconds and try again.';
    case 'relaunch-exhausted':
      return 'This session cannot re-open the application again. Quit the application and start it again from its launcher.';
    case 'open-unavailable':
      return 'This build cannot re-open itself. Quit the application and start it again from its launcher.';
    case 'wrong-address':
      return 'This page was opened at an address the application does not answer to. Open it from the application.';
    case 'defect':
      return `The application could not read its own request. This is a defect; the code is ${failure.code}.`;
    case 'server-failed':
      return `The application could not complete the request (code ${failure.code}).`;
  }
}

/** Whether "Try again" can help. It is offered only where it can. */
export function canRetry(failure: ApiFailure): boolean {
  return failure.kind === 'unreachable' || failure.kind === 'server-failed';
}
