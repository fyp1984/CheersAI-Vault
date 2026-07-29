//! Shared OCR runtime component.
//!
//! This crate is the single place where:
//! - OCR configuration is validated (preflight check)
//! - The Python/`pdf_ocr.py` subprocess is spawned, timed out, and reaped
//! - The structured JSON from the Python script is parsed into
//!   `engine_core::OcrResult`
//!
//! Architecture boundary
//! ---------------------
//! - Depends on `engine-core` for the OCR data model and markdown conversion.
//! - Does NOT depend on Tauri, SQLite, Web UI, networking, or FileBay.
//! - Both the desktop Tauri app and the enterprise `vault-runtime-api` depend
//!   on this single crate — no second copy of the executor or JSON parser.

use std::path::{Path, PathBuf};
use std::time::Duration;

use engine_core::{OcrResult, validate_ocr_result};
use tokio::io::AsyncReadExt;
use tokio::process::Command;
use tokio::sync::Semaphore;
use tokio::time::timeout;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const MAX_STDOUT_BYTES: usize = 10 * 1024 * 1024; // 10 MB
const MAX_STDERR_BYTES: usize = 1024 * 1024;      // 1 MB
const DEFAULT_DPI: u64 = 300;

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Configuration for the OCR runtime.
///
/// All paths should be validated via `preflight_check` before calling
/// `run_ocr`.
#[derive(Debug, Clone)]
pub struct OcrConfig {
    /// Path to the Python interpreter (e.g. `/usr/bin/python3`).
    pub python_path: PathBuf,

    /// Path to the `pdf_ocr.py` script.
    pub script_path: PathBuf,

    /// Directory for OCR models.  **Required** for `Ready` status —
    /// must contain the EasyOCR model files.  Never falls back to
    /// `~/.EasyOCR` or any other default.
    pub model_dir: Option<PathBuf>,

    /// Maximum time to wait for OCR completion (default 300s).
    pub timeout: Duration,

    /// Maximum pages to process (default 200).
    pub max_pages: usize,

    /// Maximum pixels (width × height) per page at 300 DPI
    /// (default 12_000_000 — see `OcrConfig::default` for the derivation).
    pub max_pixels_per_page: u64,

    /// Maximum total pixels across ALL selected pages
    /// (default = max_pages × max_pixels_per_page ≈ 1.6 Gpx).
    pub max_total_pixels: u64,

    /// Maximum time for the deep preflight self-test (default 60s).
    pub preflight_timeout: Duration,
}

impl Default for OcrConfig {
    fn default() -> Self {
        let max_pages: usize = 200;
        // `DEFAULT_DPI` (300, above) is fixed and not configurable — see the
        // module docs on why adaptive DPI is out of scope. At 300 DPI, a
        // full page of standard office paper renders to (in whole pixels):
        //   Letter (612×792pt)   -> 2550×3300  = 8,417,550 px
        //   A4     (595.28×841.89pt) -> 2480×3507 = 8,697,360 px  (measured ~8,703,348)
        //   Legal  (612×1008pt)  -> 2550×4200  = 10,710,000 px
        // The previous default (8_000_000) rejected every one of these —
        // any standard full-page scan failed with INPUT_LIMIT_EXCEEDED
        // regardless of whether the OCR component itself was configured.
        // 12_000_000 covers all three with headroom, while still rejecting
        // clearly oversized input such as A3 (~17.4 Mpx at 300 DPI).
        let max_pixels_per_page: u64 = 12_000_000;
        Self {
            python_path: PathBuf::new(),
            script_path: PathBuf::new(),
            model_dir: None,
            timeout: Duration::from_secs(300),
            max_pages,
            max_pixels_per_page,
            max_total_pixels: max_pages as u64 * max_pixels_per_page,
            preflight_timeout: Duration::from_secs(60),
        }
    }
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Stable error codes and messages for the OCR runtime.
#[derive(Debug, Clone, thiserror::Error)]
pub enum OcrError {
    #[error("OCR component is not available: {0}")]
    ComponentUnavailable(String),

    #[error("OCR component is invalid or misconfigured: {0}")]
    ComponentInvalid(String),

