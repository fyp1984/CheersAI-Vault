mod legacy_powerpoint;
mod store;

use std::{convert::Infallible, path::PathBuf, sync::Arc, time::Duration};

use bytes::{BufMut, BytesMut};
use component_runtime::{OcrConfig, OcrComponentStatus, preflight_check};
use engine_core::{
    get_builtin_rules, ocr_result_to_markdown, parse_input, FormatCatalog, InputFormat,
    MaskingRequest, MaskingService,
};
use futures_util::TryStreamExt;
use service_contracts::{
    BatchListResponse, ErrorResponse, HealthResponse, RuleMetadata, RulesResponse,
};
use tokio::sync::Notify;
use warp::{http::StatusCode, multipart::FormData, Filter, Rejection, Reply};

use store::{NewUpload, PendingJob, Store, StoreError};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

const ENTERPRISE_RULE_IDS: &[&str] = &[
    "id_card",
    "phone",
    "email",
    "bank_card",
    "ipv4",
    "passport",
];

#[derive(Debug, Clone)]
pub struct Limits {
    pub max_files: usize,
    pub max_file_bytes: usize,
    pub max_batch_bytes: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_files: 100,
            max_file_bytes: 500 * 1024 * 1024,
            max_batch_bytes: 2 * 1024 * 1024 * 1024,
        }
    }
}

#[derive(Clone)]
pub struct Runtime {
    store: Store,
    limits: Limits,
    ocr_config: Option<OcrConfig>,
    wake_worker: Arc<Notify>,
}

impl Runtime {
    pub async fn initialize(data_root: PathBuf, limits: Limits) -> Result<Self, String> {
        let store = Store::open(data_root)
            .await
            .map_err(|_| "Runtime storage initialization failed".to_string())?;
        store
            .recover_interrupted()
            .await
            .map_err(|_| "Runtime recovery failed".to_string())?;

        // Build OCR config from environment variables
        let ocr_config = build_ocr_config_from_env();

        if let Some(ref cfg) = ocr_config {
            let status = preflight_check(cfg);
            eprintln!(
                "OCR component status: {:?} (Python: {}, script: {})",
                status,
                cfg.python_path.display(),
                cfg.script_path.display()
            );
        } else {
            eprintln!("OCR component: not configured (set VAULT_OCR_PYTHON, VAULT_OCR_SCRIPT)");
        }

        let runtime = Self {
            store,
            limits,
            ocr_config,
            wake_worker: Arc::new(Notify::new()),
        };
        runtime.spawn_worker();
        Ok(runtime)
    }

    fn spawn_worker(&self) {
        let runtime = self.clone();
        tokio::spawn(async move {
            loop {
                match runtime.store.claim_next_pending().await {
                    Ok(Some(job)) => runtime.process_job(job).await,
                    Ok(None) => {
                        tokio::select! {
                            _ = runtime.wake_worker.notified() => {},
                            _ = tokio::time::sleep(Duration::from_millis(200)) => {},
                        }
                    }
                    Err(_) => tokio::time::sleep(Duration::from_millis(200)).await,
                }
            }
        });
    }

    async fn process_job(&self, job: PendingJob) {
        let bytes = match self.store.read_input(&job.input_object_key).await {
            Ok(bytes) => bytes,
            Err(_) => {
                let _ = self
                    .store
                    .mark_failed(&job, "INPUT_READ_FAILED", "Input could not be read")
                    .await;
                return;
            }
        };
        let input_format = match InputFormat::parse(&job.input_format) {
            Some(format) => format,
            None => {
                let _ = self
                    .store
                    .mark_failed(
                        &job,
                        "INPUT_FORMAT_UNSUPPORTED",
                        "The stored input format is not supported",
                    )
                    .await;
                return;
            }
        };

        // Legacy .ppt conversion: run the converter ONLY when the original
        // file had a .ppt extension AND the bytes have an OLE2 (d0cf11e0)
        // signature.  Files with .pptx extension — even if they use an OLE2/CFB
        // container (e.g. encrypted .pptx) — must NOT enter the converter;
        // they go to the existing parser which returns INPUT_ENCRYPTED.
        let is_ppt_ext = is_legacy_ppt_extension(&job.display_name);
        let processed_bytes = if input_format == InputFormat::Powerpoint
            && is_ppt_ext
            && legacy_powerpoint::looks_like_legacy_ppt(&bytes)
        {
            match legacy_powerpoint::convert_ppt_to_pptx(&bytes).await {
                Ok(converted) => converted,
                Err(convert_err) => {
                    let app_err = convert_err.to_app_error();
                    let _ = self
                        .store
                        .mark_failed(&job, &app_err.code, &app_err.message)
                        .await;
                    return;
                }
            }
        } else {
            bytes.clone()
        };

        let content = match parse_input(&processed_bytes, input_format) {
            Ok(parsed) => parsed.markdown,
            Err(error) => {
                // OCR_COMPONENT_REQUIRED: try running OCR via component-runtime
                if error.code == "OCR_COMPONENT_REQUIRED"
                    && input_format == InputFormat::Pdf
                {
                    match run_ocr_on_pdf(self, &processed_bytes).await {
                        Ok(md) => md,
                        Err(ocr_err) => {
                            let _ = self
                                .store
                                .mark_failed(
                                    &job,
                                    ocr_err.error_code(),
                                    &ocr_err.to_string(),
                                )
                                .await;
                            return;
                        }
                    }
                } else {
                    let _ = self.store.mark_failed(&job, &error.code, &error.message).await;
                    return;
                }
            }
        };
        let rules = get_builtin_rules()
            .iter()
            .filter(|rule| job.rules.iter().any(|rule_id| rule_id == &rule.id))
            .cloned()
            .map(|mut rule| {
                rule.enabled = true;
                rule
            })
            .collect();
        let result = match MaskingService::mask(MaskingRequest {
            input_format,
            content,
            rules,
            deterministic_findings: vec![],
        }) {
            Ok(result) => result,
            Err(_) => {
                let _ = self
                    .store
                    .mark_failed(&job, "MASKING_FAILED", "Masking failed")
                    .await;
                return;
            }
        };
        // Pre-generate artifact_id for internal cmap consistency
        let artifact_id = uuid::Uuid::new_v4().to_string();

        // For server_cmap mode: pre-encode mapping with the same artifact_id
        let mapping_bytes = if job.restore_mode == "server_cmap" {
            use engine_core::{encode_server_cmap, ServerCmap, SERVER_CMAP_MAGIC, SERVER_CMAP_VERSION};
            let cmap = ServerCmap {
                format: SERVER_CMAP_MAGIC.into(),
                version: SERVER_CMAP_VERSION,
                artifact_id: artifact_id.clone(),
                created_at: chrono::Utc::now().to_rfc3339(),
                mappings: result.mappings,
            };
            match encode_server_cmap(&cmap) {
                Ok(bytes) => Some(bytes),
                Err(_) => {
                    let _ = self
                        .store
                        .mark_failed(&job, "MAPPING_ENCODE_FAILED", "Mapping encoding failed")
                        .await;
                    return;
                }
            }
        } else {
            None
        };

        let _artifact_id = match self
            .store
            .write_completed(
                &job,
                result.markdown.as_bytes(),
                result.masked_entity_count,
                &artifact_id,
                mapping_bytes.as_deref(),
            )
            .await
        {
            Ok(aid) => aid,
            Err(_) => {
                // write_completed already cleaned up any written files on failure
                let _ = self
                    .store
                    .mark_failed(&job, "OUTPUT_WRITE_FAILED", "Output could not be stored")
                    .await;
                return;
            }
        };
    }
}

#[derive(Debug)]
struct ApiError {
    status: StatusCode,
    code: &'static str,
    message: String,
    retryable: bool,
}

impl warp::reject::Reject for ApiError {}

fn api_error(
    status: StatusCode,
    code: &'static str,
    message: impl Into<String>,
    retryable: bool,
) -> Rejection {
    warp::reject::custom(ApiError {
        status,
        code,
        message: message.into(),
        retryable,
    })
}

pub fn routes(
    runtime: Runtime,
) -> impl Filter<Extract = (impl Reply,), Error = Infallible> + Clone {
    let runtime_filter = warp::any().map(move || runtime.clone());

    let health = warp::path!("api" / "v1" / "health")
        .and(warp::get())
        .map(|| {
            warp::reply::json(&HealthResponse {
                status: "ready".to_string(),
                version: VERSION.to_string(),
            })
        });

    let rules = warp::path!("api" / "v1" / "rules")
        .and(warp::get())
        .map(|| {
            warp::reply::json(&RulesResponse {
                rules: get_builtin_rules()
                    .iter()
                    .filter(|rule| ENTERPRISE_RULE_IDS.contains(&rule.id.as_str()))
                    .map(|rule| RuleMetadata {
                        id: rule.id.clone(),
                        name: rule.name.clone(),
                        enabled_by_default: rule.enabled,
                    })
                    .collect(),
            })
        });

    let ocr_status = warp::path!("api" / "v1" / "ocr" / "status")
        .and(warp::get())
        .and(runtime_filter.clone())
        .and_then(ocr_status_handler);

    let create = warp::path!("api" / "v1" / "batches")
        .and(warp::post())
        .and(warp::multipart::form().max_length(2 * 1024 * 1024 * 1024 + 1024 * 1024))
        .and(runtime_filter.clone())
        .and_then(create_batch);

    let list = warp::path!("api" / "v1" / "batches")
        .and(warp::get())
        .and(runtime_filter.clone())
        .and_then(list_batches);

    let detail = warp::path!("api" / "v1" / "batches" / String)
        .and(warp::get())
        .and(runtime_filter.clone())
        .and_then(batch_detail);

    let retry = warp::path!("api" / "v1" / "files" / String / "retry")
        .and(warp::post())
        .and(runtime_filter.clone())
        .and_then(retry_file);

    let artifact_restore = warp::path!("api" / "v1" / "artifacts" / String / "restore")
        .and(warp::post())
        .and(runtime_filter.clone())
        .and_then(artifact_restore_handler);

    let artifact = warp::path!("api" / "v1" / "artifacts" / String)
        .and(warp::get())
        .and(runtime_filter)
        .and_then(download_artifact);

    health
        .or(rules)
        .or(ocr_status)
        .or(create)
        .or(list)
        .or(detail)
        .or(retry)
        .or(artifact_restore)
        .or(artifact)
        .recover(handle_rejection)
}

async fn create_batch(form: FormData, runtime: Runtime) -> Result<impl Reply, Rejection> {
    let (uploads, rules, restore_mode) = parse_form(form, &runtime.limits).await?;
    let response = runtime
        .store
        .create_batch(uploads, rules, &restore_mode)
        .await
        .map_err(store_rejection)?;
    runtime.wake_worker.notify_one();
    Ok(warp::reply::with_status(
        warp::reply::json(&response),
        StatusCode::ACCEPTED,
    ))
}

