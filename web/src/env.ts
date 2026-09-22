// Every browser object the application uses, behind one injected interface. No
// component touches `window` or `document`: `main.tsx` builds the real `AppEnv`
// once, tests build a recording fake, and the source lint keeps it that way.

import { createContext, useContext, useEffect } from 'react';

import type { ApiEnv } from './session/api.ts';

export interface HashEnv {
  getHash(): string;
  /** Calls `listener` on every hash change; returns the unsubscribe function. */
  subscribe(listener: () => void): () => void;
  /** Rewrites the hash without adding a history entry. */
  replace(hash: string): void;
}

export interface AppEnv {
  api: ApiEnv;
  hash: HashEnv;
  print(): void;
  /** Hands text to the browser as a file download. */
  saveFile(text: string, mime: string, filename: string): void;
  setTitle(text: string): void;
  /** Moves focus; `false` when no element has that id. */
  focusById(id: string): boolean;
}

export const AppEnvContext = createContext<AppEnv | null>(null);

export function useAppEnv(): AppEnv {
  const env = useContext(AppEnvContext);
  if (env === null) {
    throw new Error('AppEnvContext has no value: render inside <AppEnvContext value={env}>.');
  }
  return env;
}

/**
 * Sends a message to the one polite live region in the shell. Alerts are never
 * sent here, and a message is never sent to both channels.
 */
export const AnnounceContext = createContext<(message: string) => void>(() => undefined);

export function useAnnounce(): (message: string) => void {
  return useContext(AnnounceContext);
}

/**
 * For a screen with nothing to say on arrival: it still OWNS the polite region, so
 * it empties it when it mounts and again when it is left. Every screen ends up
 * writing authoritatively - the rate schedule does so on every state - and no
 * message outlives the screen that wrote it.
 */
export function useClearedPolite(): void {
  const announce = useAnnounce();
  useEffect(() => {
    announce('');
    return () => announce('');
  }, [announce]);
}
