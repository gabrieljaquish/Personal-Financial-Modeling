//! Session establishment (`SECURITY.md` §7.1, test ids S-08 and S-28).
//!
//! A **launch token** (256 bits, single use, 60 s) is exchanged once for a session:
//! a **cookie** value and a **proof** value, two independent 256-bit secrets. Every
//! later API call needs both. Cookies on `127.0.0.1` are scoped by host and not by
//! port, so the cookie alone proves little; the proof lives in `sessionStorage`,
//! which is scoped by origin including the port.
//!
//! * There is at most one live session. A successful bootstrap **replaces** it and
//!   calls [`SessionHooks::on_session_replaced`] (at M1: lock the plan).
//! * A wrong token does not consume the real one; a matching token is consumed on
//!   its first presentation, expired or not.
//! * Every comparison is constant-time over fixed-length decoded bytes.
//! * Time is a monotonic [`Clock`]. Validity is an elapsed-duration comparison that
//!   fails closed: if the clock is ever seen to step backwards, every outstanding
//!   launch token is void. (On macOS `Instant` does not advance during sleep, so a
//!   token minted just before sleep keeps its remainder after wake; single use
//!   bounds that.)
//! * Idle auto-lock is **plumbing only** at M0: the timestamp, the constant, the
//!   check and the hook exist; the hook's body is the launcher's, and the session
//!   is not invalidated, because there is no plan or key to protect yet and no
//!   re-unlock screen to return to. API requests do **not** count as interaction
//!   (`SECURITY.md` §15: periodic polling must still lock).
//!
//! No secret here has a `Debug`, `Display` or `Serialize` form: all are
//! [`Redacted`], and the hex form leaves only through [`Redacted::expose`].

use std::fmt;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Instant;

use subtle::ConstantTimeEq;

use crate::events::{EventCode, EventLog};
use crate::limits::{
    BOOTSTRAP_FAILURE_WARN_AT, IDLE_LOCK_AFTER, LAUNCH_TOKEN_TTL, RELAUNCH_MAX_PER_SESSION,
    RELAUNCH_MIN_INTERVAL,
};
use crate::redact::Redacted;

/// Bytes in every secret.
pub const SECRET_BYTES: usize = 32;
/// Hex characters in every secret's wire form.
pub const SECRET_HEX_LEN: usize = SECRET_BYTES * 2;
/// The session cookie's name. The `__Host-` prefix makes the browser refuse it
/// unless it is `Secure`, has `Path=/` and no `Domain`.
pub const COOKIE_NAME: &str = "__Host-pfp";
/// The proof header's name.
pub const PROOF_HEADER: &str = "x-pfp-proof";

type Secret = Redacted<[u8; SECRET_BYTES]>;

/// A monotonic clock. Never the wall clock: a launch token must not be extended by
/// an NTP step or by the user changing the date.
pub trait Clock: Send + Sync {
    /// The current instant.
    fn now(&self) -> Instant;
}

/// [`Instant::now`].
#[derive(Debug, Default, Clone, Copy)]
pub struct MonotonicClock;

impl Clock for MonotonicClock {
    fn now(&self) -> Instant {
        Instant::now()
    }
}

/// What the owner of the process does when the session changes.
pub trait SessionHooks: Send + Sync {
    /// A bootstrap replaced a live session. At M1 this locks the plan ("forces
    /// re-unlock"); at M0 there is nothing to lock.
    fn on_session_replaced(&self) {}
    /// The session has been idle for [`IDLE_LOCK_AFTER`]. Called once per expiry.
    fn on_idle_timeout(&self) {}
}

/// Hooks that do nothing.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoHooks;

impl SessionHooks for NoHooks {}

/// The operating system's entropy source failed; no secret was produced.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EntropyError;

impl fmt::Display for EntropyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("the operating system's entropy source failed")
    }
}

impl std::error::Error for EntropyError {}

/// Why a bootstrap did not produce a session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BootstrapError {
    /// Wrong, expired, already used, or none outstanding. Deliberately one case.
    Invalid,
    /// See [`EntropyError`].
    Entropy,
}

/// Why a relaunch did not mint a token.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelaunchError {
    /// Inside the minimum interval: waiting helps.
    Throttled,
    /// Past the per-session cap: waiting does **not** help; only a new session
    /// (a fresh start from the launcher) does.
    Exhausted,
    /// See [`EntropyError`].
    Entropy,
}

