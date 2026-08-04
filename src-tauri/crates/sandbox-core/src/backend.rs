use std::fmt;

/// Error returned by a [`PinBackend`] implementation.
///
/// The message must be safe to log or return in an API response: it must
/// never contain the PIN, a password hash, a salt, or a filesystem path.
/// Implementations should only ever put a short, storage-agnostic category
/// (e.g. an `io::ErrorKind` name) in here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PinBackendError(pub String);

impl fmt::Display for PinBackendError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "sandbox PIN storage failed: {}", self.0)
    }
}

impl std::error::Error for PinBackendError {}

/// A pluggable place to durably store a single PIN. Implementations own the
/// actual persistence mechanism (OS Keychain, DPAPI, or an Argon2id hash
/// file) and are the only place a [`PinBackendError`] can carry a
/// storage-specific detail; [`PinController`] never sees or logs raw PINs,
/// hashes, or paths.
pub trait PinBackend: Send + Sync {
    /// Whether a PIN is currently configured.
    fn has_pin(&self) -> Result<bool, PinBackendError>;

    /// Store `new_pin` as the current PIN, unconditionally overwriting
    /// anything previously stored. Callers (see [`PinController`]) are
    /// responsible for verifying any existing PIN *before* calling this —
    /// the backend itself does not gate on a "current" value.
    fn save_pin(&self, new_pin: &str) -> Result<(), PinBackendError>;

    /// Whether `pin` matches the currently stored PIN. Returns `Ok(false)`
    /// (not an error) for a plain mismatch; returns `Err` only for a real
    /// storage/read failure. Callers must check [`Self::has_pin`] first —
    /// implementations may error if called with no PIN configured.
    fn verify_pin(&self, pin: &str) -> Result<bool, PinBackendError>;

    /// Remove the stored PIN. Idempotent: succeeds even if no PIN exists.
    fn clear_pin(&self) -> Result<(), PinBackendError>;
}

/// PIN length bounds shared by every host (Unicode scalar count, not bytes).
pub const MIN_PIN_LEN: usize = 4;
pub const MAX_PIN_LEN: usize = 128;

/// Errors from [`PinController`]'s unified state-transition rules. Every
/// variant is safe to log or serialize as-is: none of them can carry a PIN,
/// hash, salt, or filesystem path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SandboxError {
    /// No PIN is configured; the requested operation requires one.
    NotConfigured,
    /// A submitted new PIN does not satisfy the shared length bounds
    /// (`MIN_PIN_LEN..=MAX_PIN_LEN` Unicode scalar values).
    InvalidLength,
    /// The submitted current/verification PIN did not match.
    InvalidPin,
    /// The backend failed to read or write durable storage.
    Backend(PinBackendError),
}

