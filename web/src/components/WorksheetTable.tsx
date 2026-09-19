import type { LineVm } from '../viewmodel/lines.ts';
import { ParamRefLink } from './ParamRefLink.tsx';
import styles from './Table.module.css';

/**
 * An explained worksheet: a real table in the server's computation order. The
 * line structure is a graph, not a tree (one input line feeds many), so a nested
 * list would repeat lines; a table is natively navigable and gives row and column
 * context for free. No grid role, no arrow-key handling, no focus jump between
 * rows: "Computed from" is plain text that carries its full referent.
 *
 * The two in-cell lists are styled `list-style: none`, and WebKit drops the list
 * role from such a `ul`, flattening "which lines feed this one" into one run of
 * text. `role="list"` on the `ul` restores it (WCAG 1.3.1); each `li` keeps its
 * implicit `listitem` role. The two-item primary navigation is deliberately left alone.
 *
 * The table keeps every column at every width inside a keyboard-scrollable region.
 */
export function WorksheetTable({ caption, captionId, lines }: { caption: string; captionId: string; lines: readonly LineVm[] }) {
  return (
    <div className={styles.scrollRegion} role="region" aria-labelledby={captionId} tabIndex={0}>
      <table className={styles.table}>
        <caption id={captionId}>{caption}</caption>
        <thead>
          <tr>
            <th scope="col">Line</th>
            <th scope="col" className={styles.amountHead}>
              Amount
            </th>
            <th scope="col">Computed from</th>
            <th scope="col">Parameters</th>
            <th scope="col">Rounding</th>
          </tr>
        </thead>
        <tbody>
          {lines.map((line) => (
            <tr key={line.id} className={line.isResult ? styles.resultRow : undefined}>
              <th scope="row">
                {`${String(line.no)}. ${line.label}`}
                {line.isResult ? <span className={styles.resultMark}> - Result</span> : null}
              </th>
              <td className={styles.amount}>{line.amount}</td>
              <td>
                {line.from.length === 0 ? (
                  'Entered'
                ) : (
                  <ul className={styles.cellList} role="list">
                    {line.from.map((text) => (
                      <li key={text}>{text}</li>
                    ))}
                  </ul>
                )}
              </td>
              <td>
                {line.params.length === 0 ? (
                  'None'
                ) : (
                  <ul className={styles.paramList} role="list">
                    {line.params.map((param) => (
                      <li key={param.text}>
                        <ParamRefLink param={param} />
                      </li>
                    ))}
                  </ul>
                )}
              </td>
              <td>{line.rounding ?? 'None'}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
