//! The parameter vintages compiled into the engine.
//!
//! `include_str!` is resolved by the compiler, so the crate still performs no I/O
//! and builds for `wasm32-unknown-unknown`. The application embeds the same files
//! with a digest manifest (`ARCHITECTURE.md` §9); this module is what lets an
//! engine function with no `params` argument, such as M0's `schedule_tax`, read a
//! vintage at all.
//!
//! **`federal-2026` is pending hand verification and is not locked.** Every value
//! in it was transcribed by an AI-assisted session (ADR-022); `params/VINTAGES.lock`
//! carries no entry for it, [`Vintage::id`] is `None` (its
//! [`Vintage::content_id`] is computable, and claims nothing), and
//! [`Vintage::is_verified`] is `false` until a human has read each value back.

use crate::error::ParamError;
use crate::view::Vintage;

/// Name of the first vintage.
pub const FEDERAL_2026: &str = "federal-2026";

/// Table id of the ordinary income-tax rate schedule.
pub const ORDINARY_BRACKETS: &str = "irs.ordinary_brackets";

/// Table id of the basic standard deduction.
pub const STD_DEDUCTION: &str = "irs.std_deduction";

const ORDINARY_BRACKETS_TOML: &str =
    include_str!("../../../params/vintages/federal-2026/ordinary_brackets.toml");
const STD_DEDUCTION_TOML: &str =
    include_str!("../../../params/vintages/federal-2026/std_deduction.toml");
const CPI_CHAINED_TOML: &str =
    include_str!("../../../params/index-series/cpi-chained-suur0000sa0.toml");

/// The table documents of `federal-2026`, as embedded.
#[must_use]
pub const fn federal_2026_tables() -> [&'static str; 2] {
    [ORDINARY_BRACKETS_TOML, STD_DEDUCTION_TOML]
}

/// The archived index-series documents `federal-2026` reads, as embedded. Which
/// table reads which series, and which calendar year of it, is stated by the
/// documents themselves (`projection.index_series`, `projection.lag_years`, and
/// the series' own `index_series` name), not here.
#[must_use]
pub const fn federal_2026_series() -> [&'static str; 1] {
    [CPI_CHAINED_TOML]
}

/// Parses the embedded `federal-2026` vintage.
///
/// # Errors
///
/// A [`ParamError`] if an embedded file does not validate; the crate's tests
/// assert that it does.
pub fn federal_2026() -> Result<Vintage, ParamError> {
    Vintage::parse(FEDERAL_2026, &federal_2026_tables(), &federal_2026_series())
}
