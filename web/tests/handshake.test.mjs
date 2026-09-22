// Session-handshake tests (test id S-08, the client half: the launch token leaves
// the URL before any request, is spent once, and the proof token is the only thing
// kept). The storage lint, S-05's static half, is no-storage.test.mjs.
//
// Plain Node: `node --test` imports the TypeScript module directly (Node strips the types), and every
// browser object is a recording fake, so no browser and no network is involved.
//
// Token values here are synthetic patterns built at run time, not secrets.

import assert from 'node:assert/strict';
import { test } from 'node:test';

import { bootstrapSession, describeSession, launchTokenFromFragment } from '../src/session/handshake.ts';
import { apiPost, PROOF_HEADER, PROOF_KEY } from '../src/session/api.ts';
import { fakeResponse } from './support/fakes.mjs';

const TOKEN = 'ab'.repeat(32);
const PROOF = 'cd'.repeat(32);

function fakeEnv({ hash = '', search = '', stored = {}, respond }) {
  const calls = { fetch: [], replaceState: [], storage: [] };
  const store = new Map(Object.entries(stored));
  const env = {
    location: { hash, pathname: '/', search },
    history: {
      replaceState(data, unused, url) {
        calls.replaceState.push({ data, unused, url, fetchesBefore: calls.fetch.length });
      },
    },
    sessionStorage: {
      getItem: (key) => (store.has(key) ? store.get(key) : null),
      setItem: (key, value) => {
        calls.storage.push(['set', key]);
        store.set(key, value);
      },
      removeItem: (key) => {
        calls.storage.push(['remove', key]);
        store.delete(key);
      },
    },
    fetch: async (input, init) => {
      calls.fetch.push({ input, init });
      if (respond === undefined) {
        throw new TypeError('network');
      }
      // One fake for every test: it behaves like a real `Response` (tests/support/fakes.mjs).
      const { status, body } = respond(input, init);
      return fakeResponse(status, JSON.stringify(body));
    },
  };
  return { env, calls, store };
}

test('fragment token is exchanged, proof stored, fragment cleared first', async () => {
  const { env, calls, store } = fakeEnv({
    hash: `#t=${TOKEN}`,
    respond: () => ({ status: 200, body: { proof: PROOF } }),
  });
  const state = await bootstrapSession(env);
  assert.deepEqual(state, { kind: 'connected' });

  assert.equal(calls.fetch.length, 1);
  const { input, init } = calls.fetch[0];
  assert.equal(input, '/api/v1/session/bootstrap');
  assert.equal(init.method, 'POST');
  assert.deepEqual(JSON.parse(init.body), { token: TOKEN });
  assert.equal(init.headers['Content-Type'], 'application/json');
  assert.equal(init.headers[PROOF_HEADER], undefined);
  assert.equal(init.credentials, 'same-origin');
  assert.equal(init.mode, 'same-origin');
  assert.equal(init.redirect, 'error');
  assert.equal(init.referrerPolicy, 'no-referrer');

  // The fragment was cleared before the request left, to a URL with no fragment.
  assert.deepEqual(calls.replaceState, [{ data: null, unused: '', url: '/', fetchesBefore: 0 }]);
  // The only storage write is the proof.
  assert.deepEqual(calls.storage, [['set', PROOF_KEY]]);
  assert.deepEqual([...store.entries()], [[PROOF_KEY, PROOF]]);
});

test('the token never enters a URL or a query string', async () => {
  const { env, calls } = fakeEnv({
    hash: `#t=${TOKEN}`,
    search: '?x=1',
    respond: () => ({ status: 200, body: { proof: PROOF } }),
  });
  await bootstrapSession(env);
  for (const { input } of calls.fetch) {
    assert.ok(!input.includes(TOKEN));
    assert.ok(!input.includes('?'));
  }
  assert.equal(calls.replaceState[0].url, '/?x=1');
  assert.ok(!calls.replaceState[0].url.includes(TOKEN));
});

test('a token in the query string is ignored: no request, no storage', async () => {
  const { env, calls } = fakeEnv({ search: `?t=${TOKEN}`, respond: () => ({ status: 200, body: {} }) });
  assert.deepEqual(await bootstrapSession(env), { kind: 'no-token' });
  assert.equal(calls.fetch.length, 0);
  assert.deepEqual(calls.storage, []);
  assert.deepEqual(calls.replaceState, []);
});