/// The two halves of a fresh session, in wire (hex) form.
pub struct Established {
    /// Goes into the `__Host-` cookie.
    pub cookie: Redacted<String>,
    /// Goes into the response body, then `sessionStorage`.
    pub proof: Redacted<String>,
}

impl fmt::Debug for Established {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Established").finish_non_exhaustive()
    }
}

/// What a request presented. Values are borrowed and never stored.
#[derive(Debug, Clone, Copy)]
pub struct Presented<'a> {
    /// The `X-PFP-Proof` value, when exactly one header was sent.
    pub proof: Option<&'a str>,
    /// Every value of a cookie pair named [`COOKIE_NAME`], across all `Cookie`
    /// headers. More than one is a displaced jar, not "take the first".
    pub cookies: &'a [&'a str],
}

/// The outcome of checking a request against the live session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionCheck {
    /// Both factors match.
    Ok,
    /// No live session, or the proof is missing or wrong → 401.
    Required,
    /// The proof matches; the cookie is missing, wrong or duplicated → 409.
    CookieDisplaced,
}

struct LaunchToken {
    value: Secret,
    minted: Instant,
    failed_attempts: u32,
}

struct Session {
    cookie: Secret,
    proof: Secret,
    last_interaction: Instant,
    idle_reported: bool,
    relaunches: u32,
    last_relaunch: Option<Instant>,
}

impl Session {
    fn idle_expired(&self, now: Instant) -> bool {
        now.checked_duration_since(self.last_interaction)
            .is_some_and(|idle| idle >= IDLE_LOCK_AFTER)
    }
}

#[derive(Default)]
struct Inner {
    launch: Option<LaunchToken>,
    session: Option<Session>,
    latest_now: Option<Instant>,
}

/// Owns the launch token and the one live session.
pub struct SessionManager {
    inner: Mutex<Inner>,
    clock: Arc<dyn Clock>,
    hooks: Arc<dyn SessionHooks>,
    events: Arc<EventLog>,
}

impl fmt::Debug for SessionManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SessionManager").finish_non_exhaustive()
    }
}

fn fresh_secret() -> Result<Secret, EntropyError> {
    let mut bytes = [0_u8; SECRET_BYTES];
    getrandom::getrandom(&mut bytes).map_err(|_| EntropyError)?;
    Ok(Redacted::new(bytes))
}

fn to_hex(secret: &Secret) -> Redacted<String> {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(SECRET_HEX_LEN);
    for byte in secret.expose() {
        out.push(char::from(DIGITS[usize::from(byte >> 4)]));
        out.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    Redacted::new(out)
}

/// Decodes exactly 64 lower-case hex characters. Anything else is `None`; the
/// shape of a presented value is not a secret, its content is.
fn from_hex(text: &str) -> Option<[u8; SECRET_BYTES]> {
    let bytes = text.as_bytes();
    if bytes.len() != SECRET_HEX_LEN {
        return None;
    }
    let nibble = |b: u8| match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        _ => None,
    };
    let mut out = [0_u8; SECRET_BYTES];
    let (pairs, _) = bytes.as_chunks::<2>();
    for (slot, [high, low]) in out.iter_mut().zip(pairs) {
        *slot = (nibble(*high)? << 4) | nibble(*low)?;
    }
    Some(out)
}

/// Constant-time equality of a presented value with a secret. A value of the
/// wrong shape is compared against nothing and is simply unequal.
fn matches(presented: Option<&str>, secret: &Secret) -> bool {
    presented
        .and_then(from_hex)
        .is_some_and(|bytes| bool::from(bytes.ct_eq(secret.expose())))
}

impl SessionManager {
    /// A manager with the given clock, hooks and event log.
    #[must_use]
    pub fn new(clock: Arc<dyn Clock>, hooks: Arc<dyn SessionHooks>, events: Arc<EventLog>) -> Self {
        Self {
            inner: Mutex::new(Inner::default()),
            clock,
            hooks,
            events,
        }
    }

