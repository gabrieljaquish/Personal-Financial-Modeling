// The typed API client: `session/api.ts` stays the one transport; this wraps it
// with the generated `Operations` typing, a shallow runtime guard per success
// body and one side effect - a session lost mid-use is reported to the shell, so
// it flips the whole application and not just one screen.
//
// The guards are defect detectors, not validators: the server is the same binary
// as this bundle, so a body of the wrong shape means a build defect. They check
// that the top-level keys a screen reads exist with the right kind.

import { apiPost } from '../session/api.ts';
import type { ApiEnv, ApiFailureResult } from '../session/api.ts';
import { failureFrom, MALFORMED_BODY } from './errors.ts';
import type { ApiFailure } from './errors.ts';
import type {
  Accepted,
  AssumptionsResponse,
  Cents,
  ExportFormatDto,
  FilingStatusDto,
  Operations,
  RateScheduleResponse,
  SessionStatus,
} from './schema.gen.ts';

export type Outcome<T> = { ok: true; value: T } | { ok: false; failure: ApiFailure };

export type SessionLoss = 'displaced' | 'session-ended';

/** The inputs of the rate schedule, as the screen holds them after validation. */
export interface RateScheduleInputs {
  readonly year: number;
  readonly filingStatus: FilingStatusDto;
  readonly taxableIncome: Cents;
}

/** A file the server sent. `disposition` is the raw header and is NOT trusted: see `attachmentFilename`. */
export interface ExportedFile {
  readonly text: string;
  readonly disposition: string | null;
}

export interface ApiClient {
  status(): Promise<Outcome<SessionStatus>>;
  assumptions(): Promise<Outcome<AssumptionsResponse>>;
  rateSchedule(inputs: RateScheduleInputs): Promise<Outcome<RateScheduleResponse>>;
  /** The export as the text the server sent, never parsed here, and the `Content-Disposition` it came with. */
  exportRateSchedule(inputs: RateScheduleInputs, format: ExportFormatDto): Promise<Outcome<ExportedFile>>;
  relaunch(): Promise<Outcome<Accepted>>;
}

type JsonPath = { [P in keyof Operations]: Operations[P]['expect'] extends 'json' ? P : never }[keyof Operations];
type TextPath = Exclude<keyof Operations, JsonPath>;

type Kind = 'string' | 'boolean' | 'number' | 'array' | 'object';

function kindOf(value: unknown): Kind | 'other' {
  if (Array.isArray(value)) {
    return 'array';
  }
  if (value === null) {
    return 'other';
  }
  const type = typeof value;
  return type === 'string' || type === 'boolean' || type === 'number' || type === 'object' ? type : 'other';
}

/** Whether `body` is an object whose listed keys have the listed kinds. */
export function hasShape(body: unknown, shape: Readonly<Record<string, Kind>>): boolean {
  if (kindOf(body) !== 'object') {
    return false;
  }
  const record = body as Record<string, unknown>;
  return Object.entries(shape).every(([key, kind]) => kindOf(record[key]) === kind);
}

const SHAPES: { readonly [P in JsonPath]: Readonly<Record<string, Kind>> } = {
  '/api/v1/session/bootstrap': { proof: 'string' },
  '/api/v1/session/status': { appVersion: 'string', apiVersion: 'string', trustMode: 'string', vintages: 'array' },
  '/api/v1/session/relaunch': {},
  '/api/v1/assumptions/list': { vintages: 'array' },
  '/api/v1/tax/rate-schedule': {
    year: 'number',
    filingStatus: 'string',
    taxableIncome: 'number',
    tax: 'number',
    rootLineId: 'string',
    lines: 'array',
    vintage: 'object',
    verified: 'boolean',
  },
};

export function createClient(env: ApiEnv, onSessionLost: (loss: SessionLoss) => void): ApiClient {
  function fail(result: ApiFailureResult, report: boolean): { ok: false; failure: ApiFailure } {
    const failure = failureFrom(result);
    if (report && (failure.kind === 'displaced' || failure.kind === 'session-ended')) {
      onSessionLost(failure.kind);
    }
    return { ok: false, failure };
  }

  async function call<P extends JsonPath>(path: P, body: Operations[P]['request'], report = true): Promise<Outcome<Operations[P]['ok']>> {
    const result = await apiPost(env, path, body);
    if (result.kind !== 'ok') {
      return fail(result, report);
    }
    return hasShape(result.body, SHAPES[path]) ? { ok: true, value: result.body as Operations[P]['ok'] } : { ok: false, failure: MALFORMED_BODY };
  }

  async function callText<P extends TextPath>(path: P, body: Operations[P]['request']): Promise<Outcome<ExportedFile>> {
    const result = await apiPost(env, path, body, { expect: 'text' });
    return result.kind === 'ok' ? { ok: true, value: { text: result.text, disposition: result.disposition } } : fail(result, true);
  }

  return {
    status: () => call('/api/v1/session/status', undefined),
    assumptions: () => call('/api/v1/assumptions/list', undefined),
    rateSchedule: (inputs) => call('/api/v1/tax/rate-schedule', inputs),
    exportRateSchedule: (inputs, format) => callText('/api/v1/tax/rate-schedule/export', { ...inputs, format }),
    // A 401 here is an outcome of the recovery itself, shown beside its button;
    // it must not replace the displaced notice that offers the recovery.
    relaunch: () => call('/api/v1/session/relaunch', undefined, false),
  };
}
