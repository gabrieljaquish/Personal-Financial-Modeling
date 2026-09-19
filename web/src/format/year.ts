// A tax year typed by a person -> the integer the API takes. One of the three
// files in web/src allowed a numeric conversion (with `money.ts` and `ratio.ts`);
// the conversion is of four ASCII digits, so it cannot produce a fraction.

/** `null` unless the text is exactly four ASCII digits (surrounding space allowed). */
export function yearFromText(text: string): number | null {
  const trimmed = text.trim();
  return /^[0-9]{4}$/.test(trimmed) ? Number(trimmed) : null;
}
