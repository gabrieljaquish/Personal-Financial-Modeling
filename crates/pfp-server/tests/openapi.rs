//! The `OpenAPI` document is generated from the Rust DTOs and snapshot-tested
//! (ADR-017). The snapshot is the API contract: a diff here is an API change and
//! is reviewed as one. The assertions below are **not** the snapshot, so the test
//! is not self-confirming.

#![allow(
    clippy::disallowed_types,
    clippy::disallowed_methods,
    clippy::disallowed_macros
)]

use pfp_server::openapi_json;
use serde_json::Value;

fn document() -> Value {
    serde_json::from_str(&openapi_json()).expect("the document is JSON")
}

#[test]
fn openapi_v1() {
    insta::assert_snapshot!("openapi_v1", openapi_json());
}

fn assert_sorted(value: &Value, at: &str) {
    match value {
        Value::Object(map) => {
            let keys: Vec<&String> = map.keys().collect();
            let mut sorted = keys.clone();
            sorted.sort();
            assert_eq!(keys, sorted, "keys at {at}");
            for (key, child) in map {
                assert_sorted(child, &format!("{at}/{key}"));
            }
        }
        Value::Array(items) => items.iter().for_each(|item| assert_sorted(item, at)),
        _ => {}
    }
}

#[test]
fn generation_is_deterministic_sorted_and_newline_terminated() {
    let first = openapi_json();
    assert_eq!(first, openapi_json());
    assert!(first.ends_with("}\n") && !first.ends_with("\n\n"));

    assert_sorted(&document(), "");
}

/// The method half of this test is a statement about the seven M0 operations, which
/// are `POST` by contract (README, "API methods") — not a rule that the API may
/// never have a `GET`. It is therefore tied to the admission rule: the change that
/// flips `API_READ_RULE` to admit `GET` reads is the change that adds `get`
/// operations, and it replaces the method assertion with a per-path table then.
#[test]
fn every_path_is_under_api_v1_and_the_m0_operations_are_post() {
    let doc = document();
    let paths = doc["paths"].as_object().unwrap();
    let mut names: Vec<&str> = paths.keys().map(String::as_str).collect();
    names.sort_unstable();
    let mut expected = pfp_server::api::PATHS.to_vec();
    expected.sort_unstable();
    assert_eq!(
        names, expected,
        "the document and the router name the same paths"
    );
    for (path, item) in paths {
        assert!(path.starts_with("/api/v1/"), "{path}");
        let methods: Vec<&String> = item.as_object().unwrap().keys().collect();
        if pfp_server::admission::API_READ_RULE == pfp_server::admission::ApiReadRule::PostOnly {
            assert_eq!(methods, ["post"], "{path}");
        } else {
            assert!(
                methods.iter().any(|m| *m == "post" || *m == "get"),
                "{path}"
            );
        }
    }
}

#[test]
fn every_operation_but_bootstrap_declares_the_session() {
    let doc = document();
    for (path, item) in doc["paths"].as_object().unwrap() {
        let security = &item["post"]["security"];
        if path.ends_with("/session/bootstrap") {
            assert!(security.is_null(), "{path}");
        } else if path.ends_with("/session/relaunch") {
            assert_eq!(
                security[0].as_object().unwrap().len(),
                1,
                "{path}: proof only"
            );
        } else {
            assert_eq!(
                security[0].as_object().unwrap().len(),
                2,
                "{path}: cookie and proof"
            );
        }
    }
    let schemes = &doc["components"]["securitySchemes"];
    assert_eq!(schemes["sessionCookie"]["in"], "cookie");
    assert_eq!(schemes["sessionCookie"]["name"], "__Host-pfp");
    assert_eq!(schemes["sessionProof"]["in"], "header");
    assert_eq!(schemes["sessionProof"]["name"], "X-PFP-Proof");
}

#[test]
fn money_fields_carry_x_money() {
    let doc = document();
    let schemas = doc["components"]["schemas"].as_object().unwrap();
    assert_eq!(schemas["Cents"]["x-money"], true);
    assert_eq!(schemas["Cents"]["type"], "integer");
    let money = |schema: &str, field: &str| {
        let property = &schemas[schema]["properties"][field];
        assert_eq!(
            property["$ref"], "#/components/schemas/Cents",
            "{schema}.{field} is money"
        );
    };
    money("LineDto", "value");
    money("RateScheduleRequest", "taxableIncome");
    money("RateScheduleResponse", "taxableIncome");
    money("RateScheduleResponse", "tax");
    // No other integer property anywhere is an amount in disguise: integers are
    // years, lags and ratio terms, by name, or a count of things (the validation
    // report's integers are all counts, named `count` or `…Count`).
    for (name, schema) in schemas {
        for (field, property) in schema["properties"].as_object().into_iter().flatten() {
            if property["type"] == "integer" {
                assert!(
                    [
                        "year",
                        "lagYears",
                        "baseYear",
                        "firstAdjustedYear",
                        "num",
                        "den"
                    ]
                    .contains(&field.as_str())
                        || field == "count"
                        || field.ends_with("Count"),
                    "{name}.{field} is an integer that is not declared as money or a count"
                );
            }
        }
    }
}

#[test]
fn no_schema_names_a_secret_beyond_the_handshake_pair() {
    let doc = document();
    let mut found = Vec::new();
    for (name, schema) in doc["components"]["schemas"].as_object().unwrap() {
        for field in schema["properties"]
            .as_object()
            .into_iter()
            .flatten()
            .map(|(k, _)| k)
        {
            let lower = field.to_ascii_lowercase();
            if [
                "token",
                "cookie",
                "proof",
                "secret",
                "password",
                "passphrase",
            ]
            .iter()
            .any(|word| lower.contains(word))
            {
                found.push(format!("{name}.{field}"));
            }
        }
    }
    found.sort();
    assert_eq!(found, ["BootstrapRequest.token", "BootstrapResponse.proof"]);
}

#[test]
fn every_refusal_is_the_error_body() {
    let doc = document();
    for (path, item) in doc["paths"].as_object().unwrap() {
        for (status, response) in item["post"]["responses"].as_object().unwrap() {
            if status.starts_with('4') || status.starts_with('5') {
                assert_eq!(
                    response["content"]["application/json"]["schema"]["$ref"],
                    "#/components/schemas/ErrorBody",
                    "{path} {status}"
                );
            }
        }
    }
}
