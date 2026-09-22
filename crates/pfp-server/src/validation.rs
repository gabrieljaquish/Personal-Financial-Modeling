//! The validation report compiled into this binary (`TESTING.md` §13; `PLAN.md`
//! §4.13 item 8).
//!
//! `cargo xtask validation-report` computes the report from the repository and
//! writes `build/validation-report.json`; the build script (`build.rs`) sets
//! `cfg(pfp_validation_report)` when that file exists and this module embeds it
//! with `include_str!`, exactly as `assets.rs` embeds `web/dist`. Nothing is read
//! at run time, in any profile.
//!
//! Without the file — a debug build on a checkout where nobody ran the command —
//! [`embedded`] is `Ok(None)` and the API serves an explicit "not generated"
//! state; a release build without it does not compile (`build.rs`). An embedded
//! report whose shape is not [`ValidationReport`] is a startup error, never a
//! report with a hole in it: the DTO refuses unknown fields, so the JSON writer
//! (`xtask`, which compiles the same DTO file) and this reader cannot drift apart
//! silently.

use std::fmt;

use crate::api::report_dto::ValidationReport;

/// The generated report, when the build embedded one.
#[cfg(pfp_validation_report)]
const EMBEDDED_JSON: &str = include_str!("../../../build/validation-report.json");

/// Why the embedded report could not be used.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReportError {
    /// The embedded JSON is not a [`ValidationReport`]; the detail names the
    /// field, never a value.
    Shape(String),
}

impl fmt::Display for ReportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Shape(detail) => write!(
                f,
                "the embedded validation report is not the shape this build reads ({detail}); \
run `cargo xtask validation-report` and rebuild"
            ),
        }
    }
}

impl std::error::Error for ReportError {}

/// The report compiled into this binary: `Ok(None)` when none was generated
/// (debug builds only; a release build refuses to compile without one).
///
/// # Errors
/// [`ReportError`] when what was embedded does not parse as the report.
pub fn embedded() -> Result<Option<ValidationReport>, ReportError> {
    #[cfg(pfp_validation_report)]
    {
        parse(EMBEDDED_JSON).map(Some)
    }
    #[cfg(not(pfp_validation_report))]
    {
        Ok(None)
    }
}

/// Whether this build embeds a report.
#[must_use]
pub const fn is_embedded() -> bool {
    cfg!(pfp_validation_report)
}

/// Parses a report the generator wrote.
///
/// # Errors
/// [`ReportError::Shape`] on any deviation from the DTO, unknown fields included.
pub fn parse(json: &str) -> Result<ValidationReport, ReportError> {
    // `serde_json`'s message names the offending key or type and its position; a
    // report carries no user data, so the message is safe to print as is.
    serde_json::from_str(json).map_err(|e| ReportError::Shape(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_is_either_absent_or_the_report() {
        match embedded() {
            Ok(Some(report)) => {
                assert!(is_embedded());
                let tiers: Vec<&str> = report
                    .fixtures
                    .tiers
                    .iter()
                    .map(|t| t.tier.as_str())
                    .collect();
                assert_eq!(tiers, ["tier1", "tier2", "tier3", "pending"]);
            }
            Ok(None) => assert!(!is_embedded()),
            Err(e) => panic!("{e}"),
        }
    }

    #[test]
    fn a_report_with_an_unknown_field_is_refused() {
        let Ok(Some(report)) = embedded() else {
            return;
        };
        let mut value = serde_json::to_value(&report).unwrap();
        value["pins"]["surprise"] = serde_json::Value::Bool(true);
        let error = parse(&value.to_string()).unwrap_err();
        assert!(matches!(error, ReportError::Shape(ref d) if d.contains("surprise")));
    }
}
