// The accessibility checklist, asserted on static markup. It is the cheap first
// line in front of axe and a real browser, as the storage lint is in front of the
// browser storage assertion. It proves MARKUP: labels, references, order of
// focusable elements in the DOM, table structure, live-region discipline. It does
// not prove rendered contrast, real focus movement, that visual order matches DOM
// order, rendered target sizes, or what a screen reader says.

import assert from 'node:assert/strict';

import { ancestors, byTag, elements, textOf } from './markup.mjs';

const LIVE_ROLES = new Set(['alert', 'status', 'log']);

const isLive = (e) => LIVE_ROLES.has(e.attrs.role) || 'aria-live' in e.attrs;

const idRefs = (e) =>
  ['for', 'aria-labelledby', 'aria-describedby', 'aria-controls'].flatMap((name) => (name in e.attrs ? e.attrs[name].split(/\s+/).filter(Boolean) : []));

/**
 * Asserts checklist items A1, A2, A4 (the generic half), A5a, A6, A8, A11 and A12
 * on a parsed render. `focusTargets` lists the ids that may carry `tabindex="-1"`.
 */
export function assertAccessible(tree, { focusTargets = [], shell = false } = {}) {
  const all = elements(tree);
  const ids = new Map();
  for (const e of all) {
    if ('id' in e.attrs) {
      assert.ok(e.attrs.id !== '', 'A2: an id is not empty');
      assert.ok(!ids.has(e.attrs.id), `A2: id "${e.attrs.id}" is unique`);
      ids.set(e.attrs.id, e);
    }
  }

  for (const e of all) {
    // A2: every id reference resolves within this render.
    for (const ref of idRefs(e)) {
      assert.ok(ids.has(ref), `A2: <${e.tag}> refers to #${ref}, which is not in this render`);
    }
    for (const name of ['aria-labelledby', 'aria-describedby']) {
      for (const ref of (e.attrs[name] ?? '').split(/\s+/).filter(Boolean)) {
        assert.ok(textOf(ids.get(ref)) !== '', `A2: #${ref}, which names or describes <${e.tag}>, has text`);
      }
    }

    // A1: every control has an id and exactly one label with text.
    if (['input', 'select', 'textarea'].includes(e.tag)) {
      assert.ok('id' in e.attrs, `A1: <${e.tag}> has an id`);
      const labels = all.filter((l) => l.tag === 'label' && l.attrs.for === e.attrs.id);
      const labelled = 'aria-labelledby' in e.attrs;
      assert.ok(labels.length === 1 || (labels.length === 0 && labelled), `A1: #${e.attrs.id} has exactly one label`);
      for (const label of labels) {
        assert.ok(textOf(label) !== '', `A1: the label of #${e.attrs.id} has text`);
      }
      // A3 (half): an invalid field names an error element with text.
      if (e.attrs['aria-invalid'] === 'true') {
        const described = (e.attrs['aria-describedby'] ?? '').split(/\s+/);
        assert.ok(described.includes(`${e.attrs.id}-error`), `A3: #${e.attrs.id} is described by its error`);
      }
    }

    // A4 (generic half): no positive tabindex, -1 only on declared targets, no autofocus.
    if ('tabindex' in e.attrs) {
      assert.ok(['0', '-1'].includes(e.attrs.tabindex), `A4: tabindex="${e.attrs.tabindex}"`);
      if (e.attrs.tabindex === '-1') {
        assert.ok(focusTargets.includes(e.attrs.id), `A4: tabindex="-1" on #${e.attrs.id ?? `<${e.tag}>`} is a declared focus target`);
      }
      // A5a: a focusable or focus-target element is never a live region, nor inside one, nor around one.
      assert.ok(!isLive(e), `A5a: <${e.tag}> has both a live role and a tabindex`);
      if (e.attrs.tabindex === '-1') {
        assert.ok(!ancestors(e).some(isLive), `A5a: focus target #${e.attrs.id} sits inside a live region`);
        if (e.tag !== 'main') {
          assert.ok(!elements(e).some(isLive), `A5a: focus target #${e.attrs.id} contains a live region`);
        }
      }
    }
    assert.ok(!('autofocus' in e.attrs) && !('autoFocus' in e.attrs), 'A4: no autofocus');

    // A5: the only live-region attributes are role=status (polite), role=alert and aria-busy.
    if ('aria-live' in e.attrs) {
      assert.equal(e.attrs.role, 'status', 'A5: aria-live appears only on the status region');
      assert.equal(e.attrs['aria-live'], 'polite');
    }
    if (e.attrs.role === 'alert') {
      assert.ok(textOf(e) !== '', 'A5: an alert has text');
    }

    // A8: buttons and links.
    if (e.tag === 'button') {
      assert.ok(['button', 'submit'].includes(e.attrs.type), 'A8: a button has a type');
      assert.ok(textOf(e, { includeHidden: false }) !== '', 'A8: a button has text');
      // A12: unavailable-but-focusable is aria-disabled, never the disabled attribute.
      assert.ok(!('disabled' in e.attrs), 'A12: no disabled attribute on a button');
    }
    if (e.tag === 'a') {
      assert.ok((e.attrs.href ?? '') !== '', 'A8: a link has an href');
      assert.ok(textOf(e, { includeHidden: false }) !== '', 'A8: a link has text');
    }

    // A11: nothing inline.
    assert.ok(!('style' in e.attrs), `A11: <${e.tag}> has a style attribute`);
    for (const name of Object.keys(e.attrs)) {
      assert.ok(!/^on[a-z]+$/i.test(name), `A11: inline handler ${name}`);
    }
  }

  // A8: link text is unique per target, or carries the distinguishing id.
  const targetsByText = new Map();
  for (const a of byTag(tree, 'a')) {
    const text = textOf(a, { includeHidden: false });
    const seen = targetsByText.get(text);
    assert.ok(seen === undefined || seen === a.attrs.href, `A8: link text "${text}" leads to two places`);
    targetsByText.set(text, a.attrs.href);
  }

  // A6: tables.
  for (const table of byTag(tree, 'table')) {
    const captions = table.children.filter((c) => c.tag === 'caption');
    assert.equal(captions.length, 1, 'A6: a table has one caption');
    assert.ok(textOf(captions[0]) !== '', 'A6: the caption has text');
    assert.ok(!('role' in table.attrs), 'A6: no role on a data table');
    for (const th of byTag(table, 'th')) {
      assert.ok(['col', 'row'].includes(th.attrs.scope), 'A6: every th has a scope');
      assert.ok(textOf(th, { includeHidden: false }) !== '', 'A6: no empty th');
    }
    const head = table.children.find((c) => c.tag === 'thead');
    assert.ok(head !== undefined, 'A6: a table has a thead');
    const columns = byTag(head, 'th').length;
    assert.equal(byTag(head, 'td').length, 0, 'A6: header cells are th');
    const body = table.children.find((c) => c.tag === 'tbody');
    for (const row of body.children.filter((c) => c.tag === 'tr')) {
      const cells = row.children.filter((c) => c.tag !== undefined);
      assert.equal(cells[0].tag, 'th', 'A6: a body row starts with a row header');
      assert.equal(cells[0].attrs.scope, 'row');
      assert.equal(cells.length, columns, 'A6: every row has one cell per column');
    }
    // A7: sortable headers.
    const sortable = byTag(head, 'th').filter((th) => 'aria-sort' in th.attrs);
    if (sortable.length !== 0) {
      assert.equal(sortable.filter((th) => th.attrs['aria-sort'] !== 'none').length, 1, 'A7: exactly one sorted column');
      for (const th of sortable) {
        const buttons = byTag(th, 'button');
        assert.equal(buttons.length, 1, 'A7: a sortable header contains a button');
        const text = textOf(buttons[0], { includeHidden: false });
        const direction = th.attrs['aria-sort'];
        assert.ok(direction === 'none' ? /, not sorted$/.test(text) : text.endsWith(`, sorted ${direction}`), `A7: "${text}" names the direction in words`);
      }
    }
  }

  // A9: every glyph-only element is hidden from assistive technology.
  for (const e of all) {
    const own = e.children.filter((c) => c.text !== undefined).map((c) => c.text).join('');
    if (/[△●▲▼↕]|^!\s*$/.test(own)) {
      assert.equal(e.attrs['aria-hidden'], 'true', `A9: the glyph "${own.trim()}" is aria-hidden`);
      assert.ok(textOf(e.parent, { includeHidden: false }) !== '', 'A9: a glyph sits next to text');
    }
  }

  // Headings never skip a level.
  let level = 0;
  for (const e of all.filter((h) => /^h[1-6]$/.test(h.tag))) {
    const next = Number(e.tag[1]);
    assert.ok(shell ? next <= level + 1 : level === 0 || next <= level + 1, `A10: <${e.tag}> "${textOf(e)}" skips a heading level after h${level}`);
    level = next;
  }

  if (shell) {
    // A5: exactly one status region; A10: one h1 and each landmark once.
    assert.equal(all.filter((e) => e.attrs.role === 'status').length, 1, 'A5: exactly one role="status"');
    assert.equal(byTag(tree, 'h1').length, 1, 'A10: one h1');
    for (const landmark of ['header', 'nav', 'main', 'aside', 'footer']) {
      assert.equal(byTag(tree, landmark).length, 1, `A10: one <${landmark}>`);
    }
    assert.ok('aria-label' in byTag(tree, 'nav')[0].attrs, 'A10: the nav is labelled');
    assert.ok('aria-labelledby' in byTag(tree, 'aside')[0].attrs, 'A10: the aside is labelled');
  } else {
    assert.equal(all.filter((e) => e.attrs.role === 'status').length, 0, 'A5: no status region outside the shell');
  }
}

/** A5b: the polite text of a state is not repeated by an alert or by the focused container. */
export function assertOneChannel(tree, announcements) {
  const { polite, alert, focus } = announcements;
  assert.ok(polite === undefined || alert === undefined, 'A5b: a state announces politely or by alert, never both');
  const alerts = elements(tree).filter((e) => e.attrs.role === 'alert');
  if (polite !== undefined) {
    for (const e of alerts) {
      assert.ok(!textOf(e).includes(polite), 'A5b: the polite text is not mirrored in an alert');
    }
  }
  if (alert !== undefined) {
    assert.ok(alerts.some((e) => textOf(e, { includeHidden: false }) === alert), `A5b: the alert "${alert}" is rendered as a role="alert"`);
  }
  if (focus !== undefined) {
    const target = elements(tree).find((e) => e.attrs.id === focus);
    assert.ok(target !== undefined, `the focus target #${focus} is rendered`);
    assert.equal(target.attrs.tabindex, '-1');
    assert.ok(!isLive(target) && !elements(target).some(isLive), 'A5a: the focused container is not a live region');
    if (polite !== undefined) {
      assert.ok(!textOf(target).includes(polite), 'A5b: the polite text does not duplicate the focused container');
    }
  }
}
