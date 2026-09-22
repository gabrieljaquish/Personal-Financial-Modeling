//! Export (test id S-12, `SECURITY.md` §9): a negative `Cents` round-trips as a
//! number, a label beginning with `=` as inert text, and the JSON export carries
//! exactly the API's values. The unit tests beside the writer cover each cell kind;
//! this file covers the properties and the endpoint.

// Not an engine crate: tests here legitimately touch sockets, the clock and files.
#![allow(
    clippy::disallowed_types,
    clippy::disallowed_methods,
    clippy::disallowed_macros
)]

mod support;

use pfp_server::api::export::{needs_escape, write_csv, Cell, RATE_SCHEDULE_COLUMNS};
use proptest::prelude::*;
use serde_json::{json, Value};
use support::HttpServer;

const SCHEDULE: &str = "/api/v1/tax/rate-schedule";
const EXPORT: &str = "/api/v1/tax/rate-schedule/export";

/// A small RFC 4180 reader, written here and not shared with the writer. A quoted
/// cell comes back as `(true, text)` with quotes removed and `""` un-doubled; a
/// bare cell as `(false, text)`.
fn read_csv(text: &str) -> Vec<Vec<(bool, String)>> {
    let mut records = Vec::new();
    let mut record = Vec::new();
    let mut chars = text.chars().peekable();
    while chars.peek().is_some() {
        let mut cell = String::new();
        let quoted = chars.peek() == Some(&'"');
        if quoted {
            chars.next();
            loop {
                match chars.next() {
                    Some('"') if chars.peek() == Some(&'"') => {
                        chars.next();
                        cell.push('"');
                    }
                    Some('"') | None => break,
                    Some(c) => cell.push(c),
                }
            }
        } else {
            while let Some(c) = chars.peek().copied() {
                if c == ',' || c == '\r' {
                    break;
                }
                cell.push(c);
                chars.next();
            }
        }
        record.push((quoted, cell));
        match chars.next() {
            Some(',') => {}
            Some('\r') => {
                assert_eq!(chars.next(), Some('\n'), "records end with CRLF");
                records.push(std::mem::take(&mut record));
            }
            other => panic!("unexpected {other:?} after a cell"),
        }
    }
    assert!(record.is_empty(), "the last record ends with CRLF");
    records
}

/// The exact inverse of the escape: drop one leading apostrophe if present.
fn unescape(cell: &str) -> &str {
    cell.strip_prefix('\'').unwrap_or(cell)
}

fn is_trigger(c: char) -> bool {
    matches!(c, '=' | '+' | '-' | '@' | '\t' | '\r')
}

/// Arbitrary Unicode text, biased towards what the escape exists for: leading
/// whitespace, U+FEFF, trigger characters, apostrophes, quotes and separators.
fn nasty_text() -> impl Strategy<Value = String> {
    let lead = prop::collection::vec(
        prop::sample::select(vec![
            " ", "\u{00A0}", "\u{3000}", "\u{FEFF}", "\t", "\r", "\n", "'", "\"", "=", "+", "-",
            "@", ",", "\r\n",
        ]),
        0..4,
    );
    (lead, any::<String>()).prop_map(|(lead, rest)| format!("{}{rest}", lead.concat()))
}

