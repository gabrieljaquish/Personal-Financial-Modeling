//! Structured events: stable codes in a bounded in-memory ring buffer
//! (`SECURITY.md` §8, "logs describe control flow, never content").
//!
//! An [`Event`] cannot carry content, by construction: its fields are an enum code,
//! a `&'static str` method name and route **template**, and a status number. There
//! is no field a request body, a header value, a token, a cookie, a path or an
//! amount could be put into, so no call site can log one by mistake. By default
//! only `warn` and `error` events reach stderr; `pfp serve --verbose` echoes every
//! event, which is safe for the same reason.

use std::collections::VecDeque;
use std::fmt;
use std::sync::{Mutex, PoisonError};

use crate::limits::EVENT_RING_CAPACITY;

/// Severity of an event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    /// Ordinary control flow.
    Info,
    /// Something refused or unusual; also written to stderr.
    Warn,
    /// A defect or an environment failure; also written to stderr.
    Error,
}

/// Every event the server can record. The wire form is the stable code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EventCode {
    /// A request was answered; carries method, route template and status.
    RequestServed,
    /// A request was refused by admission; carries the status.
    RequestRefused,
    /// A request exceeded the whole-request deadline.
    RequestDeadline,
    /// A launch token was minted.
    LaunchTokenMinted,
    /// A bootstrap succeeded and a session exists.
    SessionEstablished,
    /// A bootstrap replaced a live session.
    SessionReplaced,
    /// A bootstrap presented a wrong, expired or reused token.
    BootstrapRejected,
    /// Many bootstraps have failed for one launch token.
    BootstrapFailuresHigh,
    /// An API call lacked the proof, or no session is live.
    SessionRequired,
    /// An API call had the proof but not the cookie.
    SessionCookieDisplaced,
    /// A relaunch was accepted and handed to the opener.
    RelaunchAccepted,
    /// A relaunch was refused by the minimum interval.
    RelaunchThrottled,
    /// A relaunch was refused by the per-session cap.
    RelaunchExhausted,
    /// A relaunch was asked of a build with no opener.
    RelaunchUnavailable,
    /// The session clock went backwards; outstanding launch tokens were voided.
    ClockWentBackwards,
    /// The session has been idle for the auto-lock interval.
    IdleTimeout,
    /// The operating system's entropy source failed.
    EntropyUnavailable,
    /// A handler failed in a way the design does not anticipate.
    InternalError,
    /// The accept loop ended.
    ServerStopped,
}

impl EventCode {
    /// The stable code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::RequestServed => "request_served",
            Self::RequestRefused => "request_refused",
            Self::RequestDeadline => "request_deadline",
            Self::LaunchTokenMinted => "launch_token_minted",
            Self::SessionEstablished => "session_established",
            Self::SessionReplaced => "session_replaced",
            Self::BootstrapRejected => "bootstrap_rejected",
            Self::BootstrapFailuresHigh => "bootstrap_failures_high",
            Self::SessionRequired => "session_required",
            Self::SessionCookieDisplaced => "session_cookie_displaced",
            Self::RelaunchAccepted => "relaunch_accepted",
            Self::RelaunchThrottled => "relaunch_throttled",
            Self::RelaunchExhausted => "relaunch_exhausted",
            Self::RelaunchUnavailable => "relaunch_unavailable",
            Self::ClockWentBackwards => "clock_went_backwards",
            Self::IdleTimeout => "idle_timeout",
            Self::EntropyUnavailable => "entropy_unavailable",
            Self::InternalError => "internal_error",
            Self::ServerStopped => "server_stopped",
        }
    }

    /// The severity this code is always recorded at.
    #[must_use]
    pub const fn level(self) -> Level {
        match self {
            Self::RequestServed
            | Self::RequestRefused
            | Self::LaunchTokenMinted
            | Self::SessionEstablished
            | Self::SessionReplaced
            | Self::BootstrapRejected
            | Self::SessionRequired
            | Self::SessionCookieDisplaced
            | Self::RelaunchAccepted
            | Self::RelaunchUnavailable
            | Self::IdleTimeout
            | Self::ServerStopped => Level::Info,
            Self::RequestDeadline
            | Self::BootstrapFailuresHigh
            | Self::RelaunchThrottled
            | Self::RelaunchExhausted
            | Self::ClockWentBackwards => Level::Warn,
            Self::EntropyUnavailable | Self::InternalError => Level::Error,
        }
    }
}

/// One recorded event. Every field is a code, a compile-time string or a number.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Event {
    /// Position in the process-wide sequence, from 1.
    pub seq: u64,
    /// What happened.
    pub code: EventCode,
    /// The request method, for request events.
    pub method: Option<&'static str>,
    /// The route **template** (never the path as sent), for request events.
    pub route: Option<&'static str>,
    /// The response status, for request events.
    pub status: Option<u16>,
}

impl fmt::Display for Event {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{} {}", self.seq, self.code.code())?;
        if let Some(method) = self.method {
            write!(f, " {method}")?;
        }
        if let Some(route) = self.route {
            write!(f, " {route}")?;
        }
        if let Some(status) = self.status {
            write!(f, " {status}")?;
        }
        Ok(())
    }
}

#[derive(Debug)]
struct Ring {
    next_seq: u64,
    events: VecDeque<Event>,
}

/// The bounded in-memory event log. It is the real log; nothing is written to a
/// file. By default only `warn`/`error` codes are echoed to stderr; `pfp serve
/// --verbose` echoes every code. An event holds only a code, a route template, a
/// status and counters, so echoing all of them prints no token, cookie or path.
#[derive(Debug)]
pub struct EventLog {
    ring: Mutex<Ring>,
    capacity: usize,
    /// The lowest level echoed to stderr; `None` echoes nothing.
    echo_from: Option<Level>,
}

