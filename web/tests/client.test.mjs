// The transport and the typed client. Every response is a `fakeResponse`, which
// behaves like a real `Response`: `json()` rejects for a CSV body and a body can
// be read once.

import assert from 'node:assert/strict';
import { test } from 'node:test';

import { createClient, hasShape } from '../src/api/client.ts';
import { apiPost, PROOF_HEADER } from '../src/session/api.ts';
import { errorResponse, fakeApiEnv, fakeResponse } from './support/fakes.mjs';
import * as golden from './support/golden.mjs';

const EXPORT = '/api/v1/tax/rate-schedule/export';
// SYNTHETIC inputs: the first grid point of docs/PLAN.md §4.1.
const INPUTS = { year: 2026, filingStatus: 'mfj', taxableIncome: 10000000 };

test('a text body is read with text() only; json() is never called', async () => {
  const csv = golden.rateScheduleCsvExport();
  const disposition = 'attachment; filename="rate-schedule.csv"';
  const api = fakeApiEnv(() => fakeResponse(200, csv, { 'Content-Disposition': disposition }));
  const result = await apiPost(api.env, EXPORT, { ...INPUTS, format: 'csv' }, { expect: 'text' });
  assert.deepEqual(result, { kind: 'ok', status: 200, text: csv, disposition });
  // A response without the header is still a success; the name then falls back (see rate-schedule tests).
  const bare = await apiPost(fakeApiEnv(() => fakeResponse(200, csv)).env, EXPORT, { ...INPUTS, format: 'csv' }, { expect: 'text' });
  assert.equal(bare.disposition, null);
  assert.deepEqual(api.responses[0].reads, { json: 0, text: 1 });
});

test('the same CSV answer read as JSON is refused, not a success', async () => {
  const api = fakeApiEnv(() => fakeResponse(200, golden.rateScheduleCsvExport()));
  assert.deepEqual(await apiPost(api.env, EXPORT, { ...INPUTS, format: 'csv' }), { kind: 'refused', status: 200 });
  assert.deepEqual(api.responses[0].reads, { json: 1, text: 0 });
});

test('a refusal carries the server code in both modes, and the body is read once', async () => {
  for (const options of [{ expect: 'text' }, { expect: 'json' }, undefined]) {
    const api = fakeApiEnv(() => errorResponse(422, 'schedule_input_invalid'));
    assert.deepEqual(await apiPost(api.env, EXPORT, {}, options), { kind: 'refused', status: 422, code: 'schedule_input_invalid' });
    assert.deepEqual(api.responses[0].reads, { json: 1, text: 0 });
  }
});

test('the two 429 codes are read apart; 401 and 409 keep their kinds and their codes', async () => {
  const cases = [
    [429, 'relaunch_throttled', { kind: 'refused', status: 429, code: 'relaunch_throttled' }],
    [429, 'relaunch_exhausted', { kind: 'refused', status: 429, code: 'relaunch_exhausted' }],
    [503, 'open_unavailable', { kind: 'refused', status: 503, code: 'open_unavailable' }],
    [401, 'session_required', { kind: 'unauthenticated', code: 'session_required' }],
    [409, 'session_cookie_displaced', { kind: 'displaced', code: 'session_cookie_displaced' }],
  ];
  for (const [status, code, expected] of cases) {
    const api = fakeApiEnv(() => errorResponse(status, code));
    assert.deepEqual(await apiPost(api.env, '/api/v1/session/relaunch'), expected);
  }
});

test('an error body that is not JSON, not an object or has a malformed code yields no code and no throw', async () => {
  for (const body of ['<html>', '[]', 'null', '{"code":7}', '{"code":"Has Spaces"}', '{"code":"<script>"}', `{"code":"${'a'.repeat(65)}"}`, '']) {
    const api = fakeApiEnv(() => fakeResponse(500, body));
    assert.deepEqual(await apiPost(api.env, '/api/v1/session/status'), { kind: 'refused', status: 500 }, body);
  }
});

test('a second read of one body rejects, as a real Response does', async () => {
  const response = fakeResponse(200, '{}');
  await response.json();
  await assert.rejects(() => response.text(), /already used/);
  await assert.rejects(() => fakeResponse(200, 'a,b').json());
});

