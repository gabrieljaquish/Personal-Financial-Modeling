//! One year's rate schedule for one filing status, as read through `ParamView`.

use pfp_domain::{FilingStatus, Year};
use pfp_money::{Cents, Ratio};
use pfp_params::shipped::ORDINARY_BRACKETS;
use pfp_params::{Origin, ParamValue, ParamView};

use crate::error::ScheduleError;

/// The rate schedule in force for one `(year, filing status)`: the upper edge of
/// every bracket but the last, and one rate per bracket.
///
/// Bracket `b` taxes the income "over" `tops[b - 1]` (zero for the first
/// bracket) "but not over" `tops[b]` (no upper edge for the last bracket), which
/// is how the statute and the revenue procedures word every row of the tables:
/// an income exactly on an edge lies wholly in the brackets below it.
///
/// A table is only ever built from a [`ParamView`]; the crate holds no amounts
/// of its own (ADR-022).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BracketTable {
    year: Year,
    status: FilingStatus,
    tops: Vec<Cents>,
    edge_names: Vec<String>,
    rates: Vec<Ratio>,
    origin: Origin,
}

impl BracketTable {
    /// The schedule **in force** for `year`: the published row where the vintage
    /// publishes that year, otherwise the row projected from the statutory base
    /// year under the table's projection rule ([`ParamView::value`]).
    ///
    /// # Errors
    ///
    /// [`ScheduleError::Params`] when the vintage has no row, no projection or no
    /// rate ladder for the request; [`ScheduleError::MalformedTable`] when what it
    /// has is not a rate schedule.
    pub fn in_force(
        params: &ParamView<'_>,
        year: Year,
        status: FilingStatus,
    ) -> Result<Self, ScheduleError> {
        let value = params.value(ORDINARY_BRACKETS, year, status.wire())?;
        Self::build(params, year, status, value)
    }

    /// The schedule **projected** for `year` from the statutory base-year amounts
    /// and the archived index series, whether or not the year is published
    /// ([`ParamView::project`]). Published amounts are never an input. This is
    /// what lets a test, and later the Assumptions Registry, compare the
    /// statutory pipeline with the publication.
    ///
    /// # Errors
    ///
    /// As [`BracketTable::in_force`].
    pub fn projected(
        params: &ParamView<'_>,
        year: Year,
        status: FilingStatus,
    ) -> Result<Self, ScheduleError> {
        let value = params.project(ORDINARY_BRACKETS, year, status.wire())?;
        Self::build(params, year, status, value)
    }

    // `&ParamView` as in the design's signatures: the override layer will make
    // the view larger than a pointer (see `pfp-params`).
    #[allow(clippy::trivially_copy_pass_by_ref)]
    fn build(
        params: &ParamView<'_>,
        year: Year,
        status: FilingStatus,
        value: ParamValue,
    ) -> Result<Self, ScheduleError> {
        let rates = params.rates(ORDINARY_BRACKETS, year)?.to_vec();
        let edge_names = params
            .vintage()
            .table(ORDINARY_BRACKETS)
            .map(|t| t.components().to_vec())
            .unwrap_or_default();
        Self::from_parts(year, status, value.amounts, edge_names, rates, value.origin)
    }

    /// Validates the parts. Private: a table comes from parameters, never from a
    /// caller's numbers.
    fn from_parts(
        year: Year,
        status: FilingStatus,
        tops: Vec<Cents>,
        edge_names: Vec<String>,
        rates: Vec<Ratio>,
        origin: Origin,
    ) -> Result<Self, ScheduleError> {
        let malformed = |reason| Err(ScheduleError::MalformedTable { reason });
        if rates.len() != tops.len() + 1 {
            return malformed("a schedule has exactly one more rate than it has bracket edges");
        }
        if edge_names.len() != tops.len() {
            return malformed("every bracket edge needs a name");
        }
        if tops.first().is_some_and(|first| *first < Cents::ZERO) {
            return malformed("a bracket edge is negative");
        }
        if tops.windows(2).any(|pair| pair[0] >= pair[1]) {
            return malformed("bracket edges are not strictly increasing");
        }
        if rates
            .iter()
            .any(|r| r.den <= 0 || r.num < 0 || r.num > r.den)
        {
            return malformed("a rate lies outside [0, 1]");
        }
        Ok(Self {
            year,
            status,
            tops,
            edge_names,
            rates,
            origin,
        })
    }

