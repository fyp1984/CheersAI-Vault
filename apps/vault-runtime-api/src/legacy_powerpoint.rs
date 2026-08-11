//! Legacy PowerPoint (.ppt) to .pptx converter.
//!
//! Architecture boundary
//! ---------------------
//! This module lives in the Runtime layer, NOT in engine-core.
//! engine-core must never know about LibreOffice, temp directories,
//! or system application paths. All of that is handled here.
//!
//! LibreOffice path resolution order:
//!   1. `CHEERSAI_LIBREOFFICE_PATH` environment variable (explicit CI / production override)
//!      — must point to a runnable `soffice` binary
//!   2. Standard macOS `/Applications/LibreOffice.app/...` install
//!   3. Common Homebrew paths
//!   4. `PATH` lookup via `which soffice`
//!   5. Portable path from the feasibility test: `/tmp/ppt-conversion-feasibility/...`
//!      (last resort, only used if no other candidate is available)
//!
//! Every candidate is verified by running `soffice --version` before being
//! cached.  A candidate that exists on disk but fails to start (missing deps,
//! broken installation) is silently skipped and the next candidate is tried.
//! Only a verified-available path is POSITIVELY cached.  A failed resolution
//! is NOT cached — the next call re-tries from scratch.
//!
//! Error classification (callers see only safe codes, never paths or stderr):
//! - `InputCorrupted`        – input is provably corrupt: empty, cannot be
//!                            opened as a valid OLE2/CFB compound file, or
//!                            has zero streams.
//! - `Internal`              – converter ran but its process failed (non-zero
//!                            exit, missing output, or output not valid PPTX).
//! - `ConverterUnavailable`  – no candidate found or spawn failed.
//! - `ProcessingTimeout`     – conversion exceeded the deadline.
//! - `ResourceLimitExceeded` – input or output exceeds size limits.
//! Safe messages never expose command lines, absolute paths, or temp dirs.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::OnceLock;
use std::time::Duration;
use tokio::process::Command;
use tokio::time::timeout;

use engine_core::AppError;

// --------------- constants ---------------

const OLE2_SIG: &[u8] = &[0xd0, 0xcf, 0x11, 0xe0];

const MAX_PPT_BYTES: usize = 128 * 1024 * 1024; // 128 MB
const MAX_OUTPUT_BYTES: u64 = 256 * 1024 * 1024; // 256 MB
const DEFAULT_CONVERT_TIMEOUT: Duration = Duration::from_secs(120);

/// Well-known candidate paths checked on macOS when `CHEERSAI_LIBREOFFICE_PATH`
/// is not set and no cached path exists yet.
///
/// These are standard install locations ONLY.  The `/tmp` portable / CI
/// fallback path is checked as a separate absolute-last step after PATH
/// lookup (see `find_soffice`).
const MACOS_CANDIDATES: &[&str] = &[
    // Standard macOS /Applications
    "/Applications/LibreOffice.app/Contents/MacOS/soffice",
    // Homebrew cask
    "/opt/homebrew/bin/soffice",
    "/usr/local/bin/soffice",
];

/// Last-resort portable / CI fallback path.
/// Used only when all other candidates (env, standard installs, PATH) fail.
const PORTABLE_TMP_CANDIDATE: &str =
    "/tmp/ppt-conversion-feasibility/libreoffice-portable/LibreOffice.app/Contents/MacOS/soffice";

// --------------- public types ---------------

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConvertError {
    ConverterUnavailable,
    InputCorrupted,
    InputEncrypted,
    ProcessingTimeout,
    ResourceLimitExceeded,
    Internal,
}

impl ConvertError {
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn error_code(self) -> &'static str {
        match self {
            Self::ConverterUnavailable => "LEGACY_CONVERTER_UNAVAILABLE",
            Self::InputCorrupted => "INPUT_CORRUPTED",
            Self::InputEncrypted => "INPUT_ENCRYPTED",
            Self::ProcessingTimeout => "PROCESSING_TIMEOUT",
            Self::ResourceLimitExceeded => "RESOURCE_LIMIT_EXCEEDED",
            Self::Internal => "CONVERSION_INTERNAL_ERROR",
        }
    }

    pub fn to_app_error(self) -> AppError {
        let (code, msg) = match self {
            Self::ConverterUnavailable => (
                "LEGACY_CONVERTER_UNAVAILABLE",
                "Legacy PowerPoint converter is not available",
            ),
            Self::InputCorrupted => (
                "INPUT_CORRUPTED",
                "PowerPoint 97-2003 input is corrupted or invalid",
            ),
            Self::InputEncrypted => (
                "INPUT_ENCRYPTED",
                "Encrypted PowerPoint files are not supported",
            ),
            Self::ProcessingTimeout => ("PROCESSING_TIMEOUT", "PowerPoint conversion timed out"),
            Self::ResourceLimitExceeded => (
                "RESOURCE_LIMIT_EXCEEDED",
                "PowerPoint input exceeds the size limit",
            ),
            Self::Internal => (
                "CONVERSION_INTERNAL_ERROR",
                "Legacy PowerPoint conversion failed",
            ),
        };
        AppError {
            code: code.into(),
            message: msg.into(),
            retryable: matches!(self, Self::ProcessingTimeout | Self::Internal),
            safe_details: None,
        }
    }
}

