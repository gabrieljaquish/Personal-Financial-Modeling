//! The launch sequence (`ARCHITECTURE.md` §5, `SECURITY.md` §5–§7):
//!
//! 1. state directory (`0700`) and the single-instance lock — contention exits 3
//!    with "already running" and no second listener;
//! 2. the stored leaf, if it still validates; otherwise a fresh CA + leaf (the CA
//!    key does not outlive issuance) saved to the `0600` file store;
//! 3. trust: skipped entirely with `--no-trust`; otherwise the status is read, and
//!    an installation is attempted **only** with `--install-trust`, after the
//!    native explanation. Anything else is decline mode;
//! 4. bind `127.0.0.1` — the usual port ([`crate::cli::PREFERRED_PORT`]) or the one
//!    `--port N` names, so the occupied-port probe runs at **every** launch
//!    (`SECURITY.md` §6.3); only `--port 0` is OS-assigned from the start. If
//!    the preferred port was occupied the user is warned **before anything is
//!    opened**, and if the warning cannot be shown nothing is opened (S-29);
//! 5. the status console: one machine-readable `PFP-READY` line, then prose. **The
//!    launch token is never printed**;
//! 6. unless `--no-open`, the launch URL (token in the fragment) is handed to the
//!    in-process browser opener;
//! 7. serve until SIGINT/SIGTERM, shut down gracefully, release the lock, exit 0.
//!
//! (Hardening — `RLIMIT_CORE = 0` and the panic hook — happens before any of this,
//! in [`crate::run`].)

use std::io::Write;
use std::path::Path;
use std::sync::Arc;

use pfp_server::{
    issue, AssetManifest, BoundServer, LeafStore, Redacted, RelaunchHook, ReopenUnavailable,
    Server, ServerConfig, TlsMaterial, TrustMode,
};
use time::OffsetDateTime;
use tokio::sync::oneshot;

use crate::cli::{ServeArgs, TrustRemoveArgs};
use crate::leaf_store::FileLeafStore;
use crate::lock::{LockError, SingleInstance};
use crate::platform::{
    AlertChoice, BrowserOpener, InstallOutcome, Platform, PlatformError, TrustStatus,
};
use crate::state;

/// Exit code: clean shutdown.
pub const EXIT_OK: u8 = 0;
/// Exit code: the application could not start or an operation failed.
pub const EXIT_FAILED: u8 = 1;
/// Exit code: usage error.
pub const EXIT_USAGE: u8 = 2;
/// Exit code: another instance holds the lock.
pub const EXIT_ALREADY_RUNNING: u8 = 3;

/// The first word of the machine-readable status line.
pub const READY_MARKER: &str = "PFP-READY";

/// What ends `serve`.
#[derive(Debug)]
pub enum Shutdown {
    /// SIGINT or SIGTERM: the binary.
    Signals,
    /// A channel: in-process callers. A dropped sender also shuts down.
    Channel(oneshot::Receiver<()>),
}

/// Adapts the platform's opener to the server's re-open hook.
struct OpenerHook(Arc<dyn BrowserOpener>);

impl RelaunchHook for OpenerHook {
    fn reopen(&self, launch_url: &Redacted<String>) -> Result<(), ReopenUnavailable> {
        self.0.open(launch_url).map_err(|_| ReopenUnavailable)
    }
}

/// The stored material if it is still good, else freshly issued and saved.
fn load_or_issue(store: &dyn LeafStore, now: OffsetDateTime) -> Result<TlsMaterial, &'static str> {
    if let Ok(Some(material)) = store.load() {
        if material.validate(now).is_ok() {
            return Ok(material);
        }
    }
    // Missing, unreadable, loosened permissions, expiring or inconsistent: replace.
    let material = issue(now).map_err(|_| "the local certificate could not be issued")?;
    let _ = store.clear();
    store
        .save(&material)
        .map_err(|_| "the local certificate could not be saved")?;
    Ok(material)
}

/// Step 3. `--no-trust` touches nothing; without `--install-trust` the status is
/// read and nothing is ever installed (the M0 interlock).
fn establish_trust(args: &ServeArgs, platform: &Platform, ca_der: &[u8]) -> TrustMode {
    if args.no_trust {
        return TrustMode::Declined;
    }
    match platform.trust.status(ca_der) {
        Ok(TrustStatus::Trusted) => TrustMode::Installed,
        Ok(TrustStatus::NotTrusted) if args.install_trust => {
            let consent = platform.alerter.explain_trust();
            if consent != Ok(AlertChoice::Continue) {
                return TrustMode::Declined;
            }
            match platform.trust.install(ca_der) {
                Ok(InstallOutcome::Installed) => TrustMode::Installed,
                _ => TrustMode::Declined,
            }
        }
        _ => TrustMode::Declined,
    }
}

