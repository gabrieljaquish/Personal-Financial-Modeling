//! The local web server: TLS-only listener on loopback, session establishment,
//! request admission, the JSON API under `/api/v1` and the embedded web assets
//! (`ARCHITECTURE.md` §5, `SECURITY.md` §6–§7).
//!
//! This crate is the only workspace member that opens sockets. Nothing here is
//! part of the engine: the engine crates are pure and this crate drives them.
//!
//! * [`server`] — the public surface: [`ServerConfig`] → [`Server::bind`] →
//!   [`BoundServer`];
//! * [`admission`] — `Host` / `Origin` / `Sec-Fetch-Site` request admission;
//! * [`session`] — launch token → `__Host-` cookie + proof token;
//! * [`headers`] and [`csp`] — the exact response-header set;
//! * [`api`] — the JSON API, its DTOs and the `OpenAPI` document;
//! * [`assets`] — the web shell and its content-hashed assets, from memory;
//! * [`events`] and [`error`] — logs and error bodies that carry no content;
//! * [`tls`] — the certificate scheme (a name-constrained local CA whose private key
//!   is discarded after it signs the one leaf), the `rustls` configuration and the
//!   leaf fingerprint;
//! * [`bind`] and [`accept`] — the `127.0.0.1`-only, TLS-only listener;
//! * [`linger`] — the bounded graceful close after a response that did not wait for
//!   its request body;
//! * [`trust`] — the trust-installation seam (a trait; no platform code lives here);
//! * [`redact`] — [`Redacted`], the wrapper that keeps a value out of every log.
//!
//! This crate has no filesystem access: key material is handed to it as values and
//! persisted, if at all, by the launcher (`SECURITY.md` §11).

// Not an engine crate: the clock, sockets and hash maps are legitimate here.
#![forbid(unsafe_code)]
#![allow(
    clippy::disallowed_types,
    clippy::disallowed_methods,
    clippy::disallowed_macros
)]

pub mod accept;
pub mod admission;
pub mod api;
pub mod assets;
pub mod bind;
pub mod csp;
pub mod error;
pub mod events;
pub mod headers;
pub mod limits;
pub mod linger;
pub mod origin;
pub mod redact;
pub mod server;
mod service;
pub mod session;
pub mod tls;
pub mod trust;

pub use accept::{AcceptLimits, AcceptStats, HttpLimits, ListenError, TlsListener};
pub use api::openapi::openapi_json;
pub use assets::{AssetError, AssetManifest};
pub use bind::{bind_loopback, check_bound_addr, BindError, BindOutcome};
pub use events::{Event, EventCode, EventLog};
pub use linger::LingerLimits;
pub use origin::CanonicalOrigin;
pub use redact::Redacted;
pub use server::{
    BoundServer, RelaunchHook, ReopenUnavailable, Server, ServerConfig, ServerError, TestKnobs,
    TrustMode,
};
pub use session::{Clock, SessionHooks, SessionManager};
pub use tls::{
    issue, leaf_fingerprint, server_config, IssueError, LeafStore, MaterialError, MemoryLeafStore,
    StoreError, TlsMaterial,
};
pub use trust::{InstallOutcome, PlatformError, TrustStatus, TrustStore, UnsupportedTrustStore};