    #[error("OCR produced no readable text")]
    NoText,

    #[error("OCR processing timed out after {0}s")]
    Timeout(u64),

    #[error("OCR input exceeds limits: {0}")]
    LimitExceeded(String),

    #[error("OCR internal error: {0}")]
    Internal(String),
}

impl OcrError {
    /// Map to a stable error code string (matching engine-core conventions).
    pub fn error_code(&self) -> &'static str {
        match self {
            Self::ComponentUnavailable(_) => "OCR_COMPONENT_REQUIRED",
            Self::ComponentInvalid(_) => "OCR_COMPONENT_INVALID",
            Self::NoText => "OCR_NO_TEXT",
            Self::Timeout(_) => "OCR_TIMEOUT",
            Self::LimitExceeded(_) => "INPUT_LIMIT_EXCEEDED",
            Self::Internal(_) => "OCR_COMPONENT_INVALID",
        }
    }

    /// Whether the error is retryable.
    pub fn is_retryable(&self) -> bool {
        matches!(self, Self::Timeout(_) | Self::Internal(_))
    }
}

// ---------------------------------------------------------------------------
// Component status
// ---------------------------------------------------------------------------

/// Preflight check result for the OCR component.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OcrComponentStatus {
    /// Python not found, script missing, or packages not importable.
    Unavailable,
    /// Python and packages found but EasyOCR / model not ready.
    Invalid,
    /// Everything needed for a full OCR run is in place.
    Ready,
}

impl OcrComponentStatus {
    pub fn is_ready(self) -> bool {
        self == Self::Ready
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Unavailable => "unavailable",
            Self::Invalid => "invalid",
            Self::Ready => "ready",
        }
    }
}

// ---------------------------------------------------------------------------
// Global concurrency limit  (R4)
// ---------------------------------------------------------------------------

static OCR_SEMAPHORE: Semaphore = Semaphore::const_new(1);

/// Acquire an OCR permit.  Blocks until one is available.
pub async fn acquire_ocr_permit() -> tokio::sync::SemaphorePermit<'static> {
    OCR_SEMAPHORE.acquire().await.expect("OCR semaphore closed")
}

// ---------------------------------------------------------------------------
// Preflight check  (R2)
// ---------------------------------------------------------------------------

/// Cached deep preflight result.  Set once on first call, never re-checked
/// (avoids reloading EasyOCR on every status API request).
static DEEP_PREFLIGHT_CACHE: once_cell::sync::OnceCell<OcrComponentStatus> =
    once_cell::sync::OnceCell::new();

/// Run the full preflight chain:
///
/// *Level 1*: Python + script exist, `import fitz` works.
/// *Level 2*: `import easyocr`, `import PIL` succeed.
/// *Level 3* (deep): initialises EasyOCR with a tiny test image under
///   `model_storage_directory`, verifying the model is present and the
///   engine can run without network access.  Uses `download_enabled=False`.
///
/// The *deep* check is run once and cached; subsequent calls return the
/// cached value.
pub fn preflight_check(config: &OcrConfig) -> OcrComponentStatus {
    // 1. Script exists
    if !config.script_path.exists() {
        return OcrComponentStatus::Unavailable;
    }

    // 2. Python binary exists
    if !config.python_path.exists() && which_python(&config.python_path).is_none() {
        return OcrComponentStatus::Unavailable;
    }

    let python = if config.python_path.exists() {
        config.python_path.clone()
    } else {
        match which_python(&config.python_path) {
            Some(p) => p,
            None => return OcrComponentStatus::Unavailable,
        }
    };

    // 3. PyMuPDF import check
    if !check_python_import(&python, "fitz") {
        return OcrComponentStatus::Unavailable;
    }

    // 4. Full OCR stack (EasyOCR + PIL)
    if !check_python_import(&python, "easyocr")
        || !check_python_import(&python, "PIL")
    {
        return OcrComponentStatus::Invalid;
    }

    // 5. Deep preflight (cached) — actually initialises EasyOCR offline.
    if let Some(cached) = DEEP_PREFLIGHT_CACHE.get() {
        return *cached;
    }

    let status = deep_preflight_check(&python, config);
    let _ = DEEP_PREFLIGHT_CACHE.set(status);
    status
}

