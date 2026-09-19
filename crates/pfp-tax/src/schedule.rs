//! The rate schedule: the canonical tax computation (`ARCHITECTURE.md` D9,
//! `ENGINE-SPEC.md` §3.2).

use pfp_domain::{FilingStatus, Year};
use pfp_explain::{Line, Lines, ParamRef, RuleId};
use pfp_money::{Cents, Ratio, RoundingBasis, RoundingDirection, RoundingRule};
use pfp_params::shipped::{self, ORDINARY_BRACKETS};
use pfp_params::ParamView;

use crate::error::ScheduleError;
use crate::table::BracketTable;

/// Unit conversions, not amounts: the two rounding rules below are stated in
/// cents because `RoundingRule.increment` is `Cents`.
const CENTS_PER_DOLLAR: i64 = 100;
const CENTS_PER_CENT: i64 = 1;

/// Id of [`WHOLE_DOLLAR`].
pub const WHOLE_DOLLAR_ID: RuleId = RuleId::from_static("irs.whole_dollar");

/// `irs.whole_dollar`: the schedule's sum is rounded **once**, half-up, to the
/// whole dollar (`ENGINE-SPEC.md` §3.2). Tax is never negative, so the half-up
/// tie of a negative amount, which the design leaves open, cannot arise here.
pub const WHOLE_DOLLAR: RoundingRule = RoundingRule {
    increment: Cents(CENTS_PER_DOLLAR),
    direction: RoundingDirection::HalfUp,
    basis: RoundingBasis::Amount,
};

/// Id of [`CENT_HALF_EVEN`].
pub const CENT_HALF_EVEN_ID: RuleId = RuleId::from_static("money.cent_half_even");

/// `money.cent_half_even`: the rule each `rate x amount` product is taken under,
/// so that the products are "summed in cents" (`ENGINE-SPEC.md` §3.2). It is the
/// cent convention of `ARCHITECTURE.md` D5. It changes nothing unless taxable
/// income carries cents that the rate does not divide: for a whole-dollar income
/// and a whole-percent rate every product is exact.
pub const CENT_HALF_EVEN: RoundingRule = RoundingRule {
    increment: Cents(CENTS_PER_CENT),
    direction: RoundingDirection::HalfEven,
    basis: RoundingBasis::Amount,
};

/// The stable line ids of the `sched.*` worksheet.
pub mod line_id {
    use pfp_explain::{IdError, LineId};

    /// Taxable income, the worksheet's one input.
    pub const TAXABLE_INCOME: LineId = LineId::from_static("sched.ti");
    /// The sum of the per-bracket tax, in cents, before the whole-dollar rounding.
    pub const SUM: LineId = LineId::from_static("sched.sum");
    /// The tax: [`SUM`] rounded once to the whole dollar. The worksheet's result.
    pub const TAX: LineId = LineId::from_static("sched.tax");

    /// `sched.b<n>`: the tax from bracket `n`, counted from zero.
    ///
    /// # Errors
    ///
    /// Never for a bracket index; the signature is that of the id constructor.
    pub fn bracket_tax(n: usize) -> Result<LineId, IdError> {
        LineId::new(format!("sched.b{n}"))
    }

    /// `sched.b<n>.amount`: the taxable income that falls in bracket `n`.
    ///
    /// # Errors
    ///
    /// Never for a bracket index; the signature is that of the id constructor.
    pub fn bracket_amount(n: usize) -> Result<LineId, IdError> {
        LineId::new(format!("sched.b{n}.amount"))
    }
}

/// Tax on `taxable_income` under the ordinary rate schedule in force for `year`
/// and `status`, from the parameter vintage compiled into the engine
/// (`PLAN.md` §4.1).
///
/// The result is the whole `sched.*` worksheet; the tax is its last line,
/// [`line_id::TAX`]. This form parses the embedded vintage on every call. A caller
/// that evaluates more than once holds a [`ParamView`] and calls
/// [`schedule_tax_with`].
///
/// **The embedded vintage is pending hand verification and is not locked**
/// (ADR-022); `pfp_params::Vintage::is_verified` says so.
///
/// # Errors
///
/// [`ScheduleError`]: a negative income, a year the vintage neither publishes nor
/// can project, or arithmetic outside the `i64` cent range.
pub fn schedule_tax(
    year: Year,
    status: FilingStatus,
    taxable_income: Cents,
) -> Result<Lines, ScheduleError> {
    let vintage = shipped::federal_2026()?;
    schedule_tax_with(year, status, taxable_income, &ParamView::new(&vintage))
}

/// [`schedule_tax`] against a caller's [`ParamView`].
///
/// # Errors
///
/// As [`schedule_tax`].
pub fn schedule_tax_with(
    year: Year,
    status: FilingStatus,
    taxable_income: Cents,
    params: &ParamView<'_>,
) -> Result<Lines, ScheduleError> {
    BracketTable::in_force(params, year, status)?.tax(taxable_income)
}

