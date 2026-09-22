//! Malformed tables are rejected with precise errors (`TESTING.md` §11.2 gate 9).
//!
//! SYNTHETIC: every amount, index value, URL and checksum below is invented for
//! the test and describes no law, person or publication
//! (`docs/contributing.md` §1.3).

// Test documents are assembled as text; clarity beats an allocation here.
#![allow(clippy::format_push_string, clippy::type_complexity)]

use pfp_params::{IndexSeries, ParamError, ParamTable, Vintage};

/// A valid single-amount table in the documented shape. SYNTHETIC.
const GOOD: &str = r#"
id           = "test.amount"
unit         = "USD"
period       = "year"
breakdown    = ["filingStatus"]
as_of        = "2001-01-01"
verification = "pending-hand-verification"

[values.single]
2001 = 1000
[values.mfj]
2001 = 2000
[values.mfs]
2001 = 1000
[values.hoh]
2001 = 1500
[values.qss]
2001 = 2000

[projection]
rule         = "index"
index        = "test.index"
index_series = "test.series"
lag_years    = 1
base_year    = 2000
[projection.base_values]
single = 1000
mfj    = 2000
mfs    = 1000
hoh    = 1500
qss    = 2000
[projection.rounding]
increment = 50
direction = "down"
basis     = "IncreaseOverBase"

[[source]]
title     = "Synthetic source"
url       = "https://example.invalid/doc.pdf"
retrieved = "2001-02-03"
sha256    = "0000000000000000000000000000000000000000000000000000000000000000"
"#;

/// A valid series. SYNTHETIC.
const SERIES: &str = r#"
id    = "test.series.table"
kind  = "index-series"
index_series = "test.series"
as_of = "2001-01-01"
[window]
kind = "trailing-12-month-mean"
ends_month = 12
months = 2
[observations.2000]
"2000-M11" = "100.0"
"2000-M12" = "100.0"
[window_sum]
2000 = "200.0"
[[source]]
title     = "Synthetic series"
url       = "https://example.invalid/series"
retrieved = "2001-02-03"
sha256    = "0000000000000000000000000000000000000000000000000000000000000000"
"#;

fn swap(text: &str, from: &str, to: &str) -> String {
    assert!(
        text.contains(from),
        "the mutation target is present: {from}"
    );
    text.replacen(from, to, 1)
}

fn table_err(text: &str) -> ParamError {
    ParamTable::parse(text).expect_err("the mutated table must be rejected")
}

fn field_of(e: &ParamError) -> (&str, &str) {
    match e {
        ParamError::Missing { table, field }
        | ParamError::WrongType { table, field, .. }
        | ParamError::Float { table, field }
        | ParamError::UnknownField { table, field }
        | ParamError::Invalid { table, field, .. }
        | ParamError::WidthMismatch { table, field, .. } => (table, field),
        other => panic!("no field on {other:?}"),
    }
}

#[test]
fn the_baseline_is_valid() {
    let t = ParamTable::parse(GOOD).unwrap();
    assert_eq!(t.id(), "test.amount");
    assert!(t.components().is_empty());
    assert_eq!(t.sources()[0].archive(), None);
    IndexSeries::parse(SERIES).unwrap();
}

