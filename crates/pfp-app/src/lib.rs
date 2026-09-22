//! `pfp`, the launcher of the local web application (`ARCHITECTURE.md` §3, §5):
//! command line, process hardening, single-instance lock, the local certificate's
//! file store, the platform seam and the launch sequence. The server itself is
//! `pfp-server`; this crate owns the process around it.
//!
//! * [`cli`] — the hand-rolled argument parser. No flag can carry a token;
//! * [`harden`] — `RLIMIT_CORE = 0` and the silent panic hook;
//! * [`state`] and [`lock`] — the `0700` state directory and the advisory lock;
//! * [`leaf_store`] — the `0600` file store for the leaf key;
//! * [`platform`] — trust settings, alerts and the browser opener, as traits. **No
//!   real implementation exists at M0**: every build runs with
//!   [`platform::Platform::unsupported`];
//! * [`launch`] — the sequence that ties them together.

// Not an engine crate: the clock, the file system and process I/O are legitimate here.
#![forbid(unsafe_code)]
#![allow(
    clippy::disallowed_types,
    clippy::disallowed_methods,
    clippy::disallowed_macros
)]

pub mod cli;
pub mod harden;
pub mod launch;
pub mod leaf_store;
pub mod lock;
pub mod platform;
pub mod state;

use std::io::Write;
use std::path::Path;

use cli::Command;
use launch::{Shutdown, EXIT_FAILED, EXIT_OK, EXIT_USAGE};
use platform::Platform;

/// Everything `main` does, with the process boundary injected: arguments (without
/// the program name), the platform, the home directory and the two streams.
/// Returns the exit code.
pub fn run(
    args: &[String],
    platform: &Platform,
    home: Option<&Path>,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    // First, before any allocation of interest (`SECURITY.md` §5).
    let core = harden::disable_core_dumps();
    harden::install_panic_hook();

    let command = match cli::parse(args) {
        Ok(command) => command,
        Err(cli::UsageError(message)) => {
            let _ = writeln!(err, "pfp: {message}\n\n{}", cli::USAGE);
            return EXIT_USAGE;
        }
    };
    match command {
        Command::Version => {
            let _ = writeln!(out, "pfp {}", env!("CARGO_PKG_VERSION"));
            EXIT_OK
        }
        Command::Help => {
            let _ = write!(out, "{}", cli::USAGE);
            EXIT_OK
        }
        Command::OpenApi => {
            let _ = out.write_all(pfp_server::openapi_json().as_bytes());
            EXIT_OK
        }
        Command::TrustRemove(args) => launch::trust_remove(&args, platform, home, out, err),
        Command::Serve(args) => {
            // Refuse to serve a process that could still dump core.
            if core.is_err() {
                let _ = writeln!(err, "pfp: core dumps could not be disabled");
                return EXIT_FAILED;
            }
            launch::serve(&args, platform, home, out, err, Shutdown::Signals)
        }
    }
}
