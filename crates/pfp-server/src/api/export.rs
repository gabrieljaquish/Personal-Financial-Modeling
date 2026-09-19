//! CSV and JSON export with **type-aware** spreadsheet formula-injection escaping
//! (`SECURITY.md` §9, test id S-12).
//!
//! Formula injection is a CSV-*consumption* problem: a spreadsheet that opens a
//! cell beginning with `=` evaluates it. A blanket "prefix anything that starts
//! with `-`" rule would turn every negative amount into text and make a summed
//! column silently wrong, so the rule is written against the **kind** of a cell,
//! which the exporter knows because the cell is built from a typed value:
//!
//! | Kind | CSV emission |
//! |---|---|
//! | [`Cell::Cents`] | bare, never quoted, never prefixed: `11504.00`, `-12.34` |
//! | [`Cell::Int`] | bare decimal integer |
//! | [`Cell::Date`] | bare `YYYY-MM-DD` |
//! | [`Cell::Text`] | always quoted; `"` doubled; one apostrophe first when [`needs_escape`] |
//!
//! There is no "unknown" kind and no `Ratio` kind: a bare `10/100` is coerced to
//! a date by spreadsheets, so the textual form of a ratio needs a decision before
//! the first export that carries one.
//!
//! Records end with CRLF (RFC 4180). The encoding is UTF-8 without a byte-order
//! mark. The JSON export is exempt from all of this: values are emitted verbatim.
//!
//! An export **fails closed**. The body is built first and is fallible
//! ([`rate_schedule_attachment`]); the success headers exist only inside the
//! [`Attachment`] a successful build returns, so a serialization error can never be
//! answered as `200` with an empty file.

use std::borrow::Cow;
use std::fmt::Write as _;

use pfp_params::DateYmd;
use serde::Serialize;

use super::dto::{ExportFormatDto, FilingStatusDto, ParamRefDto, RateScheduleResponse};

/// One CSV cell, by the kind of value it carries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Cell<'a> {
    /// Money in integer cents; emitted as a bare decimal number of dollars.
    Cents(i64),
    /// A count, a year or a line number.
    Int(i64),
    /// A calendar date.
    Date(DateYmd),
    /// Anything else: labels, ids, enum wire forms, yes/no words.
    Text(Cow<'a, str>),
}

impl<'a> Cell<'a> {
    /// A borrowed text cell.
    #[must_use]
    pub const fn text(s: &'a str) -> Self {
        Self::Text(Cow::Borrowed(s))
    }

    /// `yes` or `no`, as text.
    #[must_use]
    pub const fn yes_no(flag: bool) -> Self {
        Self::text(if flag { "yes" } else { "no" })
    }
}

const fn is_trigger(c: char) -> bool {
    matches!(c, '=' | '+' | '-' | '@' | '\t' | '\r')
}

/// Whether a text cell gets a leading apostrophe inside its quotes.
///
/// True when any of:
/// 1. the first character is a tab or a carriage return (`SECURITY.md` §9 as
///    written; both are themselves whitespace, so they are tested before rule 2
///    strips anything);
/// 2. the first character **after any leading run of** Unicode whitespace or
///    U+FEFF is `=`, `+`, `-`, `@`, tab or CR — whether an importer trims leading
///    whitespace before deciding a cell is a formula varies by product and import
///    path, so the rule does not rest on it;
/// 3. the first character is `'` — so that every cell that starts with an
///    apostrophe was produced by this escape and the inverse is exactly "drop one
///    leading apostrophe".
#[must_use]
pub fn needs_escape(s: &str) -> bool {
    let Some(first) = s.chars().next() else {
        return false;
    };
    if first == '\t' || first == '\r' || first == '\'' {
        return true;
    }
    s.trim_start_matches(|c: char| c.is_whitespace() || c == '\u{FEFF}')
        .chars()
        .next()
        .is_some_and(is_trigger)
}

fn push_cents(out: &mut String, cents: i64) {
    // `unsigned_abs` so that `i64::MIN` has a magnitude.
    let magnitude = cents.unsigned_abs();
    if cents < 0 {
        out.push('-');
    }
    let _ = write!(out, "{}.{:02}", magnitude / 100, magnitude % 100);
}

