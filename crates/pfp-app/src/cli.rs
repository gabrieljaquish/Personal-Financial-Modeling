//! The command line, parsed by hand (no dependency for five flags).
//!
//! ```text
//! pfp                      = pfp serve
//! pfp serve [--port N] [--no-open] [--no-trust] [--install-trust] [--state-dir DIR] [--verbose]
//! pfp trust remove [--install-trust] [--state-dir DIR]
//! pfp openapi              print the OpenAPI document and start nothing
//! pfp --version | --help
//! ```
//!
//! **No flag and no environment variable can carry a token or a passphrase**
//! (`SECURITY.md` §7.1): a command line is visible to every same-user process and
//! lands in shell history. There is deliberately no way to ask for the launch URL.

use std::path::PathBuf;

/// The text of `pfp --help`.
pub const USAGE: &str = "\
usage: pfp [serve] [--port N] [--no-open] [--no-trust] [--install-trust] [--state-dir DIR] [--verbose]
       pfp trust remove [--install-trust] [--state-dir DIR]
       pfp openapi
       pfp --version | --help

serve (the default) starts the local application on https://127.0.0.1:<port>.
  --port N         prefer port N instead of the usual one. Whichever is preferred, if
                   it is occupied the run warns first and then uses an OS-assigned
                   port. --port 0 asks for an OS-assigned port from the start.
  --no-open        do not open a browser.
  --no-trust       do not look at or change any trust setting; the certificate
                   fingerprint is printed for manual comparison. Wins over --install-trust.
  --install-trust  allow this run to ask the system to trust the local certificate
                   authority (or, with `trust remove`, to remove it). Nothing is ever
                   installed or removed without it.
  --state-dir DIR  where the lock file and the local certificate live.
  --verbose        print every server event to stderr: event codes, request
                   routes and statuses only - no secret and no request content.
";

/// The application's usual port: the stable preferred port of `SECURITY.md` §6.3
/// and ADR-015, tried at **every** launch so that a browser and a password manager
/// recognise `https://127.0.0.1:<this>` as this application — and so that finding
/// it occupied is the S-29 warning rather than a silent move (threat T3).
///
/// The number is this step's choice, recorded in `README.md`: in the registered
/// (non-privileged) range, and below 49152 where the macOS ephemeral range starts,
/// so the operating system never hands it to another program by itself. Changing
/// it changes the origin every saved password is filed under.
pub const PREFERRED_PORT: u16 = 47_443;

/// Which port `pfp serve` asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PortChoice {
    /// No `--port`: the built-in [`PREFERRED_PORT`], with the loud fallback.
    #[default]
    Usual,
    /// `--port N` with `N > 0`: prefer `N` instead, with the same loud fallback.
    Prefer(u16),
    /// `--port 0`: OS-assigned from the start; nothing is preferred, so nothing can
    /// be found occupied. What every test and scripted run uses.
    OsAssigned,
}

impl PortChoice {
    /// The port whose occupation must be warned about, if any.
    #[must_use]
    pub fn preferred(self) -> Option<u16> {
        match self {
            Self::Usual => Some(PREFERRED_PORT),
            Self::Prefer(port) => Some(port),
            Self::OsAssigned => None,
        }
    }
}

/// Options of `pfp serve`.
// Each bool is one independent command-line switch; a state machine would only
// obscure the one-to-one mapping from flag to field.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ServeArgs {
    /// `--port`.
    pub port: PortChoice,
    /// `--no-open`.
    pub no_open: bool,
    /// `--no-trust`.
    pub no_trust: bool,
    /// `--install-trust`: the explicit opt-in without which nothing is installed.
    pub install_trust: bool,
    /// `--state-dir DIR`.
    pub state_dir: Option<PathBuf>,
    /// `--verbose`: echo every server event (codes, routes and statuses only).
    pub verbose: bool,
}

/// Options of `pfp trust remove`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TrustRemoveArgs {
    /// `--install-trust`: the same opt-in governs removal.
    pub install_trust: bool,
    /// `--state-dir DIR`.
    pub state_dir: Option<PathBuf>,
}

/// What the user asked for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// `pfp` or `pfp serve`.
    Serve(ServeArgs),
    /// `pfp trust remove`.
    TrustRemove(TrustRemoveArgs),
    /// `pfp openapi`.
    OpenApi,
    /// `pfp --version`.
    Version,
    /// `pfp --help`.
    Help,
}

/// A usage error. The message names the offending flag, never its value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsageError(pub &'static str);

fn value<'a>(
    args: &mut impl Iterator<Item = &'a String>,
    missing: &'static str,
) -> Result<&'a str, UsageError> {
    args.next().map(String::as_str).ok_or(UsageError(missing))
}

