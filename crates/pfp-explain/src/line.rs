//! The `Line` trace node (`ARCHITECTURE.md` §4.2, seam S3).

use std::borrow::Cow;

use pfp_money::Cents;
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;

use crate::id::{LineId, ParamRef, RuleId};

/// One computed value with everything needed to audit it: which lines it was
/// computed from, which parameter cells it read, and which named rounding rule
/// produced its final cents.
///
/// One structure serves the fixture intermediate, the UI audit trail and the
/// effective-marginal-rate machinery. `inputs` and `params` keep the order the
/// worksheet gave them, which is the order of the operands in its formula.
///
/// `label` is `Cow<'static, str>` rather than the design sketch's `&'static str`:
/// an engine writes a literal and allocates nothing, and a trace read back from
/// JSON owns its text, which a `&'static str` field could not deserialize into.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Line {
    /// Stable id; the prefix names the worksheet.
    pub id: LineId,
    /// Human-readable name of the line.
    pub label: Cow<'static, str>,
    /// The line's value.
    pub value: Cents,
    /// The lines this value was computed from, in formula order.
    #[serde(default, skip_serializing_if = "SmallVec::is_empty")]
    pub inputs: SmallVec<[LineId; 4]>,
    /// The parameter cells this value read, in formula order.
    #[serde(default, skip_serializing_if = "SmallVec::is_empty")]
    pub params: SmallVec<[ParamRef; 2]>,
    /// The named rounding rule applied to reach `value`, if one was.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rounding: Option<RuleId>,
}

impl Line {
    /// A line with no inputs, parameters or rounding yet.
    #[must_use]
    pub fn new(id: LineId, label: impl Into<Cow<'static, str>>, value: Cents) -> Self {
        Self {
            id,
            label: label.into(),
            value,
            inputs: SmallVec::new(),
            params: SmallVec::new(),
            rounding: None,
        }
    }

    /// Appends an input line.
    #[must_use]
    pub fn input(mut self, id: LineId) -> Self {
        self.inputs.push(id);
        self
    }

    /// Appends several input lines, in order.
    #[must_use]
    pub fn inputs(mut self, ids: impl IntoIterator<Item = LineId>) -> Self {
        self.inputs.extend(ids);
        self
    }

    /// Appends a parameter reference.
    #[must_use]
    pub fn param(mut self, param: ParamRef) -> Self {
        self.params.push(param);
        self
    }

    /// Names the rounding rule that produced `value`.
    #[must_use]
    pub fn rounded_by(mut self, rule: RuleId) -> Self {
        self.rounding = Some(rule);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // SYNTHETIC values; the ids follow fixtures/pending/schedule/*.json.
    #[test]
    fn a_line_serializes_to_the_designs_six_fields() {
        let line = Line::new(
            LineId::from_static("sched.b1"),
            "Tax from the 12% bracket",
            Cents(902_400),
        )
        .input(LineId::from_static("sched.ti"))
        .param(
            ParamRef::new("irs.ordinary_brackets")
                .unwrap()
                .with_year(2026)
                .with_breakdown_key("mfj")
                .unwrap()
                .with_element("top_of_12")
                .unwrap(),
        )
        .rounded_by(RuleId::from_static("money.cent_half_even"));
        let json = serde_json::to_value(&line).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "id": "sched.b1",
                "label": "Tax from the 12% bracket",
                "value": 902_400,
                "inputs": ["sched.ti"],
                "params": [{
                    "paramId": "irs.ordinary_brackets", "year": 2026,
                    "breakdownKey": "mfj", "element": "top_of_12"
                }],
                "rounding": "money.cent_half_even"
            })
        );
        let back: Line = serde_json::from_value(json).unwrap();
        assert_eq!(back, line);
        assert!(matches!(back.label, Cow::Owned(_)));
    }

    #[test]
    fn a_leaf_line_omits_its_empty_lists() {
        let leaf = Line::new(
            LineId::from_static("sched.ti"),
            "Taxable income",
            Cents(10_000_000),
        );
        let json = serde_json::to_string(&leaf).unwrap();
        assert_eq!(
            json,
            r#"{"id":"sched.ti","label":"Taxable income","value":10000000}"#
        );
        assert_eq!(serde_json::from_str::<Line>(&json).unwrap(), leaf);
        assert!(!leaf.inputs.spilled() && !leaf.params.spilled());
    }

    #[test]
    fn unknown_fields_and_wrong_types_are_rejected() {
        for bad in [
            r#"{"id":"a","label":"A","value":1,"note":"x"}"#,
            r#"{"id":"a","label":"A","value":1.5}"#,
            r#"{"id":"A","label":"A","value":1}"#,
            r#"{"id":"a","value":1}"#,
            r#"{"id":"a","label":"A","value":1,"inputs":["B"]}"#,
        ] {
            assert!(serde_json::from_str::<Line>(bad).is_err(), "{bad}");
        }
    }
}
