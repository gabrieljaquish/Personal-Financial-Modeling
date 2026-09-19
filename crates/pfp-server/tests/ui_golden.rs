//! Golden API bodies for the front-end tests (`web/tests/support/golden.mjs` reads
//! these snapshots). The views are therefore rendered from **what the server really
//! sends**: a DTO change that would break a screen fails a web test, not a user
//! session.
//!
//! JSON bodies are recorded pretty-printed with sorted keys so a diff is
//! reviewable; every map the API serves is a sorted map already, and no consumer
//! depends on key order. The two exports are recorded as sent, with each carriage
//! return drawn as `␍` because a snapshot file does not keep one.
//!
//! They are insta snapshots rather than `.json` files because the repository's
//! data-hygiene gate reserves JSON documents for `fixtures/` and `params/`.
//!
//! The input is SYNTHETIC: the first grid point of `docs/PLAN.md` §4.1.

// Not an engine crate: tests here legitimately touch sockets, the clock and files.
#![allow(
    clippy::disallowed_types,
    clippy::disallowed_methods,
    clippy::disallowed_macros
)]

mod support;

use serde_json::Value;
use support::{HttpServer, Resp};

const INPUT: &str = r#"{"year":2026,"filingStatus":"mfj","taxableIncome":10000000}"#;
const INPUT_CSV: &str =
    r#"{"year":2026,"filingStatus":"mfj","taxableIncome":10000000,"format":"csv"}"#;
const INPUT_JSON: &str =
    r#"{"year":2026,"filingStatus":"mfj","taxableIncome":10000000,"format":"json"}"#;

fn pretty(response: &Resp) -> String {
    assert_eq!(response.status, 200);
    let value: Value = response.json();
    serde_json::to_string_pretty(&value).expect("JSON")
}

fn text(response: &Resp) -> String {
    assert_eq!(response.status, 200);
    String::from_utf8(response.body.clone()).expect("UTF-8")
}

#[tokio::test]
async fn golden_bodies() {
    let server = HttpServer::start();
    let session = server.establish().await;

    let status = server
        .send(server.authed("/api/v1/session/status", &session))
        .await;
    let mut status_value: Value = status.json();
    // The one run-specific value: the crate version. Everything else is content.
    status_value["appVersion"] = Value::String("0.0.0-golden".to_owned());
    insta::assert_snapshot!(
        "ui_golden_session_status",
        serde_json::to_string_pretty(&status_value).expect("JSON")
    );

    let assumptions = server
        .send(server.authed("/api/v1/assumptions/list", &session))
        .await;
    insta::assert_snapshot!("ui_golden_assumptions_list", pretty(&assumptions));

    let schedule = server
        .send(
            server
                .authed("/api/v1/tax/rate-schedule", &session)
                .json(INPUT),
        )
        .await;
    assert_eq!(schedule.json()["tax"], 1_150_400);
    insta::assert_snapshot!("ui_golden_rate_schedule", pretty(&schedule));

    let csv = server
        .send(
            server
                .authed("/api/v1/tax/rate-schedule/export", &session)
                .json(INPUT_CSV),
        )
        .await;
    let csv = text(&csv);
    assert!(!csv.contains('␍'));
    insta::assert_snapshot!("ui_golden_rate_schedule_csv", csv.replace('\r', "␍"));

    let exported = server
        .send(
            server
                .authed("/api/v1/tax/rate-schedule/export", &session)
                .json(INPUT_JSON),
        )
        .await;
    insta::assert_snapshot!("ui_golden_rate_schedule_json", text(&exported));

    server.stop().await;
}
