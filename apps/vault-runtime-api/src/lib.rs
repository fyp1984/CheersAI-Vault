mod filebay;
mod legacy_powerpoint;
mod operation_logs;
mod preview;
mod processing;
mod sandbox;
mod sensitive_terms;
mod store;

use std::{convert::Infallible, path::PathBuf, sync::Arc, time::Duration};

use bytes::{BufMut, BytesMut};
use component_runtime::{preflight_check, OcrConfig};
use engine_core::{get_builtin_rules, FormatCatalog, InputFormat};
#[cfg(test)]
use engine_core::{MaskingRequest, MaskingService};
use futures_util::TryStreamExt;
use service_contracts::{
    BatchListResponse, ErrorResponse, HealthResponse, RuleMetadata, RulesResponse,
};
use tokio::sync::Notify;
use warp::{http::StatusCode, multipart::FormData, Filter, Rejection, Reply};

use store::{NewUpload, PendingJob, Store, StoreError};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

const ENTERPRISE_RULE_IDS: &[&str] =
    &["id_card", "phone", "email", "bank_card", "ipv4", "passport"];

use store::SENSITIVE_TERMS_RULE_ID;

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

/// Production preview TTL (30 minutes). Tests inject a shorter [`Duration`]
/// directly through [`Runtime::initialize_with_preview_ttl`] instead of a new
/// environment variable.
const DEFAULT_PREVIEW_TTL: Duration = Duration::from_secs(30 * 60);

#[derive(Clone)]
pub struct Runtime {
    store: Store,
    limits: Limits,
    ocr_config: Option<OcrConfig>,
    wake_worker: Arc<Notify>,
    wake_preview_worker: Arc<Notify>,
    preview_ttl: Duration,
    /// Incremented once per `processing::process_input` call site invocation
    /// (batch worker and preview worker). Used by tests to falsify "confirm
    /// does not reprocess" (B4): the count must be unchanged across a confirm
    /// call once a preview has reached a terminal state.
    processing_call_count: Arc<std::sync::atomic::AtomicUsize>,
    /// The single shared sandbox/PIN session for this Runtime process — one
    /// server system user, one PIN, one `locked` state shared by every
    /// browser session that reaches this Runtime. Never gates any other
    /// route in this file.
    sandbox: Arc<sandbox::SandboxSession>,
    /// The single FileBay admin-environment session for this Runtime
    /// process — configuration is read once at startup (`filebay::FileBaySession::from_env`)
    /// and never changes for the lifetime of the process.
    filebay: Arc<filebay::FileBaySession>,
}

impl Runtime {
    pub async fn initialize(data_root: PathBuf, limits: Limits) -> Result<Self, String> {
        Self::initialize_with_preview_ttl(data_root, limits, DEFAULT_PREVIEW_TTL).await
    }

    /// Same as [`Self::initialize`] but with an injectable preview TTL for
    /// tests (D2). Production code must always call [`Self::initialize`],
    /// which hard-codes the 30-minute TTL; no new environment variable is
    /// introduced for this.
    pub async fn initialize_with_preview_ttl(
        data_root: PathBuf,
        limits: Limits,
        preview_ttl: Duration,
    ) -> Result<Self, String> {
        let runtime = Self::build(data_root, limits, preview_ttl).await?;
        runtime.spawn_worker();
        runtime.spawn_preview_worker();
        runtime.spawn_preview_sweeper(preview_ttl);
        Ok(runtime)
    }

    /// Test-only initialization that performs the same store recovery,
    /// preview wipe and OCR config setup as production, but does **not**
    /// spawn background workers. This lets tests that manually drive store
    /// state (e.g. operation-log helpers) own the only claimer of pending
    /// jobs, eliminating the race with `spawn_worker`. This method is gated
    /// by `#[cfg(test)]` so it cannot be selected by production code,
    /// environment variables, HTTP parameters or public API.
    #[cfg(test)]
    pub async fn initialize_without_workers(
        data_root: PathBuf,
        limits: Limits,
    ) -> Result<Self, String> {
        Self::build(data_root, limits, DEFAULT_PREVIEW_TTL).await
    }

    async fn build(
        data_root: PathBuf,
        limits: Limits,
        preview_ttl: Duration,
    ) -> Result<Self, String> {
        // Computed before `data_root` is moved into `Store::open` below.
        // Fixed internal location within the Runtime's own data root — never
        // exposed to the API or logs (安全约束 4).
        let sandbox_dir = data_root.join("sandbox");
        std::fs::create_dir_all(&sandbox_dir)
            .map_err(|_| "Runtime sandbox directory initialization failed".to_string())?;
        let sandbox = Arc::new(sandbox::SandboxSession::new(sandbox_dir.join("pin.phc"))?);

        let store = Store::open(data_root)
            .await
            .map_err(|_| "Runtime storage initialization failed".to_string())?;
        store
            .recover_interrupted()
            .await
            .map_err(|_| "Runtime recovery failed".to_string())?;
        // Preview sessions never survive a restart (D3): unconditionally wipe
        // any preview/ data left behind by a previous, possibly-crashed run.
        store
            .wipe_all_previews()
            .await
            .map_err(|_| "Runtime preview cleanup failed".to_string())?;

        // Explicit OCR paths take priority. When all OCR path variables are
        // absent, the shared component resolver may reuse the current user's
        // existing desktop installation without copying or downloading it.
        let ocr_config = build_ocr_config_from_env();

        if let Some(ref cfg) = ocr_config {
            let status = preflight_check(cfg);
            eprintln!("OCR component status: {}", status.as_str());
        } else {
            eprintln!("OCR component status: unavailable");
        }

        Ok(Self {
            store,
            limits,
            ocr_config,
            wake_worker: Arc::new(Notify::new()),
            wake_preview_worker: Arc::new(Notify::new()),
            preview_ttl,
            processing_call_count: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            sandbox,
            filebay: Arc::new(filebay::FileBaySession::from_env()),
        })
    }

