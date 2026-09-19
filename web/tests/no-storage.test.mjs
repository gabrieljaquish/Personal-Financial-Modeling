// Source lint over web/src and web/index.html (SECURITY.md §7.4, test id S-05): the
// cheap first line in front of the browser assertion that arrives with the e2e
// suite. Comments are stripped first, so prose may name what code may not.

import assert from 'node:assert/strict';
import { readdirSync, readFileSync } from 'node:fs';
import { join, relative, sep } from 'node:path';
import { test } from 'node:test';

const web = join(import.meta.dirname, '..');
const src = join(web, 'src');

function walk(dir) {
  return readdirSync(dir, { withFileTypes: true }).flatMap((entry) =>
    entry.isDirectory() ? walk(join(dir, entry.name)) : [join(dir, entry.name)],
  );
}

function code(text) {
  return text
    .replace(/\/\*[\s\S]*?\*\//g, '')
    .split('\n')
    .filter((line) => !line.trim().startsWith('//'))
    .join('\n');
}

const sources = walk(src)
  .filter((file) => /\.(ts|tsx|css)$/.test(file))
  .map((file) => ({ file: relative(web, file).split(sep).join('/'), text: code(readFileSync(file, 'utf8')) }));

test('there are sources to lint', () => {
  assert.ok(sources.length >= 5);
  assert.ok(sources.some(({ file }) => file === 'src/session/handshake.ts'));
});

test('no storage API, worker or socket is named anywhere in web/src', () => {
  const banned = [
    'localStorage',
    'indexedDB',
    'caches',
    'serviceWorker',
    'Worker(',
    'WebSocket',
    'EventSource',
    'sendBeacon',
    'document.cookie',
    'XMLHttpRequest',
    'WebAssembly',
  ];
  for (const { file, text } of sources) {
    for (const word of banned) {
      assert.ok(!text.includes(word), `${file} names ${word}`);
    }
  }
});

test('sessionStorage is named only by the session module and the entry point that injects it', () => {
  const allowed = new Set(['src/session/api.ts', 'src/session/handshake.ts', 'src/main.tsx']);
  for (const { file, text } of sources) {
    if (!allowed.has(file)) {
      assert.ok(!text.includes('sessionStorage'), `${file} names sessionStorage`);
    }
  }
});

test('exactly one storage key exists and it is the proof', () => {
  const writes = sources.flatMap(({ file, text }) =>
    [...text.matchAll(/\.setItem\(([^,]+),/g)].map((match) => `${file}:${match[1].trim()}`),
  );
  assert.deepEqual(writes, ['src/session/handshake.ts:PROOF_KEY']);
});

test('no inline style, no raw HTML injection, no eval', () => {
  for (const { file, text } of sources) {
    for (const word of ['style={', 'dangerouslySetInnerHTML', 'eval(', 'new Function', 'innerHTML']) {
      assert.ok(!text.includes(word), `${file} uses ${word}`);
    }
  }
});

test('no absolute URL: nothing can name a third-party origin', () => {
  const documentText = readFileSync(join(web, 'index.html'), 'utf8');
  for (const { file, text } of [...sources, { file: 'index.html', text: documentText }]) {
    assert.ok(!/https?:\/\//.test(text), `${file} contains an absolute URL`);
    assert.ok(!/(src|href)=["']\/\//.test(text), `${file} contains a protocol-relative URL`);
  }
});

test('index.html has no inline script, style or event handler', () => {
  const text = readFileSync(join(web, 'index.html'), 'utf8');
  assert.ok(!/<style/i.test(text));
  assert.ok(!/\sstyle=/i.test(text));
  assert.ok(!/\son[a-z]+=/i.test(text));
  for (const [, attrs] of text.matchAll(/<script([^>]*)>/gi)) {
    assert.ok(/\ssrc=/.test(attrs), 'every script element has a src');
  }
});