    /// The tax year.
    #[must_use]
    pub const fn year(&self) -> Year {
        self.year
    }

    /// The filing status.
    #[must_use]
    pub const fn status(&self) -> FilingStatus {
        self.status
    }

    /// The upper edge of every bracket but the last, ascending.
    #[must_use]
    pub fn tops(&self) -> &[Cents] {
        &self.tops
    }

    /// The parameter table's name for each edge, parallel to [`Self::tops`].
    #[must_use]
    pub fn edge_names(&self) -> &[String] {
        &self.edge_names
    }

    /// One rate per bracket, in bracket order, exactly as the vintage writes them.
    #[must_use]
    pub fn rates(&self) -> &[Ratio] {
        &self.rates
    }

    /// Where the edges came from: published, or projected with its arithmetic.
    #[must_use]
    pub const fn origin(&self) -> &Origin {
        &self.origin
    }

    /// The highest rate of the ladder.
    #[must_use]
    pub fn top_rate(&self) -> Ratio {
        let mut top = Ratio::ZERO;
        for r in &self.rates {
            // Denominators are positive (validated), so the cross-product orders.
            if i128::from(r.num) * i128::from(top.den) > i128::from(top.num) * i128::from(r.den) {
                top = *r;
            }
        }
        top
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // SYNTHETIC schedules: invented edges and rates that describe no law.
    fn parts(tops: &[i64], rates: &[(i64, i64)]) -> Result<BracketTable, ScheduleError> {
        BracketTable::from_parts(
            2000,
            FilingStatus::Single,
            tops.iter().map(|c| Cents(*c)).collect(),
            (0..tops.len()).map(|i| format!("edge_{i}")).collect(),
            rates
                .iter()
                .map(|(n, d)| Ratio { num: *n, den: *d })
                .collect(),
            Origin::Published,
        )
    }

    #[test]
    fn a_well_formed_schedule_is_accepted() {
        let t = parts(&[1_000, 5_000], &[(1, 10), (3, 10), (2, 10)]).unwrap();
        assert_eq!(t.tops(), [Cents(1_000), Cents(5_000)]);
        assert_eq!(t.top_rate(), Ratio { num: 3, den: 10 });
        assert_eq!(t.year(), 2000);
        assert_eq!(t.status(), FilingStatus::Single);
        assert_eq!(t.edge_names().len(), 2);
        assert_eq!(*t.origin(), Origin::Published);
        // A flat tax is a schedule with no edges.
        assert!(parts(&[], &[(1, 10)]).is_ok());
    }

    #[test]
    fn malformed_schedules_are_refused() {
        for (tops, rates) in [
            (&[1_000, 5_000][..], &[(1, 10), (2, 10)][..]),
            (&[5_000, 1_000], &[(1, 10), (2, 10), (3, 10)]),
            (&[1_000, 1_000], &[(1, 10), (2, 10), (3, 10)]),
            (&[-1, 1_000], &[(1, 10), (2, 10), (3, 10)]),
            (&[1_000], &[(1, 10), (11, 10)]),
            (&[1_000], &[(-1, 10), (1, 10)]),
            (&[1_000], &[(1, 0), (1, 10)]),
            (&[], &[]),
        ] {
            assert!(
                matches!(
                    parts(tops, rates),
                    Err(ScheduleError::MalformedTable { .. })
                ),
                "{tops:?} {rates:?}"
            );
        }
    }
}
