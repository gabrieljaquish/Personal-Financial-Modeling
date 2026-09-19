import styles from './Screen.module.css';

/**
 * The heading of a screen, and the focus target of a route change. `tabIndex={-1}`
 * makes it focusable by script only; it is never in the tab order.
 */
export function ScreenHeading({ children }: { children: string }) {
  return (
    <h2 className={styles.heading} id="screen-heading" tabIndex={-1}>
      {children}
    </h2>
  );
}