#[test]
fn increase_over_base_needs_its_three_inputs() {
    let strip = |text: &str, line: &str| swap(text, line, "");
    let no_base_year = strip(GOOD, "base_year    = 2000\n");
    assert_eq!(
        table_err(&no_base_year),
        ParamError::IncreaseOverBaseIncomplete {
            table: "test.amount".into(),
            missing: vec!["base_year"],
        }
    );
    let no_series = strip(GOOD, "index_series = \"test.series\"\n");
    assert_eq!(
        table_err(&no_series),
        ParamError::IncreaseOverBaseIncomplete {
            table: "test.amount".into(),
            missing: vec!["index_series"],
        }
    );
    let no_base_values = strip(
        GOOD,
        "[projection.base_values]\nsingle = 1000\nmfj    = 2000\nmfs    = 1000\nhoh    = 1500\nqss    = 2000\n",
    );
    assert_eq!(
        table_err(&no_base_values),
        ParamError::IncreaseOverBaseIncomplete {
            table: "test.amount".into(),
            missing: vec!["base_values"],
        }
    );
    let none = strip(
        &strip(&no_base_values, "base_year    = 2000\n"),
        "index_series = \"test.series\"\n",
    );
    let e = table_err(&none);
    assert_eq!(
        e,
        ParamError::IncreaseOverBaseIncomplete {
            table: "test.amount".into(),
            missing: vec!["base_year", "base_values", "index_series"],
        }
    );
    assert_eq!(
        e.to_string(),
        "test.amount: basis IncreaseOverBase needs `projection.base_year`, `projection.base_values`, `projection.index_series`"
    );
}

#[test]
fn missing_provenance_is_an_error() {
    let cut = GOOD.find("[[source]]").unwrap();
    assert_eq!(
        table_err(&GOOD[..cut]),
        ParamError::MissingProvenance {
            table: "test.amount".into()
        }
    );
    for (from, to, field) in [
        (
            "sha256    = \"0000",
            "sha256    = \"000G",
            "source[0].sha256",
        ),
        ("sha256    = \"0000", "sha256    = \"00", "source[0].sha256"),
        (
            "https://example.invalid/doc.pdf",
            "http://example.invalid/doc.pdf",
            "source[0].url",
        ),
        (
            "retrieved = \"2001-02-03\"",
            "retrieved = \"2001-02-30\"",
            "source[0].retrieved",
        ),
        (
            "title     = \"Synthetic source\"",
            "title     = \" \"",
            "source[0].title",
        ),
        (
            "title     = \"Synthetic source\"",
            "title = \"t\"\narchive = \"local/secret.pdf\"",
            "source[0].archive",
        ),
    ] {
        let e = table_err(&swap(GOOD, from, to));
        assert!(matches!(e, ParamError::Invalid { .. }), "{e:?}");
        assert_eq!(field_of(&e), ("test.amount", field));
    }
    for line in [
        "title     = \"Synthetic source\"\n",
        "retrieved = \"2001-02-03\"\n",
    ] {
        let e = table_err(&swap(GOOD, line, ""));
        assert!(matches!(e, ParamError::Missing { .. }), "{e:?}");
    }
    let e = table_err(&swap(GOOD, "as_of        = \"2001-01-01\"\n", ""));
    assert_eq!(field_of(&e), ("test.amount", "as_of"));
}

#[test]
fn a_table_without_a_projection_rule_is_an_error() {
    let cut = GOOD.find("[projection]").unwrap();
    let tail = GOOD.find("[[source]]").unwrap();
    let no_block = format!("{}{}", &GOOD[..cut], &GOOD[tail..]);
    let want = ParamError::MissingProjectionRule {
        table: "test.amount".into(),
    };
    assert_eq!(table_err(&no_block), want);
    assert_eq!(
        table_err(&swap(GOOD, "rule         = \"index\"\n", "")),
        want
    );
    let e = table_err(&swap(GOOD, "rule         = \"index\"", "rule = \"cpi\""));
    assert_eq!(field_of(&e), ("test.amount", "projection.rule"));

    let cut = GOOD.find("[projection.rounding]").unwrap();
    let no_rounding = format!("{}{}", &GOOD[..cut], &GOOD[tail..]);
    assert_eq!(
        table_err(&no_rounding),
        ParamError::MissingRounding {
            table: "test.amount".into()
        }
    );
    // A never-indexed table says so, and needs none of the index inputs.
    let flat = format!(
        "{}[projection]\nrule = \"flat\"\n{}",
        &GOOD[..GOOD.find("[projection]").unwrap()],
        &GOOD[tail..]
    );
    ParamTable::parse(&flat).unwrap();
}