    /// Reads the clock and voids outstanding launch tokens if it went backwards.
    fn observe(&self, inner: &mut Inner) -> Instant {
        let now = self.clock.now();
        match inner.latest_now {
            Some(latest) if now < latest => {
                if inner.launch.take().is_some() {
                    self.events.record(EventCode::ClockWentBackwards);
                }
                // `latest_now` keeps the high-water mark.
            }
            _ => inner.latest_now = Some(now),
        }
        now
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Mints a launch token, replacing any outstanding one, and returns its hex
    /// form for the URL **fragment**. The caller hands it to the browser opener
    /// and nowhere else: never argv, the environment, a file, a log or stdout.
    ///
    /// # Errors
    /// [`EntropyError`] when the OS entropy source fails.
    pub fn mint_launch_token(&self) -> Result<Redacted<String>, EntropyError> {
        let value = fresh_secret().inspect_err(|_| {
            self.events.record(EventCode::EntropyUnavailable);
        })?;
        let hex = to_hex(&value);
        let mut inner = self.lock();
        let minted = self.observe(&mut inner);
        inner.launch = Some(LaunchToken {
            value,
            minted,
            failed_attempts: 0,
        });
        drop(inner);
        self.events.record(EventCode::LaunchTokenMinted);
        Ok(hex)
    }

    /// Exchanges a launch token for a session.
    ///
    /// # Errors
    /// [`BootstrapError::Invalid`] for a wrong, expired or reused token (the real
    /// one is consumed only by a matching presentation).
    pub fn bootstrap(&self, presented: &str) -> Result<Established, BootstrapError> {
        let mut inner = self.lock();
        let now = self.observe(&mut inner);

        let matched = inner
            .launch
            .as_ref()
            .is_some_and(|launch| matches(Some(presented), &launch.value));
        if !matched {
            let warn = inner.launch.as_mut().is_some_and(|launch| {
                launch.failed_attempts = launch.failed_attempts.saturating_add(1);
                launch.failed_attempts == BOOTSTRAP_FAILURE_WARN_AT
            });
            drop(inner);
            self.events.record(EventCode::BootstrapRejected);
            if warn {
                self.events.record(EventCode::BootstrapFailuresHigh);
            }
            return Err(BootstrapError::Invalid);
        }

        // Single use: consumed by the first matching presentation, live or not.
        let launch = inner.launch.take();
        let live = launch.is_some_and(|launch| {
            now.checked_duration_since(launch.minted)
                .is_some_and(|age| age < LAUNCH_TOKEN_TTL)
        });
        if !live {
            drop(inner);
            self.events.record(EventCode::BootstrapRejected);
            return Err(BootstrapError::Invalid);
        }

        let (Ok(cookie), Ok(proof)) = (fresh_secret(), fresh_secret()) else {
            drop(inner);
            self.events.record(EventCode::EntropyUnavailable);
            return Err(BootstrapError::Entropy);
        };
        let established = Established {
            cookie: to_hex(&cookie),
            proof: to_hex(&proof),
        };
        let replaced = inner
            .session
            .replace(Session {
                cookie,
                proof,
                last_interaction: now,
                idle_reported: false,
                relaunches: 0,
                last_relaunch: None,
            })
            .is_some();
        drop(inner);

        if replaced {
            self.events.record(EventCode::SessionReplaced);
            self.hooks.on_session_replaced();
        }
        self.events.record(EventCode::SessionEstablished);
        Ok(established)
    }

    /// Checks both factors. Does **not** count as interaction.
    #[must_use]
    pub fn check(&self, presented: Presented<'_>) -> SessionCheck {
        let inner = self.lock();
        let Some(session) = inner.session.as_ref() else {
            return SessionCheck::Required;
        };
        // Both comparisons always run, so timing does not say which half failed.
        let proof_ok = matches(presented.proof, &session.proof);
        let cookie_ok = matches(presented.cookies.first().copied(), &session.cookie)
            && presented.cookies.len() == 1;
        match (proof_ok, cookie_ok) {
            (true, true) => SessionCheck::Ok,
            (true, false) => SessionCheck::CookieDisplaced,
            (false, _) => SessionCheck::Required,
        }
    }

    /// Checks the proof alone: what `relaunch` requires, because its whole purpose
    /// is to recover from a displaced cookie.
    #[must_use]
    pub fn check_proof(&self, proof: Option<&str>) -> bool {
        let inner = self.lock();
        inner
            .session
            .as_ref()
            .is_some_and(|session| matches(proof, &session.proof))
    }

    /// Mints a fresh launch token for a re-open, subject to the throttle: at most
    /// one per [`RELAUNCH_MIN_INTERVAL`] and [`RELAUNCH_MAX_PER_SESSION`] per
    /// session. A refused call mints nothing. The caller has already checked the
    /// proof.
    ///
    /// The two refusals are distinct because what the user can do about them is:
    /// the interval passes by itself, the cap never does. The cap is checked first,
    /// so a session that has used it up is never told to "wait a moment".
    ///
    /// # Errors
    /// [`RelaunchError`].
    pub fn relaunch(&self) -> Result<Redacted<String>, RelaunchError> {
        {
            let mut inner = self.lock();
            let now = self.observe(&mut inner);
            let Some(session) = inner.session.as_mut() else {
                return Err(RelaunchError::Throttled);
            };
            let too_soon = session.last_relaunch.is_some_and(|last| {
                // A backwards clock counts as "too soon": fail closed.
                now.checked_duration_since(last)
                    .is_none_or(|since| since < RELAUNCH_MIN_INTERVAL)
            });
            if session.relaunches >= RELAUNCH_MAX_PER_SESSION {
                drop(inner);
                self.events.record(EventCode::RelaunchExhausted);
                return Err(RelaunchError::Exhausted);
            }
            if too_soon {
                drop(inner);
                self.events.record(EventCode::RelaunchThrottled);
                return Err(RelaunchError::Throttled);
            }
            session.relaunches += 1;
            session.last_relaunch = Some(now);
        }
        self.mint_launch_token().map_err(|_| RelaunchError::Entropy)
    }

    /// Records a user interaction. At M0 nothing calls this over HTTP; the first
    /// screen wires it to the front end's interaction heartbeat.
    pub fn note_interaction(&self) {
        let mut inner = self.lock();
        let now = self.observe(&mut inner);
        if let Some(session) = inner.session.as_mut() {
            session.last_interaction = now;
            session.idle_reported = false;
        }
    }

    /// Evaluates the idle condition; on expiry calls the hook once and returns
    /// `true`. The session is not invalidated at M0.
    pub fn check_idle(&self) -> bool {
        let fire = {
            let mut inner = self.lock();
            let now = self.observe(&mut inner);
            match inner.session.as_mut() {
                Some(session) if !session.idle_reported && session.idle_expired(now) => {
                    session.idle_reported = true;
                    true
                }
                _ => false,
            }
        };
        if fire {
            self.events.record(EventCode::IdleTimeout);
            self.hooks.on_idle_timeout();
        }
        fire
    }

    /// Whether a session is live.
    #[must_use]
    pub fn has_session(&self) -> bool {
        self.lock().session.is_some()
    }
}

/// Collects the values of every [`COOKIE_NAME`] pair in the given `Cookie` header
/// values.
#[must_use]
pub fn session_cookies<'a>(cookie_headers: impl Iterator<Item = &'a str>) -> Vec<&'a str> {
    cookie_headers
        .flat_map(|header| header.split(';'))
        .filter_map(|pair| {
            let (name, value) = pair.trim().split_once('=')?;
            (name == COOKIE_NAME).then_some(value)
        })
        .collect()
}

