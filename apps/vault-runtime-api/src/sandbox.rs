//! Runtime HTTP adapter for the browser sandbox/PIN (`/api/v1/sandbox*`).
//!
//! [`SandboxSession`] is the only place Runtime-specific state (`locked`
//! and the global rate limiter) wraps around the shared
//! `sandbox_core::PinController` — the PIN state-transition rules
//! themselves live entirely in `sandbox-core`, not here. This module owns
//! only HTTP routing, request/response shaping, the `locked` flag and the
//! rate-limiter wiring; it is not a second implementation of PIN
//! verification.
//!
//! This is not an authentication layer: it never gates the existing batch
//! submission, preview, query, download, restore, log or sensitive-term
//! routes. Locked/unlocked only reflects the sandbox page's own state.

use std::convert::Infallible;
use std::path::PathBuf;
use std::sync::Mutex;

use sandbox_core::{Argon2FileBackend, PinController, RateLimited, RateLimiter, SandboxError};
use service_contracts::{
    ClearSandboxPinRequest, SandboxStatusResponse, SetSandboxPinRequest, UnlockSandboxRequest,
    SANDBOX_STORAGE_MODE_SERVER_SYSTEM_USER,
};
use warp::{http::StatusCode, Filter, Rejection, Reply};

use crate::Runtime;

/// Request bodies are capped well below any real PIN (max 128 Unicode
/// scalar values, i.e. at most 512 bytes of UTF-8) so we reject oversized
/// bodies before they ever reach Argon2 (安全约束/核心设计约束 5).
const SANDBOX_BODY_LIMIT_BYTES: u64 = 4 * 1024;

/// Runtime-side wrapping around the shared [`PinController`]: owns the
/// `locked` flag (never part of the shared core, since desktop has no
/// equivalent concept) and the global verification rate limiter. There is
/// exactly one of these per Runtime process, constructed once in
/// [`crate::Runtime::build`].
pub(crate) struct SandboxSession {
    controller: PinController<Argon2FileBackend>,
    locked: Mutex<bool>,
    rate_limiter: RateLimiter,
}

enum SandboxOpError {
    NotConfigured,
    InvalidLength,
    InvalidPin,
    RateLimited(RateLimited),
    Storage,
}

impl From<SandboxError> for SandboxOpError {
    fn from(error: SandboxError) -> Self {
        match error {
            SandboxError::NotConfigured => SandboxOpError::NotConfigured,
            SandboxError::InvalidLength => SandboxOpError::InvalidLength,
            SandboxError::InvalidPin => SandboxOpError::InvalidPin,
            SandboxError::Backend(_) => SandboxOpError::Storage,
        }
    }
}

impl SandboxSession {
    /// `pin_file` is the exact path the Argon2id PHC hash string is stored
    /// at; its parent directory must already exist. The initial `locked`
    /// value follows 已确认产品语义 #2: a PIN already on disk from a
    /// previous run means the sandbox starts locked; no PIN means unlocked.
    /// Unlock state never survives a process restart by design.
    pub(crate) fn new(pin_file: PathBuf) -> Result<Self, String> {
        let controller = PinController::new(Argon2FileBackend::new(pin_file));
        let pin_configured = controller
            .has_pin()
            .map_err(|e| format!("sandbox PIN storage init failed: {e}"))?;
        Ok(Self {
            controller,
            locked: Mutex::new(pin_configured),
            rate_limiter: RateLimiter::with_system_clock(),
        })
    }

    /// Test-only: same as [`Self::new`] but with an injectable [`RateLimiter`]
    /// (typically backed by a fake clock) so HTTP-layer rate-limit tests
    /// (threshold, block, recovery) never need a real multi-minute wait.
    /// Gated by `#[cfg(test)]` so it cannot be reached from production code.
    #[cfg(test)]
    pub(crate) fn new_with_rate_limiter(
        pin_file: PathBuf,
        rate_limiter: RateLimiter,
    ) -> Result<Self, String> {
        let controller = PinController::new(Argon2FileBackend::new(pin_file));
        let pin_configured = controller
            .has_pin()
            .map_err(|e| format!("sandbox PIN storage init failed: {e}"))?;
        Ok(Self {
            controller,
            locked: Mutex::new(pin_configured),
            rate_limiter,
        })
    }

    fn locked(&self) -> bool {
        *self
            .locked
            .lock()
            .expect("sandbox locked-state mutex poisoned")
    }

    fn set_locked(&self, value: bool) {
        *self
            .locked
            .lock()
            .expect("sandbox locked-state mutex poisoned") = value;
    }

