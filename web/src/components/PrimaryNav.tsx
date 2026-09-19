import { hrefFor } from '../router/hash.ts';
import type { Route } from '../router/hash.ts';
import styles from './PrimaryNav.module.css';

const ITEMS: readonly { route: Route; label: string; screens: readonly Route['screen'][] }[] = [
  { route: { screen: 'rate-schedule' }, label: 'Rate schedule', screens: ['rate-schedule'] },
  { route: { screen: 'assumptions' }, label: 'Assumptions Registry', screens: ['assumptions', 'assumption'] },
];

/**
 * `route` is the route whose screen is ON SCREEN, or `null` when none is: a tab
 * that is not connected shows the session notice in place of every screen, so no
 * link may claim `aria-current="page"`.
 */
export function PrimaryNav({ route }: { route: Route | null }) {
  return (
    <nav className={styles.nav} aria-label="Primary">
      <ul className={styles.navList}>
        {ITEMS.map((item) => (
          <li key={item.label}>
            <a className={styles.navLink} href={hrefFor(item.route) ?? '#/'} aria-current={route !== null && item.screens.includes(route.screen) ? 'page' : undefined}>
              {item.label}
            </a>
          </li>
        ))}
      </ul>
    </nav>
  );
}
