import { useCallback, useEffect, useMemo, useReducer, useRef } from 'react';
import type { ReactNode } from 'react';

import { politeOwner, politeReducer, politeText, visiblePolite } from './announce.ts';
import { createClient } from './api/client.ts';
import type { SessionStatus } from './api/schema.gen.ts';
import { AboutBuild } from './components/AboutBuild.tsx';
import { LiveStatus } from './components/LiveStatus.tsx';
import { PrimaryNav } from './components/PrimaryNav.tsx';
import { VintageStatusBanner } from './components/VintageStatusBanner.tsx';
import { AnnounceContext, useAppEnv } from './env.ts';
import { documentTitle, resolutionOf } from './router/hash.ts';
import type { Route } from './router/hash.ts';
import { useRoute } from './router/useRoute.ts';
import { AssumptionsScreen } from './screens/assumptions/AssumptionsScreen.tsx';
import { NotFoundScreen } from './screens/not-found/NotFoundScreen.tsx';
import { AboutScreen } from './screens/about/AboutScreen.tsx';
import { RateScheduleScreen } from './screens/rate-schedule/RateScheduleScreen.tsx';
import type { SessionState } from './session/handshake.ts';
import { SessionNotice } from './session/SessionNotice.tsx';
import { initialShellSession, sessionAnnouncements, sessionFocus, sessionLossHandler, sessionReducer } from './session/state.ts';
import type { ShellSession } from './session/state.ts';
import { useLoadable } from './useLoadable.ts';
import { registryVm } from './viewmodel/registry.ts';
import type { Loadable } from './viewmodel/status.ts';
import styles from './App.module.css';

/**
 * The shell as a pure function: landmarks, the skip link, the primary navigation,
 * the persistent verification banner, the one polite live region, and either the
 * session notice or the routed screen.
 *
 * Styling rule for every component: class names from a CSS module, never a
 * `style` prop. The Content-Security-Policy is `style-src 'self'` with no
 * `'unsafe-inline'` (SECURITY.md §7.1), which blocks inline style attributes as
 * well as inline <style> elements.
 */
export function AppView({
  shell,
  route,
  status,
  message,
  onSkip,
  onRelaunch,
  children,
}: {
  shell: ShellSession;
  route: Route;
  status: Loadable<SessionStatus>;
  message: string;
  onSkip: () => void;
  onRelaunch: () => void;
  children: ReactNode;
}) {
  return (
    <div className={styles.shell}>
      <header className={styles.banner}>
        {/*
          First focusable element in the document. It keeps a real `href` so it is
          a link to every tool that inspects it, but the click is handled: with
          hash routing, following `#main-content` would replace the route.
        */}
        <a
          className={styles.skipLink}
          href="#main-content"
          onClick={(event) => {
            event.preventDefault();
            onSkip();
          }}
        >
          Skip to main content
        </a>
        <h1 className={styles.wordmark}>Personal Financial Modeling</h1>
        <p className={styles.tagline}>Planning, computed locally. Nothing leaves this machine.</p>
        {/* No screen is on screen while the session notice replaces it, so no link is the current page. */}
        <PrimaryNav route={shell.session === 'connected' ? route : null} />
        <VintageStatusBanner status={status} />
      </header>

      <main className={styles.main} id="main-content" tabIndex={-1}>
        <LiveStatus message={message} />
        {shell.session === 'connected' ? children : <SessionNotice state={shell} onRelaunch={onRelaunch} />}
      </main>

      <AboutBuild status={status} />

      <footer className={styles.contentinfo}>
        <p className={styles.footerText}>Personal Financial Modeling is planning software, not financial, investment, tax or legal advice.</p>
      </footer>
    </div>
  );
}

/** The container: session state, the API client, the router and the two shared loads. */
export function App({ initialSession }: { initialSession: SessionState }) {
  const env = useAppEnv();
  const [shell, dispatch] = useReducer(sessionReducer, initialSession, initialShellSession);
  // A session that ended (401) also drops the dead proof from the injected storage.
  const client = useMemo(() => createClient(env.api, sessionLossHandler(env.api, dispatch)), [env]);
  const route = useRoute(env);
  const owner = politeOwner(route.screen);
  const [message, dispatchPolite] = useReducer(politeReducer, undefined, () => ({ owner, text: politeText(sessionAnnouncements(initialShellSession(initialSession))) }));
  const announce = useMemo(() => (text: string) => dispatchPolite({ type: 'write', owner, text }), [owner]);
  // Leaving a screen empties what it wrote, even when neither screen writes anything itself.
  useEffect(() => dispatchPolite({ type: 'route', owner }), [owner]);

  // Focus follows a control the session state unmounts (see `sessionFocus`). An
  // effect, so the notice's heading is in the document by the time it is focused.
  const previousShell = useRef(shell);
  useEffect(() => {
    const target = sessionFocus(previousShell.current, shell);
    previousShell.current = shell;
    if (target !== null) {
      env.focusById(target);
    }
  }, [env, shell]);

  const connected = shell.session === 'connected';
  const [status] = useLoadable(client.status, connected);
  const [assumptions, retryAssumptions] = useLoadable(client.assumptions, connected);
  const [validation, retryValidation] = useLoadable(client.validationReport, connected);
  const registry = useMemo(() => (assumptions.kind === 'ready' ? { kind: 'ready' as const, value: registryVm(assumptions.value) } : assumptions), [assumptions]);

  // The title describes what is on screen, which the route alone does not know.
  const title = documentTitle(route, { connected, resolution: resolutionOf(route, registry.kind === 'ready' ? registry.value.ids : null) });
  useEffect(() => env.setTitle(title), [env, title]);

  const onRelaunch = useCallback(() => {
    if (shell.relaunch.kind === 'requesting') {
      return;
    }
    // One click, one request: no timer, no retry loop, no automatic click.
    dispatch({ type: 'relaunch-requested' });
    void client.relaunch().then((outcome) => dispatch({ type: 'relaunch-result', outcome }));
  }, [client, shell.relaunch.kind]);

  // A lost session empties the polite region: its sentence is the notice's alert.
  // A message is shown only on the screen that wrote it (see `visiblePolite`).
  const polite = visiblePolite(message, owner, connected);

  return (
    <AnnounceContext value={announce}>
      <AppView shell={shell} route={route} status={status} message={polite} onSkip={() => env.focusById('main-content')} onRelaunch={onRelaunch}>
        {route.screen === 'rate-schedule' ? <RateScheduleScreen client={client} publishedYears={registry.kind === 'ready' ? registry.value.publishedYears : []} /> : null}
        {route.screen === 'assumptions' || route.screen === 'assumption' ? (
          <AssumptionsScreen registry={registry} routedId={route.screen === 'assumption' ? route.id : null} onRetry={retryAssumptions} />
        ) : null}
        {route.screen === 'about' ? <AboutScreen status={status} report={validation} onRetry={retryValidation} /> : null}
        {route.screen === 'not-found' ? <NotFoundScreen /> : null}
      </AppView>
    </AnnounceContext>
  );
}
