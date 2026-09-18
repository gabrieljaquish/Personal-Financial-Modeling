//! The local web server: TLS-only listener on loopback, session establishment,
//! request admission, the JSON API under `/api/v1` and the embedded web assets
//! (`ARCHITECTURE.md` §5, `SECURITY.md` §6–§7).
//!
//! This crate is the only workspace member that opens sockets. Nothing here is
//! part of the engine: the engine crates are pure and this crate drives them.
//! The TLS bootstrap, canonical origin and session hardening land in the next
//! M0 changes; this skeleton exists so the workspace graph is complete.

// Not an engine crate: the clock, sockets and hash maps are legitimate here.
#![forbid(unsafe_code)]
#![allow(
    clippy::disallowed_types,
    clippy::disallowed_methods,
    clippy::disallowed_macros
)]
