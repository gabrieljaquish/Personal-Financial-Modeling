//! Why the rate schedule has no answer.

use core::fmt;

use pfp_explain::{IdError, TraceError};
use pfp_money::{Cents, MoneyError};
use pfp_params::{ParamError, ProjectionError};

/// Why [`schedule_tax`](crate::schedule_tax) has no answer. Every variant is a
/// refusal: the function never substitutes a value for one it could not read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ScheduleError {
    /// Taxable income below zero. The return computes
    /// `TI = max(0, AGI - deduction)` (`ENGINE-SPEC.md` §3.2 line 4), so a
    /// negative amount is a caller's defect, not a case the schedule defines.
    NegativeTaxableIncome(Cents),
    /// The embedded vintage did not validate.
    Vintage(ParamError),
    /// The vintage has no bracket table or rate ladder for the request.
    Params(ProjectionError),
    /// The bracket row and the rate ladder do not form a rate schedule.
    MalformedTable {
        /// What is wrong with it.
        reason: &'static str,
    },
    /// The money arithmetic failed.
    Money(MoneyError),
    /// A line id or parameter reference could not be built.
    Id(IdError),
    /// A line could not join the trace.
    Trace(TraceError),
}

impl fmt::Display for ScheduleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NegativeTaxableIncome(ti) => write!(f, "taxable income {ti} is negative"),
            Self::Vintage(e) => write!(f, "the embedded parameter vintage is invalid: {e}"),
            Self::Params(e) => write!(f, "no rate schedule: {e}"),
            Self::MalformedTable { reason } => write!(f, "malformed rate schedule: {reason}"),
            Self::Money(e) => write!(f, "rate schedule arithmetic: {e}"),
            Self::Id(e) => write!(f, "rate schedule trace id: {e}"),
            Self::Trace(e) => write!(f, "rate schedule trace: {e}"),
        }
    }
}

impl std::error::Error for ScheduleError {}

impl From<ParamError> for ScheduleError {
    fn from(e: ParamError) -> Self {
        Self::Vintage(e)
    }
}

impl From<ProjectionError> for ScheduleError {
    fn from(e: ProjectionError) -> Self {
        Self::Params(e)
    }
}

impl From<MoneyError> for ScheduleError {
    fn from(e: MoneyError) -> Self {
        Self::Money(e)
    }
}

impl From<IdError> for ScheduleError {
    fn from(e: IdError) -> Self {
        Self::Id(e)
    }
}

impl From<TraceError> for ScheduleError {
    fn from(e: TraceError) -> Self {
        Self::Trace(e)
    }
}