/// The exact `Set-Cookie` value for a fresh session (`SECURITY.md` §7.1). Session
/// scoped: no `Max-Age`, no `Expires`, no `Domain`.
#[must_use]
pub fn set_cookie_value(cookie: &Redacted<String>) -> Redacted<String> {
    Redacted::new(format!(
        "{COOKIE_NAME}={}; Secure; HttpOnly; SameSite=Strict; Path=/",
        cookie.expose()
    ))
}

/// A clock a test can step, in either direction.
#[cfg(test)]
pub(crate) mod fake {
    use std::time::Duration;

    use super::{Clock, Instant, Mutex, PoisonError};

    pub(crate) struct FakeClock {
        now: Mutex<Instant>,
    }

    impl FakeClock {
        pub(crate) fn new() -> Self {
            // Far enough from the platform's epoch that stepping back cannot underflow.
            Self {
                now: Mutex::new(Instant::now() + Duration::from_secs(3600)),
            }
        }
        pub(crate) fn advance(&self, by: Duration) {
            *self.now.lock().unwrap_or_else(PoisonError::into_inner) += by;
        }
        pub(crate) fn step_back(&self, by: Duration) {
            *self.now.lock().unwrap_or_else(PoisonError::into_inner) -= by;
        }
    }

    impl Clock for FakeClock {
        fn now(&self) -> Instant {
            *self.now.lock().unwrap_or_else(PoisonError::into_inner)
        }
    }
}

