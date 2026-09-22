import { canRetry, failureText } from '../api/errors.ts';
import type { ApiFailure } from '../api/errors.ts';
import styles from './Status.module.css';

/**
 * A failure that is not the form's to fix. The text is a `role="alert"` and never
 * takes focus; "Try again" sits beside it, outside the alert, and is offered only
 * where trying again can help.
 */
export function FailureAlert({ failure, onRetry }: { failure: ApiFailure; onRetry: (() => void) | null }) {
  return (
    <div className={styles.failure}>
      <p role="alert">
        <span aria-hidden="true">! </span>
        {failureText(failure)}
      </p>
      {onRetry !== null && canRetry(failure) ? (
        <button type="button" onClick={onRetry}>
          Try again
        </button>
      ) : null}
    </div>
  );
}