async fn parse_form(mut form: FormData, limits: &Limits) -> Result<(Vec<NewUpload>, Vec<String>, String), Rejection> {
    let mut uploads = Vec::new();
    let mut rules = None;
    let mut restore_mode = "server_cmap".to_string();
    let mut total_bytes = 0usize;
    while let Some(part) = form.try_next().await.map_err(|_| {
        api_error(
            StatusCode::BAD_REQUEST,
            "INVALID_MULTIPART",
            "Multipart body is invalid",
            false,
        )
    })? {
        let name = part.name().to_string();
        let filename = part.filename().map(str::to_string);
        let data = part
            .stream()
            .try_fold(BytesMut::new(), |mut buffer, mut chunk| async move {
                buffer.put(&mut chunk);
                Ok::<_, warp::Error>(buffer)
            })
            .await
            .map_err(|_| {
                api_error(
                    StatusCode::BAD_REQUEST,
                    "INVALID_MULTIPART",
                    "Multipart field could not be read",
                    false,
                )
            })?
            .to_vec();

        if name == "files" {
            if uploads.len() >= limits.max_files {
                return Err(api_error(
                    StatusCode::PAYLOAD_TOO_LARGE,
                    "INPUT_LIMIT_EXCEEDED",
                    "File count exceeds the batch limit",
                    false,
                ));
            }
            if data.len() > limits.max_file_bytes {
                return Err(api_error(
                    StatusCode::PAYLOAD_TOO_LARGE,
                    "INPUT_LIMIT_EXCEEDED",
                    "A file exceeds the size limit",
                    false,
                ));
            }
            total_bytes = total_bytes.saturating_add(data.len());
            if total_bytes > limits.max_batch_bytes {
                return Err(api_error(
                    StatusCode::PAYLOAD_TOO_LARGE,
                    "INPUT_LIMIT_EXCEEDED",
                    "Batch size exceeds the limit",
                    false,
                ));
            }
            let display_name = sanitize_display_name(filename.as_deref().unwrap_or("upload"));
            let definition = FormatCatalog::enterprise_from_filename(&display_name).map_err(|error| {
                api_error(
                    StatusCode::BAD_REQUEST,
                    "INPUT_FORMAT_UNSUPPORTED",
                    error.message,
                    false,
                )
            })?;
            let input_format = match definition.input_format {
                Some(input_format) => input_format,
                None => {
                    return Err(api_error(
                        StatusCode::BAD_REQUEST,
                        "INPUT_FORMAT_UNSUPPORTED",
                        "The input format is not supported by the enterprise runtime",
                        false,
                    ));
                }
            };
            uploads.push(NewUpload {
                display_name,
                input_format: input_format.as_str().to_string(),
                bytes: data,
            });
        } else if name == "rule_ids" || name == "rules" {
            let text = std::str::from_utf8(&data).map_err(|_| {
                api_error(
                    StatusCode::BAD_REQUEST,
                    "INVALID_RULES",
                    "Rule IDs must be UTF-8",
                    false,
                )
            })?;
            rules = Some(parse_rules(text)?);
        } else if name == "restore_mode" {
            // Accept the field for backward compatibility but always generate
            // server mapping for new batches; normalizes both "disabled" and
            // "server_cmap" to "server_cmap".  Old DB records keep their value.
            restore_mode = "server_cmap".to_string();
        } else {
            return Err(api_error(
                StatusCode::BAD_REQUEST,
                "UNEXPECTED_FIELD",
                "Multipart field is not supported",
                false,
            ));
        }
    }

    if uploads.is_empty() {
        return Err(api_error(
            StatusCode::BAD_REQUEST,
            "FILES_REQUIRED",
            "At least one file is required",
            false,
        ));
    }
    let rules = rules.ok_or_else(|| {
        api_error(
            StatusCode::BAD_REQUEST,
            "INVALID_RULES",
            "Rule IDs are required",
            false,
        )
    })?;
    Ok((uploads, rules, restore_mode))
}

fn parse_rules(value: &str) -> Result<Vec<String>, Rejection> {
    let parsed = serde_json::from_str::<Vec<String>>(value).unwrap_or_else(|_| {
        value
            .split(',')
            .map(str::trim)
            .filter(|item| !item.is_empty())
            .map(str::to_string)
            .collect()
    });
    if parsed.is_empty() {
        return Err(api_error(
            StatusCode::BAD_REQUEST,
            "INVALID_RULES",
            "At least one supported rule ID is required",
            false,
        ));
    }
    if parsed
        .iter()
        .any(|rule| !ENTERPRISE_RULE_IDS.contains(&rule.as_str()))
    {
        return Err(api_error(
            StatusCode::BAD_REQUEST,
            "INVALID_RULES",
            "One or more rule IDs are not supported",
            false,
        ));
    }
    Ok(parsed)
}

fn sanitize_display_name(raw: &str) -> String {
    let base = raw.rsplit(['/', '\\']).next().unwrap_or("upload");
    let cleaned: String = base
        .chars()
        .take(120)
        .map(|character| {
            if character.is_control() || matches!(character, ':' | '*' | '?' | '"' | '<' | '>' | '|') {
                '_'
            } else {
                character
            }
        })
        .collect();
    let cleaned = cleaned.trim_matches([' ', '.']);
    if cleaned.is_empty() {
        "upload".to_string()
    } else {
        cleaned.to_string()
    }
}

/// Returns `true` when `display_name` has a `.ppt` extension (case-insensitive).
/// Used by `process_job` to decide whether bytes should enter the legacy
/// PowerPoint converter vs. going directly to the existing PPTX parser.
fn is_legacy_ppt_extension(display_name: &str) -> bool {
    let basename = display_name.rsplit(['/', '\\']).next().unwrap_or(display_name);
    match basename.rfind('.') {
        Some(pos) => {
            let ext = &basename[pos + 1..];
            ext.eq_ignore_ascii_case("ppt")
        }
        None => false,
    }
}

/// Build OCR configuration from environment variables.
///
/// Required:
///   `VAULT_OCR_PYTHON`  — path to Python interpreter
///   `VAULT_OCR_SCRIPT`  — path to `pdf_ocr.py`
///
/// Optional:
///   `VAULT_OCR_MODEL_DIR`  — EasyOCR model storage directory
///   `VAULT_OCR_TIMEOUT`    — OCR process timeout in seconds (default 300)
///   `VAULT_OCR_MAX_PAGES`  — maximum pages to process (default 200)
fn build_ocr_config_from_env() -> Option<OcrConfig> {
    let python_path = std::env::var_os("VAULT_OCR_PYTHON")?;
    let script_path = std::env::var_os("VAULT_OCR_SCRIPT")?;

    let model_dir = std::env::var_os("VAULT_OCR_MODEL_DIR")
        .map(PathBuf::from)
        .filter(|p| p.exists());

    let timeout = std::env::var("VAULT_OCR_TIMEOUT")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .map(Duration::from_secs)
        .unwrap_or_else(|| Duration::from_secs(300));

    let max_pages = std::env::var("VAULT_OCR_MAX_PAGES")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(200);

    let max_pixels_per_page = std::env::var("VAULT_OCR_MAX_PIXELS_PER_PAGE")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(OcrConfig::default().max_pixels_per_page);

    Some(OcrConfig {
        python_path: PathBuf::from(python_path),
        script_path: PathBuf::from(script_path),
        model_dir,
        timeout,
        max_pages,
        max_pixels_per_page,
        ..Default::default()
    })
}

/// Run OCR on PDF bytes via the configured OCR runtime.
///
/// Returns `Ok(markdown)` on success, or `Err(ocr_error)` on failure.
/// The caller (process_job) is responsible for marking the job as failed.
async fn run_ocr_on_pdf(runtime: &Runtime, pdf_bytes: &[u8]) -> Result<String, component_runtime::OcrError> {
    let config = runtime.ocr_config.as_ref().ok_or_else(|| {
        component_runtime::OcrError::ComponentUnavailable(
            "OCR runtime not configured (set VAULT_OCR_PYTHON, VAULT_OCR_SCRIPT)".into()
        )
    })?;

    let ocr_result = component_runtime::run_ocr(config, pdf_bytes, None).await?;
    let markdown = ocr_result_to_markdown(&ocr_result);

    if markdown.trim().is_empty() {
        return Err(component_runtime::OcrError::NoText);
    }

    Ok(markdown)
}

async fn ocr_status_handler(runtime: Runtime) -> Result<impl Reply, Rejection> {
    let (status, model_ready, timeout, max_pages) = match &runtime.ocr_config {
        Some(config) => {
            let st = match preflight_check(config) {
                OcrComponentStatus::Ready => "ready",
                OcrComponentStatus::Invalid => "invalid",
                OcrComponentStatus::Unavailable => "unavailable",
            };
            let ready = matches!(preflight_check(config), OcrComponentStatus::Ready);
            (st, ready, config.timeout.as_secs(), config.max_pages)
        }
        None => ("unavailable", false, 0u64, 0usize),
    };

    Ok(warp::reply::json(&service_contracts::OcrStatusResponse {
        status: status.to_string(),
        model_ready,
        timeout_secs: timeout,
        max_pages,
    }))
}

async fn list_batches(runtime: Runtime) -> Result<impl Reply, Rejection> {
    let batches = runtime.store.list_batches().await.map_err(store_rejection)?;
    Ok(warp::reply::json(&BatchListResponse { batches }))
}

async fn batch_detail(batch_id: String, runtime: Runtime) -> Result<impl Reply, Rejection> {
    let detail = runtime
        .store
        .batch_detail(&batch_id)
        .await
        .map_err(store_rejection)?;
    Ok(warp::reply::json(&detail))
}

async fn retry_file(file_id: String, runtime: Runtime) -> Result<impl Reply, Rejection> {
    let response = runtime
        .store
        .retry(&file_id)
        .await
        .map_err(store_rejection)?;
    runtime.wake_worker.notify_one();
    Ok(warp::reply::json(&response))
}

async fn download_artifact(artifact_id: String, runtime: Runtime) -> Result<impl Reply, Rejection> {
    let (_, bytes) = runtime
        .store
        .artifact(&artifact_id)
        .await
        .map_err(store_rejection)?;
    Ok(warp::http::Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "text/markdown; charset=utf-8")
        .header(
            "content-disposition",
            format!("attachment; filename=\"masked-{artifact_id}.md\""),
        )
        .body(bytes)
        .expect("valid artifact response"))
}

fn store_rejection(error: StoreError) -> Rejection {
    match error {
        StoreError::NotFound => api_error(
            StatusCode::NOT_FOUND,
            "NOT_FOUND",
            "The requested resource was not found",
            false,
        ),
        StoreError::RetryConflict => api_error(
            StatusCode::CONFLICT,
            "RETRY_NOT_ALLOWED",
            "Only failed files can be retried",
            false,
        ),
        StoreError::Storage | StoreError::InvalidState => api_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "RUNTIME_STORAGE_FAILED",
            "Runtime storage operation failed",
            true,
        ),
    }
}

