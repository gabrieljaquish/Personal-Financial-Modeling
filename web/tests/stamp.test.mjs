// The build stamp `npm run build` writes must be the hash `pfp-server`'s build
// script recomputes, or every release build fails with "the front end is stale".
// The Rust side (xtask/src/build_web.rs) asserts the same known answer over the
// same three synthetic files, so neither implementation can move alone.

import assert from 'node:assert/strict';
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { test } from 'node:test';

import { inputHash, inputPaths, STAMP_PATH, writeStamp } from '../scripts/write-stamp.mjs';

// Shared with `input_hash_known_answer` in xtask/src/build_web.rs.
const KNOWN_ANSWER = '8c94bd698a53be52353f448cbd178dd29732f99925abb856c4e101e37c85a6c7';

function fixture() {
  const dir = mkdtempSync(join(tmpdir(), 'pfp-web-stamp-'));
  mkdirSync(join(dir, 'src', 'session'), { recursive: true });
  writeFileSync(join(dir, 'index.html'), 'a');
  writeFileSync(join(dir, 'src', 'main.tsx'), 'b');
  writeFileSync(join(dir, 'src', 'session', 'x.ts'), 'c');
  return dir;
}

test('the input hash is the one the Rust build script computes', () => {
  const dir = fixture();
  try {
    assert.deepEqual(inputPaths(dir), ['index.html', 'src/main.tsx', 'src/session/x.ts']);
    assert.equal(inputHash(dir), KNOWN_ANSWER);
    writeFileSync(join(dir, 'src', 'session', 'x.ts'), 'd');
    assert.notEqual(inputHash(dir), KNOWN_ANSWER);
  } finally {
    rmSync(dir, { recursive: true });
  }
});

test('the bundle is not an input, and the stamp lands inside it', () => {
  const dir = fixture();
  try {
    assert.throws(() => writeStamp(dir), /no bundle to stamp/);
    mkdirSync(join(dir, 'dist'));
    writeFileSync(join(dir, 'dist', 'index.html'), 'z');
    assert.equal(writeStamp(dir), KNOWN_ANSWER);
    assert.equal(readFileSync(join(dir, STAMP_PATH), 'utf8'), `${KNOWN_ANSWER}\n`);
    assert.equal(inputHash(dir), KNOWN_ANSWER);
  } finally {
    rmSync(dir, { recursive: true });
  }
});

test('npm run build ends by writing the stamp', () => {
  const manifest = JSON.parse(readFileSync(join(import.meta.dirname, '..', 'package.json'), 'utf8'));
  assert.match(manifest.scripts.build, /vite build && node scripts\/write-stamp\.mjs$/);
});
