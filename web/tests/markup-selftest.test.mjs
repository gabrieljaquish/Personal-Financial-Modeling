// The test harness tests itself: the `.tsx` loader, the CSS-module stub and the
// markup tokenizer every view test stands on.

import assert from 'node:assert/strict';
import { test } from 'node:test';
import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';

import { Fixture } from './fixtures/Fixture.tsx';
import { byClass, byId, byTag, decode, elements, focusables, parse, textOf } from './support/markup.mjs';
import { snapshotBody } from './support/golden.mjs';

test('a .tsx component with a CSS module renders under node:test', () => {
  const markup = renderToStaticMarkup(createElement(Fixture, { items: ['one', 'two & three'] }));
  const tree = parse(markup);
  const list = byClass(tree, 'list')[0];
  assert.equal(list.tag, 'ul');
  assert.equal(list.attrs['data-count'], '2');
  assert.deepEqual(byTag(list, 'li').slice(0, 2).map((li) => textOf(li)), ['one', 'two & three']);
  // Attribute quoting and character references survive the round trip.
  assert.equal(byId(tree, 'x').attrs.value, 'a "quoted" <value> & more');
  assert.equal(byId(tree, 'x').attrs.readOnly ?? byId(tree, 'x').attrs.readonly, '');
});

test('nesting, void elements, comments, boolean attributes and entities', () => {
  const tree = parse('<div id="a"><p>x<br/>y<img src="s" alt="">z</p><!-- c --><input disabled><span aria-hidden="true">&#x25B2;</span> &amp;&lt;&gt;&quot;&#39;&nbsp;</div>');
  const div = byId(tree, 'a');
  assert.deepEqual(elements(div).map((e) => e.tag), ['p', 'br', 'img', 'input', 'span']);
  assert.equal(textOf(byTag(tree, 'p')[0]), 'xyz');
  assert.equal(byTag(tree, 'input')[0].attrs.disabled, '');
  assert.equal(textOf(div), 'xyz▲ &<>"\'');
  assert.equal(textOf(div, { includeHidden: false }), 'xyz &<>"\'');
  assert.equal(decode('&#8722;$1'), '−$1');
});

test('malformed markup throws instead of being guessed at', () => {
  for (const bad of ['<div>', '<div></span>', '<div', '<p>&bogus;</p>', "<p class='single'></p>", '<!-- open']) {
    assert.throws(() => parse(bad), bad);
  }
});

test('focusables lists what the keyboard reaches, in DOM order', () => {
  const tree = parse(
    '<a href="#/x">Link</a><a>no href</a><button type="button">B</button><input id="f"><h2 tabindex="-1">H</h2><div tabindex="0" aria-labelledby="cap">T</div><button type="button" disabled>D</button>',
  );
  assert.deepEqual(focusables(tree), ['a:Link', 'button:B', 'input:#f', 'div:cap']);
});

test('snapshot front matter is stripped, and a file without it is refused', () => {
  assert.equal(snapshotBody('---\nsource: x\n---\n{"a":1}\n'), '{"a":1}\n');
  assert.throws(() => snapshotBody('{"a":1}'));
});
