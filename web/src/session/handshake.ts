// Session establishment (SECURITY.md §7.1):
//
//   launcher -> browser : https://127.0.0.1:<port>/#t=<launch token>   (fragment)
//   browser  -> server  : POST /api/v1/session/bootstrap { token }
//   server   -> browser : Set-Cookie: __Host-pfp=...; body { proof }
//   browser             : sessionStorage["pfp.proof"] = proof; fragment cleared
//
// Rules this module keeps, each of which has a test:
//   * the launch token is read from the fragment only and never enters a URL, a
//     query string, a log or any storage;
//   * the fragment is cleared with `history.replaceState` BEFORE the request is
//     sent, so the token leaves the address bar whatever the server answers;
//   * the only storage write is `sessionStorage["pfp.proof"]`;
//   * without a launch token and without a stored proof, no request is made.

import { apiPost, PROOF_KEY } from './api.ts';
import type { ApiEnv } from './api.ts';

export type SessionState =
  /** The handshake succeeded, or this tab's existing session is still live. */
  | { kind: 'connected' }
  /** Opened without a launch token (typed address, bookmark, restored tab). */
  | { kind: 'no-token' }
  /**
   * The server refused to start a session. `code` is its stable error code when
   * it sent one (`launch_token_invalid`: wrong, expired or already used; an
   * admission code such as `fetch_site_forbidden`: the request itself was refused
   * and the launch token was never checked).
   */
  | { kind: 'rejected'; code?: string }
  /** This tab had a session and the server no longer honours it (401 mid-use). */
  | { kind: 'session-ended' }
  /** Another local site displaced this browser's session cookie (409). */
  | { kind: 'displaced' }
  /** The server did not answer. */
  | { kind: 'unreachable' };

export interface HandshakeEnv extends ApiEnv {
  location: { hash: string; pathname: string; search: string };
  history: { replaceState(data: unknown, unused: string, url: string): void };
}

/** A launch token is 32 bytes as lower-case hex. Anything else is not sent. */
const TOKEN_PATTERN = /^[0-9a-f]{64}$/;

/** `#t=<token>` and nothing else; the token never comes from the query string. */
export function launchTokenFromFragment(hash: string): string | null {
  if (!hash.startsWith('#t=')) {
    return null;
  }
  const token = hash.slice(3);
  return TOKEN_PATTERN.test(token) ? token : null;
}

function proofFrom(body: unknown): string | null {
  if (typeof body !== 'object' || body === null || !('proof' in body)) {
    return null;
  }
  const proof = body.proof;
  return typeof proof === 'string' && TOKEN_PATTERN.test(proof) ? proof : null;
}

export async function bootstrapSession(env: HandshakeEnv): Promise<SessionState> {
  const token = launchTokenFromFragment(env.location.hash);

  // Clear the fragment first, whatever it held.
  if (env.location.hash !== '') {
    env.history.replaceState(null, '', env.location.pathname + env.location.search);
  }

  if (token !== null) {
    const result = await apiPost(env, '/api/v1/session/bootstrap', { token }, { withProof: false });
    if (result.kind === 'ok') {
      const proof = proofFrom(result.body);
      if (proof === null) {
        return { kind: 'rejected' };
      }
      env.sessionStorage.setItem(PROOF_KEY, proof);
      return { kind: 'connected' };
    }
    if (result.kind === 'unreachable') {
      return { kind: 'unreachable' };
    }
    const code = 'code' in result ? result.code : undefined;
    return code === undefined ? { kind: 'rejected' } : { kind: 'rejected', code };
  }

  // A reload of a tab that already holds a proof: ask whether the session lives.
  if (env.sessionStorage.getItem(PROOF_KEY) === null) {
    return { kind: 'no-token' };
  }
  const status = await apiPost(env, '/api/v1/session/status');
  switch (status.kind) {
    case 'ok':
      return { kind: 'connected' };
    case 'displaced':
      return { kind: 'displaced' };
    case 'unreachable':
      return { kind: 'unreachable' };
    case 'unauthenticated':
    case 'refused':
      env.sessionStorage.removeItem(PROOF_KEY);
      return { kind: 'no-token' };
  }
}

/** One sentence per state: the polite region says it when connected, the session notice otherwise. */
export function describeSession(state: SessionState): string {
  switch (state.kind) {
    case 'connected':
      return 'Connected to the application on this computer.';
    case 'no-token':
      return 'Not connected. Open this page from the application, not from a typed address or a bookmark.';
    case 'rejected':
      return describeRejection(state.code);
    case 'session-ended':
      return 'Not connected. This tab’s session has ended; open the page again from the application.';
    case 'displaced':
      return 'Not connected. This browser’s session was displaced by another local site; re-open from the application.';
    case 'unreachable':
      return 'Not connected. The application on this computer is not answering.';
  }
}

/** Admission refusals: the request was turned away before the launch token was read. */
const ADMISSION_CODES = new Set(['origin_forbidden', 'fetch_site_forbidden', 'misdirected_host']);

function describeRejection(code: string | undefined): string {
  if (code === undefined || code === 'launch_token_invalid') {
    return 'Not connected. The launch link was already used or has expired; open the page again from the application.';
  }
  if (ADMISSION_CODES.has(code)) {
    return `Not connected. This browser's request to start a session was refused by the application's cross-site protection (code ${code}); the launch link was not used. Open the page again from the application.`;
  }
  return `Not connected. The application refused to start a session (code ${code}); open the page again from the application.`;
}
