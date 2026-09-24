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
  assert.ok(sources.length >= 30);
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
    // Not even a bare scheme, not even in a `startsWith` check: the server's CSP
    // derivation refuses a bundle whose text names one.
    assert.ok(!/https?:/i.test(text), `${file} names a URL scheme`);
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

// --- Every request leaves through one `fetch` init ---------------------------------
//
// SECURITY.md §7.2: an API request must carry the real `Origin`, and under the
// document's own `no-referrer` policy a conforming engine (Firefox, WebKit) sends
// `Origin: null` on a same-origin POST. The policy that keeps it real is pinned
// on the one init in `session/api.ts`; a second fetch site (the planned NDJSON
// stream) must build its options here too, where this lint can see them.

test('fetch is called only by the entry point and the session module', () => {
  const callers = sources.filter(({ text }) => /\bfetch\(/.test(text)).map(({ file }) => file);
  assert.deepEqual(callers.sort(), ['src/main.tsx', 'src/session/api.ts']);
});

test('every request option literal keeps Origin real: strict-origin and same-origin mode', () => {
  let pinned = 0;
  for (const { file, text } of sources) {
    for (const [, policy] of text.matchAll(/referrerPolicy:\s*'([^']*)'/g)) {
      pinned += 1;
      assert.equal(policy, 'strict-origin', `${file} sets referrerPolicy ${policy}`);
    }
    for (const [, mode] of text.matchAll(/\bmode:\s*'([^']*)'/g)) {
      assert.equal(mode, 'same-origin', `${file} sets mode ${mode}`);
    }
  }
  assert.ok(pinned >= 1, 'the policy is set somewhere');
});

// --- The browser is reached through `AppEnv` only ---------------------------------

test('window, document and object URLs are named only by the entry point', () => {
  for (const { file, text } of sources) {
    if (file !== 'src/main.tsx' && !file.endsWith('.css')) {
      for (const word of ['window.', 'document.', 'createObjectURL', 'globalThis', 'navigator.']) {
        assert.ok(!text.includes(word), `${file} names ${word}`);
      }
    }
  }
});

// --- Money: the UI formats, it never computes --------------------------------------
//
// Three controls, each covering what the others cannot:
//   * the type system makes ARITHMETIC on `Cents` a compile error (cents-types.test.mjs);
//   * it does not catch a relational operator between two `Cents`, so this lint
//     bans every relational operator and every sort in the directories that render;
//   * and `Cents` may be named only where amounts are turned into text, so every
//     view and container receives amounts as already-formatted strings.

const NUMERIC_ALLOWED = new Set(['src/format/money.ts', 'src/format/ratio.ts', 'src/format/year.ts']);

test('no float and no numeric conversion outside the three format files', () => {
  for (const { file, text } of sources) {
    if (file.endsWith('.css') || NUMERIC_ALLOWED.has(file)) {
      continue;
    }
    for (const word of ['parseFloat', 'toFixed', 'toPrecision', 'Math.', 'Number(', 'parseInt', 'BigInt']) {
      assert.ok(!text.includes(word), `${file} uses ${word}`);
    }
  }
});

test('type="number" is never used for an input: floats, scroll-wheel changes and locale parsing', () => {
  for (const { file, text } of sources) {
    assert.ok(!/type=["']number["']/.test(text), `${file} has a number input`);
  }
});

/** Code with string and template literals emptied, so an operator inside text is not code. */
export function withoutLiterals(text) {
  return code(text)
    .replace(/`(?:[^`\\]|\\.)*`/g, '``')
    .replace(/'(?:[^'\\\n]|\\.)*'/g, "''")
    .replace(/"(?:[^"\\\n]|\\.)*"/g, '""');
}

const RELATIONAL = /[\w)\]]\s+(<=?|>=?)\s+[\w(\[!+-]/;
const SORT_CALL = /\.(sort|toSorted)\s*\(/;

export function moneyLintViolations(text) {
  const stripped = withoutLiterals(text);
  return [RELATIONAL.test(stripped) ? 'relational operator' : null, SORT_CALL.test(stripped) ? 'sort call' : null].filter((v) => v !== null);
}

const RENDERING_DIRS = ['src/viewmodel/', 'src/screens/', 'src/components/'];

test('no relational operator and no sort where amounts are rendered', () => {
  const linted = sources.filter(({ file }) => /\.tsx?$/.test(file) && RENDERING_DIRS.some((dir) => file.startsWith(dir)));
  assert.ok(linted.length >= 15, 'the rendering directories are being linted');
  for (const dir of RENDERING_DIRS) {
    assert.ok(linted.some(({ file }) => file.startsWith(dir)), `${dir} has sources`);
  }
  for (const { file } of linted) {
    const raw = readFileSync(join(web, file), 'utf8');
    assert.deepEqual(moneyLintViolations(raw), [], file);
  }
});

test('the money lint proves itself on fixtures', () => {
  const caught = ['if (l.value > zero) {}', 'const x = a.amount <= b.amount;', 'rows.sort((a, b) => a.n - b.n);', 'const s = xs.toSorted(cmp);', 'for (let i = 0; i < n; i += 1) {}', 'return total >= limit ? a : b;'];
  for (const line of caught) {
    assert.notDeepEqual(moneyLintViolations(line), [], line);
  }
  const passed = [
    'const m = new Map<string, T>();',
    'const f = (a) => b;',
    'const el = <td className={x}>{y}</td>;',
    "const s = 'a < b';",
    'const t = `${a} > ${b}`;',
    '// a < b in a comment',
    'function f<Row extends SortableRow>(rows: readonly Row[]): Promise<Outcome<T>> {}',
    'const r: Readonly<Record<FieldName, string>> = {};',
  ];
  for (const line of passed) {
    assert.deepEqual(moneyLintViolations(line), [], line);
  }
});

test('Cents is named only where an amount becomes text', () => {
  const allowed = (file) => ['src/api/schema.gen.ts', 'src/api/client.ts', 'src/format/money.ts'].includes(file) || /^src\/viewmodel\/[^/]+\.ts$/.test(file);
  let named = 0;
  for (const { file, text } of sources) {
    if (/\bCents\b/.test(text)) {
      named += 1;
      assert.ok(allowed(file), `${file} names Cents`);
    }
  }
  assert.ok(named >= 3);
});

test('the only sorter lives outside the rendering directories', () => {
  const sorters = sources.filter(({ file, text }) => /\.tsx?$/.test(file) && SORT_CALL.test(withoutLiterals(text))).map(({ file }) => file);
  assert.deepEqual(sorters, ['src/table/sort.ts']);
});