fn say(out: &mut dyn Write, line: &str) {
    let _ = writeln!(out, "{line}");
    let _ = out.flush();
}

/// Steps 1–3 and the server configuration: everything before a socket exists.
struct Prepared {
    /// Held until `serve` returns.
    _lock: SingleInstance,
    config: ServerConfig,
    trust_mode: TrustMode,
    can_open: bool,
}

fn prepare(
    args: &ServeArgs,
    platform: &Platform,
    home: Option<&Path>,
) -> Result<Prepared, (u8, String)> {
    let failed = |message: String| (EXIT_FAILED, message);
    let state_dir =
        state::prepare(args.state_dir.as_deref(), home).map_err(|e| failed(e.to_string()))?;
    let lock = SingleInstance::acquire(&state_dir).map_err(|e| match e {
        LockError::AlreadyRunning => (
            EXIT_ALREADY_RUNNING,
            "already running (another instance holds the lock)".to_owned(),
        ),
        LockError::Io => failed("the lock file could not be opened".to_owned()),
    })?;

    let store = FileLeafStore::new(&state_dir);
    let material = load_or_issue(&store, OffsetDateTime::now_utc())
        .map_err(|message| failed(message.to_owned()))?;
    let trust_mode = establish_trust(args, platform, material.ca_cert_der.as_ref());
    let can_open = !args.no_open && platform.opener.available();

    let mut config = ServerConfig::new(material);
    config.preferred_port = args.port.preferred();
    config.assets = AssetManifest::embedded().map_err(|e| failed(e.to_string()))?;
    config.trust_mode = trust_mode;
    config.relaunch = can_open
        .then(|| Arc::new(OpenerHook(Arc::clone(&platform.opener))) as Arc<dyn RelaunchHook>);
    Ok(Prepared {
        _lock: lock,
        config,
        trust_mode,
        can_open,
    })
}

/// Step 5: the status console. The launch token is never part of it.
fn announce(out: &mut dyn Write, bound: &BoundServer, trust_mode: TrustMode, open: &str) {
    let trust = match trust_mode {
        TrustMode::Installed => "installed",
        TrustMode::Declined => "declined",
    };
    say(
        out,
        &format!(
            "{READY_MARKER} origin={} fingerprint=SHA256:{} trust={trust} open={open}",
            bound.origin().origin(),
            bound.leaf_fingerprint(),
        ),
    );
    if bound.fell_back() {
        say(
            out,
            "WARNING: another program is using this application's usual address.",
        );
        say(
            out,
            "Any page that appears there is NOT this application: do not type a passphrase into it.",
        );
        say(out, "This run uses the address on the first line instead.");
    }
    if trust_mode == TrustMode::Declined {
        say(
            out,
            "The local certificate is not trusted by this computer, so the browser will warn.",
        );
        say(
            out,
            "Compare the certificate's SHA-256 fingerprint with the first line before trusting the page.",
        );
    }
}

/// `pfp serve`. Returns the process exit code.
pub fn serve(
    args: &ServeArgs,
    platform: &Platform,
    home: Option<&Path>,
    out: &mut dyn Write,
    err: &mut dyn Write,
    shutdown: Shutdown,
) -> u8 {
    let prepared = match prepare(args, platform, home) {
        Ok(prepared) => prepared,
        Err((code, message)) => {
            say(err, &format!("pfp: {message}"));
            return code;
        }
    };
    let Ok(runtime) = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    else {
        say(err, "pfp: the runtime could not be started");
        return EXIT_FAILED;
    };
    let Prepared {
        _lock,
        config,
        trust_mode,
        can_open,
    } = prepared;
    runtime.block_on(async {
        // Signal handlers first, so a SIGTERM that arrives the instant the ready
        // line is printed is already a graceful shutdown.
        let Ok(stop) = stop_signal(shutdown) else {
            say(err, "pfp: signal handlers could not be installed");
            return EXIT_FAILED;
        };
        let bound = match Server::bind(config) {
            Ok(bound) => bound,
            Err(e) => {
                say(err, &format!("pfp: {e}"));
                return EXIT_FAILED;
            }
        };

        // S-29: warn before anything is opened; no warning shown, nothing opened.
        let mut open = if can_open { "requested" } else { "skipped" };
        if bound.fell_back() {
            let preferred = args.port.preferred().unwrap_or_default();
            let warned = platform.alerter.warn_port_occupied(preferred).is_ok();
            if can_open && !warned {
                open = "withheld";
            }
        }
        announce(out, &bound, trust_mode, open);

        if open == "requested" {
            let opened = bound
                .launch_url()
                .map_err(|_| PlatformError::Unsupported)
                .and_then(|url| platform.opener.open(&url));
            if opened.is_err() {
                say(out, "The browser could not be opened.");
            }
        } else {
            say(out, "No browser was opened by this run.");
        }
        say(out, "Press Ctrl-C to stop.");

        bound.serve(stop).await;
        say(out, "Stopped.");
        EXIT_OK
    })
}

