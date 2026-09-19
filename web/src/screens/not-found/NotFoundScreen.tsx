import { useClearedPolite } from '../../env.ts';
import { hrefFor } from '../../router/hash.ts';
import { ScreenHeading } from '../ScreenHeading.tsx';
import styles from '../Screen.module.css';

export function NotFoundScreen() {
  useClearedPolite();
  return (
    <section className={styles.screen} aria-labelledby="screen-heading">
      <ScreenHeading>Nothing at this address</ScreenHeading>
      <p>This address does not name a screen of the application. The two screens are:</p>
      <ul role="list">
        <li>
          <a className={styles.inlineLink} href={hrefFor({ screen: 'rate-schedule' }) ?? '#/'}>
            Rate schedule
          </a>
        </li>
        <li>
          <a className={styles.inlineLink} href={hrefFor({ screen: 'assumptions' }) ?? '#/'}>
            Assumptions Registry
          </a>
        </li>
      </ul>
    </section>
  );
}
