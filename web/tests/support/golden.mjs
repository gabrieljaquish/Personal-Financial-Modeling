// The golden API bodies: exactly what the server sends for one synthetic input,
// recorded by `crates/pfp-server/tests/ui_golden.rs` as insta snapshots. Rendering
// the views from these means a DTO change that would break a screen fails a test
// here instead of a user session.

import { readFileSync } from 'node:fs';
import { join } from 'node:path';

const SNAPSHOTS = join(import.meta.dirname, '..', '..', '..', 'crates', 'pfp-server', 'tests', 'snapshots');

/** The body of an insta snapshot: everything after the second `---` line. */
export function snapshotBody(text) {
  const lines = text.split('\n');
  if (lines[0] !== '---') {
    throw new Error('not an insta snapshot: no front matter');
  }
  const end = lines.indexOf('---', 1);
  if (end === -1) {
    throw new Error('not an insta snapshot: unterminated front matter');
  }
  return lines.slice(end + 1).join('\n');
}

function read(name) {
  return snapshotBody(readFileSync(join(SNAPSHOTS, name), 'utf8')).replace(/\n+$/, '');
}

export const OPENAPI_SNAPSHOT = join(SNAPSHOTS, 'openapi__openapi_v1.snap');

export const openapi = () => JSON.parse(read('openapi__openapi_v1.snap'));
export const sessionStatus = () => JSON.parse(read('ui_golden__ui_golden_session_status.snap'));
export const assumptions = () => JSON.parse(read('ui_golden__ui_golden_assumptions_list.snap'));
export const rateSchedule = () => JSON.parse(read('ui_golden__ui_golden_rate_schedule.snap'));

/** The JSON export as sent: pretty-printed, one trailing newline. */
export const rateScheduleJsonExport = () => `${read('ui_golden__ui_golden_rate_schedule_json.snap')}\n`;

/** The CSV export as sent. The snapshot draws each carriage return as U+240D. */
export function rateScheduleCsvExport() {
  const drawn = read('ui_golden__ui_golden_rate_schedule_csv.snap');
  if (!drawn.endsWith('␍')) {
    throw new Error('the CSV snapshot does not end with a record terminator');
  }
  return `${drawn.replaceAll('␍', '\r')}\n`;
}
