//! `pfp`: the launcher for the local web application (`ARCHITECTURE.md` §3).
//!
//! This file is the process boundary and nothing else; the behaviour is
//! [`pfp_app::run`]. It is also the only place a real platform implementation
//! could ever be constructed — and at M0 there is none: every build runs with
//! `Platform::unsupported()`, which reads and changes no trust setting, shows no
//! alert and opens no browser.

// Not an engine crate: the clock, the file system and process I/O are legitimate here.
#![forbid(unsafe_code)]
#![allow(
    clippy::disallowed_types,
    clippy::disallowed_methods,
    clippy::disallowed_macros
)]

use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    // Lossy on purpose: `std::env::args` panics on a non-UTF-8 argument.
    let args: Vec<String> = std::env::args_os()
        .skip(1)
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();
    // HOME locates the default state directory; nothing secret is ever read from
    // the environment.
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let platform = pfp_app::platform::Platform::unsupported();
    let code = pfp_app::run(
        &args,
        &platform,
        home.as_deref(),
        &mut std::io::stdout(),
        &mut std::io::stderr(),
    );
    ExitCode::from(code)
}
