#!/usr/bin/env node
/*
 * Per-package front-end licence check.
 *
 * Every installed package — production AND development, at every depth of
 * node_modules — must declare a licence that is an exact member of the
 * permitted set. Anything else fails, and the failure is the point: no front-end
 * licence has been confirmed per package in advance (DECISIONS.md ADR-004,
 * "Consequences"; SECURITY.md §11 supply-chain table, where this row is a
 * gate-fails control).
 *
 * Where the permitted set actually lives: ADR-004 mandates the check but does
 * not enumerate the set. The enumeration is ARCHITECTURE.md §8 ("cargo-deny
 * licence allowlist (MIT, Apache-2.0, BSD-2/3-Clause, ISC, CC0-1.0, Unicode-3.0,
 * Zlib; anything else needs an ADR)") and the identical row in SECURITY.md §11.
 * The npm side is held to the same set so that one decision governs both halves
 * of the dependency tree.
 *
 * Deliberate design choices, each of which is a way this check could have been
 * made useless:
 *
 *   1. No dependency. A licence gate that installs a package to run has widened
 *      the tree it is meant to police.
 *   2. No JSON configuration file. web/scripts/*.json would also trip the
 *      repository's data-hygiene linter, which allows JSON under version control
 *      only at a short list of known configuration basenames.
 *   3. SPDX *expressions* are not parsed. "(MIT OR CC0-1.0)" is not an exact
 *      member of the set and therefore fails. Teaching this script to accept the
 *      permitted half of a disjunction is how an allowlist widens quietly, and a
 *      dual-licensed package is a decision for a human and docs/licence-watchlist.md,
 *      not for a regular expression.
 *   4. A missing licence field fails. Absent licences are exactly what scanners
 *      pass silently; ADR-004 names that phenomenon as the reason the watchlist
 *      exists.
 *
 * Usage: npm run licenses   (after npm ci --ignore-scripts)
 */

