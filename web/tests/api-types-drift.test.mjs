// ADR-017: "a DTO change breaks the front-end build, not a user session". The
// committed API types must be exactly what the generator makes of the server's
// OpenAPI snapshot; and the generator must fail closed.

import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';

import { generate, generateFromSnapshot, neutralise, OUTPUT_PATH, SNAPSHOT_PATH, snapshotBody } from '../scripts/gen-api-types.mjs';

const snapshot = readFileSync(SNAPSHOT_PATH, 'utf8');
const committed = readFileSync(OUTPUT_PATH, 'utf8');
const document = () => JSON.parse(snapshotBody(snapshot));

test('the committed types are byte-identical to a fresh generation (run `npm run gen:api`)', () => {
  assert.equal(generateFromSnapshot(snapshot), committed);
});

test('generation is deterministic, LF-only and newline-terminated', () => {
  assert.equal(generateFromSnapshot(snapshot), generateFromSnapshot(snapshot));
  assert.ok(!committed.includes('\r'));
  assert.ok(committed.endsWith('\n') && !committed.endsWith('\n\n'));
});

test('the drift test proves itself: a renamed property changes the output', () => {
  const doc = document();
  const { properties } = doc.components.schemas.LineDto;
  properties.amount = properties.value;
  delete properties.value;
  doc.components.schemas.LineDto.required = doc.components.schemas.LineDto.required.map((k) => (k === 'value' ? 'amount' : k));
  const mutated = generate(doc);
  assert.notEqual(mutated, committed);
  assert.ok(mutated.includes('readonly amount: Cents;'));
});

test('money is opaque, enums come with their values, every operation is typed', () => {
  assert.ok(committed.includes('export type Cents = { readonly __cents: unique symbol };'));
  assert.ok(!/Cents = number/.test(committed));
  assert.ok(committed.includes("export const FilingStatusDtoValues = ['single', 'mfj', 'mfs', 'hoh', 'qss'] as const;"));
  assert.ok(committed.includes("export const ExportFormatDtoValues = ['csv', 'json'] as const;"));
  const doc = document();
  for (const path of Object.keys(doc.paths)) {
    assert.ok(committed.includes(`readonly '${path}': {`), path);
  }
  assert.equal(Object.keys(doc.paths).length, 7);
  assert.match(committed, /'\/api\/v1\/validation\/report': \{\n {4}readonly request: undefined;\n {4}readonly ok: ValidationReportResponse;/);
  assert.ok(committed.includes("export const SectionStateValues = ['not-yet-introduced', 'empty', 'partial', 'present'] as const;"));
  assert.ok(committed.includes("export const ReportStateDtoValues = ['generated', 'not-generated'] as const;"));
  assert.match(committed, /'\/api\/v1\/tax\/rate-schedule\/export': \{\n {4}readonly request: RateScheduleExportRequest;\n {4}readonly ok: string;\n {4}readonly okStatus: 200;\n {4}readonly expect: 'text';/);
  assert.match(committed, /'\/api\/v1\/session\/relaunch': \{\n {4}readonly request: undefined;\n {4}readonly ok: Accepted;\n {4}readonly okStatus: 202;\n {4}readonly expect: 'json';\n {4}readonly errorStatuses: 401 \| 429 \| 503;/);
});

test('an integer bound is kept as a note, never dropped', () => {
  assert.match(committed, /\* Minimum: 0\.\n {3}\*\/\n {2}readonly fileCount: number;/);
});

test('optional properties admit absence and null; required ones do not', () => {
  assert.ok(committed.includes('readonly lockedId?: string | null;'));
  assert.ok(committed.includes('readonly verified: boolean;'));
  assert.ok(committed.includes('readonly inputs?: readonly string[] | null;'));
});

test('the generator fails closed on a keyword it does not know', () => {
  const cases = [
    (doc) => (doc.components.schemas.LineDto.properties.label.pattern = '^x$'),
    (doc) => (doc.components.schemas.LineDto.allOf = []),
    (doc) => (doc.components.schemas.RatioDto.properties.num.multipleOf = 2),
    (doc) => (doc.components.schemas.LineDto.properties.value = { type: 'integer', 'x-money': true }),
    (doc) => (doc.components.schemas.LineDto.properties.label.type = 'number'),
    (doc) => (doc.paths['/api/v1/session/status'].get = {}),
    (doc) => (doc.paths['/api/v1/session/status'].post.parameters = []),
    (doc) => (doc.paths['/api/v1/session/status'].post.responses['401'].content['application/json'].schema = { type: 'string' }),
  ];
  for (const mutate of cases) {
    const doc = document();
    mutate(doc);
    assert.throws(() => generate(doc), /unsupported|money|refusal/, String(mutate));
  }
});

test('comment text cannot carry a URL scheme or close its comment', () => {
  const doc = document();
  const nasty = 'See https://example.invalid/a and HTTP://x and */ done';
  doc.components.schemas.LineDto.description = nasty;
  doc.components.schemas.LineDto.properties.label.description = nasty;
  doc.paths['/api/v1/session/status'].post.summary = nasty;
  const out = generate(doc);
  assert.ok(!/https?:/i.test(out));
  assert.ok(out.includes('See example.invalid/a and x and * / done'));
  // Every comment that opens closes exactly once, and nothing follows a terminator on its line.
  assert.equal(out.match(/\/\*\*/g).length, out.match(/\*\//g).length);
  assert.equal(neutralise('hTTps://A */'), 'A * /');
});

test('the committed file names no URL scheme at all', () => {
  assert.ok(!/https?:/i.test(committed));
});