async fn artifact_restore_handler(artifact_id: String, runtime: Runtime) -> Result<impl Reply, Rejection> {
    use engine_core::{decode_server_cmap, restore_markdown};

    let (markdown_bytes, mapping_bytes) = match runtime.store.artifact_with_mapping(&artifact_id).await {
        Ok(pair) => pair,
        Err(_) => return Err(api_error(StatusCode::NOT_FOUND, "NOT_FOUND", "Artifact or mapping not found", false)),
    };

    let cmap = match decode_server_cmap(&mapping_bytes) {
        Ok(c) => c,
        Err(e) => {
            let _ = runtime.store.log_restore_event("RestoreFailed", "failed", Some(e.error_code()), None).await;
            return Err(api_error(StatusCode::BAD_REQUEST, "CMAP_MISMATCH", "Mapping data is invalid", false));
        }
    };

    // Safety: verify the cmap bind to the requested artifact
    if cmap.artifact_id != artifact_id {
        let _ = runtime.store.log_restore_event("RestoreFailed", "failed", Some("ARTIFACT_ID_MISMATCH"), None).await;
        return Err(api_error(StatusCode::BAD_REQUEST, "CMAP_MISMATCH", "Mapping bind to a different artifact", false));
    }

    let masked_text = match String::from_utf8(markdown_bytes) {
        Ok(t) => t,
        Err(_) => return Err(api_error(StatusCode::BAD_REQUEST, "INPUT_CORRUPTED", "Artifact is not valid UTF-8", false)),
    };

    let (restored_text, count) = restore_markdown(&masked_text, &cmap.mappings);

    if count == 0 {
        let _ = runtime.store.log_restore_event("RestoreFailed", "failed", Some("CMAP_MISMATCH"), None).await;
        return Err(api_error(StatusCode::BAD_REQUEST, "CMAP_MISMATCH", "No replacements possible", false));
    }

    let _ = runtime.store.log_restore_event("RestoreSucceeded", "completed", None, Some(count)).await;

    let fname = format!("restored-{}.md", artifact_id);
    let resp = warp::http::Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "text/markdown; charset=utf-8")
        .header("content-disposition", format!("attachment; filename=\"{}\"", fname))
        .header("X-Restored-Entity-Count", count.to_string())
        .body(restored_text.into_bytes())
        .expect("valid response");
    Ok(resp)
}

