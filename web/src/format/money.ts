// Money crosses the API as integer cents (`Cents`, opaque by design). This file is
// the ONLY place an amount becomes text or text becomes an amount, and the only
// file in web/src allowed to compare digits or to call `Number(`.
//
// Nothing here computes: formatting is `String(n)`, padding, slicing and inserting
// separators; parsing is pattern matching on digits followed by one conversion of
// an all-digit string already proven to be a safe integer. No float exists at any
// point, and nothing is rounded.

import type { Cents } from '../api/schema.gen.ts';

/** Shown in place of an amount that is not a safe integer. Never a guess. */
export const UNREADABLE_AMOUNT = 'unreadable amount';

/** U+2212 MINUS SIGN: read as "minus" by screen readers, unlike a hyphen. */
const MINUS = '−';

/** 2^53 - 1 cents, in whole dollars: the largest income the API accepts. */
const MAX_WHOLE_DOLLARS = '90071992547409';

function group(digits: string): string {
  return digits.replace(/\B(?=(?:[0-9]{3})+$)/g, ',');
}

/**
 * `1150400` -> `$11,504.00`; `-1234` -> `−$12.34`. The amount and its sign are one
 * string, so they land in one text node and are read as one figure.
 */
export function centsToText(amount: Cents): string {
  const n: unknown = amount;
  if (typeof n !== 'number' || !Number.isSafeInteger(n)) {
    return UNREADABLE_AMOUNT;
  }
  const text = String(n);
  const negative = text.startsWith('-');
  const digits = (negative ? text.slice(1) : text).padStart(3, '0');
  const dollars = digits.slice(0, -2);
  const cents = digits.slice(-2);
  return `${negative ? MINUS : ''}$${group(dollars)}.${cents}`;
}

export type InputError = { error: 'empty' | 'not-whole-dollars' | 'too-large' };

/**
 * Whole dollars typed by a person -> `Cents`. Accepts digits, an optional leading
 * `$` and well-placed commas; refuses decimals, signs, exponents, inner spaces and
 * non-ASCII digits. The server stays the validator of meaning.
 */
export function wholeDollarsToCents(text: string): Cents | InputError {
  const trimmed = text.trim();
  if (trimmed === '') {
    return { error: 'empty' };
  }
  const unsigned = trimmed.startsWith('$') ? trimmed.slice(1) : trimmed;
  if (!/^[0-9]{1,3}(,[0-9]{3})+$/.test(unsigned) && !/^[0-9]+$/.test(unsigned)) {
    return { error: 'not-whole-dollars' };
  }
  const digits = unsigned.replaceAll(',', '').replace(/^0+(?=[0-9])/, '');
  // Same-length strings of digits compare as the numbers they spell.
  if (digits.length > MAX_WHOLE_DOLLARS.length || (digits.length === MAX_WHOLE_DOLLARS.length && digits > MAX_WHOLE_DOLLARS)) {
    return { error: 'too-large' };
  }
  const cents = Number(`${digits}00`);
  if (!Number.isSafeInteger(cents)) {
    return { error: 'too-large' };
  }
  return cents as unknown as Cents;
}

export function isInputError(value: Cents | InputError): value is InputError {
  return typeof value === 'object' && value !== null && 'error' in value;
}
