// A fixture for tests/markup-selftest.test.mjs: proves that a `.tsx` component with
// a CSS module renders under `node --test` through tests/support/register.mjs.
import styles from './Fixture.module.css';

export function Fixture({ items }: { items: readonly string[] }) {
  return (
    <ul className={styles.list} data-count={items.length}>
      {items.map((item) => (
        <li key={item}>{item}</li>
      ))}
      <li>
        <input id="x" type="text" readOnly value={'a "quoted" <value> & more'} />
        <br />
      </li>
    </ul>
  );
}