#[cfg(test)]
// Secrets are compared with `assert!`, never `assert_eq!`/`assert_ne!`, which would
// print both sides on failure.
#[allow(clippy::manual_assert_eq)]
mod tests {
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::time::Duration;

    use super::fake::FakeClock;
    use super::*;

    #[derive(Default)]
    struct CountingHooks {
        replaced: AtomicU32,
        idle: AtomicU32,
    }

    impl SessionHooks for CountingHooks {
        fn on_session_replaced(&self) {
            self.replaced.fetch_add(1, Ordering::SeqCst);
        }
        fn on_idle_timeout(&self) {
            self.idle.fetch_add(1, Ordering::SeqCst);
        }
    }

    struct Rig {
        clock: Arc<FakeClock>,
        hooks: Arc<CountingHooks>,
        events: Arc<EventLog>,
        manager: SessionManager,
    }

    fn rig() -> Rig {
        let clock = Arc::new(FakeClock::new());
        let hooks = Arc::new(CountingHooks::default());
        let events = Arc::new(EventLog::with_capacity(256, false));
        let manager = SessionManager::new(clock.clone(), hooks.clone(), events.clone());
        Rig {
            clock,
            hooks,
            events,
            manager,
        }
    }

    fn secs(n: u64) -> Duration {
        Duration::from_secs(n)
    }

    fn check(manager: &SessionManager, proof: Option<&str>, cookies: &[&str]) -> SessionCheck {
        manager.check(Presented { proof, cookies })
    }

    #[test]
    fn tokens_are_64_lower_hex_and_distinct() {
        let r = rig();
        let a = r.manager.mint_launch_token().unwrap();
        let b = r.manager.mint_launch_token().unwrap();
        for t in [&a, &b] {
            assert_eq!(t.expose().len(), SECRET_HEX_LEN);
            assert!(from_hex(t.expose()).is_some());
        }
        assert!(a.expose() != b.expose());
        let s = r.manager.bootstrap(b.expose()).unwrap();
        assert!(s.cookie.expose() != s.proof.expose());
        assert!(s.cookie.expose() != b.expose());
    }

    #[test]
    fn launch_token_is_single_use() {
        let r = rig();
        let token = r.manager.mint_launch_token().unwrap();
        assert!(r.manager.bootstrap(token.expose()).is_ok());
        assert_eq!(
            r.manager.bootstrap(token.expose()).unwrap_err(),
            BootstrapError::Invalid
        );
    }

    #[test]
    fn launch_token_lives_less_than_60_seconds() {
        let r = rig();
        let token = r.manager.mint_launch_token().unwrap();
        r.clock.advance(secs(59));
        assert!(r.manager.bootstrap(token.expose()).is_ok());

        let token = r.manager.mint_launch_token().unwrap();
        r.clock.advance(secs(60));
        assert!(r.manager.bootstrap(token.expose()).is_err());
        // Presenting it consumed it: it does not come back if the clock is wound.
        r.clock.step_back(secs(30));
        assert!(r.manager.bootstrap(token.expose()).is_err());
    }

    #[test]
    fn wrong_token_does_not_consume_the_real_one() {
        let r = rig();
        let token = r.manager.mint_launch_token().unwrap();
        for wrong in ["", "00", &"0".repeat(64), &"g".repeat(64), &"A".repeat(64)] {
            assert!(r.manager.bootstrap(wrong).is_err());
        }
        assert!(r.manager.bootstrap(token.expose()).is_ok());
    }

    #[test]
    fn upper_case_hex_of_the_right_token_is_refused() {
        let r = rig();
        let token = r.manager.mint_launch_token().unwrap();
        let upper = token.expose().to_ascii_uppercase();
        if upper != *token.expose() {
            assert!(r.manager.bootstrap(&upper).is_err());
        }
        assert!(r.manager.bootstrap(token.expose()).is_ok());
    }

