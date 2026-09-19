import styles from './Status.module.css';

/**
 * THE polite live region: exactly one `role="status"` exists in the application,
 * and this is it. It stays in the accessibility tree while empty, because a
 * region inserted together with its first message is not announced. Alerts are
 * never mirrored here (`announce.ts`).
 */
export function LiveStatus({ message }: { message: string }) {
  return (
    <p className={styles.liveRegion} role="status" aria-live="polite">
      {message}
    </p>
  );
}
