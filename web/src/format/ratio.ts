// A rate crosses the API as an exact fraction, as the source printed it (`10/100`).
// It is shown by moving a decimal point inside a string; nothing is divided.

import type { RatioDto } from '../api/schema.gen.ts';

/** How many zeros follow the leading 1 of a power of ten, or `null`. */
function zerosOfPowerOfTen(den: number): number | null {
  const text = String(den);
  return /^10*$/.test(text) ? text.length - 1 : null;
}

/** `num / 10^places` as a decimal string, by shifting digits. */
function shift(num: number, places: number): string {
  const text = String(num);
  const negative = text.startsWith('-');
  const digits = (negative ? text.slice(1) : text).padStart(places + 1, '0');
  const whole = digits.slice(0, digits.length - places);
  const fraction = places === 0 ? '' : digits.slice(-places).replace(/0+$/, '');
  return `${negative ? '−' : ''}${whole}${fraction === '' ? '' : `.${fraction}`}`;
}

/**
 * `{10, 100}` -> `10%`; `{765, 10000}` -> `7.65%`; `{1, 10}` -> `10%`;
 * anything whose denominator is not a power of ten -> `num/den`, verbatim.
 */
export function ratioToText(ratio: RatioDto): string {
  const { num, den } = ratio;
  if (!Number.isSafeInteger(num) || !Number.isSafeInteger(den)) {
    return 'unreadable rate';
  }
  const zeros = zerosOfPowerOfTen(den);
  if (zeros === null) {
    return `${String(num)}/${String(den)}`;
  }
  // A percentage is the fraction with the point moved two places right.
  return zeros >= 2 ? `${shift(num, zeros - 2)}%` : `${shift(num, 0)}${'0'.repeat(2 - zeros)}%`;
}