/// Run the deep preflight: initialise EasyOCR on a tiny in-memory test image
/// with `download_enabled=False` and an explicit `model_storage_directory`.
fn deep_preflight_check(python: &Path, config: &OcrConfig) -> OcrComponentStatus {
    // Must have a model directory for Level 3.
    let model_dir = match &config.model_dir {
        Some(dir) if dir.exists() => dir.to_string_lossy().to_string(),
        _ => return OcrComponentStatus::Invalid,
    };

    let py_code = format!(
        r#"
import sys, json
try:
    import easyocr
    import numpy as np
    from PIL import Image
    reader = easyocr.Reader(
        ['ch_sim', 'en'],
        gpu=False,
        verbose=False,
        model_storage_directory='{model_dir}',
        download_enabled=False,
    )
    # Create a tiny test image (100×20 px white background)
    img = np.ones((20, 100, 3), dtype=np.uint8) * 255
    result = reader.readtext(img)
    # Success: model is usable offline
    print(json.dumps({{"ok": True, "blocks": len(result)}}))
except Exception as e:
    # Failure: model missing, checksum error, or network required
    print(json.dumps({{"ok": False, "error": str(e)}}))
    sys.exit(1)
"#,
        model_dir = model_dir
    );

    let result = std::process::Command::new(python)
        .arg("-c")
        .arg(&py_code)
        .output()
        .ok();

    let Some(output) = result else { return OcrComponentStatus::Invalid };
    if !output.status.success() {
        return OcrComponentStatus::Invalid;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    if stdout.contains(r#""ok": true"#) {
        OcrComponentStatus::Ready
    } else {
        OcrComponentStatus::Invalid
    }
}

fn which_python(python: &Path) -> Option<PathBuf> {
    let name = python.file_name()?.to_str()?;
    std::process::Command::new("which")
        .arg(name)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| {
            let path = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if path.is_empty() { None } else { Some(PathBuf::from(path)) }
        })
}

fn check_python_import(python: &Path, module: &str) -> bool {
    std::process::Command::new(python)
        .arg("-c")
        .arg(format!("import {module}; print('OK')"))
        .output()
        .map(|o| o.status.success() && String::from_utf8_lossy(&o.stdout).contains("OK"))
        .unwrap_or(false)
}

// ---------------------------------------------------------------------------
// OCR execution
// ---------------------------------------------------------------------------