    #[cfg(test)]
    fn processing_call_count(&self) -> usize {
        self.processing_call_count
            .load(std::sync::atomic::Ordering::SeqCst)
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

        self.processing_call_count
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let result = match processing::process_input(processing::ProcessingInput {
            bytes: &bytes,
            display_name: &job.display_name,
            input_format,
            rule_ids: &job.rules,
            ocr_config: self.ocr_config.as_ref(),
        })
        .await
        {
            Ok(result) => result,
            Err(failure) => {
                let _ = self
                    .store
                    .mark_failed(&job, &failure.code, &failure.message)
                    .await;
                return;
            }
        };

        // Pre-generate artifact_id for internal cmap consistency
        let artifact_id = uuid::Uuid::new_v4().to_string();

        // For server_cmap mode: pre-encode mapping with the same artifact_id
        let mapping_bytes = if job.restore_mode == "server_cmap" {
            use engine_core::{
                encode_server_cmap, ServerCmap, SERVER_CMAP_MAGIC, SERVER_CMAP_VERSION,
            };
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
            let mut rules: Vec<RuleMetadata> = get_builtin_rules()
                .iter()
                .filter(|rule| ENTERPRISE_RULE_IDS.contains(&rule.id.as_str()))
                .map(|rule| RuleMetadata {
                    id: rule.id.clone(),
                    name: rule.name.clone(),
                    enabled_by_default: rule.enabled,
                })
                .collect();
            // Special metadata entry for the sensitive-term library (7.1):
            // never a per-term listing, and enabled by default to preserve
            // the desktop RuleSelector's "sensitive terms auto-enabled"
            // product semantics (7.2).
            rules.push(RuleMetadata {
                id: SENSITIVE_TERMS_RULE_ID.to_string(),
                name: "敏感词库".to_string(),
                enabled_by_default: true,
            });
            warp::reply::json(&RulesResponse { rules })
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
        .and(runtime_filter.clone())
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
        .or(preview::routes(runtime_filter.clone()))
        .or(sensitive_terms::routes(runtime_filter.clone()))
        .or(operation_logs::routes(runtime_filter.clone()))
        .or(sandbox::routes(runtime_filter.clone()))
        .or(filebay::routes(runtime_filter))
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

async fn parse_form(
    mut form: FormData,
    limits: &Limits,
) -> Result<(Vec<NewUpload>, Vec<String>, String), Rejection> {
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
            let definition =
                FormatCatalog::enterprise_from_filename(&display_name).map_err(|error| {
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
    if parsed.iter().any(|rule| {
        !ENTERPRISE_RULE_IDS.contains(&rule.as_str()) && rule != SENSITIVE_TERMS_RULE_ID
    }) {
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
            if character.is_control()
                || matches!(character, ':' | '*' | '?' | '"' | '<' | '>' | '|')
            {
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

/// Build OCR configuration through the shared resolver, then apply the
/// Runtime-only scalar limits from the environment. Path variables are
/// explicit as a group; when all three are absent, the resolver performs the
/// same-user desktop-installation discovery used by the Tauri app.
///   `VAULT_OCR_TIMEOUT`    — OCR process timeout in seconds (default 300)
///   `VAULT_OCR_MAX_PAGES`  — maximum pages to process (default 200)
fn build_ocr_config_from_env() -> Option<OcrConfig> {
    let mut config = component_runtime::resolve_ocr_config()?;

    config.timeout = std::env::var("VAULT_OCR_TIMEOUT")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .map(Duration::from_secs)
        .unwrap_or_else(|| Duration::from_secs(300));

    config.max_pages = std::env::var("VAULT_OCR_MAX_PAGES")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(200);

    config.max_pixels_per_page = std::env::var("VAULT_OCR_MAX_PIXELS_PER_PAGE")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(OcrConfig::default().max_pixels_per_page);
    config.max_total_pixels = config.max_pages as u64 * config.max_pixels_per_page;

    Some(config)
}

async fn ocr_status_handler(runtime: Runtime) -> Result<impl Reply, Rejection> {
    let (status, model_ready, timeout, max_pages) = match &runtime.ocr_config {
        Some(config) => {
            let status = preflight_check(config);
            (
                status.as_str(),
                status.is_ready(),
                config.timeout.as_secs(),
                config.max_pages,
            )
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
    let batches = runtime
        .store
        .list_batches()
        .await
        .map_err(store_rejection)?;
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
        StoreError::PreviewAlreadyConfirmed => api_error(
            StatusCode::BAD_REQUEST,
            "PREVIEW_ALREADY_CONFIRMED",
            "This preview has already been confirmed into a batch",
            false,
        ),
        // Sensitive-term-specific variants are only ever produced by the
        // sensitive-term store methods, which route their errors through
        // `sensitive_terms::sensitive_term_rejection` instead — this arm
        // exists solely for match exhaustiveness on the shared `StoreError`
        // enum and is not expected to be reached from batch/preview handlers.
        StoreError::SensitiveTermInvalid(_)
        | StoreError::SensitiveTermDuplicate
        | StoreError::SensitiveTermNotFound
        | StoreError::SensitiveTermsImportInvalid(_)
        | StoreError::InputLimitExceeded => api_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "RUNTIME_STORAGE_FAILED",
            "Runtime storage operation failed",
            true,
        ),
    }
}

async fn artifact_restore_handler(
    artifact_id: String,
    runtime: Runtime,
) -> Result<impl Reply, Rejection> {
    use engine_core::{decode_server_cmap, restore_markdown};

    let (markdown_bytes, mapping_bytes) =
        match runtime.store.artifact_with_mapping(&artifact_id).await {
            Ok(pair) => pair,
            Err(_) => {
                return Err(api_error(
                    StatusCode::NOT_FOUND,
                    "NOT_FOUND",
                    "Artifact or mapping not found",
                    false,
                ))
            }
        };

    let cmap = match decode_server_cmap(&mapping_bytes) {
        Ok(c) => c,
        Err(e) => {
            let _ = runtime
                .store
                .log_restore_event("RestoreFailed", "failed", Some(e.error_code()), None)
                .await;
            return Err(api_error(
                StatusCode::BAD_REQUEST,
                "CMAP_MISMATCH",
                "Mapping data is invalid",
                false,
            ));
        }
    };

    // Safety: verify the cmap bind to the requested artifact
    if cmap.artifact_id != artifact_id {
        let _ = runtime
            .store
            .log_restore_event(
                "RestoreFailed",
                "failed",
                Some("ARTIFACT_ID_MISMATCH"),
                None,
            )
            .await;
        return Err(api_error(
            StatusCode::BAD_REQUEST,
            "CMAP_MISMATCH",
            "Mapping bind to a different artifact",
            false,
        ));
    }

    let masked_text = match String::from_utf8(markdown_bytes) {
        Ok(t) => t,
        Err(_) => {
            return Err(api_error(
                StatusCode::BAD_REQUEST,
                "INPUT_CORRUPTED",
                "Artifact is not valid UTF-8",
                false,
            ))
        }
    };

    let (restored_text, count) = restore_markdown(&masked_text, &cmap.mappings);

    if count == 0 {
        let _ = runtime
            .store
            .log_restore_event("RestoreFailed", "failed", Some("CMAP_MISMATCH"), None)
            .await;
        return Err(api_error(
            StatusCode::BAD_REQUEST,
            "CMAP_MISMATCH",
            "No replacements possible",
            false,
        ));
    }

    let _ = runtime
        .store
        .log_restore_event("RestoreSucceeded", "completed", None, Some(count))
        .await;

    let fname = format!("restored-{}.md", artifact_id);
    let resp = warp::http::Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "text/markdown; charset=utf-8")
        .header(
            "content-disposition",
            format!("attachment; filename=\"{}\"", fname),
        )
        .header("X-Restored-Entity-Count", count.to_string())
        .body(restored_text.into_bytes())
        .expect("valid response");
    Ok(resp)
}

async fn handle_rejection(rejection: Rejection) -> Result<Box<dyn Reply>, Infallible> {
    if let Some(limited) = rejection.find::<sandbox::SandboxRateLimited>() {
        let response = ErrorResponse {
            code: "SANDBOX_PIN_RATE_LIMITED".to_string(),
            message: "Too many failed sandbox PIN attempts; try again later".to_string(),
            retryable: true,
        };
        let reply =
            warp::reply::with_status(warp::reply::json(&response), StatusCode::TOO_MANY_REQUESTS);
        let reply = warp::reply::with_header(
            reply,
            "Retry-After",
            limited.retry_after_seconds.to_string(),
        );
        return Ok(Box::new(reply));
    }

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
    Ok(Box::new(warp::reply::with_status(
        warp::reply::json(&response),
        status,
    )))
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
        for filename in ["fixture.doc", "fixture.json", "fixture.png"] {
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
        BatchDetail, BatchListResponse, BatchStatus, ClearOperationLogsResponse,
        CreateBatchResponse, FileStatus, OperationLogListResponse, OperationLogStatistics,
        OperationLogStorageStatus, RetryResponse,
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

    async fn test_runtime_without_ocr() -> (TempDir, Runtime) {
        let temp = tempfile::tempdir().unwrap();
        let mut runtime = Runtime::build(
            temp.path().join("enterprise-data"),
            Limits::default(),
            DEFAULT_PREVIEW_TTL,
        )
        .await
        .unwrap();
        runtime.ocr_config = None;
        runtime.spawn_worker();
        (temp, runtime)
    }

    /// Returns a Runtime that does **not** spawn background workers. Use this
    /// for tests that manually create batches and call `claim_next_pending()`
    /// (e.g. operation-log tests via `create_and_claim`), so the test helper
    /// is the only claimer and cannot race with `spawn_worker`.
    async fn test_runtime_without_workers() -> (TempDir, Runtime) {
        let temp = tempfile::tempdir().unwrap();
        let runtime = Runtime::initialize_without_workers(
            temp.path().join("enterprise-data"),
            Limits::default(),
        )
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
        // 7.1: the enterprise builtin rules plus the special sensitive-term
        // library metadata entry, appended last.
        let expected_ids = [
            "id_card",
            "phone",
            "email",
            "bank_card",
            "ipv4",
            "passport",
            "use_sensitive_terms",
        ];
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
            &[
                ("first.txt", first),
                ("second.md", second),
                ("third.csv", third),
            ],
        )
        .await;
        assert_eq!(created.files.len(), 3);
        let detail = wait_terminal(&runtime, &created.batch_id).await;
        if detail.batch.status != BatchStatus::Completed {
            panic!(
                "expected Completed, got {:?}, files: {:?}",
                detail.batch.status,
                detail
                    .files
                    .iter()
                    .map(|f| (&f.status, &f.error_code, &f.error_message))
                    .collect::<Vec<_>>()
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
        assert_eq!(
            file_detail(&created.files[0].file_id).masked_entity_count,
            Some(2)
        );
        assert_eq!(
            file_detail(&created.files[1].file_id).masked_entity_count,
            Some(2)
        );
        assert_eq!(
            file_detail(&created.files[2].file_id).masked_entity_count,
            Some(1)
        );
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
            .path(&format!("/api/v1/files/{}/retry", first_file.file_id))
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
                detail
                    .files
                    .iter()
                    .map(|f| (&f.status, &f.error_code, &f.error_message))
                    .collect::<Vec<_>>()
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
        assert_eq!(
            downloaded.headers()["content-type"],
            "text/markdown; charset=utf-8"
        );
        let body = std::str::from_utf8(downloaded.body()).unwrap();
        assert!(body.contains("| Column 1 | Column 2 | Column 3 |"));
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
    async fn malformed_csv_fails_without_an_artifact_and_retry_is_safe() {
        let (_temp, runtime) = test_runtime().await;
        let created = submit(&runtime, &[("broken.csv", b"a,b\n\"unclosed")]).await;
        let detail = wait_terminal(&runtime, &created.batch_id).await;
        assert_eq!(detail.batch.status, BatchStatus::Failed);
        assert_eq!(detail.files[0].status, FileStatus::Failed);
        assert_eq!(
            detail.files[0].error_code.as_deref(),
            Some("INPUT_CORRUPTED")
        );
        assert!(detail.files[0].artifact_id.is_none());

        let response = request()
            .method("POST")
            .path(&format!("/api/v1/files/{}/retry", detail.files[0].file_id))
            .reply(&routes(runtime.clone()))
            .await;
        assert_eq!(response.status(), StatusCode::OK);
        let retried = wait_terminal(&runtime, &created.batch_id).await;
        assert_eq!(retried.files[0].attempt, 2);
        assert_eq!(
            retried.files[0].error_code.as_deref(),
            Some("INPUT_CORRUPTED")
        );
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
        assert_eq!(
            detail.files[0].error_code.as_deref(),
            Some("INPUT_CORRUPTED")
        );
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
                detail
                    .files
                    .iter()
                    .map(|f| (&f.status, &f.error_code, &f.error_message))
                    .collect::<Vec<_>>()
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
        assert_eq!(
            downloaded.headers()["content-type"],
            "text/markdown; charset=utf-8"
        );
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
                detail
                    .files
                    .iter()
                    .map(|f| (&f.status, &f.error_code, &f.error_message))
                    .collect::<Vec<_>>()
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
        assert_eq!(
            detail.files[0].error_code.as_deref(),
            Some("INPUT_ENCRYPTED")
        );
        assert!(detail.files[0].artifact_id.is_none());

        let response = request()
            .method("POST")
            .path(&format!("/api/v1/files/{}/retry", detail.files[0].file_id))
            .reply(&routes(runtime.clone()))
            .await;
        assert_eq!(response.status(), StatusCode::OK);
        let retried = wait_terminal(&runtime, &created.batch_id).await;
        assert_eq!(retried.files[0].attempt, 2);
        assert_eq!(
            retried.files[0].error_code.as_deref(),
            Some("INPUT_ENCRYPTED")
        );
        assert!(retried.files[0].artifact_id.is_none());
    }

    #[tokio::test]
    async fn excel_mixed_with_csv_batch_aggregates_correctly() {
        let (_temp, runtime) = test_runtime().await;
        let csv = "name,phone\nAlice,13900000000\n";
        let mixed = submit(
            &runtime,
            &[
                ("contacts.xlsx", &sample_xlsx()),
                ("contacts.csv", csv.as_bytes()),
            ],
        )
        .await;
        let mixed_detail = wait_terminal(&runtime, &mixed.batch_id).await;
        assert_eq!(mixed_detail.batch.status, BatchStatus::Completed);
        assert_eq!(mixed_detail.batch.completed_count, 2);
        assert_eq!(mixed_detail.batch.failed_count, 0);
        assert!(mixed_detail
            .files
            .iter()
            .all(|file| file.status == FileStatus::Completed));
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
                detail
                    .files
                    .iter()
                    .map(|f| (&f.status, &f.error_code, &f.error_message))
                    .collect::<Vec<_>>()
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
        assert_eq!(
            downloaded.headers()["content-type"],
            "text/markdown; charset=utf-8"
        );
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
        assert_eq!(
            detail.files[0].error_code.as_deref(),
            Some("INPUT_NO_CONTENT")
        );
        assert!(detail.files[0].artifact_id.is_none());

        let response = request()
            .method("POST")
            .path(&format!("/api/v1/files/{}/retry", detail.files[0].file_id))
            .reply(&routes(runtime.clone()))
            .await;
        assert_eq!(response.status(), StatusCode::OK);
        let retried = wait_terminal(&runtime, &created.batch_id).await;
        assert_eq!(retried.files[0].attempt, 2);
        assert_eq!(
            retried.files[0].error_code.as_deref(),
            Some("INPUT_NO_CONTENT")
        );
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
        assert_eq!(mixed_detail.batch.status, BatchStatus::CompletedWithErrors);
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
            runtime
                .store
                .event_count(&file_id, "RetryQueued")
                .await
                .unwrap(),
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
        store
            .force_processing(&created.files[0].file_id)
            .await
            .unwrap();
        let sandbox_dir = root.join("sandbox");
        std::fs::create_dir_all(&sandbox_dir).unwrap();
        let stopped_runtime = Runtime {
            store: store.clone(),
            limits: Limits::default(),
            ocr_config: None,
            wake_worker: Arc::new(Notify::new()),
            wake_preview_worker: Arc::new(Notify::new()),
            preview_ttl: DEFAULT_PREVIEW_TTL,
            processing_call_count: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            sandbox: Arc::new(sandbox::SandboxSession::new(sandbox_dir.join("pin.phc")).unwrap()),
            filebay: Arc::new(filebay::FileBaySession::from_env()),
        };
        let processing_retry = request()
            .method("POST")
            .path(&format!("/api/v1/files/{}/retry", created.files[0].file_id))
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
        assert_eq!(
            detail.files[0].error_code.as_deref(),
            Some("PROCESS_INTERRUPTED")
        );
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
        let (content_type, body) = multipart(&[("one.txt", b"1"), ("two.md", b"2")], "[\"phone\"]");
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
        let (content_type, body) =
            multipart(&[("one.txt", b"12"), ("two.md", b"34")], "[\"phone\"]");
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
        let database = tokio::fs::read(runtime.store.database_path())
            .await
            .unwrap();
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
        blob.extend_from_slice(b"\xa1\xb1\xc1\xd1"); // CLSID fragment
        blob.resize(516, 0); // >= 512 bytes
        blob
    }

    #[tokio::test]
    async fn legacy_ppt_normal_file_succeeds() {
        if !require_libreoffice() {
            return;
        }
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
        assert_eq!(
            restore_resp.status(),
            StatusCode::OK,
            "restore must succeed"
        );
        let restored_count: usize = restore_resp
            .headers()
            .get("x-restored-entity-count")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        assert!(
            restored_count > 0,
            "restored entity count must be > 0, got {restored_count}"
        );
    }

    #[tokio::test]
    async fn legacy_ppt_and_pptx_consistent_entity_count() {
        if !require_libreoffice() {
            return;
        }
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

        let ppt_file = detail
            .files
            .iter()
            .find(|f| f.display_name == "demo.ppt")
            .unwrap();
        assert!(
            ppt_file.masked_entity_count.unwrap_or(0) > 0,
            ".ppt should have masked entities"
        );
    }

    #[tokio::test]
    async fn legacy_ppt_repeated_values_reuse_placeholder() {
        if !require_libreoffice() {
            return;
        }
        let (_temp, runtime) = test_runtime().await;
        let created = submit(&runtime, &[("demo.ppt", &ppt_demo())]).await;
        let detail = wait_terminal(&runtime, &created.batch_id).await;
        if detail.batch.status != BatchStatus::Completed {
            panic!(
                "expected Completed, got {:?}, files: {:?}",
                detail.batch.status,
                detail
                    .files
                    .iter()
                    .map(|f| (&f.status, &f.error_code, &f.error_message))
                    .collect::<Vec<_>>()
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
        assert!(
            !body.contains("zhangwei@example.cn"),
            "email should be masked"
        );
        assert!(
            body.contains("***PHONE***") && body.contains("***EMAIL***"),
            "phone and email should be masked with default rules"
        );
    }

    #[tokio::test]
    async fn legacy_ppt_chinese_content_preserved() {
        if !require_libreoffice() {
            return;
        }
        let (_temp, runtime) = test_runtime().await;
        let created = submit(&runtime, &[("contacts.ppt", &ppt_contacts())]).await;
        let detail = wait_terminal(&runtime, &created.batch_id).await;
        if detail.batch.status != BatchStatus::Completed {
            panic!(
                "expected Completed, got {:?}, files: {:?}",
                detail.batch.status,
                detail
                    .files
                    .iter()
                    .map(|f| (&f.status, &f.error_code, &f.error_message))
                    .collect::<Vec<_>>()
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
        assert!(
            body.contains("客户通讯录"),
            "Chinese text should be preserved"
        );
        assert!(
            body.contains("华东区客户"),
            "Chinese text should be preserved"
        );
        assert!(
            body.contains("李娜") || body.contains("***PHONE***"),
            "Chinese name or masked phone should appear"
        );
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
        if !require_libreoffice() {
            return;
        }
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
                detail
                    .files
                    .iter()
                    .map(|f| (&f.status, &f.error_code, &f.error_message))
                    .collect::<Vec<_>>()
            );
        }
        assert_eq!(detail.batch.completed_count, 3);
        assert_eq!(detail.batch.failed_count, 0);
    }

    #[tokio::test]
    async fn legacy_ppt_mixed_with_corrupt_fails_partially() {
        if !require_libreoffice() {
            return;
        }
        let (_temp, runtime) = test_runtime().await;
        let created = submit(
            &runtime,
            &[("good.ppt", &ppt_demo()), ("bad.ppt", &ppt_corrupt())],
        )
        .await;
        assert_eq!(created.files.len(), 2);
        let detail = wait_terminal(&runtime, &created.batch_id).await;
        assert_eq!(
            detail.batch.status,
            BatchStatus::CompletedWithErrors,
            "mixed batch must be CompletedWithErrors, got {:?}\nfiles: {:?}",
            detail.batch.status,
            detail
                .files
                .iter()
                .map(|f| (&f.status, &f.error_code))
                .collect::<Vec<_>>()
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
        if !require_libreoffice() {
            return;
        }
        let (_temp, runtime) = test_runtime().await;
        let (content_type, body) = multipart(&[("demo.ppt", &ppt_demo())], "[\"phone\",\"email\"]");
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
                detail
                    .files
                    .iter()
                    .map(|f| (&f.status, &f.error_code, &f.error_message))
                    .collect::<Vec<_>>()
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
        let _ = std::fs::remove_file("/tmp/ppt-conversion-feasibility/source/test_corrupt.ppt");
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
                detail
                    .files
                    .iter()
                    .map(|f| (&f.status, &f.error_code, &f.error_message))
                    .collect::<Vec<_>>()
            );
        }
        assert_eq!(detail.files[0].input_format, "powerpoint");
        assert_eq!(detail.files[0].masked_entity_count, Some(5));
    }

    #[tokio::test]
    async fn legacy_ppt_and_txt_mixed_batch_aggregates() {
        if !require_libreoffice() {
            return;
        }
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
                detail
                    .files
                    .iter()
                    .map(|f| (&f.status, &f.error_code, &f.error_message))
                    .collect::<Vec<_>>()
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
        assert!(
            detail
                .files
                .iter()
                .any(|file| file.display_name == "slides.pptx"
                    && file.status == FileStatus::Completed)
        );
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
    async fn submit_with_mode(
        runtime: &Runtime,
        files: &[(&str, &[u8])],
        mode: Option<&str>,
    ) -> CreateBatchResponse {
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
        assert_eq!(
            resp.status(),
            StatusCode::ACCEPTED,
            "submit should be accepted"
        );
        serde_json::from_slice(resp.body()).unwrap()
    }

    #[tokio::test]
    async fn r1_omitted_restore_mode_generates_mapping() {
        let (_temp, runtime) = test_runtime().await;
        let created = submit_with_mode(&runtime, &[("r1.txt", b"13900000000")], None).await;
        let detail = wait_terminal(&runtime, &created.batch_id).await;
        assert_eq!(detail.batch.status, BatchStatus::Completed);
        assert!(
            detail.files[0].restore_available,
            "mapping must be generated when restore_mode is omitted"
        );
        let aid = detail.files[0].artifact_id.clone().unwrap();
        let restore = request()
            .method("POST")
            .path(&format!("/api/v1/artifacts/{aid}/restore"))
            .reply(&routes(runtime))
            .await;
        assert_eq!(
            restore.status(),
            StatusCode::OK,
            "restore must succeed with mapping"
        );
    }

    #[tokio::test]
    async fn r1_disabled_value_is_normalized_to_server_cmap() {
        let (_temp, runtime) = test_runtime().await;
        let created =
            submit_with_mode(&runtime, &[("r1d.txt", b"13900000000")], Some("disabled")).await;
        let detail = wait_terminal(&runtime, &created.batch_id).await;
        assert_eq!(detail.batch.status, BatchStatus::Completed);
        assert!(
            detail.files[0].restore_available,
            "mapping must be generated even when caller sends disabled"
        );
        let aid = detail.files[0].artifact_id.clone().unwrap();
        let restore = request()
            .method("POST")
            .path(&format!("/api/v1/artifacts/{aid}/restore"))
            .reply(&routes(runtime))
            .await;
        assert_eq!(
            restore.status(),
            StatusCode::OK,
            "restore must succeed after disabled normalization"
        );
    }

    #[tokio::test]
    async fn r1_server_cmap_explicit_value_generates_mapping() {
        let (_temp, runtime) = test_runtime().await;
        let created = submit_with_mode(
            &runtime,
            &[("r1s.txt", b"13900000000")],
            Some("server_cmap"),
        )
        .await;
        let detail = wait_terminal(&runtime, &created.batch_id).await;
        assert_eq!(detail.batch.status, BatchStatus::Completed);
        assert!(
            detail.files[0].restore_available,
            "server_cmap mode must generate mapping"
        );
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
        let (_temp, runtime) = test_runtime_without_ocr().await;
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
        let required_texts: std::collections::HashSet<&str> = required
            .iter()
            .map(|f| f["text"].as_str().unwrap())
            .collect();
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

        assert!(
            json.get("note").is_none(),
            "note must not reappear in the JSON (G4)"
        );
        assert!(
            json.get("notes").is_none(),
            "notes must not reappear in the JSON (G4)"
        );
    }

    /// 本机 OCR 集成冒烟测试（TASK-OCR-READY-GOLD-TIMEOUT-RELIABILITY-001）。
    ///
    /// 用真实无文本层金标 PDF 走共享 resolver → `component_runtime::run_ocr`
    /// → preview worker，断言在 Runtime 的 300s 预算内到达 `Ready` 且
    /// `masked_entity_count == 5`。该测试验证"当前这台 macOS 机器上，
    /// 常规/空闲负载下金标端到端可完成"，不是对"任意外部负载下必然 <300s"
    /// 或高负载可靠性修复的机制回归，也不宣称诊断已消除超时。
    ///
    /// 依赖本机共享 OCR 安装与真实金标，因此标记为 `#[ignore]`，需要显式
    /// 运行：`cargo test --lib -- --ignored ocr_gold_standard`。未安装 OCR 时
    /// 必须失败（不能静默记为通过），由管理员安装后再显式运行。
    #[tokio::test]
    #[ignore = "本机 OCR 集成冒烟测试：需本机共享 OCR 安装与真实金标，显式运行"]
    async fn ocr_gold_standard_preview_completes_ready_within_timeout_budget() {
        component_runtime::resolve_ocr_config().expect(
            "本机 OCR 集成冒烟测试需要本机共享 OCR 安装，但当前未找到；\
             请由管理员安装 OCR 后显式运行本测试",
        );
        let (_temp, runtime) = test_runtime().await;
        let created = submit_preview_with_rules(
            &runtime,
            &[("pdf_ocr_gold_standard.pdf", &pdf_ocr_gold_standard_pdf())],
            "[\"phone\",\"email\",\"id_card\"]",
        )
        .await;
        assert_eq!(created.files.len(), 1);

        let start = std::time::Instant::now();
        let mut detail = runtime
            .store
            .preview_detail(&created.preview_id)
            .await
            .unwrap();
        while detail.status == PreviewSessionStatus::Processing
            && start.elapsed() < Duration::from_secs(300)
        {
            tokio::time::sleep(Duration::from_millis(1000)).await;
            detail = runtime
                .store
                .preview_detail(&created.preview_id)
                .await
                .unwrap();
        }
        let elapsed = start.elapsed();

        assert_eq!(
            detail.status,
            PreviewSessionStatus::Ready,
            "gold standard preview must reach Ready, got {:?} (error {:?})",
            detail.status,
            detail.files.first().map(|f| f.error_code.clone())
        );
        assert_eq!(detail.masked_entity_count, 5);
        assert!(
            elapsed < Duration::from_secs(300),
            "gold standard OCR exceeded the 300s Runtime budget: {elapsed:?}"
        );
    }

    // ------------------------------------------------------------------
    // Preview sessions (two-phase browser preview/confirm, PC task)
    // ------------------------------------------------------------------

    use service_contracts::{ConfirmPreviewResponse, CreatePreviewResponse, PreviewSessionStatus};

    async fn submit_preview_with_rules(
        runtime: &Runtime,
        files: &[(&str, &[u8])],
        rules: &str,
    ) -> CreatePreviewResponse {
        let (content_type, body) = multipart(files, rules);
        let response = request()
            .method("POST")
            .path("/api/v1/previews")
            .header("content-type", content_type)
            .body(body)
            .reply(&routes(runtime.clone()))
            .await;
        assert_eq!(response.status(), StatusCode::ACCEPTED);
        serde_json::from_slice(response.body()).unwrap()
    }

    async fn submit_preview(runtime: &Runtime, files: &[(&str, &[u8])]) -> CreatePreviewResponse {
        submit_preview_with_rules(runtime, files, "[\"phone\",\"email\"]").await
    }

    async fn wait_preview_terminal(
        runtime: &Runtime,
        preview_id: &str,
    ) -> service_contracts::PreviewDetail {
        for _ in 0..200 {
            let detail = runtime.store.preview_detail(preview_id).await.unwrap();
            if detail.status != PreviewSessionStatus::Processing {
                return detail;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        panic!("preview did not reach a terminal state");
    }

    #[tokio::test]
    async fn preview_creation_reaches_ready_with_masked_content_and_no_original_text() {
        let (_temp, runtime) = test_runtime().await;
        let created = submit_preview(&runtime, &[("note.txt", b"Call 13900000000")]).await;
        assert_eq!(created.files.len(), 1);
        let file_id = created.files[0].file_id.clone();

        let detail = wait_preview_terminal(&runtime, &created.preview_id).await;
        assert_eq!(detail.status, PreviewSessionStatus::Ready);
        assert_eq!(detail.ready_count, 1);
        assert_eq!(detail.failed_count, 0);
        assert_eq!(detail.masked_entity_count, 1);

        let response = request()
            .method("GET")
            .path(&format!(
                "/api/v1/previews/{}/files/{}/content",
                created.preview_id, file_id
            ))
            .reply(&routes(runtime.clone()))
            .await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get("cache-control")
                .map(|v| v.to_str().unwrap()),
            Some("no-store")
        );
        let body = std::str::from_utf8(response.body()).unwrap();
        assert!(body.contains("***PHONE***1"));
        assert!(!body.contains("13900000000"));
    }

    #[tokio::test]
    async fn preview_before_confirm_creates_no_real_batch_records() {
        let (_temp, runtime) = test_runtime().await;
        let before_counts = runtime.store.record_counts().await.unwrap();
        let created = submit_preview(
            &runtime,
            &[
                ("a.txt", b"Call 13900000000"),
                ("b.txt", b"Call 13900000001"),
            ],
        )
        .await;
        let _detail = wait_preview_terminal(&runtime, &created.preview_id).await;

        assert_eq!(runtime.store.record_counts().await.unwrap(), before_counts);
        assert_eq!(batch_ids(&runtime).await, Vec::<String>::new());
    }

    #[tokio::test]
    async fn preview_detail_response_never_contains_original_text_or_internal_fields() {
        let (_temp, runtime) = test_runtime().await;
        let created = submit_preview(&runtime, &[("secret.txt", b"Call 13900000000")]).await;
        let _ = wait_preview_terminal(&runtime, &created.preview_id).await;

        let response = request()
            .method("GET")
            .path(&format!("/api/v1/previews/{}", created.preview_id))
            .reply(&routes(runtime.clone()))
            .await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = std::str::from_utf8(response.body()).unwrap();
        assert!(!body.contains("13900000000"));
        for forbidden in [
            "original",
            "mapping",
            "object_key",
            "input_object_key",
            "markdown_object_key",
            "mapping_object_key",
            "\"path\"",
        ] {
            assert!(
                !body.contains(forbidden),
                "detail body must not contain `{forbidden}`: {body}"
            );
        }
    }

    #[tokio::test]
    async fn preview_content_rejects_unknown_cross_session_and_malformed_ids() {
        let (_temp, runtime) = test_runtime().await;
        let created_a = submit_preview(&runtime, &[("a.txt", b"Call 13900000000")]).await;
        let created_b = submit_preview(&runtime, &[("b.txt", b"Call 13900000001")]).await;
        let _ = wait_preview_terminal(&runtime, &created_a.preview_id).await;
        let _ = wait_preview_terminal(&runtime, &created_b.preview_id).await;

        // Unknown file id within a real preview.
        let response = request()
            .method("GET")
            .path(&format!(
                "/api/v1/previews/{}/files/{}/content",
                created_a.preview_id,
                uuid::Uuid::new_v4()
            ))
            .reply(&routes(runtime.clone()))
            .await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);

        // Cross-session file id: file from preview B fetched via preview A's id.
        let response = request()
            .method("GET")
            .path(&format!(
                "/api/v1/previews/{}/files/{}/content",
                created_a.preview_id, created_b.files[0].file_id
            ))
            .reply(&routes(runtime.clone()))
            .await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);

        // Unknown / traversal-shaped / overlong preview ids never reach the
        // filesystem: `preview_id` is only ever joined into a path after a
        // successful `SELECT ... WHERE id = ?` lookup (see `store.rs`
        // `delete_preview`/`sweep_expired_previews`), and a client-supplied
        // id can never match a server-generated UUID row.
        for bad_id in ["not-a-real-id", "..", &"x".repeat(5000)] {
            let response = request()
                .method("GET")
                .path(&format!("/api/v1/previews/{bad_id}"))
                .reply(&routes(runtime.clone()))
                .await;
            assert_eq!(response.status(), StatusCode::NOT_FOUND, "bad_id={bad_id}");
        }
    }

    #[tokio::test]
    async fn direct_batch_and_preview_produce_byte_identical_masked_output() {
        let (_temp, runtime) = test_runtime().await;
        let content: &[u8] = b"Call 13900000000 and unit.test@example.invalid";

        let batch_created = submit(&runtime, &[("input.txt", content)]).await;
        let batch_detail = wait_terminal(&runtime, &batch_created.batch_id).await;
        assert_eq!(batch_detail.batch.status, BatchStatus::Completed);
        let artifact_id = batch_detail.files[0].artifact_id.clone().unwrap();
        let artifact_response = request()
            .method("GET")
            .path(&format!("/api/v1/artifacts/{artifact_id}"))
            .reply(&routes(runtime.clone()))
            .await;
        assert_eq!(artifact_response.status(), StatusCode::OK);
        let batch_markdown = artifact_response.body().to_vec();
        let batch_count = batch_detail.files[0].masked_entity_count;

        let preview_created = submit_preview(&runtime, &[("input.txt", content)]).await;
        let preview_detail = wait_preview_terminal(&runtime, &preview_created.preview_id).await;
        assert_eq!(preview_detail.status, PreviewSessionStatus::Ready);
        let preview_content_response = request()
            .method("GET")
            .path(&format!(
                "/api/v1/previews/{}/files/{}/content",
                preview_created.preview_id, preview_created.files[0].file_id
            ))
            .reply(&routes(runtime.clone()))
            .await;
        assert_eq!(preview_content_response.status(), StatusCode::OK);
        let preview_markdown = preview_content_response.body().to_vec();

        assert_eq!(batch_markdown, preview_markdown);
        assert_eq!(
            Sha256::digest(&batch_markdown),
            Sha256::digest(&preview_markdown)
        );
        assert_eq!(batch_count, Some(preview_detail.masked_entity_count));
    }

    #[tokio::test]
    async fn confirm_never_reprocesses_a_ready_preview() {
        let (_temp, runtime) = test_runtime().await;
        let created = submit_preview(&runtime, &[("a.txt", b"Call 13900000000")]).await;
        let detail = wait_preview_terminal(&runtime, &created.preview_id).await;
        assert_eq!(detail.status, PreviewSessionStatus::Ready);

        let count_before_confirm = runtime.processing_call_count();
        assert!(count_before_confirm >= 1);

        let response = request()
            .method("POST")
            .path(&format!("/api/v1/previews/{}/confirm", created.preview_id))
            .reply(&routes(runtime.clone()))
            .await;
        assert_eq!(response.status(), StatusCode::OK);

        assert_eq!(runtime.processing_call_count(), count_before_confirm);
    }

    #[tokio::test]
    async fn partial_failure_preview_confirm_becomes_completed_with_errors() {
        let (_temp, runtime) = test_runtime().await;
        let created = submit_preview(
            &runtime,
            &[("good.txt", b"Call 13900000000"), ("bad.md", b"%PDF-1.7\n")],
        )
        .await;
        let detail = wait_preview_terminal(&runtime, &created.preview_id).await;
        assert_eq!(detail.status, PreviewSessionStatus::ReadyWithErrors);
        assert_eq!(detail.ready_count, 1);
        assert_eq!(detail.failed_count, 1);

        let response = request()
            .method("POST")
            .path(&format!("/api/v1/previews/{}/confirm", created.preview_id))
            .reply(&routes(runtime.clone()))
            .await;
        assert_eq!(response.status(), StatusCode::OK);
        let confirm: ConfirmPreviewResponse = serde_json::from_slice(response.body()).unwrap();

        let batch_detail = runtime.store.batch_detail(&confirm.batch_id).await.unwrap();
        assert_eq!(batch_detail.batch.status, BatchStatus::CompletedWithErrors);
        assert_eq!(batch_detail.batch.completed_count, 1);
        assert_eq!(batch_detail.batch.failed_count, 1);

        // The failed file keeps working with the existing retry endpoint.
        let failed_file = batch_detail
            .files
            .iter()
            .find(|f| f.status == FileStatus::Failed)
            .unwrap();
        let retry_response = request()
            .method("POST")
            .path(&format!("/api/v1/files/{}/retry", failed_file.file_id))
            .reply(&routes(runtime.clone()))
            .await;
        assert_eq!(retry_response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn all_failed_preview_cannot_be_confirmed() {
        let (_temp, runtime) = test_runtime().await;
        let created = submit_preview(&runtime, &[("bad.md", b"%PDF-1.7\n")]).await;
        let detail = wait_preview_terminal(&runtime, &created.preview_id).await;
        assert_eq!(detail.status, PreviewSessionStatus::Failed);

        let response = request()
            .method("POST")
            .path(&format!("/api/v1/previews/{}/confirm", created.preview_id))
            .reply(&routes(runtime.clone()))
            .await;
        assert_eq!(response.status(), StatusCode::CONFLICT);
        let error: ErrorResponse = serde_json::from_slice(response.body()).unwrap();
        assert_eq!(error.code, "PREVIEW_NOT_READY");
        assert_eq!(runtime.store.record_counts().await.unwrap(), (0, 0));
    }

    #[tokio::test]
    async fn concurrent_confirm_produces_exactly_one_batch() {
        let (_temp, runtime) = test_runtime().await;
        let created = submit_preview(&runtime, &[("a.txt", b"Call 13900000000")]).await;
        let _ = wait_preview_terminal(&runtime, &created.preview_id).await;

        let (first, second) = tokio::join!(
            runtime.store.confirm_preview(&created.preview_id),
            runtime.store.confirm_preview(&created.preview_id)
        );
        let first = first.unwrap();
        let second = second.unwrap();
        let batch_id_of = |outcome: &crate::store::ConfirmOutcome| match outcome {
            crate::store::ConfirmOutcome::Confirmed { batch_id }
            | crate::store::ConfirmOutcome::AlreadyConfirmed { batch_id } => Some(batch_id.clone()),
            _ => None,
        };
        let ids: Vec<String> = [&first, &second]
            .iter()
            .filter_map(|o| batch_id_of(o))
            .collect();
        assert!(
            !ids.is_empty(),
            "at least one concurrent confirm must succeed: {:?} {:?}",
            first,
            second
        );
        assert!(
            ids.iter().all(|id| id == &ids[0]),
            "both outcomes must reference the same batch: {:?} {:?}",
            first,
            second
        );

        let (batches, _) = runtime.store.record_counts().await.unwrap();
        assert_eq!(batches, 1);
    }

    #[tokio::test]
    async fn repeated_confirm_after_success_returns_the_same_batch_id_and_no_duplicate_batch() {
        let (_temp, runtime) = test_runtime().await;
        let created = submit_preview(&runtime, &[("a.txt", b"Call 13900000000")]).await;
        let _ = wait_preview_terminal(&runtime, &created.preview_id).await;

        let first = request()
            .method("POST")
            .path(&format!("/api/v1/previews/{}/confirm", created.preview_id))
            .reply(&routes(runtime.clone()))
            .await;
        assert_eq!(first.status(), StatusCode::OK);
        let first: ConfirmPreviewResponse = serde_json::from_slice(first.body()).unwrap();

        let second = request()
            .method("POST")
            .path(&format!("/api/v1/previews/{}/confirm", created.preview_id))
            .reply(&routes(runtime.clone()))
            .await;
        assert_eq!(second.status(), StatusCode::OK);
        let second: ConfirmPreviewResponse = serde_json::from_slice(second.body()).unwrap();

        assert_eq!(first.batch_id, second.batch_id);
        let (batches, _) = runtime.store.record_counts().await.unwrap();
        assert_eq!(batches, 1);
    }

    #[tokio::test]
    async fn cancel_deletes_temp_preview_data_and_creates_no_batch() {
        let (_temp, runtime) = test_runtime().await;
        let created = submit_preview(&runtime, &[("a.txt", b"Call 13900000000")]).await;
        let _ = wait_preview_terminal(&runtime, &created.preview_id).await;

        let root = runtime
            .store
            .database_path()
            .parent()
            .unwrap()
            .to_path_buf();
        let preview_dir = root.join("preview").join(&created.preview_id);
        assert!(
            tokio::fs::metadata(&preview_dir).await.is_ok(),
            "preview temp dir must exist before cancel"
        );

        let response = request()
            .method("DELETE")
            .path(&format!("/api/v1/previews/{}", created.preview_id))
            .reply(&routes(runtime.clone()))
            .await;
        assert_eq!(response.status(), StatusCode::NO_CONTENT);

        assert!(
            tokio::fs::metadata(&preview_dir).await.is_err(),
            "preview temp dir must be removed after cancel"
        );
        assert_eq!(runtime.store.record_counts().await.unwrap(), (0, 0));
        assert!(matches!(
            runtime.store.preview_detail(&created.preview_id).await,
            Err(StoreError::NotFound)
        ));
    }

    #[tokio::test]
    async fn mid_flight_cancel_never_produces_a_real_batch() {
        let (_temp, runtime) = test_runtime().await;
        let created = submit_preview(
            &runtime,
            &[
                ("a.txt", b"Call 13900000000"),
                ("b.txt", b"Call 13900000001"),
                ("c.txt", b"Call 13900000002"),
            ],
        )
        .await;

        // Cancel immediately, racing the worker that may already be mid-flight
        // on one of the files (D4).
        let response = request()
            .method("DELETE")
            .path(&format!("/api/v1/previews/{}", created.preview_id))
            .reply(&routes(runtime.clone()))
            .await;
        assert_eq!(response.status(), StatusCode::NO_CONTENT);

        // Give any in-flight worker task time to finish and attempt to persist
        // its (now-discardable) result.
        tokio::time::sleep(Duration::from_millis(300)).await;

        assert_eq!(runtime.store.record_counts().await.unwrap(), (0, 0));
        assert!(matches!(
            runtime.store.preview_detail(&created.preview_id).await,
            Err(StoreError::NotFound)
        ));
    }

    #[tokio::test]
    async fn ttl_expiry_returns_safe_not_found_and_cleans_up_temp_data() {
        let temp = tempfile::tempdir().unwrap();
        let runtime = Runtime::initialize_with_preview_ttl(
            temp.path().join("enterprise-data"),
            Limits::default(),
            Duration::from_millis(500),
        )
        .await
        .unwrap();

        let created = submit_preview(&runtime, &[("a.txt", b"Call 13900000000")]).await;
        let _ = wait_preview_terminal(&runtime, &created.preview_id).await;
        let root = runtime
            .store
            .database_path()
            .parent()
            .unwrap()
            .to_path_buf();
        let preview_dir = root.join("preview").join(&created.preview_id);

        tokio::time::sleep(Duration::from_millis(700)).await;

        let response = request()
            .method("GET")
            .path(&format!("/api/v1/previews/{}", created.preview_id))
            .reply(&routes(runtime.clone()))
            .await;
        assert_eq!(
            response.status(),
            StatusCode::NOT_FOUND,
            "expired preview must return a safe not-found status"
        );
        let body = std::str::from_utf8(response.body()).unwrap();
        assert!(!body.to_lowercase().contains("panic"));

        assert!(
            tokio::fs::metadata(&preview_dir).await.is_err(),
            "expired preview temp dir must be cleaned up"
        );
    }

    #[tokio::test]
    async fn runtime_restart_wipes_leftover_preview_sessions() {
        let temp = tempfile::tempdir().unwrap();
        let data_root = temp.path().join("enterprise-data");
        let runtime = Runtime::initialize(data_root.clone(), Limits::default())
            .await
            .unwrap();
        let created = submit_preview(&runtime, &[("a.txt", b"Call 13900000000")]).await;
        let _ = wait_preview_terminal(&runtime, &created.preview_id).await;

        let root = runtime
            .store
            .database_path()
            .parent()
            .unwrap()
            .to_path_buf();
        let preview_dir = root.join("preview").join(&created.preview_id);
        assert!(tokio::fs::metadata(&preview_dir).await.is_ok());

        // Simulate an abnormal-exit restart against the same data root (D3).
        let restarted = Runtime::initialize(data_root, Limits::default())
            .await
            .unwrap();

        assert!(
            tokio::fs::metadata(&preview_dir).await.is_err(),
            "startup must wipe leftover preview/ temp data"
        );
        assert!(matches!(
            restarted.store.preview_detail(&created.preview_id).await,
            Err(StoreError::NotFound)
        ));
    }

    #[tokio::test]
    async fn confirm_failure_rolls_back_and_reverts_preview_to_ready() {
        let (_temp, runtime) = test_runtime().await;
        let created = submit_preview(&runtime, &[("a.txt", b"Call 13900000000")]).await;
        let detail = wait_preview_terminal(&runtime, &created.preview_id).await;
        assert_eq!(detail.status, PreviewSessionStatus::Ready);

        // Force the confirm's file-copy step to fail by deleting the
        // preview's already-written mapping file out from under it (E5).
        let root = runtime
            .store
            .database_path()
            .parent()
            .unwrap()
            .to_path_buf();
        let mapping_dir = root
            .join("preview")
            .join(&created.preview_id)
            .join("mapping");
        let mut entries = tokio::fs::read_dir(&mapping_dir).await.unwrap();
        let mapping_file = entries.next_entry().await.unwrap().unwrap().path();
        tokio::fs::remove_file(&mapping_file).await.unwrap();

        let before_counts = runtime.store.record_counts().await.unwrap();
        let outcome = runtime.store.confirm_preview(&created.preview_id).await;
        assert!(
            outcome.is_err(),
            "confirm must fail when a preview file is missing: {:?}",
            outcome
        );

        assert_eq!(
            runtime.store.record_counts().await.unwrap(),
            before_counts,
            "a failed confirm must leave no half-finished batch/artifact"
        );

        let after = runtime
            .store
            .preview_detail(&created.preview_id)
            .await
            .unwrap();
        assert_eq!(
            after.status,
            PreviewSessionStatus::Ready,
            "preview must revert to Ready, not be stuck Confirming"
        );
    }

    // ------------------------------------------------------------------
    // Sensitive-term library (ST task)
    // ------------------------------------------------------------------

    use service_contracts::{
        SensitiveTerm, SensitiveTermCategoriesResponse, SensitiveTermsImportResponse,
        SensitiveTermsResponse, SensitiveTermsStats,
    };

    async fn create_term_response(
        runtime: &Runtime,
        term: &str,
        category: &str,
        description: Option<&str>,
    ) -> warp::http::Response<bytes::Bytes> {
        let body =
            serde_json::json!({ "term": term, "category": category, "description": description });
        request()
            .method("POST")
            .path("/api/v1/sensitive-terms")
            .header("content-type", "application/json")
            .body(serde_json::to_vec(&body).unwrap())
            .reply(&routes(runtime.clone()))
            .await
    }

    async fn create_term(
        runtime: &Runtime,
        term: &str,
        category: &str,
        description: Option<&str>,
    ) -> SensitiveTerm {
        let response = create_term_response(runtime, term, category, description).await;
        assert_eq!(
            response.status(),
            StatusCode::CREATED,
            "{:?}",
            std::str::from_utf8(response.body())
        );
        serde_json::from_slice(response.body()).unwrap()
    }

    async fn update_term_response(
        runtime: &Runtime,
        id: &str,
        body: serde_json::Value,
    ) -> warp::http::Response<bytes::Bytes> {
        request()
            .method("PUT")
            .path(&format!("/api/v1/sensitive-terms/{id}"))
            .header("content-type", "application/json")
            .body(serde_json::to_vec(&body).unwrap())
            .reply(&routes(runtime.clone()))
            .await
    }

    async fn list_terms(runtime: &Runtime, query: &str) -> SensitiveTermsResponse {
        let path = if query.is_empty() {
            "/api/v1/sensitive-terms".to_string()
        } else {
            format!("/api/v1/sensitive-terms?{query}")
        };
        let response = request()
            .method("GET")
            .path(&path)
            .reply(&routes(runtime.clone()))
            .await;
        assert_eq!(response.status(), StatusCode::OK);
        serde_json::from_slice(response.body()).unwrap()
    }

    fn csv_multipart(csv_bytes: &[u8]) -> (String, Vec<u8>) {
        let boundary = "sensitive-terms-csv-boundary";
        let mut body = Vec::new();
        body.extend_from_slice(
            format!(
                "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"terms.csv\"\r\nContent-Type: text/csv\r\n\r\n"
            )
            .as_bytes(),
        );
        body.extend_from_slice(csv_bytes);
        body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
        (format!("multipart/form-data; boundary={boundary}"), body)
    }

    async fn import_csv(runtime: &Runtime, csv_bytes: &[u8]) -> warp::http::Response<bytes::Bytes> {
        let (content_type, body) = csv_multipart(csv_bytes);
        request()
            .method("POST")
            .path("/api/v1/sensitive-terms/import")
            .header("content-type", content_type)
            .body(body)
            .reply(&routes(runtime.clone()))
            .await
    }

    #[tokio::test]
    async fn sensitive_term_crud_search_category_and_stats_work() {
        let (_temp, runtime) = test_runtime().await;

        // D2/4.1: trimming on create.
        let created = create_term(
            &runtime,
            "  alpha-term  ",
            "  category-a  ",
            Some("  a note  "),
        )
        .await;
        assert_eq!(created.term, "alpha-term");
        assert_eq!(created.category, "category-a");
        assert_eq!(created.description.as_deref(), Some("a note"));
        assert!(created.enabled);

        let second = create_term(&runtime, "beta-term", "category-b", None).await;
        assert!(second.description.is_none());

        let all = list_terms(&runtime, "").await;
        assert_eq!(all.terms.len(), 2);

        // category filter
        let by_category = list_terms(&runtime, "category=category-a").await;
        assert_eq!(by_category.terms.len(), 1);
        assert_eq!(by_category.terms[0].id, created.id);

        // search
        let by_query = list_terms(&runtime, "query=beta").await;
        assert_eq!(by_query.terms.len(), 1);
        assert_eq!(by_query.terms[0].id, second.id);

        // update: partial fields only, enabled toggled off
        let update_response = update_term_response(
            &runtime,
            &created.id,
            serde_json::json!({ "enabled": false }),
        )
        .await;
        assert_eq!(update_response.status(), StatusCode::OK);
        let updated: SensitiveTerm = serde_json::from_slice(update_response.body()).unwrap();
        assert_eq!(
            updated.term, "alpha-term",
            "unspecified fields must be left unchanged"
        );
        assert!(!updated.enabled);

        // enabled_only filter now excludes it
        let enabled_only = list_terms(&runtime, "enabled_only=true").await;
        assert_eq!(enabled_only.terms.len(), 1);
        assert_eq!(enabled_only.terms[0].id, second.id);

        // categories
        let response = request()
            .method("GET")
            .path("/api/v1/sensitive-terms/categories")
            .reply(&routes(runtime.clone()))
            .await;
        assert_eq!(response.status(), StatusCode::OK);
        let categories: SensitiveTermCategoriesResponse =
            serde_json::from_slice(response.body()).unwrap();
        assert_eq!(categories.categories, vec!["category-a", "category-b"]);

        // stats
        let response = request()
            .method("GET")
            .path("/api/v1/sensitive-terms/stats")
            .reply(&routes(runtime.clone()))
            .await;
        assert_eq!(response.status(), StatusCode::OK);
        let stats: SensitiveTermsStats = serde_json::from_slice(response.body()).unwrap();
        assert_eq!(
            stats,
            SensitiveTermsStats {
                total: 2,
                enabled: 1,
                disabled: 1,
                categories: 2
            }
        );

        // delete
        let response = request()
            .method("DELETE")
            .path(&format!("/api/v1/sensitive-terms/{}", second.id))
            .reply(&routes(runtime.clone()))
            .await;
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        assert_eq!(list_terms(&runtime, "").await.terms.len(), 1);
    }

    /// A5: duplicate/empty/oversized-input/not-found all return stable,
    /// safe errors and never persist a bad row.
    #[tokio::test]
    async fn sensitive_term_validation_rejects_duplicate_empty_oversized_and_unknown_id() {
        let (_temp, runtime) = test_runtime().await;
        let first = create_term(&runtime, "duplicate-me", "cat", None).await;

        let dup = create_term_response(&runtime, "duplicate-me", "other-cat", None).await;
        assert_eq!(dup.status(), StatusCode::CONFLICT);
        let error: ErrorResponse = serde_json::from_slice(dup.body()).unwrap();
        assert_eq!(error.code, "SENSITIVE_TERM_DUPLICATE");

        let empty_term = create_term_response(&runtime, "   ", "cat", None).await;
        assert_eq!(empty_term.status(), StatusCode::BAD_REQUEST);
        let error: ErrorResponse = serde_json::from_slice(empty_term.body()).unwrap();
        assert_eq!(error.code, "SENSITIVE_TERM_INVALID");

        let empty_category = create_term_response(&runtime, "some-term", "  ", None).await;
        assert_eq!(empty_category.status(), StatusCode::BAD_REQUEST);

        let too_long_term = "x".repeat(257);
        let oversized = create_term_response(&runtime, &too_long_term, "cat", None).await;
        assert_eq!(oversized.status(), StatusCode::BAD_REQUEST);
        let error: ErrorResponse = serde_json::from_slice(oversized.body()).unwrap();
        assert_eq!(error.code, "SENSITIVE_TERM_INVALID");

        // update to a duplicate term is also rejected
        let second = create_term(&runtime, "unique-term", "cat", None).await;
        let dup_update = update_term_response(
            &runtime,
            &second.id,
            serde_json::json!({ "term": "duplicate-me" }),
        )
        .await;
        assert_eq!(dup_update.status(), StatusCode::CONFLICT);

        // unknown id
        let missing_update = update_term_response(
            &runtime,
            "does-not-exist",
            serde_json::json!({ "enabled": false }),
        )
        .await;
        assert_eq!(missing_update.status(), StatusCode::NOT_FOUND);
        let error: ErrorResponse = serde_json::from_slice(missing_update.body()).unwrap();
        assert_eq!(error.code, "SENSITIVE_TERM_NOT_FOUND");

        let missing_delete = request()
            .method("DELETE")
            .path("/api/v1/sensitive-terms/does-not-exist")
            .reply(&routes(runtime.clone()))
            .await;
        assert_eq!(missing_delete.status(), StatusCode::NOT_FOUND);

        // only the two genuinely valid terms persisted
        assert_eq!(list_terms(&runtime, "").await.terms.len(), 2);
        let _ = first;
    }

    /// D4: CSV export → clear → import round-trips fields exactly, including
    /// a description containing a comma and an embedded double quote (RFC
    /// 4180 quoting), and accepts a UTF-8 BOM prefix.
    #[tokio::test]
    async fn sensitive_terms_csv_export_then_import_round_trips_quoted_fields_and_bom() {
        let (_temp, runtime) = test_runtime().await;
        create_term(
            &runtime,
            "csv-term-one",
            "cat-a",
            Some("has, a comma and \"quotes\""),
        )
        .await;
        let disabled = create_term(&runtime, "csv-term-two", "cat-b", None).await;
        update_term_response(
            &runtime,
            &disabled.id,
            serde_json::json!({ "enabled": false }),
        )
        .await;

        let export = request()
            .method("GET")
            .path("/api/v1/sensitive-terms/export")
            .reply(&routes(runtime.clone()))
            .await;
        assert_eq!(export.status(), StatusCode::OK);
        assert_eq!(
            export.headers().get("content-type").unwrap(),
            "text/csv; charset=utf-8"
        );
        let csv_body = export.body().to_vec();
        let exported_text = std::str::from_utf8(&csv_body).unwrap();
        assert!(exported_text.contains("csv-term-one"));
        assert!(exported_text.contains("csv-term-two"));
        assert!(exported_text.contains("启用"));
        assert!(exported_text.contains("禁用"));
        // no server path or internal id leaks into the CSV body
        assert!(!exported_text.contains("/enterprise-data"));

        // Clear the library, then re-import the exported CSV with a UTF-8
        // BOM prefix prepended (must be accepted, not treated as invalid
        // UTF-8 or a broken header).
        let all = list_terms(&runtime, "").await;
        for term in &all.terms {
            let response = request()
                .method("DELETE")
                .path(&format!("/api/v1/sensitive-terms/{}", term.id))
                .reply(&routes(runtime.clone()))
                .await;
            assert_eq!(response.status(), StatusCode::NO_CONTENT);
        }
        assert_eq!(list_terms(&runtime, "").await.terms.len(), 0);

        let mut with_bom = vec![0xEF, 0xBB, 0xBF];
        with_bom.extend_from_slice(&csv_body);
        let import_response = import_csv(&runtime, &with_bom).await;
        assert_eq!(
            import_response.status(),
            StatusCode::OK,
            "{:?}",
            std::str::from_utf8(import_response.body())
        );
        let imported: SensitiveTermsImportResponse =
            serde_json::from_slice(import_response.body()).unwrap();
        assert_eq!(imported.imported_count, 2);

        let reimported = list_terms(&runtime, "").await;
        assert_eq!(reimported.terms.len(), 2);
        let one = reimported
            .terms
            .iter()
            .find(|t| t.term == "csv-term-one")
            .unwrap();
        assert_eq!(
            one.description.as_deref(),
            Some("has, a comma and \"quotes\"")
        );
        assert_eq!(one.category, "cat-a");
        assert!(one.enabled);
        let two = reimported
            .terms
            .iter()
            .find(|t| t.term == "csv-term-two")
            .unwrap();
        assert!(
            !two.enabled,
            "禁用 status column must round-trip to enabled=false"
        );
    }

    /// D4/8.5: an incorrect header, an illegal status value, non-UTF-8
    /// bytes, a wrong column count, and a within-file duplicate term all
    /// fail the *entire* import — never a partial success.
    #[tokio::test]
    async fn sensitive_terms_csv_import_rejects_malformed_input_wholesale() {
        let (_temp, runtime) = test_runtime().await;

        let bad_header = "wrong,header,here,x\ncat,term,,启用\n";
        let response = import_csv(&runtime, bad_header.as_bytes()).await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let error: ErrorResponse = serde_json::from_slice(response.body()).unwrap();
        assert_eq!(error.code, "SENSITIVE_TERMS_IMPORT_INVALID");

        let bad_status = "分类,敏感词,描述,状态\ncat,term-x,,maybe\n";
        let response = import_csv(&runtime, bad_status.as_bytes()).await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let error: ErrorResponse = serde_json::from_slice(response.body()).unwrap();
        assert_eq!(error.code, "SENSITIVE_TERMS_IMPORT_INVALID");
        assert_eq!(
            list_terms(&runtime, "").await.terms.len(),
            0,
            "no partial import"
        );

        let bad_utf8: &[u8] = &[0x88, 0x99, 0xAA, 0xBB];
        let response = import_csv(&runtime, bad_utf8).await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let wrong_columns = "分类,敏感词,描述,状态\ncat,term-y,启用\n";
        let response = import_csv(&runtime, wrong_columns.as_bytes()).await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let internal_duplicate = "分类,敏感词,描述,状态\ncat,dup-term,,启用\ncat,dup-term,,禁用\n";
        let response = import_csv(&runtime, internal_duplicate.as_bytes()).await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let error: ErrorResponse = serde_json::from_slice(response.body()).unwrap();
        assert_eq!(error.code, "SENSITIVE_TERMS_IMPORT_INVALID");

        assert_eq!(
            list_terms(&runtime, "").await.terms.len(),
            0,
            "every rejected import must leave the library untouched"
        );

        // A genuinely valid CSV still imports fine afterwards.
        let valid = "分类,敏感词,描述,状态\ncat,term-z,,启用\n";
        let response = import_csv(&runtime, valid.as_bytes()).await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(list_terms(&runtime, "").await.terms.len(), 1);

        // importing a term that already exists in the library also fails
        // wholesale, not just for that one row.
        let against_existing = "分类,敏感词,描述,状态\ncat,term-z,,启用\ncat,term-new,,启用\n";
        let response = import_csv(&runtime, against_existing.as_bytes()).await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            list_terms(&runtime, "").await.terms.len(),
            1,
            "no partial success against existing rows"
        );
    }

    /// B4: `chinese_name` remains rejected from the enterprise `/rules`
    /// contract even when combined with `use_sensitive_terms`.
    #[tokio::test]
    async fn chinese_name_stays_rejected_alongside_use_sensitive_terms() {
        let (_temp, runtime) = test_runtime().await;
        let (content_type, body) = multipart(
            &[("safe.txt", b"content")],
            r#"["chinese_name","use_sensitive_terms"]"#,
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
        assert_eq!(runtime.store.record_counts().await.unwrap(), (0, 0));
    }

    /// A3/A4: the sensitive-term table survives a fresh `Store::open` against
    /// the same data directory (the schema-init statements are idempotent
    /// `CREATE TABLE IF NOT EXISTS`/`ALTER TABLE` — a restart, not a reset).
    #[tokio::test]
    async fn sensitive_terms_and_existing_batches_persist_across_a_runtime_restart() {
        let temp = tempfile::tempdir().unwrap();
        let data_dir = temp.path().join("enterprise-data");
        let runtime = Runtime::initialize(data_dir.clone(), Limits::default())
            .await
            .unwrap();
        create_term(&runtime, "persisted-term", "cat", None).await;
        let batch = submit(&runtime, &[("note.txt", b"13900000000")]).await;
        wait_terminal(&runtime, &batch.batch_id).await;
        drop(runtime);

        let restarted = Runtime::initialize(data_dir, Limits::default())
            .await
            .unwrap();
        let terms = list_terms(&restarted, "").await;
        assert_eq!(terms.terms.len(), 1);
        assert_eq!(terms.terms[0].term, "persisted-term");
        let detail = restarted.store.batch_detail(&batch.batch_id).await.unwrap();
        assert_eq!(detail.batch.status, BatchStatus::Completed);
    }

    /// C1/C2/C3/C5/E2/E3/E5: the core immutable-snapshot behaviour. Creates
    /// one enabled sensitive term, runs it through both a direct batch and a
    /// preview→confirm using the same content (E5: identical masked output
    /// and counts), then disables the term and proves two things at once:
    /// a *new* batch created afterwards no longer masks it (C2's "future
    /// tasks use the latest library"), while *retrying* the original file —
    /// whose batch row's snapshot was frozen before the edit — still masks
    /// it exactly as before (C3's "retry uses the task's original
    /// snapshot"). Also scans every JSON response body for the raw term
    /// text and the internal snapshot sentinel prefix (C5).
    #[tokio::test]
    async fn sensitive_term_snapshot_is_frozen_at_creation_and_survives_edit_disable_and_retry() {
        let (_temp, runtime) = test_runtime().await;
        let term = create_term(&runtime, "内部代号", "机密", None).await;
        let content: &[u8] = "项目 内部代号 联系人 13900000000".as_bytes();
        let rules = r#"["phone","use_sensitive_terms"]"#;

        // E5: direct batch and preview must mask identically under the same
        // live snapshot.
        let batch = submit_preview_with_rules(&runtime, &[("a.txt", content)], rules).await;
        let batch_preview_detail = wait_preview_terminal(&runtime, &batch.preview_id).await;
        assert_eq!(batch_preview_detail.status, PreviewSessionStatus::Ready);
        assert_eq!(
            batch_preview_detail.masked_entity_count, 2,
            "E2: phone + sensitive term"
        );

        let (content_type, body) = multipart(&[("a.txt", content)], rules);
        let direct = request()
            .method("POST")
            .path("/api/v1/batches")
            .header("content-type", content_type)
            .body(body)
            .reply(&routes(runtime.clone()))
            .await;
        assert_eq!(direct.status(), StatusCode::ACCEPTED);
        let direct: CreateBatchResponse = serde_json::from_slice(direct.body()).unwrap();
        let direct_detail = wait_terminal(&runtime, &direct.batch_id).await;
        assert_eq!(direct_detail.batch.masked_entity_count, 2);
        let direct_file = &direct_detail.files[0];
        let direct_artifact_id = direct_file.artifact_id.clone().unwrap();
        let direct_response = request()
            .method("GET")
            .path(&format!("/api/v1/artifacts/{direct_artifact_id}"))
            .reply(&routes(runtime.clone()))
            .await;
        let direct_markdown = direct_response.body().to_vec();

        // Confirm the preview and compare its artifact byte-for-byte (E3).
        let confirm = request()
            .method("POST")
            .path(&format!("/api/v1/previews/{}/confirm", batch.preview_id))
            .reply(&routes(runtime.clone()))
            .await;
        assert_eq!(confirm.status(), StatusCode::OK);
        let confirm: ConfirmPreviewResponse = serde_json::from_slice(confirm.body()).unwrap();
        let confirmed_detail = wait_terminal(&runtime, &confirm.batch_id).await;
        let confirmed_artifact_id = confirmed_detail.files[0].artifact_id.clone().unwrap();
        let confirmed_response = request()
            .method("GET")
            .path(&format!("/api/v1/artifacts/{confirmed_artifact_id}"))
            .reply(&routes(runtime.clone()))
            .await;
        let confirmed_markdown = confirmed_response.body().to_vec();

        assert_eq!(
            Sha256::digest(&direct_markdown),
            Sha256::digest(&confirmed_markdown),
            "E5/E3: preview and direct batch must mask identically under the same snapshot"
        );
        let masked_text = std::str::from_utf8(&direct_markdown).unwrap();
        assert!(masked_text.contains("[机密]"));
        assert!(masked_text.contains("***PHONE***1"));
        assert!(
            !masked_text.contains("内部代号"),
            "E2: masked output must never contain the raw term"
        );

        // C5: scan every response body collected so far for the raw term
        // text and the internal snapshot channel's sentinel prefix — only
        // the correctly-masked artifact bodies may legitimately be absent
        // of it (already asserted above); the *metadata* responses must
        // never carry it at all.
        for body in [
            batch_preview_detail_json(&batch_preview_detail),
            direct_detail_json(&direct_detail),
        ] {
            assert!(
                !body.contains("内部代号"),
                "raw sensitive term leaked into API metadata: {body}"
            );
            assert!(
                !body.contains("__sensitive_terms_snapshot__"),
                "internal snapshot channel leaked: {body}"
            );
        }

        // Now disable the term.
        update_term_response(&runtime, &term.id, serde_json::json!({ "enabled": false })).await;

        // C2: a brand-new batch created after the edit must NOT mask it.
        let (content_type, body) = multipart(&[("b.txt", content)], rules);
        let after_disable = request()
            .method("POST")
            .path("/api/v1/batches")
            .header("content-type", content_type)
            .body(body)
            .reply(&routes(runtime.clone()))
            .await;
        let after_disable: CreateBatchResponse =
            serde_json::from_slice(after_disable.body()).unwrap();
        let after_disable_detail = wait_terminal(&runtime, &after_disable.batch_id).await;
        assert_eq!(
            after_disable_detail.batch.masked_entity_count, 1,
            "C2: only phone should mask once the sensitive term is disabled"
        );

        // C3: a batch's own frozen snapshot column is immutable — retry
        // reads it via the exact same `claim_next_pending` query used for
        // first-attempt processing, so proving the stored column survives
        // the edit unchanged proves retry cannot observe the edit either.
        // (A full worker-driven retry-after-success would additionally hit
        // this store's `UNIQUE(file_id)` constraint on `artifacts` — that
        // constraint exists precisely because production retry is only ever
        // offered for genuinely `Failed` files, which never got an artifact
        // row in the first place; forcing a *Completed* file back to
        // `Failed` purely to reprocess it is not a real state this product
        // reaches, so this test proves the mechanism at the layer retry
        // actually depends on instead.)
        let original_snapshot = runtime
            .store
            .sensitive_terms_snapshot_json_for_batch(&direct.batch_id)
            .await
            .unwrap();
        assert!(
            original_snapshot.as_deref().is_some_and(|json| json.contains("内部代号")),
            "the original batch's frozen snapshot must still contain the term after it was disabled: {original_snapshot:?}"
        );
        let after_disable_snapshot = runtime
            .store
            .sensitive_terms_snapshot_json_for_batch(&after_disable.batch_id)
            .await
            .unwrap();
        assert_eq!(
            after_disable_snapshot.as_deref(),
            Some("[]"),
            "C2: a batch created after disabling the only term must snapshot an empty set"
        );
    }

    /// C3 (full worker end-to-end, not a storage-layer inference): a file
    /// that genuinely fails *before* ever producing an artifact — created
    /// via `Store::create_batch` directly so the HTTP handler's
    /// `wake_worker.notify_one()` is never fired, giving a real window
    /// before the background worker's own idle-timeout poll to force a
    /// legitimate pre-success failure via the same `force_failed` path a
    /// crash/interrupt would take — is then, after the sensitive term is
    /// disabled, retried through the real `POST /api/v1/files/{id}/retry`
    /// endpoint and picked up by the same live background worker
    /// (`test_runtime()`'s `spawn_worker`) for a genuine second processing
    /// pass. The assertion is on the *masked output* the real worker
    /// produced, not on an unchanged storage column.
    #[tokio::test]
    async fn sensitive_term_retry_reprocesses_through_the_real_worker_using_the_original_snapshot()
    {
        let (_temp, runtime) = test_runtime().await;
        let term = create_term(&runtime, "内部代号三", "机密", None).await;
        let content: &[u8] = "项目 内部代号三 联系人 13900000000".as_bytes();

        // Bypass the HTTP handler so the worker is not woken immediately;
        // this leaves a real window (worker's own 200ms idle poll) to force
        // a genuine failure before any artifact is written.
        let created = runtime
            .store
            .create_batch(
                vec![NewUpload {
                    display_name: "retry-real.txt".into(),
                    input_format: "text".into(),
                    bytes: content.to_vec(),
                }],
                vec!["phone".to_string(), "use_sensitive_terms".to_string()],
                "server_cmap",
            )
            .await
            .unwrap();
        let file_id = created.files[0].file_id.clone();

        runtime.store.force_failed(&file_id).await.unwrap();

        // Precondition the review asked for: genuinely Failed, no artifact.
        let before_retry = runtime.store.batch_detail(&created.batch_id).await.unwrap();
        assert_eq!(
            before_retry.files[0].status,
            FileStatus::Failed,
            "precondition: file must be Failed before any artifact was ever produced"
        );
        assert!(
            before_retry.files[0].artifact_id.is_none(),
            "precondition: no artifact must exist yet — this is not a forced-after-success state"
        );

        // Disable the term while the file sits Failed.
        let disable =
            update_term_response(&runtime, &term.id, serde_json::json!({ "enabled": false })).await;
        assert_eq!(disable.status(), StatusCode::OK);

        // Real retry endpoint.
        let retry = request()
            .method("POST")
            .path(&format!("/api/v1/files/{file_id}/retry"))
            .reply(&routes(runtime.clone()))
            .await;
        assert_eq!(retry.status(), StatusCode::OK);

        // Real second pass by the live background worker (not manually
        // invoked processing/store functions).
        let after_retry = wait_terminal(&runtime, &created.batch_id).await;
        assert_eq!(after_retry.files[0].status, FileStatus::Completed);
        assert_eq!(after_retry.files[0].attempt, 2);
        assert_eq!(
            after_retry.files[0].masked_entity_count,
            Some(2),
            "the real second worker pass must still mask both phone and the sensitive term \
             using the snapshot frozen at batch creation, despite the term now being disabled"
        );
        let artifact_id = after_retry.files[0].artifact_id.clone().unwrap();
        let artifact_response = request()
            .method("GET")
            .path(&format!("/api/v1/artifacts/{artifact_id}"))
            .reply(&routes(runtime.clone()))
            .await;
        let markdown = String::from_utf8(artifact_response.body().to_vec()).unwrap();
        assert!(markdown.contains("[机密]"), "retry output: {markdown}");
        assert!(
            markdown.contains("***PHONE***1"),
            "retry output: {markdown}"
        );
        assert!(
            !markdown.contains("内部代号三"),
            "retry output must never contain the raw term: {markdown}"
        );
    }

    /// Claims a fresh single-file batch and returns `(batch_id, job)` with
    /// the real `Queued`+`ProcessingStarted` job_events already written by
    /// production code — every OL test below drives genuine store methods
    /// (`write_completed`/`mark_failed`/`retry`) rather than forging rows,
    /// so the resulting log projection is exercising the real event trail.
    async fn create_and_claim(runtime: &Runtime, display_name: &str) -> (String, PendingJob) {
        let created = runtime
            .store
            .create_batch(
                vec![NewUpload {
                    display_name: display_name.to_string(),
                    input_format: "text".to_string(),
                    bytes: b"fixture content".to_vec(),
                }],
                vec!["phone".to_string()],
                "disabled",
            )
            .await
            .unwrap();
        let job = runtime.store.claim_next_pending().await.unwrap().unwrap();
        (created.batch_id, job)
    }

    /// B1/B3/§4/§5: real Queued/ProcessingStarted/Completed/Failed job
    /// events and real restore success/failure events project through the
    /// browser API with the correct fixed level mapping and the exact safe
    /// field set — restore events (no batch/file association in the
    /// schema) leave those fields `None` rather than guessing one.
    #[tokio::test]
    async fn operation_log_projects_job_and_restore_events_with_correct_levels_and_safe_fields() {
        let (temp, runtime) = test_runtime_without_workers().await;

        let (batch_id, job) = create_and_claim(&runtime, "success-fixture.txt").await;
        runtime
            .store
            .write_completed(&job, b"[masked]", 2, "artifact-success-1", None)
            .await
            .unwrap();

        let (fail_batch_id, fail_job) = create_and_claim(&runtime, "fail-fixture.txt").await;
        runtime
            .store
            .mark_failed(&fail_job, "INPUT_CORRUPTED", "fixture failure")
            .await
            .unwrap();

        runtime
            .store
            .log_restore_event("RestoreSucceeded", "completed", None, Some(3))
            .await
            .unwrap();
        runtime
            .store
            .log_restore_event("RestoreFailed", "failed", Some("CMAP_AUTH_FAILED"), None)
            .await
            .unwrap();

        let response = request()
            .method("GET")
            .path("/api/v1/operation-logs?page_size=100")
            .reply(&routes(runtime.clone()))
            .await;
        assert_eq!(response.status(), StatusCode::OK);
        let list: OperationLogListResponse = serde_json::from_slice(response.body()).unwrap();

        // Queued+ProcessingStarted+Completed, Queued+ProcessingStarted+Failed, 2 restore events.
        assert_eq!(list.total_count, 8);
        assert_eq!(list.entries.len(), 8);

        let completed = list
            .entries
            .iter()
            .find(|entry| entry.event_type == "Completed")
            .expect("Completed event present");
        assert_eq!(
            completed.level,
            service_contracts::OperationLogLevel::Success
        );
        assert_eq!(completed.batch_id.as_deref(), Some(batch_id.as_str()));
        assert_eq!(completed.file_id.as_deref(), Some(job.file_id.as_str()));
        assert_eq!(
            completed.display_name.as_deref(),
            Some("success-fixture.txt")
        );
        assert_eq!(completed.input_format.as_deref(), Some("text"));
        assert_eq!(completed.masked_entity_count, Some(2));
        assert_eq!(completed.error_code, None);
        assert_eq!(completed.restored_entity_count, None);

        let failed = list
            .entries
            .iter()
            .find(|entry| entry.event_type == "Failed")
            .expect("Failed event present");
        assert_eq!(failed.level, service_contracts::OperationLogLevel::Error);
        assert_eq!(failed.batch_id.as_deref(), Some(fail_batch_id.as_str()));
        assert_eq!(failed.error_code.as_deref(), Some("INPUT_CORRUPTED"));

        for event_type in ["Queued", "ProcessingStarted"] {
            let matching: Vec<_> = list
                .entries
                .iter()
                .filter(|e| e.event_type == event_type)
                .collect();
            assert_eq!(matching.len(), 2, "{event_type}");
            for entry in matching {
                assert_eq!(
                    entry.level,
                    service_contracts::OperationLogLevel::Info,
                    "{event_type}"
                );
            }
        }

        let restore_success = list
            .entries
            .iter()
            .find(|entry| entry.event_type == "RestoreSucceeded")
            .expect("restore success event present");
        assert_eq!(
            restore_success.level,
            service_contracts::OperationLogLevel::Success
        );
        assert_eq!(restore_success.batch_id, None);
        assert_eq!(restore_success.file_id, None);
        assert_eq!(
            restore_success.display_name, None,
            "restore events must never guess a file association"
        );
        assert_eq!(restore_success.input_format, None);
        assert_eq!(restore_success.masked_entity_count, None);
        assert_eq!(restore_success.restored_entity_count, Some(3));

        let restore_failure = list
            .entries
            .iter()
            .find(|entry| entry.event_type == "RestoreFailed")
            .expect("restore failure event present");
        assert_eq!(
            restore_failure.level,
            service_contracts::OperationLogLevel::Error
        );
        assert_eq!(
            restore_failure.error_code.as_deref(),
            Some("CMAP_AUTH_FAILED")
        );

        let body_text = std::str::from_utf8(response.body()).unwrap();
        let temp_root = temp.path().to_string_lossy();
        assert!(
            !body_text.contains(temp_root.as_ref()),
            "response leaked the local data directory path"
        );
        assert!(
            !body_text.contains("fixture content"),
            "response leaked original text"
        );
        assert!(
            !body_text.contains(".cmap"),
            "response leaked mapping-file naming"
        );
    }

    /// A4: stable combined level/status/batch-id-prefix filtering, where
    /// `count`(`total_count`) and the returned `rows` always agree, and
    /// pagination never overlaps or drops rows across pages.
    #[tokio::test]
    async fn operation_log_list_supports_stable_pagination_and_combined_filters() {
        let (_temp, runtime) = test_runtime_without_workers().await;

        let created = runtime
            .store
            .create_batch(
                vec![
                    NewUpload {
                        display_name: "a.txt".into(),
                        input_format: "text".into(),
                        bytes: b"a".to_vec(),
                    },
                    NewUpload {
                        display_name: "b.txt".into(),
                        input_format: "text".into(),
                        bytes: b"b".to_vec(),
                    },
                    NewUpload {
                        display_name: "c.txt".into(),
                        input_format: "text".into(),
                        bytes: b"c".to_vec(),
                    },
                ],
                vec!["phone".to_string()],
                "disabled",
            )
            .await
            .unwrap();
        let batch_id = created.batch_id.clone();

        let job_a = runtime.store.claim_next_pending().await.unwrap().unwrap();
        runtime
            .store
            .write_completed(&job_a, b"x", 1, "artifact-a", None)
            .await
            .unwrap();
        let job_b = runtime.store.claim_next_pending().await.unwrap().unwrap();
        runtime
            .store
            .mark_failed(&job_b, "INPUT_CORRUPTED", "fixture")
            .await
            .unwrap();
        let job_c = runtime.store.claim_next_pending().await.unwrap().unwrap();
        runtime
            .store
            .write_completed(&job_c, b"y", 1, "artifact-c", None)
            .await
            .unwrap();

        // 3x Queued, 3x ProcessingStarted, 2x Completed, 1x Failed = 9.
        let all = request()
            .method("GET")
            .path(&format!(
                "/api/v1/operation-logs?batch_id={batch_id}&page_size=100"
            ))
            .reply(&routes(runtime.clone()))
            .await;
        let all_list: OperationLogListResponse = serde_json::from_slice(all.body()).unwrap();
        assert_eq!(all_list.total_count, 9);

        let level_filtered = request()
            .method("GET")
            .path(&format!(
                "/api/v1/operation-logs?batch_id={batch_id}&level=success&page_size=100"
            ))
            .reply(&routes(runtime.clone()))
            .await;
        let level_list: OperationLogListResponse =
            serde_json::from_slice(level_filtered.body()).unwrap();
        assert_eq!(level_list.total_count, 2);
        assert_eq!(level_list.entries.len(), 2);
        assert!(level_list
            .entries
            .iter()
            .all(|e| e.event_type == "Completed"));

        let status_filtered = request()
            .method("GET")
            .path(&format!(
                "/api/v1/operation-logs?batch_id={batch_id}&status=Failed&page_size=100"
            ))
            .reply(&routes(runtime.clone()))
            .await;
        let status_list: OperationLogListResponse =
            serde_json::from_slice(status_filtered.body()).unwrap();
        assert_eq!(status_list.total_count, 1);
        assert_eq!(status_list.entries[0].event_type, "Failed");

        let prefix = &batch_id[..8];
        let prefix_filtered = request()
            .method("GET")
            .path(&format!(
                "/api/v1/operation-logs?batch_id={prefix}&page_size=100"
            ))
            .reply(&routes(runtime.clone()))
            .await;
        let prefix_list: OperationLogListResponse =
            serde_json::from_slice(prefix_filtered.body()).unwrap();
        assert_eq!(prefix_list.total_count, 9);

        let page1 = request()
            .method("GET")
            .path(&format!(
                "/api/v1/operation-logs?batch_id={batch_id}&page=1&page_size=4"
            ))
            .reply(&routes(runtime.clone()))
            .await;
        let page1_list: OperationLogListResponse = serde_json::from_slice(page1.body()).unwrap();
        assert_eq!(page1_list.entries.len(), 4);
        assert_eq!(page1_list.total_count, 9);
        assert_eq!(page1_list.total_pages, 3);

        let page3 = request()
            .method("GET")
            .path(&format!(
                "/api/v1/operation-logs?batch_id={batch_id}&page=3&page_size=4"
            ))
            .reply(&routes(runtime.clone()))
            .await;
        let page3_list: OperationLogListResponse = serde_json::from_slice(page3.body()).unwrap();
        assert_eq!(page3_list.entries.len(), 1);
        assert_eq!(page3_list.total_count, 9);

        let page1_ids: std::collections::HashSet<_> = page1_list
            .entries
            .iter()
            .map(|e| e.event_id.clone())
            .collect();
        let page3_ids: std::collections::HashSet<_> = page3_list
            .entries
            .iter()
            .map(|e| e.event_id.clone())
            .collect();
        assert!(page1_ids.is_disjoint(&page3_ids), "pages must not overlap");

        let no_match = request()
            .method("GET")
            .path("/api/v1/operation-logs?batch_id=00000000")
            .reply(&routes(runtime.clone()))
            .await;
        let no_match_list: OperationLogListResponse =
            serde_json::from_slice(no_match.body()).unwrap();
        assert_eq!(no_match_list.total_count, 0);
        assert_eq!(no_match_list.entries.len(), 0);
        assert_eq!(no_match_list.total_pages, 0);
    }

    /// A5: an event_type this Runtime binary does not recognize (e.g. from
    /// a newer/older version) still lists safely as `info` — never a 500,
    /// never dropped from the page silently.
    #[tokio::test]
    async fn operation_log_unknown_event_type_degrades_to_info_without_500() {
        let (_temp, runtime) = test_runtime_without_workers().await;
        let (batch_id, job) = create_and_claim(&runtime, "future-event.txt").await;
        runtime
            .store
            .write_completed(&job, b"x", 1, "artifact-future", None)
            .await
            .unwrap();
        runtime
            .store
            .insert_raw_job_event(
                &batch_id,
                &job.file_id,
                "SomeFutureEventType",
                "Unknown",
                &chrono::Utc::now().to_rfc3339(),
            )
            .await
            .unwrap();

        let response = request()
            .method("GET")
            .path(&format!(
                "/api/v1/operation-logs?batch_id={batch_id}&page_size=100"
            ))
            .reply(&routes(runtime.clone()))
            .await;
        assert_eq!(response.status(), StatusCode::OK);
        let list: OperationLogListResponse = serde_json::from_slice(response.body()).unwrap();
        let unknown = list
            .entries
            .iter()
            .find(|entry| entry.event_type == "SomeFutureEventType")
            .expect("unknown event type must still be listed");
        assert_eq!(unknown.level, service_contracts::OperationLogLevel::Info);

        let filtered = request()
            .method("GET")
            .path(&format!(
                "/api/v1/operation-logs?batch_id={batch_id}&level=info&page_size=100"
            ))
            .reply(&routes(runtime.clone()))
            .await;
        assert_eq!(filtered.status(), StatusCode::OK);
        let filtered_list: OperationLogListResponse =
            serde_json::from_slice(filtered.body()).unwrap();
        assert!(filtered_list
            .entries
            .iter()
            .any(|entry| entry.event_type == "SomeFutureEventType"));
    }

    /// B2/B3: statistics counts/masked-items/success-rate/recent-7-days
    /// come straight from `batches`/`batch_files`, never from the current
    /// log page.
    #[tokio::test]
    async fn operation_log_statistics_counts_and_masked_items_from_authoritative_batch_data() {
        let (temp, runtime) = test_runtime_without_workers().await;

        let (_batch_a, job_a) = create_and_claim(&runtime, "stat-success.txt").await;
        runtime
            .store
            .write_completed(&job_a, b"x", 3, "artifact-stat-success", None)
            .await
            .unwrap();

        let (_batch_b, job_b) = create_and_claim(&runtime, "stat-failure.txt").await;
        runtime
            .store
            .mark_failed(&job_b, "INPUT_CORRUPTED", "fixture")
            .await
            .unwrap();

        let response = request()
            .method("GET")
            .path("/api/v1/operation-logs/statistics")
            .reply(&routes(runtime.clone()))
            .await;
        assert_eq!(response.status(), StatusCode::OK);
        let stats: OperationLogStatistics = serde_json::from_slice(response.body()).unwrap();
        assert_eq!(stats.total_files, 2);
        assert_eq!(stats.successful_files, 1);
        assert_eq!(stats.failed_files, 1);
        assert_eq!(stats.total_masked_items, 3);
        assert_eq!(stats.success_rate, 50.0);
        assert_eq!(stats.recent_files_7days, 2);

        let body_text = std::str::from_utf8(response.body()).unwrap();
        let temp_root = temp.path().to_string_lossy();
        assert!(!body_text.contains(temp_root.as_ref()));
    }

    /// §6: retry rounds are paired as "each start with its own immediately
    /// following terminal event" using deterministic, hand-authored
    /// timestamps — proving the pairing is exact, not just "some positive
    /// number". A `RecoveredInterrupted` round (no genuine Completed/Failed
    /// following its start) must contribute zero samples, never a
    /// crash-inflated duration standing in for real processing time.
    #[tokio::test]
    async fn operation_log_statistics_pairs_multi_round_processing_time_using_only_genuine_terminal_events(
    ) {
        let (_temp, runtime) = test_runtime().await;
        let batch_id = "11111111-1111-1111-1111-111111111111";

        let file_a = "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa";
        runtime
            .store
            .insert_raw_job_event(
                batch_id,
                file_a,
                "ProcessingStarted",
                "Processing",
                "2026-01-01T00:00:00.000Z",
            )
            .await
            .unwrap();
        runtime
            .store
            .insert_raw_job_event(
                batch_id,
                file_a,
                "Completed",
                "Completed",
                "2026-01-01T00:00:02.000Z",
            )
            .await
            .unwrap();

        let file_b = "bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb";
        runtime
            .store
            .insert_raw_job_event(
                batch_id,
                file_b,
                "ProcessingStarted",
                "Processing",
                "2026-01-01T00:00:00.000Z",
            )
            .await
            .unwrap();
        runtime
            .store
            .insert_raw_job_event(
                batch_id,
                file_b,
                "Failed",
                "Failed",
                "2026-01-01T00:00:01.000Z",
            )
            .await
            .unwrap();
        runtime
            .store
            .insert_raw_job_event(
                batch_id,
                file_b,
                "RetryQueued",
                "Pending",
                "2026-01-01T00:00:02.000Z",
            )
            .await
            .unwrap();
        runtime
            .store
            .insert_raw_job_event(
                batch_id,
                file_b,
                "ProcessingStarted",
                "Processing",
                "2026-01-01T00:00:05.000Z",
            )
            .await
            .unwrap();
        runtime
            .store
            .insert_raw_job_event(
                batch_id,
                file_b,
                "Completed",
                "Completed",
                "2026-01-01T00:00:08.000Z",
            )
            .await
            .unwrap();

        let file_c = "cccccccc-cccc-cccc-cccc-cccccccccccc";
        runtime
            .store
            .insert_raw_job_event(
                batch_id,
                file_c,
                "ProcessingStarted",
                "Processing",
                "2026-01-01T00:00:00.000Z",
            )
            .await
            .unwrap();
        runtime
            .store
            .insert_raw_job_event(
                batch_id,
                file_c,
                "RecoveredInterrupted",
                "Failed",
                "2026-01-01T01:00:00.000Z",
            )
            .await
            .unwrap();

        let response = request()
            .method("GET")
            .path("/api/v1/operation-logs/statistics")
            .reply(&routes(runtime.clone()))
            .await;
        let stats: OperationLogStatistics = serde_json::from_slice(response.body()).unwrap();
        // (2000 + 1000 + 3000) / 3 = 2000ms exactly; file C's interrupted
        // round must not contribute any sample.
        assert_eq!(stats.average_processing_time_ms, 2000);
    }

    /// C1-C5/E5: clearing only removes `job_events`/`restore_events`; every
    /// batch/file/artifact/mapping/sensitive-term record, byte and count
    /// survives, statistics keep reading from that surviving data (average
    /// time legitimately falls to 0 since its source events are gone), and
    /// a fresh event after the clear shows up again.
    #[tokio::test]
    async fn operation_log_clear_only_deletes_events_and_preserves_all_other_data() {
        let (_temp, runtime) = test_runtime_without_workers().await;

        let (batch_id, job) = create_and_claim(&runtime, "clear-fixture.txt").await;
        runtime
            .store
            .write_completed(
                &job,
                b"clear fixture masked content",
                1,
                "artifact-clear",
                None,
            )
            .await
            .unwrap();
        runtime
            .store
            .log_restore_event("RestoreSucceeded", "completed", None, Some(1))
            .await
            .unwrap();
        let term = runtime
            .store
            .create_sensitive_term("clear词", "clear分类", None)
            .await
            .unwrap();

        let before_counts = runtime.store.record_counts().await.unwrap();
        let before_detail = runtime.store.batch_detail(&batch_id).await.unwrap();
        let artifact_id = before_detail.files[0].artifact_id.clone().unwrap();
        let before_artifact = request()
            .method("GET")
            .path(&format!("/api/v1/artifacts/{artifact_id}"))
            .reply(&routes(runtime.clone()))
            .await;
        let before_artifact_body = before_artifact.body().to_vec();
        let before_terms = runtime
            .store
            .list_sensitive_terms(None, None, false)
            .await
            .unwrap();

        let events_before = request()
            .method("GET")
            .path("/api/v1/operation-logs?page_size=100")
            .reply(&routes(runtime.clone()))
            .await;
        let events_before_list: OperationLogListResponse =
            serde_json::from_slice(events_before.body()).unwrap();
        assert!(events_before_list.total_count > 0);

        let clear = request()
            .method("DELETE")
            .path("/api/v1/operation-logs")
            .reply(&routes(runtime.clone()))
            .await;
        assert_eq!(clear.status(), StatusCode::OK);
        let clear_response: ClearOperationLogsResponse =
            serde_json::from_slice(clear.body()).unwrap();
        assert_eq!(clear_response.deleted_job_events, 3);
        assert_eq!(clear_response.deleted_restore_events, 1);

        let events_after = request()
            .method("GET")
            .path("/api/v1/operation-logs?page_size=100")
            .reply(&routes(runtime.clone()))
            .await;
        let events_after_list: OperationLogListResponse =
            serde_json::from_slice(events_after.body()).unwrap();
        assert_eq!(events_after_list.total_count, 0);

        assert_eq!(runtime.store.record_counts().await.unwrap(), before_counts);
        let after_detail = runtime.store.batch_detail(&batch_id).await.unwrap();
        assert_eq!(
            after_detail.files[0].artifact_id,
            before_detail.files[0].artifact_id
        );
        assert_eq!(after_detail.files[0].status, before_detail.files[0].status);
        let after_artifact = request()
            .method("GET")
            .path(&format!("/api/v1/artifacts/{artifact_id}"))
            .reply(&routes(runtime.clone()))
            .await;
        assert_eq!(after_artifact.body().to_vec(), before_artifact_body);
        let after_terms = runtime
            .store
            .list_sensitive_terms(None, None, false)
            .await
            .unwrap();
        assert_eq!(after_terms, before_terms);
        assert_eq!(after_terms[0].id, term.id);

        let stats_response = request()
            .method("GET")
            .path("/api/v1/operation-logs/statistics")
            .reply(&routes(runtime.clone()))
            .await;
        let stats: OperationLogStatistics = serde_json::from_slice(stats_response.body()).unwrap();
        assert_eq!(stats.total_files, 1);
        assert_eq!(stats.successful_files, 1);
        assert_eq!(stats.average_processing_time_ms, 0);

        let (_batch2, job2) = create_and_claim(&runtime, "after-clear.txt").await;
        runtime
            .store
            .write_completed(&job2, b"y", 1, "artifact-after-clear", None)
            .await
            .unwrap();
        let events_new = request()
            .method("GET")
            .path("/api/v1/operation-logs?page_size=100")
            .reply(&routes(runtime.clone()))
            .await;
        let events_new_list: OperationLogListResponse =
            serde_json::from_slice(events_new.body()).unwrap();
        assert!(
            events_new_list.total_count > 0,
            "new events must appear after a clear"
        );
    }

    /// §7/B4: storage status only ever exposes a ready flag, the event
    /// count and the Runtime's own version — never the sqlite file name or
    /// the local data directory path.
    #[tokio::test]
    async fn operation_log_storage_status_reports_ready_and_never_leaks_paths() {
        let (temp, runtime) = test_runtime_without_workers().await;
        let (_batch_id, job) = create_and_claim(&runtime, "storage-status.txt").await;
        runtime
            .store
            .write_completed(&job, b"x", 1, "artifact-storage-status", None)
            .await
            .unwrap();

        let response = request()
            .method("GET")
            .path("/api/v1/operation-logs/storage-status")
            .reply(&routes(runtime.clone()))
            .await;
        assert_eq!(response.status(), StatusCode::OK);
        let status: OperationLogStorageStatus = serde_json::from_slice(response.body()).unwrap();
        assert_eq!(status.status, "ready");
        assert_eq!(status.event_count, 3);
        assert_eq!(status.runtime_version, VERSION);

        let body_text = std::str::from_utf8(response.body()).unwrap();
        assert!(!body_text.contains("vault-pro.db"));
        let temp_root = temp.path().to_string_lossy();
        assert!(!body_text.contains(temp_root.as_ref()));
    }

    /// 安全约束 3-5/8.4: illegal `page`/`page_size`/`level`/`batch_id`
    /// query parameters are rejected with a safe `INVALID_QUERY` error
    /// before ever reaching the database — never a raw 500, never a SQL
    /// fragment echoed back.
    #[tokio::test]
    async fn operation_log_query_validation_rejects_illegal_parameters() {
        let (_temp, runtime) = test_runtime().await;

        for query in [
            "page=0",
            "page_size=0",
            "page_size=101",
            "level=critical",
            "batch_id=",
            "batch_id=has%20space",
            "batch_id=DROP%20TABLE",
        ] {
            let response = request()
                .method("GET")
                .path(&format!("/api/v1/operation-logs?{query}"))
                .reply(&routes(runtime.clone()))
                .await;
            assert_eq!(response.status(), StatusCode::BAD_REQUEST, "query: {query}");
            let error: ErrorResponse = serde_json::from_slice(response.body()).unwrap();
            assert_eq!(error.code, "INVALID_QUERY", "query: {query}");
            assert!(
                !error.message.to_uppercase().contains("SELECT"),
                "must not leak SQL: {query}"
            );
        }

        let long_prefix = "a".repeat(40);
        let response = request()
            .method("GET")
            .path(&format!("/api/v1/operation-logs?batch_id={long_prefix}"))
            .reply(&routes(runtime.clone()))
            .await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    /// A3: each operation-log endpoint must convert an actual storage failure
    /// into the same safe error shape. The test drops one of the source event
    /// tables after Runtime initialization, then exercises every route; this
    /// is a real StoreError path, not a mocked HTTP response.
    #[tokio::test]
    async fn operation_log_apis_return_safe_storage_errors_for_all_endpoints() {
        for (method, path) in [
            ("GET", "/api/v1/operation-logs"),
            ("GET", "/api/v1/operation-logs/statistics"),
            ("GET", "/api/v1/operation-logs/storage-status"),
            ("DELETE", "/api/v1/operation-logs"),
        ] {
            let (_temp, runtime) = test_runtime().await;
            runtime
                .store
                .drop_operation_log_tables_for_test()
                .await
                .unwrap();

            let response = request()
                .method(method)
                .path(path)
                .reply(&routes(runtime.clone()))
                .await;
            assert_eq!(
                response.status(),
                StatusCode::INTERNAL_SERVER_ERROR,
                "{method} {path}"
            );

            let error: ErrorResponse = serde_json::from_slice(response.body()).unwrap();
            assert_eq!(error.code, "STORAGE_INTERNAL_ERROR", "{method} {path}");
            assert_eq!(
                error.message, "Operation log storage operation failed",
                "{method} {path}"
            );
            assert!(error.retryable, "{method} {path} must be retryable");

            let body = std::str::from_utf8(response.body()).unwrap();
            for forbidden in ["SELECT", "INSERT", "DELETE", "sqlite", "stack", "/Users/"] {
                assert!(
                    !body.contains(forbidden),
                    "{method} {path} leaked {forbidden}: {body}"
                );
            }
        }
    }

    /// E4: the operation log, its filters and its statistics all survive a
    /// Runtime restart pointed at the same data directory.
    #[tokio::test]
    async fn operation_log_persists_across_a_runtime_restart() {
        let temp = tempfile::tempdir().unwrap();
        let data_root = temp.path().join("enterprise-data");
        // First Runtime: no background workers, so create_and_claim owns the claim.
        let runtime = Runtime::initialize_without_workers(data_root.clone(), Limits::default())
            .await
            .unwrap();

        let (batch_id, job) = create_and_claim(&runtime, "restart-fixture.txt").await;
        runtime
            .store
            .write_completed(&job, b"x", 1, "artifact-restart", None)
            .await
            .unwrap();

        let before = request()
            .method("GET")
            .path(&format!(
                "/api/v1/operation-logs?batch_id={batch_id}&page_size=100"
            ))
            .reply(&routes(runtime.clone()))
            .await;
        let before_list: OperationLogListResponse = serde_json::from_slice(before.body()).unwrap();
        assert_eq!(before_list.total_count, 3);

        let stats_before = request()
            .method("GET")
            .path("/api/v1/operation-logs/statistics")
            .reply(&routes(runtime.clone()))
            .await;
        let stats_before_body: OperationLogStatistics =
            serde_json::from_slice(stats_before.body()).unwrap();

        drop(runtime);

        let restarted = Runtime::initialize(data_root, Limits::default())
            .await
            .unwrap();
        let after = request()
            .method("GET")
            .path(&format!(
                "/api/v1/operation-logs?batch_id={batch_id}&page_size=100"
            ))
            .reply(&routes(restarted.clone()))
            .await;
        let after_list: OperationLogListResponse = serde_json::from_slice(after.body()).unwrap();
        assert_eq!(after_list.total_count, 3);
        assert_eq!(after_list.entries, before_list.entries);

        let stats_after = request()
            .method("GET")
            .path("/api/v1/operation-logs/statistics")
            .reply(&routes(restarted))
            .await;
        let stats_after_body: OperationLogStatistics =
            serde_json::from_slice(stats_after.body()).unwrap();
        assert_eq!(stats_after_body, stats_before_body);
    }

    /// B1/E1: a batch created through the real preview→confirm flow (the
    /// only path the browser `/process` page actually uses) must be visible
    /// in the operation log too — this test was added after live browser
    /// E2E surfaced that `confirm_preview_locked` wrote no `job_events` at
    /// all, making every confirmed batch invisible on `/log`. Each
    /// confirmed file gets exactly one genuine terminal event (Completed/
    /// Failed); since there is no real "start" timestamp for preview-
    /// resolved files, none of them are paired into the average-processing-
    /// time statistic (§6) — that is correct, not a bug (非目标 6: no
    /// invented durations).
    #[tokio::test]
    async fn confirmed_preview_batch_writes_genuine_terminal_operation_log_events() {
        let (_temp, runtime) = test_runtime().await;
        let created = submit_preview(
            &runtime,
            &[("ok.txt", b"Call 13900000000"), ("bad.md", b"%PDF-1.7\n")],
        )
        .await;
        let preview_detail = wait_preview_terminal(&runtime, &created.preview_id).await;
        assert_eq!(preview_detail.status, PreviewSessionStatus::ReadyWithErrors);

        let response = request()
            .method("POST")
            .path(&format!("/api/v1/previews/{}/confirm", created.preview_id))
            .reply(&routes(runtime.clone()))
            .await;
        assert_eq!(response.status(), StatusCode::OK);
        let confirm: ConfirmPreviewResponse = serde_json::from_slice(response.body()).unwrap();

        let log_response = request()
            .method("GET")
            .path(&format!(
                "/api/v1/operation-logs?batch_id={}&page_size=20",
                confirm.batch_id
            ))
            .reply(&routes(runtime.clone()))
            .await;
        assert_eq!(log_response.status(), StatusCode::OK);
        let list: OperationLogListResponse = serde_json::from_slice(log_response.body()).unwrap();
        assert_eq!(
            list.total_count, 2,
            "confirmed batch must produce exactly one event per file"
        );

        let completed = list
            .entries
            .iter()
            .find(|entry| entry.event_type == "Completed")
            .expect("Completed event for the successful preview file");
        assert_eq!(
            completed.level,
            service_contracts::OperationLogLevel::Success
        );
        assert_eq!(completed.display_name.as_deref(), Some("ok.txt"));
        assert_eq!(completed.masked_entity_count, Some(1));
        assert_eq!(
            completed.batch_id.as_deref(),
            Some(confirm.batch_id.as_str())
        );

        let failed = list
            .entries
            .iter()
            .find(|entry| entry.event_type == "Failed")
            .expect("Failed event for the failed preview file");
        assert_eq!(failed.level, service_contracts::OperationLogLevel::Error);
        assert_eq!(failed.display_name.as_deref(), Some("bad.md"));
        assert_eq!(failed.batch_id.as_deref(), Some(confirm.batch_id.as_str()));

        // Neither event has a paired ProcessingStarted, so it must not
        // fabricate a processing duration.
        let stats_response = request()
            .method("GET")
            .path("/api/v1/operation-logs/statistics")
            .reply(&routes(runtime.clone()))
            .await;
        let stats: OperationLogStatistics = serde_json::from_slice(stats_response.body()).unwrap();
        assert_eq!(stats.average_processing_time_ms, 0);
        assert_eq!(stats.total_files, 2);
        assert_eq!(stats.successful_files, 1);
        assert_eq!(stats.failed_files, 1);
    }

    fn batch_preview_detail_json(detail: &service_contracts::PreviewDetail) -> String {
        serde_json::to_string(detail).unwrap()
    }

    fn direct_detail_json(detail: &BatchDetail) -> String {
        serde_json::to_string(detail).unwrap()
    }

    // ===== Sandbox/PIN (SANDBOX-PIN-BOUNDARY-001, acceptance B1-B5) =====

    use sandbox_core::{Clock, RateLimiter};
    use service_contracts::{
        ClearSandboxPinRequest, SandboxStatusResponse, SetSandboxPinRequest, UnlockSandboxRequest,
        SANDBOX_STORAGE_MODE_SERVER_SYSTEM_USER,
    };

    /// Deterministic, manually-advanced clock for HTTP-layer rate-limit
    /// tests — mirrors `sandbox-core`'s own unit-test `FakeClock` but is
    /// reachable from here since that one is private to `rate_limit.rs`.
    /// Never sleeps; time only moves via `advance()`.
    struct FakeClock {
        now: std::sync::Mutex<std::time::Instant>,
    }

    impl FakeClock {
        fn new() -> Arc<Self> {
            Arc::new(Self {
                now: std::sync::Mutex::new(std::time::Instant::now()),
            })
        }
        fn advance(&self, d: Duration) {
            *self.now.lock().unwrap() += d;
        }
    }

    impl Clock for FakeClock {
        fn now(&self) -> std::time::Instant {
            *self.now.lock().unwrap()
        }
    }

    /// A Runtime whose sandbox rate limiter is backed by a `FakeClock`
    /// instead of the real system clock, so threshold/block/recovery tests
    /// never wait a real 5 minutes (核心设计约束 6).
    async fn test_runtime_with_fake_sandbox_clock() -> (TempDir, Runtime, Arc<FakeClock>) {
        let (temp, mut runtime) = test_runtime_without_workers().await;
        let clock = FakeClock::new();
        let rate_limiter = RateLimiter::new(
            clock.clone() as Arc<dyn Clock>,
            Duration::from_secs(300),
            5,
            Duration::from_secs(300),
        );
        let pin_file = temp.path().join("enterprise-data/sandbox/pin.phc");
        runtime.sandbox = Arc::new(
            sandbox::SandboxSession::new_with_rate_limiter(pin_file, rate_limiter).unwrap(),
        );
        (temp, runtime, clock)
    }

    async fn sandbox_status(runtime: &Runtime) -> (StatusCode, SandboxStatusResponse) {
        let response = request()
            .method("GET")
            .path("/api/v1/sandbox/status")
            .reply(&routes(runtime.clone()))
            .await;
        let status = response.status();
        (status, serde_json::from_slice(response.body()).unwrap())
    }

    async fn sandbox_set_pin(
        runtime: &Runtime,
        current_pin: Option<&str>,
        new_pin: &str,
    ) -> warp::http::Response<bytes::Bytes> {
        request()
            .method("PUT")
            .path("/api/v1/sandbox/pin")
            .json(&SetSandboxPinRequest {
                new_pin: new_pin.to_string(),
                current_pin: current_pin.map(str::to_string),
            })
            .reply(&routes(runtime.clone()))
            .await
    }

    async fn sandbox_unlock(runtime: &Runtime, pin: &str) -> warp::http::Response<bytes::Bytes> {
        request()
            .method("POST")
            .path("/api/v1/sandbox/unlock")
            .json(&UnlockSandboxRequest {
                pin: pin.to_string(),
            })
            .reply(&routes(runtime.clone()))
            .await
    }

    async fn sandbox_lock(runtime: &Runtime) -> warp::http::Response<bytes::Bytes> {
        request()
            .method("POST")
            .path("/api/v1/sandbox/lock")
            .reply(&routes(runtime.clone()))
            .await
    }

    async fn sandbox_clear_pin(
        runtime: &Runtime,
        current_pin: &str,
    ) -> warp::http::Response<bytes::Bytes> {
        request()
            .method("DELETE")
            .path("/api/v1/sandbox/pin")
            .json(&ClearSandboxPinRequest {
                current_pin: current_pin.to_string(),
            })
            .reply(&routes(runtime.clone()))
            .await
    }

    #[tokio::test]
    async fn sandbox_status_reports_no_pin_and_unlocked_by_default() {
        let (_temp, runtime) = test_runtime_without_workers().await;
        let (status, body) = sandbox_status(&runtime).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            body,
            SandboxStatusResponse {
                pin_configured: false,
                locked: false,
                storage_mode: SANDBOX_STORAGE_MODE_SERVER_SYSTEM_USER.to_string(),
                rate_limited: false,
                retry_after_seconds: None,
            }
        );
    }

    #[tokio::test]
    async fn sandbox_first_time_set_pin_needs_no_current_pin_and_locks() {
        let (_temp, runtime) = test_runtime_without_workers().await;
        let response = sandbox_set_pin(&runtime, None, "1234").await;
        assert_eq!(response.status(), StatusCode::OK);
        let body: SandboxStatusResponse = serde_json::from_slice(response.body()).unwrap();
        assert!(body.pin_configured);
        assert!(body.locked);

        let (status, get_body) = sandbox_status(&runtime).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(get_body, body);
    }

    #[tokio::test]
    async fn sandbox_set_pin_missing_new_pin_field_is_rejected() {
        let (_temp, runtime) = test_runtime_without_workers().await;
        let response = request()
            .method("PUT")
            .path("/api/v1/sandbox/pin")
            .header("content-type", "application/json")
            .body("{}")
            .reply(&routes(runtime.clone()))
            .await;
        assert_ne!(response.status(), StatusCode::OK);
        let error: ErrorResponse = serde_json::from_slice(response.body()).unwrap();
        assert!(!error.code.is_empty());
        let (status, _) = sandbox_status(&runtime).await;
        assert_eq!(status, StatusCode::OK);
    }

    #[tokio::test]
    async fn sandbox_set_pin_body_over_the_4kib_limit_is_rejected_before_argon2() {
        let (_temp, runtime) = test_runtime_without_workers().await;
        let oversized_pin = "a".repeat(8192);
        let response = request()
            .method("PUT")
            .path("/api/v1/sandbox/pin")
            .json(&SetSandboxPinRequest {
                new_pin: oversized_pin,
                current_pin: None,
            })
            .reply(&routes(runtime.clone()))
            .await;
        assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
        let (status, body) = sandbox_status(&runtime).await;
        assert_eq!(status, StatusCode::OK);
        assert!(
            !body.pin_configured,
            "oversized body must never reach Argon2/storage"
        );
    }

    #[tokio::test]
    async fn sandbox_set_pin_below_min_length_is_rejected_with_invalid_length_and_does_not_configure(
    ) {
        let (_temp, runtime) = test_runtime_without_workers().await;
        let response = sandbox_set_pin(&runtime, None, "12").await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let error: ErrorResponse = serde_json::from_slice(response.body()).unwrap();
        assert_eq!(error.code, "SANDBOX_PIN_INVALID");
        let (_, body) = sandbox_status(&runtime).await;
        assert!(!body.pin_configured);
    }

    #[tokio::test]
    async fn sandbox_wrong_http_method_on_status_is_rejected_not_500() {
        let (_temp, runtime) = test_runtime_without_workers().await;
        let response = request()
            .method("POST")
            .path("/api/v1/sandbox/status")
            .reply(&routes(runtime.clone()))
            .await;
        assert_ne!(response.status(), StatusCode::OK);
        assert!(response.status().is_client_error());
        // Must still be a well-formed, safe ErrorResponse, not a raw panic body.
        let _: ErrorResponse = serde_json::from_slice(response.body()).unwrap();
    }

    #[tokio::test]
    async fn sandbox_lock_without_a_configured_pin_returns_a_safe_conflict() {
        let (_temp, runtime) = test_runtime_without_workers().await;
        let response = sandbox_lock(&runtime).await;
        assert_eq!(response.status(), StatusCode::CONFLICT);
        let error: ErrorResponse = serde_json::from_slice(response.body()).unwrap();
        assert_eq!(error.code, "SANDBOX_PIN_NOT_CONFIGURED");
    }

    #[tokio::test]
    async fn sandbox_unlock_lock_round_trip_with_the_correct_pin() {
        let (_temp, runtime) = test_runtime_without_workers().await;
        assert_eq!(
            sandbox_set_pin(&runtime, None, "1234").await.status(),
            StatusCode::OK
        );

        let unlocked = sandbox_unlock(&runtime, "1234").await;
        assert_eq!(unlocked.status(), StatusCode::OK);
        let body: SandboxStatusResponse = serde_json::from_slice(unlocked.body()).unwrap();
        assert!(!body.locked);

        let locked = sandbox_lock(&runtime).await;
        assert_eq!(locked.status(), StatusCode::OK);
        let body: SandboxStatusResponse = serde_json::from_slice(locked.body()).unwrap();
        assert!(body.locked);
    }

    #[tokio::test]
    async fn sandbox_unlock_with_wrong_pin_is_unauthorized_and_stays_locked() {
        let (_temp, runtime) = test_runtime_without_workers().await;
        assert_eq!(
            sandbox_set_pin(&runtime, None, "1234").await.status(),
            StatusCode::OK
        );

        let response = sandbox_unlock(&runtime, "9999").await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        let error: ErrorResponse = serde_json::from_slice(response.body()).unwrap();
        assert_eq!(error.code, "SANDBOX_PIN_INVALID");

        let (_, body) = sandbox_status(&runtime).await;
        assert!(body.locked, "a wrong PIN must never unlock the sandbox");
    }

    #[tokio::test]
    async fn sandbox_replace_pin_with_wrong_current_pin_leaves_the_old_pin_working() {
        let (_temp, runtime) = test_runtime_without_workers().await;
        assert_eq!(
            sandbox_set_pin(&runtime, None, "1111").await.status(),
            StatusCode::OK
        );

        let response = sandbox_set_pin(&runtime, Some("9999"), "2222").await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        let error: ErrorResponse = serde_json::from_slice(response.body()).unwrap();
        assert_eq!(error.code, "SANDBOX_PIN_INVALID");

        // The hash must be unchanged: the original PIN still unlocks, the
        // rejected replacement PIN does not.
        assert_eq!(
            sandbox_unlock(&runtime, "1111").await.status(),
            StatusCode::OK
        );
        assert_eq!(sandbox_lock(&runtime).await.status(), StatusCode::OK);
        assert_eq!(
            sandbox_unlock(&runtime, "2222").await.status(),
            StatusCode::UNAUTHORIZED
        );
    }

    #[tokio::test]
    async fn sandbox_replace_pin_with_correct_current_pin_rotates_the_hash() {
        let (_temp, runtime) = test_runtime_without_workers().await;
        assert_eq!(
            sandbox_set_pin(&runtime, None, "1111").await.status(),
            StatusCode::OK
        );
        assert_eq!(
            sandbox_set_pin(&runtime, Some("1111"), "2222")
                .await
                .status(),
            StatusCode::OK
        );

        assert_eq!(
            sandbox_unlock(&runtime, "1111").await.status(),
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            sandbox_unlock(&runtime, "2222").await.status(),
            StatusCode::OK
        );
    }

    #[tokio::test]
    async fn sandbox_clear_pin_with_wrong_current_pin_leaves_the_pin_configured() {
        let (_temp, runtime) = test_runtime_without_workers().await;
        assert_eq!(
            sandbox_set_pin(&runtime, None, "1234").await.status(),
            StatusCode::OK
        );

        let response = sandbox_clear_pin(&runtime, "9999").await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        let error: ErrorResponse = serde_json::from_slice(response.body()).unwrap();
        assert_eq!(error.code, "SANDBOX_PIN_INVALID");

        let (_, body) = sandbox_status(&runtime).await;
        assert!(
            body.pin_configured,
            "a wrong current PIN must never clear the stored hash"
        );
    }

    #[tokio::test]
    async fn sandbox_clear_pin_with_correct_current_pin_removes_it_and_unlocks() {
        let (_temp, runtime) = test_runtime_without_workers().await;
        assert_eq!(
            sandbox_set_pin(&runtime, None, "1234").await.status(),
            StatusCode::OK
        );

        let response = sandbox_clear_pin(&runtime, "1234").await;
        assert_eq!(response.status(), StatusCode::OK);
        let body: SandboxStatusResponse = serde_json::from_slice(response.body()).unwrap();
        assert!(!body.pin_configured);
        assert!(!body.locked);

        // First-time semantics apply again: setting a fresh PIN needs no
        // current PIN.
        assert_eq!(
            sandbox_set_pin(&runtime, None, "5678").await.status(),
            StatusCode::OK
        );
    }

    #[tokio::test]
    async fn sandbox_corrupted_pin_file_surfaces_as_a_safe_storage_error_not_a_panic() {
        let (temp, runtime) = test_runtime_without_workers().await;
        assert_eq!(
            sandbox_set_pin(&runtime, None, "1234").await.status(),
            StatusCode::OK
        );

        let pin_file = temp.path().join("enterprise-data/sandbox/pin.phc");
        std::fs::write(&pin_file, b"not a valid argon2 phc string at all").unwrap();

        let response = sandbox_unlock(&runtime, "1234").await;
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let error: ErrorResponse = serde_json::from_slice(response.body()).unwrap();
        assert_eq!(error.code, "SANDBOX_PIN_STORAGE_FAILED");
        let raw = String::from_utf8_lossy(response.body());
        assert!(!raw.contains(&temp.path().display().to_string()));
    }

    #[tokio::test]
    async fn sandbox_five_failed_unlocks_trigger_a_global_rate_limit_with_retry_after() {
        let (_temp, runtime, _clock) = test_runtime_with_fake_sandbox_clock().await;
        assert_eq!(
            sandbox_set_pin(&runtime, None, "1234").await.status(),
            StatusCode::OK
        );

        for _ in 0..5 {
            assert_eq!(
                sandbox_unlock(&runtime, "wrong").await.status(),
                StatusCode::UNAUTHORIZED
            );
        }

        // Even the *correct* PIN must be refused once blocked.
        let response = sandbox_unlock(&runtime, "1234").await;
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
        let retry_after = response
            .headers()
            .get("retry-after")
            .expect("429 must carry a Retry-After header")
            .to_str()
            .unwrap();
        assert_eq!(retry_after, "300");
        let error: ErrorResponse = serde_json::from_slice(response.body()).unwrap();
        assert_eq!(error.code, "SANDBOX_PIN_RATE_LIMITED");

        let (_, status_body) = sandbox_status(&runtime).await;
        assert!(status_body.rate_limited);
        assert_eq!(status_body.retry_after_seconds, Some(300));
    }

    #[tokio::test]
    async fn sandbox_rate_limit_recovers_after_the_block_duration_via_fake_clock_no_real_sleep() {
        let (_temp, runtime, clock) = test_runtime_with_fake_sandbox_clock().await;
        assert_eq!(
            sandbox_set_pin(&runtime, None, "1234").await.status(),
            StatusCode::OK
        );

        for _ in 0..5 {
            sandbox_unlock(&runtime, "wrong").await;
        }
        assert_eq!(
            sandbox_unlock(&runtime, "1234").await.status(),
            StatusCode::TOO_MANY_REQUESTS
        );

        clock.advance(Duration::from_secs(299));
        assert_eq!(
            sandbox_unlock(&runtime, "1234").await.status(),
            StatusCode::TOO_MANY_REQUESTS,
            "must still be blocked one second before the block elapses"
        );

        clock.advance(Duration::from_secs(2));
        let response = sandbox_unlock(&runtime, "1234").await;
        assert_eq!(response.status(), StatusCode::OK);
        let body: SandboxStatusResponse = serde_json::from_slice(response.body()).unwrap();
        assert!(!body.locked);
    }

    #[tokio::test]
    async fn sandbox_a_successful_unlock_clears_the_failure_counter() {
        let (_temp, runtime, _clock) = test_runtime_with_fake_sandbox_clock().await;
        assert_eq!(
            sandbox_set_pin(&runtime, None, "1234").await.status(),
            StatusCode::OK
        );

        for _ in 0..4 {
            assert_eq!(
                sandbox_unlock(&runtime, "wrong").await.status(),
                StatusCode::UNAUTHORIZED
            );
        }
        assert_eq!(
            sandbox_unlock(&runtime, "1234").await.status(),
            StatusCode::OK
        );
        assert_eq!(sandbox_lock(&runtime).await.status(), StatusCode::OK);

        // 4 more failures after the reset must still not trip the block.
        for _ in 0..4 {
            assert_eq!(
                sandbox_unlock(&runtime, "wrong").await.status(),
                StatusCode::UNAUTHORIZED
            );
        }
        assert_eq!(
            sandbox_unlock(&runtime, "1234").await.status(),
            StatusCode::OK
        );
    }

    #[tokio::test]
    async fn sandbox_concurrent_failed_unlocks_cannot_race_past_the_threshold() {
        let (_temp, runtime, _clock) = test_runtime_with_fake_sandbox_clock().await;
        assert_eq!(
            sandbox_set_pin(&runtime, None, "1234").await.status(),
            StatusCode::OK
        );

        let mut handles = Vec::new();
        for _ in 0..10 {
            let runtime = runtime.clone();
            handles.push(tokio::spawn(async move {
                sandbox_unlock(&runtime, "wrong").await.status()
            }));
        }
        for h in handles {
            h.await.unwrap();
        }

        // Regardless of interleaving, 10 concurrent failures must engage the
        // block exactly once and never unlock the sandbox.
        let (_, status_body) = sandbox_status(&runtime).await;
        assert!(status_body.rate_limited);
        assert!(status_body.locked);
        assert_eq!(
            sandbox_unlock(&runtime, "1234").await.status(),
            StatusCode::TOO_MANY_REQUESTS,
            "a blocked caller must be refused even with the correct PIN"
        );
    }

    #[tokio::test]
    async fn sandbox_responses_never_contain_the_pin_hash_salt_or_storage_path() {
        let (temp, runtime) = test_runtime_without_workers().await;
        let set_response = sandbox_set_pin(&runtime, None, "1234").await;
        let unlock_response = sandbox_unlock(&runtime, "wrong-guess").await;
        let status_response = request()
            .method("GET")
            .path("/api/v1/sandbox/status")
            .reply(&routes(runtime.clone()))
            .await;

        let pin_file = temp.path().join("enterprise-data/sandbox/pin.phc");
        let phc = std::fs::read_to_string(&pin_file).unwrap();
        assert!(phc.starts_with("$argon2id$"));

        for response in [&set_response, &unlock_response, &status_response] {
            let body = String::from_utf8_lossy(response.body());
            assert!(!body.contains("1234"), "must never echo the PIN back");
            assert!(
                !body.contains("wrong-guess"),
                "must never echo a failed guess back"
            );
            assert!(!body.contains(&phc), "must never echo the stored PHC hash");
            assert!(
                !body.contains("$argon2id$"),
                "must never echo Argon2 internals"
            );
            assert!(
                !body.contains(&temp.path().display().to_string()),
                "must never echo the server-side storage path"
            );
        }
    }

    #[tokio::test]
    async fn sandbox_locked_state_never_gates_the_existing_enterprise_batch_api() {
        let (_temp, runtime) = test_runtime().await;
        assert_eq!(
            sandbox_set_pin(&runtime, None, "1234").await.status(),
            StatusCode::OK
        );
        let (_, body) = sandbox_status(&runtime).await;
        assert!(
            body.locked,
            "sandbox must be locked after first-time PIN set"
        );

        // The pre-existing enterprise batch pipeline must be completely
        // unaffected by the sandbox's locked state (非目标 2, 已确认产品语义 7).
        let created = submit(&runtime, &[("safe.txt", b"13900000000")]).await;
        assert!(!created.batch_id.is_empty());
        let ids = batch_ids(&runtime).await;
        assert!(ids.contains(&created.batch_id));

        let health = request()
            .method("GET")
            .path("/api/v1/health")
            .reply(&routes(runtime.clone()))
            .await;
        assert_eq!(health.status(), StatusCode::OK);
    }

    // -----------------------------------------------------------------
    // FileBay adapter (browser upload) tests
    // -----------------------------------------------------------------

    use filebay_core::{testing::FakeTransport, Endpoint, Token};
    use service_contracts::{
        FileBayCandidatesResponse, FileBayConfigStatus, FileBayRepositoryResponse,
        FileBayRepositoryStatus, FileBayStatusResponse, FileBayTestResponse, FileBayUploadItem,
        FileBayUploadRequest, FileBayUploadResponse, OperationLogLevel,
    };

    fn fake_filebay_target() -> (Endpoint, String, String, Token) {
        (
            Endpoint::parse("https://filebay.example.com").unwrap(),
            "test-owner".to_string(),
            "test-repo".to_string(),
            Token::new("fake-token-for-tests-never-a-real-credential"),
        )
    }

    /// Swaps in a `FileBaySession::new_for_test` session backed by a fresh
    /// `FakeTransport` — never a real network call, never a real Token —
    /// while keeping the same underlying `store` (so batches created
    /// through `runtime` before this call remain visible).
    fn with_configured_filebay(mut runtime: Runtime) -> (Runtime, Arc<FakeTransport>) {
        let (endpoint, owner, repo, token) = fake_filebay_target();
        let transport = Arc::new(FakeTransport::new());
        runtime.filebay = Arc::new(filebay::FileBaySession::new_for_test(
            FileBayConfigStatus::Configured,
            Some(endpoint),
            Some(owner),
            Some(repo),
            Some(token),
            transport.clone(),
        ));
        (runtime, transport)
    }

    fn with_unconfigured_filebay(mut runtime: Runtime) -> (Runtime, Arc<FakeTransport>) {
        let transport = Arc::new(FakeTransport::new());
        runtime.filebay = Arc::new(filebay::FileBaySession::new_for_test(
            FileBayConfigStatus::Unconfigured,
            None,
            None,
            None,
            None,
            transport.clone(),
        ));
        (runtime, transport)
    }

    fn with_invalid_filebay(mut runtime: Runtime) -> (Runtime, Arc<FakeTransport>) {
        let transport = Arc::new(FakeTransport::new());
        runtime.filebay = Arc::new(filebay::FileBaySession::new_for_test(
            FileBayConfigStatus::Invalid,
            None,
            None,
            None,
            None,
            transport.clone(),
        ));
        (runtime, transport)
    }

    #[tokio::test]
    async fn filebay_status_reports_unconfigured_configured_and_invalid_without_any_transport_call()
    {
        let (_temp, runtime) = test_runtime_without_workers().await;

        let (unconfigured, unconfigured_transport) = with_unconfigured_filebay(runtime.clone());
        let response = request()
            .method("GET")
            .path("/api/v1/filebay/status")
            .reply(&routes(unconfigured))
            .await;
        assert_eq!(response.status(), StatusCode::OK);
        let body: FileBayStatusResponse = serde_json::from_slice(response.body()).unwrap();
        assert_eq!(body.status, FileBayConfigStatus::Unconfigured);
        assert!(!body.configured);
        assert!(!body.has_token);
        assert!(body.target_origin.is_none());
        assert_eq!(unconfigured_transport.call_count(), 0);

        let (invalid, invalid_transport) = with_invalid_filebay(runtime.clone());
        let response = request()
            .method("GET")
            .path("/api/v1/filebay/status")
            .reply(&routes(invalid))
            .await;
        let body: FileBayStatusResponse = serde_json::from_slice(response.body()).unwrap();
        assert_eq!(body.status, FileBayConfigStatus::Invalid);
        assert!(!body.configured);
        assert_eq!(invalid_transport.call_count(), 0);

        let (configured, transport) = with_configured_filebay(runtime);
        let response = request()
            .method("GET")
            .path("/api/v1/filebay/status")
            .reply(&routes(configured))
            .await;
        let body: FileBayStatusResponse = serde_json::from_slice(response.body()).unwrap();
        assert_eq!(body.status, FileBayConfigStatus::Configured);
        assert!(body.configured);
        assert!(body.has_token);
        assert_eq!(
            body.target_origin.as_deref(),
            Some("https://filebay.example.com")
        );
        assert_eq!(body.owner.as_deref(), Some("test-owner"));
        assert_eq!(body.repo.as_deref(), Some("test-repo"));
        assert_eq!(
            transport.call_count(),
            0,
            "status must never touch the network"
        );
    }

    #[tokio::test]
    async fn filebay_status_rejects_non_get_method() {
        let (_temp, runtime) = test_runtime_without_workers().await;
        let (runtime, _transport) = with_unconfigured_filebay(runtime);
        let response = request()
            .method("POST")
            .path("/api/v1/filebay/status")
            .reply(&routes(runtime))
            .await;
        assert_ne!(response.status(), StatusCode::OK);
        assert!(response.status().is_client_error());
        let _: ErrorResponse = serde_json::from_slice(response.body()).unwrap();
    }

    #[tokio::test]
    async fn filebay_test_connection_reports_existing_repository_with_exactly_one_transport_call() {
        let (_temp, runtime) = test_runtime_without_workers().await;
        let (runtime, transport) = with_configured_filebay(runtime);
        transport.stub("/api/v1/repos/test-owner/test-repo", 200, None);

        let response = request()
            .method("POST")
            .path("/api/v1/filebay/test")
            .body("")
            .reply(&routes(runtime))
            .await;
        assert_eq!(response.status(), StatusCode::OK);
        let body: FileBayTestResponse = serde_json::from_slice(response.body()).unwrap();
        assert!(body.repository_exists);
        assert_eq!(transport.call_count(), 1);
    }

    #[tokio::test]
    async fn filebay_test_connection_when_not_configured_is_rejected_before_any_transport_call() {
        let (_temp, runtime) = test_runtime_without_workers().await;
        let (runtime, transport) = with_unconfigured_filebay(runtime);

        let response = request()
            .method("POST")
            .path("/api/v1/filebay/test")
            .body("")
            .reply(&routes(runtime))
            .await;
        assert_eq!(response.status(), StatusCode::CONFLICT);
        let body: ErrorResponse = serde_json::from_slice(response.body()).unwrap();
        assert_eq!(body.code, "FILEBAY_NOT_CONFIGURED");
        assert_eq!(
            transport.call_count(),
            0,
            "an unconfigured session must never reach for its own transport"
        );
    }

    #[tokio::test]
    async fn filebay_test_connection_maps_auth_failure_to_a_safe_error_code() {
        let (_temp, runtime) = test_runtime_without_workers().await;
        let (runtime, transport) = with_configured_filebay(runtime);
        transport.stub("/api/v1/repos/test-owner/test-repo", 401, None);

        let response = request()
            .method("POST")
            .path("/api/v1/filebay/test")
            .body("")
            .reply(&routes(runtime))
            .await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        let body: ErrorResponse = serde_json::from_slice(response.body()).unwrap();
        assert_eq!(body.code, "FILEBAY_AUTH_FAILED");
        let raw = String::from_utf8_lossy(response.body());
        assert!(!raw.contains("fake-token-for-tests"));
    }

    #[tokio::test]
    async fn filebay_repository_creates_when_missing_and_is_idempotent_when_already_present() {
        let (_temp, runtime) = test_runtime_without_workers().await;

        let (runtime_missing, transport_missing) = with_configured_filebay(runtime.clone());
        transport_missing.stub("/api/v1/repos/test-owner/test-repo", 404, None);
        transport_missing.stub("/api/v1/user/repos", 201, None);
        let response = request()
            .method("POST")
            .path("/api/v1/filebay/repository")
            .body("{}")
            .reply(&routes(runtime_missing))
            .await;
        assert_eq!(response.status(), StatusCode::OK);
        let body: FileBayRepositoryResponse = serde_json::from_slice(response.body()).unwrap();
        assert_eq!(body.status, FileBayRepositoryStatus::Created);
        assert_eq!(transport_missing.call_count(), 2);

        let (runtime_present, transport_present) = with_configured_filebay(runtime);
        transport_present.stub("/api/v1/repos/test-owner/test-repo", 200, None);
        let response = request()
            .method("POST")
            .path("/api/v1/filebay/repository")
            .body("{}")
            .reply(&routes(runtime_present))
            .await;
        assert_eq!(response.status(), StatusCode::OK);
        let body: FileBayRepositoryResponse = serde_json::from_slice(response.body()).unwrap();
        assert_eq!(body.status, FileBayRepositoryStatus::Ready);
        assert_eq!(
            transport_present.call_count(),
            1,
            "an already-existing repository must not trigger a create call"
        );
    }

    #[tokio::test]
    async fn filebay_repository_rejects_oversized_body_before_any_transport_call() {
        let (_temp, runtime) = test_runtime_without_workers().await;
        let (runtime, transport) = with_configured_filebay(runtime);
        let oversized = vec![b'a'; 32 * 1024];

        let response = request()
            .method("POST")
            .path("/api/v1/filebay/repository")
            .body(oversized)
            .reply(&routes(runtime))
            .await;
        assert_ne!(response.status(), StatusCode::OK);
        assert_eq!(transport.call_count(), 0);
    }

    #[tokio::test]
    async fn filebay_candidates_only_returns_completed_markdown_and_excludes_failed_and_unknown_batches(
    ) {
        let (_temp, runtime) = test_runtime().await;
        let created = submit(
            &runtime,
            &[("ok.txt", b"13900000000"), ("bad.md", &[0xff, 0xfe])],
        )
        .await;
        let detail = wait_terminal(&runtime, &created.batch_id).await;
        let completed_artifact_id = detail
            .files
            .iter()
            .find(|f| f.status == FileStatus::Completed)
            .and_then(|f| f.artifact_id.clone())
            .expect("one file must complete");
        assert!(
            detail
                .files
                .iter()
                .any(|f| f.status == FileStatus::Failed && f.artifact_id.is_none()),
            "the corrupted markdown file must fail without producing an artifact"
        );

        // Candidates must be servable even while FileBay itself is
        // unconfigured — this endpoint never touches the network (C §7).
        let (runtime, candidates_transport) = with_unconfigured_filebay(runtime);

        let response = request()
            .method("GET")
            .path(&format!(
                "/api/v1/filebay/batches/{}/candidates",
                created.batch_id
            ))
            .reply(&routes(runtime.clone()))
            .await;
        assert_eq!(response.status(), StatusCode::OK);
        let body: FileBayCandidatesResponse = serde_json::from_slice(response.body()).unwrap();
        assert_eq!(body.candidates.len(), 1);
        assert_eq!(body.candidates[0].artifact_id, completed_artifact_id);
        assert_eq!(body.candidates[0].display_name, "ok.txt");
        assert_eq!(
            body.candidates[0].remote_path,
            format!("masked/{completed_artifact_id}-ok.md")
        );

        let empty = request()
            .method("GET")
            .path("/api/v1/filebay/batches/does-not-exist/candidates")
            .reply(&routes(runtime))
            .await;
        assert_eq!(empty.status(), StatusCode::OK);
        let empty_body: FileBayCandidatesResponse = serde_json::from_slice(empty.body()).unwrap();
        assert!(empty_body.candidates.is_empty());
        assert_eq!(candidates_transport.call_count(), 0);
    }

    async fn post_uploads(
        runtime: &Runtime,
        artifact_ids: Vec<String>,
    ) -> warp::http::Response<bytes::Bytes> {
        request()
            .method("POST")
            .path("/api/v1/filebay/uploads")
            .json(&FileBayUploadRequest { artifact_ids })
            .reply(&routes(runtime.clone()))
            .await
    }

    #[tokio::test]
    async fn filebay_uploads_rejects_malformed_requests_before_any_transport_call() {
        let (_temp, runtime) = test_runtime_without_workers().await;
        let (runtime, transport) = with_configured_filebay(runtime);

        let empty = post_uploads(&runtime, vec![]).await;
        assert_eq!(empty.status(), StatusCode::BAD_REQUEST);

        let too_many = post_uploads(
            &runtime,
            (0..101).map(|i| format!("artifact-{i}")).collect(),
        )
        .await;
        assert_eq!(too_many.status(), StatusCode::BAD_REQUEST);

        let duplicated =
            post_uploads(&runtime, vec!["same-id".to_string(), "same-id".to_string()]).await;
        assert_eq!(duplicated.status(), StatusCode::BAD_REQUEST);

        assert_eq!(
            transport.call_count(),
            0,
            "a malformed request must never reach FileBay"
        );
    }

    #[tokio::test]
    async fn filebay_uploads_when_not_configured_is_rejected_before_any_transport_call() {
        let (_temp, runtime) = test_runtime_without_workers().await;
        let (runtime, transport) = with_unconfigured_filebay(runtime);

        let response = post_uploads(&runtime, vec!["some-artifact-id".to_string()]).await;
        let body: ErrorResponse = serde_json::from_slice(response.body()).unwrap();
        assert_eq!(body.code, "FILEBAY_NOT_CONFIGURED");
        assert_eq!(transport.call_count(), 0);
    }

    #[tokio::test]
    async fn filebay_uploads_denies_a_nonexistent_artifact_without_touching_transport() {
        let (_temp, runtime) = test_runtime_without_workers().await;
        let (runtime, transport) = with_configured_filebay(runtime);

        let response = post_uploads(&runtime, vec!["does-not-exist".to_string()]).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body: FileBayUploadResponse = serde_json::from_slice(response.body()).unwrap();
        assert_eq!(body.items.len(), 1);
        let item = &body.items[0];
        assert!(!item.success);
        assert_eq!(item.error_code.as_deref(), Some("FILEBAY_UPLOAD_DENIED"));
        assert_eq!(item.remote_path, "");
        assert_eq!(
            transport.call_count(),
            0,
            "our own whitelist rejection must never touch FileBay"
        );
    }

    #[tokio::test]
    async fn filebay_uploads_succeeds_for_a_verified_artifact_and_returns_its_deterministic_remote_path(
    ) {
        let (_temp, runtime) = test_runtime().await;
        let created = submit(&runtime, &[("ok.txt", b"13900000000")]).await;
        let detail = wait_terminal(&runtime, &created.batch_id).await;
        let artifact_id = detail.files[0].artifact_id.clone().unwrap();

        let (runtime, transport) = with_configured_filebay(runtime);
        transport.stub("/contents/", 404, None);
        transport.stub(
            "/contents/",
            200,
            Some(serde_json::json!({
                "content": {
                    "html_url": "https://filebay.example.com/test-owner/test-repo/blob/master/masked/ok.md",
                }
            })),
        );

        let response = post_uploads(&runtime, vec![artifact_id.clone()]).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body: FileBayUploadResponse = serde_json::from_slice(response.body()).unwrap();
        assert_eq!(body.items.len(), 1);
        let item = &body.items[0];
        assert!(item.success);
        assert_eq!(item.error_code, None);
        assert_eq!(
            item.url.as_deref(),
            Some("https://filebay.example.com/test-owner/test-repo/blob/master/masked/ok.md")
        );
        assert_eq!(item.remote_path, format!("masked/{artifact_id}-ok.md"));
        assert_eq!(transport.call_count(), 2, "one sha lookup, one upload");
    }

    #[tokio::test]
    async fn filebay_uploads_reports_true_partial_failure_without_faking_success() {
        let (_temp, runtime) = test_runtime().await;
        let created = submit(
            &runtime,
            &[("a.txt", b"13900000000"), ("b.txt", b"a@b.com")],
        )
        .await;
        let detail = wait_terminal(&runtime, &created.batch_id).await;
        let artifact_a = detail.files[0].artifact_id.clone().unwrap();
        let artifact_b = detail.files[1].artifact_id.clone().unwrap();

        let (runtime, transport) = with_configured_filebay(runtime);
        // First artifact: sha lookup then a successful upload.
        transport.stub("/contents/", 404, None);
        transport.stub(
            "/contents/",
            200,
            Some(serde_json::json!({"content": {"html_url": "https://filebay.example.com/ok"}})),
        );
        // Second artifact: sha lookup then a failing upload (server error).
        transport.stub("/contents/", 404, None);
        transport.stub("/contents/", 500, None);

        let response = post_uploads(&runtime, vec![artifact_a.clone(), artifact_b.clone()]).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body: FileBayUploadResponse = serde_json::from_slice(response.body()).unwrap();
        assert_eq!(body.items.len(), 2);

        let by_id = |id: &str| body.items.iter().find(|i| i.artifact_id == id).unwrap();
        let first: &FileBayUploadItem = by_id(&artifact_a);
        let second: &FileBayUploadItem = by_id(&artifact_b);
        assert!(first.success, "the first item must genuinely succeed");
        assert!(
            !second.success,
            "the second item's failure must never be reported as success"
        );
        assert_eq!(second.error_code.as_deref(), Some("FILEBAY_UPLOAD_FAILED"));
        assert_eq!(transport.call_count(), 4);
    }

    #[tokio::test]
    async fn filebay_upload_events_are_logged_and_surfaced_through_operation_logs_without_leaking_secrets(
    ) {
        let (_temp, runtime) = test_runtime().await;
        let created = submit(&runtime, &[("ok.txt", b"13900000000")]).await;
        let detail = wait_terminal(&runtime, &created.batch_id).await;
        let artifact_id = detail.files[0].artifact_id.clone().unwrap();

        let (runtime, transport) = with_configured_filebay(runtime);
        transport.stub("/contents/", 404, None);
        transport.stub(
            "/contents/",
            200,
            Some(serde_json::json!({"content": {"html_url": "https://filebay.example.com/ok"}})),
        );
        let upload_response = post_uploads(&runtime, vec![artifact_id.clone()]).await;
        assert_eq!(upload_response.status(), StatusCode::OK);

        let logs_response = request()
            .method("GET")
            .path("/api/v1/operation-logs")
            .reply(&routes(runtime))
            .await;
        assert_eq!(logs_response.status(), StatusCode::OK);
        let logs: OperationLogListResponse = serde_json::from_slice(logs_response.body()).unwrap();
        let entry = logs
            .entries
            .iter()
            .find(|e| {
                e.event_type == "FileBayUpload"
                    && e.file_id == Some(detail.files[0].file_id.clone())
            })
            .expect("the successful FileBay upload must be logged");
        assert_eq!(entry.level, OperationLogLevel::Success);
        assert_eq!(entry.batch_id.as_deref(), Some(created.batch_id.as_str()));
        assert_eq!(entry.error_code, None);

        let raw = String::from_utf8_lossy(logs_response.body());
        assert!(!raw.contains("fake-token-for-tests"));
        assert!(!raw.contains("filebay_events"));
        assert!(
            !raw.to_lowercase().contains("authorization"),
            "operation log must never leak an Authorization header"
        );
    }
}
