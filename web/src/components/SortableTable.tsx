import { useState } from 'react';
import type { ReactNode } from 'react';

import { flip, sortRows } from '../table/sort.ts';
import type { SortableRow, SortDirection } from '../table/sort.ts';
import styles from './Table.module.css';

export interface Column<Row> {
  /** Also the sort key, when `sortable`. */
  readonly key: string;
  readonly header: string;
  readonly sortable?: boolean;
  /** The first column: rendered as `<th scope="row">`. */
  readonly rowHeader?: boolean;
  /** Hidden on narrow viewports; its value is also in the entry's detail. */
  readonly secondary?: boolean;
  readonly cell: (row: Row) => ReactNode;
}

export interface SortState {
  readonly key: string;
  readonly direction: SortDirection;
}

/** What a click on a sortable header does to the sort. */
export function nextSort(current: SortState, key: string): SortState {
  return current.key === key ? { key, direction: flip(current.direction) } : { key, direction: 'ascending' };
}

/**
 * A data table with sortable columns and no library. A sortable header carries
 * `aria-sort` and contains a real button whose text names the column and the
 * direction in words; the arrow is decoration. Sorting is announced through the
 * polite region by the caller.
 */
export function SortableTable<Row extends SortableRow>({
  caption,
  captionId,
  columns,
  rows,
  initialSort,
  secondaryClassName,
  onSorted,
}: {
  caption: string;
  captionId: string;
  columns: readonly Column<Row>[];
  rows: readonly Row[];
  initialSort: SortState;
  secondaryClassName: string;
  onSorted: (message: string) => void;
}) {
  const [sort, setSort] = useState(initialSort);
  const sorted = sortRows(rows, sort.key, sort.direction);

  function choose(column: Column<Row>) {
    const next = nextSort(sort, column.key);
    setSort(next);
    onSorted(`Sorted by ${column.header}, ${next.direction}.`);
  }

  return (
    <div className={styles.scrollRegion} role="region" aria-labelledby={captionId} tabIndex={0}>
      <table className={styles.table}>
        <caption id={captionId}>{caption}</caption>
        <thead>
          <tr>
            {columns.map((column) => {
              const active = sort.key === column.key;
              const className = column.secondary === true ? secondaryClassName : undefined;
              if (column.sortable !== true) {
                return (
                  <th key={column.key} scope="col" className={className}>
                    {column.header}
                  </th>
                );
              }
              return (
                <th key={column.key} scope="col" className={className} aria-sort={active ? sort.direction : 'none'}>
                  <button className={styles.sortButton} type="button" onClick={() => choose(column)}>
                    {active ? `${column.header}, sorted ${sort.direction}` : `${column.header}, not sorted`}
                    <span aria-hidden="true">{active ? (sort.direction === 'ascending' ? ' ▲' : ' ▼') : ' ↕'}</span>
                  </button>
                </th>
              );
            })}
          </tr>
        </thead>
        <tbody>
          {sorted.map((row) => (
            <tr key={row.id}>
              {columns.map((column) =>
                column.rowHeader === true ? (
                  <th key={column.key} scope="row">
                    {column.cell(row)}
                  </th>
                ) : (
                  <td key={column.key} className={column.secondary === true ? secondaryClassName : undefined}>
                    {column.cell(row)}
                  </td>
                ),
              )}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