/// Run OCR on a PDF file's bytes.
///
/// 1. Acquires a global concurrency permit (max 1 concurrent OCR).
/// 2. Enforces page / pixel limits **before** spawning the subprocess.
/// 3. Writes the PDF bytes to an isolated temp directory.
/// 4. Spawns `python pdf_ocr.py <args>` with a configurable timeout.
/// 5. Drains stdout/stderr concurrently (bounded, no deadlock).
/// 6. Parses the JSON from stdout.
/// 7. Validates the parsed `OcrResult`.
/// 8. Cleans up the temp directory.
pub async fn run_ocr(
    config: &OcrConfig,
    pdf_bytes: &[u8],
    page_range: Option<(usize, usize)>,
) -> Result<OcrResult, OcrError> {
    // --- concurrency permit (R4) ---
    let _permit = acquire_ocr_permit().await;

    // --- size / limit checks ---
    if pdf_bytes.is_empty() {
        return Err(OcrError::Internal("empty input".into()));
    }
    if pdf_bytes.len() > 128 * 1024 * 1024 {
        return Err(OcrError::LimitExceeded("PDF exceeds 128 MB".into()));
    }

    // --- temp directory ---
    let tmp_dir = tempfile::tempdir().map_err(|e| OcrError::Internal(e.to_string()))?;
    let tmp_path = tmp_dir.path().to_path_buf();

    let input_path = tmp_path.join("input.pdf");
    std::fs::write(&input_path, pdf_bytes)
        .map_err(|e| OcrError::Internal(format!("cannot write input: {e}")))?;

    // --- build command ---
    let python = &config.python_path;
    let mut cmd = Command::new(python);
    cmd.arg(&config.script_path)
        .arg(&input_path)
        .arg("--max-pages").arg(config.max_pages.to_string())
        .arg("--max-pixels-per-page").arg(config.max_pixels_per_page.to_string())
        .arg("--max-total-pixels").arg(config.max_total_pixels.to_string())
        .arg("--dpi").arg(DEFAULT_DPI.to_string())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);

    if let Some((start, end)) = page_range {
        cmd.arg("--start-page").arg(start.to_string());
        cmd.arg("--end-page").arg(end.to_string());
    }

    // model_dir: pass to Python for download_enabled=False
    if let Some(model_dir) = &config.model_dir {
        cmd.arg("--model-dir").arg(model_dir);
    }

    // --- spawn ---
    let mut child = cmd.spawn().map_err(|e| {
        OcrError::ComponentUnavailable(format!("cannot spawn OCR process: {e}"))
    })?;

    // --- drain stdout/stderr CONCURRENTLY with wait (R8) ---
    let stdout_handle = child.stdout.take();
    let stderr_handle = child.stderr.take();

    let stdout_task = tokio::spawn(async move {
        read_pipe_bounded(stdout_handle, MAX_STDOUT_BYTES).await
    });
    let stderr_task = tokio::spawn(async move {
        read_pipe_bounded(stderr_handle, MAX_STDERR_BYTES).await
    });

    // --- wait with timeout ---
    let deadline = timeout(config.timeout, child.wait());

    let status = match deadline.await {
        Ok(Ok(status)) => status,
        Ok(Err(e)) => {
            let _ = child.start_kill();
            let _ = child.wait().await;
            let _ = stdout_task.await;
            let _ = stderr_task.await;
            return Err(OcrError::ComponentUnavailable(format!(
                "OCR process error: {e}"
            )));
        }
        Err(_elapsed) => {
            let _ = child.start_kill();
            let _ = child.wait().await;
            let _ = stdout_task.await;
            let _ = stderr_task.await;
            return Err(OcrError::Timeout(config.timeout.as_secs()));
        }
    };

    // --- collect pipe output ---
    let stdout = stdout_task.await.unwrap_or_default();
    let stderr = stderr_task.await.unwrap_or_default();

    // --- check exit status ---
    if !status.success() {
        let safe_msg = sanitise_error(&stderr);

        if stderr.contains("import error")
            || stderr.contains("ModuleNotFoundError")
            || stderr.contains("No module named")
        {
            return Err(OcrError::ComponentInvalid(safe_msg));
        }

        // pdf_ocr.py's check_limits() (R3) reports page/pixel limit
        // violations to stderr with a "LIMIT: " prefix before exiting
        // non-zero (see pdf_ocr.py). This is an input-shape problem, not a
        // component fault: classify it as LimitExceeded (INPUT_LIMIT_EXCEEDED,
        // not retryable) rather than letting it fall through to Internal
        // (OCR_COMPONENT_INVALID, retryable), which would misreport the
        // input as a broken OCR component and cause pointless retries.
        if stderr.contains("LIMIT:") {
            return Err(OcrError::LimitExceeded(safe_msg));
        }

        return Err(OcrError::Internal(safe_msg));
    }

    // --- parse JSON ---
    let result = parse_ocr_json(&stdout)?;

    // --- validate (R7: reject failed_pages > 0) ---
    if let Some(msg) = validate_ocr_result(&result) {
        return Err(OcrError::Internal(msg));
    }

    // --- check for empty text ---
    let has_text = result
        .pages
        .iter()
        .any(|p| p.blocks.iter().any(|b| !b.text.trim().is_empty()));
    if !has_text {
        return Err(OcrError::NoText);
    }

    // Explicitly drop the temp dir so it is cleaned up eagerly.
    drop(tmp_dir);

    Ok(result)
}

