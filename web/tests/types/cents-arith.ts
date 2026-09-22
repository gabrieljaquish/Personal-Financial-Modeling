// NOT part of the build and NOT expected to compile. tests/cents-types.test.mjs
// runs `tsc` on this file and asserts the exact diagnostics, line by line.
//
// What it records, as an executable fact:
//   * arithmetic on the opaque `Cents` is a type error (TS2365 for `+` and for a
//     comparison against a number, TS2362/TS2363 for `- * /`);
//   * a relational operator between TWO `Cents` values is NOT a type error.
//     TypeScript has no type that makes it one, so that hole is closed by the
//     source lint in tests/no-storage.test.mjs (relational ban + `Cents`
//     confinement). If a future compiler closes it, this test notices.

import type { Cents } from '../../src/api/schema.gen.ts';

declare const a: Cents;
declare const b: Cents;

export const sum = a + b; // expect: TS2365
export const difference = a - b; // expect: TS2362
export const product = a * b; // expect: TS2362
export const quotient = a / b; // expect: TS2362
export const plusOne = a + 1; // expect: TS2365
export const belowOne = a < 1; // expect: TS2365
export const asNumber: number = a; // expect: TS2322

export const less = a < b; // expect: none
export const greater = a > b; // expect: none
export const lessOrEqual = a <= b; // expect: none
export const greaterOrEqual = a >= b; // expect: none
