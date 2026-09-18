//! Parameter tables, vintages, projection and provenance: seam S1 (`PLAN.md` §3,
//! `ARCHITECTURE.md` §6, `DECISIONS.md` ADR-010 and C3).
//!
//! Year-keyed tables with per-breakdown values, a `projection` block carrying the
//! rule, the index series, the statutory `base_year` and `base_values`, and a
//! `RoundingRule`; `[[source]]` provenance with archived checksummed primary text;
//! immutable vintages identified by name plus content hash and locked by
//! `params/VINTAGES.lock`; a visible override layer. Uprating is computed from the
//! base year in one step (`basis: IncreaseOverBase`), never chained year over year.
//! The loader and the first vintage land in the next M0 change.
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
