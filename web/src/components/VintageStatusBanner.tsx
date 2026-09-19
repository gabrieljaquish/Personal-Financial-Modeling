import type { SessionStatus } from '../api/schema.gen.ts';
import { bannerSentences } from '../viewmodel/status.ts';
import type { Loadable } from '../viewmodel/status.ts';
import styles from './Status.module.css';

/**
 * The persistent statement of what this build's parameters are: on every route,
 * not dismissible, present when the status could not be read, and printed. It is
 * a labelled region, not a live region - it does not change under the reader.
 */
export function VintageStatusBanner({ status }: { status: Loadable<SessionStatus> }) {
  const sentences = bannerSentences(status);
  if (sentences.length === 0) {
    return null;
  }
  return (
    <div className={styles.vintageBanner} role="region" aria-label="Parameter verification status">
      {sentences.map((sentence) => (
        <p key={sentence}>
          <span aria-hidden="true">△ </span>
          {sentence}
        </p>
      ))}
    </div>
  );
}