fn push_text(out: &mut String, text: &str) {
    out.push('"');
    if needs_escape(text) {
        out.push('\'');
    }
    for c in text.chars() {
        if c == '"' {
            out.push('"');
        }
        out.push(c);
    }
    out.push('"');
}

fn push_record(out: &mut String, cells: &[Cell<'_>]) {
    for (at, cell) in cells.iter().enumerate() {
        if at > 0 {
            out.push(',');
        }
        match cell {
            Cell::Cents(c) => push_cents(out, *c),
            Cell::Int(i) => {
                let _ = write!(out, "{i}");
            }
            Cell::Date(d) => {
                let _ = write!(out, "{d}");
            }
            Cell::Text(t) => push_text(out, t),
        }
    }
    out.push_str("\r\n");
}

/// A CSV document: a header row of text cells, then the rows.
#[must_use]
pub fn write_csv(header: &[&str], rows: &[Vec<Cell<'_>>]) -> String {
    let mut out = String::new();
    let header: Vec<Cell<'_>> = header.iter().map(|name| Cell::text(name)).collect();
    push_record(&mut out, &header);
    for row in rows {
        push_record(&mut out, row);
    }
    out
}

/// Column names of the rate-schedule CSV. The vintage's status travels with every
/// row, so a row detached from the file is still honest about what produced it.
pub const RATE_SCHEDULE_COLUMNS: [&str; 13] = [
    "tax_year",
    "filing_status",
    "line_no",
    "line_id",
    "label",
    "amount_usd",
    "computed_from",
    "parameters",
    "rounding",
    "is_result",
    "vintage_id",
    "vintage_locked",
    "vintage_verified",
];

const fn filing_status_wire(status: FilingStatusDto) -> &'static str {
    match status {
        FilingStatusDto::Single => "single",
        FilingStatusDto::Mfj => "mfj",
        FilingStatusDto::Mfs => "mfs",
        FilingStatusDto::Hoh => "hoh",
        FilingStatusDto::Qss => "qss",
    }
}

fn param_ref_text(param: &ParamRefDto) -> String {
    let mut out = param.param_id.clone();
    if let Some(year) = param.year {
        let _ = write!(out, " {year}");
    }
    for part in [&param.breakdown_key, &param.element].into_iter().flatten() {
        out.push(' ');
        out.push_str(part);
    }
    out
}

/// The rate-schedule worksheet as CSV: one row per line, in computation order.
#[must_use]
pub fn rate_schedule_csv(response: &RateScheduleResponse) -> String {
    let status = filing_status_wire(response.filing_status);
    let rows: Vec<Vec<Cell<'_>>> = response
        .lines
        .iter()
        .zip(1_i64..)
        .map(|(line, number)| {
            let params: Vec<String> = line.params.iter().map(param_ref_text).collect();
            vec![
                Cell::Int(i64::from(response.year)),
                Cell::text(status),
                Cell::Int(number),
                Cell::text(&line.id),
                Cell::text(&line.label),
                Cell::Cents(line.value.0),
                Cell::Text(Cow::Owned(line.inputs.join("; "))),
                Cell::Text(Cow::Owned(params.join("; "))),
                Cell::text(line.rounding.as_deref().unwrap_or("")),
                Cell::yes_no(line.id == response.root_line_id),
                Cell::text(&response.vintage.content_id),
                Cell::yes_no(response.vintage.locked_id.is_some()),
                Cell::yes_no(response.verified),
            ]
        })
        .collect();
    write_csv(&RATE_SCHEDULE_COLUMNS, &rows)
}

/// The export body could not be produced. Carries no detail: the caller answers
/// the API's standard `500` body and records an event code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExportError;

impl std::fmt::Display for ExportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("the export body could not be serialized")
    }
}

impl std::error::Error for ExportError {}

/// A finished export: the body and the two literal headers that describe it. It
/// can only be obtained from a build that succeeded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attachment {
    /// The file's text, exactly as sent.
    pub body: String,
    /// The `Content-Type` value; a literal.
    pub content_type: &'static str,
    /// The `Content-Disposition` value; a literal. The front end takes the saved
    /// file's name from it, so it is the one source of that name.
    pub disposition: &'static str,
}

