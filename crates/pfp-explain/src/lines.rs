//! `Lines`: what a worksheet returns.

use core::fmt;
use std::collections::{BTreeMap, BTreeSet};

use pfp_money::Cents;
use serde::{Deserialize, Serialize};

use crate::id::LineId;
use crate::line::Line;

/// Why a line cannot join a trace.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TraceError {
    /// A line with this id is already present.
    DuplicateLine(LineId),
    /// `line` names `input`, which is not (yet) present. Lines are pushed in
    /// computation order, so this also catches a cycle and a self-reference.
    UnknownInput {
        /// The line being added.
        line: LineId,
        /// The input it names.
        input: LineId,
    },
}

impl fmt::Display for TraceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateLine(id) => write!(f, "line {id} is already in the trace"),
            Self::UnknownInput { line, input } => {
                write!(
                    f,
                    "line {line} names input {input}, which is not in the trace before it"
                )
            }
        }
    }
}

impl std::error::Error for TraceError {}

/// An ordered, validated set of [`Line`]s: every id unique, every input
/// resolving to an earlier line. Iteration and serialization follow computation
/// order, which is therefore a topological order of an acyclic graph.
///
/// On the wire it is the JSON array of its lines; reading one back re-validates it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "Vec<Line>", into = "Vec<Line>")]
pub struct Lines {
    lines: Vec<Line>,
    index: BTreeMap<LineId, usize>,
}

impl Lines {
    /// An empty trace.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends `line`.
    ///
    /// # Errors
    ///
    /// [`TraceError`] when the id is taken or an input is not already present;
    /// the trace is unchanged.
    pub fn push(&mut self, line: Line) -> Result<(), TraceError> {
        if self.index.contains_key(&line.id) {
            return Err(TraceError::DuplicateLine(line.id));
        }
        if let Some(input) = line.inputs.iter().find(|i| !self.index.contains_key(*i)) {
            return Err(TraceError::UnknownInput {
                input: input.clone(),
                line: line.id,
            });
        }
        self.index.insert(line.id.clone(), self.lines.len());
        self.lines.push(line);
        Ok(())
    }

    /// The line with this id.
    #[must_use]
    pub fn get(&self, id: &LineId) -> Option<&Line> {
        self.index.get(id).map(|&i| &self.lines[i])
    }

    /// The value of the line with this id.
    #[must_use]
    pub fn value(&self, id: &LineId) -> Option<Cents> {
        self.get(id).map(|line| line.value)
    }

    /// The most recently computed line: a worksheet's result.
    #[must_use]
    pub fn last(&self) -> Option<&Line> {
        self.lines.last()
    }

    /// The lines, in computation order.
    pub fn iter(&self) -> core::slice::Iter<'_, Line> {
        self.lines.iter()
    }

    /// The lines, in computation order.
    #[must_use]
    pub fn as_slice(&self) -> &[Line] {
        &self.lines
    }

    /// How many lines.
    #[must_use]
    pub fn len(&self) -> usize {
        self.lines.len()
    }

    /// Whether there are no lines.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    /// The audit trail of one line: the line and everything it was computed
    /// from, transitively, in computation order with the line itself last.
    /// `None` when the id is not present.
    #[must_use]
    pub fn trail(&self, id: &LineId) -> Option<Vec<&Line>> {
        let root = *self.index.get(id)?;
        let mut seen = BTreeSet::from([root]);
        let mut pending = vec![root];
        while let Some(i) = pending.pop() {
            for input in &self.lines[i].inputs {
                // `push` guarantees every input is indexed.
                let j = self.index[input];
                if seen.insert(j) {
                    pending.push(j);
                }
            }
        }
        Some(seen.into_iter().map(|i| &self.lines[i]).collect())
    }
}

impl TryFrom<Vec<Line>> for Lines {
    type Error = TraceError;

    fn try_from(lines: Vec<Line>) -> Result<Self, TraceError> {
        let mut out = Self::new();
        for line in lines {
            out.push(line)?;
        }
        Ok(out)
    }
}

impl From<Lines> for Vec<Line> {
    fn from(lines: Lines) -> Self {
        lines.lines
    }
}

