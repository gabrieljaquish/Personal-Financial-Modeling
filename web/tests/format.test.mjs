// Formatting and validation. Money is integer cents in, text out; whole-dollar
// text in, integer cents out. No float exists at any point.

import assert from 'node:assert/strict';
import { test } from 'node:test';

import { centsToText, isInputError, UNREADABLE_AMOUNT, wholeDollarsToCents } from '../src/format/money.ts';
import { ratioToText } from '../src/format/ratio.ts';
import { filingStatusLabel, listInWords, roundingRuleLabel } from '../src/format/text.ts';
import { yearFromText } from '../src/format/year.ts';

test('centsToText: golden table', () => {
  const table = [
    [0, '$0.00'],
    [5, '$0.05'],
    [99, '$0.99'],
    [100, '$1.00'],
    [1150400, '$11,504.00'],
    [10000000, '$100,000.00'],
    [-1234, '−$12.34'],
    [-5, '−$0.05'],
    [123456789, '$1,234,567.89'],
    [9007199254740991, '$90,071,992,547,409.91'],
  ];
  for (const [cents, text] of table) {
    assert.equal(centsToText(cents), text);
  }
});

test('A13: an amount is one string - sign, dollar sign, grouped thousands, two decimals', () => {
  for (const cents of [0, 7, -7, 1150400, -9007199254740991]) {
    assert.match(centsToText(cents), /^−?\$[0-9]{1,3}(,[0-9]{3})*\.[0-9]{2}$/);
  }
});

test('centsToText refuses what is not a safe integer with a visible marker, never a guess', () => {
  for (const bad of [1.5, Number.NaN, Infinity, 9007199254740992, '100', null, undefined, {}]) {
    assert.equal(centsToText(bad), UNREADABLE_AMOUNT);
  }
});

test('wholeDollarsToCents accepts digits, commas in the right places, one leading dollar sign', () => {
  const table = [
    ['100000', 10000000],
    ['100,000', 10000000],
    ['$100000', 10000000],
    ['$1,000', 100000],
    ['000100', 10000],
    ['0', 0],
    ['000', 0],
    [' 42 ', 4200],
    ['90071992547409', 9007199254740900],
    ['90,071,992,547,409', 9007199254740900],
  ];
  for (const [text, cents] of table) {
    const result = wholeDollarsToCents(text);
    assert.equal(result, cents, text);
    assert.ok(Number.isSafeInteger(result));
  }
});

test('wholeDollarsToCents refuses everything else', () => {
  const refused = {
    empty: ['', '   '],
    'not-whole-dollars': ['1e5', '100.50', '100.00', '-1', '+1', '1,00', '1,0000', ',100', '100,', '1 000', '$$5', '$', '١٢٣', '１２３', 'abc', '0x10', '1_000', '=1+1'],
    'too-large': ['90071992547410', '100000000000000', '999999999999999999999'],
  };
  for (const [error, texts] of Object.entries(refused)) {
    for (const text of texts) {
      const result = wholeDollarsToCents(text);
      assert.ok(isInputError(result), text);
      assert.equal(result.error, error, text);
    }
  }
});

test('ratioToText moves a decimal point and divides nothing', () => {
  const table = [
    [{ num: 10, den: 100 }, '10%'],
    [{ num: 37, den: 100 }, '37%'],
    [{ num: 0, den: 100 }, '0%'],
    [{ num: 765, den: 10000 }, '7.65%'],
    [{ num: 145, den: 10000 }, '1.45%'],
    [{ num: 9, den: 1000 }, '0.9%'],
    [{ num: 5, den: 100000 }, '0.005%'],
    [{ num: 1, den: 10 }, '10%'],
    [{ num: 2, den: 1 }, '200%'],
    [{ num: 1, den: 3 }, '1/3'],
    [{ num: 1, den: 200 }, '1/200'],
    [{ num: -5, den: 100 }, '−5%'],
    [{ num: 1.5, den: 100 }, 'unreadable rate'],
  ];
  for (const [ratio, text] of table) {
    assert.equal(ratioToText(ratio), text, JSON.stringify(ratio));
  }
});

test('labels are lookups; an unknown wire value is shown verbatim', () => {
  assert.equal(filingStatusLabel('mfj'), 'Married filing jointly');
  assert.equal(filingStatusLabel('qss'), 'Qualifying surviving spouse');
  assert.equal(filingStatusLabel('new-status'), 'new-status');
  assert.equal(filingStatusLabel('constructor'), 'constructor');
  assert.match(roundingRuleLabel('irs.whole_dollar'), /whole dollar.*\(irs\.whole_dollar\)/);
  assert.equal(roundingRuleLabel('some.new_rule'), 'some.new_rule');
  assert.equal(listInWords([]), '');
  assert.equal(listInWords(['2026']), '2026');
  assert.equal(listInWords(['2025', '2026']), '2025 and 2026');
  assert.equal(listInWords(['a', 'b', 'c']), 'a, b and c');
});

test('a year is exactly four ASCII digits', () => {
  assert.equal(yearFromText('2026'), 2026);
  assert.equal(yearFromText(' 2026 '), 2026);
  for (const bad of ['', '26', '20266', '2o26', '2026.0', '-202', '２０２６', '20 6']) {
    assert.equal(yearFromText(bad), null, bad);
  }
});
