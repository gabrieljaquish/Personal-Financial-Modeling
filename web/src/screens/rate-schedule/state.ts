// The rate-schedule screen as a pure reducer. The container owns the effects; the
// view is a pure function of this state, so every state can be rendered and
// checked without a browser.
//
// Client-side validation is limited to "can a request be formed at all": a
// well-formed whole-dollar string, a four-digit year, one of the five statuses.
// What the numbers MEAN is the server's to judge; a range error comes back as 422
// and is shown with this UI's own words for that code.
// (A recorded departure from ARCHITECTURE.md §5: docs/contributing.md §8 item 8.)

import type { Announcements } from '../../announce.ts';
import type { Outcome, RateScheduleInputs } from '../../api/client.ts';
import { failureText } from '../../api/errors.ts';
import type { ApiFailure } from '../../api/errors.ts';
import { FilingStatusDtoValues } from '../../api/schema.gen.ts';
import type { ExportFormatDto, FilingStatusDto, RateScheduleResponse } from '../../api/schema.gen.ts';
import { isInputError, wholeDollarsToCents } from '../../format/money.ts';
import { yearFromText } from '../../format/year.ts';
import { worksheetVm } from '../../viewmodel/lines.ts';
import type { WorksheetVm } from '../../viewmodel/lines.ts';

export const FIELD_IDS = { status: 'rs-status', year: 'rs-year', income: 'rs-income' } as const;
export const ERROR_SUMMARY_ID = 'rs-errors';
/** The heading of the result section: the focus target when "Try again" is unmounted. */
export const RESULT_HEADING_ID = 'rs-result-heading';

export type FieldName = keyof typeof FIELD_IDS;
export type Fields = Readonly<Record<FieldName, string>>;

export interface FieldError {
  readonly field: FieldName;
  /** Says what to type. Never echoes the rejected value. */
  readonly message: string;
}

export type Phase = { kind: 'idle' } | { kind: 'loading' } | { kind: 'ready'; vm: WorksheetVm } | { kind: 'failed'; failure: ApiFailure };

export type ExportPhase =
  | { kind: 'idle' }
  | { kind: 'working'; format: ExportFormatDto }
  | { kind: 'done'; filename: string }
  | { kind: 'failed'; failure: ApiFailure };

export interface RsState {
  readonly fields: Fields;
  readonly errors: readonly FieldError[];
  /** The server refused well-formed inputs (422): shown in the error summary. */
  readonly refusal: string | null;
  readonly phase: Phase;
  /** Counts submissions, so a newer one supersedes an older answer. */
  readonly request: number;
  /** What the request in flight was formed from. */
  readonly pending: RateScheduleInputs | null;
  /** The inputs of the result on screen: what an export re-sends. */
  readonly lastInputs: RateScheduleInputs | null;
  readonly exportPhase: ExportPhase;
  /** Bumped whenever the error summary must receive focus. */
  readonly focusSeq: number;
  /**
   * Bumped whenever the result heading must receive focus: "Try again" started a
   * request, so the failure alert - and the button that holds focus - is unmounted.
   */
  readonly resultFocusSeq: number;
}

export type RsEvent =
  | { type: 'change'; field: FieldName; value: string }
  | { type: 'submit' }
  | { type: 'retry' }
  | { type: 'result'; request: number; outcome: Outcome<RateScheduleResponse> }
  | { type: 'export-started'; format: ExportFormatDto }
  /** On success the value is the name the file was saved under: the server's, validated (see `attachmentFilename`). */
  | { type: 'export-result'; outcome: Outcome<string> };

export const INITIAL_RS_STATE: RsState = {
  // Every field starts empty: no default status, year or amount.
  fields: { status: '', year: '', income: '' },
  errors: [],
  refusal: null,
  phase: { kind: 'idle' },
  request: 0,
  pending: null,
  lastInputs: null,
  exportPhase: { kind: 'idle' },
  focusSeq: 0,
  resultFocusSeq: 0,
};

function isFilingStatus(value: string): value is FilingStatusDto {
  return (FilingStatusDtoValues as readonly string[]).includes(value);
}