#[test]
fn breakdown_keys_are_filing_status_wire_forms_and_all_of_them() {
    let e = table_err(&swap(GOOD, "[values.mfj]", "[values.JOINT]"));
    assert_eq!(
        e,
        ParamError::UnknownBreakdownKey {
            table: "test.amount".into(),
            field: "values".into(),
            key: "JOINT".into(),
        }
    );
    let e = table_err(&swap(GOOD, "[values.qss]\n2001 = 2000\n", ""));
    assert_eq!(
        e,
        ParamError::MissingBreakdownKey {
            table: "test.amount".into(),
            field: "values".into(),
            key: "qss".into(),
        }
    );
    let e = table_err(&swap(GOOD, "hoh    = 1500\n", ""));
    assert_eq!(
        e,
        ParamError::MissingBreakdownKey {
            table: "test.amount".into(),
            field: "projection.base_values".into(),
            key: "hoh".into(),
        }
    );
    let e = table_err(&swap(GOOD, "hoh    = 1500\n", "hoh = 1500\nMarried = 1\n"));
    assert!(matches!(e, ParamError::UnknownBreakdownKey { ref key, .. } if key == "Married"));
    let e = table_err(&swap(GOOD, "[\"filingStatus\"]", "[\"state\"]"));
    assert_eq!(
        e,
        ParamError::UnsupportedBreakdown {
            table: "test.amount".into(),
            breakdown: "state".into(),
        }
    );
}

#[test]
fn floats_wrong_types_and_bad_values_name_their_field() {
    let cases: [(&str, &str, &str, fn(&ParamError) -> bool); 12] = [
        ("2001 = 1500", "2001 = 1500.0", "values.hoh.2001", |e| {
            matches!(e, ParamError::Float { .. })
        }),
        ("2001 = 1500", "2001 = \"1500\"", "values.hoh.2001", |e| {
            matches!(e, ParamError::WrongType { .. })
        }),
        ("2001 = 1500", "2001 = -1", "values.hoh.2001", |e| {
            matches!(e, ParamError::Invalid { .. })
        }),
        ("2001 = 1500", "1 = 1500", "values.hoh.1", |e| {
            matches!(e, ParamError::Invalid { .. })
        }),
        ("2001 = 1500", "2001 = [1500]", "values.hoh.2001", |e| {
            matches!(e, ParamError::WrongType { .. })
        }),
        (
            "increment = 50",
            "increment = 0.5",
            "projection.rounding.increment",
            |e| matches!(e, ParamError::Float { .. }),
        ),
        (
            "increment = 50",
            "increment = 0",
            "projection.rounding.increment",
            |e| matches!(e, ParamError::Invalid { .. }),
        ),
        (
            "direction = \"down\"",
            "direction = \"floor\"",
            "projection.rounding.direction",
            |e| matches!(e, ParamError::Invalid { .. }),
        ),
        (
            "basis     = \"IncreaseOverBase\"",
            "basis = \"Total\"",
            "projection.rounding.basis",
            |e| matches!(e, ParamError::Invalid { .. }),
        ),
        (
            "base_year    = 2000",
            "base_year = 20000",
            "projection.base_year",
            |e| matches!(e, ParamError::Invalid { .. }),
        ),
        // DOMAIN-MODEL §15: a zero base value means "not yet transcribed".
        (
            "hoh    = 1500",
            "hoh = 0",
            "projection.base_values.hoh",
            |e| matches!(e, ParamError::Invalid { .. }),
        ),
        ("unit         = \"USD\"", "unit = \"EUR\"", "unit", |e| {
            matches!(e, ParamError::Invalid { .. })
        }),
    ];
    for (from, to, field, is_kind) in cases {
        let e = table_err(&swap(GOOD, from, to));
        assert!(is_kind(&e), "{to}: {e:?}");
        assert_eq!(field_of(&e), ("test.amount", field), "{to}");
        assert!(e.to_string().contains(field), "{e}");
    }
}