// --------------- path resolution ---------------

/// Injectable resolver.  The cache ONLY holds successful paths —
/// a failed resolution leaves the cache empty so the next call retries.
/// No `Box::leak` or `mem::forget` is used; references come from the cache.
pub struct SofficeResolver {
    cache: OnceLock<PathBuf>,
}

impl SofficeResolver {
    pub const fn new() -> Self {
        Self {
            cache: OnceLock::new(),
        }
    }

    /// Resolve a soffice path, recording each probe attempt into `log`.
    ///
    /// Fast path (cache hit): returns the cached path immediately without
    /// calling env/PATH sources or the availability probe.
    ///
    /// Slow path (cache miss): walks env → candidates → PATH → tmp, caches
    /// the first success.  Failures are NOT cached — the next call retries.
    pub fn resolve(
        &self,
        env_path: Option<&str>,
        candidates: &[&str],
        path_result: Option<&str>,
        tmp: &str,
        available: &dyn Fn(&Path) -> bool,
        log: &mut Vec<String>,
    ) -> Option<&Path> {
        // Fast path: cache hit — zero calls to sources/probes
        if let Some(path) = self.cache.get() {
            return Some(path.as_path());
        }
        // Slow path: search
        if let Some(found) = Self::search(env_path, candidates, path_result, tmp, available, log) {
            // Cache the success (first writer wins on race)
            let _ = self.cache.set(found);
        }
        // A concurrent search may have populated the cache while this search ran.
        self.cache.get().map(|p| p.as_path())
    }

    /// Non-caching probe: walk the ordered chain and return the first
    /// available candidate.  Logs each attempt to `log`.
    ///
    /// The `available` callback is the single source of truth — it should
    /// check existence AND runnability.  Production uses `soffice_is_available`.
    pub fn search(
        env_path: Option<&str>,
        candidates: &[&str],
        path_result: Option<&str>,
        tmp: &str,
        available: &dyn Fn(&Path) -> bool,
        log: &mut Vec<String>,
    ) -> Option<PathBuf> {
        // 1. Environment variable
        if let Some(path) = env_path {
            let p = Path::new(path);
            log.push(format!("env:{}", path));
            if available(p) {
                return Some(p.to_path_buf());
            }
        }
        // 2. Standard install candidates
        for cand in candidates {
            let p = Path::new(cand);
            log.push(format!("candidate:{}", cand));
            if available(p) {
                return Some(p.to_path_buf());
            }
        }
        // 3. PATH lookup
        if let Some(path) = path_result {
            let p = Path::new(path);
            log.push(format!("path:{}", path));
            if available(p) {
                return Some(p.to_path_buf());
            }
        }
        // 4. Last resort /tmp
        let tmp_p = Path::new(tmp);
        log.push(format!("tmp:{}", tmp));
        if available(tmp_p) {
            return Some(tmp_p.to_path_buf());
        }
        None
    }
}

/// Global production resolver.  Created once on first use.
static GLOBAL_RESOLVER: SofficeResolver = SofficeResolver::new();

/// Lightweight availability check: run `soffice --version` and verify the
/// binary starts successfully.
fn soffice_is_available(path: &Path) -> bool {
    std::process::Command::new(path)
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .stdin(Stdio::null())
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

/// Production path lookup: run `which soffice`.
fn which_soffice() -> Option<String> {
    std::process::Command::new("which")
        .arg("soffice")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
}

pub fn resolve_soffice() -> Option<&'static Path> {
    // Fast path: cache hit — zero env/which work
    if let Some(path) = GLOBAL_RESOLVER.cache.get() {
        return Some(path);
    }
    // Cache miss: gather dynamic data and resolve
    let env_path = std::env::var("CHEERSAI_LIBREOFFICE_PATH").ok();
    GLOBAL_RESOLVER.resolve(
        env_path.as_deref(),
        MACOS_CANDIDATES,
        which_soffice().as_deref(),
        PORTABLE_TMP_CANDIDATE,
        &soffice_is_available,
        &mut Vec::new(), // production discards probe log
    )
}

/// Check whether bytes look like a legacy OLE2-based .ppt file.
///
/// Minimum size check (512 bytes, the standard OLE2 sector size) avoids
/// treating tiny byte sequences or signature-only fragments as PPT files;
/// those will reach the existing parser, which returns `INPUT_ENCRYPTED`
/// if the format truly is encrypted/corrupted.
pub fn looks_like_legacy_ppt(bytes: &[u8]) -> bool {
    bytes.len() >= 512 && bytes.starts_with(OLE2_SIG)
}

/// Required OLE2 stream name for legacy PowerPoint 97-2003 files.
const POWERPOINT_DOCUMENT_STREAM: &str = "PowerPoint Document";

