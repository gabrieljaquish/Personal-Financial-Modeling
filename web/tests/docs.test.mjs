// The two READMEs make claims a reader acts on. These pin the ones a review found
// wrong or missing, so the wording cannot drift back.

import assert from 'node:assert/strict';
import { existsSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { test } from 'node:test';

const repo = join(import.meta.dirname, '..', '..');
const flat = (path) => readFileSync(join(repo, path), 'utf8').replace(/\s+/g, ' ');
const root = flat('README.md');
const web = flat('web/README.md');

test('the JSON export is described as the same VALUES, pretty-printed - never as the same bytes', () => {
  assert.ok(!/byte-for-byte/i.test(root), 'the export is pretty-printed plus a newline: it is not the API body byte for byte');
  assert.match(root, /JSON export carries the API's values verbatim, pretty-printed/);
  // The claim is the one the exporter makes about itself.
  const exporter = flat('crates/pfp-server/src/api/export.rs').replaceAll('/// ', '');
  assert.match(exporter, /pretty-printed, one trailing newline/);
  assert.match(root, /fails closed/);
});

test('the projection deferral points at the erratum that already proposes its wording', () => {
  const errata = 'docs/verification/m0-design-errata.md';
  assert.ok(existsSync(join(repo, errata)));
  assert.match(readFileSync(join(repo, errata), 'utf8'), /^## E7 - projection "under an editable inflation assumption"/m);
  for (const [name, text] of [['README.md', root], ['web/README.md', web]]) {
    assert.ok(text.includes(errata), `${name} names ${errata}`);
    assert.match(text, /\bE7\b/, `${name} names the erratum`);
  }
  assert.ok(root.includes(`](${errata})`), 'and links it, so the deferral and its wording are one click apart');
});

test('the READMEs describe one filename source and a warning in front of both downloads and Print', () => {
  for (const text of [root, web]) {
    assert.ok(!text.includes('pfp-rate-schedule-'), 'no client-composed filename is documented');
    assert.match(text, /Content-Disposition/);
    assert.match(text, /printed page/);
  }
});