    fn rate_limit_status(&self) -> (bool, Option<u64>) {
        match self.rate_limiter.check() {
            Ok(()) => (false, None),
            Err(RateLimited { retry_after }) => (true, Some(retry_after.as_secs().max(1))),
        }
    }

    fn status(&self) -> Result<SandboxStatusResponse, SandboxOpError> {
        let pin_configured = self.controller.has_pin()?;
        let (rate_limited, retry_after_seconds) = self.rate_limit_status();
        Ok(SandboxStatusResponse {
            pin_configured,
            locked: self.locked(),
            storage_mode: SANDBOX_STORAGE_MODE_SERVER_SYSTEM_USER.to_string(),
            rate_limited,
            retry_after_seconds,
        })
    }

    /// `PUT /api/v1/sandbox/pin`. First-time set (`current` is `None`)
    /// never touches the rate limiter — there is no secret being guessed
    /// yet. Replacing an existing PIN counts against the same global
    /// limiter as unlock/clear, since it also requires verifying a current
    /// PIN. On success the sandbox becomes locked (已确认产品语义 #3).
    fn set_pin(
        &self,
        current: Option<&str>,
        new_pin: &str,
    ) -> Result<SandboxStatusResponse, SandboxOpError> {
        let already_configured = self.controller.has_pin()?;
        if already_configured {
            let (rate_limited, retry_after_seconds) = self.rate_limit_status();
            if rate_limited {
                return Err(SandboxOpError::RateLimited(RateLimited {
                    retry_after: std::time::Duration::from_secs(retry_after_seconds.unwrap_or(1)),
                }));
            }
        }
        match self.controller.set_pin(current, new_pin) {
            Ok(()) => {
                if already_configured {
                    self.rate_limiter.record_success();
                }
                self.set_locked(true);
                self.status()
            }
            Err(SandboxError::InvalidPin) => {
                self.rate_limiter.record_failure();
                Err(SandboxOpError::InvalidPin)
            }
            Err(other) => Err(other.into()),
        }
    }

    /// `POST /api/v1/sandbox/lock`. Requires a configured PIN — locking a
    /// sandbox with no PIN would be a state nothing could ever unlock.
    fn lock(&self) -> Result<SandboxStatusResponse, SandboxOpError> {
        if !self.controller.has_pin()? {
            return Err(SandboxOpError::NotConfigured);
        }
        self.set_locked(true);
        self.status()
    }

    /// `POST /api/v1/sandbox/unlock`. Rate-limited: the limiter is checked
    /// *before* any Argon2 verification runs, so a blocked caller never
    /// causes a hash computation.
    fn unlock(&self, pin: &str) -> Result<SandboxStatusResponse, SandboxOpError> {
        let (rate_limited, retry_after_seconds) = self.rate_limit_status();
        if rate_limited {
            return Err(SandboxOpError::RateLimited(RateLimited {
                retry_after: std::time::Duration::from_secs(retry_after_seconds.unwrap_or(1)),
            }));
        }
        match self.controller.verify_pin(pin) {
            Ok(true) => {
                self.rate_limiter.record_success();
                self.set_locked(false);
                self.status()
            }
            Ok(false) => {
                self.rate_limiter.record_failure();
                Err(SandboxOpError::InvalidPin)
            }
            Err(SandboxError::InvalidLength) => {
                // A wrong-length guess against a configured PIN is still a
                // failed verification attempt for rate-limiting purposes;
                // exposed to the caller as the same generic "invalid PIN"
                // as a content mismatch, so length bounds are not a usable
                // oracle.
                self.rate_limiter.record_failure();
                Err(SandboxOpError::InvalidPin)
            }
            Err(other) => Err(other.into()),
        }
    }

    /// `DELETE /api/v1/sandbox/pin`. Same rate-limit treatment as unlock.
    fn clear_pin(&self, current: &str) -> Result<SandboxStatusResponse, SandboxOpError> {
        let (rate_limited, retry_after_seconds) = self.rate_limit_status();
        if rate_limited {
            return Err(SandboxOpError::RateLimited(RateLimited {
                retry_after: std::time::Duration::from_secs(retry_after_seconds.unwrap_or(1)),
            }));
        }
        match self.controller.clear_pin(current) {
            Ok(()) => {
                self.rate_limiter.record_success();
                self.set_locked(false);
                self.status()
            }
            Err(SandboxError::InvalidPin) => {
                self.rate_limiter.record_failure();
                Err(SandboxOpError::InvalidPin)
            }
            Err(SandboxError::InvalidLength) => {
                self.rate_limiter.record_failure();
                Err(SandboxOpError::InvalidPin)
            }
            Err(other) => Err(other.into()),
        }
    }
}