/// `Content-Disposition` of the CSV export.
pub const CSV_DISPOSITION: &str = "attachment; filename=\"rate-schedule.csv\"";
/// `Content-Disposition` of the JSON export.
pub const JSON_DISPOSITION: &str = "attachment; filename=\"rate-schedule.json\"";

/// A value as pretty-printed JSON with one trailing newline. No value is touched.
///
/// # Errors
/// [`ExportError`] when the value cannot be serialized. Never an empty document.
pub fn pretty_json<T: Serialize + ?Sized>(value: &T) -> Result<String, ExportError> {
    let mut text = serde_json::to_string_pretty(value).map_err(|_| ExportError)?;
    text.push('\n');
    Ok(text)
}

/// A value as a JSON attachment. The fallible step comes first: on an error no
/// success header has been chosen.
///
/// # Errors
/// [`ExportError`] when the value cannot be serialized.
pub fn json_attachment<T: Serialize + ?Sized>(value: &T) -> Result<Attachment, ExportError> {
    let body = pretty_json(value)?;
    Ok(Attachment {
        body,
        content_type: "application/json",
        disposition: JSON_DISPOSITION,
    })
}

/// The rate-schedule worksheet as JSON: the API body, pretty-printed, one
/// trailing newline. No value is touched.
///
/// # Errors
/// [`ExportError`] when the worksheet cannot be serialized.
pub fn rate_schedule_json(response: &RateScheduleResponse) -> Result<String, ExportError> {
    pretty_json(response)
}

