// The two effects of the rate-schedule screen as plain async functions, so tests
// drive reducer -> effect -> reducer with a fake client and no browser.

import type { ApiClient, RateScheduleInputs } from '../../api/client.ts';
import type { ExportFormatDto } from '../../api/schema.gen.ts';
import type { AppEnv } from '../../env.ts';
import type { RsEvent } from './state.ts';

export const EXPORT_MIME: Readonly<Record<ExportFormatDto, string>> = {
  csv: 'text/csv;charset=utf-8',
  json: 'application/json',
};

/**
 * The ONE source of an export's filename is the `Content-Disposition` the server
 * sent (`crates/pfp-server/src/api/export.rs`). A header is still input, so the
 * name is accepted only in exactly the shape the server writes: a quoted, lowercase
 * `[a-z0-9-]` stem of at most 64 characters and the extension of the format that was
 * asked for. No path separator, dot-file, space, percent-escape or `filename*` form
 * can match. Anything else yields the fallback, which is a literal.
 */
const ATTACHMENT_PATTERN = /^attachment; filename="([a-z0-9](?:[a-z0-9-]{0,62}[a-z0-9])?)\.(csv|json)"$/;

export function fallbackFilename(format: ExportFormatDto): string {
  return `export.${format}`;
}

export function attachmentFilename(disposition: string | null, format: ExportFormatDto): string {
  const match = disposition === null ? null : ATTACHMENT_PATTERN.exec(disposition);
  if (match === null || match[2] !== format) {
    return fallbackFilename(format);
  }
  return `${match[1] ?? 'export'}.${format}`;
}

export async function performCalculation(client: ApiClient, inputs: RateScheduleInputs, request: number): Promise<RsEvent> {
  return { type: 'result', request, outcome: await client.rateSchedule(inputs) };
}

/**
 * Asks the server for the file and hands the text it sent to the browser,
 * untouched: the JSON export is never parsed and re-serialised here.
 */
export async function performExport(client: ApiClient, saveFile: AppEnv['saveFile'], inputs: RateScheduleInputs, format: ExportFormatDto): Promise<RsEvent> {
  const outcome = await client.exportRateSchedule(inputs, format);
  if (!outcome.ok) {
    return { type: 'export-result', outcome };
  }
  const filename = attachmentFilename(outcome.value.disposition, format);
  saveFile(outcome.value.text, EXPORT_MIME[format], filename);
  return { type: 'export-result', outcome: { ok: true, value: filename } };
}