impl Default for EventLog {
    fn default() -> Self {
        Self::new()
    }
}

impl EventLog {
    /// A log of the production capacity that echoes `warn` and `error` to stderr.
    #[must_use]
    pub fn new() -> Self {
        Self::with_capacity(EVENT_RING_CAPACITY, true)
    }

    /// A log with an explicit capacity; `echo_to_stderr` off keeps tests quiet.
    #[must_use]
    pub fn with_capacity(capacity: usize, echo_to_stderr: bool) -> Self {
        Self::with_echo(capacity, echo_to_stderr.then_some(Level::Warn))
    }

    /// A log with an explicit capacity that echoes every event at or above
    /// `echo_from` to stderr (`None`: echo nothing).
    #[must_use]
    pub fn with_echo(capacity: usize, echo_from: Option<Level>) -> Self {
        Self {
            ring: Mutex::new(Ring {
                next_seq: 1,
                events: VecDeque::with_capacity(capacity.min(EVENT_RING_CAPACITY)),
            }),
            capacity: capacity.max(1),
            echo_from,
        }
    }

    /// Records an event that is not about a request.
    pub fn record(&self, code: EventCode) {
        self.push(code, None, None, None);
    }

    /// Records a request event: method name, route template and status only.
    pub fn record_request(
        &self,
        code: EventCode,
        method: &'static str,
        route: &'static str,
        status: u16,
    ) {
        self.push(code, Some(method), Some(route), Some(status));
    }

    fn push(
        &self,
        code: EventCode,
        method: Option<&'static str>,
        route: Option<&'static str>,
        status: Option<u16>,
    ) {
        let event = {
            let mut ring = self.ring.lock().unwrap_or_else(PoisonError::into_inner);
            let event = Event {
                seq: ring.next_seq,
                code,
                method,
                route,
                status,
            };
            ring.next_seq += 1;
            if ring.events.len() == self.capacity {
                ring.events.pop_front();
            }
            ring.events.push_back(event);
            event
        };
        // DEFERRED (`SECURITY.md` §8; README, "Security test ids in this step"): the
        // system-log sink is specified as `os_log` with every interpolated value
        // `%{private}`. That is FFI and this crate forbids `unsafe`; it lands with
        // the macOS platform module, before the first launchd-launched `.app`
        // build. Until then this echo is a terminal's stderr, and an `Event` can
        // only hold a code, a route template, a status and counters.
        if self.echo_from.is_some_and(|from| code.level() >= from) {
            eprintln!("pfp: {event}");
        }
    }

    /// The events still in the buffer, oldest first.
    #[must_use]
    pub fn snapshot(&self) -> Vec<Event> {
        let ring = self.ring.lock().unwrap_or_else(PoisonError::into_inner);
        ring.events.iter().copied().collect()
    }

    /// How many buffered events carry `code`.
    #[must_use]
    pub fn count(&self, code: EventCode) -> usize {
        let ring = self.ring.lock().unwrap_or_else(PoisonError::into_inner);
        ring.events.iter().filter(|e| e.code == code).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_echo_threshold_is_warn_by_default_info_when_verbose_and_none_when_quiet() {
        assert_eq!(EventLog::new().echo_from, Some(Level::Warn));
        assert_eq!(
            EventLog::with_capacity(4, true).echo_from,
            Some(Level::Warn)
        );
        assert_eq!(EventLog::with_capacity(4, false).echo_from, None);
        assert_eq!(
            EventLog::with_echo(4, Some(Level::Info)).echo_from,
            Some(Level::Info)
        );
        // Every code has a level, and Info is the lowest: verbose echoes them all.
        assert!(Level::Info < Level::Warn && Level::Warn < Level::Error);
    }

    #[test]
    fn the_ring_is_bounded_and_keeps_the_newest() {
        let log = EventLog::with_capacity(3, false);
        for _ in 0..5 {
            log.record(EventCode::LaunchTokenMinted);
        }
        let seqs: Vec<u64> = log.snapshot().iter().map(|e| e.seq).collect();
        assert_eq!(seqs, [3, 4, 5]);
    }

    #[test]
    fn a_request_event_renders_as_code_method_template_status() {
        let log = EventLog::with_capacity(4, false);
        log.record_request(
            EventCode::RequestServed,
            "POST",
            "/api/v1/session/status",
            200,
        );
        assert_eq!(
            log.snapshot()[0].to_string(),
            "#1 request_served POST /api/v1/session/status 200"
        );
    }

    #[test]
    fn codes_are_unique_and_snake_case() {
        let all = [
            EventCode::RequestServed,
            EventCode::RequestRefused,
            EventCode::RequestDeadline,
            EventCode::LaunchTokenMinted,
            EventCode::SessionEstablished,
            EventCode::SessionReplaced,
            EventCode::BootstrapRejected,
            EventCode::BootstrapFailuresHigh,
            EventCode::SessionRequired,
            EventCode::SessionCookieDisplaced,
            EventCode::RelaunchAccepted,
            EventCode::RelaunchThrottled,
            EventCode::RelaunchExhausted,
            EventCode::RelaunchUnavailable,
            EventCode::ClockWentBackwards,
            EventCode::IdleTimeout,
            EventCode::EntropyUnavailable,
            EventCode::InternalError,
            EventCode::ServerStopped,
        ];
        let mut codes: Vec<&str> = all.iter().map(|c| c.code()).collect();
        assert!(codes
            .iter()
            .all(|c| c.bytes().all(|b| b.is_ascii_lowercase() || b == b'_')));
        codes.sort_unstable();
        codes.dedup();
        assert_eq!(codes.len(), all.len());
    }
}