/// The rate-schedule worksheet as an attachment in the requested format.
///
/// # Errors
/// [`ExportError`] when the body cannot be produced.
pub fn rate_schedule_attachment(
    format: ExportFormatDto,
    response: &RateScheduleResponse,
) -> Result<Attachment, ExportError> {
    match format {
        ExportFormatDto::Csv => Ok(Attachment {
            body: rate_schedule_csv(response),
            content_type: "text/csv; charset=utf-8",
            disposition: CSV_DISPOSITION,
        }),
        ExportFormatDto::Json => json_attachment(response),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one(cell: Cell<'_>) -> String {
        let mut out = String::new();
        push_record(&mut out, &[cell]);
        out.strip_suffix("\r\n").expect("CRLF").to_owned()
    }

    /// A value whose serialization always fails: what a future DTO with a
    /// non-string map key or a failing custom `Serialize` would look like.
    struct Unserializable;

    impl Serialize for Unserializable {
        fn serialize<S: serde::Serializer>(&self, _: S) -> Result<S::Ok, S::Error> {
            Err(serde::ser::Error::custom("always fails"))
        }
    }

    #[test]
    fn a_serialization_error_fails_closed_and_never_yields_an_empty_document() {
        assert_eq!(pretty_json(&Unserializable), Err(ExportError));
        // No `Attachment` - so no `200`, no `Content-Disposition` - exists on the error path.
        assert_eq!(json_attachment(&Unserializable), Err(ExportError));
        // A map with a non-string key is the realistic way a derived DTO fails.
        let bad: std::collections::BTreeMap<(i32, i32), i32> = [((1, 2), 3)].into_iter().collect();
        assert_eq!(json_attachment(&bad), Err(ExportError));
    }

    #[test]
    fn a_successful_json_attachment_is_pretty_printed_with_one_trailing_newline() {
        let attachment = json_attachment(&serde_json::json!({ "a": -1 })).expect("serializes");
        assert_eq!(attachment.body, "{\n  \"a\": -1\n}\n");
        assert_eq!(attachment.content_type, "application/json");
        assert_eq!(attachment.disposition, JSON_DISPOSITION);
    }

    #[test]
    fn cents_are_bare_numbers_whatever_their_sign() {
        for (cents, text) in [
            (1_150_400, "11504.00"),
            (-1234, "-12.34"),
            (0, "0.00"),
            (5, "0.05"),
            (-5, "-0.05"),
            (99, "0.99"),
            (100, "1.00"),
            (i64::MAX, "92233720368547758.07"),
            (i64::MIN, "-92233720368547758.08"),
        ] {
            assert_eq!(one(Cell::Cents(cents)), text);
        }
    }

    #[test]
    fn ints_and_dates_are_bare() {
        assert_eq!(one(Cell::Int(2026)), "2026");
        assert_eq!(one(Cell::Int(-3)), "-3");
        let date = DateYmd::parse("2026-01-31").expect("date");
        assert_eq!(one(Cell::Date(date)), "2026-01-31");
    }

    #[test]
    fn each_trigger_character_gets_the_apostrophe_inside_the_quotes() {
        for trigger in ['=', '+', '-', '@', '\t', '\r'] {
            let text = format!("{trigger}1+1");
            assert_eq!(
                one(Cell::text(&text)),
                format!("\"'{text}\""),
                "{trigger:?}"
            );
        }
    }

    #[test]
    fn the_same_characters_as_text_and_as_money_differ() {
        // The design's core point: `-5` the label is inert text, -500 cents is a number.
        assert_eq!(one(Cell::text("-5")), "\"'-5\"");
        assert_eq!(one(Cell::Cents(-500)), "-5.00");
    }

    #[test]
    fn leading_whitespace_does_not_hide_a_trigger() {
        for escaped in [
            " =1+1",
            "\u{00A0}=1",
            "\u{3000}+1",
            " \t-1",
            "\u{FEFF}@x",
            "\n=1",
        ] {
            assert!(needs_escape(escaped), "{escaped:?}");
            assert_eq!(one(Cell::text(escaped)), format!("\"'{escaped}\""));
        }
        for plain in [" plain", " 'x", "", "a=b", "1-2", "x@y"] {
            assert!(!needs_escape(plain), "{plain:?}");
            assert_eq!(one(Cell::text(plain)), format!("\"{plain}\""));
        }
    }

    #[test]
    fn a_leading_apostrophe_is_itself_escaped_so_the_inverse_is_exact() {
        assert_eq!(one(Cell::text("'abc")), "\"''abc\"");
    }

    #[test]
    fn quotes_are_doubled_and_separators_stay_verbatim_inside_the_quotes() {
        assert_eq!(one(Cell::text("say \"hi\"")), "\"say \"\"hi\"\"\"");
        assert_eq!(one(Cell::text("a,b\r\nc\nd")), "\"a,b\r\nc\nd\"");
        assert_eq!(one(Cell::text("")), "\"\"");
    }

    #[test]
    fn non_ascii_text_is_kept_byte_for_byte_and_is_not_mistaken_for_a_trigger() {
        // Letters, CJK, a character outside the BMP, a combining mark, a
        // right-to-left run, look-alikes of `-` (en dash, minus sign) and a
        // non-breaking space before plain text: none is one of the six characters,
        // so none is escaped and none is altered.
        for plain in [
            "Imp\u{f4}t sur le revenu",
            "\u{6240}\u{5f97}\u{7a0e} 2026",
            "\u{1F4C8} growth",
            "e\u{0301}tat",
            "\u{05DE}\u{05E1}",
            "\u{2013}5",
            "\u{2212}5",
            "\u{00A0}plain",
        ] {
            assert!(!needs_escape(plain), "{plain:?}");
            let cell = one(Cell::text(plain));
            assert_eq!(cell, format!("\"{plain}\""));
            assert_eq!(cell.len(), plain.len() + 2, "no re-encoding");
        }
        // A trigger after a non-ASCII letter is inert; after non-ASCII whitespace
        // (em space) it is not.
        assert_eq!(one(Cell::text("\u{e9}=1")), "\"\u{e9}=1\"");
        assert_eq!(one(Cell::text("\u{2003}=1")), "\"'\u{2003}=1\"");
        // Non-ASCII beside a quote and a newline: doubled, verbatim, one cell.
        assert_eq!(
            one(Cell::text("\u{ab}\"\u{fc}\"\u{bb}\n\u{7a0e}")),
            "\"\u{ab}\"\"\u{fc}\"\"\u{bb}\n\u{7a0e}\""
        );
    }

    #[test]
    fn documents_have_a_quoted_header_crlf_records_and_no_bom() {
        let csv = write_csv(
            &["label", "amount_usd"],
            &[vec![Cell::text("=SUM(A1)"), Cell::Cents(-1)]],
        );
        assert_eq!(csv, "\"label\",\"amount_usd\"\r\n\"'=SUM(A1)\",-0.01\r\n");
        assert!(!csv.starts_with('\u{FEFF}'));
    }
}
