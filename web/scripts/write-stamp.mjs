// Records the front-end input hash beside the bundle, as the last step of
// `npm run build`. `vite build` empties `dist`, stamp included, so a build that
// did not end here would leave a bundle that a release build of `pfp-server`
// refuses as stale ("the front end is stale").
//
// This is the same function as `input_hash` in
// crates/pfp-server/build_support/web_inputs.rs, which is what the Rust build
// script checks the stamp against: SHA-256 over the byte-sorted list of
// `<relative path> NUL <sha256 hex of the file> LF`, over the five named input
// files that exist plus everything under `src/`. The two are held together by a
// shared known-answer vector (tests/stamp.test.mjs here, build_web.rs in xtask)
// and by `cargo xtask build-web`, which recomputes the hash in Rust after every
// build and fails if this script wrote anything else.
//
// No dependency, no network, and it writes exactly one file: dist/.dist-stamp.

import { createHash } from 'node:crypto';
import { existsSync, readdirSync, readFileSync, statSync, writeFileSync } from 'node:fs';
import { join, relative, sep } from 'node:path';
import { pathToFileURL } from 'node:url';

export const INPUT_FILES = ['index.html', 'package.json', 'package-lock.json', 'tsconfig.json', 'vite.config.ts'];
export const INPUT_DIR = 'src';
export const STAMP_PATH = 'dist/.dist-stamp';

const sha256 = (bytes) => createHash('sha256').update(bytes).digest('hex');

function walk(dir) {
  return readdirSync(dir, { withFileTypes: true }).flatMap((entry) =>
    entry.isDirectory() ? walk(join(dir, entry.name)) : [join(dir, entry.name)],
  );
}

/** Every input file relative to `webDir`, `/`-separated, sorted by bytes. */
export function inputPaths(webDir) {
  const files = INPUT_FILES.map((name) => join(webDir, name)).filter(
    (path) => existsSync(path) && statSync(path).isFile(),
  );
  const src = join(webDir, INPUT_DIR);
  if (existsSync(src) && statSync(src).isDirectory()) {
    files.push(...walk(src));
  }
  return files
    .map((path) => relative(webDir, path).split(sep).join('/'))
    .sort((a, b) => Buffer.compare(Buffer.from(a, 'utf8'), Buffer.from(b, 'utf8')));
}

/** The lower-case hex input hash of the front end rooted at `webDir`. */
export function inputHash(webDir) {
  const outer = createHash('sha256');
  for (const path of inputPaths(webDir)) {
    outer.update(Buffer.from(path, 'utf8'));
    outer.update(Buffer.from([0]));
    outer.update(sha256(readFileSync(join(webDir, path))), 'utf8');
    outer.update('\n', 'utf8');
  }
  return outer.digest('hex');
}

/** Writes the stamp for the bundle in `webDir`/dist; returns the hash. */
export function writeStamp(webDir) {
  if (!existsSync(join(webDir, 'dist', 'index.html'))) {
    throw new Error('dist/index.html is absent: there is no bundle to stamp');
  }
  const hash = inputHash(webDir);
  writeFileSync(join(webDir, STAMP_PATH), `${hash}\n`);
  return hash;
}

if (process.argv[1] !== undefined && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const hash = writeStamp(join(import.meta.dirname, '..'));
  console.log(`input hash ${hash} -> ${STAMP_PATH}`);
}
