//! Shared, host-agnostic FileBay/Gitea HTTP client. Depends on neither
//! Tauri, Warp, SQLx, nor any frontend code — the desktop app and the
//! Runtime HTTP server both wrap this crate instead of each maintaining
//! their own upload client.
//!
//! Design invariants enforced by this crate (not by callers):
//! - [`Endpoint::parse`] is the only way to obtain a target origin, and it
//!   rejects anything but a bare `https://host[:port]` root origin.
//! - [`Token`] never derives `Debug` or `Serialize`.
//! - [`path::validate_remote_path`] rejects traversal/control characters
//!   before any request is built.
//! - [`transport::Transport`] is injectable so hosts can swap in a fake for
//!   tests; production always uses [`transport::ReqwestTransport`], which is
//!   built with `rustls-tls` only and has no option to disable certificate
//!   verification.

pub mod client;
pub mod endpoint;
pub mod error;
pub mod identity;
pub mod path;
pub mod testing;
pub mod token;
pub mod transport;

pub use client::{FileBayClient, RepositoryTarget, UploadOutcome};
pub use endpoint::Endpoint;
pub use error::FileBayError;
pub use identity::validate_identity;
pub use path::{build_remote_path, sanitize_stem, validate_remote_path, REMOTE_PATH_PREFIX};
pub use token::Token;
pub use transport::{ReqwestTransport, Transport, TransportResponse};
