//! Exact money arithmetic: seam S2 (`PLAN.md` §3, `ARCHITECTURE.md` §4.3 D3, D5).
//!
//! Money is `Cents(i64)`; statutory rates are exact `Ratio { num, den }` parsed from
//! decimal strings or fractions; `Cents::mul_ratio` computes in `i128` and rounds
//! **once** with a named `RoundingRule { increment, direction, basis }` stored as data
//! beside the parameter. `Cents * Cents` does not compile. `f64` enters money in
//! exactly two named places, `Cents::grow` and `Cents::from_f64_half_even`; M0
//! ships the first, and the second arrives with its first caller (the solvers and
//! spending rules of `SIMULATION-SPEC.md` §2.6).
//!
//! # Overflow, and the two ties the design leaves open
//!
//! Nothing in this crate wraps, saturates or silently truncates. Every operation
//! has a `checked_*` form returning [`MoneyError`]; the operator impls and the
//! design-named infallible forms ([`Cents::mul_ratio`], [`Cents::grow`],
//! [`RoundingRule::round`]) panic **in every build profile** on the same
//! conditions, because a wrapped balance is a wrong answer that looks like a right
//! one.
//!
//! Two tie rules are not specified anywhere in the design and are therefore an
//! explicit error, [`MoneyError::UnspecifiedTie`], rather than a guess
//! (`fixtures/pending/rounding/table.json`, `openQuestions`): `Nearest` at an exact
//! tie, and `HalfUp` at an exact tie of a **negative** amount. Turning an error into
//! a value later is compatible; changing a value is not.
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

mod cents;
mod error;
mod ratio;
mod rounding;

pub use cents::Cents;
pub use error::MoneyError;
pub use ratio::{Ratio, RatioError};
pub use rounding::{RoundingBasis, RoundingDirection, RoundingRule};