pub fn routes<F>(
    runtime_filter: F,
) -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone
where
    F: Filter<Extract = (Runtime,), Error = Infallible> + Clone + Send + Sync + 'static,
{
    let status = warp::path!("api" / "v1" / "sandbox" / "status")
        .and(warp::get())
        .and(runtime_filter.clone())
        .and_then(status_handler);

    let set_pin = warp::path!("api" / "v1" / "sandbox" / "pin")
        .and(warp::put())
        .and(warp::body::content_length_limit(SANDBOX_BODY_LIMIT_BYTES))
        .and(warp::body::json())
        .and(runtime_filter.clone())
        .and_then(set_pin_handler);

    let lock = warp::path!("api" / "v1" / "sandbox" / "lock")
        .and(warp::post())
        .and(runtime_filter.clone())
        .and_then(lock_handler);

    let unlock = warp::path!("api" / "v1" / "sandbox" / "unlock")
        .and(warp::post())
        .and(warp::body::content_length_limit(SANDBOX_BODY_LIMIT_BYTES))
        .and(warp::body::json())
        .and(runtime_filter.clone())
        .and_then(unlock_handler);

    let clear_pin = warp::path!("api" / "v1" / "sandbox" / "pin")
        .and(warp::delete())
        .and(warp::body::content_length_limit(SANDBOX_BODY_LIMIT_BYTES))
        .and(warp::body::json())
        .and(runtime_filter)
        .and_then(clear_pin_handler);

    status.or(set_pin).or(lock).or(unlock).or(clear_pin)
}

async fn status_handler(runtime: Runtime) -> Result<impl Reply, Rejection> {
    let status = runtime.sandbox.status().map_err(sandbox_rejection)?;
    Ok(warp::reply::json(&status))
}

async fn set_pin_handler(
    body: SetSandboxPinRequest,
    runtime: Runtime,
) -> Result<impl Reply, Rejection> {
    let status = runtime
        .sandbox
        .set_pin(body.current_pin.as_deref(), &body.new_pin)
        .map_err(sandbox_rejection)?;
    Ok(warp::reply::json(&status))
}

async fn lock_handler(runtime: Runtime) -> Result<impl Reply, Rejection> {
    let status = runtime.sandbox.lock().map_err(sandbox_rejection)?;
    Ok(warp::reply::json(&status))
}

async fn unlock_handler(
    body: UnlockSandboxRequest,
    runtime: Runtime,
) -> Result<impl Reply, Rejection> {
    let status = runtime
        .sandbox
        .unlock(&body.pin)
        .map_err(sandbox_rejection)?;
    Ok(warp::reply::json(&status))
}

async fn clear_pin_handler(
    body: ClearSandboxPinRequest,
    runtime: Runtime,
) -> Result<impl Reply, Rejection> {
    let status = runtime
        .sandbox
        .clear_pin(&body.current_pin)
        .map_err(sandbox_rejection)?;
    Ok(warp::reply::json(&status))
}

fn sandbox_rejection(error: SandboxOpError) -> Rejection {
    match error {
        SandboxOpError::NotConfigured => crate::api_error(
            StatusCode::CONFLICT,
            "SANDBOX_PIN_NOT_CONFIGURED",
            "Sandbox PIN is not configured",
            false,
        ),
        SandboxOpError::InvalidLength => crate::api_error(
            StatusCode::BAD_REQUEST,
            "SANDBOX_PIN_INVALID",
            "Sandbox PIN length is out of bounds",
            false,
        ),
        SandboxOpError::InvalidPin => crate::api_error(
            StatusCode::UNAUTHORIZED,
            "SANDBOX_PIN_INVALID",
            "Sandbox PIN did not match",
            false,
        ),
        SandboxOpError::RateLimited(RateLimited { retry_after }) => {
            warp::reject::custom(SandboxRateLimited {
                retry_after_seconds: retry_after.as_secs().max(1),
            })
        }
        SandboxOpError::Storage => crate::api_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "SANDBOX_PIN_STORAGE_FAILED",
            "Sandbox PIN storage operation failed",
            true,
        ),
    }
}

/// Distinct rejection type (rather than [`crate::ApiError`]) purely so the
/// crate-wide rejection handler can attach the safe `Retry-After` header
/// HTTP契约 requires, without adding an optional header field to every
/// other error path in the Runtime.
#[derive(Debug)]
pub(crate) struct SandboxRateLimited {
    pub(crate) retry_after_seconds: u64,
}

impl warp::reject::Reject for SandboxRateLimited {}