impl<'a> IntoIterator for &'a Lines {
    type Item = &'a Line;
    type IntoIter = core::slice::Iter<'a, Line>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl IntoIterator for Lines {
    type Item = Line;
    type IntoIter = std::vec::IntoIter<Line>;

    fn into_iter(self) -> Self::IntoIter {
        self.lines.into_iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(s: &'static str) -> LineId {
        LineId::from_static(s)
    }

    /// A SYNTHETIC two-bracket worksheet: income, two slices, a total, and one
    /// unrelated line the total does not depend on.
    fn worksheet() -> Lines {
        let mut lines = Lines::new();
        lines
            .push(Line::new(id("w.income"), "Income", Cents(500)))
            .unwrap();
        lines
            .push(Line::new(id("w.other"), "Unrelated", Cents(7)))
            .unwrap();
        lines
            .push(Line::new(id("w.b1"), "Slice 1", Cents(30)).input(id("w.income")))
            .unwrap();
        lines
            .push(Line::new(id("w.b0"), "Slice 0", Cents(20)).input(id("w.income")))
            .unwrap();
        lines
            .push(Line::new(id("w.total"), "Total", Cents(50)).inputs([id("w.b0"), id("w.b1")]))
            .unwrap();
        lines
    }

    #[test]
    fn order_is_computation_order_not_id_order() {
        let lines = worksheet();
        let order: Vec<&str> = lines.iter().map(|l| l.id.as_str()).collect();
        assert_eq!(order, ["w.income", "w.other", "w.b1", "w.b0", "w.total"]);
        assert_eq!(lines.len(), 5);
        assert!(!lines.is_empty());
        assert_eq!(lines.last().unwrap().id, id("w.total"));
        assert_eq!(lines.value(&id("w.b1")), Some(Cents(30)));
        assert_eq!(lines.value(&id("w.missing")), None);
        assert_eq!((&lines).into_iter().count(), 5);
        assert_eq!(lines.as_slice().len(), 5);
    }

    #[test]
    fn duplicates_forward_references_and_cycles_are_refused() {
        let mut lines = worksheet();
        let before = lines.clone();
        assert_eq!(
            lines.push(Line::new(id("w.b0"), "Again", Cents(1))),
            Err(TraceError::DuplicateLine(id("w.b0")))
        );
        assert_eq!(
            lines.push(Line::new(id("w.x"), "Forward", Cents(1)).input(id("w.y"))),
            Err(TraceError::UnknownInput {
                line: id("w.x"),
                input: id("w.y")
            })
        );
        assert_eq!(
            lines.push(Line::new(id("w.self"), "Self", Cents(1)).input(id("w.self"))),
            Err(TraceError::UnknownInput {
                line: id("w.self"),
                input: id("w.self")
            })
        );
        assert_eq!(lines, before, "a refused line leaves the trace unchanged");
    }

    #[test]
    fn a_trail_is_the_transitive_inputs_in_computation_order() {
        let lines = worksheet();
        let trail: Vec<&str> = lines
            .trail(&id("w.total"))
            .unwrap()
            .into_iter()
            .map(|l| l.id.as_str())
            .collect();
        assert_eq!(trail, ["w.income", "w.b1", "w.b0", "w.total"]);
        assert_eq!(lines.trail(&id("w.other")).unwrap().len(), 1);
        assert!(lines.trail(&id("w.missing")).is_none());
    }

    #[test]
    fn json_is_the_ordered_array_and_is_revalidated_on_the_way_in() {
        let lines = worksheet();
        let json = serde_json::to_string(&lines).unwrap();
        assert!(json.starts_with(r#"[{"id":"w.income","#));
        assert_eq!(serde_json::from_str::<Lines>(&json).unwrap(), lines);
        assert_eq!(serde_json::to_string(&Lines::new()).unwrap(), "[]");

        let forward =
            r#"[{"id":"a","label":"A","value":1,"inputs":["b"]},{"id":"b","label":"B","value":2}]"#;
        let err = serde_json::from_str::<Lines>(forward).unwrap_err();
        assert!(err.to_string().contains("names input b"), "{err}");
        let duplicate = r#"[{"id":"a","label":"A","value":1},{"id":"a","label":"A","value":1}]"#;
        assert!(serde_json::from_str::<Lines>(duplicate).is_err());
    }
}