/// Parses the arguments after the program name.
///
/// # Errors
/// [`UsageError`] for anything that is not the grammar above.
pub fn parse(args: &[String]) -> Result<Command, UsageError> {
    let mut rest = args.iter().peekable();
    match rest.peek().map(|s| s.as_str()) {
        Some("--version" | "-V") => return only(args, Command::Version),
        Some("--help" | "-h" | "help") => return only(args, Command::Help),
        Some("openapi") => return only(args, Command::OpenApi),
        Some("trust") => {
            rest.next();
            if rest.next().map(String::as_str) != Some("remove") {
                return Err(UsageError("`pfp trust` takes one subcommand: remove"));
            }
            let mut out = TrustRemoveArgs::default();
            while let Some(flag) = rest.next() {
                match flag.as_str() {
                    "--install-trust" => out.install_trust = true,
                    "--state-dir" => {
                        out.state_dir = Some(PathBuf::from(value(
                            &mut rest,
                            "--state-dir needs a directory",
                        )?));
                    }
                    _ => return Err(UsageError("unknown argument to `pfp trust remove`")),
                }
            }
            return Ok(Command::TrustRemove(out));
        }
        Some("serve") => {
            rest.next();
        }
        _ => {}
    }

    let mut out = ServeArgs::default();
    while let Some(flag) = rest.next() {
        match flag.as_str() {
            "--no-open" => out.no_open = true,
            "--no-trust" => out.no_trust = true,
            "--install-trust" => out.install_trust = true,
            "--verbose" => out.verbose = true,
            "--port" => {
                let port: u16 = value(&mut rest, "--port needs a number")?
                    .parse()
                    .map_err(|_| UsageError("--port needs a number from 0 to 65535"))?;
                out.port = if port == 0 {
                    PortChoice::OsAssigned
                } else {
                    PortChoice::Prefer(port)
                };
            }
            "--state-dir" => {
                out.state_dir = Some(PathBuf::from(value(
                    &mut rest,
                    "--state-dir needs a directory",
                )?));
            }
            _ => return Err(UsageError("unknown argument to `pfp serve`")),
        }
    }
    Ok(Command::Serve(out))
}

fn only(args: &[String], command: Command) -> Result<Command, UsageError> {
    if args.len() == 1 {
        Ok(command)
    } else {
        Err(UsageError("this command takes no further arguments"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(args: &[&str]) -> Result<Command, UsageError> {
        parse(&args.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>())
    }

    #[test]
    fn bare_invocation_is_serve_with_defaults() {
        assert_eq!(p(&[]), Ok(Command::Serve(ServeArgs::default())));
        assert_eq!(p(&["serve"]), Ok(Command::Serve(ServeArgs::default())));
    }

    #[test]
    fn serve_flags() {
        let Ok(Command::Serve(args)) = p(&[
            "serve",
            "--port",
            "8443",
            "--no-open",
            "--no-trust",
            "--install-trust",
            "--state-dir",
            "/x y",
            "--verbose",
        ]) else {
            panic!("serve");
        };
        assert_eq!(args.port, PortChoice::Prefer(8443));
        assert!(args.no_open && args.no_trust && args.install_trust && args.verbose);
        assert!(!ServeArgs::default().verbose, "quiet unless asked");
        assert_eq!(args.state_dir, Some(PathBuf::from("/x y")));
        // Port 0 means "no preference": OS-assigned, and no probe to fail.
        let Ok(Command::Serve(zero)) = p(&["--port", "0"]) else {
            panic!("serve");
        };
        assert_eq!(zero.port, PortChoice::OsAssigned);
        assert_eq!(zero.port.preferred(), None);
    }

    /// S-29 / `SECURITY.md` §6.3: the probe runs at every launch, so a run with no
    /// `--port` must prefer the built-in port rather than go straight to an
    /// OS-assigned one (which can never be "occupied", so could never warn).
    #[test]
    fn a_bare_run_prefers_the_usual_port() {
        let Ok(Command::Serve(bare)) = p(&[]) else {
            panic!("serve");
        };
        assert_eq!(bare.port, PortChoice::Usual);
        assert_eq!(bare.port.preferred(), Some(PREFERRED_PORT));
        // Registered range, below the macOS ephemeral range.
        assert!((1024..49152).contains(&PREFERRED_PORT));
        assert_eq!(PortChoice::Prefer(8443).preferred(), Some(8443));
    }

    #[test]
    fn other_commands() {
        assert_eq!(p(&["--version"]), Ok(Command::Version));
        assert_eq!(p(&["--help"]), Ok(Command::Help));
        assert_eq!(p(&["openapi"]), Ok(Command::OpenApi));
        assert_eq!(
            p(&["trust", "remove", "--install-trust"]),
            Ok(Command::TrustRemove(TrustRemoveArgs {
                install_trust: true,
                state_dir: None
            }))
        );
    }

    #[test]
    fn refusals() {
        for bad in [
            &["--port"][..],
            &["--port", "70000"],
            &["--port", "x"],
            &["--state-dir"],
            &["--bind", "0.0.0.0"],
            // No credential ever travels in argv (SECURITY.md §3.7, §7.1). These
            // spellings are named in xtask/src/lint_server.rs as the only
            // credential-shaped literals `pfp-app` may contain, and only here.
            &["--token", "x"],
            &["--launch-token", "x"],
            &["--passphrase", "x"],
            &["--password", "x"],
            &["--secret", "x"],
            &["--key", "x"],
            &["--print-launch-url"],
            &["serve", "extra"],
            &["trust"],
            &["trust", "install"],
            &["trust", "remove", "--no-open"],
            &["--version", "x"],
            &["openapi", "--out", "f"],
        ] {
            assert!(p(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn a_usage_error_never_echoes_the_argument() {
        let secret = "zz-not-a-flag-zz";
        let UsageError(message) = p(&[secret]).unwrap_err();
        assert!(!message.contains(secret));
    }
}
