import { useEffect, useReducer } from 'react';

import type { AppEnv } from '../env.ts';
import { INITIAL_ROUTER_STATE, needsRedirect, hrefFor, DEFAULT_ROUTE, routerReducer } from './hash.ts';
import type { Route, RouterState } from './hash.ts';

/** The router state for the hash the application booted with. */
export function bootRouterState(hash: string): RouterState {
  return routerReducer(INITIAL_ROUTER_STATE, { type: 'hash', hash });
}

/**
 * The current route. Focus moves to the screen's heading only on changes after the
 * first render. The title is the shell's to set (`documentTitle`).
 */
export function useRoute(env: AppEnv): Route {
  const [state, dispatch] = useReducer(routerReducer, env.hash.getHash(), bootRouterState);

  useEffect(() => {
    if (needsRedirect(env.hash.getHash())) {
      env.hash.replace(hrefFor(DEFAULT_ROUTE) ?? '#/');
    }
    return env.hash.subscribe(() => dispatch({ type: 'hash', hash: env.hash.getHash() }));
  }, [env]);

  useEffect(() => {
    for (const effect of state.effects) {
      if (!env.focusById(effect.id)) {
        // An entry's detail does not exist until the registry has loaded.
        env.focusById('screen-heading');
      }
    }
  }, [env, state]);

  return state.route;
}
