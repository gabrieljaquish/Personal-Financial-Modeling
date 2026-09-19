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
    referrerPolicy: 'no-referrer';
  },
) => Promise<{ status: number; json(): Promise<unknown> }>;

export interface ApiEnv {
  fetch: FetchLike;
  sessionStorage: ProofStorage;
}

export type ApiResult =
  | { kind: 'ok'; status: number; body: unknown }
  /** 409: another loopback site displaced the cookie; recoverable by a re-open. */
  | { kind: 'displaced' }
  /** 401: there is no session for this tab. */
  | { kind: 'unauthenticated' }
  | { kind: 'refused'; status: number }
  | { kind: 'unreachable' };

/**
 * POSTs to a path under `/api/v1`. The path is a literal chosen by the caller,
 * never assembled from a token: nothing secret may enter a URL.
 */
export async function apiPost(
  env: ApiEnv,
  path: `/api/v1/${string}`,
  body?: unknown,
  options: { withProof: boolean } = { withProof: true },
): Promise<ApiResult> {
  const headers: Record<string, string> = {};
  if (options.withProof) {
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
    referrerPolicy: 'no-referrer',
  };
  if (body !== undefined) {
    headers['Content-Type'] = 'application/json';
    init.body = JSON.stringify(body);
  }

  let response: Awaited<ReturnType<FetchLike>>;
  try {
    response = await env.fetch(path, init);
  } catch {
    return { kind: 'unreachable' };
  }
  if (response.status === 409) {
    return { kind: 'displaced' };
  }
  if (response.status === 401) {
    return { kind: 'unauthenticated' };
  }
  if (response.status < 200 || response.status > 299) {
    return { kind: 'refused', status: response.status };
  }
  try {
    return { kind: 'ok', status: response.status, body: await response.json() };
  } catch {
    return { kind: 'refused', status: response.status };
  }
}