/// A future that completes on the chosen shutdown trigger. Must be called inside
/// the runtime.
fn stop_signal(
    shutdown: Shutdown,
) -> Result<std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>>, ()> {
    use tokio::signal::unix::{signal, SignalKind};
    match shutdown {
        Shutdown::Channel(receiver) => Ok(Box::pin(async move {
            let _ = receiver.await;
        })),
        Shutdown::Signals => {
            let mut terminate = signal(SignalKind::terminate()).map_err(|_| ())?;
            let mut interrupt = signal(SignalKind::interrupt()).map_err(|_| ())?;
            Ok(Box::pin(async move {
                tokio::select! {
                    _ = terminate.recv() => {}
                    _ = interrupt.recv() => {}
                }
            }))
        }
    }
}

/// `pfp trust remove`: removes the trust setting for the stored CA, then the stored
/// leaf. Refuses without `--install-trust` (nothing is read or touched) and while
/// another instance is running.
pub fn trust_remove(
    args: &TrustRemoveArgs,
    platform: &Platform,
    home: Option<&Path>,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    if !args.install_trust {
        say(
            err,
            "pfp: `trust remove` changes trust settings; pass --install-trust to allow it",
        );
        return EXIT_USAGE;
    }
    let state_dir = match state::prepare(args.state_dir.as_deref(), home) {
        Ok(dir) => dir,
        Err(e) => {
            say(err, &format!("pfp: {e}"));
            return EXIT_FAILED;
        }
    };
    let _lock = match SingleInstance::acquire(&state_dir) {
        Ok(lock) => lock,
        Err(LockError::AlreadyRunning) => {
            say(err, "pfp: already running; stop it before removing trust");
            return EXIT_ALREADY_RUNNING;
        }
        Err(LockError::Io) => {
            say(err, "pfp: the lock file could not be opened");
            return EXIT_FAILED;
        }
    };
    let store = FileLeafStore::new(&state_dir);
    if let Ok(Some(material)) = store.load() {
        match platform.trust.remove(material.ca_cert_der.as_ref()) {
            // A build with no platform support never installed anything.
            Ok(()) | Err(PlatformError::Unsupported) => {}
            Err(e) => {
                say(
                    err,
                    &format!("pfp: the trust setting could not be removed: {e}"),
                );
                return EXIT_FAILED;
            }
        }
    }
    if store.clear().is_err() {
        say(err, "pfp: the stored certificate could not be removed");
        return EXIT_FAILED;
    }
    say(
        out,
        "The local certificate and its trust setting were removed.",
    );
    EXIT_OK
}

