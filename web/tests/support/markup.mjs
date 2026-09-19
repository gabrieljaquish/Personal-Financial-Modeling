// A small, tolerant tokenizer for the well-formed static markup `react-dom/server`
// emits, and query helpers over the resulting tree. It is not an HTML parser: it
// understands what React writes (double-quoted attributes, bare boolean
// attributes, the void elements, character references) and throws on anything else, which
// is the right way for a test helper to fail. No jsdom, no dependency.

const VOID = new Set(['area', 'base', 'br', 'col', 'embed', 'hr', 'img', 'input', 'link', 'meta', 'source', 'track', 'wbr']);

const ENTITIES = { amp: '&', lt: '<', gt: '>', quot: '"', apos: "'", nbsp: ' ' };

export function decode(text) {
  return text.replace(/&(#x[0-9a-fA-F]+|#[0-9]+|[a-zA-Z]+);/g, (whole, name) => {
    if (name.startsWith('#x')) {
      return String.fromCodePoint(Number.parseInt(name.slice(2), 16));
    }
    if (name.startsWith('#')) {
      return String.fromCodePoint(Number.parseInt(name.slice(1), 10));
    }
    if (!(name in ENTITIES)) {
      throw new Error(`unknown character reference ${whole}`);
    }
    return ENTITIES[name];
  });
}

function parseAttrs(source) {
  const attrs = {};
  const pattern = /\s+([^\s=/>]+)(?:="([^"]*)")?/gy;
  let at = 0;
  pattern.lastIndex = 0;
  for (;;) {
    const match = pattern.exec(source);
    if (match === null) {
      break;
    }
    attrs[match[1]] = match[2] === undefined ? '' : decode(match[2]);
    at = pattern.lastIndex;
  }
  if (source.slice(at).trim() !== '' && source.slice(at).trim() !== '/') {
    throw new Error(`unreadable attributes: ${source}`);
  }
  return attrs;
}

/** Parses markup into `{ tag: '#root', children }`. Text nodes are `{ text }`. */
export function parse(markup) {
  const root = { tag: '#root', attrs: {}, children: [], parent: null };
  let current = root;
  let at = 0;
  while (at < markup.length) {
    const open = markup.indexOf('<', at);
    if (open === -1) {
      current.children.push({ text: decode(markup.slice(at)) });
      break;
    }
    if (open > at) {
      current.children.push({ text: decode(markup.slice(at, open)) });
    }
    if (markup.startsWith('<!--', open)) {
      const end = markup.indexOf('-->', open);
      if (end === -1) {
        throw new Error('unterminated comment');
      }
      at = end + 3;
      continue;
    }
    const close = markup.indexOf('>', open);
    if (close === -1) {
      throw new Error('unterminated tag');
    }
    const inner = markup.slice(open + 1, close);
    if (inner.startsWith('/')) {
      const tag = inner.slice(1).trim();
      if (current.tag !== tag) {
        throw new Error(`</${tag}> closes <${current.tag}>`);
      }
      current = current.parent;
    } else {
      const name = /^[a-zA-Z][a-zA-Z0-9-]*/.exec(inner);
      if (name === null) {
        throw new Error(`unreadable tag <${inner}>`);
      }
      const tag = name[0];
      const node = { tag, attrs: parseAttrs(inner.slice(tag.length)), children: [], parent: current };
      current.children.push(node);
      if (!VOID.has(tag) && !inner.endsWith('/')) {
        current = node;
      }
    }
    at = close + 1;
  }
  if (current !== root) {
    throw new Error(`<${current.tag}> was never closed`);
  }
  return root;
}

/** Every element under `node`, in document order. */
export function elements(node) {
  const out = [];
  const visit = (n) => {
    for (const child of n.children ?? []) {
      if (child.tag !== undefined) {
        out.push(child);
        visit(child);
      }
    }
  };
  visit(node);
  return out;
}

/** The text of `node`, whitespace collapsed; `aria-hidden` subtrees included only on request. */
export function textOf(node, { includeHidden = true } = {}) {
  const parts = [];
  const visit = (n) => {
    if (n.text !== undefined) {
      parts.push(n.text);
      return;
    }
    if (!includeHidden && n.attrs?.['aria-hidden'] === 'true') {
      return;
    }
    for (const child of n.children) {
      visit(child);
    }
  };
  visit(node);
  return parts.join('').replace(/\s+/g, ' ').trim();
}

export const byTag = (node, tag) => elements(node).filter((e) => e.tag === tag);
export const byAttr = (node, name, value) =>
  elements(node).filter((e) => (value === undefined ? name in e.attrs : e.attrs[name] === value));
export const byId = (node, id) => elements(node).find((e) => e.attrs.id === id);
export const byClass = (node, name) =>
  elements(node).filter((e) => (e.attrs.class ?? '').split(/\s+/).includes(name));

export function ancestors(node) {
  const out = [];
  for (let p = node.parent; p !== null && p !== undefined; p = p.parent) {
    out.push(p);
  }
  return out;
}

/** Elements a keyboard user can tab to, in DOM order, as short descriptions. */
export function focusables(node) {
  return elements(node)
    .filter((e) => {
      if (e.attrs.tabindex === '-1') {
        return false;
      }
      if (e.tag === 'a') {
        return 'href' in e.attrs;
      }
      if (['button', 'input', 'select', 'textarea'].includes(e.tag)) {
        return !('disabled' in e.attrs);
      }
      return e.attrs.tabindex === '0';
    })
    .map((e) => {
      if (['input', 'select', 'textarea'].includes(e.tag)) {
        return `${e.tag}:#${e.attrs.id ?? ''}`;
      }
      if (e.tag === 'a' || e.tag === 'button') {
        return `${e.tag}:${textOf(e, { includeHidden: false })}`;
      }
      // A focusable container (a scroll region) is named by reference, not by its contents.
      return `${e.tag}:${e.attrs['aria-labelledby'] ?? e.attrs['aria-label'] ?? ''}`;
    });
}