proptest! {
    #[test]
    fn text_round_trips_through_an_independent_reader(texts in prop::collection::vec(nasty_text(), 1..5)) {
        let row: Vec<Cell<'_>> = texts.iter().map(|t| Cell::text(t)).collect();
        let csv = write_csv(&["h"], &[row]);
        let records = read_csv(&csv);
        prop_assert_eq!(records.len(), 2);
        prop_assert_eq!(records[1].len(), texts.len());
        for ((quoted, cell), original) in records[1].iter().zip(&texts) {
            prop_assert!(*quoted, "text is always quoted");
            prop_assert_eq!(unescape(cell), original.as_str());
        }
    }

    #[test]
    fn no_emitted_text_cell_begins_with_a_trigger_even_after_trimming(text in nasty_text()) {
        let csv = write_csv(&["h"], &[vec![Cell::text(&text)]]);
        let records = read_csv(&csv);
        let emitted = &records[1][0].1;
        let first = emitted
            .trim_start_matches(|c: char| c.is_whitespace() || c == '\u{FEFF}')
            .chars()
            .next();
        prop_assert!(!first.is_some_and(is_trigger), "{:?} -> {:?}", text, emitted);
        prop_assert!(!emitted.starts_with(['\t', '\r']));
        prop_assert_eq!(emitted.starts_with('\''), needs_escape(&text));
    }

    #[test]
    fn cents_are_always_a_bare_number(cents in any::<i64>()) {
        let csv = write_csv(&["h"], &[vec![Cell::Cents(cents)]]);
        let records = read_csv(&csv);
        let (quoted, cell) = &records[1][0];
        prop_assert!(!quoted);
        let (sign, digits) = cell.strip_prefix('-').map_or(("", cell.as_str()), |d| ("-", d));
        let (dollars, pennies) = digits.split_once('.').expect("two decimals");
        prop_assert_eq!(pennies.len(), 2);
        let back: i128 = format!("{sign}{dollars}{pennies}").parse().expect("integer");
        prop_assert_eq!(back, i128::from(cents));
    }
}

fn export_request(format: &str) -> String {
    // SYNTHETIC input: the first grid point of docs/PLAN.md §4.1.
    json!({"year": 2026, "filingStatus": "mfj", "taxableIncome": 10_000_000, "format": format})
        .to_string()
}

#[tokio::test]
async fn csv_export_is_an_attachment_whose_result_row_is_a_number() {
    let server = HttpServer::start();
    let session = server.establish().await;
    let response = server
        .send(server.authed(EXPORT, &session).json(&export_request("csv")))
        .await;
    assert_eq!(response.status, 200);
    assert_eq!(
        response.header("content-type"),
        Some("text/csv; charset=utf-8")
    );
    assert_eq!(
        response.header("content-disposition"),
        Some("attachment; filename=\"rate-schedule.csv\"")
    );
    let text = String::from_utf8(response.body.clone()).expect("UTF-8");
    assert!(!text.starts_with('\u{FEFF}'), "no byte-order mark");

    let records = read_csv(&text);
    // Header plus the seventeen lines of a seven-rate schedule.
    assert_eq!(records.len(), 18);
    let header: Vec<&str> = records[0].iter().map(|(_, c)| c.as_str()).collect();
    assert_eq!(header, RATE_SCHEDULE_COLUMNS);
    let column = |name: &str| header.iter().position(|h| *h == name).expect("column");

    let result = records.last().expect("result row");
    // Typed in dollars from docs/PLAN.md §4.1, not read back from the engine.
    assert_eq!(result[column("amount_usd")], (false, "11504.00".to_owned()));
    assert_eq!(result[column("is_result")], (true, "yes".to_owned()));
    assert_eq!(result[column("tax_year")], (false, "2026".to_owned()));
    assert_eq!(result[column("filing_status")], (true, "mfj".to_owned()));
    assert_eq!(result[column("line_no")], (false, "17".to_owned()));
    assert_eq!(
        result[column("rounding")],
        (true, "irs.whole_dollar".to_owned())
    );
    // The shipped vintage is unlocked and unverified, and every row says so.
    for row in &records[1..] {
        assert_eq!(row.len(), header.len());
        assert_eq!(row[column("vintage_locked")].1, "no");
        assert_eq!(row[column("vintage_verified")].1, "no");
        assert!(row[column("vintage_id")].1.starts_with("federal-2026@"));
        assert!(!row[column("amount_usd")].0, "money is never quoted");
    }
    assert_eq!(
        records[1..]
            .iter()
            .filter(|row| row[column("is_result")].1 == "yes")
            .count(),
        1
    );
    server.stop().await;
}