#[cfg(test)]
mod tests {
    use std::net::{Ipv4Addr, TcpListener};
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex, PoisonError};

    use super::*;
    use crate::cli::PortChoice;
    use crate::platform::fake;

    /// A `Write` the test can read while `serve` runs on another thread.
    #[derive(Clone, Default)]
    struct Shared(Arc<Mutex<Vec<u8>>>);

    impl Write for Shared {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    impl Shared {
        fn text(&self) -> String {
            String::from_utf8_lossy(&self.0.lock().unwrap_or_else(PoisonError::into_inner))
                .into_owned()
        }
    }

    struct TempState(PathBuf);

    impl TempState {
        fn new(name: &str) -> Self {
            let dir =
                std::env::temp_dir().join(format!("pfp-launch-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            Self(dir)
        }
    }

    impl Drop for TempState {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// Runs `serve` to the ready line, stops it, returns (exit code, stdout).
    fn run_once(args: ServeArgs, platform: Platform) -> (u8, String) {
        let out = Shared::default();
        let (stop, stopped) = oneshot::channel();
        let thread = {
            let mut out = out.clone();
            std::thread::spawn(move || {
                let mut err = Vec::new();
                serve(
                    &args,
                    &platform,
                    None,
                    &mut out,
                    &mut err,
                    Shutdown::Channel(stopped),
                )
            })
        };
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
        while !out.text().contains("Press Ctrl-C") && !thread.is_finished() {
            assert!(
                std::time::Instant::now() < deadline,
                "serve did not become ready"
            );
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let _ = stop.send(());
        (thread.join().expect("serve thread"), out.text())
    }

    fn args(state: &TempState) -> ServeArgs {
        ServeArgs {
            state_dir: Some(state.0.clone()),
            // Tests never touch the usual port: OS-assigned unless a test says otherwise.
            port: PortChoice::OsAssigned,
            ..ServeArgs::default()
        }
    }

    #[test]
    fn no_trust_never_touches_the_store_and_wins_over_install_trust() {
        let state = TempState::new("no-trust");
        let (platform, recorder) = fake::platform(AlertChoice::Continue, InstallOutcome::Installed);
        let (code, out) = run_once(
            ServeArgs {
                no_trust: true,
                install_trust: true,
                no_open: true,
                ..args(&state)
            },
            platform,
        );
        assert_eq!(code, EXIT_OK);
        assert_eq!(recorder.calls(), Vec::<String>::new());
        assert!(out.contains("trust=declined open=skipped"));
    }

    #[test]
    fn without_install_trust_nothing_is_installed() {
        let state = TempState::new("interlock");
        let (platform, recorder) = fake::platform(AlertChoice::Continue, InstallOutcome::Installed);
        let (code, out) = run_once(
            ServeArgs {
                no_open: true,
                ..args(&state)
            },
            platform,
        );
        assert_eq!(code, EXIT_OK);
        // Read-only status, then nothing: no explanation, no installation.
        assert_eq!(recorder.calls(), ["trust.status"]);
        assert!(out.contains("trust=declined"));
    }

    #[test]
    fn install_trust_explains_then_installs() {
        let state = TempState::new("install");
        let (platform, recorder) = fake::platform(AlertChoice::Continue, InstallOutcome::Installed);
        let (_, out) = run_once(
            ServeArgs {
                install_trust: true,
                no_open: true,
                ..args(&state)
            },
            platform,
        );
        assert_eq!(
            recorder.calls(),
            ["trust.status", "alert.explain_trust", "trust.install"]
        );
        assert!(out.contains("trust=installed"));
    }

    #[test]
    fn decline_prints_fingerprint() {
        for (choice, outcome, expected) in [
            (
                AlertChoice::Decline,
                InstallOutcome::Installed,
                vec!["trust.status", "alert.explain_trust"],
            ),
            (
                AlertChoice::Continue,
                InstallOutcome::DeclinedByUser,
                vec!["trust.status", "alert.explain_trust", "trust.install"],
            ),
        ] {
            let state = TempState::new("decline");
            let (platform, recorder) = fake::platform(choice, outcome);
            let (code, out) = run_once(
                ServeArgs {
                    install_trust: true,
                    no_open: true,
                    ..args(&state)
                },
                platform,
            );
            assert_eq!(code, EXIT_OK);
            assert_eq!(recorder.calls(), expected);
            let ready = out.lines().next().unwrap();
            assert!(ready.contains("trust=declined"));
            let fingerprint = ready
                .split("fingerprint=SHA256:")
                .nth(1)
                .unwrap()
                .split(' ')
                .next()
                .unwrap();
            assert_eq!(fingerprint.split(':').count(), 32);
            assert!(out.contains("Compare the certificate's SHA-256 fingerprint"));
        }
    }

    #[test]
    fn open_hands_the_launch_url_to_the_opener_and_never_prints_it() {
        let state = TempState::new("open");
        let (platform, recorder) = fake::platform(AlertChoice::Continue, InstallOutcome::Installed);
        let (code, out) = run_once(
            ServeArgs {
                no_trust: true,
                ..args(&state)
            },
            platform,
        );
        assert_eq!(code, EXIT_OK);
        assert_eq!(recorder.calls(), ["open"]);
        let opened = recorder.opened();
        let origin = out
            .lines()
            .next()
            .unwrap()
            .split("origin=")
            .nth(1)
            .unwrap()
            .split(' ')
            .next()
            .unwrap()
            .to_owned();
        let (before, token) = opened[0].split_once("/#t=").expect("token in the fragment");
        assert_eq!(before, origin);
        assert!(!opened[0].contains('?'));
        assert_eq!(token.len(), 64);
        assert!(!out.contains(token), "the token is never printed");
        assert!(out.contains("open=requested"));
    }

    #[test]
    fn occupied_preferred_port_alerts_before_open() {
        let state = TempState::new("occupied");
        let squatter = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let taken = squatter.local_addr().unwrap().port();
        let (platform, recorder) = fake::platform(AlertChoice::Continue, InstallOutcome::Installed);
        let (code, out) = run_once(
            ServeArgs {
                no_trust: true,
                port: PortChoice::Prefer(taken),
                ..args(&state)
            },
            platform,
        );
        assert_eq!(code, EXIT_OK);
        assert_eq!(
            recorder.calls(),
            [format!("alert.port_occupied:{taken}"), "open".to_owned()]
        );
        // Opened only at the fallback origin.
        let opened = recorder.opened();
        assert!(!opened[0].contains(&format!(":{taken}/")));
        assert!(out.contains("WARNING: another program"));
        drop(squatter);
    }

    #[test]
    fn occupied_port_without_a_working_alert_opens_nothing() {
        let state = TempState::new("withheld");
        let squatter = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let taken = squatter.local_addr().unwrap().port();
        let (fake_platform, recorder) =
            fake::platform(AlertChoice::Continue, InstallOutcome::Installed);
        let platform = Platform {
            alerter: Box::new(crate::platform::UnsupportedPlatform),
            ..fake_platform
        };
        let (_, out) = run_once(
            ServeArgs {
                no_trust: true,
                port: PortChoice::Prefer(taken),
                ..args(&state)
            },
            platform,
        );
        assert_eq!(recorder.calls(), Vec::<String>::new());
        assert!(out.contains("open=withheld"));
        drop(squatter);
    }

    #[test]
    fn unsupported_platform_is_decline_mode_no_open_and_no_relaunch_hook() {
        let state = TempState::new("unsupported");
        let (code, out) = run_once(
            ServeArgs {
                install_trust: true,
                ..args(&state)
            },
            Platform::unsupported(),
        );
        assert_eq!(code, EXIT_OK);
        assert!(out.contains("trust=declined open=skipped"));
        assert!(out.contains("No browser was opened"));
    }

    #[test]
    fn the_leaf_is_reused_across_launches() {
        let state = TempState::new("reuse");
        let fingerprint = |out: &str| {
            out.split("fingerprint=")
                .nth(1)
                .unwrap()
                .split(' ')
                .next()
                .unwrap()
                .to_owned()
        };
        let (_, first) = run_once(
            ServeArgs {
                no_trust: true,
                no_open: true,
                ..args(&state)
            },
            Platform::unsupported(),
        );
        let (_, second) = run_once(
            ServeArgs {
                no_trust: true,
                no_open: true,
                ..args(&state)
            },
            Platform::unsupported(),
        );
        assert_eq!(fingerprint(&first), fingerprint(&second));
    }

    #[test]
    fn trust_remove_clears_store_and_leaf_and_needs_the_opt_in() {
        let state = TempState::new("remove");
        let (_, _) = run_once(
            ServeArgs {
                no_trust: true,
                no_open: true,
                ..args(&state)
            },
            Platform::unsupported(),
        );
        let key = state.0.join("tls").join(crate::leaf_store::LEAF_KEY_FILE);
        assert!(key.exists());

        let (platform, recorder) = fake::platform(AlertChoice::Continue, InstallOutcome::Installed);
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let refused = trust_remove(
            &TrustRemoveArgs {
                install_trust: false,
                state_dir: Some(state.0.clone()),
            },
            &platform,
            None,
            &mut out,
            &mut err,
        );
        assert_eq!(refused, EXIT_USAGE);
        assert!(key.exists());
        assert_eq!(recorder.calls(), Vec::<String>::new());

        let done = trust_remove(
            &TrustRemoveArgs {
                install_trust: true,
                state_dir: Some(state.0.clone()),
            },
            &platform,
            None,
            &mut out,
            &mut err,
        );
        assert_eq!(done, EXIT_OK);
        assert_eq!(recorder.calls(), ["trust.remove"]);
        assert!(!key.exists());
        assert!(!state.0.join("tls").exists());
    }

    #[test]
    fn a_second_instance_exits_3() {
        let state = TempState::new("second");
        std::fs::create_dir_all(&state.0).unwrap();
        let held = SingleInstance::acquire(&state.0).unwrap();
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let (_stop, stopped) = oneshot::channel();
        let code = serve(
            &args(&state),
            &Platform::unsupported(),
            None,
            &mut out,
            &mut err,
            Shutdown::Channel(stopped),
        );
        assert_eq!(code, EXIT_ALREADY_RUNNING);
        assert!(out.is_empty());
        assert!(String::from_utf8_lossy(&err).contains("already running"));
        drop(held);
    }
}
