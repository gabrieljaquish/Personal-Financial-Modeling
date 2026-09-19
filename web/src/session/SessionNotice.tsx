import { NOTICE_HEADING_ID, noticeText, relaunchOffered } from './state.ts';
import type { ShellSession } from './state.ts';
import styles from './SessionNotice.module.css';

/**
 * Shown in place of the screens whenever this tab is not connected. The sentence
 * is a `role="alert"`; it never takes focus and is never mirrored into the polite
 * region. The re-open button sits outside the alert. In a terminal state the
 * button is ABSENT, not disabled: a control that cannot work is not offered.
 *
 * The heading is the notice's focus target (`tabIndex={-1}`: by script only, never
 * in the tab order). The shell moves focus to it when the notice replaces a screen
 * or withdraws its button (`sessionFocus`), so that focus is never left on an
 * unmounted control. It is a sibling of the alert, not the alert and not around it.
 */
export function SessionNotice({ state, onRelaunch }: { state: ShellSession; onRelaunch: () => void }) {
  const text = noticeText(state);
  if (text === null) {
    return null;
  }
  return (
    <section className={styles.notice} aria-labelledby={NOTICE_HEADING_ID}>
      <h2 className={styles.heading} id={NOTICE_HEADING_ID} tabIndex={-1}>
        Not connected
      </h2>
      <p className={styles.text} role="alert">
        <span aria-hidden="true">! </span>
        {text}
      </p>
      {relaunchOffered(state) ? (
        <button className={styles.action} type="button" aria-disabled={state.relaunch.kind === 'requesting' ? 'true' : undefined} onClick={onRelaunch}>
          Re-open from the application
        </button>
      ) : null}
    </section>
  );
}