#[tokio::test]
async fn json_export_carries_exactly_the_api_values() {
    let server = HttpServer::start();
    let session = server.establish().await;
    let api = server
        .send(
            server
                .authed(SCHEDULE, &session)
                .json(r#"{"year":2026,"filingStatus":"mfj","taxableIncome":10000000}"#),
        )
        .await;
    let export = server
        .send(
            server
                .authed(EXPORT, &session)
                .json(&export_request("json")),
        )
        .await;
    assert_eq!(export.status, 200);
    assert_eq!(export.header("content-type"), Some("application/json"));
    assert_eq!(
        export.header("content-disposition"),
        Some("attachment; filename=\"rate-schedule.json\"")
    );
    let text = String::from_utf8(export.body.clone()).expect("UTF-8");
    assert!(text.ends_with("}\n") && !text.ends_with("\n\n"));
    let exported: Value = serde_json::from_str(&text).expect("JSON");
    assert_eq!(exported, api.json());
    assert_eq!(exported["tax"], 1_150_400);
    for line in exported["lines"].as_array().unwrap() {
        assert!(line["value"].is_i64(), "amounts stay integers");
    }
    server.stop().await;
}

#[test]
fn json_export_leaves_a_label_that_begins_with_a_trigger_alone() {
    use pfp_server::api::dto::{
        CentsDto, FilingStatusDto, LineDto, RateScheduleResponse, VintageSummaryDto,
    };
    // SYNTHETIC worksheet: one line whose label would be a formula in a spreadsheet.
    let label = "=HYPERLINK(\"x\")";
    let worksheet = RateScheduleResponse {
        year: 2026,
        filing_status: FilingStatusDto::Single,
        taxable_income: CentsDto(0),
        tax: CentsDto(-500),
        root_line_id: "synthetic.line".to_owned(),
        lines: vec![LineDto {
            id: "synthetic.line".to_owned(),
            label: label.to_owned(),
            value: CentsDto(-500),
            inputs: Vec::new(),
            params: Vec::new(),
            rounding: None,
        }],
        vintage: VintageSummaryDto {
            name: "synthetic".to_owned(),
            content_id: "synthetic@sha256:00".to_owned(),
            locked_id: None,
            verified: false,
        },
        verified: false,
    };
    let exported: Value = serde_json::from_str(
        &pfp_server::api::export::rate_schedule_json(&worksheet).expect("serializes"),
    )
    .expect("JSON");
    assert_eq!(exported["lines"][0]["label"], label);
    assert_eq!(exported["lines"][0]["value"], -500);

    // The same worksheet as CSV: the label is inert text, the amount a number.
    let csv = pfp_server::api::export::rate_schedule_csv(&worksheet);
    let records = read_csv(&csv);
    assert_eq!(records[1][4], (true, format!("'{label}")));
    assert_eq!(records[1][5], (false, "-5.00".to_owned()));
}

#[tokio::test]
async fn the_export_route_is_post_only_session_gated_and_strict() {
    let server = HttpServer::start();
    let session = server.establish().await;
    let anonymous = server
        .send(server.api(EXPORT).json(&export_request("csv")))
        .await;
    assert_eq!(
        (anonymous.status, anonymous.code().as_str()),
        (401, "session_required")
    );
    for method in ["GET", "PUT", "DELETE", "PATCH"] {
        let response = server
            .send(server.authed(EXPORT, &session).method(method))
            .await;
        assert_eq!(response.status, 405, "{method}");
        assert_eq!(response.header("allow"), Some("POST"));
    }
    let cases: [(&str, u16, &str); 5] = [
        (
            r#"{"year":2026,"filingStatus":"mfj","taxableIncome":100}"#,
            422,
            "request_invalid",
        ),
        (
            r#"{"year":2026,"filingStatus":"mfj","taxableIncome":100,"format":"xlsx"}"#,
            422,
            "request_invalid",
        ),
        (
            r#"{"year":2026,"filingStatus":"mfj","taxableIncome":100,"format":"csv","lines":[]}"#,
            422,
            "request_invalid",
        ),
        (
            r#"{"year":1999,"filingStatus":"mfj","taxableIncome":100,"format":"csv"}"#,
            422,
            "schedule_input_invalid",
        ),
        ("{", 400, "invalid_json"),
    ];
    for (body, status, code) in cases {
        let response = server
            .send(server.authed(EXPORT, &session).json(body))
            .await;
        assert_eq!(
            (response.status, response.code().as_str()),
            (status, code),
            "{body}"
        );
        assert!(response.header("content-disposition").is_none(), "{body}");
    }
    server.stop().await;
}
