//! TLS on loopback (`SECURITY.md` §6, ADR-006).
//!
//! * [`issue`] — the certificate scheme: a name-constrained local CA whose private
//!   key is discarded inside the function that signs the one leaf.
//! * [`store`] — [`TlsMaterial`], its pure validity check, and the [`LeafStore`] seam
//!   (in-memory here; the file-backed store lives in the launcher, because this
//!   crate has no filesystem access).
//! * [`config`] — the `rustls` server configuration: `ring`, TLS 1.3 only, ALPN
//!   `http/1.1`.
//! * [`fingerprint`] — the SHA-256 leaf fingerprint shown in the status console.

pub mod config;
pub mod fingerprint;
pub mod issue;
pub mod store;

pub use config::{server_config, TlsConfigError, ALPN_HTTP_1_1};
pub use fingerprint::leaf_fingerprint;
pub use issue::{issue, IssueError};
pub use store::{LeafStore, MaterialError, MemoryLeafStore, StoreError, TlsMaterial};
