//! Parameter tables, vintages, projection and provenance: seam S1 (`PLAN.md` §3,
//! `ARCHITECTURE.md` §6, `DECISIONS.md` ADR-010 and C3).
//!
//! Year-keyed tables with per-breakdown values, a `projection` block carrying the
//! rule, the index series, the statutory `base_year` and `base_values`, and a
//! `RoundingRule`; `[[source]]` provenance with archived checksummed primary text;
//! immutable vintages identified by name plus content hash and locked by
//! `params/VINTAGES.lock`; a visible override layer. Uprating is computed from the
//! base year in one step (`basis: IncreaseOverBase`), never chained year over year.
//!
//! # What is here
//!
//! - [`ParamTable`] and [`IndexSeries`]: typed, validated tables parsed from TOML
//!   **text** (the crate never opens a file); the shape read, including each
//!   extension the first vintage needed, is tabulated in the `table` module docs.
//! - [`Vintage`] and [`ParamView`]: published-value lookup by `(year, breakdown
//!   key)`, the projection of any year from the statutory base, and provenance.
//! - [`shipped`]: the first vintage, embedded at compile time. It is **pending
//!   hand verification and not locked**; [`Vintage::is_verified`] says so.
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
mod provenance;
mod raw;
mod series;
pub mod shipped;
mod table;
mod view;

pub use error::{ParamError, ProjectionError};
pub use provenance::{DateYmd, Source, VerificationStatus};
pub use series::IndexSeries;
pub use table::{
    BaseYear, Derivation, Increment, ParamTable, Projection, ProjectionRule, RoundingSpec,
};
pub use view::{Origin, ParamValue, ParamView, ProjectionStep, Vintage, VintageId};