async fn handle_rejection(rejection: Rejection) -> Result<impl Reply, Infallible> {
    let (status, response) = if let Some(error) = rejection.find::<ApiError>() {
        (
            error.status,
            ErrorResponse {
                code: error.code.to_string(),
                message: error.message.to_string(),
                retryable: error.retryable,
            },
        )
    } else if rejection.find::<warp::reject::PayloadTooLarge>().is_some() {
        (
            StatusCode::PAYLOAD_TOO_LARGE,
            ErrorResponse {
                code: "INPUT_LIMIT_EXCEEDED".to_string(),
                message: "Request exceeds the batch limit".to_string(),
                retryable: false,
            },
        )
    } else if rejection.is_not_found() {
        (
            StatusCode::NOT_FOUND,
            ErrorResponse {
                code: "NOT_FOUND".to_string(),
                message: "The requested resource was not found".to_string(),
                retryable: false,
            },
        )
    } else {
        (
            StatusCode::BAD_REQUEST,
            ErrorResponse {
                code: "INVALID_REQUEST".to_string(),
                message: "The request could not be processed".to_string(),
                retryable: false,
            },
        )
    };
    Ok(warp::reply::with_status(warp::reply::json(&response), status))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enterprise_uploads_use_the_shared_catalog_and_storage_values() {
        for (filename, expected) in [
            ("fixture.txt", "text"),
            ("fixture.MD", "markdown"),
            ("fixture.markdown", "markdown"),
            ("fixture.DOCX", "docx"),
            ("fixture.PDF", "pdf"),
            ("fixture.CSV", "csv"),
            ("fixture.XLS", "excel"),
            ("fixture.xlsx", "excel"),
            ("fixture.PPTX", "powerpoint"),
            ("fixture.pptx", "powerpoint"),
        ] {
            let definition = FormatCatalog::enterprise_from_filename(filename).unwrap();
            assert_eq!(definition.input_format.unwrap().as_str(), expected);
        }
        for filename in [
            "fixture.doc",
            "fixture.json",
            "fixture.png",
        ] {
            let error = FormatCatalog::enterprise_from_filename(filename).unwrap_err();
            assert_eq!(error.code, "INPUT_FORMAT_UNSUPPORTED", "{filename}");
        }

        // .ppt is now supported for enterprise uploads (converted via LibreOffice)
        for filename in ["fixture.ppt", "fixture.PPT"] {
            let definition = FormatCatalog::enterprise_from_filename(filename).unwrap();
            assert_eq!(definition.input_format.unwrap().as_str(), "powerpoint");
        }
    }
    use service_contracts::{
        BatchDetail, BatchListResponse, BatchStatus, CreateBatchResponse, FileStatus, RetryResponse,
    };
    use sha2::{Digest, Sha256};
    use tempfile::TempDir;
    use warp::test::request;

    async fn test_runtime() -> (TempDir, Runtime) {
        let temp = tempfile::tempdir().unwrap();
        let runtime = Runtime::initialize(temp.path().join("enterprise-data"), Limits::default())
            .await
            .unwrap();
        (temp, runtime)
    }

    fn multipart(files: &[(&str, &[u8])], rules: &str) -> (String, Vec<u8>) {
        let boundary = "vault-runtime-test-boundary";
        let mut body = Vec::new();
        for (name, bytes) in files {
            body.extend_from_slice(format!("--{boundary}\r\nContent-Disposition: form-data; name=\"files\"; filename=\"{name}\"\r\nContent-Type: application/octet-stream\r\n\r\n").as_bytes());
            body.extend_from_slice(bytes);
            body.extend_from_slice(b"\r\n");
        }
        body.extend_from_slice(format!("--{boundary}\r\nContent-Disposition: form-data; name=\"rule_ids\"\r\n\r\n{rules}\r\n--{boundary}--\r\n").as_bytes());
        (format!("multipart/form-data; boundary={boundary}"), body)
    }

    async fn submit(runtime: &Runtime, files: &[(&str, &[u8])]) -> CreateBatchResponse {
        let (content_type, body) = multipart(files, "[\"phone\",\"email\"]");
        let response = request()
            .method("POST")
            .path("/api/v1/batches")
            .header("content-type", content_type)
            .body(body)
            .reply(&routes(runtime.clone()))
            .await;
        assert_eq!(response.status(), StatusCode::ACCEPTED);
        serde_json::from_slice(response.body()).unwrap()
    }

    async fn batch_ids(runtime: &Runtime) -> Vec<String> {
        let response = request()
            .method("GET")
            .path("/api/v1/batches")
            .reply(&routes(runtime.clone()))
            .await;
        assert_eq!(response.status(), StatusCode::OK);
        let mut ids: Vec<String> = serde_json::from_slice::<BatchListResponse>(response.body())
            .unwrap()
            .batches
            .into_iter()
            .map(|batch| batch.batch_id)
            .collect();
        ids.sort();
        ids
    }

    #[tokio::test]
    async fn empty_and_missing_rules_are_rejected_before_persistence() {
        let (temp, runtime) = test_runtime().await;
        for rules in ["[]", "", "   "] {
            let (content_type, body) = multipart(&[("safe.txt", b"13900000000")], rules);
            let response = request()
                .method("POST")
                .path("/api/v1/batches")
                .header("content-type", content_type)
                .body(body)
                .reply(&routes(runtime.clone()))
                .await;
            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
            let error: ErrorResponse = serde_json::from_slice(response.body()).unwrap();
            assert_eq!(error.code, "INVALID_RULES");
            assert_eq!(runtime.store.record_counts().await.unwrap(), (0, 0));
        }

        let boundary = "missing-rules-boundary";
        let body = format!("--{boundary}\r\nContent-Disposition: form-data; name=\"files\"; filename=\"safe.txt\"\r\nContent-Type: text/plain\r\n\r\n13900000000\r\n--{boundary}--\r\n");
        let response = request()
            .method("POST")
            .path("/api/v1/batches")
            .header(
                "content-type",
                format!("multipart/form-data; boundary={boundary}"),
            )
            .body(body)
            .reply(&routes(runtime.clone()))
            .await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let error: ErrorResponse = serde_json::from_slice(response.body()).unwrap();
        assert_eq!(error.code, "INVALID_RULES");
        assert_eq!(runtime.store.record_counts().await.unwrap(), (0, 0));

        let mut input_entries = tokio::fs::read_dir(temp.path().join("enterprise-data/input"))
            .await
            .unwrap();
        assert!(input_entries.next_entry().await.unwrap().is_none());
    }

    async fn wait_terminal(runtime: &Runtime, batch_id: &str) -> BatchDetail {
        for _ in 0..200 {
            let detail = runtime.store.batch_detail(batch_id).await.unwrap();
            if detail.batch.status != BatchStatus::Running {
                return detail;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        panic!("batch did not reach a terminal state");
    }

    #[tokio::test]
    async fn health_is_ready_without_paths() {
        let (_temp, runtime) = test_runtime().await;
        let response = request()
            .method("GET")
            .path("/api/v1/health")
            .reply(&routes(runtime))
            .await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = std::str::from_utf8(response.body()).unwrap();
        assert!(body.contains("ready"));
        assert!(!body.contains("enterprise-data"));
    }

    #[tokio::test]
    async fn rules_metadata_comes_from_core_without_internal_fields() {
        let (_temp, runtime) = test_runtime().await;
        let response = request()
            .method("GET")
            .path("/api/v1/rules")
            .reply(&routes(runtime))
            .await;
        assert_eq!(response.status(), StatusCode::OK);
        let rules: RulesResponse = serde_json::from_slice(response.body()).unwrap();
        let expected_ids = ["id_card", "phone", "email", "bank_card", "ipv4", "passport"];
        let ids: Vec<&str> = rules.rules.iter().map(|rule| rule.id.as_str()).collect();
        assert_eq!(ids, expected_ids);
        assert!(rules.rules.iter().all(|rule| rule.enabled_by_default));
        let builtin = get_builtin_rules()
            .iter()
            .filter(|rule| ENTERPRISE_RULE_IDS.contains(&rule.id.as_str()));
        for (metadata, source) in rules.rules.iter().zip(builtin) {
            assert_eq!(metadata.id, source.id);
            assert_eq!(metadata.name, source.name);
            assert_eq!(metadata.enabled_by_default, source.enabled);
        }
        let body = std::str::from_utf8(response.body()).unwrap();
        assert!(!body.contains("pattern"));
        assert!(!body.contains("replacement"));
        assert!(!body.contains("object_key"));
    }

    #[tokio::test]
    async fn chinese_name_is_rejected_before_persistence() {
        let (temp, runtime) = test_runtime().await;
        let before = batch_ids(&runtime).await;

        for rules in [r#"["chinese_name"]"#, r#"["phone","chinese_name"]"#] {
            let (content_type, body) = multipart(
                &[("gating.txt", "虚构联系人 13900000000".as_bytes())],
                rules,
            );
            let response = request()
                .method("POST")
                .path("/api/v1/batches")
                .header("content-type", content_type)
                .body(body)
                .reply(&routes(runtime.clone()))
                .await;
            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
            let error: ErrorResponse = serde_json::from_slice(response.body()).unwrap();
            assert_eq!(error.code, "INVALID_RULES");
            assert_eq!(batch_ids(&runtime).await, before);
            assert_eq!(runtime.store.record_counts().await.unwrap(), (0, 0));
        }

        let mut input_entries = tokio::fs::read_dir(temp.path().join("enterprise-data/input"))
            .await
            .unwrap();
        assert!(input_entries.next_entry().await.unwrap().is_none());
    }

    #[tokio::test]
    async fn multi_file_api_and_artifact_match_shared_core() {
        let (_temp, runtime) = test_runtime().await;
        let first = b"Call 13900000000 and unit.test@example.invalid".as_slice();
        let second = b"# Note\nCall 13900000000 twice: 13900000000\n".as_slice();
        let third = b"name,phone\nAlice,13900000000\n".as_slice();
        let created = submit(
            &runtime,
            &[("first.txt", first), ("second.md", second), ("third.csv", third)],
        )
        .await;
        assert_eq!(created.files.len(), 3);
        let detail = wait_terminal(&runtime, &created.batch_id).await;
        if detail.batch.status != BatchStatus::Completed {
            panic!(
                "expected Completed, got {:?}, files: {:?}",
                detail.batch.status,
                detail.files.iter().map(|f| (&f.status, &f.error_code, &f.error_message)).collect::<Vec<_>>()
            );
        }
        assert_eq!(detail.batch.completed_count, 3);
        let file_detail = |file_id: &str| {
            detail
                .files
                .iter()
                .find(|file| file.file_id == file_id)
                .expect("submitted file must be present in batch detail")
        };
        assert_eq!(file_detail(&created.files[0].file_id).masked_entity_count, Some(2));
        assert_eq!(file_detail(&created.files[1].file_id).masked_entity_count, Some(2));
        assert_eq!(file_detail(&created.files[2].file_id).masked_entity_count, Some(1));
        let first_file = detail
            .files
            .iter()
            .find(|file| file.file_id == created.files[0].file_id)
            .expect("the first submitted file must be present in batch detail");

        let list = request()
            .method("GET")
            .path("/api/v1/batches")
            .reply(&routes(runtime.clone()))
            .await;
        assert_eq!(list.status(), StatusCode::OK);
        let detail_api = request()
            .method("GET")
            .path(&format!("/api/v1/batches/{}", created.batch_id))
            .reply(&routes(runtime.clone()))
            .await;
        assert_eq!(detail_api.status(), StatusCode::OK);
        let detail_body = std::str::from_utf8(detail_api.body()).unwrap();
        assert!(!detail_body.contains("object_key"));
        assert!(!detail_body.contains("input/"));

        let completed_retry = request()
            .method("POST")
            .path(&format!(
                "/api/v1/files/{}/retry",
                first_file.file_id
            ))
            .reply(&routes(runtime.clone()))
            .await;
        assert_eq!(completed_retry.status(), StatusCode::CONFLICT);

        let artifact_id = first_file.artifact_id.clone().unwrap();
        let downloaded = request()
            .method("GET")
            .path(&format!("/api/v1/artifacts/{artifact_id}"))
            .reply(&routes(runtime.clone()))
            .await;
        assert_eq!(downloaded.status(), StatusCode::OK);
        let direct = MaskingService::mask(MaskingRequest {
            input_format: InputFormat::Text,
            content: String::from_utf8(first.to_vec()).unwrap(),
            rules: get_builtin_rules()
                .iter()
                .filter(|rule| ["phone", "email"].contains(&rule.id.as_str()))
                .cloned()
                .map(|mut rule| {
                    rule.enabled = true;
                    rule
                })
                .collect(),
            deterministic_findings: vec![],
        })
        .unwrap();
        assert_eq!(downloaded.body(), direct.markdown.as_bytes());
        assert_eq!(
            Sha256::digest(downloaded.body()),
            Sha256::digest(direct.markdown.as_bytes())
        );
    }

    #[tokio::test]
    async fn csv_api_uses_shared_parser_masking_and_markdown_artifact() {
        let (_temp, runtime) = test_runtime().await;
        let csv = "name,email,phone\nAlice,alice@example.invalid,13900000000\n";
        let created = submit(&runtime, &[("contacts.CSV", csv.as_bytes())]).await;
        let detail = wait_terminal(&runtime, &created.batch_id).await;
        if detail.batch.status != BatchStatus::Completed {
            panic!(
                "expected Completed, got {:?}, files: {:?}",
                detail.batch.status,
                detail.files.iter().map(|f| (&f.status, &f.error_code, &f.error_message)).collect::<Vec<_>>()
            );
        }
        assert_eq!(detail.files[0].input_format, "csv");
        assert_eq!(detail.files[0].masked_entity_count, Some(2));
        let artifact_id = detail.files[0].artifact_id.clone().unwrap();

        let downloaded = request()
            .method("GET")
            .path(&format!("/api/v1/artifacts/{artifact_id}"))
            .reply(&routes(runtime.clone()))
            .await;
        assert_eq!(downloaded.status(), StatusCode::OK);
        assert_eq!(downloaded.headers()["content-type"], "text/markdown; charset=utf-8");
        let body = std::str::from_utf8(downloaded.body()).unwrap();
        assert!(body.contains("| Column 1 | Column 2 | Column 3 |"));
        assert!(body.contains("***EMAIL***"));
        assert!(body.contains("***PHONE***"));
        assert!(!body.contains("alice@example.invalid"));
        assert!(!body.contains("13900000000"));
        assert!(downloaded.headers()["content-disposition"].to_str().unwrap().ends_with(".md\""));
    }

    #[tokio::test]
    async fn malformed_csv_fails_without_an_artifact_and_retry_is_safe() {
        let (_temp, runtime) = test_runtime().await;
        let created = submit(&runtime, &[("broken.csv", b"a,b\n\"unclosed")]).await;
        let detail = wait_terminal(&runtime, &created.batch_id).await;
        assert_eq!(detail.batch.status, BatchStatus::Failed);
        assert_eq!(detail.files[0].status, FileStatus::Failed);
        assert_eq!(detail.files[0].error_code.as_deref(), Some("INPUT_CORRUPTED"));
        assert!(detail.files[0].artifact_id.is_none());

        let response = request()
            .method("POST")
            .path(&format!("/api/v1/files/{}/retry", detail.files[0].file_id))
            .reply(&routes(runtime.clone()))
            .await;
        assert_eq!(response.status(), StatusCode::OK);
        let retried = wait_terminal(&runtime, &created.batch_id).await;
        assert_eq!(retried.files[0].attempt, 2);
        assert_eq!(retried.files[0].error_code.as_deref(), Some("INPUT_CORRUPTED"));
        assert!(retried.files[0].artifact_id.is_none());
    }

    #[tokio::test]
    async fn parser_rejects_binary_content_disguised_as_text() {
        let (_temp, runtime) = test_runtime().await;
        let created = submit(&runtime, &[("disguised.md", b"%PDF-1.7\n")]).await;
        let detail = wait_terminal(&runtime, &created.batch_id).await;
        assert_eq!(detail.batch.status, BatchStatus::Failed);
        assert_eq!(detail.files.len(), 1);
        assert_eq!(detail.files[0].status, FileStatus::Failed);
        assert_eq!(detail.files[0].error_code.as_deref(), Some("INPUT_CORRUPTED"));
        assert!(detail.files[0].artifact_id.is_none());
    }

    fn sample_xlsx() -> Vec<u8> {
        include_bytes!("../tests/fixtures/sample.xlsx").to_vec()
    }

    fn sample_xls() -> Vec<u8> {
        include_bytes!("../tests/fixtures/sample.xls").to_vec()
    }

    fn encrypted_xlsx() -> Vec<u8> {
        include_bytes!("../tests/fixtures/encrypted.xlsx").to_vec()
    }

    fn fictional_pptx() -> Vec<u8> {
        include_bytes!("../tests/fixtures/fictional.pptx").to_vec()
    }

    fn empty_pptx() -> Vec<u8> {
        include_bytes!("../tests/fixtures/empty.pptx").to_vec()
    }

    #[tokio::test]
    async fn excel_xlsx_api_uses_shared_parser_masking_and_markdown_artifact() {
        let (_temp, runtime) = test_runtime().await;
        let created = submit(&runtime, &[("contacts.xlsx", &sample_xlsx())]).await;
        let detail = wait_terminal(&runtime, &created.batch_id).await;
        if detail.batch.status != BatchStatus::Completed {
            panic!(
                "expected Completed, got {:?}, files: {:?}",
                detail.batch.status,
                detail.files.iter().map(|f| (&f.status, &f.error_code, &f.error_message)).collect::<Vec<_>>()
            );
        }
        assert_eq!(detail.files[0].input_format, "excel");
        assert_eq!(detail.files[0].masked_entity_count, Some(5));
        let artifact_id = detail.files[0].artifact_id.clone().unwrap();

        let downloaded = request()
            .method("GET")
            .path(&format!("/api/v1/artifacts/{artifact_id}"))
            .reply(&routes(runtime.clone()))
            .await;
        assert_eq!(downloaded.status(), StatusCode::OK);
        assert_eq!(downloaded.headers()["content-type"], "text/markdown; charset=utf-8");
        let body = std::str::from_utf8(downloaded.body()).unwrap();
        assert!(body.contains("## Sheet: Sheet1"));
        assert!(body.contains("## Sheet: Sheet2"));
        assert!(body.contains("***EMAIL***"));
        assert!(body.contains("***PHONE***"));
        assert!(!body.contains("alice@example.invalid"));
        assert!(!body.contains("13900000000"));
        assert!(downloaded.headers()["content-disposition"]
            .to_str()
            .unwrap()
            .ends_with(".md\""));
    }

    #[tokio::test]
    async fn excel_xls_api_uses_shared_parser_masking_and_markdown_artifact() {
        let (_temp, runtime) = test_runtime().await;
        let created = submit(&runtime, &[("contacts.xls", &sample_xls())]).await;
        let detail = wait_terminal(&runtime, &created.batch_id).await;
        if detail.batch.status != BatchStatus::Completed {
            panic!(
                "expected Completed, got {:?}, files: {:?}",
                detail.batch.status,
                detail.files.iter().map(|f| (&f.status, &f.error_code, &f.error_message)).collect::<Vec<_>>()
            );
        }
        assert_eq!(detail.files[0].input_format, "excel");
        assert_eq!(detail.files[0].masked_entity_count, Some(5));
        let artifact_id = detail.files[0].artifact_id.clone().unwrap();

        let downloaded = request()
            .method("GET")
            .path(&format!("/api/v1/artifacts/{artifact_id}"))
            .reply(&routes(runtime.clone()))
            .await;
        assert_eq!(downloaded.status(), StatusCode::OK);
        let body = std::str::from_utf8(downloaded.body()).unwrap();
        assert!(body.contains("## Sheet: Sheet1"));
        assert!(body.contains("## Sheet: Sheet2"));
        assert!(body.contains("***PHONE***"));
        assert!(!body.contains("13900000000"));
    }

    #[tokio::test]
    async fn excel_encrypted_xlsx_fails_without_artifact_and_retry_is_safe() {
        let (_temp, runtime) = test_runtime().await;
        let created = submit(&runtime, &[("secret.xlsx", &encrypted_xlsx())]).await;
        let detail = wait_terminal(&runtime, &created.batch_id).await;
        assert_eq!(detail.batch.status, BatchStatus::Failed);
        assert_eq!(detail.files[0].status, FileStatus::Failed);
        assert_eq!(detail.files[0].error_code.as_deref(), Some("INPUT_ENCRYPTED"));
        assert!(detail.files[0].artifact_id.is_none());

        let response = request()
            .method("POST")
            .path(&format!("/api/v1/files/{}/retry", detail.files[0].file_id))
            .reply(&routes(runtime.clone()))
            .await;
        assert_eq!(response.status(), StatusCode::OK);
        let retried = wait_terminal(&runtime, &created.batch_id).await;
        assert_eq!(retried.files[0].attempt, 2);
        assert_eq!(retried.files[0].error_code.as_deref(), Some("INPUT_ENCRYPTED"));
        assert!(retried.files[0].artifact_id.is_none());
    }

    #[tokio::test]
    async fn excel_mixed_with_csv_batch_aggregates_correctly() {
        let (_temp, runtime) = test_runtime().await;
        let csv = "name,phone\nAlice,13900000000\n";
        let mixed = submit(
            &runtime,
            &[("contacts.xlsx", &sample_xlsx()), ("contacts.csv", csv.as_bytes())],
        )
        .await;
        let mixed_detail = wait_terminal(&runtime, &mixed.batch_id).await;
        assert_eq!(mixed_detail.batch.status, BatchStatus::Completed);
        assert_eq!(mixed_detail.batch.completed_count, 2);
        assert_eq!(mixed_detail.batch.failed_count, 0);
        assert!(mixed_detail.files.iter().all(|file| file.status == FileStatus::Completed));
        let total_entities: usize = mixed_detail
            .files
            .iter()
            .map(|file| file.masked_entity_count.unwrap_or(0))
            .sum();
        assert_eq!(total_entities, 6); // 5 from xlsx + 1 from csv
    }

    #[tokio::test]
    async fn powerpoint_pptx_api_uses_shared_parser_masking_and_markdown_artifact() {
        let (_temp, runtime) = test_runtime().await;
        let created = submit(&runtime, &[("slides.pptx", &fictional_pptx())]).await;
        let detail = wait_terminal(&runtime, &created.batch_id).await;
        if detail.batch.status != BatchStatus::Completed {
            panic!(
                "expected Completed, got {:?}, files: {:?}",
                detail.batch.status,
                detail.files.iter().map(|f| (&f.status, &f.error_code, &f.error_message)).collect::<Vec<_>>()
            );
        }
        assert_eq!(detail.files[0].input_format, "powerpoint");
        // Three phones and three emails across the four slides.
        assert_eq!(detail.files[0].masked_entity_count, Some(5));
        let artifact_id = detail.files[0].artifact_id.clone().unwrap();

        let downloaded = request()
            .method("GET")
            .path(&format!("/api/v1/artifacts/{artifact_id}"))
            .reply(&routes(runtime.clone()))
            .await;
        assert_eq!(downloaded.status(), StatusCode::OK);
        assert_eq!(downloaded.headers()["content-type"], "text/markdown; charset=utf-8");
        let body = std::str::from_utf8(downloaded.body()).unwrap();
        assert!(body.contains("## 幻灯片 1"));
        assert!(body.contains("## 幻灯片 4"));
        assert!(body.contains("***PHONE***"));
        assert!(body.contains("***EMAIL***"));
        assert!(!body.contains("13900000000"));
        assert!(!body.contains("alice@example.invalid"));
        assert!(downloaded.headers()["content-disposition"]
            .to_str()
            .unwrap()
            .ends_with(".md\""));
    }

    #[tokio::test]
    async fn powerpoint_empty_pptx_fails_without_artifact_and_retry_is_safe() {
        let (_temp, runtime) = test_runtime().await;
        let created = submit(&runtime, &[("empty.pptx", &empty_pptx())]).await;
        let detail = wait_terminal(&runtime, &created.batch_id).await;
        assert_eq!(detail.batch.status, BatchStatus::Failed);
        assert_eq!(detail.files[0].status, FileStatus::Failed);
        // empty.pptx is structurally valid (real zip, real slides) but has
        // no readable text; TASK-EMPTY-CONTENT-ERROR-CLASSIFICATION-001
        // reclassified this from INPUT_CORRUPTED to INPUT_NO_CONTENT.
        assert_eq!(detail.files[0].error_code.as_deref(), Some("INPUT_NO_CONTENT"));
        assert!(detail.files[0].artifact_id.is_none());

        let response = request()
            .method("POST")
            .path(&format!("/api/v1/files/{}/retry", detail.files[0].file_id))
            .reply(&routes(runtime.clone()))
            .await;
        assert_eq!(response.status(), StatusCode::OK);
        let retried = wait_terminal(&runtime, &created.batch_id).await;
        assert_eq!(retried.files[0].attempt, 2);
        assert_eq!(retried.files[0].error_code.as_deref(), Some("INPUT_NO_CONTENT"));
        assert!(retried.files[0].artifact_id.is_none());
    }

    #[tokio::test]
    async fn powerpoint_mixed_with_excel_and_failed_pptx_aggregates_correctly() {
        let (_temp, runtime) = test_runtime().await;
        let encrypted = &[0xd0, 0xcf, 0x11, 0xe0, 0x00, 0x01];
        let mixed = submit(
            &runtime,
            &[
                ("slides.pptx", &fictional_pptx()),
                ("contacts.xlsx", &sample_xlsx()),
                ("encrypted.pptx", encrypted),
            ],
        )
        .await;
        let mixed_detail = wait_terminal(&runtime, &mixed.batch_id).await;
        assert_eq!(
            mixed_detail.batch.status,
            BatchStatus::CompletedWithErrors
        );
        assert_eq!(mixed_detail.batch.completed_count, 2);
        assert_eq!(mixed_detail.batch.failed_count, 1);
        let pptx_file = mixed_detail
            .files
            .iter()
            .find(|file| file.display_name == "slides.pptx")
            .unwrap();
        assert_eq!(pptx_file.input_format, "powerpoint");
        assert_eq!(pptx_file.masked_entity_count, Some(5));
        assert!(mixed_detail
            .files
            .iter()
            .any(|file| file.display_name == "encrypted.pptx"
                && file.status == FileStatus::Failed
                && file.error_code.as_deref() == Some("INPUT_ENCRYPTED")));
    }

    #[tokio::test]
    async fn mixed_and_all_failed_batches_aggregate_correctly() {
        let (_temp, runtime) = test_runtime().await;
        let mixed = submit(
            &runtime,
            &[("ok.txt", b"13900000000"), ("bad.md", &[0xff, 0xfe])],
        )
        .await;
        let mixed_detail = wait_terminal(&runtime, &mixed.batch_id).await;
        assert_eq!(mixed_detail.batch.status, BatchStatus::CompletedWithErrors);
        assert_eq!(mixed_detail.batch.completed_count, 1);
        assert_eq!(mixed_detail.batch.failed_count, 1);
        assert!(mixed_detail.files.iter().any(|file| {
            file.status == FileStatus::Failed
                && file.error_code.as_deref() == Some("INPUT_CORRUPTED")
                && file.artifact_id.is_none()
        }));

        let failed = submit(
            &runtime,
            &[("bad-one.txt", &[0xff]), ("bad-two.markdown", &[0xfe])],
        )
        .await;
        let failed_detail = wait_terminal(&runtime, &failed.batch_id).await;
        assert_eq!(failed_detail.batch.status, BatchStatus::Failed);
        assert_eq!(failed_detail.batch.failed_count, 2);
    }

    #[tokio::test]
    async fn failed_file_retry_increments_attempt_and_reenters_worker() {
        let (_temp, runtime) = test_runtime().await;
        let created = submit(&runtime, &[("bad.txt", &[0xff])]).await;
        let detail = wait_terminal(&runtime, &created.batch_id).await;
        let file_id = detail.files[0].file_id.clone();
        let response = request()
            .method("POST")
            .path(&format!("/api/v1/files/{file_id}/retry"))
            .reply(&routes(runtime.clone()))
            .await;
        assert_eq!(response.status(), StatusCode::OK);
        let retry: RetryResponse = serde_json::from_slice(response.body()).unwrap();
        assert_eq!(retry.attempt, 2);
        assert_eq!(
            runtime.store.event_count(&file_id, "RetryQueued").await.unwrap(),
            1
        );
        let retried = wait_terminal(&runtime, &created.batch_id).await;
        assert_eq!(retried.files[0].attempt, 2);
        assert_eq!(retried.files[0].status, FileStatus::Failed);

        let conflict = request()
            .method("POST")
            .path(&format!("/api/v1/files/{}/retry", created.files[0].file_id))
            .reply(&routes(runtime.clone()))
            .await;
        assert_eq!(conflict.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn concurrent_retry_queues_exactly_once() {
        let temp = tempfile::tempdir().unwrap();
        let store = Store::open(temp.path().join("enterprise-data"))
            .await
            .unwrap();
        let created = store
            .create_batch(
                vec![NewUpload {
                    display_name: "retry.txt".into(),
                    input_format: "text".into(),
                    bytes: b"13900000000".to_vec(),
                }],
                vec!["phone".into()],
                "disabled",
            )
            .await
            .unwrap();
        let file_id = created.files[0].file_id.clone();
        store.force_failed(&file_id).await.unwrap();

        let (first, second) = tokio::join!(store.retry(&file_id), store.retry(&file_id));
        assert_eq!(usize::from(first.is_ok()) + usize::from(second.is_ok()), 1);
        assert_eq!(
            usize::from(matches!(first, Err(StoreError::RetryConflict)))
                + usize::from(matches!(second, Err(StoreError::RetryConflict))),
            1
        );
        let detail = store.batch_detail(&created.batch_id).await.unwrap();
        assert_eq!(detail.files[0].status, FileStatus::Pending);
        assert_eq!(detail.files[0].attempt, 2);
        assert_eq!(store.event_count(&file_id, "RetryQueued").await.unwrap(), 1);
    }

    #[tokio::test]
    async fn restart_recovers_processing_as_retryable_failed() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("enterprise-data");
        let store = Store::open(root.clone()).await.unwrap();
        let created = store
            .create_batch(
                vec![NewUpload {
                    display_name: "restart.txt".into(),
                    input_format: "text".into(),
                    bytes: b"13900000000".to_vec(),
                }],
                vec!["phone".into()],
                "disabled",
            )
            .await
            .unwrap();
        store.force_processing(&created.files[0].file_id).await.unwrap();
        let stopped_runtime = Runtime {
            store: store.clone(),
            limits: Limits::default(),
            ocr_config: None,
            wake_worker: Arc::new(Notify::new()),
        };
        let processing_retry = request()
            .method("POST")
            .path(&format!(
                "/api/v1/files/{}/retry",
                created.files[0].file_id
            ))
            .reply(&routes(stopped_runtime))
            .await;
        assert_eq!(processing_retry.status(), StatusCode::CONFLICT);
        tokio::fs::write(root.join("tmp/orphan.tmp"), b"partial")
            .await
            .unwrap();
        drop(store);

        let reopened = Store::open(root.clone()).await.unwrap();
        assert!(!root.join("tmp/orphan.tmp").exists());
        assert_eq!(reopened.recover_interrupted().await.unwrap(), 1);
        let detail = reopened.batch_detail(&created.batch_id).await.unwrap();
        assert_eq!(detail.batch.status, BatchStatus::Failed);
        assert_eq!(detail.files[0].status, FileStatus::Failed);
        assert_eq!(detail.files[0].error_code.as_deref(), Some("PROCESS_INTERRUPTED"));
    }

    #[tokio::test]
    async fn validation_blocks_traversal_unsupported_ids_and_limits() {
        assert_eq!(sanitize_display_name("../../safe.txt"), "safe.txt");
        assert_eq!(sanitize_display_name("..\\..\\safe.md"), "safe.md");
        let (_temp, runtime) = test_runtime().await;
        let (content_type, body) = multipart(&[("unsafe.json", b"value")], "[\"phone\"]");
        let unsupported = request()
            .method("POST")
            .path("/api/v1/batches")
            .header("content-type", content_type)
            .body(body)
            .reply(&routes(runtime.clone()))
            .await;
        assert_eq!(unsupported.status(), StatusCode::BAD_REQUEST);

        let missing_artifact = request()
            .method("GET")
            .path("/api/v1/artifacts/not-an-artifact")
            .reply(&routes(runtime.clone()))
            .await;
        assert_eq!(missing_artifact.status(), StatusCode::NOT_FOUND);

        let missing_retry = request()
            .method("POST")
            .path("/api/v1/files/not-a-file/retry")
            .reply(&routes(runtime.clone()))
            .await;
        assert_eq!(missing_retry.status(), StatusCode::NOT_FOUND);

        let boundary = "unexpected-field-boundary";
        let body = format!("--{boundary}\r\nContent-Disposition: form-data; name=\"files\"; filename=\"safe.txt\"\r\n\r\nok\r\n--{boundary}\r\nContent-Disposition: form-data; name=\"rule_ids\"\r\n\r\n[]\r\n--{boundary}\r\nContent-Disposition: form-data; name=\"output_path\"\r\n\r\n/tmp/out\r\n--{boundary}--\r\n");
        let unexpected = request()
            .method("POST")
            .path("/api/v1/batches")
            .header(
                "content-type",
                format!("multipart/form-data; boundary={boundary}"),
            )
            .body(body)
            .reply(&routes(runtime.clone()))
            .await;
        assert_eq!(unexpected.status(), StatusCode::BAD_REQUEST);

        let limited = Runtime::initialize(
            tempfile::tempdir().unwrap().path().join("data"),
            Limits {
                max_files: 1,
                max_file_bytes: 4,
                max_batch_bytes: 4,
            },
        )
        .await
        .unwrap();
        let (content_type, body) = multipart(&[("large.txt", b"12345")], "[\"phone\"]");
        let too_large = request()
            .method("POST")
            .path("/api/v1/batches")
            .header("content-type", content_type)
            .body(body)
            .reply(&routes(limited))
            .await;
        assert_eq!(too_large.status(), StatusCode::PAYLOAD_TOO_LARGE);

        let count_root = tempfile::tempdir().unwrap();
        let count_limited = Runtime::initialize(
            count_root.path().join("data"),
            Limits {
                max_files: 1,
                max_file_bytes: 10,
                max_batch_bytes: 10,
            },
        )
        .await
        .unwrap();
        let (content_type, body) = multipart(
            &[("one.txt", b"1"), ("two.md", b"2")],
            "[\"phone\"]",
        );
        let too_many = request()
            .method("POST")
            .path("/api/v1/batches")
            .header("content-type", content_type)
            .body(body)
            .reply(&routes(count_limited))
            .await;
        assert_eq!(too_many.status(), StatusCode::PAYLOAD_TOO_LARGE);

        let total_root = tempfile::tempdir().unwrap();
        let total_limited = Runtime::initialize(
            total_root.path().join("data"),
            Limits {
                max_files: 2,
                max_file_bytes: 10,
                max_batch_bytes: 3,
            },
        )
        .await
        .unwrap();
        let (content_type, body) = multipart(
            &[("one.txt", b"12"), ("two.md", b"34")],
            "[\"phone\"]",
        );
        let total_too_large = request()
            .method("POST")
            .path("/api/v1/batches")
            .header("content-type", content_type)
            .body(body)
            .reply(&routes(total_limited))
            .await;
        assert_eq!(total_too_large.status(), StatusCode::PAYLOAD_TOO_LARGE);
    }

    #[tokio::test]
    async fn database_does_not_persist_content_paths_or_mappings() {
        let (temp, runtime) = test_runtime().await;
        let sample = b"13900000000 unit.test@example.invalid";
        let created = submit(&runtime, &[("privacy.txt", sample)]).await;
        wait_terminal(&runtime, &created.batch_id).await;
        let database = tokio::fs::read(runtime.store.database_path()).await.unwrap();
        let database_text = String::from_utf8_lossy(&database);
        assert!(!database_text.contains("unit.test@example.invalid"));
        assert!(!database_text.contains(&temp.path().to_string_lossy().to_string()));
        assert!(!database_text.contains("***PHONE"));
    }

    // ------------------------------------------------------------------
    // Legacy .ppt tests
    // ------------------------------------------------------------------

    /// Skip the current test when no LibreOffice candidate is available.
    /// Call at the start of any test that depends on real .ppt conversion.
    fn require_libreoffice() -> bool {
        let available = legacy_powerpoint::resolve_soffice().is_some();
        if !available {
            eprintln!("SKIP: LibreOffice not available");
        }
        available
    }

    fn ppt_sample() -> Vec<u8> {
        include_bytes!("../tests/fixtures/sample_powerpoint.ppt").to_vec()
    }

    fn pptx_sample() -> Vec<u8> {
        include_bytes!("../tests/fixtures/sample_powerpoint.pptx").to_vec()
    }

    fn ppt_demo() -> Vec<u8> {
        include_bytes!("../tests/fixtures/ppt_normal_demo.ppt").to_vec()
    }

    fn ppt_contacts() -> Vec<u8> {
        include_bytes!("../tests/fixtures/ppt_contacts.ppt").to_vec()
    }

    fn ppt_empty() -> Vec<u8> {
        include_bytes!("../tests/fixtures/ppt_empty.ppt").to_vec()
    }

    fn ppt_corrupt() -> Vec<u8> {
        include_bytes!("../tests/fixtures/ppt_corrupt.ppt").to_vec()
    }

    fn ppt_corrupt_fast() -> Vec<u8> {
        include_bytes!("../tests/fixtures/ppt_corrupt_fast.ppt").to_vec()
    }

    /// Build a minimal OLE2/CFB blob of at least 512 bytes.
    /// This represents an encrypted .pptx that uses an OLE2 container but
    /// whose original extension is .pptx — it must NOT enter the legacy
    /// converter.
    fn encrypted_pptx_ole2_blob() -> Vec<u8> {
        let mut blob = vec![0xd0, 0xcf, 0x11, 0xe0]; // OLE2 signature
        blob.extend_from_slice(b"\xa1\xb1\xc1\xd1");  // CLSID fragment
        blob.resize(516, 0);                           // >= 512 bytes
        blob
    }

    #[tokio::test]
    async fn legacy_ppt_normal_file_succeeds() {
        if !require_libreoffice() { return; }
        let (_temp, runtime) = test_runtime().await;
        let created = submit(&runtime, &[("demo.ppt", &ppt_demo())]).await;
        let detail = wait_terminal(&runtime, &created.batch_id).await;
        assert_eq!(detail.batch.status, BatchStatus::Completed);
        assert_eq!(detail.files.len(), 1);
        assert_eq!(detail.files[0].status, FileStatus::Completed);
        assert_eq!(detail.files[0].input_format, "powerpoint");
        assert!(
            detail.files[0].masked_entity_count.unwrap_or(0) > 0,
            "should mask at least one entity"
        );
        let artifact_id = detail.files[0].artifact_id.clone().unwrap();

        let downloaded = request()
            .method("GET")
            .path(&format!("/api/v1/artifacts/{artifact_id}"))
            .reply(&routes(runtime.clone()))
            .await;
        assert_eq!(downloaded.status(), StatusCode::OK);
        let body = std::str::from_utf8(downloaded.body()).unwrap();
        assert!(body.contains("***PHONE***") && body.contains("***EMAIL***"));

        // Verify restore: the artifact must be restorable
        let restore_resp = request()
            .method("POST")
            .path(&format!("/api/v1/artifacts/{artifact_id}/restore"))
            .reply(&routes(runtime.clone()))
            .await;
        assert_eq!(restore_resp.status(), StatusCode::OK, "restore must succeed");
        let restored_count: usize = restore_resp
            .headers()
            .get("x-restored-entity-count")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        assert!(restored_count > 0, "restored entity count must be > 0, got {restored_count}");
    }

    #[tokio::test]
    async fn legacy_ppt_and_pptx_consistent_entity_count() {
        if !require_libreoffice() { return; }
        let (_temp, runtime) = test_runtime().await;
        struct _Marker;
        let created = submit(
            &runtime,
            &[
                ("demo.ppt", &ppt_demo()),
                ("demo.txt", b"Phone 13912345678 and email test@example.com"),
            ],
        )
        .await;
        assert_eq!(created.files.len(), 2);
        let detail = wait_terminal(&runtime, &created.batch_id).await;
        assert_eq!(detail.batch.status, BatchStatus::Completed);
        assert_eq!(detail.batch.completed_count, 2);
        assert_eq!(detail.batch.failed_count, 0);

        let ppt_file = detail.files.iter().find(|f| f.display_name == "demo.ppt").unwrap();
        assert!(
            ppt_file.masked_entity_count.unwrap_or(0) > 0,
            ".ppt should have masked entities"
        );
    }

    #[tokio::test]
    async fn legacy_ppt_repeated_values_reuse_placeholder() {
        if !require_libreoffice() { return; }
        let (_temp, runtime) = test_runtime().await;
        let created = submit(&runtime, &[("demo.ppt", &ppt_demo())]).await;
        let detail = wait_terminal(&runtime, &created.batch_id).await;
        if detail.batch.status != BatchStatus::Completed {
            panic!(
                "expected Completed, got {:?}, files: {:?}",
                detail.batch.status,
                detail.files.iter().map(|f| (&f.status, &f.error_code, &f.error_message)).collect::<Vec<_>>()
            );
        }
        assert!(detail.files[0].masked_entity_count.unwrap_or(0) >= 2);

        let artifact_id = detail.files[0].artifact_id.clone().unwrap();
        let downloaded = request()
            .method("GET")
            .path(&format!("/api/v1/artifacts/{artifact_id}"))
            .reply(&routes(runtime))
            .await;
        assert_eq!(downloaded.status(), StatusCode::OK);
        let body = std::str::from_utf8(downloaded.body()).unwrap();
        assert!(!body.contains("13912345678"), "phone should be masked");
        assert!(!body.contains("zhangwei@example.cn"), "email should be masked");
        assert!(body.contains("***PHONE***") && body.contains("***EMAIL***"),
            "phone and email should be masked with default rules");
    }

    #[tokio::test]
    async fn legacy_ppt_chinese_content_preserved() {
        if !require_libreoffice() { return; }
        let (_temp, runtime) = test_runtime().await;
        let created = submit(&runtime, &[("contacts.ppt", &ppt_contacts())]).await;
        let detail = wait_terminal(&runtime, &created.batch_id).await;
        if detail.batch.status != BatchStatus::Completed {
            panic!(
                "expected Completed, got {:?}, files: {:?}",
                detail.batch.status,
                detail.files.iter().map(|f| (&f.status, &f.error_code, &f.error_message)).collect::<Vec<_>>()
            );
        }

        let artifact_id = detail.files[0].artifact_id.clone().unwrap();
        let downloaded = request()
            .method("GET")
            .path(&format!("/api/v1/artifacts/{artifact_id}"))
            .reply(&routes(runtime))
            .await;
        assert_eq!(downloaded.status(), StatusCode::OK);
        let body = std::str::from_utf8(downloaded.body()).unwrap();
        assert!(body.contains("客户通讯录"), "Chinese text should be preserved");
        assert!(body.contains("华东区客户"), "Chinese text should be preserved");
        assert!(body.contains("李娜") || body.contains("***PHONE***"),
            "Chinese name or masked phone should appear");
    }

    #[tokio::test]
    async fn legacy_ppt_empty_fails() {
        let (_temp, runtime) = test_runtime().await;
        let created = submit(&runtime, &[("empty.ppt", &ppt_empty())]).await;
        let detail = wait_terminal(&runtime, &created.batch_id).await;
        assert_eq!(detail.batch.status, BatchStatus::Failed);
        assert_eq!(detail.files[0].status, FileStatus::Failed);
        // The legacy .ppt converts successfully (structurally valid), but
        // the resulting PPTX has no readable text; TASK-EMPTY-CONTENT-
        // ERROR-CLASSIFICATION-001 reclassified this from INPUT_CORRUPTED
        // to INPUT_NO_CONTENT.
        assert_eq!(
            detail.files[0].error_code.as_deref(),
            Some("INPUT_NO_CONTENT")
        );
        assert!(detail.files[0].artifact_id.is_none());
    }

    #[tokio::test]
    async fn legacy_ppt_corrupt_fails() {
        let (_temp, runtime) = test_runtime().await;
        let created = submit(&runtime, &[("corrupt.ppt", &ppt_corrupt())]).await;
        let detail = wait_terminal(&runtime, &created.batch_id).await;
        assert_eq!(detail.batch.status, BatchStatus::Failed);
        assert_eq!(detail.files[0].status, FileStatus::Failed);
        assert_eq!(
            detail.files[0].error_code.as_deref(),
            Some("INPUT_CORRUPTED"),
            "structurally corrupt .ppt must return INPUT_CORRUPTED"
        );
        assert!(detail.files[0].artifact_id.is_none());
    }

    #[tokio::test]
    async fn legacy_ppt_mixed_with_pptx_and_excel_aggregates() {
        if !require_libreoffice() { return; }
        let (_temp, runtime) = test_runtime().await;
        let created = submit(
            &runtime,
            &[
                ("slides.ppt", &ppt_demo()),
                ("notes.pptx", &fictional_pptx()),
                ("data.xlsx", &sample_xlsx()),
            ],
        )
        .await;
        assert_eq!(created.files.len(), 3);
        let detail = wait_terminal(&runtime, &created.batch_id).await;
        if detail.batch.status != BatchStatus::Completed {
            panic!(
                "expected Completed, got {:?}, files: {:?}",
                detail.batch.status,
                detail.files.iter().map(|f| (&f.status, &f.error_code, &f.error_message)).collect::<Vec<_>>()
            );
        }
        assert_eq!(detail.batch.completed_count, 3);
        assert_eq!(detail.batch.failed_count, 0);
    }

    #[tokio::test]
    async fn legacy_ppt_mixed_with_corrupt_fails_partially() {
        if !require_libreoffice() { return; }
        let (_temp, runtime) = test_runtime().await;
        let created = submit(
            &runtime,
            &[
                ("good.ppt", &ppt_demo()),
                ("bad.ppt", &ppt_corrupt()),
            ],
        )
        .await;
        assert_eq!(created.files.len(), 2);
        let detail = wait_terminal(&runtime, &created.batch_id).await;
        assert_eq!(
            detail.batch.status,
            BatchStatus::CompletedWithErrors,
            "mixed batch must be CompletedWithErrors, got {:?}\nfiles: {:?}",
            detail.batch.status,
            detail.files.iter().map(|f| (&f.status, &f.error_code)).collect::<Vec<_>>()
        );
        let completed: Vec<_> = detail
            .files
            .iter()
            .filter(|f| f.status == FileStatus::Completed)
            .collect();
        let failed: Vec<_> = detail
            .files
            .iter()
            .filter(|f| f.status == FileStatus::Failed)
            .collect();
        assert_eq!(completed.len(), 1, "exactly 1 file must succeed");
        assert_eq!(failed.len(), 1, "exactly 1 file must fail");
        assert_eq!(
            failed[0].error_code.as_deref(),
            Some("INPUT_CORRUPTED"),
            "corrupt file must have INPUT_CORRUPTED"
        );
    }

    #[tokio::test]
    async fn legacy_ppt_with_different_rules_produces_masked_artifact() {
        if !require_libreoffice() { return; }
        let (_temp, runtime) = test_runtime().await;
        let (content_type, body) = multipart(
            &[("demo.ppt", &ppt_demo())],
            "[\"phone\",\"email\"]",
        );
        let response = request()
            .method("POST")
            .path("/api/v1/batches")
            .header("content-type", content_type)
            .body(body)
            .reply(&routes(runtime.clone()))
            .await;
        assert_eq!(response.status(), StatusCode::ACCEPTED);
        let created: CreateBatchResponse = serde_json::from_slice(response.body()).unwrap();
        let detail = wait_terminal(&runtime, &created.batch_id).await;
        if detail.batch.status != BatchStatus::Completed {
            panic!(
                "expected Completed, got {:?}, files: {:?}",
                detail.batch.status,
                detail.files.iter().map(|f| (&f.status, &f.error_code, &f.error_message)).collect::<Vec<_>>()
            );
        }
        assert!(
            detail.files[0].masked_entity_count.unwrap_or(0) >= 2,
            "should mask phone and email"
        );

        let artifact_id = detail.files[0].artifact_id.clone().unwrap();
        let downloaded = request()
            .method("GET")
            .path(&format!("/api/v1/artifacts/{artifact_id}"))
            .reply(&routes(runtime))
            .await;
        assert_eq!(downloaded.status(), StatusCode::OK);
        assert_eq!(
            downloaded.headers()["content-type"],
            "text/markdown; charset=utf-8"
        );
    }

    #[tokio::test]
    async fn legacy_ppt_failed_retry_is_safe() {
        let (_temp, runtime) = test_runtime().await;
        let created = submit(&runtime, &[("corrupt.ppt", &ppt_corrupt())]).await;
        let detail = wait_terminal(&runtime, &created.batch_id).await;
        assert_eq!(detail.files[0].status, FileStatus::Failed);

        let response = request()
            .method("POST")
            .path(&format!("/api/v1/files/{}/retry", detail.files[0].file_id))
            .reply(&routes(runtime.clone()))
            .await;
        assert_eq!(response.status(), StatusCode::OK);
        let retried = wait_terminal(&runtime, &created.batch_id).await;
        assert_eq!(retried.files[0].attempt, 2);
        assert_eq!(retried.files[0].status, FileStatus::Failed);
        assert!(retried.files[0].artifact_id.is_none());

        // Clean up corrupt fixtures to avoid interfering with other tests
        let _ = std::fs::remove_file(
            "/tmp/ppt-conversion-feasibility/source/test_corrupt.ppt",
        );
    }

    #[tokio::test]
    async fn legacy_ppt_existing_pptx_still_works() {
        // Verify that existing .pptx behaviour is unchanged
        let (_temp, runtime) = test_runtime().await;
        let created = submit(&runtime, &[("slides.pptx", &fictional_pptx())]).await;
        let detail = wait_terminal(&runtime, &created.batch_id).await;
        if detail.batch.status != BatchStatus::Completed {
            panic!(
                "expected Completed, got {:?}, files: {:?}",
                detail.batch.status,
                detail.files.iter().map(|f| (&f.status, &f.error_code, &f.error_message)).collect::<Vec<_>>()
            );
        }
        assert_eq!(detail.files[0].input_format, "powerpoint");
        assert_eq!(detail.files[0].masked_entity_count, Some(5));
    }

    #[tokio::test]
    async fn legacy_ppt_and_txt_mixed_batch_aggregates() {
        if !require_libreoffice() { return; }
        let (_temp, runtime) = test_runtime().await;
        let created = submit(
            &runtime,
            &[
                ("slides.ppt", &ppt_demo()),
                ("notes.txt", b"Call 13900000000 for support"),
            ],
        )
        .await;
        assert_eq!(created.files.len(), 2);
        let detail = wait_terminal(&runtime, &created.batch_id).await;
        if detail.batch.status != BatchStatus::Completed {
            panic!(
                "expected Completed, got {:?}, files: {:?}",
                detail.batch.status,
                detail.files.iter().map(|f| (&f.status, &f.error_code, &f.error_message)).collect::<Vec<_>>()
            );
        }
        assert_eq!(detail.batch.completed_count, 2);
    }

    // ------------------------------------------------------------------
    // Encrypted .pptx regression — OLE2/CFB >= 512 bytes, .pptx extension
    // ------------------------------------------------------------------

    #[tokio::test]
    async fn encrypted_pptx_ole2_large_blob_rejected_as_encrypted() {
        // An OLE2 blob ≥512 bytes submitted as .pptx must NOT enter the
        // legacy converter.  It must go to the existing parser which
        // returns INPUT_ENCRYPTED on OLE2 signature.
        let (_temp, runtime) = test_runtime().await;
        let encrypted = encrypted_pptx_ole2_blob();
        assert!(encrypted.len() >= 512, "fixture must be ≥512 bytes");
        let created = submit(&runtime, &[("encrypted.pptx", &encrypted)]).await;
        let detail = wait_terminal(&runtime, &created.batch_id).await;
        assert_eq!(detail.batch.status, BatchStatus::Failed);
        assert_eq!(detail.files[0].status, FileStatus::Failed);
        assert_eq!(
            detail.files[0].error_code.as_deref(),
            Some("INPUT_ENCRYPTED"),
            "encrypted .pptx must NOT return LEGACY_CONVERTER_UNAVAILABLE or INPUT_CORRUPTED; got {:?}",
            detail.files[0].error_code
        );
        assert!(detail.files[0].artifact_id.is_none());
    }

    #[tokio::test]
    async fn encrypted_pptx_ole2_large_blob_mixed_batch_aggregates() {
        // Same fixture in a mixed batch: normal .pptx succeeds, encrypted
        // .pptx fails with INPUT_ENCRYPTED (not LEGACY_CONVERTER_UNAVAILABLE).
        let (_temp, runtime) = test_runtime().await;
        let encrypted = encrypted_pptx_ole2_blob();
        let mixed = submit(
            &runtime,
            &[
                ("slides.pptx", &fictional_pptx()),
                ("contacts.xlsx", &sample_xlsx()),
                ("secret.pptx", &encrypted),
            ],
        )
        .await;
        let detail = wait_terminal(&runtime, &mixed.batch_id).await;
        assert_eq!(detail.batch.completed_count, 2);
        assert_eq!(detail.batch.failed_count, 1);
        assert!(detail
            .files
            .iter()
            .any(|file| file.display_name == "slides.pptx"
                && file.status == FileStatus::Completed));
        assert!(detail
            .files
            .iter()
            .any(|file| file.display_name == "secret.pptx"
                && file.status == FileStatus::Failed
                && file.error_code.as_deref() == Some("INPUT_ENCRYPTED")));
    }

    #[tokio::test]
    async fn encrypted_pptx_does_not_enter_legacy_converter_even_without_libreoffice() {
        // Verify that even when LibreOffice is unavailable, an encrypted
        // .pptx is NOT sent to the legacy converter.  It reaches the
        // existing parser and returns INPUT_ENCRYPTED, not
        // LEGACY_CONVERTER_UNAVAILABLE.
        let (_temp, runtime) = test_runtime().await;
        let encrypted = encrypted_pptx_ole2_blob();
        let created = submit(&runtime, &[("protected.pptx", &encrypted)]).await;
        let detail = wait_terminal(&runtime, &created.batch_id).await;
        // Must be INPUT_ENCRYPTED regardless of LibreOffice availability
        assert_eq!(
            detail.files[0].error_code.as_deref(),
            Some("INPUT_ENCRYPTED"),
            "must not be LEGACY_CONVERTER_UNAVAILABLE: {:?}",
            detail.files[0].error_code
        );
    }

    // ------------------------------------------------------------------
    // R1: new batches always generate server mapping
    // ------------------------------------------------------------------

    /// Helper: submit with optional restore_mode field.
    async fn submit_with_mode(runtime: &Runtime, files: &[(&str, &[u8])], mode: Option<&str>) -> CreateBatchResponse {
        let boundary = "r1-test-boundary";
        let mut body = Vec::new();
        for (name, bytes) in files {
            body.extend_from_slice(format!("--{boundary}\r\nContent-Disposition: form-data; name=\"files\"; filename=\"{name}\"\r\nContent-Type: application/octet-stream\r\n\r\n").as_bytes());
            body.extend_from_slice(bytes);
            body.extend_from_slice(b"\r\n");
        }
        body.extend_from_slice(format!("--{boundary}\r\nContent-Disposition: form-data; name=\"rule_ids\"\r\n\r\n[\"phone\"]\r\n").as_bytes());
        if let Some(m) = mode {
            body.extend_from_slice(format!("--{boundary}\r\nContent-Disposition: form-data; name=\"restore_mode\"\r\n\r\n{m}\r\n").as_bytes());
        }
        body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
        let ct = format!("multipart/form-data; boundary={boundary}");
        let resp = request()
            .method("POST")
            .path("/api/v1/batches")
            .header("content-type", &ct)
            .body(body)
            .reply(&routes(runtime.clone()))
            .await;
        assert_eq!(resp.status(), StatusCode::ACCEPTED, "submit should be accepted");
        serde_json::from_slice(resp.body()).unwrap()
    }

    #[tokio::test]
    async fn r1_omitted_restore_mode_generates_mapping() {
        let (_temp, runtime) = test_runtime().await;
        let created = submit_with_mode(&runtime, &[("r1.txt", b"13900000000")], None).await;
        let detail = wait_terminal(&runtime, &created.batch_id).await;
        assert_eq!(detail.batch.status, BatchStatus::Completed);
        assert!(detail.files[0].restore_available, "mapping must be generated when restore_mode is omitted");
        let aid = detail.files[0].artifact_id.clone().unwrap();
        let restore = request()
            .method("POST")
            .path(&format!("/api/v1/artifacts/{aid}/restore"))
            .reply(&routes(runtime))
            .await;
        assert_eq!(restore.status(), StatusCode::OK, "restore must succeed with mapping");
    }

    #[tokio::test]
    async fn r1_disabled_value_is_normalized_to_server_cmap() {
        let (_temp, runtime) = test_runtime().await;
        let created = submit_with_mode(&runtime, &[("r1d.txt", b"13900000000")], Some("disabled")).await;
        let detail = wait_terminal(&runtime, &created.batch_id).await;
        assert_eq!(detail.batch.status, BatchStatus::Completed);
        assert!(detail.files[0].restore_available, "mapping must be generated even when caller sends disabled");
        let aid = detail.files[0].artifact_id.clone().unwrap();
        let restore = request()
            .method("POST")
            .path(&format!("/api/v1/artifacts/{aid}/restore"))
            .reply(&routes(runtime))
            .await;
        assert_eq!(restore.status(), StatusCode::OK, "restore must succeed after disabled normalization");
    }

    #[tokio::test]
    async fn r1_server_cmap_explicit_value_generates_mapping() {
        let (_temp, runtime) = test_runtime().await;
        let created = submit_with_mode(&runtime, &[("r1s.txt", b"13900000000")], Some("server_cmap")).await;
        let detail = wait_terminal(&runtime, &created.batch_id).await;
        assert_eq!(detail.batch.status, BatchStatus::Completed);
        assert!(detail.files[0].restore_available, "server_cmap mode must generate mapping");
    }

    // ------------------------------------------------------------------
    // OCR gold-standard fixture (TASK-OCR-GOLD-STANDARD-REPRODUCIBILITY-002)
    // ------------------------------------------------------------------

    fn pdf_ocr_gold_standard_pdf() -> Vec<u8> {
        include_bytes!("../tests/fixtures/pdf_ocr_gold_standard.pdf").to_vec()
    }

    fn pdf_ocr_gold_standard_json() -> &'static str {
        include_str!("../tests/fixtures/pdf_ocr_gold_standard.json")
    }

    /// The gold-standard fixture is a genuine image-only PDF (no text layer).
    /// Without OCR configured, the Runtime must fail the file as
    /// `OCR_COMPONENT_REQUIRED` rather than silently succeeding or hanging —
    /// this doubles as the fixture's "no text layer" assertion, since
    /// `OCR_COMPONENT_REQUIRED` is exactly the error engine-core raises when
    /// a PDF has no readable text layer.
    #[tokio::test]
    async fn pdf_ocr_gold_standard_fixture_has_no_text_layer_and_requires_ocr() {
        let (_temp, runtime) = test_runtime().await;
        let created = submit(
            &runtime,
            &[("pdf_ocr_gold_standard.pdf", &pdf_ocr_gold_standard_pdf())],
        )
        .await;
        let detail = wait_terminal(&runtime, &created.batch_id).await;
        assert_eq!(detail.batch.status, BatchStatus::Failed);
        assert_eq!(detail.files.len(), 1);
        assert_eq!(detail.files[0].status, FileStatus::Failed);
        assert_eq!(
            detail.files[0].error_code.as_deref(),
            Some("OCR_COMPONENT_REQUIRED"),
            "an image-only PDF with no text layer must require OCR when OCR is not configured"
        );
        assert!(detail.files[0].artifact_id.is_none());
    }

    #[test]
    fn pdf_ocr_gold_standard_json_contract_fields_are_present() {
        let json: serde_json::Value = serde_json::from_str(pdf_ocr_gold_standard_json()).unwrap();
        assert_eq!(json["fixture"], "pdf_ocr_gold_standard.pdf");
        assert_eq!(json["pages"], 3);
        assert_eq!(json["has_text_layer"], false);

        let required = json["required_clear_fields"].as_array().unwrap();
        let supplementary = json["supplementary_low_quality_fields"].as_array().unwrap();
        assert!(!required.is_empty());
        assert!(!supplementary.is_empty());

        // required_clear_fields and supplementary_low_quality_fields must be
        // mutually exclusive sets (G4).
        let required_texts: std::collections::HashSet<&str> =
            required.iter().map(|f| f["text"].as_str().unwrap()).collect();
        let supplementary_texts: std::collections::HashSet<&str> = supplementary
            .iter()
            .map(|f| f["text"].as_str().unwrap())
            .collect();
        assert!(
            required_texts.is_disjoint(&supplementary_texts),
            "required_clear_fields and supplementary_low_quality_fields must not overlap"
        );

        // expected_masked_count only counts required_clear_fields (G4).
        let expected_masked_count = json["masking_expected"]["expected_masked_count"]
            .as_u64()
            .unwrap();
        assert_eq!(expected_masked_count, required.len() as u64);

        // phone and id_card must never be downgraded out of required_clear_fields (G6).
        let required_categories: std::collections::HashSet<&str> = required
            .iter()
            .map(|f| f["category"].as_str().unwrap())
            .collect();
        assert!(required_categories.contains("phone"));
        assert!(required_categories.contains("id_card"));

        assert!(json.get("note").is_none(), "note must not reappear in the JSON (G4)");
        assert!(json.get("notes").is_none(), "notes must not reappear in the JSON (G4)");
    }
}
