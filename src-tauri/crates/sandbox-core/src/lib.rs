//! Shared sandbox/PIN core.
//!
//! This crate is the single place where the PIN state-transition rules
//! (first-time set vs. replace-requires-current-PIN vs. clear-requires-
//! current-PIN, and the shared PIN length bounds) live. Both the Tauri
//! desktop app and the enterprise `vault-runtime-api` Runtime depend on this
//! one crate — there is no second copy of this logic.
//!
//! Architecture boundary
//! ----------------------
//! - Does not depend on Tauri, Warp, React, or any UI/HTTP framework.
//! - Does not own a `locked` flag or a rate limiter as part of the PIN
//!   state machine itself — those are host-specific concerns each caller
//!   wraps around [`PinController`] (see `apps/vault-runtime-api/src/sandbox.rs`
//!   for the Runtime's wrapping). The [`RateLimiter`] type in this crate is
//!   a reusable, independently-tested building block, not something
//!   [`PinController`] invokes on its own.
//! - [`Argon2FileBackend`] is the one place the `argon2` dependency is used;
//!   it never stores or logs the plaintext PIN, only a PHC-formatted
//!   Argon2id hash string.

mod argon2_backend;
mod backend;
mod rate_limit;

pub use argon2_backend::Argon2FileBackend;
pub use backend::{
    PinBackend, PinBackendError, PinController, SandboxError, MAX_PIN_LEN, MIN_PIN_LEN,
};
pub use rate_limit::{Clock, RateLimited, RateLimiter, SystemClock};
