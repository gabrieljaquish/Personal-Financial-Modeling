import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';

import { App } from './App.tsx';
import { bootstrapSession } from './session/handshake.ts';
import './index.css';

// The only storage this application touches is `sessionStorage["pfp.proof"]`,
// written by the session handshake and nowhere else: no localStorage, no
// IndexedDB, no Cache Storage, no service worker (ADR-002, SECURITY.md §7.4).

const container = document.getElementById('root');

if (container === null) {
  throw new Error('Mount point #root was not found in the document.');
}

// The handshake runs before the first render so the launch token has left the
// address bar by the time anything is on screen.
const session = await bootstrapSession({
  location: window.location,
  history: window.history,
  sessionStorage: window.sessionStorage,
  fetch: (input, init) => window.fetch(input, init),
});

createRoot(container).render(
  <StrictMode>
    <App session={session} />
  </StrictMode>,
);