/// Validate that `input` is a well-formed OLE2/CFB compound file whose
/// required "PowerPoint Document" entry is a stream (not a storage) and
/// can be successfully opened.
///
/// Uses the `cfb` crate for reliable structural parsing.  A CFB whose
/// "PowerPoint Document" is a storage, or cannot be opened as a stream,
/// is NOT a valid PowerPoint file and returns `InputCorrupted`.
fn validate_cfb(input: &[u8]) -> Option<ConvertError> {
    use std::io::Cursor;
    if input.len() < 512 {
        return Some(ConvertError::InputCorrupted);
    }
    let cursor = Cursor::new(input);
    let mut compound = match cfb::CompoundFile::open(cursor) {
        Ok(c) => c,
        Err(_) => return Some(ConvertError::InputCorrupted),
    };
    // Must exist and be a stream (not a storage)
    if !compound.is_stream(POWERPOINT_DOCUMENT_STREAM) {
        return Some(ConvertError::InputCorrupted);
    }
    // Must be openable
    if compound.open_stream(POWERPOINT_DOCUMENT_STREAM).is_err() {
        return Some(ConvertError::InputCorrupted);
    }
    None
}

// --------------- test-only override ---------------

#[cfg(test)]
thread_local! {
    static TEST_SOFFICE: std::cell::RefCell<Option<Option<PathBuf>>> =
        const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
pub fn set_test_soffice(path: Option<PathBuf>) {
    TEST_SOFFICE.with(|cell| *cell.borrow_mut() = Some(path));
}

#[cfg(test)]
pub fn clear_test_soffice() {
    TEST_SOFFICE.with(|cell| *cell.borrow_mut() = None);
}

// --------------- conversion ---------------

/// Convert legacy .ppt bytes to .pptx bytes (production entry point).
///
/// Validates input structure first (size, OLE2 header) — these checks do not
/// need a converter.  Then resolves the soffice path via env → candidates →
/// PATH → /tmp and delegates to `convert_with_soffice`.
pub async fn convert_ppt_to_pptx(input: &[u8]) -> Result<Vec<u8>, ConvertError> {
    // --- input checks that do not need a converter ---
    if input.is_empty() {
        return Err(ConvertError::InputCorrupted);
    }
    if input.len() > MAX_PPT_BYTES {
        return Err(ConvertError::ResourceLimitExceeded);
    }
    if let Some(err) = validate_cfb(input) {
        return Err(err);
    }

    #[cfg(test)]
    let soffice: Option<PathBuf> = TEST_SOFFICE.with(|cell| match &*cell.borrow() {
        Some(override_path) => override_path.clone(),
        None => resolve_soffice().map(|p| p.to_path_buf()),
    });

    #[cfg(not(test))]
    let soffice: Option<PathBuf> = resolve_soffice().map(|p| p.to_path_buf());

    let soffice = soffice.ok_or(ConvertError::ConverterUnavailable)?;
    convert_with_soffice(input, &soffice).await
}

/// Core conversion logic: runs the given `soffice` binary to convert
/// .ppt → .pptx.
///
/// This is the testable inner function — callers provide an explicit
/// soffice path so that tests can inject fake converters.
/// Callers are responsible for validating the input (size, CFB structure)
/// before calling this function.
pub async fn convert_with_soffice(input: &[u8], soffice: &Path) -> Result<Vec<u8>, ConvertError> {
    convert_with_soffice_opt(input, soffice, DEFAULT_CONVERT_TIMEOUT, MAX_OUTPUT_BYTES).await
}

/// Conversion with configurable timeout and max output size (for testing).
#[doc(hidden)]
pub async fn convert_with_soffice_opt(
    input: &[u8],
    soffice: &Path,
    convert_timeout: Duration,
    max_output_bytes: u64,
) -> Result<Vec<u8>, ConvertError> {
    // --- temp directory ---
    let tmp_dir = tempfile::tempdir().map_err(|_| ConvertError::Internal)?;
    let tmp_path = tmp_dir.path().to_path_buf();

    let input_path = tmp_path.join("input.ppt");
    let output_dir = tmp_path.join("output");
    let profile_dir = tmp_path.join("profile");

    std::fs::create_dir_all(&output_dir).map_err(|_| ConvertError::Internal)?;
    std::fs::create_dir_all(&profile_dir).map_err(|_| ConvertError::Internal)?;

    // Write input file
    std::fs::write(&input_path, input).map_err(|_| ConvertError::Internal)?;

    // --- run converter ---
    let mut child = Command::new(soffice)
        .arg("--headless")
        .arg("--norestore")
        .arg("--nologo")
        .arg(format!(
            "-env:UserInstallation=file://{}",
            profile_dir.to_string_lossy()
        ))
        .arg("--convert-to")
        .arg("pptx")
        .arg("--outdir")
        .arg(&output_dir)
        .arg(&input_path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .map_err(|_| ConvertError::ConverterUnavailable)?;

    let deadline = timeout(convert_timeout, child.wait());
    let status = match deadline.await {
        Ok(Ok(status)) => status,
        Ok(Err(_)) => {
            let _ = child.start_kill();
            return Err(ConvertError::ConverterUnavailable);
        }
        Err(_elapsed) => {
            let _ = child.start_kill();
            let _ = child.wait().await;
            return Err(ConvertError::ProcessingTimeout);
        }
    };

    if !status.success() {
        return Err(ConvertError::Internal);
    }

    // --- collect output ---
    let expected = output_dir.join("input.pptx");
    if !expected.exists() {
        return Err(ConvertError::Internal);
    }

    let metadata = std::fs::metadata(&expected).map_err(|_| ConvertError::Internal)?;
    if metadata.len() > max_output_bytes {
        return Err(ConvertError::ResourceLimitExceeded);
    }

    let output_bytes = std::fs::read(&expected).map_err(|_| ConvertError::Internal)?;

    // --- validate output: must parse as real PowerPoint via engine_core ---
    if !validate_pptx_output(&output_bytes) {
        return Err(ConvertError::Internal);
    }

    Ok(output_bytes)
}

/// Validate that `bytes` is a structurally valid PPTX by delegating to
/// engine_core's real PowerPoint parser.  A plain ZIP, a file with only
/// PK header + EOCD, or a ZIP missing [Content_Types].xml or
/// ppt/presentation.xml will fail the parser → `false`.
fn validate_pptx_output(bytes: &[u8]) -> bool {
    engine_core::parse_input(bytes, engine_core::InputFormat::Powerpoint).is_ok()
}

// --------------- tests ---------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{mpsc, Arc, Barrier};
    use std::thread;

    // ---- helpers ----

    /// Build a minimal CFB that contains the required "PowerPoint Document" stream.
    fn minimal_valid_cfb() -> Vec<u8> {
        let mut buf = std::io::Cursor::new(Vec::new());
        {
            let mut comp = cfb::CompoundFile::create(&mut buf).unwrap();
            comp.create_stream(POWERPOINT_DOCUMENT_STREAM).unwrap();
        }
        buf.into_inner()
    }

    /// Build a CFB that contains "Hello" but NOT "PowerPoint Document".
    fn cfb_with_hello_stream() -> Vec<u8> {
        let mut buf = std::io::Cursor::new(Vec::new());
        {
            let mut comp = cfb::CompoundFile::create(&mut buf).unwrap();
            comp.create_stream("Hello").unwrap();
        }
        buf.into_inner()
    }

    /// Random bytes that are NOT a valid CFB (no OLE2 magic).
    fn corrupt_bytes_not_cfb() -> Vec<u8> {
        let mut buf = vec![0u8; 1024];
        buf[0] = 0xDE;
        buf[1] = 0xAD;
        buf
    }

    /// Read the shared `fictional.pptx` fixture for tests that need valid PPTX output.
    fn valid_pptx_bytes() -> Vec<u8> {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/fictional.pptx");
        std::fs::read(path).expect("fictional.pptx fixture must exist")
    }

    // ---- signature detection ----

    #[test]
    fn detects_ole2_signature() {
        let mut ole2 = vec![0xd0, 0xcf, 0x11, 0xe0];
        ole2.resize(512, 0);
        assert!(looks_like_legacy_ppt(&ole2));
        assert!(!looks_like_legacy_ppt(&[
            0xd0, 0xcf, 0x11, 0xe0, 0x00, 0x01
        ]));
        assert!(!looks_like_legacy_ppt(b"PK\x03\x04"));
        assert!(!looks_like_legacy_ppt(b""));
        assert!(!looks_like_legacy_ppt(b"not ole2"));
    }

    // ---- CFB structural validation ----

    /// Build a CFB with "PowerPoint Document" as a storage (not stream).
    fn cfb_with_powerpoint_document_storage() -> Vec<u8> {
        let mut buf = std::io::Cursor::new(Vec::new());
        {
            let mut comp = cfb::CompoundFile::create(&mut buf).unwrap();
            comp.create_storage(POWERPOINT_DOCUMENT_STREAM).unwrap();
        }
        buf.into_inner()
    }

    #[test]
    fn validate_accepts_cfb_with_powerpoint_document_stream() {
        assert!(validate_cfb(&minimal_valid_cfb()).is_none());
    }

    #[test]
    fn validate_rejects_cfb_with_same_name_storage() {
        // Same name as a storage — must be rejected
        assert_eq!(
            validate_cfb(&cfb_with_powerpoint_document_storage()),
            Some(ConvertError::InputCorrupted),
            "PowerPoint Document as storage must be INPUT_CORRUPTED"
        );
    }

    #[test]
    fn validate_rejects_cfb_without_powerpoint_document_stream() {
        assert_eq!(
            validate_cfb(&cfb_with_hello_stream()),
            Some(ConvertError::InputCorrupted),
            "CFB without PowerPoint Document stream must be INPUT_CORRUPTED"
        );
    }

    #[test]
    fn validate_accepts_real_ppt_fixtures() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/ppt_normal_demo.ppt"
        );
        let data = std::fs::read(path).unwrap();
        assert!(
            validate_cfb(&data).is_none(),
            "real .ppt fixture must pass CFB validation"
        );
        let path2 = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/ppt_contacts.ppt"
        );
        let data2 = std::fs::read(path2).unwrap();
        assert!(
            validate_cfb(&data2).is_none(),
            "real .ppt fixture must pass CFB validation"
        );
        let path3 = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/ppt_corrupt.ppt"
        );
        let data3 = std::fs::read(path3).unwrap();
        assert_eq!(validate_cfb(&data3), Some(ConvertError::InputCorrupted));
    }

    #[test]
    fn validate_rejects_empty() {
        assert_eq!(validate_cfb(b""), Some(ConvertError::InputCorrupted));
    }

    #[test]
    fn validate_rejects_too_small() {
        let tiny = vec![0u8; 100];
        assert_eq!(validate_cfb(&tiny), Some(ConvertError::InputCorrupted));
    }

    #[test]
    fn validate_rejects_random_bytes() {
        assert_eq!(
            validate_cfb(&corrupt_bytes_not_cfb()),
            Some(ConvertError::InputCorrupted)
        );
    }

    // ---- PPTX output validation (via engine_core parser) ----

    #[test]
    fn engine_core_accepts_valid_pptx() {
        assert!(validate_pptx_output(&valid_pptx_bytes()));
    }

    #[test]
    fn engine_core_rejects_plain_zip() {
        // A valid ZIP that is not a PPTX
        let plain_zip = minimal_valid_zip();
        assert!(
            !validate_pptx_output(&plain_zip),
            "plain ZIP must be rejected by engine_core parser"
        );
    }

    #[test]
    fn engine_core_rejects_pk_header_only() {
        assert!(!validate_pptx_output(b"PK\x03\x04garbage"));
    }

    // ---- P1/P2: resolver/cache controllable tests ----

    /// Minimal valid ZIP (file "a" with "hello") for output tests.
    /// Not a valid PPTX — engine_core parser will reject it.
    fn minimal_valid_zip() -> &'static [u8] {
        &[
            0x50, 0x4b, 0x03, 0x04, 0x14, 0x00, 0x00, 0x00, 0x00, 0x00, 0xf4, 0x8e, 0xf9, 0x5c,
            0x86, 0xa6, 0x10, 0x36, 0x05, 0x00, 0x00, 0x00, 0x05, 0x00, 0x00, 0x00, 0x01, 0x00,
            0x00, 0x00, 0x61, 0x68, 0x65, 0x6c, 0x6c, 0x6f, 0x50, 0x4b, 0x01, 0x02, 0x14, 0x03,
            0x14, 0x00, 0x00, 0x00, 0x00, 0x00, 0xf4, 0x8e, 0xf9, 0x5c, 0x86, 0xa6, 0x10, 0x36,
            0x05, 0x00, 0x00, 0x00, 0x05, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x80, 0x01, 0x00, 0x00, 0x00, 0x00, 0x61, 0x50,
            0x4b, 0x05, 0x06, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x01, 0x00, 0x2f, 0x00, 0x00,
            0x00, 0x24, 0x00, 0x00, 0x00, 0x00, 0x00,
        ]
    }

    /// Always-false probe for testing "nothing available".
    fn probe_none(_: &Path) -> bool {
        false
    }

    /// Always-true probe (for testing candidate acceptance).
    fn probe_all(_: &Path) -> bool {
        true
    }

    #[test]
    fn resolver_search_order_env_then_candidates_then_path_then_tmp() {
        let env = Some("/fake/env/soffice");
        let candidates: &[&str] = &["/a/soffice", "/b/soffice"];
        let path_result = Some("/usr/local/bin/soffice");
        let tmp = "/tmp/portable/soffice";
        let mut log = Vec::new();

        // All probes fail → None
        let result =
            SofficeResolver::search(env, candidates, path_result, tmp, &probe_none, &mut log);
        assert!(result.is_none());
        assert_eq!(
            log.len(),
            5,
            "must probe all 5 sources (env + 2 candidates + path + tmp)"
        );
        assert!(log[0].starts_with("env:"), "first probe must be env");
        assert!(log[1].starts_with("candidate:"));
        assert!(log[2].starts_with("candidate:"));
        assert!(log[3].starts_with("path:"));
        assert!(log[4].starts_with("tmp:"));
    }

    #[test]
    fn resolver_env_wins_over_candidates() {
        let mut log = Vec::new();
        let result = SofficeResolver::search(
            Some("/valid/soffice"),
            &["/never/reached"],
            Some("/never/reached"),
            "/never/reached",
            &probe_all,
            &mut log,
        );
        assert!(result.is_some());
        assert_eq!(log.len(), 1, "must stop after env succeeds");
        assert!(log[0].starts_with("env:"));
    }

    #[test]
    fn resolver_bad_env_falls_through_to_candidates() {
        let mut log = Vec::new();
        let result = SofficeResolver::search(
            Some("/bad/env"),
            &["/never/here"],
            Some("/valid/path/soffice"),
            "/never/reached",
            &|p: &Path| p.to_str().unwrap().contains("/valid/"),
            &mut log,
        );
        assert!(result.is_some());
        assert!(log[0].starts_with("env:"));
        assert!(log[1].starts_with("candidate:"));
        assert!(log[2].starts_with("path:"));
        assert_eq!(log.len(), 3);
    }

    #[test]
    fn resolver_path_before_tmp() {
        let mut log = Vec::new();
        // path probe fails, tmp probe succeeds — order must be path then tmp
        let result = SofficeResolver::search(
            None,
            &[],
            Some("/path/soffice"),
            "/tmp/soffice",
            &|p: &Path| p.to_str().unwrap().contains("/tmp/"),
            &mut log,
        );
        assert!(result.is_some());
        assert_eq!(log.len(), 2);
        assert!(
            log[0].starts_with("path:"),
            "PATH must be probed before tmp"
        );
        assert!(log[1].starts_with("tmp:"), "tmp must be last");
    }

    #[test]
    fn resolver_cache_positive_prevents_repeat_probing() {
        let resolver = SofficeResolver::new();
        let mut log1 = Vec::new();
        let mut log2 = Vec::new();

        // First call: must probe
        let r1 = resolver.resolve(None, &["/a/soffice"], None, "/tmp/x", &probe_all, &mut log1);
        assert!(r1.is_some());
        assert!(!log1.is_empty(), "first call must probe");

        // Second call: must return cached without probing
        let r2 = resolver.resolve(None, &["/a/soffice"], None, "/tmp/x", &probe_all, &mut log2);
        assert_eq!(r1.map(|p| p.to_path_buf()), r2.map(|p| p.to_path_buf()));
        assert!(log2.is_empty(), "cached call must not probe again");
    }

    #[test]
    fn resolver_failure_not_cached_resolve_retries() {
        // Same resolver instance: first call all probes fail, second call
        // with new probes succeeds — failure must not be cached.
        let resolver = SofficeResolver::new();
        let mut log1 = Vec::new();
        let r1 = resolver.resolve(None, &["/bad"], None, "/bad2", &probe_none, &mut log1);
        assert!(r1.is_none());
        assert!(!log1.is_empty(), "first call must probe");

        // Second call: now probes succeed — must work
        let mut log2 = Vec::new();
        let r2 = resolver.resolve(None, &["/a/soffice"], None, "/tmp/x", &probe_all, &mut log2);
        assert!(
            r2.is_some(),
            "failure must not be cached; second call with valid probe must succeed"
        );
        assert!(!log2.is_empty(), "second call must probe again");
    }

    #[test]
    fn resolver_concurrent_failure_rechecks_first_writer_cache() {
        let resolver = Arc::new(SofficeResolver::new());
        let both_searches_started = Arc::new(Barrier::new(2));
        let (cache_written_tx, cache_written_rx) = mpsc::channel();

        let success_resolver = Arc::clone(&resolver);
        let success_barrier = Arc::clone(&both_searches_started);
        let success_thread = thread::spawn(move || {
            let available = move |path: &Path| {
                assert_eq!(path, Path::new("/first-writer/soffice"));
                success_barrier.wait();
                true
            };
            let result = success_resolver
                .resolve(
                    Some("/first-writer/soffice"),
                    &[],
                    None,
                    "/unused-success-tmp",
                    &available,
                    &mut Vec::new(),
                )
                .map(Path::to_path_buf);
            cache_written_tx.send(()).unwrap();
            result
        });

        let failure_resolver = Arc::clone(&resolver);
        let failure_probe_resolver = Arc::clone(&failure_resolver);
        let failure_barrier = Arc::clone(&both_searches_started);
        let failure_thread = thread::spawn(move || {
            let wait_for_cache = AtomicBool::new(true);
            let available = move |_: &Path| {
                if wait_for_cache.swap(false, Ordering::SeqCst) {
                    failure_barrier.wait();
                    cache_written_rx.recv().unwrap();
                }
                assert_eq!(
                    failure_probe_resolver
                        .cache
                        .get()
                        .map(|path| path.as_path()),
                    Some(Path::new("/first-writer/soffice")),
                    "failure probe must observe the first writer before returning false"
                );
                false
            };
            failure_resolver
                .resolve(
                    None,
                    &["/failure/candidate"],
                    None,
                    "/failure/tmp",
                    &available,
                    &mut Vec::new(),
                )
                .map(Path::to_path_buf)
        });

        let success_result = success_thread.join().unwrap();
        let failure_result = failure_thread.join().unwrap();
        let expected = Some(PathBuf::from("/first-writer/soffice"));
        assert_eq!(success_result, expected);
        assert_eq!(failure_result, expected);
    }

    #[test]
    fn resolver_cache_hit_does_not_call_probe() {
        // After a successful resolve, subsequent calls must return the cached
        // path without calling the probe at all.
        let resolver = SofficeResolver::new();

        // Establish cache
        let r1 = resolver.resolve(
            None,
            &["/a/soffice"],
            None,
            "/tmp/x",
            &probe_all,
            &mut Vec::new(),
        );
        assert!(r1.is_some());

        // Probe that panics if called — cache hit must skip it entirely
        let r2 = resolver.resolve(
            None,
            &["/a/soffice"],
            None,
            "/tmp/x",
            &|_: &Path| -> bool {
                panic!("probe must not be called on cache hit");
            },
            &mut Vec::new(),
        );
        assert!(r2.is_some());
        assert_eq!(r1.unwrap(), r2.unwrap(), "must return same cached path");
    }

    // ---- fake converter tests ----

    fn write_fake_soffice_script(script_path: &Path, exit_code: i32, copy_output: Option<&Path>) {
        use std::io::Write;
        let script = if let Some(src) = copy_output {
            format!(
                "#!/bin/sh\nmkdir -p \"$8\"\ncp \"{}\" \"$8/input.pptx\"\nexit {}\n",
                src.display(),
                exit_code
            )
        } else {
            format!("#!/bin/sh\nexit {}\n", exit_code)
        };
        let mut file = std::fs::File::create(script_path).unwrap();
        file.write_all(script.as_bytes()).unwrap();
        file.flush().unwrap();
        std::process::Command::new("chmod")
            .arg("+x")
            .arg(script_path)
            .output()
            .unwrap();
    }

    /// Write a valid PPTX to `dest` from the fictional.pptx fixture.
    fn copy_valid_pptx_to(dest: &Path) {
        std::fs::write(dest, &valid_pptx_bytes()).unwrap();
    }

    #[tokio::test]
    async fn fake_converter_nonzero_exit_is_internal() {
        let dir = tempfile::tempdir().unwrap();
        let fake = dir.path().join("fake-soffice");
        write_fake_soffice_script(&fake, 1, None);
        let result = convert_with_soffice(&minimal_valid_cfb(), &fake).await;
        assert_eq!(result.unwrap_err(), ConvertError::Internal);
    }

    #[tokio::test]
    async fn fake_converter_no_output_is_internal() {
        let dir = tempfile::tempdir().unwrap();
        let fake = dir.path().join("fake-soffice");
        write_fake_soffice_script(&fake, 0, None);
        let result = convert_with_soffice(&minimal_valid_cfb(), &fake).await;
        assert_eq!(result.unwrap_err(), ConvertError::Internal);
    }

    #[tokio::test]
    async fn fake_converter_invalid_output_plain_zip_is_internal() {
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("bad.pptx");
        std::fs::write(&out, minimal_valid_zip()).unwrap(); // plain ZIP, not PPTX
        let fake = dir.path().join("fake-soffice");
        write_fake_soffice_script(&fake, 0, Some(&out));
        let result = convert_with_soffice(&minimal_valid_cfb(), &fake).await;
        assert_eq!(
            result.unwrap_err(),
            ConvertError::Internal,
            "plain ZIP output must be CONVERSION_INTERNAL_ERROR"
        );
    }

    #[tokio::test]
    async fn fake_converter_valid_pptx_succeeds() {
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("valid.pptx");
        copy_valid_pptx_to(&out);
        let fake = dir.path().join("fake-soffice");
        write_fake_soffice_script(&fake, 0, Some(&out));
        let result = convert_with_soffice(&minimal_valid_cfb(), &fake).await;
        assert!(result.is_ok(), "valid PPTX output must succeed");
    }

    #[tokio::test]
    async fn fake_converter_spawn_fail_is_unavailable() {
        let result =
            convert_with_soffice(&minimal_valid_cfb(), Path::new("/nonexistent/path/soffice"))
                .await;
        assert_eq!(result.unwrap_err(), ConvertError::ConverterUnavailable);
    }

    #[tokio::test]
    async fn no_candidate_is_unavailable() {
        set_test_soffice(None);
        let result = convert_ppt_to_pptx(&minimal_valid_cfb()).await;
        clear_test_soffice();
        assert_eq!(result.unwrap_err(), ConvertError::ConverterUnavailable);
    }

    // ---- timeout / output-limit tests (R4) ----

    #[tokio::test]
    async fn fake_converter_timeout_is_processing_timeout() {
        // Fake soffice that sleeps 5 seconds; we set timeout to 1 second
        let dir = tempfile::tempdir().unwrap();
        let fake = dir.path().join("fake-soffice");
        let script = "#!/bin/sh\nsleep 5\nexit 0\n";
        std::fs::write(&fake, script).unwrap();
        std::process::Command::new("chmod")
            .arg("+x")
            .arg(&fake)
            .output()
            .unwrap();

        let result = convert_with_soffice_opt(
            &minimal_valid_cfb(),
            &fake,
            Duration::from_secs(1),
            MAX_OUTPUT_BYTES,
        )
        .await;
        assert_eq!(
            result.unwrap_err(),
            ConvertError::ProcessingTimeout,
            "converter that exceeds deadline must be PROCESSING_TIMEOUT"
        );
    }

    #[tokio::test]
    async fn fake_converter_output_too_large_is_resource_limit_exceeded() {
        // Fake soffice that writes a 1 MB file; we set max output to 100 bytes
        let dir = tempfile::tempdir().unwrap();
        let fake = dir.path().join("fake-soffice");
        let script = "#!/bin/sh\nmkdir -p \"$8\"\ndd if=/dev/zero of=\"$8/input.pptx\" bs=1024 count=1 2>/dev/null\nexit 0\n";
        std::fs::write(&fake, script).unwrap();
        std::process::Command::new("chmod")
            .arg("+x")
            .arg(&fake)
            .output()
            .unwrap();

        let result = convert_with_soffice_opt(
            &minimal_valid_cfb(),
            &fake,
            DEFAULT_CONVERT_TIMEOUT,
            100, // very small max output
        )
        .await;
        assert_eq!(
            result.unwrap_err(),
            ConvertError::ResourceLimitExceeded,
            "output exceeding limit must be RESOURCE_LIMIT_EXCEEDED"
        );
    }

    // ---- error code coverage ----

    #[test]
    fn error_codes_are_meaningful_and_distinct() {
        let codes: Vec<&str> = vec![
            ConvertError::ConverterUnavailable,
            ConvertError::InputCorrupted,
            ConvertError::InputEncrypted,
            ConvertError::ProcessingTimeout,
            ConvertError::ResourceLimitExceeded,
            ConvertError::Internal,
        ]
        .into_iter()
        .map(|e| e.error_code())
        .collect();
        assert!(codes.contains(&"LEGACY_CONVERTER_UNAVAILABLE"));
        assert!(codes.contains(&"INPUT_CORRUPTED"));
        assert!(codes.contains(&"INPUT_ENCRYPTED"));
        assert!(codes.contains(&"PROCESSING_TIMEOUT"));
        assert!(codes.contains(&"RESOURCE_LIMIT_EXCEEDED"));
        assert!(codes.contains(&"CONVERSION_INTERNAL_ERROR"));
        let unique: std::collections::HashSet<&str> = codes.iter().copied().collect();
        assert_eq!(unique.len(), codes.len());
    }

    #[test]
    fn converter_unavailable_is_not_retryable() {
        assert!(!ConvertError::ConverterUnavailable.to_app_error().retryable);
    }

    #[test]
    fn timeout_is_retryable() {
        assert!(ConvertError::ProcessingTimeout.to_app_error().retryable);
    }

    #[test]
    fn internal_is_retryable() {
        assert!(ConvertError::Internal.to_app_error().retryable);
    }

    #[test]
    fn to_app_error_maps_every_variant() {
        for variant in &[
            ConvertError::ConverterUnavailable,
            ConvertError::InputCorrupted,
            ConvertError::InputEncrypted,
            ConvertError::ProcessingTimeout,
            ConvertError::ResourceLimitExceeded,
            ConvertError::Internal,
        ] {
            let app_err = variant.to_app_error();
            assert!(!app_err.code.is_empty());
            assert!(!app_err.message.is_empty());
        }
    }

    #[test]
    fn app_error_messages_never_contain_paths() {
        for variant in &[
            ConvertError::ConverterUnavailable,
            ConvertError::InputCorrupted,
            ConvertError::Internal,
            ConvertError::ProcessingTimeout,
            ConvertError::ResourceLimitExceeded,
        ] {
            let app_err = variant.to_app_error();
            assert!(
                !app_err.message.contains('/'),
                "{variant:?} message must not contain path"
            );
            assert!(
                !app_err.message.contains("soffice"),
                "{variant:?} message must not expose binary name"
            );
            assert!(
                !app_err.message.contains("tmp"),
                "{variant:?} message must not expose tmp dir"
            );
        }
    }

    // ---- basic input checks (no converter needed) ----

    #[tokio::test]
    async fn rejects_empty_input() {
        assert_eq!(
            convert_ppt_to_pptx(b"").await.unwrap_err(),
            ConvertError::InputCorrupted
        );
    }

    #[tokio::test]
    async fn rejects_oversized_input() {
        let large = vec![0u8; MAX_PPT_BYTES + 1];
        assert_eq!(
            convert_ppt_to_pptx(&large).await.unwrap_err(),
            ConvertError::ResourceLimitExceeded
        );
    }

    #[tokio::test]
    async fn structurally_corrupt_input_is_exactly_input_corrupted() {
        let result = convert_ppt_to_pptx(&corrupt_bytes_not_cfb()).await;
        assert_eq!(result.unwrap_err(), ConvertError::InputCorrupted);
    }

    /// Integration test: runs real LibreOffice if available using a bundled fixture.
    #[test]
    fn real_libreoffice_converts_bundled_ppt() {
        if resolve_soffice().is_none() {
            eprintln!("SKIP: LibreOffice not available for integration test");
            return;
        }
        let fixture_path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/ppt_normal_demo.ppt"
        );
        let ppt_data = match std::fs::read(fixture_path) {
            Ok(data) => data,
            Err(_) => {
                eprintln!("SKIP: bundled fixture not found");
                return;
            }
        };
        if !looks_like_legacy_ppt(&ppt_data) {
            eprintln!("SKIP: fixture does not look like a legacy .ppt");
            return;
        }
        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(convert_ppt_to_pptx(&ppt_data));
        match result {
            Ok(pptx_bytes) => {
                assert!(
                    validate_pptx_output(&pptx_bytes),
                    "output must parse as valid PPTX"
                );
                assert!(pptx_bytes.len() > 100, "output should be substantial");
            }
            Err(ConvertError::ConverterUnavailable) => {
                eprintln!("SKIP: LibreOffice resolved but not runnable");
            }
            Err(e) => panic!("conversion failed: {e:?}"),
        }
    }
}
