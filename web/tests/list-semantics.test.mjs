// Source rule: WebKit drops the implicit `list` role from a <ul> styled with
// `list-style: none`, so VoiceOver announces neither the list nor its item
// count. Every <ul> in the application therefore states role="list" explicitly.
// The one exception is the primary navigation list, which sits inside <nav> -
// the affordance WebKit keeps list semantics for.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join, relative } from 'node:path';

const SRC = join(import.meta.dirname, '..', 'src');
const EXEMPT = new Set(['components/PrimaryNav.tsx']);

function tsxFiles(dir) {
  return readdirSync(dir).flatMap((name) => {
    const path = join(dir, name);
    if (statSync(path).isDirectory()) return tsxFiles(path);
    return name.endsWith('.tsx') ? [path] : [];
  });
}

test('every <ul> outside the primary navigation states role="list"', () => {
  const offenders = [];
  let seen = 0;
  for (const file of tsxFiles(SRC)) {
    const rel = relative(SRC, file).split('\\').join('/');
    if (EXEMPT.has(rel)) continue;
    for (const tag of readFileSync(file, 'utf8').match(/<ul\b[^>]*>/g) ?? []) {
      seen += 1;
      if (!/\brole="list"/.test(tag)) offenders.push(`${rel}: ${tag}`);
    }
  }
  assert.ok(seen >= 4, `expected the known lists to be found, saw ${seen}`);
  assert.deepEqual(offenders, []);
});
