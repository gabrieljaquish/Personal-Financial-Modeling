// Recording fakes for every browser object the front end is handed. One
// `fakeResponse` serves every test, and it behaves like a real `Response`:
// `json()` parses the text (so it REJECTS for a CSV body), `text()` resolves it,
// and a body can be read once. A hand-written `{ status, json }` object would hide
// exactly the defect the export path had to fix.

export function fakeResponse(status, bodyText = '', headers = {}) {
  const named = new Map(Object.entries(headers).map(([name, value]) => [name.toLowerCase(), value]));
  const reads = { json: 0, text: 0 };
  let used = false;
  const take = () => {
    if (used) {
      throw new TypeError('body already used');
    }
    used = true;
  };
  return {
    status,
    reads,
    // Like `Headers`: names are case-insensitive and an absent header is `null`.
    headers: { get: (name) => named.get(name.toLowerCase()) ?? null },
    async json() {
      reads.json += 1;
      take();
      return JSON.parse(bodyText);
    },
    async text() {
      reads.text += 1;
      take();
      return bodyText;
    },
  };
}

export const errorResponse = (status, code) => fakeResponse(status, JSON.stringify({ code, message: 'server text that is never rendered' }));

/**
 * An `ApiEnv` whose `fetch` answers from `respond(path, init)`, which returns a
 * `fakeResponse` (or throws, for an unreachable server).
 */
export function fakeApiEnv(respond, { proof = 'cd'.repeat(32) } = {}) {
  const calls = [];
  const responses = [];
  const store = new Map(proof === null ? [] : [['pfp.proof', proof]]);
  return {
    calls,
    responses,
    store,
    env: {
      sessionStorage: {
        getItem: (key) => (store.has(key) ? store.get(key) : null),
        setItem: (key, value) => void store.set(key, value),
        removeItem: (key) => void store.delete(key),
      },
      fetch: async (input, init) => {
        calls.push({ input, init, body: init.body === undefined ? undefined : JSON.parse(init.body) });
        const response = await respond(input, init);
        responses.push(response);
        return response;
      },
    },
  };
}

/** An `AppEnv` that records every effect and touches nothing. */
export function fakeAppEnv(api, { hash = '' } = {}) {
  const log = { titles: [], focus: [], prints: 0, files: [], replaced: [] };
  let current = hash;
  const listeners = new Set();
  return {
    log,
    navigate(next) {
      current = next;
      for (const listener of listeners) {
        listener();
      }
    },
    env: {
      api,
      hash: {
        getHash: () => current,
        subscribe(listener) {
          listeners.add(listener);
          return () => listeners.delete(listener);
        },
        replace(next) {
          log.replaced.push(next);
          current = next;
        },
      },
      print: () => void (log.prints += 1),
      saveFile: (text, mime, filename) => void log.files.push({ text, mime, filename }),
      setTitle: (text) => void log.titles.push(text),
      focusById: (id) => {
        log.focus.push(id);
        return true;
      },
    },
  };
}
