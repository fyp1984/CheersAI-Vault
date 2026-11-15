use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use argon2::password_hash::{
    rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString,
};
use argon2::Argon2;

use crate::backend::{PinBackend, PinBackendError};

/// Argon2id-backed [`PinBackend`] for hosts (the enterprise Runtime) that
/// have no OS-level secure credential store to delegate to. Only ever
/// stores a PHC-formatted Argon2id hash string on disk — never the PIN
/// itself, never a reversible encoding. Salts are generated per-write with
/// the OS CSPRNG (`OsRng`); nothing is ever fixed or reused.
///
/// All writes go through an internal mutex and a temp-file-then-`rename`
/// atomic replace, so concurrent callers can never observe or produce a
/// torn/partial state file.
pub struct Argon2FileBackend {
    path: PathBuf,
    write_lock: Mutex<()>,
}

impl Argon2FileBackend {
    /// `path` is the exact file the PHC hash string is stored in. The
    /// parent directory must already exist; this does not create it — the
    /// caller owns the Runtime data-root layout.
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            write_lock: Mutex::new(()),
        }
    }

    fn read_phc(&self) -> Result<Option<String>, PinBackendError> {
        match fs::read_to_string(&self.path) {
            Ok(contents) => {
                let trimmed = contents.trim();
                if trimmed.is_empty() {
                    Ok(None)
                } else {
                    Ok(Some(trimmed.to_string()))
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(io_err(e)),
        }
    }
}

impl PinBackend for Argon2FileBackend {
    fn has_pin(&self) -> Result<bool, PinBackendError> {
        Ok(self.read_phc()?.is_some())
    }

    fn save_pin(&self, new_pin: &str) -> Result<(), PinBackendError> {
        let salt = SaltString::generate(&mut OsRng);
        let phc = Argon2::default()
            .hash_password(new_pin.as_bytes(), &salt)
            .map_err(|_| PinBackendError("hash_failed".into()))?
            .to_string();
        let _guard = self
            .write_lock
            .lock()
            .expect("sandbox PIN write lock poisoned");
        atomic_write_0600(&self.path, phc.as_bytes())
    }

    fn verify_pin(&self, pin: &str) -> Result<bool, PinBackendError> {
        let phc = match self.read_phc()? {
            Some(p) => p,
            None => return Err(PinBackendError("not_configured".into())),
        };
        let parsed =
            PasswordHash::new(&phc).map_err(|_| PinBackendError("corrupt_state_file".into()))?;
        Ok(Argon2::default()
            .verify_password(pin.as_bytes(), &parsed)
            .is_ok())
    }

    fn clear_pin(&self) -> Result<(), PinBackendError> {
        let _guard = self
            .write_lock
            .lock()
            .expect("sandbox PIN write lock poisoned");
        match fs::remove_file(&self.path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(io_err(e)),
        }
    }
}

fn io_err(e: std::io::Error) -> PinBackendError {
    PinBackendError(e.kind().to_string())
}

/// Write `contents` to `path` via a temp-file-then-`rename` atomic replace.
/// On Unix the temp file is created with mode `0600` directly (never wider,
/// regardless of umask, since umask can only narrow the requested mode) so
/// there is no window where the data is readable by anyone but the owner;
/// `rename` preserves that mode on the final path. Any failure removes the
/// temp file and leaves whatever was previously at `path` (if anything)
/// untouched — this never leaves a half-written file at `path` itself.
fn atomic_write_0600(path: &Path, contents: &[u8]) -> Result<(), PinBackendError> {
    let tmp_path = path.with_extension(format!("tmp-{}", std::process::id()));

    let result = (|| -> std::io::Result<()> {
        let mut open_options = fs::OpenOptions::new();
        open_options.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            open_options.mode(0o600);
        }
        let mut file = open_options.open(&tmp_path)?;
        file.write_all(contents)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&tmp_path, path)?;
        Ok(())
    })();

    if let Err(e) = result {
        let _ = fs::remove_file(&tmp_path);
        return Err(io_err(e));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::PinController;

    fn backend_at(dir: &Path) -> Argon2FileBackend {
        Argon2FileBackend::new(dir.join("sandbox-pin.phc"))
    }

    #[test]
    fn stored_file_never_contains_the_plaintext_pin() {
        let dir = tempfile::tempdir().unwrap();
        let backend = backend_at(dir.path());
        backend.save_pin("correct-horse-battery-staple").unwrap();
        let raw = fs::read_to_string(dir.path().join("sandbox-pin.phc")).unwrap();
        assert!(!raw.contains("correct-horse-battery-staple"));
        assert!(raw.starts_with("$argon2id$"));
    }

    #[test]
    fn wrong_pin_does_not_verify() {
        let dir = tempfile::tempdir().unwrap();
        let backend = backend_at(dir.path());
        backend.save_pin("1234").unwrap();
        assert!(!backend.verify_pin("0000").unwrap());
        assert!(backend.verify_pin("1234").unwrap());
    }

    #[test]
    fn two_saves_use_different_random_salts() {
        let dir = tempfile::tempdir().unwrap();
        let backend = backend_at(dir.path());
        backend.save_pin("1234").unwrap();
        let first = fs::read_to_string(dir.path().join("sandbox-pin.phc")).unwrap();
        backend.save_pin("1234").unwrap();
        let second = fs::read_to_string(dir.path().join("sandbox-pin.phc")).unwrap();
        assert_ne!(
            first, second,
            "same PIN must hash to a different PHC string each time (random salt)"
        );
    }

    #[test]
    #[cfg(unix)]
    fn file_permissions_are_0600_after_create_and_after_replace() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let backend = backend_at(dir.path());
        let path = dir.path().join("sandbox-pin.phc");

        backend.save_pin("1234").unwrap();
        let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "newly created state file must be 0600");

        backend.save_pin("5678").unwrap();
        let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "replaced state file must still be 0600");
    }

    #[test]
    fn clear_pin_removes_the_file_and_is_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let backend = backend_at(dir.path());
        backend.save_pin("1234").unwrap();
        assert!(backend.has_pin().unwrap());
        backend.clear_pin().unwrap();
        assert!(!backend.has_pin().unwrap());
        // Clearing again (nothing left) must not error.
        backend.clear_pin().unwrap();
    }

    #[test]
    fn corrupted_state_file_fails_closed_on_verify() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sandbox-pin.phc");
        fs::write(&path, b"not a valid PHC string").unwrap();
        let backend = Argon2FileBackend::new(path);
        let err = backend.verify_pin("1234").unwrap_err();
        assert_eq!(err, PinBackendError("corrupt_state_file".into()));
    }

    #[test]
    fn missing_state_file_never_produces_a_reachable_ready_state() {
        let dir = tempfile::tempdir().unwrap();
        let backend = backend_at(dir.path());
        assert!(!backend.has_pin().unwrap());
        let err = backend.verify_pin("anything").unwrap_err();
        assert_eq!(err, PinBackendError("not_configured".into()));
    }

    #[test]
    fn restart_reads_back_the_same_pin_from_a_fresh_backend_instance() {
        let dir = tempfile::tempdir().unwrap();
        {
            let backend = backend_at(dir.path());
            backend.save_pin("1234").unwrap();
        }
        // Simulate a Runtime restart: a brand new backend instance pointed
        // at the same path must still verify the previously-set PIN.
        let restarted = backend_at(dir.path());
        assert!(restarted.has_pin().unwrap());
        assert!(restarted.verify_pin("1234").unwrap());
    }

    #[test]
    fn concurrent_set_pin_calls_never_produce_a_torn_or_unparseable_file() {
        use std::sync::Arc;
        use std::thread;

        let dir = tempfile::tempdir().unwrap();
        let backend = Arc::new(backend_at(dir.path()));
        backend.save_pin("initial").unwrap();

        let mut handles = Vec::new();
        for i in 0..8 {
            let backend = Arc::clone(&backend);
            handles.push(thread::spawn(move || {
                backend.save_pin(&format!("candidate-{i}")).unwrap();
            }));
        }
        for h in handles {
            h.join().unwrap();
        }

        // Whichever write landed last, the file must be a single complete,
        // parseable PHC string — never a mix of two concurrent writers.
        let raw = fs::read_to_string(dir.path().join("sandbox-pin.phc")).unwrap();
        assert!(
            PasswordHash::new(raw.trim()).is_ok(),
            "state file must remain a valid PHC string after concurrent writes"
        );
    }

    #[test]
    fn shared_pin_controller_end_to_end_with_the_argon2_backend() {
        let dir = tempfile::tempdir().unwrap();
        let controller = PinController::new(backend_at(dir.path()));
        controller.set_pin(None, "1234").unwrap();
        assert!(controller.verify_pin("1234").unwrap());
        controller.set_pin(Some("1234"), "5678").unwrap();
        assert!(!controller.verify_pin("1234").unwrap());
        assert!(controller.verify_pin("5678").unwrap());
        controller.clear_pin("5678").unwrap();
        assert!(!controller.has_pin().unwrap());
    }
}