/** Either a request that can be sent, or what to fix. */
export function validate(fields: Fields): { inputs: RateScheduleInputs } | { errors: FieldError[] } {
  const errors: FieldError[] = [];
  const status = fields.status;
  if (!isFilingStatus(status)) {
    errors.push({ field: 'status', message: 'Choose a filing status.' });
  }
  const year = yearFromText(fields.year);
  if (year === null) {
    errors.push({ field: 'year', message: 'Enter the tax year as four digits, for example 2026.' });
  }
  const income = wholeDollarsToCents(fields.income);
  if (isInputError(income)) {
    errors.push({
      field: 'income',
      message:
        income.error === 'empty'
          ? 'Enter the taxable income in whole dollars.'
          : income.error === 'too-large'
            ? 'Enter a smaller taxable income; this amount is larger than the application can hold exactly.'
            : 'Enter the taxable income in whole dollars, using digits only. Commas are allowed; cents, signs and letters are not.',
    });
  }
  if (errors.length !== 0 || !isFilingStatus(status) || year === null || isInputError(income)) {
    return { errors };
  }
  return { inputs: { year, filingStatus: status, taxableIncome: income } };
}

function submit(state: RsState): RsState {
  if (state.phase.kind === 'loading') {
    // The button is `aria-disabled` while loading; its clicks are ignored.
    return state;
  }
  const checked = validate(state.fields);
  if ('errors' in checked) {
    return { ...state, errors: checked.errors, refusal: null, focusSeq: state.focusSeq + 1 };
  }
  return {
    ...state,
    errors: [],
    refusal: null,
    phase: { kind: 'loading' },
    request: state.request + 1,
    pending: checked.inputs,
    exportPhase: { kind: 'idle' },
  };
}

export function rsReducer(state: RsState, event: RsEvent): RsState {
  switch (event.type) {
    case 'change':
      return { ...state, fields: { ...state.fields, [event.field]: event.value } };
    case 'submit':
      // Focus is on Calculate or in a field, and both survive: nothing to move.
      return submit(state);
    case 'retry': {
      const next = submit(state);
      // "Try again" lives inside the failure alert's block, which the loading state
      // unmounts. If the form no longer validates, the summary takes focus instead.
      return state.phase.kind === 'failed' && next.phase.kind === 'loading' ? { ...next, resultFocusSeq: state.resultFocusSeq + 1 } : next;
    }
    case 'result': {
      if (event.request !== state.request || state.phase.kind !== 'loading') {
        return state;
      }
      if (event.outcome.ok) {
        return { ...state, phase: { kind: 'ready', vm: worksheetVm(event.outcome.value) }, lastInputs: state.pending, pending: null };
      }
      const { failure } = event.outcome;
      if (failure.kind === 'input-refused') {
        return { ...state, phase: { kind: 'idle' }, pending: null, lastInputs: null, refusal: failureText(failure), focusSeq: state.focusSeq + 1 };
      }
      return { ...state, phase: { kind: 'failed', failure }, pending: null, lastInputs: null };
    }
    case 'export-started':
      return state.lastInputs === null || state.exportPhase.kind === 'working' ? state : { ...state, exportPhase: { kind: 'working', format: event.format } };
    case 'export-result':
      if (state.exportPhase.kind !== 'working') {
        return state;
      }
      return { ...state, exportPhase: event.outcome.ok ? { kind: 'done', filename: event.outcome.value } : { kind: 'failed', failure: event.outcome.failure } };
  }
}

export function hasProblems(state: RsState): boolean {
  return state.errors.length !== 0 || state.refusal !== null;
}

/** One message, one channel (see `announce.ts`). */
export function rsAnnouncements(state: RsState): Announcements {
  if (hasProblems(state)) {
    // Focus alone announces the summary: it is a named group, not a live region.
    return { focus: ERROR_SUMMARY_ID };
  }
  // `resultFocusSeq` is not declared here: it is a transition, not a state. The
  // heading it focuses says "Result"; the polite region says "Calculating…".
  if (state.exportPhase.kind === 'failed') {
    return { alert: failureText(state.exportPhase.failure) };
  }
  if (state.exportPhase.kind === 'done') {
    return { polite: `The file ${state.exportPhase.filename} was handed to your browser to save.` };
  }
  if (state.exportPhase.kind === 'working') {
    return { polite: 'Preparing the file…' };
  }
  switch (state.phase.kind) {
    case 'idle':
      return {};
    case 'loading':
      return { polite: 'Calculating…' };
    case 'ready':
      return { polite: state.phase.vm.announcement };
    case 'failed':
      return { alert: failureText(state.phase.failure) };
  }
}
