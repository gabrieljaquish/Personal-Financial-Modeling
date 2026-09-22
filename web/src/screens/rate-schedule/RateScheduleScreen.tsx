import { useEffect, useReducer } from 'react';

import type { ApiClient } from '../../api/client.ts';
import { politeText } from '../../announce.ts';
import { useAnnounce, useAppEnv } from '../../env.ts';
import { RateScheduleView } from './RateScheduleView.tsx';
import { performCalculation, performExport } from './effects.ts';
import { ERROR_SUMMARY_ID, INITIAL_RS_STATE, RESULT_HEADING_ID, rsAnnouncements, rsReducer } from './state.ts';

/** The container: owns the reducer and the effects, and no markup decisions. */
export function RateScheduleScreen({ client, publishedYears }: { client: ApiClient; publishedYears: readonly string[] }) {
  const env = useAppEnv();
  const announce = useAnnounce();
  const [state, dispatch] = useReducer(rsReducer, INITIAL_RS_STATE);
  const polite = politeText(rsAnnouncements(state));

  useEffect(() => {
    if (state.phase.kind !== 'loading' || state.pending === null) {
      return;
    }
    void performCalculation(client, state.pending, state.request).then(dispatch);
  }, [client, state.phase.kind, state.pending, state.request]);

  useEffect(() => {
    if (state.exportPhase.kind !== 'working' || state.lastInputs === null) {
      return;
    }
    // The last SUCCESSFUL inputs, not the current form contents.
    void performExport(client, env.saveFile, state.lastInputs, state.exportPhase.format).then(dispatch);
  }, [client, env, state.exportPhase, state.lastInputs]);

  useEffect(() => {
    if (state.focusSeq !== 0) {
      env.focusById(ERROR_SUMMARY_ID);
    }
  }, [env, state.focusSeq]);

  useEffect(() => {
    if (state.resultFocusSeq !== 0) {
      env.focusById(RESULT_HEADING_ID);
    }
  }, [env, state.resultFocusSeq]);

  useEffect(() => {
    // Authoritative, including on mount: '' clears what an earlier state or screen left.
    announce(polite);
  }, [announce, polite]);

  // Leaving the screen empties the region it owns, whatever state it was left in.
  useEffect(() => () => announce(''), [announce]);

  return (
    <RateScheduleView
      state={state}
      publishedYears={publishedYears}
      handlers={{
        onChange: (field, value) => dispatch({ type: 'change', field, value }),
        onSubmit: () => dispatch({ type: 'submit' }),
        onRetry: () => dispatch({ type: 'retry' }),
        onJump: (inputId) => env.focusById(inputId),
        onExport: (format) => dispatch({ type: 'export-started', format }),
        onPrint: () => env.print(),
      }}
    />
  );
}
