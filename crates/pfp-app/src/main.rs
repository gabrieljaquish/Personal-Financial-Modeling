//! `pfp`: the launcher for the local web application (`ARCHITECTURE.md` §3).
//!
//! This file is the process boundary and nothing else; the behaviour is
//! [`pfp_app::run`]. It is also the only place a real platform implementation is
//! constructed. On macOS that is the browser opener in `macos_opener.rs`
//! (`LSOpenCFURLRef`, in-process); trust settings and native alerts stay
//! unsupported, so no build reads or changes a trust setting or shows an alert.
//! On any other system every platform service is unsupported.

// Not an engine crate: the clock, the file system and process I/O are legitimate here.
// `unsafe` is denied workspace-wide; the one module that calls C allows it locally.
#![deny(unsafe_code)]
#![allow(
    clippy::disallowed_types,
    clippy::disallowed_methods,
    clippy::disallowed_macros
)]

#[cfg(target_os = "macos")]
mod macos_opener;

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
    #[cfg(target_os = "macos")]
    let platform = macos_opener::platform();
    #[cfg(not(target_os = "macos"))]
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
