import type { BadgeVm } from '../viewmodel/status.ts';
import styles from './Status.module.css';

/**
 * A verification status. The words carry the meaning; the glyph and the border
 * style are decoration and are hidden from assistive technology. Never colour alone.
 */
export function VerificationBadge({ badge }: { badge: BadgeVm }) {
  const attention = badge.tone === 'attention';
  return (
    <span className={attention ? styles.badgeAttention : styles.badgeConfirmed}>
      <span aria-hidden="true">{attention ? '△ ' : '● '}</span>
      {badge.text}
    </span>
  );
}
