import assert from 'node:assert/strict';
import { test } from 'node:test';

import { nextSort } from '../src/components/SortableTable.tsx';
import { compareText, flip, sortRows, sortText } from '../src/table/sort.ts';

const rows = [
  { id: 'b', sortKeys: { id: 'b', asOf: '2025-10-09', verification: 0, openItems: 10 } },
  { id: 'a', sortKeys: { id: 'a', asOf: '2026-01-01', verification: 4, openItems: 2 } },
  { id: 'c', sortKeys: { id: 'c', asOf: '2025-10-09', verification: 0, openItems: 9 } },
];
const ids = (sorted) => sorted.map((row) => row.id).join('');

test('text keys sort by code unit, counts as numbers, ties break on id ascending', () => {
  assert.equal(ids(sortRows(rows, 'id', 'ascending')), 'abc');
  assert.equal(ids(sortRows(rows, 'id', 'descending')), 'cba');
  assert.equal(ids(sortRows(rows, 'asOf', 'ascending')), 'bca');
  assert.equal(ids(sortRows(rows, 'asOf', 'descending')), 'abc');
  // 9 before 10: a count is a number, not text.
  assert.equal(ids(sortRows(rows, 'openItems', 'ascending')), 'acb');
  // Pending (rank 0) leads; the tie breaks on id whatever the direction.
  assert.equal(ids(sortRows(rows, 'verification', 'ascending')), 'bca');
  assert.equal(ids(sortRows(rows, 'verification', 'descending')), 'abc');
  assert.equal(ids(sortRows(rows, 'no-such-key', 'ascending')), 'abc');
});

test('sorting copies; the input keeps the server order', () => {
  const before = ids(rows);
  sortRows(rows, 'id', 'ascending');
  assert.equal(ids(rows), before);
});

test('helpers', () => {
  assert.deepEqual(sortText(['2026', '2025', '2025']), ['2025', '2025', '2026']);
  assert.equal(compareText('a', 'b'), -1);
  assert.equal(flip('ascending'), 'descending');
  assert.deepEqual(nextSort({ key: 'id', direction: 'ascending' }, 'id'), { key: 'id', direction: 'descending' });
  assert.deepEqual(nextSort({ key: 'id', direction: 'descending' }, 'asOf'), { key: 'asOf', direction: 'ascending' });
});
