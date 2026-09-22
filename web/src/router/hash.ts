// Hash routing, as two pure functions and a reducer. No router dependency.
//
// Rules (each has a test):
//   * the launch fragment `#t=<token>` is consumed and cleared by the session
//     handshake before anything here runs; the router never sees it;
//   * a hash that does not start with `#/` is ignored, so nothing in the page can
//     knock the application off its route;
//   * the only data a route carries is a parameter-table id - repository-safe
//     data. No input value, amount or token ever enters a URL (SECURITY.md §7.4);
//   * the document title describes what is ON SCREEN, which the route alone does
//     not know: the shell computes it with `documentTitle` from the route, whether
//     the session is connected and whether a routed registry id resolves. The
//     reducer therefore emits no title effect;
//   * focus moves to the screen's heading only on changes AFTER the first, so the
//     skip link stays the first thing a keyboard user meets and nothing is stolen
//     on load.

export const APP_NAME = 'Personal Financial Modeling';

/** The one pattern for an id that may appear in a route. */
export const ROUTE_ID_PATTERN = /^[A-Za-z0-9._-]{1,80}$/;

export type Route =
  | { screen: 'rate-schedule' }
  | { screen: 'assumptions' }
  | { screen: 'assumption'; id: string }
  | { screen: 'about' }
  | { screen: 'not-found' };

export const DEFAULT_ROUTE: Route = { screen: 'rate-schedule' };

/** `null` when the hash is not a route at all and must be ignored. */
export function parseRoute(hash: string): Route | null {
  if (hash === '' || hash === '#' || hash === '#/') {
    return DEFAULT_ROUTE;
  }
  if (!hash.startsWith('#/')) {
    return null;
  }
  const path = hash.slice(2);
  if (path === 'rate-schedule') {
    return { screen: 'rate-schedule' };
  }
  if (path === 'assumptions') {
    return { screen: 'assumptions' };
  }
  if (path === 'about') {
    return { screen: 'about' };
  }
  const prefix = 'assumptions/';
  if (path.startsWith(prefix)) {
    const id = path.slice(prefix.length);
    return ROUTE_ID_PATTERN.test(id) ? { screen: 'assumption', id } : { screen: 'not-found' };
  }
  return { screen: 'not-found' };
}

/**
 * The hash for a route, or `null` when the route cannot be addressed: an id
 * outside the pattern is rendered as text, never as a link that lands on
 * "Not found".
 */
export function hrefFor(route: Route): string | null {
  switch (route.screen) {
    case 'rate-schedule':
      return '#/rate-schedule';
    case 'assumptions':
      return '#/assumptions';
    case 'assumption':
      return ROUTE_ID_PATTERN.test(route.id) ? `#/assumptions/${route.id}` : null;
    case 'about':
      return '#/about';
    case 'not-found':
      return null;
  }
}

/** Whether the boot hash should be rewritten to the default route's hash. */
export function needsRedirect(hash: string): boolean {
  return hash === '' || hash === '#' || hash === '#/';
}

export function titleFor(route: Route): string {
  switch (route.screen) {
    case 'rate-schedule':
      return `Rate schedule - ${APP_NAME}`;
    case 'assumptions':
      return `Assumptions Registry - ${APP_NAME}`;
    case 'assumption':
      return `${route.id} - Assumptions Registry - ${APP_NAME}`;
    case 'about':
      return `About - ${APP_NAME}`;
    case 'not-found':
      return `Not found - ${APP_NAME}`;
  }
}

export const NOT_CONNECTED_TITLE = `Not connected - ${APP_NAME}`;

/** Whether a routed registry id names an entry: unknown until the registry has loaded. */
export type Resolution = 'unknown' | 'resolved' | 'missing';

export function resolutionOf(route: Route, registryIds: readonly string[] | null): Resolution {
  if (route.screen !== 'assumption' || registryIds === null) {
    return 'unknown';
  }
  return registryIds.includes(route.id) ? 'resolved' : 'missing';
}

/**
 * The document title for what is on screen (WCAG 2.4.2). A tab that is not
 * connected shows the session notice on every route, so it says so. A registry
 * address names its entry only once the registry confirms the entry exists: an id
 * that does not resolve gets the not-found title, and an id not yet checked gets
 * the registry's own title, never a claim that the id exists.
 */
export function documentTitle(route: Route, shell: { connected: boolean; resolution: Resolution }): string {
  if (!shell.connected) {
    return NOT_CONNECTED_TITLE;
  }
  if (route.screen !== 'assumption' || shell.resolution === 'resolved') {
    return titleFor(route);
  }
  return titleFor(shell.resolution === 'missing' ? { screen: 'not-found' } : { screen: 'assumptions' });
}

/** The id of the element that receives focus when a route is navigated to. */
export function focusTargetFor(route: Route): string {
  return route.screen === 'assumption' ? detailHeadingId(route.id) : 'screen-heading';
}

export function detailHeadingId(tableId: string): string {
  return `detail-${tableId}`;
}

export function sameRoute(a: Route, b: Route): boolean {
  return a.screen === b.screen && (a.screen !== 'assumption' || (b.screen === 'assumption' && a.id === b.id));
}

export interface RouterEffect {
  kind: 'focus';
  id: string;
}

export interface RouterState {
  route: Route;
  /** How many route events have been applied: none, the initial one, or more. */
  navigations: 'none' | 'initial' | 'many';
  /** What the last event asks the environment to do. */
  effects: readonly RouterEffect[];
}

export const INITIAL_ROUTER_STATE: RouterState = { route: DEFAULT_ROUTE, navigations: 'none', effects: [] };

/**
 * Applies a hash. The first event (the boot route, direct or via the redirect)
 * moves no focus; later ones do. A hash that is not a route changes nothing. The
 * title is not decided here: see `documentTitle`.
 */
export function routerReducer(state: RouterState, event: { type: 'hash'; hash: string }): RouterState {
  const route = parseRoute(event.hash);
  if (route === null) {
    return { ...state, effects: [] };
  }
  if (state.navigations === 'none') {
    return { route, navigations: 'initial', effects: [] };
  }
  if (sameRoute(route, state.route)) {
    return { ...state, effects: [] };
  }
  return { route, navigations: 'many', effects: [{ kind: 'focus', id: focusTargetFor(route) }] };
}
