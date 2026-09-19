import { FilingStatusDtoValues } from '../../api/schema.gen.ts';
import type { ExportFormatDto } from '../../api/schema.gen.ts';
import { ExportActions } from '../../components/ExportActions.tsx';
import { FailureAlert } from '../../components/FailureAlert.tsx';
import { FormErrorSummary } from '../../components/FormErrorSummary.tsx';
import type { SummaryItem } from '../../components/FormErrorSummary.tsx';
import { WorksheetTable } from '../../components/WorksheetTable.tsx';
import formStyles from '../../components/Form.module.css';
import statusStyles from '../../components/Status.module.css';
import { filingStatusLabel, listInWords } from '../../format/text.ts';
import type { WorksheetVm } from '../../viewmodel/lines.ts';
import { ScreenHeading } from '../ScreenHeading.tsx';
import screenStyles from '../Screen.module.css';
import styles from './RateSchedule.module.css';
import { ERROR_SUMMARY_ID, FIELD_IDS, RESULT_HEADING_ID, hasProblems } from './state.ts';
import type { FieldName, RsState } from './state.ts';

export interface RateScheduleHandlers {
  onChange(field: FieldName, value: string): void;
  onSubmit(): void;
  onRetry(): void;
  onJump(inputId: string): void;
  onExport(format: ExportFormatDto): void;
  onPrint(): void;
}

const FIELD_LABELS: Readonly<Record<FieldName, string>> = {
  status: 'Filing status',
  year: 'Tax year',
  income: 'Taxable income, whole dollars',
};

/** What follows a server refusal: one general instruction. The server names no field, so neither does this. */
export const REFUSAL_GUIDANCE = 'The application does not say which of the three values it refused. Review all three, then choose Calculate again.';

/**
 * One entry per field the CLIENT found wrong - each of those fields carries
 * `aria-invalid` and its own message, so a jump lands on a field that says what is
 * wrong. A server refusal (422) identifies no field: it gets no entries, because a
 * "Check: <field>" jump would imply a field-level error nobody established.
 */
function summaryItems(state: RsState): SummaryItem[] {
  return state.errors.map((error) => ({ inputId: FIELD_IDS[error.field], text: error.message }));
}

/** `aria-describedby` and `aria-invalid` for one field: hint first, then the error. */
function described(state: RsState, field: FieldName, hintId: string | null) {
  const error = state.errors.find((candidate) => candidate.field === field);
  const ids = [hintId, error === undefined ? null : `${FIELD_IDS[field]}-error`].filter((id) => id !== null);
  return {
    error,
    props: {
      'aria-describedby': ids.length === 0 ? undefined : ids.join(' '),
      'aria-invalid': error === undefined ? undefined : ('true' as const),
    },
  };
}

function FieldError({ id, message }: { id: string; message: string | undefined }) {
  return message === undefined ? null : (
    <p className={formStyles.fieldError} id={id}>
      <span aria-hidden="true">! </span>
      {message}
    </p>
  );
}

function Form({ state, publishedYears, handlers }: { state: RsState; publishedYears: readonly string[]; handlers: RateScheduleHandlers }) {
  const loading = state.phase.kind === 'loading';
  const status = described(state, 'status', null);
  const year = described(state, 'year', publishedYears.length === 0 ? null : 'rs-year-hint');
  const income = described(state, 'income', 'rs-income-hint');
  return (
    <form
      className={styles.form}
      noValidate
      onSubmit={(event) => {
        // Always: the Content-Security-Policy forbids a real submission anyway.
        event.preventDefault();
        handlers.onSubmit();
      }}
    >
      <div className={formStyles.field}>
        <label htmlFor={FIELD_IDS.status}>{FIELD_LABELS.status}</label>
        <select id={FIELD_IDS.status} required value={state.fields.status} onChange={(event) => handlers.onChange('status', event.target.value)} {...status.props}>
          <option value="" disabled>
            Choose a filing status
          </option>
          {FilingStatusDtoValues.map((wire) => (
            <option key={wire} value={wire}>
              {filingStatusLabel(wire)}
            </option>
          ))}
        </select>
        <FieldError id={`${FIELD_IDS.status}-error`} message={status.error?.message} />
      </div>

      <div className={formStyles.field}>
        <label htmlFor={FIELD_IDS.year}>{FIELD_LABELS.year}</label>
        {publishedYears.length === 0 ? null : (
          <p className={formStyles.hint} id="rs-year-hint">
            {`This build publishes parameters for ${listInWords(publishedYears)}.`}
          </p>
        )}
        <input
          id={FIELD_IDS.year}
          type="text"
          inputMode="numeric"
          autoComplete="off"
          maxLength={4}
          required
          value={state.fields.year}
          onChange={(event) => handlers.onChange('year', event.target.value)}
          {...year.props}
        />
        <FieldError id={`${FIELD_IDS.year}-error`} message={year.error?.message} />
      </div>

      <div className={formStyles.field}>
        <label htmlFor={FIELD_IDS.income}>{FIELD_LABELS.income}</label>
        <p className={formStyles.hint} id="rs-income-hint">
          Whole dollars, digits only; commas are allowed. Example: 100000 (a synthetic figure).
        </p>
        <input
          id={FIELD_IDS.income}
          type="text"
          inputMode="numeric"
          autoComplete="off"
          required
          value={state.fields.income}
          onChange={(event) => handlers.onChange('income', event.target.value)}
          {...income.props}
        />
        <FieldError id={`${FIELD_IDS.income}-error`} message={income.error?.message} />
      </div>

      {/* `aria-disabled`, not `disabled`: a disabled button drops focus. The reducer ignores the click. */}
      <button className={formStyles.submit} type="submit" aria-disabled={loading ? 'true' : undefined}>
        Calculate
      </button>
    </form>
  );
}