    #[test]
    fn backwards_clock_never_extends_a_token() {
        let r = rig();
        r.clock.advance(secs(100));
        let _ = r.manager.has_session();
        let token = r.manager.mint_launch_token().unwrap();
        r.clock.step_back(secs(50));
        assert!(r.manager.bootstrap(token.expose()).is_err());
        assert_eq!(r.events.count(EventCode::ClockWentBackwards), 1);
        // Even once the clock is back where it was.
        r.clock.advance(secs(50));
        assert!(r.manager.bootstrap(token.expose()).is_err());
    }

    #[test]
    fn a_new_launch_token_replaces_the_outstanding_one() {
        let r = rig();
        let first = r.manager.mint_launch_token().unwrap();
        let second = r.manager.mint_launch_token().unwrap();
        assert!(r.manager.bootstrap(first.expose()).is_err());
        assert!(r.manager.bootstrap(second.expose()).is_ok());
    }

    #[test]
    fn both_factors_are_required() {
        let r = rig();
        assert_eq!(check(&r.manager, None, &[]), SessionCheck::Required);
        let token = r.manager.mint_launch_token().unwrap();
        let s = r.manager.bootstrap(token.expose()).unwrap();
        let (cookie, proof) = (s.cookie.expose().as_str(), s.proof.expose().as_str());

        assert_eq!(check(&r.manager, Some(proof), &[cookie]), SessionCheck::Ok);
        // Cookie only, neither, wrong proof: 401.
        assert_eq!(check(&r.manager, None, &[cookie]), SessionCheck::Required);
        assert_eq!(check(&r.manager, None, &[]), SessionCheck::Required);
        assert_eq!(
            check(&r.manager, Some(cookie), &[cookie]),
            SessionCheck::Required
        );
        // Proof only, wrong cookie, duplicated cookie: 409.
        assert_eq!(
            check(&r.manager, Some(proof), &[]),
            SessionCheck::CookieDisplaced
        );
        assert_eq!(
            check(&r.manager, Some(proof), &[proof]),
            SessionCheck::CookieDisplaced
        );
        assert_eq!(
            check(&r.manager, Some(proof), &[cookie, cookie]),
            SessionCheck::CookieDisplaced
        );
        assert!(r.manager.check_proof(Some(proof)));
        assert!(!r.manager.check_proof(Some(cookie)));
        assert!(!r.manager.check_proof(None));
    }

    #[test]
    fn second_bootstrap_replaces_the_session_and_calls_the_hook() {
        let r = rig();
        let t1 = r.manager.mint_launch_token().unwrap();
        let s1 = r.manager.bootstrap(t1.expose()).unwrap();
        assert_eq!(r.hooks.replaced.load(Ordering::SeqCst), 0);
        let t2 = r.manager.mint_launch_token().unwrap();
        let s2 = r.manager.bootstrap(t2.expose()).unwrap();
        assert_eq!(r.hooks.replaced.load(Ordering::SeqCst), 1);
        assert_eq!(
            check(&r.manager, Some(s1.proof.expose()), &[s1.cookie.expose()]),
            SessionCheck::Required
        );
        assert_eq!(
            check(&r.manager, Some(s2.proof.expose()), &[s2.cookie.expose()]),
            SessionCheck::Ok
        );
    }

    #[test]
    fn failed_bootstraps_are_counted_not_refused() {
        let r = rig();
        let token = r.manager.mint_launch_token().unwrap();
        for _ in 0..25 {
            assert!(r.manager.bootstrap(&"0".repeat(64)).is_err());
        }
        assert_eq!(r.events.count(EventCode::BootstrapRejected), 25);
        assert_eq!(r.events.count(EventCode::BootstrapFailuresHigh), 1);
        assert!(r.manager.bootstrap(token.expose()).is_ok());
    }

