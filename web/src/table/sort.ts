// The one sorter in the front end. It lives outside `viewmodel/`, `screens/` and
// `components/`, where comparison operators and `.toSorted(` are banned by the
// source lint, and it cannot name an amount: sort keys are strings and counts
// (`array.length`), never money. The server's order is the order of every
// worksheet; only the registry's summary tables are sortable.

export type SortDirection = 'ascending' | 'descending';

/** A sort key: text compared by UTF-16 code unit, or a count. */
export type SortValue = string | number;

export interface SortableRow {
  /** Ties break on this, ascending, so an order is total and stable across runs. */
  readonly id: string;
  readonly sortKeys: Readonly<Record<string, SortValue>>;
}

/** Code-unit comparison: right for ASCII ids and for ISO `YYYY-MM-DD` dates. */
export function compareText(a: string, b: string): number {
  return a < b ? -1 : a > b ? 1 : 0;
}

function compareValues(a: SortValue | undefined, b: SortValue | undefined): number {
  if (typeof a === 'number' && typeof b === 'number') {
    return a < b ? -1 : a > b ? 1 : 0;
  }
  return compareText(String(a ?? ''), String(b ?? ''));
}

/** A sorted copy; the input is not touched. */
export function sortRows<Row extends SortableRow>(rows: readonly Row[], key: string, direction: SortDirection): Row[] {
  const sign = direction === 'ascending' ? 1 : -1;
  return rows.toSorted((a, b) => {
    const byKey = compareValues(a.sortKeys[key], b.sortKeys[key]);
    return byKey !== 0 ? sign * byKey : compareText(a.id, b.id);
  });
}

/** A sorted copy of strings, by code unit (ISO dates and years sort correctly). */
export function sortText(items: readonly string[]): string[] {
  return items.toSorted(compareText);
}

export function flip(direction: SortDirection): SortDirection {
  return direction === 'ascending' ? 'descending' : 'ascending';
}