/// Synchronous wrapper for desktop environments (Tauri).
pub fn run_ocr_blocking(
    config: &OcrConfig,
    pdf_bytes: &[u8],
    page_range: Option<(usize, usize)>,
) -> Result<OcrResult, OcrError> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| OcrError::Internal(format!("cannot create runtime: {e}")))?;
    rt.block_on(run_ocr(config, pdf_bytes, page_range))
}

// ---------------------------------------------------------------------------
// JSON parsing
// ---------------------------------------------------------------------------

pub fn parse_ocr_json(json_text: &str) -> Result<OcrResult, OcrError> {
    let text = json_text.trim_start_matches('\u{feff}').trim();

    if text.is_empty() {
        return Err(OcrError::Internal("empty OCR output".into()));
    }

    let result: OcrResult =
        serde_json::from_str(text).map_err(|e| OcrError::Internal(format!(
            "OCR JSON parse error: {e}. First 200 chars: {}",
            &text[..text.len().min(200)]
        )))?;

    // Validate schema version
    if result.schema_version != "1.0" {
        return Err(OcrError::Internal(format!(
            "unsupported OCR schema version: {}",
            result.schema_version
        )));
    }

    Ok(result)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Read a pipe to completion with a byte cap (prevents DoS / OOM on
/// runaway output).
async fn read_pipe_bounded<R: tokio::io::AsyncRead + Unpin + Send + 'static>(
    pipe: Option<R>,
    max_bytes: usize,
) -> String {
    let Some(handle) = pipe else { return String::new() };
    let mut buf = Vec::with_capacity(max_bytes.min(4096));
    let mut limited = handle.take(max_bytes as u64);
    let _ = limited.read_to_end(&mut buf).await;
    String::from_utf8_lossy(&buf).to_string()
}

fn sanitise_error(msg: &str) -> String {
    let lines: Vec<&str> = msg
        .lines()
        .filter(|l| {
            !l.trim().starts_with("Traceback")
                && !l.trim().starts_with("File \"")
                && !l.trim().starts_with("  File \"")
        })
        .collect();
    let safe = lines.join("; ");

    if safe.len() > 500 {
        format!("{}… (truncated)", &safe[..500])
    } else {
        safe
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // --------------- parse_ocr_json tests ---------------

    #[test]
    fn parses_valid_json() {
        let json = r#"{
            "schema_version": "1.0",
            "pages": [
                {
                    "page_number": 1,
                    "width": 612.0,
                    "height": 792.0,
                    "blocks": [
                        {"text": "Hello", "bbox": [10.0, 10.0, 100.0, 30.0], "confidence": 0.95, "language": "en"}
                    ]
                }
            ],
            "quality": {
                "total_pages": 1,
                "empty_pages": 0,
                "failed_pages": 0,
                "avg_confidence": 0.95
            }
        }"#;
        let result = parse_ocr_json(json).unwrap();
        assert_eq!(result.schema_version, "1.0");
        assert_eq!(result.pages.len(), 1);
        assert_eq!(result.pages[0].blocks[0].text, "Hello");
    }

    #[test]
    fn reject_wrong_schema_version() {
        let json = r#"{"schema_version":"0.9","pages":[],"quality":{"total_pages":0,"empty_pages":0,"failed_pages":0,"avg_confidence":0.0}}"#;
        let result = parse_ocr_json(json);
        assert!(result.is_err());
    }

    #[test]
    fn reject_empty_output() {
        let result = parse_ocr_json("");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("empty"));
    }

    #[test]
    fn reject_bad_json() {
        let result = parse_ocr_json("{{{bad json");
        assert!(result.is_err());
    }

    #[test]
    fn reject_json_without_pages() {
        let json = r#"{"schema_version":"1.0","pages":[],"quality":{"total_pages":0,"empty_pages":0,"failed_pages":0,"avg_confidence":0.0}}"#;
        let parsed = parse_ocr_json(json).unwrap();
        assert!(engine_core::validate_ocr_result(&parsed).is_some());
    }

    #[test]
    fn handles_utf8_bom() {
        let json = format!("\u{feff}{{\"schema_version\":\"1.0\",\"pages\":[{{\"page_number\":1,\"width\":612.0,\"height\":792.0,\"blocks\":[{{\"text\":\"BOM test\",\"bbox\":[0.0,0.0,10.0,10.0],\"confidence\":1.0,\"language\":\"en\"}}]}}],\"quality\":{{\"total_pages\":1,\"empty_pages\":0,\"failed_pages\":0,\"avg_confidence\":1.0}}}}");
        let result = parse_ocr_json(&json).unwrap();
        assert_eq!(result.pages[0].blocks[0].text, "BOM test");
    }

    // --------------- preflight config defaults ---------------

    #[test]
    fn default_config_has_reasonable_values() {
        let config = OcrConfig::default();
        assert_eq!(config.max_pages, 200);
        assert_eq!(config.timeout, Duration::from_secs(300));
        assert!(config.python_path.as_os_str().is_empty());
        assert_eq!(config.max_total_pixels, 200 * 12_000_000);
    }

    /// Full-page pixel count for a page of the given size (in PDF points,
    /// 72/inch) rendered at the fixed `DEFAULT_DPI`. Mirrors the
    /// `pdf_ocr.py` `check_limits()` calculation (ceil(width*zoom) *
    /// ceil(height*zoom), zoom = dpi/72) so the test doesn't hardcode
    /// magic pixel counts.
    fn page_pixels_at_default_dpi(width_pt: f64, height_pt: f64) -> u64 {
        let zoom = DEFAULT_DPI as f64 / 72.0;
        (width_pt * zoom).ceil() as u64 * (height_pt * zoom).ceil() as u64
    }

    #[test]
    fn default_max_pixels_per_page_covers_standard_office_paper() {
        let config = OcrConfig::default();
        let letter = page_pixels_at_default_dpi(612.0, 792.0);
        let a4 = page_pixels_at_default_dpi(595.28, 841.89);
        let legal = page_pixels_at_default_dpi(612.0, 1008.0);
        assert!(
            config.max_pixels_per_page >= letter,
            "default must cover Letter ({letter} px)"
        );
        assert!(
            config.max_pixels_per_page >= a4,
            "default must cover A4 ({a4} px)"
        );
        assert!(
            config.max_pixels_per_page >= legal,
            "default must cover Legal ({legal} px)"
        );
    }

    #[test]
    fn default_max_pixels_per_page_still_rejects_a3() {
        let config = OcrConfig::default();
        let a3 = page_pixels_at_default_dpi(841.89, 1190.55);
        assert!(
            config.max_pixels_per_page < a3,
            "default must still reject A3 ({a3} px) to preserve the resource guard"
        );
        assert!(config.max_pixels_per_page > 0, "the limit must never be disabled");
    }

    // --------------- error codes ---------------

    #[test]
    fn error_code_mapping_is_stable() {
        assert_eq!(
            OcrError::ComponentUnavailable("test".into()).error_code(),
            "OCR_COMPONENT_REQUIRED"
        );
        assert_eq!(
            OcrError::ComponentInvalid("test".into()).error_code(),
            "OCR_COMPONENT_INVALID"
        );
        assert_eq!(OcrError::NoText.error_code(), "OCR_NO_TEXT");
        assert_eq!(OcrError::Timeout(30).error_code(), "OCR_TIMEOUT");
        assert_eq!(
            OcrError::LimitExceeded("test".into()).error_code(),
            "INPUT_LIMIT_EXCEEDED"
        );
    }

    #[test]
    fn retryable_errors() {
        assert!(OcrError::Timeout(30).is_retryable());
        assert!(OcrError::Internal("test".into()).is_retryable());
        assert!(!OcrError::NoText.is_retryable());
        assert!(!OcrError::ComponentInvalid("test".into()).is_retryable());
        assert!(!OcrError::ComponentUnavailable("test".into()).is_retryable());
        assert!(!OcrError::LimitExceeded("test".into()).is_retryable());
    }

    // --------------- process failure classification (L4/L5) ---------------
    //
    // These drive `run_ocr` against a fake "python" (really `/bin/sh`
    // running a throwaway shell script) so the exit-status/stderr
    // classification branch is exercised end-to-end, without depending on
    // a real Python/EasyOCR installation.

    /// Build an `OcrConfig` whose "python" is `/bin/sh` running `script_body`
    /// (the pdf_ocr.py CLI args are passed through as positional shell
    /// params and ignored). Returns the config plus the TempDir that must
    /// stay alive for the script file to exist.
    fn fake_script_config(script_body: &str) -> (OcrConfig, tempfile::TempDir) {
        use std::io::Write;
        let dir = tempfile::tempdir().unwrap();
        let script_path = dir.path().join("fake_pdf_ocr.sh");
        let mut file = std::fs::File::create(&script_path).unwrap();
        writeln!(file, "#!/bin/sh").unwrap();
        writeln!(file, "{script_body}").unwrap();
        drop(file);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&script_path, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let config = OcrConfig {
            python_path: PathBuf::from("/bin/sh"),
            script_path,
            model_dir: None,
            timeout: Duration::from_secs(10),
            ..OcrConfig::default()
        };
        (config, dir)
    }

    #[tokio::test]
    async fn limit_prefixed_stderr_is_classified_as_limit_exceeded() {
        let (config, _dir) = fake_script_config(
            "echo 'LIMIT: Page 1 at 300 DPI: 999,999,999 px exceeds max 12,000,000 px per page' 1>&2; exit 1",
        );
        let err = run_ocr(&config, b"fake-pdf-bytes", None).await.unwrap_err();
        assert!(
            matches!(err, OcrError::LimitExceeded(_)),
            "expected LimitExceeded, got {err:?}"
        );
        assert_eq!(err.error_code(), "INPUT_LIMIT_EXCEEDED");
        assert!(!err.is_retryable(), "limit-exceeded input must not be retried");
    }

    #[tokio::test]
    async fn import_error_stderr_is_still_classified_as_component_invalid() {
        let (config, _dir) = fake_script_config(
            "echo \"ModuleNotFoundError: No module named 'easyocr'\" 1>&2; exit 1",
        );
        let err = run_ocr(&config, b"fake-pdf-bytes", None).await.unwrap_err();
        assert!(
            matches!(err, OcrError::ComponentInvalid(_)),
            "expected ComponentInvalid, got {err:?}"
        );
        assert_eq!(err.error_code(), "OCR_COMPONENT_INVALID");
    }

    #[tokio::test]
    async fn unrecognised_failure_stderr_is_still_classified_as_internal() {
        let (config, _dir) = fake_script_config("echo 'boom: unexpected failure' 1>&2; exit 1");
        let err = run_ocr(&config, b"fake-pdf-bytes", None).await.unwrap_err();
        assert!(
            matches!(err, OcrError::Internal(_)),
            "expected Internal, got {err:?}"
        );
        assert_eq!(err.error_code(), "OCR_COMPONENT_INVALID");
        assert!(err.is_retryable(), "unrecognised internal failures remain retryable");
    }

    // --------------- sanitise_error ---------------

    #[test]
    fn sanitise_removes_traceback_paths() {
        let msg = "Traceback (most recent call last):\n  File \"/home/user/script.py\", line 10, in <module>\nModuleNotFoundError: No module named 'easyocr'";
        let safe = sanitise_error(msg);
        assert!(!safe.contains("Traceback"));
        assert!(!safe.contains("/home/user/"));
        assert!(safe.contains("ModuleNotFoundError"));
    }

    #[test]
    fn sanitise_truncates_long_messages() {
        let long = "x".repeat(1000);
        let safe = sanitise_error(&long);
        assert!(safe.len() < 600);
        assert!(safe.ends_with("(truncated)"));
    }

    // --------------- full JSON round-trip ---------------

    #[test]
    fn full_round_trip_from_fake_ocr_json() {
        let json = r#"{
            "schema_version": "1.0",
            "pages": [
                {
                    "page_number": 1,
                    "width": 612.0,
                    "height": 792.0,
                    "blocks": [
                        {"text": "Phone: 13900000000", "bbox": [10.0, 10.0, 200.0, 30.0], "confidence": 0.95, "language": "en"},
                        {"text": "Email: test@example.cn", "bbox": [10.0, 40.0, 250.0, 60.0], "confidence": 0.88, "language": "en"}
                    ]
                },
                {
                    "page_number": 2,
                    "width": 612.0,
                    "height": 792.0,
                    "blocks": [
                        {"text": "中文姓名：张三", "bbox": [10.0, 10.0, 200.0, 30.0], "confidence": 0.75, "language": "zh"}
                    ]
                }
            ],
            "quality": {
                "total_pages": 2,
                "empty_pages": 0,
                "failed_pages": 0,
                "avg_confidence": 0.86
            }
        }"#;

        let result = parse_ocr_json(json).unwrap();
        assert_eq!(result.pages.len(), 2);
        assert_eq!(result.pages[0].blocks.len(), 2);
        assert_eq!(result.pages[1].blocks[0].text, "中文姓名：张三");

        let md = engine_core::ocr_result_to_markdown(&result);
        assert!(md.contains("13900000000"));
        assert!(md.contains("test@example.cn"));
        assert!(md.contains("张三"));
        assert!(md.contains("## Page 1"));
        assert!(md.contains("## Page 2"));
    }

    // --------------- concurrency test (R4) ---------------

    #[tokio::test]
    async fn concurrency_is_limited_to_one() {
        // Two fake OCR tasks: only one should hold the permit at a time.
        let permit1 = acquire_ocr_permit().await;
        let attempt2 = tokio::time::timeout(
            Duration::from_millis(100),
            acquire_ocr_permit(),
        );
        // Second acquire should time out while first permit is held.
        assert!(attempt2.await.is_err(),
            "second concurrent OCR permit should time out");
        drop(permit1);

        // After releasing, acquiring should succeed immediately.
        let _permit2 = tokio::time::timeout(
            Duration::from_millis(100),
            acquire_ocr_permit(),
        )
        .await
        .expect("permit should be available after release");
    }

    // --------------- validate_ocr_result: failed_pages (R7) ---------------

    #[test]
    fn validate_rejects_failed_pages() {
        let result = OcrResult {
            schema_version: "1.0".into(),
            pages: vec![engine_core::OcrPage {
                page_number: 1,
                blocks: vec![],
                width: 612.0,
                height: 792.0,
            }],
            quality: engine_core::OcrQualitySummary {
                total_pages: 1,
                empty_pages: 1,
                failed_pages: 1,
                avg_confidence: 0.0,
            },
        };
        let err = engine_core::validate_ocr_result(&result);
        assert!(err.is_some(), "failed_pages > 0 should be rejected");
        let msg = err.unwrap();
        assert!(msg.contains("failed"), "error should mention failed: {msg}");
    }

    // --------------- pipe drain stress test (R8) ---------------

    #[tokio::test]
    async fn pipe_drain_handles_large_output() {
        let large_chunk = "x".repeat(100_000);
        // Spawn a simple echo process that writes lots of data to stdout.
        let mut child = tokio::process::Command::new("echo")
            .arg(&large_chunk)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .expect("failed to spawn echo");

        let stdout_handle = child.stdout.take();
        let stderr_handle = child.stderr.take();

        let stdout_task = tokio::spawn(async move {
            read_pipe_bounded(stdout_handle, 200_000).await
        });
        let stderr_task = tokio::spawn(async move {
            read_pipe_bounded(stderr_handle, 200_000).await
        });

        let status = child.wait().await.expect("wait failed");
        assert!(status.success());

        let stdout = stdout_task.await.unwrap_or_default();
        let stderr = stderr_task.await.unwrap_or_default();
        assert!(stdout.len() >= 100_000, "stdout should contain large output");
        assert_eq!(stderr.len(), 0, "stderr should be empty");
    }

    // --------------- OcrComponentStatus as_str ---------------

    #[test]
    fn status_as_str_matches_expected_values() {
        assert_eq!(OcrComponentStatus::Unavailable.as_str(), "unavailable");
        assert_eq!(OcrComponentStatus::Invalid.as_str(), "invalid");
        assert_eq!(OcrComponentStatus::Ready.as_str(), "ready");
    }
}