#[test]
fn unknown_fields_and_double_statements_are_rejected() {
    let e = table_err(&swap(GOOD, "period       = \"year\"", "perod = \"year\""));
    assert_eq!(
        e,
        ParamError::UnknownField {
            table: "test.amount".into(),
            field: "perod".into()
        }
    );
    let e = table_err(&swap(
        GOOD,
        "base_year    = 2000",
        "base_year = 2000\nbase_yr = 1",
    ));
    assert_eq!(field_of(&e), ("test.amount", "projection.base_yr"));
    let e = table_err(&swap(
        GOOD,
        "increment = 50",
        "increment = 50\nmode = \"x\"",
    ));
    assert_eq!(field_of(&e), ("test.amount", "projection.rounding.mode"));

    let both = format!(
        "{GOOD}\n[projection.rounding.increment_by_key]\nsingle = 25\nmfj = 50\nmfs = 25\nhoh = 50\nqss = 50\n"
    );
    assert_eq!(
        table_err(&both),
        ParamError::Conflict {
            table: "test.amount".into(),
            first: "projection.rounding.increment",
            second: "projection.rounding.increment_by_key",
        }
    );
    let e = table_err(&swap(GOOD, "id           = \"test.amount\"\n", ""));
    assert_eq!(field_of(&e), ("<no id>", "id"));
    assert!(matches!(table_err("id = "), ParamError::Syntax { .. }));
}

/// The vector shape: `edges`, arrays, per-edge base years, `[rates]`. SYNTHETIC.
fn vector_table() -> String {
    let mut s = String::from(
        "id = \"test.edges\"\nunit = \"USD\"\nbreakdown = [\"filingStatus\"]\nas_of = \"2001-01-01\"\nedges = [\"a\", \"b\"]\n",
    );
    for key in ["single", "mfj", "mfs", "hoh", "qss"] {
        s += &format!("[values.{key}]\n2001 = [100, 200]\n");
    }
    s += "[rates]\nunit = \"ratio\"\nrule = \"flat\"\n2001 = [\"0.1\", \"0.2\", \"0.3\"]\n";
    s += "[projection]\nrule = \"index\"\nindex = \"i\"\nindex_series = \"test.series\"\nlag_years = 1\n";
    s += "[projection.base_values]\n";
    for key in ["single", "mfj", "mfs", "hoh", "qss"] {
        s += &format!("{key} = [100, 200]\n");
    }
    s += "[projection.base_year_by_edge.2001]\na = 2000\nb = 2000\n";
    s += "[projection.rounding]\nincrement = 50\ndirection = \"down\"\nbasis = \"IncreaseOverBase\"\n";
    s += &GOOD[GOOD.find("[[source]]").unwrap()..];
    s
}

