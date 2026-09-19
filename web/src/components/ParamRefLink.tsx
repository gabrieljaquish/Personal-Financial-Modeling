import type { ParamRefVm } from '../viewmodel/lines.ts';
import styles from './Table.module.css';

/**
 * A reference to one parameter cell. The text always carries the parameter id, so
 * link text is distinguishable; an id that no route can address is rendered as
 * the same text without a link, never as a link that lands on "Not found".
 */
export function ParamRefLink({ param }: { param: ParamRefVm }) {
  return param.href === null ? (
    <span className={styles.paramText}>{param.text}</span>
  ) : (
    <a className={styles.paramRef} href={param.href}>
      {param.text}
    </a>
  );
}
