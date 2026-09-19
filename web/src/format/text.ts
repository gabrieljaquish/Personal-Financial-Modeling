// Fixed label tables. Lookups only; an unknown wire value is shown verbatim
// rather than dropped or guessed at.

import type { FilingStatusDto } from '../api/schema.gen.ts';

const FILING_STATUS: Readonly<Record<FilingStatusDto, string>> = {
  single: 'Single',
  mfj: 'Married filing jointly',
  mfs: 'Married filing separately',
  hoh: 'Head of household',
  qss: 'Qualifying surviving spouse',
};

export function filingStatusLabel(wire: string): string {
  return Object.hasOwn(FILING_STATUS, wire) ? FILING_STATUS[wire as FilingStatusDto] : wire;
}

const ROUNDING_RULES: Readonly<Record<string, string>> = {
  'money.cent_half_even': 'Rounded to the cent, half to even',
  'irs.whole_dollar': 'Rounded once to the whole dollar, half up',
};

/** A plain description for a known rounding rule id; the id itself otherwise. */
export function roundingRuleLabel(id: string): string {
  return Object.hasOwn(ROUNDING_RULES, id) ? `${ROUNDING_RULES[id] ?? id} (${id})` : id;
}

const DIRECTIONS: Readonly<Record<string, string>> = {
  down: 'down',
  up: 'up',
  halfUp: 'half up',
  halfEven: 'half to even',
  nearest: 'to the nearest',
};

export function roundingDirectionLabel(wire: string): string {
  return Object.hasOwn(DIRECTIONS, wire) ? (DIRECTIONS[wire] ?? wire) : wire;
}

const BASES: Readonly<Record<string, string>> = {
  Amount: 'the amount itself',
  IncreaseOverBase: 'the increase over the statutory base amount',
};

export function roundingBasisLabel(wire: string): string {
  return Object.hasOwn(BASES, wire) ? (BASES[wire] ?? wire) : wire;
}

/** `a`, `a and b`, `a, b and c`. */
export function listInWords(items: readonly string[]): string {
  if (items.length <= 1) {
    return items.join('');
  }
  return `${items.slice(0, -1).join(', ')} and ${items[items.length - 1] ?? ''}`;
}