#[test]
fn the_vector_shape_is_checked_against_its_edges() {
    let good = vector_table();
    let t = ParamTable::parse(&good).unwrap();
    assert_eq!(t.components(), ["a", "b"]);
    assert_eq!(t.rates(2001).unwrap().len(), 3);

    let e = table_err(&swap(
        &good,
        "[values.hoh]\n2001 = [100, 200]",
        "[values.hoh]\n2001 = [100]",
    ));
    assert_eq!(
        e,
        ParamError::WidthMismatch {
            table: "test.edges".into(),
            field: "values.hoh.2001".into(),
            expected: 2,
            found: 1,
        }
    );
    let e = table_err(&swap(
        &good,
        "[values.hoh]\n2001 = [100, 200]",
        "[values.hoh]\n2001 = [200, 100]",
    ));
    assert_eq!(field_of(&e), ("test.edges", "values.hoh.2001"));
    let e = table_err(&swap(
        &good,
        "[values.hoh]\n2001 = [100, 200]",
        "[values.hoh]\n2001 = 100",
    ));
    assert!(matches!(e, ParamError::WrongType { .. }));
    let e = table_err(&swap(&good, "hoh = [100, 200]", "hoh = [100, 200, 300]"));
    assert_eq!(field_of(&e), ("test.edges", "projection.base_values.hoh"));
    let e = table_err(&swap(
        &good,
        "[\"0.1\", \"0.2\", \"0.3\"]",
        "[\"0.1\", \"0.2\"]",
    ));
    assert_eq!(field_of(&e), ("test.edges", "rates.2001"));
    let e = table_err(&swap(
        &good,
        "[\"0.1\", \"0.2\", \"0.3\"]",
        "[0.1, 0.2, 0.3]",
    ));
    assert_eq!(
        e,
        ParamError::Float {
            table: "test.edges".into(),
            field: "rates.2001".into()
        }
    );
    let e = table_err(&swap(
        &good,
        "[\"0.1\", \"0.2\", \"0.3\"]",
        "[\"0.1\", \"0.2\", \"1.3\"]",
    ));
    assert_eq!(field_of(&e), ("test.edges", "rates.2001"));
    let e = table_err(&swap(
        &good,
        "rule = \"flat\"\n2001",
        "rule = \"index\"\n2001",
    ));
    assert_eq!(field_of(&e), ("test.edges", "rates.rule"));
    let e = table_err(&swap(&good, "a = 2000\nb = 2000", "a = 2000"));
    assert_eq!(
        field_of(&e),
        ("test.edges", "projection.base_year_by_edge.2001.b")
    );
    let e = table_err(&swap(
        &good,
        "a = 2000\nb = 2000",
        "a = 2000\nb = 2000\nc = 2000",
    ));
    assert_eq!(
        field_of(&e),
        ("test.edges", "projection.base_year_by_edge.2001.c")
    );
    let e = table_err(&swap(
        &good,
        "[projection.base_values]",
        "base_year = 2000\n[projection.base_values]",
    ));
    assert_eq!(
        e,
        ParamError::Conflict {
            table: "test.edges".into(),
            first: "projection.base_year",
            second: "projection.base_year_by_edge",
        }
    );
    let e = table_err(&swap(
        &good,
        "edges = [\"a\", \"b\"]",
        "edges = [\"a\", \"a\"]",
    ));
    assert_eq!(field_of(&e), ("test.edges", "edges"));
    // `[rates]` and per-edge base years mean nothing on a single-amount table.
    let e = table_err(&format!(
        "{GOOD}\n[rates]\nunit = \"ratio\"\nrule = \"flat\"\n2001 = [\"0.1\"]\n"
    ));
    assert_eq!(field_of(&e), ("test.amount", "rates"));
}

#[test]
fn a_derivation_must_name_a_real_underived_key() {
    let ok = format!("{GOOD}\n[projection.derived.mfj]\nfrom = \"single\"\nmultiplier = \"2\"\n");
    let t = ParamTable::parse(&ok).unwrap();
    assert_eq!(t.projection().derived["mfj"].from, "single");
    // A derived key needs no base value of its own.
    ParamTable::parse(&swap(&ok, "mfj    = 2000\nmfs", "mfs")).unwrap();
    for (from, to) in [
        ("from = \"single\"", "from = \"joint\""),
        ("from = \"single\"", "from = \"mfj\""),
        ("multiplier = \"2\"", "multiplier = \"0\""),
        ("multiplier = \"2\"", "multiplier = \"two\""),
    ] {
        let e = table_err(&swap(&ok, from, to));
        assert!(matches!(e, ParamError::Invalid { .. }), "{to}: {e:?}");
    }
    let e = table_err(&swap(&ok, "multiplier = \"2\"", "multiplier = 2.0"));
    assert!(matches!(e, ParamError::Float { .. }), "{e:?}");
    let e = table_err(&swap(
        &ok,
        "[projection.derived.mfj]",
        "[projection.derived.joint]",
    ));
    assert!(matches!(e, ParamError::UnknownBreakdownKey { .. }), "{e:?}");
}