function Result({ vm, exportBusy, handlers }: { vm: WorksheetVm; exportBusy: boolean; handlers: RateScheduleHandlers }) {
  return (
    <>
      <p className={styles.headline}>{`Tax: ${vm.tax}`}</p>
      {vm.notice === null ? null : (
        <p className={statusStyles.unverifiedNotice}>
          <span aria-hidden="true">△ </span>
          {vm.notice}
        </p>
      )}
      <h4 className={styles.subheading}>Inputs, as the application read them</h4>
      <dl className={styles.inputs}>
        <div>
          <dt>Filing status</dt>
          <dd>{vm.filingStatus}</dd>
        </div>
        <div>
          <dt>Tax year</dt>
          <dd>{vm.year}</dd>
        </div>
        <div>
          <dt>Taxable income</dt>
          <dd>{vm.taxableIncome}</dd>
        </div>
      </dl>
      <WorksheetTable caption={vm.caption} captionId="rs-worksheet-caption" lines={vm.lines} />
      <div className={styles.actions}>
        <ExportActions busy={exportBusy} onExport={handlers.onExport} onPrint={handlers.onPrint} />
      </div>
    </>
  );
}

/** The whole screen as a pure function of its state. */
export function RateScheduleView({ state, publishedYears, handlers }: { state: RsState; publishedYears: readonly string[]; handlers: RateScheduleHandlers }) {
  const { phase } = state;
  return (
    <section className={screenStyles.screen} aria-labelledby="screen-heading">
      <ScreenHeading>Rate schedule</ScreenHeading>
      <p className={screenStyles.lede}>
        The ordinary-income tax for one filing status, tax year and taxable income, with every line of the computation and the parameters it read.
      </p>

      {hasProblems(state) ? <FormErrorSummary id={ERROR_SUMMARY_ID} intro={state.refusal === null ? null : `${state.refusal} ${REFUSAL_GUIDANCE}`} items={summaryItems(state)} onJump={handlers.onJump} /> : null}

      <Form state={state} publishedYears={publishedYears} handlers={handlers} />

      <section className={styles.result} aria-labelledby={RESULT_HEADING_ID} aria-busy={phase.kind === 'loading' ? 'true' : undefined}>
        {/* Focusable by script only: where focus goes when "Try again" is unmounted. A sibling of the alert, never around it. */}
        <h3 className={styles.resultHeading} id={RESULT_HEADING_ID} tabIndex={-1}>
          Result
        </h3>
        {phase.kind === 'idle' ? <p className={screenStyles.muted}>No result yet. Enter the three values and choose Calculate.</p> : null}
        {phase.kind === 'loading' ? <p className={screenStyles.muted}>Calculating…</p> : null}
        {phase.kind === 'failed' ? <FailureAlert failure={phase.failure} onRetry={handlers.onRetry} /> : null}
        {phase.kind === 'ready' ? <Result vm={phase.vm} exportBusy={state.exportPhase.kind === 'working'} handlers={handlers} /> : null}
        {state.exportPhase.kind === 'failed' ? <FailureAlert failure={state.exportPhase.failure} onRetry={null} /> : null}
      </section>
    </section>
  );
}
