// The one way this front end talks to the server: a same-origin POST that carries
// the proof token in `X-PFP-Proof` (the `__Host-` cookie travels by itself and is
// not readable from script). SECURITY.md §7.1–§7.2.
//
// Every dependency on the browser is injected, so the module is testable under
// plain Node and nothing here can reach a storage API other than the one it is
// handed.

/** The single `sessionStorage` key this application ever writes. */
export const PROOF_KEY = 'pfp.proof';

/** The custom header carrying the port-scoped half of the session credential. */
export const PROOF_HEADER = 'X-PFP-Proof';

/** The part of `Storage` that is used. */
export interface ProofStorage {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
  removeItem(key: string): void;
}

/**
 * The part of a `Response` that is used. A body can be read once, as JSON or as
 * text; no path below reads one response twice.
 */
export interface ResponseLike {
  status: number;
  /** Same-origin, so every response header is readable. */
  headers: { get(name: string): string | null };
  json(): Promise<unknown>;
  text(): Promise<string>;
}

/** The part of `fetch` that is used. */
export type FetchLike = (
  input: string,
  init: {
    method: 'POST';
    headers: Record<string, string>;
    body?: string;
    credentials: 'same-origin';
    mode: 'same-origin';
    cache: 'no-store';
    redirect: 'error';
    referrerPolicy: 'strict-origin';
  },
) => Promise<ResponseLike>;

export interface ApiEnv {
  fetch: FetchLike;
  sessionStorage: ProofStorage;
}

/**
 * Every way a call can fail. `code` is the server's stable machine-readable code
 * when the refusal body carried a well-formed one, and absent otherwise; the
 * server's `message` is never read, so no server string can reach the screen.
 */
export type ApiFailureResult =
  /** 409: another loopback site displaced the cookie; recoverable by a re-open. */
  | { kind: 'displaced'; code?: string }
  /** 401: there is no session for this tab. */
  | { kind: 'unauthenticated'; code?: string }
  | { kind: 'refused'; status: number; code?: string }
  | { kind: 'unreachable' };

export type ApiResult = { kind: 'ok'; status: number; body: unknown } | ApiFailureResult;

/** The result of a call whose success body is a file, kept as the text the server sent. */
export type ApiTextResult =
  | { kind: 'ok'; status: number; text: string; /** The raw `Content-Disposition`, unvalidated. */ disposition: string | null }
  | ApiFailureResult;

export interface ApiPostOptions {
  withProof?: boolean;
  /**
   * How a 2xx body is read. `'text'` calls `response.text()` only and never
   * `json()`: a CSV body is not JSON, and an exported JSON file is saved as the
   * bytes the server sent, never parsed and re-serialised here.
   */
  expect?: 'json' | 'text';
}

/**
 * Forgets the proof. Called when the server says the session has ended (401), so
 * the one stored key never holds a credential that is known to be dead. NOT called
 * for a displaced cookie (409): there the proof is the half that survived, and the
 * proof-only `session/relaunch` recovery needs it.
 */
export function clearProof(env: Pick<ApiEnv, 'sessionStorage'>): void {
  env.sessionStorage.removeItem(PROOF_KEY);
}

const CODE_PATTERN = /^[a-z_]{1,64}$/;

/** The `code` of a refusal body, or `undefined` for anything that is not one. */
async function errorCode(response: ResponseLike): Promise<string | undefined> {
  let body: unknown;
  try {
    body = await response.json();
  } catch {
    return undefined;
  }
  if (typeof body !== 'object' || body === null || !('code' in body)) {
    return undefined;
  }
  const code = body.code;
  return typeof code === 'string' && CODE_PATTERN.test(code) ? code : undefined;
}

/**
 * POSTs to a path under `/api/v1`. The path is a literal chosen by the caller,
 * never assembled from a token: nothing secret may enter a URL.
 */
export async function apiPost(
  env: ApiEnv,
  path: `/api/v1/${string}`,
  body: unknown,
  options: ApiPostOptions & { expect: 'text' },
): Promise<ApiTextResult>;
export async function apiPost(
  env: ApiEnv,
  path: `/api/v1/${string}`,
  body?: unknown,
  options?: ApiPostOptions & { expect?: 'json' },
): Promise<ApiResult>;
export async function apiPost(
  env: ApiEnv,
  path: `/api/v1/${string}`,
  body?: unknown,
  options: ApiPostOptions = {},
): Promise<ApiResult | ApiTextResult> {
  const headers: Record<string, string> = {};
  if (options.withProof !== false) {
    const proof = env.sessionStorage.getItem(PROOF_KEY);
    if (proof !== null) {
      headers[PROOF_HEADER] = proof;
    }
  }
  const init: Parameters<FetchLike>[1] = {
    method: 'POST',
    headers,
    credentials: 'same-origin',
    mode: 'same-origin',
    cache: 'no-store',
    redirect: 'error',
    // Not `no-referrer`, although the document's own policy is. Fetch's "append
    // a request Origin header" step serialises `Origin` as `null` for a
    // non-CORS-mode request that is not GET/HEAD under `no-referrer`, and
    // Firefox implements that step as written: the server's exact-`Origin` rule
    // (SECURITY.md §7.2) then refuses our own POST as cross-origin. Under
    // `strict-origin` the step nulls `Origin` only on an https-to-http downgrade,
    // which cannot happen on one loopback origin, and the `Referer` it allows is
    // exactly `https://127.0.0.1:<port>/` - our own origin, nothing more. The
    // policy is set per request because the document's `no-referrer` would
    // otherwise apply (client tests, "cannot be `null`").
    referrerPolicy: 'strict-origin',
  };
  if (body !== undefined) {
    headers['Content-Type'] = 'application/json';
    init.body = JSON.stringify(body);
  }

  let response: ResponseLike;
  try {
    response = await env.fetch(path, init);
  } catch {
    return { kind: 'unreachable' };
  }
  const { status } = response;
  if (status < 200 || status > 299) {
    // The server's refusals are always JSON, whatever the success body would be.
    const code = await errorCode(response);
    const coded = code === undefined ? {} : { code };
    if (status === 409) {
      return { kind: 'displaced', ...coded };
    }
    if (status === 401) {
      return { kind: 'unauthenticated', ...coded };
    }
    return { kind: 'refused', status, ...coded };
  }
  try {
    return options.expect === 'text'
      ? { kind: 'ok', status, text: await response.text(), disposition: response.headers.get('Content-Disposition') }
      : { kind: 'ok', status, body: await response.json() };
  } catch {
    return { kind: 'refused', status };
  }
}
