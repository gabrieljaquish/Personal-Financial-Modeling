import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';

import { App } from './App.tsx';
import { AppEnvContext } from './env.ts';
import type { AppEnv } from './env.ts';
import { bootstrapSession } from './session/handshake.ts';
import './index.css';

// The only storage this application touches is `sessionStorage["pfp.proof"]`,
// written by the session handshake and nowhere else: no localStorage, no
// IndexedDB, no Cache Storage, no service worker (ADR-002, SECURITY.md §7.4).
//
// This is the one file that names `window` and `document`. Everything else gets
// the browser through the `AppEnv` built here.

const container = document.getElementById('root');

if (container === null) {
  throw new Error('Mount point #root was not found in the document.');
}

const api = {
  fetch: (input: string, init: RequestInit) => window.fetch(input, init),
  sessionStorage: window.sessionStorage,
};

// The handshake runs before the first render and before the router exists, so the
// launch token has left the address bar by the time anything reads the hash.
const session = await bootstrapSession({
  ...api,
  location: window.location,
  history: window.history,
});

const env: AppEnv = {
  api,
  hash: {
    getHash: () => window.location.hash,
    subscribe(listener) {
      window.addEventListener('hashchange', listener);
      return () => window.removeEventListener('hashchange', listener);
    },
    replace(hash) {
      window.history.replaceState(null, '', window.location.pathname + window.location.search + hash);
    },
  },
  print: () => window.print(),
  saveFile(text, mime, filename) {
    // A user-initiated download of bytes the server sent (SECURITY.md §7.4:
    // "exports download via fetch + blob"). The anchor is attached for the click
    // because a click on an element that was never in the document is not
    // honoured by every browser; it is removed in the same tick, and the object
    // URL is revoked on the next.
    const url = URL.createObjectURL(new Blob([text], { type: mime }));
    const anchor = document.createElement('a');
    anchor.href = url;
    anchor.download = filename;
    anchor.hidden = true;
    document.body.append(anchor);
    anchor.click();
    anchor.remove();
    window.setTimeout(() => URL.revokeObjectURL(url), 0);
  },
  setTitle(text) {
    document.title = text;
  },
  focusById(id) {
    const target = document.getElementById(id);
    target?.focus();
    return target !== null;
  },
};

createRoot(container).render(
  <StrictMode>
    <AppEnvContext value={env}>
      <App initialSession={session} />
    </AppEnvContext>
  </StrictMode>,
);
