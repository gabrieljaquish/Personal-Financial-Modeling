import { useCallback, useEffect, useState } from 'react';

import type { Outcome } from './api/client.ts';
import type { Loadable } from './viewmodel/status.ts';

// Stands in for TanStack Query, which ADR-002 names and M0 does not install:
// docs/contributing.md §8 item 5.

/** Loads once when `enabled`, and again on `retry`. A superseded answer is dropped. */
export function useLoadable<T>(load: () => Promise<Outcome<T>>, enabled: boolean): readonly [Loadable<T>, () => void] {
  const [state, setState] = useState<Loadable<T>>({ kind: 'idle' });
  const [attempt, setAttempt] = useState(0);

  useEffect(() => {
    if (!enabled) {
      return undefined;
    }
    let current = true;
    setState({ kind: 'loading' });
    void load().then((outcome) => {
      if (current) {
        setState(outcome.ok ? { kind: 'ready', value: outcome.value } : { kind: 'failed', failure: outcome.failure });
      }
    });
    return () => {
      current = false;
    };
  }, [load, enabled, attempt]);

  const retry = useCallback(() => setAttempt((n) => n + 1), []);
  return [state, retry];
}