import { readFileSync, readdirSync, statSync } from 'node:fs';
import { dirname, join, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

/**
 * The permitted set. Extending it requires an ADR — see the header above.
 * Entries are exact SPDX identifiers, compared case-sensitively.
 */
const PERMITTED = [
  'MIT',
  'Apache-2.0',
  'BSD-2-Clause',
  'BSD-3-Clause',
  'ISC',
  'CC0-1.0',
  'Unicode-3.0',
  'Zlib',
];

const PERMITTED_SET = new Set(PERMITTED);

const WEB_ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const NODE_MODULES = join(WEB_ROOT, 'node_modules');

/** Marker used in the report when a package declares nothing usable. */
const NO_DECLARATION = '(no licence declared)';

/**
 * Read a package's declared licence without interpreting it.
 *
 * Handles every shape npm has ever blessed:
 *   "license": "MIT"                       modern SPDX identifier or expression
 *   "license": { "type": "MIT", ... }      deprecated object form
 *   "licenses": [ { "type": "MIT" }, ... ] legacy array form
 *   absent                                 → NO_DECLARATION
 *
 * A legacy array with more than one entry is joined with " OR " so that it is
 * reported honestly and then fails, for the same reason an SPDX expression does.
 */
function declaredLicence(manifest) {
  const { license, licenses } = manifest;

  if (typeof license === 'string' && license.trim() !== '') {
    return license.trim();
  }

  if (license !== null && typeof license === 'object') {
    const type = license.type;
    if (typeof type === 'string' && type.trim() !== '') {
      return type.trim();
    }
  }

  if (Array.isArray(licenses)) {
    const types = licenses
      .map((entry) => {
        if (typeof entry === 'string') return entry.trim();
        if (entry !== null && typeof entry === 'object' && typeof entry.type === 'string') {
          return entry.type.trim();
        }
        return '';
      })
      .filter((type) => type !== '');

    if (types.length > 0) {
      return types.join(' OR ');
    }
  }

  return NO_DECLARATION;
}

/**
 * Walk a node_modules directory, descending into scoped namespaces and into any
 * nested node_modules, and collect one record per installed package.
 */
function collect(nodeModulesDir, found) {
  let entries;
  try {
    entries = readdirSync(nodeModulesDir, { withFileTypes: true });
  } catch {
    return found;
  }

  for (const entry of entries) {
    // `.bin`, `.package-lock.json`, `.cache` and friends are npm bookkeeping.
    if (entry.name.startsWith('.')) continue;

    const dir = join(nodeModulesDir, entry.name);

    let isDirectory = entry.isDirectory();
    if (!isDirectory && entry.isSymbolicLink()) {
      try {
        isDirectory = statSync(dir).isDirectory();
      } catch {
        isDirectory = false;
      }
    }
    if (!isDirectory) continue;

    // A scoped namespace directory holds packages, not a package.
    if (entry.name.startsWith('@')) {
      collect(dir, found);
      continue;
    }

    const manifestPath = join(dir, 'package.json');
    let manifest = null;
    try {
      manifest = JSON.parse(readFileSync(manifestPath, 'utf8'));
    } catch {
      manifest = null;
    }

    if (manifest !== null && typeof manifest === 'object') {
      const name = typeof manifest.name === 'string' ? manifest.name : entry.name;
      const version = typeof manifest.version === 'string' ? manifest.version : '(unknown)';
      found.push({
        name,
        version,
        licence: declaredLicence(manifest),
        path: relative(WEB_ROOT, dir),
      });
    }

    collect(join(dir, 'node_modules'), found);
  }

  return found;
}

function pad(text, width) {
  return text.length >= width ? text : text + ' '.repeat(width - text.length);
}

function main() {
  try {
    if (!statSync(NODE_MODULES).isDirectory()) throw new Error('not a directory');
  } catch {
    process.stderr.write(
      'web/node_modules is missing. Run `npm ci --ignore-scripts` first.\n',
    );
    process.exitCode = 1;
    return;
  }

  const packages = collect(NODE_MODULES, []);

  if (packages.length === 0) {
    process.stderr.write(
      'No installed packages were found under web/node_modules. Refusing to report a\n' +
        'vacuous pass: run `npm ci --ignore-scripts` and try again.\n',
    );
    process.exitCode = 1;
    return;
  }

  // Deduplicate identical name@version installed at several depths.
  const byKey = new Map();
  for (const pkg of packages) {
    const key = `${pkg.name}@${pkg.version}`;
    if (!byKey.has(key)) byKey.set(key, pkg);
  }

  const unique = [...byKey.values()].sort((a, b) =>
    a.name === b.name ? a.version.localeCompare(b.version) : a.name.localeCompare(b.name),
  );

  const nameWidth = Math.min(
    48,
    unique.reduce((max, pkg) => Math.max(max, `${pkg.name}@${pkg.version}`.length), 0),
  );

  const lines = [
    'Front-end per-package licence check (DECISIONS.md ADR-004).',
    `Permitted: ${PERMITTED.join(', ')}.`,
    'An SPDX expression, a dual licence or a missing licence is not permitted.',
    '',
  ];

  const rejected = [];

  for (const pkg of unique) {
    const permitted = PERMITTED_SET.has(pkg.licence);
    if (!permitted) rejected.push(pkg);
    lines.push(
      `  ${permitted ? 'ok  ' : 'FAIL'}  ${pad(`${pkg.name}@${pkg.version}`, nameWidth)}  ${pkg.licence}`,
    );
  }

  lines.push('');
  process.stdout.write(`${lines.join('\n')}\n`);

  if (rejected.length > 0) {
    const detail = rejected
      .map((pkg) => `  ${pkg.name}@${pkg.version}  ${pkg.licence}  (${pkg.path})`)
      .join('\n');
    process.stderr.write(
      `Licence check FAILED: ${rejected.length} of ${unique.length} package(s) declare a\n` +
        `licence outside the permitted set.\n\n${detail}\n\n` +
        'Resolve this by removing the dependency, by replacing it, or — if the licence is\n' +
        'genuinely acceptable — by widening the permitted set in a new ADR and recording\n' +
        'the package in docs/licence-watchlist.md. Do not special-case it here.\n',
    );
    process.exitCode = 1;
    return;
  }

  process.stdout.write(
    `Licence check passed: ${unique.length} package(s), all within the permitted set.\n`,
  );
}

main();