#[test]
fn a_malformed_series_is_rejected() {
    let err = |text: &str| IndexSeries::parse(text).expect_err("rejected");
    assert_eq!(
        err(&swap(SERIES, "2000 = \"200.0\"", "2000 = \"200.1\"")),
        ParamError::WindowSumMismatch {
            table: "test.series.table".into(),
            year: 2000
        }
    );
    // A missing month, a wrong month and a float observation.
    let e = err(&swap(SERIES, "\"2000-M11\" = \"100.0\"\n", ""));
    assert!(
        matches!(
            e,
            ParamError::WidthMismatch {
                expected: 2,
                found: 1,
                ..
            }
        ),
        "{e:?}"
    );
    let e = err(&swap(SERIES, "\"2000-M11\"", "\"2000-M10\""));
    assert_eq!(
        field_of(&e),
        ("test.series.table", "observations.2000.2000-M11")
    );
    let e = err(&swap(
        SERIES,
        "\"2000-M12\" = \"100.0\"",
        "\"2000-M12\" = 100.0",
    ));
    assert!(matches!(e, ParamError::Float { .. }), "{e:?}");
    let e = err(&swap(
        SERIES,
        "\"2000-M12\" = \"100.0\"",
        "\"2000-M12\" = \"1e2\"",
    ));
    assert!(matches!(e, ParamError::Invalid { .. }), "{e:?}");
    let e = err(&swap(
        SERIES,
        "kind  = \"index-series\"",
        "kind = \"table\"",
    ));
    assert_eq!(field_of(&e), ("test.series.table", "kind"));
    let cut = SERIES.find("[[source]]").unwrap();
    assert!(matches!(
        err(&SERIES[..cut]),
        ParamError::MissingProvenance { .. }
    ));
    // A series that does not say what name tables read it under binds to nothing.
    let e = err(&swap(SERIES, "index_series = \"test.series\"\n", ""));
    assert_eq!(field_of(&e), ("test.series.table", "index_series"));
    let e = err(&swap(
        SERIES,
        "index_series = \"test.series\"",
        "index_series = \"\"",
    ));
    assert_eq!(field_of(&e), ("test.series.table", "index_series"));
    // A parameter table is not a series, and the reverse.
    assert!(IndexSeries::parse(GOOD).is_err());
    assert!(ParamTable::parse(SERIES).is_err());
}

#[test]
fn an_indexed_table_says_which_year_of_the_series_it_reads() {
    let e = table_err(&swap(GOOD, "lag_years    = 1\n", ""));
    assert_eq!(
        e,
        ParamError::Missing {
            table: "test.amount".into(),
            field: "projection.lag_years".into(),
        }
    );
    for bad in ["-1", "11", "\"1\"", "1.0"] {
        let e = table_err(&swap(
            GOOD,
            "lag_years    = 1",
            &format!("lag_years = {bad}"),
        ));
        assert_eq!(
            field_of(&e),
            ("test.amount", "projection.lag_years"),
            "{bad}"
        );
    }
    ParamTable::parse(&swap(GOOD, "lag_years    = 1", "lag_years = 0")).unwrap();
}

#[test]
fn a_vintage_resolves_every_index_series_and_rejects_duplicates() {
    let v = Vintage::parse("synthetic", &[GOOD], &[SERIES]).unwrap();
    assert_eq!(
        v.index_series("test.series").unwrap().id(),
        "test.series.table"
    );
    assert_eq!(
        Vintage::parse("synthetic", &[GOOD], &[]).unwrap_err(),
        ParamError::UnresolvedIndexSeries {
            table: "test.amount".into(),
            index_series: "test.series".into(),
        }
    );
    assert_eq!(
        Vintage::parse("synthetic", &[GOOD, GOOD], &[SERIES]).unwrap_err(),
        ParamError::DuplicateTable {
            table: "test.amount".into()
        }
    );
    assert!(matches!(
        Vintage::parse("synthetic", &[GOOD], &[SERIES, SERIES]).unwrap_err(),
        ParamError::DuplicateTable { .. }
    ));
}
