import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';

import { App } from './App.tsx';
import './index.css';

// No storage API is touched here or anywhere below: no localStorage, no
// IndexedDB, no Cache Storage, no service worker (ADR-002, SECURITY.md §7.2).
// The session proof token that will live in sessionStorage is introduced with
// the session bootstrap, not with the shell.

const container = document.getElementById('root');

if (container === null) {
  throw new Error('Mount point #root was not found in the document.');
}

createRoot(container).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
