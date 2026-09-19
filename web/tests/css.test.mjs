// CSS-source tests: regex over comment-stripped stylesheets. They prove what the
// SOURCE says - focus is never removed, status markers have a non-colour property,
// print keeps the honesty notices, controls declare a 24px minimum, layout cannot
// reorder the DOM, the registry collapses on narrow viewports, colour pairs meet
// AA - not what a browser renders. Rendered sizes, reflow at 320px and real
// contrast are for the browser suite.

import assert from 'node:assert/strict';
import { readdirSync, readFileSync } from 'node:fs';
import { join, relative, sep } from 'node:path';
import { test } from 'node:test';

const src = join(import.meta.dirname, '..', 'src');

function walk(dir) {
  return readdirSync(dir, { withFileTypes: true }).flatMap((e) => (e.isDirectory() ? walk(join(dir, e.name)) : [join(dir, e.name)]));
}

const sheets = Object.fromEntries(
  walk(src)
    .filter((file) => file.endsWith('.css'))
    .map((file) => [relative(src, file).split(sep).join('/'), readFileSync(file, 'utf8').replace(/\/\*[\s\S]*?\*\//g, '')]),
);
const raw = (name) => readFileSync(join(src, name), 'utf8');

/** Top-level blocks of a stylesheet: `[{ prelude, body }]`, with at-rule bodies kept whole. */
function blocks(css) {
  const out = [];
  let depth = 0;
  let start = 0;
  let prelude = '';
  for (let i = 0; i < css.length; i += 1) {
    if (css[i] === '{') {
      if (depth === 0) {
        prelude = css.slice(start, i).trim();
        start = i + 1;
      }
      depth += 1;
    } else if (css[i] === '}') {
      depth -= 1;
      if (depth === 0) {
        out.push({ prelude, body: css.slice(start, i) });
        start = i + 1;
      }
    }
  }
  assert.equal(depth, 0, 'balanced braces');
  return out;
}

/** Every style rule as `{ selectors, declarations, media }`, at-rules flattened one level. */
function rules(css) {
  return blocks(css).flatMap(({ prelude, body }) =>
    prelude.startsWith('@')
      ? blocks(body).map((inner) => ({ selectors: inner.prelude, declarations: inner.body, media: prelude }))
      : [{ selectors: prelude, declarations: body, media: null }],
  );
}

const allRules = Object.entries(sheets).flatMap(([file, css]) => rules(css).map((rule) => ({ file, ...rule })));
const hasClass = (selectors, name) => new RegExp(`\\.${name}(?![\\w-])`).test(selectors);
const declares = (declarations, property) => new RegExp(`(^|[;\\s])${property}\\s*:`).test(declarations);
const valueOf = (declarations, property) => new RegExp(`(?:^|[;\\s])${property}\\s*:\\s*([^;]+)`).exec(declarations)?.[1].trim();

test('there are stylesheets, and every one parses', () => {
  assert.ok(Object.keys(sheets).length >= 9);
  assert.ok(allRules.length > 80);
});

test('focus is never removed', () => {
  for (const [file, css] of Object.entries(sheets)) {
    assert.ok(!/outline\s*:\s*(none|0)\b/.test(css), `${file} removes an outline`);
    assert.ok(!/outline-width\s*:\s*0\b/.test(css), `${file} removes an outline`);
  }
  assert.match(sheets['index.css'], /:focus-visible\s*\{[^}]*outline:\s*3px solid/);
});

test('!important appears only in the reduced-motion block', () => {
  for (const { file, declarations, media } of allRules) {
    if (declarations.includes('!important')) {
      assert.equal(file, 'index.css');
      assert.match(media ?? '', /prefers-reduced-motion:\s*reduce/);
    }
  }
  const reduced = allRules.filter((r) => /prefers-reduced-motion/.test(r.media ?? ''));
  assert.ok(reduced.some((r) => /animation-duration/.test(r.declarations) && /transition-duration/.test(r.declarations)));
});

test('no animation or transition is declared outside the reduced-motion override', () => {
  for (const { file, declarations, media } of allRules) {
    if (!/prefers-reduced-motion/.test(media ?? '')) {
      assert.ok(!/(^|[;\s])(animation|transition)(-[a-z]+)?\s*:/.test(declarations), `${file} animates`);
    }
  }
});

test('non-colour encoding: every status marker sets a border style, a text decoration or generated content', () => {
  const markers = ['badgeAttention', 'badgeConfirmed', 'vintageBanner', 'unverifiedNotice', 'failure', 'fieldError', 'errorSummary', 'resultRow', 'resultMark', 'statusLine', 'notice'];
  for (const marker of markers) {
    const own = allRules.filter((r) => r.media === null && hasClass(r.selectors, marker));
    assert.ok(own.length !== 0, `.${marker} is styled`);
    const text = own.map((r) => r.declarations).join(';');
    assert.ok(/border(-[a-z]+)?-style\s*:|border(-[a-z]+)?\s*:[^;]*(solid|dashed|double|dotted)|text-decoration\s*:|content\s*:/.test(text), `.${marker} has a non-colour property`);
  }
  // Attention and confirmed differ by more than colour.
  const style = (name) => valueOf(allRules.find((r) => r.media === null && r.selectors.trim() === `.${name}`).declarations, 'border-style');
  assert.notEqual(style('badgeAttention'), style('badgeConfirmed'));
  // aria-disabled and aria-invalid are said without colour too.
  assert.match(sheets['index.css'], /button\[aria-disabled='true'\]\s*\{[^}]*border-style:\s*dashed/);
  assert.match(sheets['index.css'], /\[aria-invalid='true'\]\s*\{[^}]*border-style:\s*double/);
});

function printHidden(file) {
  return rules(sheets[file])
    .filter((r) => r.media === '@media print' && /display\s*:\s*none/.test(r.declarations))
    .flatMap((r) => r.selectors.split(',').map((s) => s.trim()));
}

test('print: the shell and the rate schedule hide their furniture', () => {
  assert.deepEqual(printHidden('App.module.css'), ['.skipLink', '.tagline']);
  assert.deepEqual(printHidden('components/PrimaryNav.module.css'), ['.nav']);
  assert.deepEqual(printHidden('components/AboutBuild.module.css'), ['.aside']);
  assert.deepEqual(printHidden('screens/rate-schedule/RateSchedule.module.css'), ['.form', '.actions']);
  assert.deepEqual(printHidden('components/Form.module.css'), ['.errorSummary', '.actions']);
  assert.deepEqual(printHidden('session/SessionNotice.module.css'), ['.action']);
});

test('print: the unverified notice and the vintage banner are never hidden', () => {
  for (const { file, selectors, declarations, media } of allRules) {
    if (/display\s*:\s*none/.test(declarations)) {
      for (const kept of ['unverifiedNotice', 'vintageBanner', 'headline', 'inputs', 'table', 'wordmark', 'contentinfo', 'footerText', 'statusLine', 'badgeAttention']) {
        assert.ok(!hasClass(selectors, kept), `${file} hides .${kept} (${media ?? 'all media'})`);
      }
    }
  }
});

test('print: black on white, bordered cells, repeated header, unbroken rows, no clipped table', () => {
  const index = rules(sheets['index.css']).filter((r) => r.media === '@media print');
  assert.ok(index.some((r) => r.selectors === 'body' && /background-color:\s*#ffffff/.test(r.declarations) && /color:\s*#000000/.test(r.declarations)));
  assert.match(sheets['index.css'], /@page\s*\{\s*margin:\s*15mm;?\s*\}/);
  const table = rules(sheets['components/Table.module.css']).filter((r) => r.media === '@media print');
  const find = (selector) => table.find((r) => r.selectors.includes(selector)).declarations;
  assert.match(find('.scrollRegion'), /overflow:\s*visible/);
  assert.match(find('.table thead'), /display:\s*table-header-group/);
  assert.match(find('.table tr'), /break-inside:\s*avoid/);
  assert.match(find('.table td'), /border:\s*1px solid #000000/);
  // No URL expansion: no generated content from an href anywhere.
  for (const css of Object.values(sheets)) {
    assert.ok(!/attr\(\s*href/.test(css));
  }
});

function lengthInPx(value) {
  const match = /^([0-9.]+)(rem|px)$/.exec(value ?? '');
  return match === null ? 0 : Number(match[1]) * (match[2] === 'rem' ? 16 : 1);
}

test('target size (WCAG 2.2 SC 2.5.8): every in-cell and inline control declares a 24px minimum', () => {
  const controls = ['sortButton', 'paramRef', 'rowLink', 'errorJump', 'navLink', 'inlineLink'];
  for (const name of controls) {
    const rule = allRules.find((r) => r.media === null && hasClass(r.selectors, name) && declares(r.declarations, 'min-block-size'));
    assert.ok(rule !== undefined, `.${name} declares a minimum size`);
    assert.ok(lengthInPx(valueOf(rule.declarations, 'min-block-size')) >= 24, `.${name} min-block-size`);
    assert.ok(lengthInPx(valueOf(rule.declarations, 'min-inline-size')) >= 24, `.${name} min-inline-size`);
    assert.equal(valueOf(rule.declarations, 'display'), 'inline-flex', `.${name}`);
    assert.equal(valueOf(rule.declarations, 'align-items'), 'center', `.${name}`);
    // The technique is stated at the top of the module.
    assert.match(raw(rule.file), /MINIMUM SIZE/, rule.file);
  }
  // Several controls in one cell: their list keeps a gap.
  const list = allRules.find((r) => r.media === null && hasClass(r.selectors, 'paramList'));
  assert.ok(lengthInPx(valueOf(list.declarations, 'gap').replace('var(--pfp-space-1)', '0.25rem')) >= 4);
  assert.match(sheets['index.css'], /--pfp-space-1:\s*0\.25rem/);
  // The base rule carries the same minimum, so a control added later inherits it.
  const button = rules(sheets['index.css']).find((r) => r.selectors === 'button');
  assert.ok(lengthInPx(valueOf(button.declarations, 'min-block-size')) >= 24);
  assert.ok(lengthInPx(valueOf(button.declarations, 'min-inline-size')) >= 24);
  assert.equal(valueOf(button.declarations, 'display'), 'inline-flex');
});

test('order-preserving layout: nothing can make visual order differ from DOM order', () => {
  for (const { file, selectors, declarations } of allRules) {
    assert.ok(!/(^|[;\s])order\s*:/.test(declarations), `${file} ${selectors}: order`);
    assert.ok(!/-reverse\b/.test(declarations), `${file} ${selectors}: a reversed flex direction`);
    assert.ok(!/(^|[;\s])(grid-row|grid-column|grid-row-start|grid-column-start)\s*:/.test(declarations), `${file} ${selectors}: grid placement`);
    assert.ok(!/(^|[;\s])float\s*:/.test(declarations), `${file} ${selectors}: float`);
    if (/position\s*:\s*(absolute|fixed|sticky)/.test(declarations)) {
      // The one exception: the off-screen skip link, first in the DOM and first when focused.
      assert.deepEqual([file, selectors], ['App.module.css', '.skipLink']);
    }
    if (declares(declarations, 'grid-area')) {
      assert.equal(file, 'App.module.css', `${selectors}: grid-area outside the shell`);
    }
  }
});

test('the shell\'s grid areas, read row by row, are in the order of its landmarks in the DOM', () => {
  const shell = rules(sheets['App.module.css']).find((r) => declares(r.declarations, 'grid-template-areas'));
  const areas = [...shell.declarations.matchAll(/'([^']+)'/g)].flatMap((m) => m[1].trim().split(/\s+/));
  const order = [...new Set(areas)];
  // shell.test.mjs asserts the DOM order header, main, aside, footer.
  assert.deepEqual(order, ['banner', 'main', 'aside', 'footer']);
  const assigned = rules(sheets['App.module.css'])
    .filter((r) => declares(r.declarations, 'grid-area'))
    .map((r) => [r.selectors, valueOf(r.declarations, 'grid-area')]);
  assert.deepEqual(assigned, [['.banner', 'banner'], ['.main', 'main'], ['.shell > aside', 'aside'], ['.contentinfo', 'footer']]);
});

test('narrow viewports: the registry hides its secondary columns below 48rem and says so', () => {
  const css = rules(sheets['screens/assumptions/Assumptions.module.css']);
  const narrow = css.filter((r) => /^@media\s*\(max-width:\s*48rem\)$/.test(r.media ?? ''));
  assert.match(narrow.find((r) => r.selectors === '.secondaryCol').declarations, /display:\s*none/);
  assert.match(narrow.find((r) => r.selectors === '.narrowNote').declarations, /display:\s*block/);
  const base = css.find((r) => r.media === null && r.selectors === '.narrowNote');
  assert.match(base.declarations, /display:\s*none/);
  assert.ok(!css.some((r) => r.media === null && hasClass(r.selectors, 'secondaryCol')), 'the columns are visible at full width');
});

test('reflow: wide tables scroll inside their own region; long identifiers wrap', () => {
  const region = allRules.find((r) => r.media === null && r.selectors === '.scrollRegion');
  assert.match(region.declarations, /overflow-x:\s*auto/);
  assert.match(region.declarations, /max-inline-size:\s*100%/);
  assert.match(sheets['screens/assumptions/Assumptions.module.css'], /\.code\s*\{[^}]*overflow-wrap:\s*anywhere/);
  assert.match(sheets['index.css'], /input,\s*select\s*\{[^}]*max-inline-size:\s*100%/);
  for (const [file, css] of Object.entries(sheets)) {
    assert.ok(!/(^|[;\s{])(min-)?width\s*:\s*[0-9]{4,}px/.test(css), `${file} sets a fixed wide width`);
  }
});

function luminance(hex) {
  const channel = (at) => {
    const c = Number.parseInt(hex.slice(at, at + 2), 16) / 255;
    return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
  };
  return 0.2126 * channel(1) + 0.7152 * channel(3) + 0.0722 * channel(5);
}

function contrast(a, b) {
  const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x);
  return (hi + 0.05) / (lo + 0.05);
}

test('colour tokens used for text keep the AA pairs documented in index.css (arithmetic on colours, not money)', () => {
  const token = (name) => {
    const value = new RegExp(`--pfp-${name}:\\s*(#[0-9a-fA-F]{6});`).exec(sheets['index.css'])?.[1];
    assert.ok(value !== undefined, `--pfp-${name} is a six-digit hex colour`);
    return value;
  };
  const pairs = [
    ['ink', 'surface'],
    ['ink', 'surface-sunken'],
    ['ink', 'attention-surface'],
    ['ink', 'confirmed-surface'],
    ['ink-muted', 'surface'],
    ['ink-muted', 'surface-sunken'],
    ['accent', 'surface'],
    ['accent', 'surface-sunken'],
    ['accent-ink', 'accent'],
    ['attention-ink', 'attention-surface'],
    ['attention-ink', 'surface'],
    ['confirmed-ink', 'confirmed-surface'],
    ['confirmed-ink', 'surface'],
  ];
  for (const [fg, bg] of pairs) {
    const ratio = contrast(token(fg), token(bg));
    assert.ok(ratio >= 4.5, `${fg} on ${bg} is ${ratio.toFixed(2)}:1`);
    assert.ok(raw('index.css').includes(fg), `${fg} is documented`);
  }
  assert.equal(contrast('#000000', '#ffffff').toFixed(0), '21');
});

test('every colour in a component stylesheet is a token; literals exist only in index.css and in print blocks', () => {
  for (const { file, declarations, media } of allRules) {
    if (file !== 'index.css' && media !== '@media print') {
      assert.ok(!/#[0-9a-fA-F]{3,8}\b/.test(declarations), `${file} has a literal colour`);
    }
  }
});

test('no remote resource: no @import, no url()', () => {
  for (const [file, css] of Object.entries(sheets)) {
    assert.ok(!/@import/.test(css), file);
    assert.ok(!/url\(/.test(css), file);
  }
});
