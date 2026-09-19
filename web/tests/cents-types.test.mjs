// `Cents` is opaque: arithmetic on money is a compile error. This runs the pinned
// `tsc` on a file that is expected to FAIL and asserts the diagnostics by line.

import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { test } from 'node:test';

const web = join(import.meta.dirname, '..');
const file = join('tests', 'types', 'cents-arith.ts');

test('arithmetic on Cents is a type error; comparison of two Cents is not (the lint closes that)', () => {
  const expected = new Map();
  for (const [index, line] of readFileSync(join(web, file), 'utf8').split('\n').entries()) {
    const match = /\/\/ expect: (TS[0-9]+|none)$/.exec(line);
    if (match !== null) {
      expected.set(index + 1, match[1]);
    }
  }
  assert.equal(expected.size, 11);

  const tsc = join(web, 'node_modules', 'typescript', 'bin', 'tsc');
  const run = spawnSync(
    process.execPath,
    [tsc, '--ignoreConfig', '--noEmit', '--strict', '--target', 'es2022', '--module', 'esnext', '--moduleResolution', 'bundler', '--allowImportingTsExtensions', '--pretty', 'false', file],
    { cwd: web, encoding: 'utf8' },
  );
  assert.notEqual(run.status, 0, 'the file must not compile');

  const actual = new Map();
  for (const [, line, code] of `${run.stdout}${run.stderr}`.matchAll(/cents-arith\.ts\(([0-9]+),[0-9]+\): error (TS[0-9]+)/g)) {
    const at = Number(line);
    actual.set(at, [...(actual.get(at) ?? []), code]);
  }
  for (const [line, code] of expected) {
    if (code === 'none') {
      assert.equal(actual.get(line), undefined, `line ${line} compiles: the type system leaves this hole`);
    } else {
      // `a - b` reports each operand: TS2362 (left) and TS2363 (right).
      assert.ok((actual.get(line) ?? []).includes(code), `line ${line}: expected ${code}, got ${actual.get(line)}\n${run.stdout}`);
    }
  }
  for (const line of actual.keys()) {
    assert.ok(expected.has(line) && expected.get(line) !== 'none', `unexpected diagnostic on line ${line}\n${run.stdout}`);
  }
});
