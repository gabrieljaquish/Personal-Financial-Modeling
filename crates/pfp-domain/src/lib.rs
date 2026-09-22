//! Cross-cutting domain enums and aliases, defined exactly once.
//!
//! `FilingStatus`, `Owner`, `PersonId`, `AssetClass`, `TaxType`, `Year` and `Seed`
//! live here and are re-exported by every crate that needs them, never redefined
//! or converted at a boundary (`ARCHITECTURE.md` §2 constraints table, §3). M0
//! needs two of them, [`FilingStatus`] and [`Year`], because
//! `schedule_tax(year, status, taxable_income)` and the parameter tables'
//! `filingStatus` breakdown both read them; the rest arrive with the milestone
//! that first uses them. The crate carries no dependency beyond `serde`.
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

mod filing_status;
mod year;

pub use filing_status::{FilingStatus, UnknownFilingStatus};
pub use year::{year_in_domain, Year, YEAR_MAX, YEAR_MIN};
