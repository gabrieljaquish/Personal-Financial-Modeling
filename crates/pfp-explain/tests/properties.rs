//! Property tests for the `Lines` trace. Every input is random and SYNTHETIC.

use std::collections::BTreeSet;

use pfp_explain::{Line, LineId, Lines};
use pfp_money::Cents;
use proptest::prelude::*;

fn config() -> ProptestConfig {
    ProptestConfig {
        cases: 256,
        failure_persistence: None,
        ..ProptestConfig::default()
    }
}

/// A random acyclic worksheet: line `k` draws its inputs from lines `0..k`.
fn worksheet() -> impl Strategy<Value = Vec<(i64, Vec<prop::sample::Index>)>> {
    prop::collection::vec(
        (
            any::<i64>(),
            prop::collection::vec(any::<prop::sample::Index>(), 0..6),
        ),
        1..24,
    )
}

fn build(spec: &[(i64, Vec<prop::sample::Index>)]) -> Lines {
    let mut lines = Lines::new();
    for (k, (value, picks)) in spec.iter().enumerate() {
        let inputs = picks
            .iter()
            .filter(|_| k > 0)
            .map(|pick| LineId::new(format!("w.l{}", pick.index(k))).unwrap());
        let line = Line::new(
            LineId::new(format!("w.l{k}")).unwrap(),
            "line",
            Cents(*value),
        );
        lines.push(line.inputs(inputs)).unwrap();
    }
    lines
}

proptest! {
    #![proptest_config(config())]

    /// Serialization is a pure function of the trace and reads back identically,
    /// in the same order.
    #[test]
    fn json_round_trips_in_order(spec in worksheet()) {
        let lines = build(&spec);
        let json = serde_json::to_string(&lines).unwrap();
        let back: Lines = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(&back, &lines);
        prop_assert_eq!(serde_json::to_string(&back).unwrap(), json);
        prop_assert_eq!(serde_json::to_string(&build(&spec)).unwrap(), serde_json::to_string(&lines).unwrap());
    }

    /// Computation order is a topological order: every input precedes its user.
    #[test]
    fn every_input_precedes_the_line_that_reads_it(spec in worksheet()) {
        let lines = build(&spec);
        let mut seen = BTreeSet::new();
        for line in &lines {
            for input in &line.inputs {
                prop_assert!(seen.contains(input));
            }
            prop_assert!(seen.insert(line.id.clone()));
        }
    }

    /// A trail is closed under "is an input of", ends at the line asked for,
    /// holds no line twice and follows computation order.
    #[test]
    fn a_trail_is_closed_and_ordered(spec in worksheet(), pick in any::<prop::sample::Index>()) {
        let lines = build(&spec);
        let root = &lines.as_slice()[pick.index(lines.len())].id;
        let trail = lines.trail(root).unwrap();
        prop_assert_eq!(&trail.last().unwrap().id, root);
        let ids: BTreeSet<&LineId> = trail.iter().map(|l| &l.id).collect();
        prop_assert_eq!(ids.len(), trail.len());
        for line in &trail {
            for input in &line.inputs {
                prop_assert!(ids.contains(input));
            }
        }
        let positions: Vec<usize> = trail
            .iter()
            .map(|l| lines.iter().position(|m| m.id == l.id).unwrap())
            .collect();
        prop_assert!(positions.windows(2).all(|w| w[0] < w[1]));
    }

    /// Reversing a trace with at least one dependency is refused on the way in.
    #[test]
    fn a_reversed_trace_does_not_deserialize(spec in worksheet()) {
        let lines = build(&spec);
        prop_assume!(lines.iter().any(|l| !l.inputs.is_empty()));
        let mut reversed: Vec<Line> = lines.into_iter().collect();
        reversed.reverse();
        let json = serde_json::to_string(&reversed).unwrap();
        prop_assert!(serde_json::from_str::<Lines>(&json).is_err());
    }
}