test('the request is a same-origin POST with the proof and no ambient authority beyond it', async () => {
  const api = fakeApiEnv(() => fakeResponse(200, '{}'));
  await apiPost(api.env, '/api/v1/session/status');
  await apiPost(api.env, EXPORT, { ...INPUTS, format: 'json' }, { expect: 'text' });
  for (const { init } of api.calls) {
    assert.equal(init.method, 'POST');
    assert.equal(init.credentials, 'same-origin');
    assert.equal(init.mode, 'same-origin');
    assert.equal(init.cache, 'no-store');
    assert.equal(init.redirect, 'error');
    assert.equal(init.referrerPolicy, 'no-referrer');
    assert.equal(init.headers[PROOF_HEADER], 'cd'.repeat(32));
  }
  assert.equal(api.calls[0].init.headers['Content-Type'], undefined);
  assert.equal(api.calls[0].init.body, undefined);
  assert.equal(api.calls[1].init.headers['Content-Type'], 'application/json');
});

test('the client returns typed successes for the golden bodies', async () => {
  const bodies = {
    '/api/v1/session/status': golden.sessionStatus(),
    '/api/v1/assumptions/list': golden.assumptions(),
    '/api/v1/tax/rate-schedule': golden.rateSchedule(),
  };
  const api = fakeApiEnv((path) => fakeResponse(200, JSON.stringify(bodies[path])));
  const client = createClient(api.env, () => assert.fail('no session was lost'));
  assert.deepEqual(await client.status(), { ok: true, value: bodies['/api/v1/session/status'] });
  assert.deepEqual(await client.assumptions(), { ok: true, value: bodies['/api/v1/assumptions/list'] });
  assert.deepEqual(await client.rateSchedule(INPUTS), { ok: true, value: bodies['/api/v1/tax/rate-schedule'] });
  assert.deepEqual(api.calls[2].body, INPUTS);
});

test('a success body of the wrong shape is a defect, never rendered', async () => {
  for (const body of ['{}', '[]', 'null', '{"tax":"1"}', JSON.stringify({ ...golden.rateSchedule(), lines: 'x' })]) {
    const api = fakeApiEnv(() => fakeResponse(200, body));
    const client = createClient(api.env, () => {});
    assert.deepEqual(await client.rateSchedule(INPUTS), { ok: false, failure: { kind: 'defect', code: 'malformed_response' } }, body);
  }
  assert.equal(hasShape({ a: [] }, { a: 'array' }), true);
  assert.equal(hasShape({ a: null }, { a: 'object' }), false);
});

test('a 409 or a 401 on any screen call is reported to the shell; a relaunch 401 is not', async () => {
  const lost = [];
  const displaced = createClient(fakeApiEnv(() => errorResponse(409, 'session_cookie_displaced')).env, (loss) => lost.push(loss));
  assert.deepEqual(await displaced.assumptions(), { ok: false, failure: { kind: 'displaced' } });
  await displaced.exportRateSchedule(INPUTS, 'csv');
  const ended = createClient(fakeApiEnv(() => errorResponse(401, 'session_required')).env, (loss) => lost.push(loss));
  await ended.rateSchedule(INPUTS);
  assert.deepEqual(await ended.relaunch(), { ok: false, failure: { kind: 'session-ended' } });
  assert.deepEqual(lost, ['displaced', 'displaced', 'session-ended']);
});

test('the export posts the inputs plus the format and returns the text untouched', async () => {
  const json = golden.rateScheduleJsonExport();
  const disposition = 'attachment; filename="rate-schedule.json"';
  const api = fakeApiEnv(() => fakeResponse(200, json, { 'content-disposition': disposition }));
  const client = createClient(api.env, () => {});
  assert.deepEqual(await client.exportRateSchedule(INPUTS, 'json'), { ok: true, value: { text: json, disposition } });
  assert.equal(api.calls[0].input, EXPORT);
  assert.deepEqual(api.calls[0].body, { ...INPUTS, format: 'json' });
  assert.deepEqual(api.responses[0].reads, { json: 0, text: 1 });
});

test('relaunch reads 202 as success and an unreachable server as unreachable', async () => {
  const ok = createClient(fakeApiEnv(() => fakeResponse(202, '{}')).env, () => {});
  assert.deepEqual(await ok.relaunch(), { ok: true, value: {} });
  const down = createClient(fakeApiEnv(() => { throw new TypeError('network'); }).env, () => {});
  assert.deepEqual(await down.relaunch(), { ok: false, failure: { kind: 'unreachable' } });
});
