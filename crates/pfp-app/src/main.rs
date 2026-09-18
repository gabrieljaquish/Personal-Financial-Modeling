//! `pfp`: the launcher for the local web application (`ARCHITECTURE.md` §3).
//!
//! At this step of M0 the binary only reports its version and exits. The
//! single-instance lock, TLS bootstrap, browser open and the `serve` / `vault` /
//! `trust` / `export` subcommands arrive with the server (next M0 changes).

// Not an engine crate: the clock, the file system and process I/O are legitimate here.
#![forbid(unsafe_code)]
#![allow(
    clippy::disallowed_types,
    clippy::disallowed_methods,
    clippy::disallowed_macros
)]

fn main() {
    println!("pfp {}", env!("CARGO_PKG_VERSION"));
}