    #[test]
    fn relaunch_is_throttled_by_interval_and_by_cap() {
        let r = rig();
        assert_eq!(r.manager.relaunch().unwrap_err(), RelaunchError::Throttled);
        let token = r.manager.mint_launch_token().unwrap();
        r.manager.bootstrap(token.expose()).unwrap();

        let first = r.manager.relaunch().unwrap();
        r.clock.advance(secs(9));
        assert_eq!(r.manager.relaunch().unwrap_err(), RelaunchError::Throttled);
        r.clock.advance(secs(1));
        let second = r.manager.relaunch().unwrap();
        // Each accepted call replaces the outstanding launch token.
        assert!(r.manager.bootstrap(first.expose()).is_err());

        for _ in 2..RELAUNCH_MAX_PER_SESSION {
            r.clock.advance(secs(10));
            r.manager.relaunch().unwrap();
        }
        // The cap is its own refusal, however long the caller waits — and also
        // when the call happens to be inside the interval as well.
        r.clock.advance(secs(10));
        assert_eq!(r.manager.relaunch().unwrap_err(), RelaunchError::Exhausted);
        assert_eq!(r.manager.relaunch().unwrap_err(), RelaunchError::Exhausted);
        r.clock.advance(secs(3600));
        assert_eq!(r.manager.relaunch().unwrap_err(), RelaunchError::Exhausted);
        assert_eq!(r.events.count(EventCode::RelaunchExhausted), 3);
        assert_eq!(r.events.count(EventCode::RelaunchThrottled), 1);
        assert!(r.manager.bootstrap(second.expose()).is_err());

        // A successful bootstrap resets the cap.
        let last = r.manager.mint_launch_token().unwrap();
        r.manager.bootstrap(last.expose()).unwrap();
        assert!(r.manager.relaunch().is_ok());
    }

    #[test]
    fn idle_expiry_fires_hook_once_at_15_min() {
        let r = rig();
        let token = r.manager.mint_launch_token().unwrap();
        r.manager.bootstrap(token.expose()).unwrap();
        r.clock.advance(secs(15 * 60 - 1));
        assert!(!r.manager.check_idle());
        r.clock.advance(secs(1));
        assert!(r.manager.check_idle());
        r.clock.advance(secs(600));
        assert!(!r.manager.check_idle(), "once per expiry");
        assert_eq!(r.hooks.idle.load(Ordering::SeqCst), 1);
        assert_eq!(r.events.count(EventCode::IdleTimeout), 1);
        // M0: the session is not invalidated.
        assert!(r.manager.has_session());
        // An interaction re-arms it.
        r.manager.note_interaction();
        r.clock.advance(secs(15 * 60));
        assert!(r.manager.check_idle());
        assert_eq!(r.hooks.idle.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn api_requests_do_not_count_as_interaction() {
        let r = rig();
        let token = r.manager.mint_launch_token().unwrap();
        let s = r.manager.bootstrap(token.expose()).unwrap();
        for _ in 0..15 {
            r.clock.advance(secs(60));
            assert_eq!(
                check(&r.manager, Some(s.proof.expose()), &[s.cookie.expose()]),
                SessionCheck::Ok
            );
        }
        assert!(r.manager.check_idle(), "polling must not keep it unlocked");
    }

    #[test]
    fn cookie_pairs_are_collected_across_headers() {
        let found = session_cookies(
            [
                "a=1; __Host-pfp=x; b=2",
                "__Host-pfp=y",
                "__Host-pfpx=z; pfp=w",
            ]
            .into_iter(),
        );
        assert_eq!(found, ["x", "y"]);
        assert!(session_cookies([""].into_iter()).is_empty());
    }

    #[test]
    fn set_cookie_is_the_documented_string_and_redacted() {
        let value = set_cookie_value(&Redacted::new("ab".repeat(32)));
        assert_eq!(
            *value.expose(),
            format!(
                "__Host-pfp={}; Secure; HttpOnly; SameSite=Strict; Path=/",
                "ab".repeat(32)
            )
        );
        assert_eq!(format!("{value:?}"), "<redacted>");
    }

    #[test]
    fn secrets_never_reach_events_or_debug() {
        let r = rig();
        let token = r.manager.mint_launch_token().unwrap();
        let s = r.manager.bootstrap(token.expose()).unwrap();
        let _ = r.manager.bootstrap(token.expose());
        let rendered = format!(
            "{:?} {:?} {:?} {:?}",
            r.manager,
            s,
            token,
            r.events.snapshot()
        );
        for secret in [token.expose(), s.cookie.expose(), s.proof.expose()] {
            assert!(!rendered.contains(secret.as_str()));
        }
        // No run of 64 hex characters at all.
        let longest = rendered
            .split(|c: char| !c.is_ascii_hexdigit())
            .map(str::len)
            .max()
            .unwrap_or(0);
        assert!(longest < SECRET_HEX_LEN);
    }
}
