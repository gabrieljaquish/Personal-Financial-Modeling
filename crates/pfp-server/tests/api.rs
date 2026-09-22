//! The JSON API over real TLS: status, the rate schedule with its `Line` tree, and
//! the Assumptions Registry.

// Not an engine crate: tests here legitimately touch sockets, the clock and files.
#![allow(
    clippy::disallowed_types,
    clippy::disallowed_methods,
    clippy::disallowed_macros
)]

mod support;

use serde_json::{json, Value};
use support::HttpServer;

const STATUS: &str = "/api/v1/session/status";
const SCHEDULE: &str = "/api/v1/tax/rate-schedule";
const ASSUMPTIONS: &str = "/api/v1/assumptions/list";
const VALIDATION: &str = "/api/v1/validation/report";

#[tokio::test]
async fn status_reports_versions_trust_mode_and_the_unverified_vintage() {
    let server = HttpServer::start();
    let session = server.establish().await;
    let response = server.send(server.authed(STATUS, &session)).await;
    assert_eq!(response.status, 200);
    assert_eq!(response.header("content-type"), Some("application/json"));
    let body = response.json();
    assert_eq!(body["apiVersion"], "v1");
    assert_eq!(body["appVersion"], env!("CARGO_PKG_VERSION"));
    assert_eq!(body["licence"], "Apache-2.0");
    assert_eq!(body["trustMode"], "declined");
    let vintage = &body["vintages"][0];
    assert_eq!(vintage["name"], "federal-2026");
    assert!(vintage["contentId"]
        .as_str()
        .unwrap()
        .starts_with("federal-2026@"));
    // Pending hand verification and not locked (ADR-022): the API says so.
    assert_eq!(vintage["verified"], false);
    assert!(vintage.get("lockedId").is_none());
    server.stop().await;
}

/// The three grid points of `docs/PLAN.md` §4.1 (2026, married filing jointly),
/// typed from that document in dollars — not read back from the engine.
const PLAN_GRID_DOLLARS: [(i64, i64); 3] =
    [(100_000, 11_504), (150_000, 22_424), (250_000, 45_196)];

#[tokio::test]
async fn rate_schedule_matches_the_plan_grid_and_returns_the_line_tree() {
    let server = HttpServer::start();
    let session = server.establish().await;
    for (income, tax) in PLAN_GRID_DOLLARS {
        let request = json!({"year": 2026, "filingStatus": "mfj", "taxableIncome": income * 100});
        let response = server
            .send(server.authed(SCHEDULE, &session).json(&request.to_string()))
            .await;
        assert_eq!(response.status, 200, "{income}");
        let body = response.json();
        assert_eq!(body["tax"], tax * 100, "{income}");
        assert_eq!(body["year"], 2026);
        assert_eq!(body["filingStatus"], "mfj");
        assert_eq!(body["taxableIncome"], income * 100);
        assert_eq!(body["verified"], false);
        assert_eq!(body["vintage"]["name"], "federal-2026");

        // The tree: a flat ordered list plus the root id; inputs resolve within it
        // and only backwards, so a client can walk it without a cycle check.
        let lines = body["lines"].as_array().unwrap();
        let root = body["rootLineId"].as_str().unwrap();
        assert_eq!(lines.last().unwrap()["id"], root);
        assert_eq!(lines.last().unwrap()["value"], tax * 100);
        assert_eq!(lines.last().unwrap()["rounding"], "irs.whole_dollar");
        let mut seen: Vec<&str> = Vec::new();
        for line in lines {
            for input in line["inputs"].as_array().into_iter().flatten() {
                assert!(
                    seen.contains(&input.as_str().unwrap()),
                    "{input} precedes its use"
                );
            }
            seen.push(line["id"].as_str().unwrap());
            assert!(line["value"].is_i64(), "money is integer cents");
        }
        // Seven brackets, two lines each, plus income, sum and tax.
        assert_eq!(lines.len(), 17);
        // Every parameter read is cited down to the cell.
        let cited: Vec<&Value> = lines
            .iter()
            .flat_map(|l| l["params"].as_array().into_iter().flatten())
            .collect();
        assert!(!cited.is_empty());
        for param in cited {
            assert_eq!(param["paramId"], "irs.ordinary_brackets");
            assert_eq!(param["year"], 2026);
        }
    }
    server.stop().await;
}

