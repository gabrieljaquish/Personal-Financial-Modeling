import { useAnnounce, useAppEnv, useClearedPolite } from '../../env.ts';
import type { AppEnv } from '../../env.ts';
import type { RegistryVm } from '../../viewmodel/registry.ts';
import type { Loadable } from '../../viewmodel/status.ts';
import { AssumptionsView } from './AssumptionsView.tsx';

/** The screen heading: it is rendered in every state of the registry, so it survives a retry. */
export const RETRY_FOCUS_ID = 'screen-heading';

/**
 * "Try again" sits beside the failure alert, and the loading state unmounts both.
 * Focus is moved to the screen heading FIRST - while the button still exists - so
 * it is never left on a removed element (WCAG 2.4.3). The heading is outside the
 * alert and is not a live region.
 */
export function retryWithFocus(env: Pick<AppEnv, 'focusById'>, onRetry: () => void): () => void {
  return () => {
    env.focusById(RETRY_FOCUS_ID);
    onRetry();
  };
}

/** The container: the registry is loaded once by the shell and shared with the rate-schedule hint. */
export function AssumptionsScreen({ registry, routedId, onRetry }: { registry: Loadable<RegistryVm>; routedId: string | null; onRetry: () => void }) {
  const env = useAppEnv();
  const announce = useAnnounce();
  // The registry writes only on a sort; arriving and leaving empty the region.
  useClearedPolite();
  return <AssumptionsView registry={registry} routedId={routedId} onRetry={retryWithFocus(env, onRetry)} onSorted={announce} />;
}
