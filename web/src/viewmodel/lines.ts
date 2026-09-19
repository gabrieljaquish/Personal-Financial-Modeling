// The rate-schedule worksheet as display strings. The UI formats; it never
// computes: everything here is a lookup, a join or a position in the server's
// own order. The tax shown is `response.tax`, not a sum of lines.

import type { LineDto, ParamRefDto, RateScheduleResponse } from '../api/schema.gen.ts';
import { centsToText } from '../format/money.ts';
import { filingStatusLabel, roundingRuleLabel } from '../format/text.ts';
import { hrefFor } from '../router/hash.ts';
import { isLocked, unverifiedNotice } from './status.ts';

export interface ParamRefVm {
  /** `irs.ordinary_brackets - 2026 - mfj - top_of_10`: the id is always part of the text. */
  readonly text: string;
  /** `null` when the id cannot be addressed by a route: rendered as text, never a dead link. */
  readonly href: string | null;
}

export interface LineVm {
  /** 1-based position in the server's computation order. */
  readonly no: number;
  readonly id: string;
  readonly label: string;
  readonly amount: string;
  readonly isResult: boolean;
  /** "Line 2 - Taxable income taxed at 10%", one per input, in formula order. */
  readonly from: readonly string[];
  readonly params: readonly ParamRefVm[];
  readonly rounding: string | null;
}

export interface WorksheetVm {
  readonly caption: string;
  readonly year: string;
  readonly filingStatus: string;
  readonly filingStatusWire: string;
  readonly taxableIncome: string;
  readonly tax: string;
  readonly lines: readonly LineVm[];
  /** Present unless the vintage is verified: the sentence that must sit beside the number. */
  readonly notice: string | null;
  readonly announcement: string;
}

export function paramRefVm(ref: ParamRefDto): ParamRefVm {
  const parts = [ref.paramId, ref.year === undefined || ref.year === null ? null : String(ref.year), ref.breakdownKey ?? null, ref.element ?? null];
  return {
    text: parts.filter((part): part is string => part !== null).join(' - '),
    href: hrefFor({ screen: 'assumption', id: ref.paramId }),
  };
}

function lineVm(line: LineDto, no: number, rootLineId: string, positions: ReadonlyMap<string, { no: number; label: string }>): LineVm {
  return {
    no,
    id: line.id,
    label: line.label,
    amount: centsToText(line.value),
    isResult: line.id === rootLineId,
    from: (line.inputs ?? []).map((id) => {
      const target = positions.get(id);
      // An id that does not resolve is shown, never dropped.
      return target === undefined ? `${id} (not in this worksheet)` : `Line ${String(target.no)} - ${target.label}`;
    }),
    params: (line.params ?? []).map(paramRefVm),
    rounding: line.rounding === undefined || line.rounding === null ? null : roundingRuleLabel(line.rounding),
  };
}

export function worksheetVm(response: RateScheduleResponse): WorksheetVm {
  const positions = new Map<string, { no: number; label: string }>();
  for (const [index, line] of response.lines.entries()) {
    positions.set(line.id, { no: index + 1, label: line.label });
  }
  const year = String(response.year);
  const filingStatus = filingStatusLabel(response.filingStatus);
  const tax = centsToText(response.tax);
  const count = response.lines.length;
  return {
    caption: `Rate schedule worksheet, ${year}, ${filingStatus}`,
    year,
    filingStatus,
    filingStatusWire: response.filingStatus,
    taxableIncome: centsToText(response.taxableIncome),
    tax,
    lines: response.lines.map((line, index) => lineVm(line, index + 1, response.rootLineId, positions)),
    // Fail closed: the notice is withheld only for the boolean `true`, and the
    // vintage's own flag must agree.
    notice:
      response.verified === true && response.vintage.verified === true && isLocked(response.vintage)
        ? null
        : unverifiedNotice(response.vintage.contentId, { ...response.vintage, verified: response.verified === true && response.vintage.verified === true }),
    announcement: `Rate schedule calculated. Tax ${tax}. ${String(count)} ${count === 1 ? 'line' : 'lines'}.`,
  };
}