#[tokio::test]
async fn rate_schedule_refuses_bad_input_with_fixed_messages() {
    let server = HttpServer::start();
    let session = server.establish().await;
    let cases: [(&str, u16, &str); 11] = [
        (
            r#"{"year":2026,"filingStatus":"mfj","taxableIncome":-1}"#,
            422,
            "schedule_input_invalid",
        ),
        (
            r#"{"year":2026,"filingStatus":"mfj","taxableIncome":9007199254740992}"#,
            422,
            "schedule_input_invalid",
        ),
        (
            r#"{"year":1999,"filingStatus":"mfj","taxableIncome":100}"#,
            422,
            "schedule_input_invalid",
        ),
        (
            r#"{"year":2026,"filingStatus":"married","taxableIncome":100}"#,
            422,
            "request_invalid",
        ),
        (
            r#"{"year":2026,"filingStatus":"mfj","taxableIncome":100.5}"#,
            422,
            "request_invalid",
        ),
        (
            r#"{"year":2026,"filingStatus":"mfj","taxableIncome":"100"}"#,
            422,
            "request_invalid",
        ),
        (
            r#"{"year":2026,"filingStatus":"mfj"}"#,
            422,
            "request_invalid",
        ),
        (
            r#"{"year":2026,"filingStatus":"mfj","taxableIncome":100,"extra":1}"#,
            422,
            "request_invalid",
        ),
        (r#"{"year":2026,"#, 400, "invalid_json"),
        ("not json <script>alert(1)</script>", 400, "invalid_json"),
        ("[]", 422, "request_invalid"),
    ];
    for (body, status, code) in cases {
        let response = server
            .send(server.authed(SCHEDULE, &session).json(body))
            .await;
        assert_eq!(
            (response.status, response.code().as_str()),
            (status, code),
            "{body}"
        );
        // Error bodies leak nothing: no echo of the input, no parser detail.
        let text = String::from_utf8(response.body.clone()).unwrap();
        for leak in [
            "script",
            "married",
            "extra",
            "line",
            "column",
            "9007199254740992",
            "unknown",
            "at ",
        ] {
            assert!(!text.contains(leak), "{body} -> {text}");
        }
    }
    // The largest accepted income is 2^53 - 1 cents.
    let top = server
        .send(
            server
                .authed(SCHEDULE, &session)
                .json(r#"{"year":2026,"filingStatus":"single","taxableIncome":9007199254740991}"#),
        )
        .await;
    assert_eq!(top.status, 200);
    server.stop().await;
}

#[tokio::test]
async fn assumptions_registry_lists_every_table_with_provenance() {
    let server = HttpServer::start();
    let session = server.establish().await;
    let response = server.send(server.authed(ASSUMPTIONS, &session)).await;
    assert_eq!(response.status, 200);
    let body = response.json();
    let vintages = body["vintages"].as_array().unwrap();
    assert_eq!(vintages.len(), 1);
    let vintage = &vintages[0];
    assert_eq!(vintage["name"], "federal-2026");
    assert_eq!(vintage["verified"], false);

    let tables = vintage["tables"].as_array().unwrap();
    let ids: Vec<&str> = tables.iter().map(|t| t["id"].as_str().unwrap()).collect();
    assert_eq!(ids, ["irs.ordinary_brackets", "irs.std_deduction"]);
    for table in tables {
        let id = &table["id"];
        assert_eq!(table["vintageId"], vintage["contentId"], "{id}");
        assert!(table.get("vintage").is_none(), "{id}");
        assert_eq!(table["verification"], "pending-hand-verification", "{id}");
        assert_eq!(table["servedUnit"], "cents", "{id}");
        assert!(table.get("unit").is_none(), "{id}");
        // as-of date
        let as_of = table["asOf"].as_str().unwrap();
        assert_eq!(
            (as_of.len(), &as_of[4..5], &as_of[7..8]),
            (10, "-", "-"),
            "{id}"
        );
        // source
        let sources = table["sources"].as_array().unwrap();
        assert!(!sources.is_empty(), "{id}");
        for source in sources {
            assert!(!source["title"].as_str().unwrap().is_empty());
            assert!(source["url"].as_str().unwrap().starts_with("https://"));
            assert_eq!(source["sha256"].as_str().unwrap().len(), 64);
            assert_eq!(source["retrieved"].as_str().unwrap().len(), 10);
        }
        // projection and rounding rule
        assert_eq!(table["projection"]["rule"], "index", "{id}");
        assert!(table["projection"]["indexSeries"].is_string(), "{id}");
        assert!(!table["projection"]["baseValues"]
            .as_object()
            .unwrap()
            .is_empty());
        let rounding = &table["rounding"];
        assert!(rounding["direction"].is_string(), "{id}");
        assert!(rounding["basis"].is_string(), "{id}");
        assert!(rounding.get("increment").is_some() || rounding.get("incrementByKey").is_some());
        // values: key -> year -> amounts in cents
        assert!(table["values"]["mfj"]["2026"].is_array(), "{id}");
    }
    // Values are served in cents whatever unit the source file is written in.
    assert_eq!(tables[0]["values"]["mfj"]["2026"][0], 2_480_000);
    assert_eq!(
        tables[0]["rates"]["2026"][0],
        json!({"num": 10, "den": 100})
    );
    assert_eq!(tables[0]["components"].as_array().unwrap().len(), 6);

    let series = vintage["indexSeries"].as_array().unwrap();
    assert_eq!(series.len(), 1);
    assert!(!series[0]["sources"].as_array().unwrap().is_empty());
    server.stop().await;
}

/// The report is whatever the build embedded, or the explicit statement that
/// nothing was: this test passes in both states and asserts the honest one.
#[tokio::test]
async fn validation_report_is_served_or_declared_not_generated() {
    let server = HttpServer::start();
    let session = server.establish().await;
    let response = server.send(server.authed(VALIDATION, &session)).await;
    assert_eq!(response.status, 200);
    assert_eq!(response.header("content-type"), Some("application/json"));
    let body = response.json();
    if pfp_server::validation::is_embedded() {
        assert_eq!(body["state"], "generated");
        let report = &body["report"];
        let tiers = report["fixtures"]["tiers"].as_array().unwrap();
        assert_eq!(
            tiers
                .iter()
                .map(|t| t["tier"].as_str().unwrap())
                .collect::<Vec<_>>(),
            ["tier1", "tier2", "tier3", "personas", "plans", "pending"]
        );
        // Every section carries a state and a milestone, present or not.
        for section in [
            "tier2Suites",
            "tier3Goldens",
            "oracles",
            "mutationScore",
            "fuzzCorpora",
            "browserEndToEnd",
        ] {
            assert!(report[section]["state"].is_string(), "{section}");
            assert!(report[section]["milestone"].is_string(), "{section}");
        }
        // The unverified block is printed, never hidden, and pending is never verified.
        let unverified = &report["unverified"];
        assert!(unverified["items"]
            .as_array()
            .is_some_and(|i| !i.is_empty()));
        assert!(unverified["pendingFixtureCount"].is_u64());
        for tier in tiers {
            for file in tier["files"].as_array().unwrap() {
                if file["verification"] == "pending-hand-verification" {
                    assert_ne!(tier["tier"], "tier1", "{}", file["path"]);
                }
            }
        }
        // The server and the report agree about the vintage it serves.
        let status = server.send(server.authed(STATUS, &session)).await.json();
        let content_ids = report["pins"]["paramVintageContentIds"].as_array().unwrap();
        assert!(content_ids.contains(&status["vintages"][0]["contentId"]));
        assert_eq!(report["pins"]["licence"], status["licence"]);
        assert!(!report["basis"].as_str().unwrap().is_empty());
    } else {
        assert_eq!(body["state"], "not-generated");
        assert!(body.get("report").is_none());
    }
    server.stop().await;
}

#[tokio::test]
async fn every_api_route_is_post_only_and_session_gated() {
    let server = HttpServer::start();
    let session = server.establish().await;
    for path in [
        STATUS,
        SCHEDULE,
        "/api/v1/tax/rate-schedule/export",
        ASSUMPTIONS,
        VALIDATION,
        "/api/v1/session/relaunch",
    ] {
        let anonymous = server.send(server.api(path)).await;
        assert_eq!(anonymous.status, 401, "{path}");
        for method in ["GET", "PUT", "DELETE", "PATCH"] {
            let response = server
                .send(server.authed(path, &session).method(method))
                .await;
            assert_eq!(response.status, 405, "{method} {path}");
            assert_eq!(response.header("allow"), Some("POST"));
        }
    }
    let bootstrap_get = server
        .send(server.api(support::BOOTSTRAP).method("GET"))
        .await;
    assert_eq!(bootstrap_get.status, 405);
    server.stop().await;
}
