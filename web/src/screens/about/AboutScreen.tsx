import type { SessionStatus, ValidationReportResponse } from '../../api/schema.gen.ts';
import { useAppEnv, useClearedPolite } from '../../env.ts';
import type { Loadable } from '../../viewmodel/status.ts';
import { retryWithFocus } from '../assumptions/AssumptionsScreen.tsx';
import { AboutView } from './AboutView.tsx';

/** The container: the report and the status are loaded once by the shell. */
export function AboutScreen({ status, report, onRetry }: { status: Loadable<SessionStatus>; report: Loadable<ValidationReportResponse>; onRetry: () => void }) {
  const env = useAppEnv();
  // Nothing to announce on arrival: the headline is read as content.
  useClearedPolite();
  return <AboutView status={status} report={report} onRetry={retryWithFocus(env, onRetry)} />;
}