test('no token and no proof: no request at all', async () => {
  const { env, calls } = fakeEnv({ respond: () => ({ status: 200, body: {} }) });
  assert.deepEqual(await bootstrapSession(env), { kind: 'no-token' });
  assert.equal(calls.fetch.length, 0);
});

test('a malformed fragment is cleared and never sent', async () => {
  for (const hash of ['#t=', '#t=xyz', `#t=${TOKEN}0`, `#t=${TOKEN.toUpperCase()}`, `#x=${TOKEN}`, `#t=${TOKEN}&u=1`]) {
    const { env, calls } = fakeEnv({ hash, respond: () => ({ status: 200, body: {} }) });
    assert.deepEqual(await bootstrapSession(env), { kind: 'no-token' }, hash);
    assert.equal(calls.fetch.length, 0, hash);
    assert.equal(calls.replaceState.length, 1, hash);
    assert.equal(launchTokenFromFragment(hash), null);
  }
});

test('a refused token stores nothing and still clears the fragment', async () => {
  const { env, calls } = fakeEnv({
    hash: `#t=${TOKEN}`,
    respond: () => ({ status: 401, body: { code: 'launch_token_invalid', message: 'x' } }),
  });
  assert.deepEqual(await bootstrapSession(env), { kind: 'rejected' });
  assert.deepEqual(calls.storage, []);
  assert.equal(calls.replaceState.length, 1);
});

test('a 200 without a well-formed proof is rejected and stores nothing', async () => {
  for (const body of [{}, { proof: 7 }, { proof: 'short' }, null, 'text']) {
    const { env, calls } = fakeEnv({ hash: `#t=${TOKEN}`, respond: () => ({ status: 200, body }) });
    assert.deepEqual(await bootstrapSession(env), { kind: 'rejected' });
    assert.deepEqual(calls.storage, []);
  }
});

test('an unreachable server is reported, not thrown', async () => {
  const { env, calls } = fakeEnv({ hash: `#t=${TOKEN}` });
  assert.deepEqual(await bootstrapSession(env), { kind: 'unreachable' });
  assert.deepEqual(calls.storage, []);
});

test('reload with a stored proof asks for status and sends the proof header', async () => {
  const { env, calls } = fakeEnv({
    stored: { [PROOF_KEY]: PROOF },
    respond: () => ({ status: 200, body: { apiVersion: 'v1' } }),
  });
  assert.deepEqual(await bootstrapSession(env), { kind: 'connected' });
  assert.equal(calls.fetch[0].input, '/api/v1/session/status');
  assert.equal(calls.fetch[0].init.headers[PROOF_HEADER], PROOF);
  assert.equal(calls.fetch[0].init.body, undefined);
  assert.deepEqual(calls.storage, []);
});

test('409 on reload is the displaced state and keeps the proof for the recovery path', async () => {
  const { env, store } = fakeEnv({
    stored: { [PROOF_KEY]: PROOF },
    respond: () => ({ status: 409, body: { code: 'session_cookie_displaced', message: 'x' } }),
  });
  assert.deepEqual(await bootstrapSession(env), { kind: 'displaced' });
  assert.equal(store.get(PROOF_KEY), PROOF);
});

test('401 on reload drops the dead proof', async () => {
  const { env, calls, store } = fakeEnv({
    stored: { [PROOF_KEY]: PROOF },
    respond: () => ({ status: 401, body: { code: 'session_required', message: 'x' } }),
  });
  assert.deepEqual(await bootstrapSession(env), { kind: 'no-token' });
  assert.deepEqual(calls.storage, [['remove', PROOF_KEY]]);
  assert.equal(store.size, 0);
});

test('apiPost maps statuses', async () => {
  const cases = [
    [200, 'ok'],
    [202, 'ok'],
    [401, 'unauthenticated'],
    [409, 'displaced'],
    [403, 'refused'],
    [503, 'refused'],
  ];
  for (const [status, kind] of cases) {
    const { env } = fakeEnv({ respond: () => ({ status, body: {} }) });
    assert.equal((await apiPost(env, '/api/v1/session/status')).kind, kind);
  }
});

test('every state has a message and only one says connected', () => {
  const kinds = ['connected', 'no-token', 'rejected', 'displaced', 'unreachable'];
  const messages = kinds.map((kind) => describeSession({ kind }));
  assert.equal(new Set(messages).size, kinds.length);
  assert.ok(messages[0].startsWith('Connected'));
  for (const message of messages.slice(1)) {
    assert.ok(message.startsWith('Not connected'));
  }
});
