//! Federal tax worksheets as named lines (`ENGINE-SPEC.md` §3).
//!
//! At M0 this crate carries only the rate-schedule function
//! [`schedule_tax`]`(year, status, taxable_income) -> Lines`, the canonical
//! computation (D9). The full `federal(year, status, inputs, params, trace)`
//! signature, `payroll` and `marginal` freeze with seam S3 at M1; later worksheets
//! are additions, never signature changes.
//!
//! # The rate schedule
//!
//! `schedule(x) = sum_b rate_b x max(0, min(x, top_b) - bottom_b)`, each product
//! via `Cents::mul_ratio`, summed in cents, rounded once to the whole dollar under
//! `irs.whole_dollar` (half-up). The bracket edges and the rate ladder are read
//! through `pfp_params::ParamView`; [`BracketTable`] is that read, and
//! [`BracketTable::tax`] is the computation. The result is a `pfp_explain::Lines`
//! worksheet whose last line, [`line_id::TAX`], is the tax; every bracket
//! contributes the income that falls in it and the tax on that income, each
//! citing the parameter cells it read.
//!
//! The parameter vintage compiled into the engine is **pending hand verification
//! and is not locked** (ADR-022, `docs/verification/m0-hand-verification.md`).
//!
//! # Purity and determinism contract (engine crate)
//!
//! This crate is part of the engine (`ARCHITECTURE.md` §3, §4.3). It performs no
//! I/O and reads no clock, environment, entropy or global mutable state (D1); the
//! same inputs produce bit-identical outputs on every architecture and for any
//! thread count (D2); engine output never depends on hash-map iteration order
//! (D8). The contract is enforced mechanically, not by review: CI compiles this
//! crate for `wasm32-unknown-unknown`, `unsafe_code` is forbidden below, and the
//! clippy disallow lists in `clippy.toml` reject the clock, the file system, the
//! environment and `HashMap`/`HashSet`.
//!
//! No dollar amount is written as a literal in this crate outside tests
//! (ADR-022, `cargo xtask lint-dollars`): statutory constants come from `params/`.

#![forbid(unsafe_code)]

mod error;
mod schedule;
mod table;

pub use error::ScheduleError;
pub use schedule::{
    line_id, schedule_tax, schedule_tax_with, CENT_HALF_EVEN, CENT_HALF_EVEN_ID, WHOLE_DOLLAR,
    WHOLE_DOLLAR_ID,
};
pub use table::BracketTable;