impl BracketTable {
    /// `schedule(x) = sum_b rate_b x max(0, min(x, top_b) - bottom_b)`: each
    /// product through `mul_ratio` (exact `i128` inside, [`CENT_HALF_EVEN`]), the
    /// products summed in cents, and the sum rounded once under [`WHOLE_DOLLAR`].
    ///
    /// Lines, in computation order: [`line_id::TAXABLE_INCOME`]; for every
    /// bracket `n` its [`line_id::bracket_amount`] (citing the edges that bound
    /// it) and [`line_id::bracket_tax`] (citing its rate); [`line_id::SUM`];
    /// [`line_id::TAX`]. A bracket the income does not reach still has both of
    /// its lines, at zero.
    ///
    /// # Errors
    ///
    /// [`ScheduleError::NegativeTaxableIncome`], or [`ScheduleError::Money`] when
    /// a result leaves the `i64` cent range.
    pub fn tax(&self, taxable_income: Cents) -> Result<Lines, ScheduleError> {
        if taxable_income < Cents::ZERO {
            return Err(ScheduleError::NegativeTaxableIncome(taxable_income));
        }
        let mut lines = Lines::new();
        lines.push(Line::new(
            line_id::TAXABLE_INCOME,
            "Taxable income",
            taxable_income,
        ))?;

        let mut bottom = Cents::ZERO;
        let mut bottom_ref: Option<ParamRef> = None;
        let mut sum = Cents::ZERO;
        let mut bracket_ids = Vec::with_capacity(self.rates().len());
        for (n, rate) in self.rates().iter().enumerate() {
            // "Over `bottom` but not over `top`": on an edge, nothing is above it.
            let top = self.tops().get(n).copied();
            let top_ref = match self.edge_names().get(n) {
                Some(name) => Some(self.cell(name.clone())?),
                None => None,
            };
            let reached = top.map_or(taxable_income, |t| taxable_income.min(t));
            let amount = reached.checked_sub(bottom)?.max(Cents::ZERO);

            let amount_id = line_id::bracket_amount(n)?;
            let mut amount_line = Line::new(
                amount_id.clone(),
                format!("Taxable income taxed at {}", percent(*rate)),
                amount,
            )
            .input(line_id::TAXABLE_INCOME);
            for edge in bottom_ref.iter().chain(top_ref.iter()) {
                amount_line = amount_line.param(edge.clone());
            }
            lines.push(amount_line)?;

            let tax = amount.checked_mul_ratio(*rate, &CENT_HALF_EVEN)?;
            let tax_id = line_id::bracket_tax(n)?;
            lines.push(
                Line::new(tax_id.clone(), format!("Tax at {}", percent(*rate)), tax)
                    .input(amount_id)
                    .param(
                        ParamRef::new(ORDINARY_BRACKETS)?
                            .with_year(self.year())
                            .with_element(format!("rates.{n}"))?,
                    )
                    .rounded_by(CENT_HALF_EVEN_ID),
            )?;

            sum = sum.checked_add(tax)?;
            bracket_ids.push(tax_id);
            if let Some(t) = top {
                bottom = t;
            }
            bottom_ref = top_ref;
        }

        lines.push(
            Line::new(line_id::SUM, "Tax before rounding to the whole dollar", sum)
                .inputs(bracket_ids),
        )?;
        lines.push(
            Line::new(line_id::TAX, "Tax", WHOLE_DOLLAR.checked_round(sum)?)
                .input(line_id::SUM)
                .rounded_by(WHOLE_DOLLAR_ID),
        )?;
        Ok(lines)
    }

    /// The parameter cell of one bracket edge.
    fn cell(&self, edge: String) -> Result<ParamRef, ScheduleError> {
        Ok(ParamRef::new(ORDINARY_BRACKETS)?
            .with_year(self.year())
            .with_breakdown_key(self.status().wire())?
            .with_element(edge)?)
    }
}

/// `10%` for a whole-percent rate, otherwise the exact fraction.
fn percent(rate: Ratio) -> String {
    let scaled = i128::from(rate.num) * i128::from(CENTS_PER_DOLLAR);
    let den = i128::from(rate.den);
    if den != 0 && scaled % den == 0 {
        format!("{}%", scaled / den)
    } else {
        rate.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_rate_prints_as_a_percentage_when_it_is_one() {
        assert_eq!(percent(Ratio { num: 10, den: 100 }), "10%");
        assert_eq!(percent(Ratio { num: 37, den: 100 }), "37%");
        assert_eq!(percent(Ratio { num: 1, den: 4 }), "25%");
        assert_eq!(percent(Ratio { num: 1, den: 3 }), "1/3");
    }

    #[test]
    fn the_named_rules_are_what_their_ids_say() {
        // SYNTHETIC amounts.
        assert_eq!(WHOLE_DOLLAR.checked_round(Cents(150)), Ok(Cents(200)));
        assert_eq!(WHOLE_DOLLAR.checked_round(Cents(149)), Ok(Cents(100)));
        assert_eq!(CENT_HALF_EVEN.checked_round(Cents(149)), Ok(Cents(149)));
        assert_eq!(WHOLE_DOLLAR_ID.as_str(), "irs.whole_dollar");
        assert_eq!(CENT_HALF_EVEN_ID.as_str(), "money.cent_half_even");
    }
}
