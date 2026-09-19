import type { SessionStatus } from '../api/schema.gen.ts';
import { failureText } from '../api/errors.ts';
import { aboutVm } from '../viewmodel/status.ts';
import type { Loadable } from '../viewmodel/status.ts';
import styles from './AboutBuild.module.css';

/** Build and parameter identity, as the server reports it. */
export function AboutBuild({ status }: { status: Loadable<SessionStatus> }) {
  return (
    <aside className={styles.aside} aria-labelledby="about-heading">
      <h2 className={styles.heading} id="about-heading">
        About this build
      </h2>
      {status.kind === 'ready' ? (
        <dl className={styles.list}>
          {aboutVm(status.value).entries.map(([label, value]) => (
            <div key={label} className={styles.entry}>
              <dt>{label}</dt>
              <dd>{value}</dd>
            </div>
          ))}
        </dl>
      ) : (
        <p className={styles.text}>
          {status.kind === 'failed' ? `Build identity could not be read. ${failureText(status.failure)}` : 'Build identity is shown here once this tab is connected to the application.'}
        </p>
      )}
    </aside>
  );
}