impl fmt::Display for SandboxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SandboxError::NotConfigured => write!(f, "sandbox PIN is not configured"),
            SandboxError::InvalidLength => write!(f, "sandbox PIN length is out of bounds"),
            SandboxError::InvalidPin => write!(f, "sandbox PIN did not match"),
            SandboxError::Backend(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for SandboxError {}

impl From<PinBackendError> for SandboxError {
    fn from(e: PinBackendError) -> Self {
        SandboxError::Backend(e)
    }
}

fn validate_pin_length(pin: &str) -> Result<(), SandboxError> {
    let len = pin.chars().count();
    if len < MIN_PIN_LEN || len > MAX_PIN_LEN {
        return Err(SandboxError::InvalidLength);
    }
    Ok(())
}

/// The single, host-agnostic PIN state-transition core shared by the Tauri
/// desktop app and the enterprise Runtime. It owns the "does this operation
/// require verifying the current PIN first" business rule and the shared
/// PIN length bounds; it deliberately does **not** own a `locked` flag, a
/// rate limiter, or any HTTP/Tauri-specific concept — those are host-specific
/// wrapping left to each caller (see `apps/vault-runtime-api/src/sandbox.rs`
/// for the Runtime's `locked` + rate-limit wrapping around this type).
pub struct PinController<B: PinBackend> {
    backend: B,
}

impl<B: PinBackend> PinController<B> {
    pub fn new(backend: B) -> Self {
        Self { backend }
    }

    pub fn has_pin(&self) -> Result<bool, SandboxError> {
        Ok(self.backend.has_pin()?)
    }

    /// Set a new PIN. When no PIN is currently configured, `current` must be
    /// `None` (first-time set never requires a current PIN). When a PIN is
    /// already configured, `current` must be `Some` and must verify —
    /// otherwise the existing PIN is left untouched and `InvalidPin` (or
    /// `NotConfigured`, if `current` was omitted) is returned.
    pub fn set_pin(&self, current: Option<&str>, new_pin: &str) -> Result<(), SandboxError> {
        validate_pin_length(new_pin)?;
        if self.backend.has_pin()? {
            let current = current.ok_or(SandboxError::NotConfigured)?;
            if !self.backend.verify_pin(current)? {
                return Err(SandboxError::InvalidPin);
            }
        }
        self.backend.save_pin(new_pin)?;
        Ok(())
    }

    /// Verify `pin` against the currently stored PIN. Returns
    /// [`SandboxError::NotConfigured`] if no PIN exists yet, so callers
    /// never have to special-case "no PIN" as a plain mismatch.
    pub fn verify_pin(&self, pin: &str) -> Result<bool, SandboxError> {
        validate_pin_length(pin)?;
        if !self.backend.has_pin()? {
            return Err(SandboxError::NotConfigured);
        }
        Ok(self.backend.verify_pin(pin)?)
    }

    /// Remove the current PIN. Requires verifying `current` first; the PIN
    /// is left untouched if verification fails.
    pub fn clear_pin(&self, current: &str) -> Result<(), SandboxError> {
        if !self.backend.has_pin()? {
            return Err(SandboxError::NotConfigured);
        }
        if !self.backend.verify_pin(current)? {
            return Err(SandboxError::InvalidPin);
        }
        self.backend.clear_pin()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// In-memory [`PinBackend`] for exercising [`PinController`]'s
    /// state-transition rules without touching any real storage.
    struct MemoryBackend {
        pin: Mutex<Option<String>>,
    }

    impl MemoryBackend {
        fn empty() -> Self {
            Self {
                pin: Mutex::new(None),
            }
        }
    }

    impl PinBackend for MemoryBackend {
        fn has_pin(&self) -> Result<bool, PinBackendError> {
            Ok(self.pin.lock().unwrap().is_some())
        }
        fn save_pin(&self, new_pin: &str) -> Result<(), PinBackendError> {
            *self.pin.lock().unwrap() = Some(new_pin.to_string());
            Ok(())
        }
        fn verify_pin(&self, pin: &str) -> Result<bool, PinBackendError> {
            Ok(self.pin.lock().unwrap().as_deref() == Some(pin))
        }
        fn clear_pin(&self) -> Result<(), PinBackendError> {
            *self.pin.lock().unwrap() = None;
            Ok(())
        }
    }

    #[test]
    fn first_time_set_does_not_require_current_pin() {
        let controller = PinController::new(MemoryBackend::empty());
        assert!(!controller.has_pin().unwrap());
        controller.set_pin(None, "1234").unwrap();
        assert!(controller.has_pin().unwrap());
        assert!(controller.verify_pin("1234").unwrap());
    }

    #[test]
    fn replace_without_current_pin_is_rejected_and_old_pin_survives() {
        let controller = PinController::new(MemoryBackend::empty());
        controller.set_pin(None, "1234").unwrap();
        let err = controller.set_pin(None, "5678").unwrap_err();
        assert_eq!(err, SandboxError::NotConfigured);
        assert!(controller.verify_pin("1234").unwrap());
        assert!(!controller.verify_pin("5678").unwrap());
    }

    #[test]
    fn replace_with_wrong_current_pin_is_rejected_and_old_pin_survives() {
        let controller = PinController::new(MemoryBackend::empty());
        controller.set_pin(None, "1234").unwrap();
        let err = controller.set_pin(Some("0000"), "5678").unwrap_err();
        assert_eq!(err, SandboxError::InvalidPin);
        assert!(controller.verify_pin("1234").unwrap());
    }

    #[test]
    fn replace_with_correct_current_pin_succeeds() {
        let controller = PinController::new(MemoryBackend::empty());
        controller.set_pin(None, "1234").unwrap();
        controller.set_pin(Some("1234"), "5678").unwrap();
        assert!(!controller.verify_pin("1234").unwrap());
        assert!(controller.verify_pin("5678").unwrap());
    }

    #[test]
    fn clear_requires_verifying_current_pin() {
        let controller = PinController::new(MemoryBackend::empty());
        controller.set_pin(None, "1234").unwrap();
        assert_eq!(
            controller.clear_pin("0000").unwrap_err(),
            SandboxError::InvalidPin
        );
        assert!(controller.has_pin().unwrap());
        controller.clear_pin("1234").unwrap();
        assert!(!controller.has_pin().unwrap());
    }

    #[test]
    fn clear_without_any_pin_is_not_configured() {
        let controller = PinController::new(MemoryBackend::empty());
        assert_eq!(
            controller.clear_pin("anything").unwrap_err(),
            SandboxError::NotConfigured
        );
    }

    #[test]
    fn verify_without_any_pin_is_not_configured() {
        let controller = PinController::new(MemoryBackend::empty());
        assert_eq!(
            controller.verify_pin("1234").unwrap_err(),
            SandboxError::NotConfigured
        );
    }

    #[test]
    fn pin_length_bounds_are_enforced_by_unicode_char_count_not_bytes() {
        let controller = PinController::new(MemoryBackend::empty());
        assert_eq!(
            controller.set_pin(None, "123").unwrap_err(),
            SandboxError::InvalidLength
        );
        assert_eq!(
            controller.set_pin(None, &"1".repeat(129)).unwrap_err(),
            SandboxError::InvalidLength
        );
        // A 4-character Chinese PIN is 12 bytes but exactly 4 Unicode
        // scalar values — must be accepted, not rejected as too short.
        controller.set_pin(None, "沙箱密钥").unwrap();
        assert!(controller.verify_pin("沙箱密钥").unwrap());
    }

    #[test]
    fn wrong_length_verify_pin_is_invalid_length_not_a_backend_call() {
        let controller = PinController::new(MemoryBackend::empty());
        controller.set_pin(None, "1234").unwrap();
        assert_eq!(
            controller.verify_pin("ab").unwrap_err(),
            SandboxError::InvalidLength
        );
    }
}
