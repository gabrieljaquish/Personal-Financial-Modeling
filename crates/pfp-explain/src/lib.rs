//! The `Line` trace node and the `Explanation` tree (`ARCHITECTURE.md` §4.2).
//!
//! One `Line { id, label, value, inputs, params, rounding }` structure serves the
//! fixture intermediate, the UI audit trail and the effective-marginal-rate
//! machinery. `Explanation`, `Reason` and `Bound` carry the reconciliation
//! invariant (agreement, coverage, reachability) that is the decision code's only
//! test oracle. The `Line` node lands in the next M0 change; the explanation tree
//! and the flip-value bisection helper freeze with seam S7 at M2.
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
