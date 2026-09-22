// Every error code the API names has the UI's own words. The server's `message`
// is never rendered, so a server string can never put wording on a screen.

import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { test } from 'node:test';

import { canRetry, FAILURE_BY_CODE, failureFrom, failureText } from '../src/api/errors.ts';
import { openapi } from './support/golden.mjs';

function codesInOpenapi() {
  const codes = new Set();
  for (const item of Object.values(openapi().paths)) {
    for (const [status, response] of Object.entries(item.post.responses)) {
      if (!status.startsWith('2')) {
        for (const [, code] of response.description.matchAll(/`([a-z_]+)`/g)) {
          codes.add(code);
        }
      }
    }
  }
  return [...codes].sort();
}

test('every code named in the OpenAPI responses has a taxonomy entry', () => {
  const codes = codesInOpenapi();
  assert.ok(codes.length >= 8, 'the extraction found the codes');
  for (const code of codes) {
    assert.ok(Object.hasOwn(FAILURE_BY_CODE, code), `no entry for ${code}`);
  }
});

test('every code the server defines has a taxonomy entry', () => {
  const source = readFileSync(join(import.meta.dirname, '..', '..', 'crates', 'pfp-server', 'src', 'error.rs'), 'utf8');
  const codes = [...source.matchAll(/StatusCode::[A-Z_]+,\s*"([a-z_]+)",/g)].map((m) => m[1]);
  assert.ok(codes.length >= 15);
  for (const code of codes) {
    assert.ok(Object.hasOwn(FAILURE_BY_CODE, code), `no entry for ${code}`);
  }
});

test('codes map to the documented failures', () => {
  const kind = (result) => failureFrom(result).kind;
  assert.equal(kind({ kind: 'unreachable' }), 'unreachable');
  assert.equal(kind({ kind: 'unauthenticated', code: 'session_required' }), 'session-ended');
  assert.equal(kind({ kind: 'unauthenticated', code: 'launch_token_invalid' }), 'launch-rejected');
  assert.equal(kind({ kind: 'unauthenticated' }), 'session-ended');
  assert.equal(kind({ kind: 'displaced' }), 'displaced');
  assert.equal(kind({ kind: 'refused', status: 422, code: 'schedule_input_invalid' }), 'input-refused');
  assert.equal(kind({ kind: 'refused', status: 429, code: 'relaunch_throttled' }), 'relaunch-throttled');
  assert.equal(kind({ kind: 'refused', status: 429, code: 'relaunch_exhausted' }), 'relaunch-exhausted');
  assert.equal(kind({ kind: 'refused', status: 503, code: 'open_unavailable' }), 'open-unavailable');
  assert.equal(kind({ kind: 'refused', status: 421, code: 'misdirected_host' }), 'wrong-address');
  assert.equal(kind({ kind: 'refused', status: 403 }), 'wrong-address');
  assert.deepEqual(failureFrom({ kind: 'refused', status: 422, code: 'request_invalid' }), { kind: 'defect', code: 'request_invalid' });
  assert.deepEqual(failureFrom({ kind: 'refused', status: 500, code: 'internal_error' }), { kind: 'server-failed', code: 'internal_error' });
});

test('an unknown code falls to its status class and is shown as a code, never as server text', () => {
  assert.deepEqual(failureFrom({ kind: 'refused', status: 400, code: 'brand_new_code' }), { kind: 'defect', code: 'brand_new_code' });
  assert.deepEqual(failureFrom({ kind: 'refused', status: 502 }), { kind: 'server-failed', code: 'status_502' });
  assert.deepEqual(failureFrom({ kind: 'refused', status: 200 }), { kind: 'server-failed', code: 'status_200' });
  assert.match(failureText({ kind: 'defect', code: 'brand_new_code' }), /brand_new_code/);
});

test('every failure has a sentence, and retry is offered only where it can help', () => {
  const kinds = ['unreachable', 'session-ended', 'launch-rejected', 'displaced', 'input-refused', 'relaunch-throttled', 'relaunch-exhausted', 'open-unavailable', 'wrong-address'];
  for (const kind of kinds) {
    assert.ok(failureText({ kind }).length > 20, kind);
  }
  assert.deepEqual(
    [...kinds.map((kind) => ({ kind })), { kind: 'defect', code: 'x' }, { kind: 'server-failed', code: 'x' }].filter(canRetry).map((f) => f.kind),
    ['unreachable', 'server-failed'],
  );
});
