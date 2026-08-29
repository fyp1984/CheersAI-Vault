use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileStatus {
    Pending,
    Processing,
    Completed,
    Failed,
}

impl FileStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "Pending",
            Self::Processing => "Processing",
            Self::Completed => "Completed",
            Self::Failed => "Failed",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        [
            Self::Pending,
            Self::Processing,
            Self::Completed,
            Self::Failed,
        ]
        .into_iter()
        .find(|status| status.as_str() == value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BatchStatus {
    Running,
    Completed,
    CompletedWithErrors,
    Failed,
}

impl BatchStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Running => "Running",
            Self::Completed => "Completed",
            Self::CompletedWithErrors => "CompletedWithErrors",
            Self::Failed => "Failed",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        [
            Self::Running,
            Self::Completed,
            Self::CompletedWithErrors,
            Self::Failed,
        ]
        .into_iter()
        .find(|status| status.as_str() == value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuleMetadata {
    pub id: String,
    pub name: String,
    pub enabled_by_default: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RulesResponse {
    pub rules: Vec<RuleMetadata>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreatedFile {
    pub file_id: String,
    pub display_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateBatchResponse {
    pub batch_id: String,
    pub files: Vec<CreatedFile>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BatchSummary {
    pub batch_id: String,
    pub status: BatchStatus,
    pub file_count: usize,
    pub completed_count: usize,
    pub failed_count: usize,
    /// Total count of masked entities across all completed files in this batch,
    /// aggregated by the Runtime from `batch_files.masked_entity_count`.
    /// Zero when no files have completed yet.
    pub masked_entity_count: usize,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BatchListResponse {
    pub batches: Vec<BatchSummary>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactKind {
    Markdown,
    ExcelBundleManifest,
}

impl ArtifactKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Markdown => "markdown",
            Self::ExcelBundleManifest => "excel_bundle_manifest",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        [Self::Markdown, Self::ExcelBundleManifest]
            .into_iter()
            .find(|kind| kind.as_str() == value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BatchFile {
    pub file_id: String,
    pub display_name: String,
    pub input_format: String,
    pub status: FileStatus,
    pub attempt: usize,
    pub masked_entity_count: Option<usize>,
    pub artifact_id: Option<String>,
    pub artifact_kind: Option<ArtifactKind>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    #[serde(default)]
    pub restore_available: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BatchDetail {
    pub batch: BatchSummary,
    pub files: Vec<BatchFile>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetryResponse {
    pub file_id: String,
    pub status: FileStatus,
    pub attempt: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExcelArtifactMemberKind {
    MaskedWorkbook,
    Ecmap,
    EncryptedSource,
    Report,
}

impl ExcelArtifactMemberKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MaskedWorkbook => "masked_workbook",
            Self::Ecmap => "ecmap",
            Self::EncryptedSource => "encrypted_source",
            Self::Report => "report",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        [
            Self::MaskedWorkbook,
            Self::Ecmap,
            Self::EncryptedSource,
            Self::Report,
        ]
        .into_iter()
        .find(|kind| kind.as_str() == value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExcelPersistedFile {
    pub kind: ExcelArtifactMemberKind,
    pub display_name: String,
    pub size_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExcelPersistArtifactsResponse {
    pub batch_id: String,
    pub file_id: String,
    pub artifact_id: String,
    pub persisted_files: Vec<ExcelPersistedFile>,
    pub saved_directory_hint: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExcelArtifactMembersResponse {
    pub artifact_id: String,
    pub batch_id: String,
    pub saved_directory_hint: String,
    pub persisted_files: Vec<ExcelPersistedFile>,
}

/// The two supported enterprise Excel restore modes (R-closeout
/// TASK-EXCEL-OUTPUT-RECOVERY-CONSISTENCY-CLOSEOUT-001, 工作包 C).
///
/// - `PathA`: the server already holds `masked + ecmap + encrypted_source`;
///   the client only supplies the matching passphrase.
/// - `PathB`: the server holds `masked + ecmap`; the client additionally
///   uploads the original file and the matching passphrase.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExcelRestoreMode {
    PathA,
    PathB,
}

impl ExcelRestoreMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PathA => "path_a",
            Self::PathB => "path_b",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "path_a" => Some(Self::PathA),
            "path_b" => Some(Self::PathB),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub code: String,
    pub message: String,
    pub retryable: bool,
}

/// OCR component status (R9).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OcrComponentStatus {
    Unavailable,
    Invalid,
    Ready,
}

impl OcrComponentStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unavailable => "unavailable",
            Self::Invalid => "invalid",
            Self::Ready => "ready",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "unavailable" => Some(Self::Unavailable),
            "invalid" => Some(Self::Invalid),
            "ready" => Some(Self::Ready),
            _ => None,
        }
    }
}

/// OCR status API response — no local paths or internal details (R6).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OcrStatusResponse {
    pub status: String,
    pub model_ready: bool,
    pub timeout_secs: u64,
    pub max_pages: usize,
}

/// Restore security event types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RestoreEventType {
    RestoreSucceeded,
    RestoreFailed,
}

impl RestoreEventType {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RestoreSucceeded => "RestoreSucceeded",
            Self::RestoreFailed => "RestoreFailed",
        }
    }
}

/// Minimal restore security event stored in the database.
/// Contains ONLY safe metadata — no file contents, paths, plaintext, or hashes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RestoreSecurityEvent {
    pub event_id: String,
    pub event_type: String,
    pub timestamp: String,
    pub status: String,
    pub error_code: Option<String>,
    pub restored_entity_count: Option<usize>,
}

/// Two-phase browser preview session status (added in 0.2.0).
///
/// `Confirming`/`Confirmed` are only ever set by the confirm operation
/// itself; the background preview worker only ever transitions a session
/// through `Processing -> Ready | ReadyWithErrors | Failed`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PreviewSessionStatus {
    Processing,
    Ready,
    ReadyWithErrors,
    Failed,
    Confirming,
    Confirmed,
}

impl PreviewSessionStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Processing => "Processing",
            Self::Ready => "Ready",
            Self::ReadyWithErrors => "ReadyWithErrors",
            Self::Failed => "Failed",
            Self::Confirming => "Confirming",
            Self::Confirmed => "Confirmed",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        [
            Self::Processing,
            Self::Ready,
            Self::ReadyWithErrors,
            Self::Failed,
            Self::Confirming,
            Self::Confirmed,
        ]
        .into_iter()
        .find(|status| status.as_str() == value)
    }
}

/// Two-phase browser preview file status (added in 0.2.0).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PreviewFileStatus {
    Pending,
    Processing,
    Ready,
    Failed,
}

impl PreviewFileStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "Pending",
            Self::Processing => "Processing",
            Self::Ready => "Ready",
            Self::Failed => "Failed",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        [Self::Pending, Self::Processing, Self::Ready, Self::Failed]
            .into_iter()
            .find(|status| status.as_str() == value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreatedPreviewFile {
    pub file_id: String,
    pub display_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreatePreviewResponse {
    pub preview_id: String,
    pub files: Vec<CreatedPreviewFile>,
    pub expires_at: String,
}

/// Safe per-file preview metadata. Deliberately excludes original text,
/// masked Markdown body, mapping data and any path/object-key fields —
/// callers must fetch Markdown separately via the content endpoint, which
/// itself only ever returns the masked (never the original) text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreviewFile {
    pub file_id: String,
    pub display_name: String,
    pub input_format: String,
    pub status: PreviewFileStatus,
    pub masked_entity_count: Option<usize>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub content_available: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreviewDetail {
    pub preview_id: String,
    pub status: PreviewSessionStatus,
    pub file_count: usize,
    pub ready_count: usize,
    pub failed_count: usize,
    pub masked_entity_count: usize,
    pub created_at: String,
    pub expires_at: String,
    pub files: Vec<PreviewFile>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfirmPreviewResponse {
    pub preview_id: String,
    pub batch_id: String,
}

/// A single sensitive-term library entry (added in 0.3.0). Shared by the
/// browser HTTP CRUD API and, indirectly, the desktop database projection —
/// field names and shapes are frozen here as the single contract both hosts
/// serialize against.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SensitiveTerm {
    pub id: String,
    pub term: String,
    pub category: String,
    pub description: Option<String>,
    pub enabled: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateSensitiveTermRequest {
    pub term: String,
    pub category: String,
    pub description: Option<String>,
}

/// All fields optional: only fields present are changed, matching the
/// desktop `update_sensitive_term` command's partial-update semantics.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateSensitiveTermRequest {
    pub term: Option<String>,
    pub category: Option<String>,
    pub description: Option<String>,
    pub enabled: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SensitiveTermsResponse {
    pub terms: Vec<SensitiveTerm>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SensitiveTermCategoriesResponse {
    pub categories: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SensitiveTermsStats {
    pub total: usize,
    pub enabled: usize,
    pub disabled: usize,
    pub categories: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SensitiveTermsImportResponse {
    pub imported_count: usize,
}

/// Stable level a browser operation-log entry is bucketed into (added in
/// 0.4.0). Computed server-side from `event_type` via a single fixed
/// mapping — never stored freeform, never derived by the frontend.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationLogLevel {
    Info,
    Success,
    Warning,
    Error,
}

impl OperationLogLevel {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Success => "success",
            Self::Warning => "warning",
            Self::Error => "error",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "info" => Some(Self::Info),
            "success" => Some(Self::Success),
            "warning" => Some(Self::Warning),
            "error" => Some(Self::Error),
            _ => None,
        }
    }
}

/// A single browser-safe operation-log entry (added in 0.4.0), projected
/// directly from the Runtime's existing `job_events`/`restore_events`
/// tables — never a separate parallel log store. Deliberately excludes
/// original/masked text, mapping data, sensitive-term content, full paths,
/// credentials, SQL and stack traces (task §4). `display_name` is always
/// the already-controlled safe display name of a `batch_files` row, never a
/// raw path; restore events (which have no batch/file association in the
/// schema) leave `batch_id`/`file_id`/`display_name`/`input_format`/
/// `masked_entity_count` as `None` rather than guessing an association.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperationLogEntry {
    pub event_id: String,
    pub event_type: String,
    pub timestamp: String,
    pub level: OperationLogLevel,
    pub batch_id: Option<String>,
    pub file_id: Option<String>,
    pub display_name: Option<String>,
    pub input_format: Option<String>,
    pub status: String,
    pub masked_entity_count: Option<usize>,
    pub error_code: Option<String>,
    pub restored_entity_count: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperationLogListResponse {
    pub entries: Vec<OperationLogEntry>,
    pub page: usize,
    pub page_size: usize,
    pub total_count: usize,
    pub total_pages: usize,
}

/// Processing statistics computed directly from the authoritative `batches`/
/// `batch_files`/`job_events` data at query time (added in 0.4.0) — never
/// derived from the current log page. `success_rate` is a 0-100 percentage,
/// zero when there are no terminal (Completed/Failed) files yet.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct OperationLogStatistics {
    pub total_files: usize,
    pub successful_files: usize,
    pub failed_files: usize,
    pub total_masked_items: usize,
    pub success_rate: f64,
    pub recent_files_7days: usize,
    pub average_processing_time_ms: u64,
}

/// Safe storage-status projection (added in 0.4.0) — never a real database
/// path, table name, SQL or host username.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperationLogStorageStatus {
    pub status: String,
    pub event_count: usize,
    pub runtime_version: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClearOperationLogsResponse {
    pub deleted_job_events: usize,
    pub deleted_restore_events: usize,
}

/// Fixed value of [`SandboxStatusResponse::storage_mode`] for the enterprise
/// Runtime: the browser sandbox always operates the single server system
/// user's shared PIN/locked state — never a per-client or per-user store.
pub const SANDBOX_STORAGE_MODE_SERVER_SYSTEM_USER: &str = "server_system_user";

/// `GET /api/v1/sandbox/status` response, and the shared "resulting state"
/// payload returned by every other sandbox endpoint on success. Never
/// contains a path, a PIN, a hash, or a salt — only booleans/counters safe
/// to show directly in the browser UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SandboxStatusResponse {
    pub pin_configured: bool,
    pub locked: bool,
    pub storage_mode: String,
    pub rate_limited: bool,
    pub retry_after_seconds: Option<u64>,
}

/// `PUT /api/v1/sandbox/pin` request body. `current_pin` is required when a
/// PIN is already configured and omitted only for the very first set.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetSandboxPinRequest {
    pub new_pin: String,
    #[serde(default)]
    pub current_pin: Option<String>,
}

/// `POST /api/v1/sandbox/unlock` request body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnlockSandboxRequest {
    pub pin: String,
}

/// `DELETE /api/v1/sandbox/pin` request body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClearSandboxPinRequest {
    pub current_pin: String,
}

/// FileBay admin-environment configuration state, as derived once at
/// Runtime startup from `VAULT_FILEBAY_URL`/`_TOKEN`/`_OWNER`/`_REPO`. Never
/// changes at runtime; the browser cannot influence it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileBayConfigStatus {
    /// All four environment variables are absent.
    Unconfigured,
    /// All four are present and pass validation.
    Configured,
    /// At least one is present but the set is incomplete, or the URL/owner/
    /// repo fail validation.
    Invalid,
}

impl FileBayConfigStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unconfigured => "unconfigured",
            Self::Configured => "configured",
            Self::Invalid => "invalid",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "unconfigured" => Some(Self::Unconfigured),
            "configured" => Some(Self::Configured),
            "invalid" => Some(Self::Invalid),
            _ => None,
        }
    }
}

/// `GET /api/v1/filebay/status` response. Never triggers a FileBay
/// out-of-process request; never contains a token value, only whether one
/// is present.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileBayStatusResponse {
    pub status: FileBayConfigStatus,
    pub configured: bool,
    pub has_token: bool,
    pub target_origin: Option<String>,
    pub owner: Option<String>,
    pub repo: Option<String>,
}

/// `POST /api/v1/filebay/test` response — the one user-initiated, explicit
/// connectivity check that is allowed to reach FileBay.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileBayTestResponse {
    pub repository_exists: bool,
}

/// `POST /api/v1/filebay/repository` response.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileBayRepositoryStatus {
    /// Repository already existed; the call was a no-op.
    Ready,
    /// Repository did not exist and was just created as private.
    Created,
}

impl FileBayRepositoryStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Created => "created",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileBayRepositoryResponse {
    pub status: FileBayRepositoryStatus,
}

/// One `Completed`, upload-eligible Markdown artifact returned by
/// `GET /api/v1/filebay/batches/{batch_id}/candidates`. Never carries an
/// object key, local path, mapping path, or original text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileBayCandidate {
    pub artifact_id: String,
    pub display_name: String,
    pub remote_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileBayCandidatesResponse {
    pub candidates: Vec<FileBayCandidate>,
}

/// `POST /api/v1/filebay/uploads` request body. The **only** field the
/// browser may send; no remote path, file content, URL, token, or local
/// path is ever accepted here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileBayUploadRequest {
    pub artifact_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileBayUploadItem {
    pub artifact_id: String,
    pub remote_path: String,
    pub success: bool,
    pub url: Option<String>,
    pub error_code: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileBayUploadResponse {
    pub items: Vec<FileBayUploadItem>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_strings_round_trip_through_single_contract() {
        for status in [
            FileStatus::Pending,
            FileStatus::Processing,
            FileStatus::Completed,
            FileStatus::Failed,
        ] {
            assert_eq!(FileStatus::parse(status.as_str()), Some(status));
        }
        for status in [
            BatchStatus::Running,
            BatchStatus::Completed,
            BatchStatus::CompletedWithErrors,
            BatchStatus::Failed,
        ] {
            assert_eq!(BatchStatus::parse(status.as_str()), Some(status));
        }
    }

    #[test]
    fn rule_metadata_exposes_only_safe_public_fields() {
        let response = RulesResponse {
            rules: vec![RuleMetadata {
                id: "phone".into(),
                name: "Phone".into(),
                enabled_by_default: true,
            }],
        };
        assert_eq!(response.rules[0].id, "phone");
        assert_eq!(response.rules[0].name, "Phone");
        assert!(response.rules[0].enabled_by_default);
    }

    #[test]
    fn ocr_status_round_trips_through_contract() {
        for status in [
            OcrComponentStatus::Unavailable,
            OcrComponentStatus::Invalid,
            OcrComponentStatus::Ready,
        ] {
            let json = serde_json::to_string(&status).unwrap();
            let parsed: OcrComponentStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed, status);
            assert_eq!(OcrComponentStatus::parse(status.as_str()), Some(status));
        }
    }

    #[test]
    fn ocr_status_response_contains_no_paths() {
        let response = OcrStatusResponse {
            status: "ready".into(),
            model_ready: true,
            timeout_secs: 300,
            max_pages: 200,
        };
        let json = serde_json::to_string(&response).unwrap();
        assert!(!json.contains("/Users/"), "must not contain local path");
        assert!(!json.contains("C:\\"), "must not contain Windows path");
        assert!(!json.contains("pdf_ocr.py"), "must not contain script path");
        assert!(!json.contains("python"), "must not contain python path");
    }

    #[test]
    fn ocr_status_response_field_names_are_stable() {
        let response = OcrStatusResponse {
            status: "ready".into(),
            model_ready: true,
            timeout_secs: 300,
            max_pages: 200,
        };
        let json = serde_json::to_value(&response).unwrap();
        let map = json.as_object().unwrap();
        assert!(map.contains_key("status"));
        assert!(map.contains_key("model_ready"));
        assert!(map.contains_key("timeout_secs"));
        assert!(map.contains_key("max_pages"));
        assert!(!map.contains_key("python"));
        assert!(!map.contains_key("script"));
    }

    #[test]
    fn preview_statuses_round_trip_through_single_contract() {
        for status in [
            PreviewSessionStatus::Processing,
            PreviewSessionStatus::Ready,
            PreviewSessionStatus::ReadyWithErrors,
            PreviewSessionStatus::Failed,
            PreviewSessionStatus::Confirming,
            PreviewSessionStatus::Confirmed,
        ] {
            assert_eq!(PreviewSessionStatus::parse(status.as_str()), Some(status));
            let json = serde_json::to_string(&status).unwrap();
            let parsed: PreviewSessionStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed, status);
        }
        for status in [
            PreviewFileStatus::Pending,
            PreviewFileStatus::Processing,
            PreviewFileStatus::Ready,
            PreviewFileStatus::Failed,
        ] {
            assert_eq!(PreviewFileStatus::parse(status.as_str()), Some(status));
            let json = serde_json::to_string(&status).unwrap();
            let parsed: PreviewFileStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed, status);
        }
    }

    #[test]
    fn preview_session_status_parse_rejects_unknown_values() {
        assert_eq!(PreviewSessionStatus::parse("processing"), None);
        assert_eq!(PreviewSessionStatus::parse(""), None);
        assert_eq!(PreviewFileStatus::parse("ready"), None);
    }

    fn sample_preview_detail() -> PreviewDetail {
        PreviewDetail {
            preview_id: "11111111-1111-1111-1111-111111111111".into(),
            status: PreviewSessionStatus::ReadyWithErrors,
            file_count: 2,
            ready_count: 1,
            failed_count: 1,
            masked_entity_count: 3,
            created_at: "2026-01-01T00:00:00Z".into(),
            expires_at: "2026-01-01T00:30:00Z".into(),
            files: vec![
                PreviewFile {
                    file_id: "22222222-2222-2222-2222-222222222222".into(),
                    display_name: "note.txt".into(),
                    input_format: "text".into(),
                    status: PreviewFileStatus::Ready,
                    masked_entity_count: Some(3),
                    error_code: None,
                    error_message: None,
                    content_available: true,
                },
                PreviewFile {
                    file_id: "33333333-3333-3333-3333-333333333333".into(),
                    display_name: "broken.docx".into(),
                    input_format: "docx".into(),
                    status: PreviewFileStatus::Failed,
                    masked_entity_count: None,
                    error_code: Some("INPUT_CORRUPTED".into()),
                    error_message: Some("DOCX structure is invalid".into()),
                    content_available: false,
                },
            ],
        }
    }

    /// A2: preview response types must never carry original text, mapping
    /// values, path/object-key fields, passphrases or tokens — checked both
    /// by field-name absence and by scanning the serialized JSON text.
    #[test]
    fn preview_detail_and_related_types_contain_no_private_fields() {
        let detail = sample_preview_detail();
        let json = serde_json::to_value(&detail).unwrap();
        let forbidden_keys = [
            "original",
            "mapping",
            "mappings",
            "path",
            "object_key",
            "input_object_key",
            "markdown_object_key",
            "mapping_object_key",
            "password",
            "passphrase",
            "token",
            "markdown",
            "content",
        ];
        fn walk_keys(value: &serde_json::Value, forbidden: &[&str], out: &mut Vec<String>) {
            match value {
                serde_json::Value::Object(map) => {
                    for (key, nested) in map {
                        if forbidden.contains(&key.as_str()) {
                            out.push(key.clone());
                        }
                        walk_keys(nested, forbidden, out);
                    }
                }
                serde_json::Value::Array(items) => {
                    for item in items {
                        walk_keys(item, forbidden, out);
                    }
                }
                _ => {}
            }
        }
        let mut hits = Vec::new();
        walk_keys(&json, &forbidden_keys, &mut hits);
        assert!(hits.is_empty(), "forbidden fields present: {hits:?}");

        let text = serde_json::to_string(&detail).unwrap();
        for needle in ["DOCX structure is invalid"] {
            // Error messages are allowed (they are safe, pre-classified strings);
            // only structural/content fields above are forbidden.
            assert!(text.contains(needle));
        }

        let create_response = CreatePreviewResponse {
            preview_id: "11111111-1111-1111-1111-111111111111".into(),
            files: vec![CreatedPreviewFile {
                file_id: "22222222-2222-2222-2222-222222222222".into(),
                display_name: "note.txt".into(),
            }],
            expires_at: "2026-01-01T00:30:00Z".into(),
        };
        let mut hits = Vec::new();
        walk_keys(
            &serde_json::to_value(&create_response).unwrap(),
            &forbidden_keys,
            &mut hits,
        );
        assert!(hits.is_empty(), "forbidden fields present: {hits:?}");

        let confirm_response = ConfirmPreviewResponse {
            preview_id: "11111111-1111-1111-1111-111111111111".into(),
            batch_id: "44444444-4444-4444-4444-444444444444".into(),
        };
        let mut hits = Vec::new();
        walk_keys(
            &serde_json::to_value(&confirm_response).unwrap(),
            &forbidden_keys,
            &mut hits,
        );
        assert!(hits.is_empty(), "forbidden fields present: {hits:?}");
    }

    /// A4: pre-existing batch/file/error/OCR contracts serialize with an
    /// unchanged field set after the 0.2.0 preview additions.
    #[test]
    fn existing_batch_and_error_contracts_are_unchanged_by_the_0_2_0_additions() {
        let batch_json = serde_json::to_value(BatchSummary {
            batch_id: "b".into(),
            status: BatchStatus::Completed,
            file_count: 1,
            completed_count: 1,
            failed_count: 0,
            masked_entity_count: 1,
            created_at: "t".into(),
            updated_at: "t".into(),
        })
        .unwrap();
        let batch_keys: std::collections::BTreeSet<&str> = batch_json
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(
            batch_keys,
            [
                "batch_id",
                "status",
                "file_count",
                "completed_count",
                "failed_count",
                "masked_entity_count",
                "created_at",
                "updated_at"
            ]
            .into_iter()
            .collect()
        );

        let error_json = serde_json::to_value(ErrorResponse {
            code: "X".into(),
            message: "Y".into(),
            retryable: false,
        })
        .unwrap();
        let error_keys: std::collections::BTreeSet<&str> = error_json
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(
            error_keys,
            ["code", "message", "retryable"].into_iter().collect()
        );
    }

    fn sample_sensitive_term() -> SensitiveTerm {
        SensitiveTerm {
            id: "55555555-5555-5555-5555-555555555555".into(),
            term: "示例词".into(),
            category: "示例分类".into(),
            description: Some("示例描述".into()),
            enabled: true,
            created_at: "2026-01-01T00:00:00Z".into(),
            updated_at: "2026-01-01T00:00:00Z".into(),
        }
    }

    /// A1: sensitive-term contracts round-trip through serde without loss.
    #[test]
    fn sensitive_term_contracts_round_trip_through_single_contract() {
        let term = sample_sensitive_term();
        let json = serde_json::to_string(&term).unwrap();
        let parsed: SensitiveTerm = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, term);

        let create = CreateSensitiveTermRequest {
            term: "示例词".into(),
            category: "示例分类".into(),
            description: None,
        };
        let json = serde_json::to_string(&create).unwrap();
        let parsed: CreateSensitiveTermRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, create);

        let update = UpdateSensitiveTermRequest {
            term: None,
            category: None,
            description: None,
            enabled: Some(false),
        };
        let json = serde_json::to_string(&update).unwrap();
        let parsed: UpdateSensitiveTermRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, update);

        let list = SensitiveTermsResponse {
            terms: vec![term.clone()],
        };
        let json = serde_json::to_string(&list).unwrap();
        let parsed: SensitiveTermsResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, list);

        let categories = SensitiveTermCategoriesResponse {
            categories: vec!["示例分类".into()],
        };
        let json = serde_json::to_string(&categories).unwrap();
        let parsed: SensitiveTermCategoriesResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, categories);

        let stats = SensitiveTermsStats {
            total: 3,
            enabled: 2,
            disabled: 1,
            categories: 1,
        };
        let json = serde_json::to_string(&stats).unwrap();
        let parsed: SensitiveTermsStats = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, stats);

        let import = SensitiveTermsImportResponse { imported_count: 5 };
        let json = serde_json::to_string(&import).unwrap();
        let parsed: SensitiveTermsImportResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, import);
    }

    /// A1/A2: missing/illegal fields on the write contracts are rejected by
    /// serde rather than silently defaulting.
    #[test]
    fn sensitive_term_write_contracts_reject_missing_required_fields() {
        let missing_term = r#"{"category":"示例分类","description":null}"#;
        assert!(serde_json::from_str::<CreateSensitiveTermRequest>(missing_term).is_err());

        let wrong_type = r#"{"term":"x","category":"y","description":null,"enabled":"not-a-bool"}"#;
        assert!(serde_json::from_str::<UpdateSensitiveTermRequest>(wrong_type).is_err());
    }

    /// A2: sensitive-term response contracts never carry raw term content
    /// under any field name other than the intended public `term`/`category`/
    /// `description` display fields — no internal snapshot/path/token keys.
    #[test]
    fn sensitive_term_responses_contain_no_private_fields() {
        let forbidden_keys = [
            "path",
            "object_key",
            "token",
            "password",
            "passphrase",
            "snapshot",
            "pattern",
            "regex",
        ];
        fn walk_keys(value: &serde_json::Value, forbidden: &[&str], out: &mut Vec<String>) {
            match value {
                serde_json::Value::Object(map) => {
                    for (key, nested) in map {
                        if forbidden.contains(&key.as_str()) {
                            out.push(key.clone());
                        }
                        walk_keys(nested, forbidden, out);
                    }
                }
                serde_json::Value::Array(items) => {
                    for item in items {
                        walk_keys(item, forbidden, out);
                    }
                }
                _ => {}
            }
        }
        let list = SensitiveTermsResponse {
            terms: vec![sample_sensitive_term()],
        };
        let mut hits = Vec::new();
        walk_keys(
            &serde_json::to_value(&list).unwrap(),
            &forbidden_keys,
            &mut hits,
        );
        assert!(hits.is_empty(), "forbidden fields present: {hits:?}");
    }

    /// A1: operation-log level round-trips through both the string helper
    /// and serde, and rejects unrecognized/lowercase-mismatched values.
    #[test]
    fn operation_log_level_round_trips_and_rejects_unknown_values() {
        for level in [
            OperationLogLevel::Info,
            OperationLogLevel::Success,
            OperationLogLevel::Warning,
            OperationLogLevel::Error,
        ] {
            assert_eq!(OperationLogLevel::parse(level.as_str()), Some(level));
            let json = serde_json::to_string(&level).unwrap();
            let parsed: OperationLogLevel = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed, level);
        }
        assert_eq!(OperationLogLevel::parse("Info"), None);
        assert_eq!(OperationLogLevel::parse(""), None);
        assert_eq!(OperationLogLevel::parse("critical"), None);
    }

    fn sample_operation_log_entry() -> OperationLogEntry {
        OperationLogEntry {
            event_id: "66666666-6666-6666-6666-666666666666".into(),
            event_type: "Completed".into(),
            timestamp: "2026-01-01T00:00:00Z".into(),
            level: OperationLogLevel::Success,
            batch_id: Some("77777777-7777-7777-7777-777777777777".into()),
            file_id: Some("88888888-8888-8888-8888-888888888888".into()),
            display_name: Some("fixture.txt".into()),
            input_format: Some("text".into()),
            status: "Completed".into(),
            masked_entity_count: Some(2),
            error_code: None,
            restored_entity_count: None,
        }
    }

    /// A1: operation-log contracts round-trip through serde without loss,
    /// including a restore-style entry with no batch/file association.
    #[test]
    fn operation_log_contracts_round_trip_through_single_contract() {
        let entry = sample_operation_log_entry();
        let json = serde_json::to_string(&entry).unwrap();
        let parsed: OperationLogEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, entry);

        let restore_entry = OperationLogEntry {
            event_id: "99999999-9999-9999-9999-999999999999".into(),
            event_type: "RestoreSucceeded".into(),
            timestamp: "2026-01-01T00:05:00Z".into(),
            level: OperationLogLevel::Success,
            batch_id: None,
            file_id: None,
            display_name: None,
            input_format: None,
            status: "completed".into(),
            masked_entity_count: None,
            error_code: None,
            restored_entity_count: Some(3),
        };
        let json = serde_json::to_string(&restore_entry).unwrap();
        let parsed: OperationLogEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, restore_entry);

        let list = OperationLogListResponse {
            entries: vec![entry.clone(), restore_entry.clone()],
            page: 1,
            page_size: 10,
            total_count: 2,
            total_pages: 1,
        };
        let json = serde_json::to_string(&list).unwrap();
        let parsed: OperationLogListResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, list);

        let statistics = OperationLogStatistics {
            total_files: 10,
            successful_files: 8,
            failed_files: 2,
            total_masked_items: 42,
            success_rate: 80.0,
            recent_files_7days: 5,
            average_processing_time_ms: 1234,
        };
        let json = serde_json::to_string(&statistics).unwrap();
        let parsed: OperationLogStatistics = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, statistics);

        let storage_status = OperationLogStorageStatus {
            status: "ready".into(),
            event_count: 12,
            runtime_version: "0.4.0".into(),
        };
        let json = serde_json::to_string(&storage_status).unwrap();
        let parsed: OperationLogStorageStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, storage_status);

        let clear_response = ClearOperationLogsResponse {
            deleted_job_events: 7,
            deleted_restore_events: 1,
        };
        let json = serde_json::to_string(&clear_response).unwrap();
        let parsed: ClearOperationLogsResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, clear_response);
    }

    /// A2/B4: operation-log response contracts never carry original/masked
    /// text, mapping/sensitive-term content, paths, credentials, SQL or
    /// stack-trace fields under any key name.
    #[test]
    fn operation_log_responses_contain_no_private_fields() {
        let forbidden_keys = [
            "original",
            "mapping",
            "mappings",
            "path",
            "object_key",
            "token",
            "password",
            "passphrase",
            "snapshot",
            "sql",
            "stack",
            "database_path",
            "table",
        ];
        fn walk_keys(value: &serde_json::Value, forbidden: &[&str], out: &mut Vec<String>) {
            match value {
                serde_json::Value::Object(map) => {
                    for (key, nested) in map {
                        if forbidden.contains(&key.as_str()) {
                            out.push(key.clone());
                        }
                        walk_keys(nested, forbidden, out);
                    }
                }
                serde_json::Value::Array(items) => {
                    for item in items {
                        walk_keys(item, forbidden, out);
                    }
                }
                _ => {}
            }
        }
        let list = OperationLogListResponse {
            entries: vec![sample_operation_log_entry()],
            page: 1,
            page_size: 10,
            total_count: 1,
            total_pages: 1,
        };
        let mut hits = Vec::new();
        walk_keys(
            &serde_json::to_value(&list).unwrap(),
            &forbidden_keys,
            &mut hits,
        );
        assert!(hits.is_empty(), "forbidden fields present: {hits:?}");

        let storage_status = OperationLogStorageStatus {
            status: "ready".into(),
            event_count: 3,
            runtime_version: "0.4.0".into(),
        };
        let mut hits = Vec::new();
        walk_keys(
            &serde_json::to_value(&storage_status).unwrap(),
            &forbidden_keys,
            &mut hits,
        );
        assert!(hits.is_empty(), "forbidden fields present: {hits:?}");
    }

    /// A2: an illegal `level` field is rejected by serde rather than
    /// silently defaulting to a known variant.
    #[test]
    fn operation_log_entry_rejects_illegal_level_field() {
        let bad = r#"{"event_id":"x","event_type":"Completed","timestamp":"t","level":"Critical","batch_id":null,"file_id":null,"display_name":null,"input_format":null,"status":"Completed","masked_entity_count":null,"error_code":null,"restored_entity_count":null}"#;
        assert!(serde_json::from_str::<OperationLogEntry>(bad).is_err());
    }

    #[test]
    fn sandbox_status_response_round_trips_and_contains_no_forbidden_fields() {
        fn walk_keys(value: &serde_json::Value, forbidden: &[&str], out: &mut Vec<String>) {
            match value {
                serde_json::Value::Object(map) => {
                    for (key, nested) in map {
                        if forbidden.contains(&key.as_str()) {
                            out.push(key.clone());
                        }
                        walk_keys(nested, forbidden, out);
                    }
                }
                serde_json::Value::Array(items) => {
                    for item in items {
                        walk_keys(item, forbidden, out);
                    }
                }
                _ => {}
            }
        }

        let status = SandboxStatusResponse {
            pin_configured: true,
            locked: true,
            storage_mode: SANDBOX_STORAGE_MODE_SERVER_SYSTEM_USER.to_string(),
            rate_limited: false,
            retry_after_seconds: None,
        };
        let json = serde_json::to_value(&status).unwrap();
        let back: SandboxStatusResponse = serde_json::from_value(json.clone()).unwrap();
        assert_eq!(status, back);

        let forbidden_keys = [
            "pin",
            "hash",
            "phc",
            "salt",
            "path",
            "current_pin",
            "new_pin",
        ];
        let mut hits = Vec::new();
        walk_keys(&json, &forbidden_keys, &mut hits);
        assert!(hits.is_empty(), "forbidden fields present: {hits:?}");
    }

    #[test]
    fn set_sandbox_pin_request_allows_omitted_current_pin_for_first_time_set() {
        let first_time = r#"{"new_pin":"1234"}"#;
        let parsed: SetSandboxPinRequest = serde_json::from_str(first_time).unwrap();
        assert_eq!(parsed.new_pin, "1234");
        assert_eq!(parsed.current_pin, None);

        let replace = r#"{"new_pin":"5678","current_pin":"1234"}"#;
        let parsed: SetSandboxPinRequest = serde_json::from_str(replace).unwrap();
        assert_eq!(parsed.current_pin, Some("1234".to_string()));
    }

    #[test]
    fn unlock_and_clear_sandbox_requests_reject_missing_required_fields() {
        assert!(serde_json::from_str::<UnlockSandboxRequest>("{}").is_err());
        assert!(serde_json::from_str::<ClearSandboxPinRequest>("{}").is_err());
    }

    #[test]
    fn filebay_config_status_round_trips_through_snake_case_strings() {
        for status in [
            FileBayConfigStatus::Unconfigured,
            FileBayConfigStatus::Configured,
            FileBayConfigStatus::Invalid,
        ] {
            assert_eq!(FileBayConfigStatus::parse(status.as_str()), Some(status));
        }
        assert_eq!(FileBayConfigStatus::parse("bogus"), None);
    }

    #[test]
    fn filebay_status_response_never_carries_a_token_field() {
        fn walk_keys(value: &serde_json::Value, forbidden: &[&str], out: &mut Vec<String>) {
            match value {
                serde_json::Value::Object(map) => {
                    for (key, nested) in map {
                        if forbidden.contains(&key.as_str()) {
                            out.push(key.clone());
                        }
                        walk_keys(nested, forbidden, out);
                    }
                }
                serde_json::Value::Array(items) => {
                    for item in items {
                        walk_keys(item, forbidden, out);
                    }
                }
                _ => {}
            }
        }

        let status = FileBayStatusResponse {
            status: FileBayConfigStatus::Configured,
            configured: true,
            has_token: true,
            target_origin: Some("https://filebay.example.com".into()),
            owner: Some("acme".into()),
            repo: Some("vault-artifacts".into()),
        };
        let json = serde_json::to_value(&status).unwrap();
        let forbidden_keys = ["token", "authorization", "path"];
        let mut hits = Vec::new();
        walk_keys(&json, &forbidden_keys, &mut hits);
        assert!(hits.is_empty(), "forbidden fields present: {hits:?}");
        let back: FileBayStatusResponse = serde_json::from_value(json).unwrap();
        assert_eq!(status, back);
    }

    #[test]
    fn filebay_upload_request_only_accepts_artifact_ids() {
        let parsed: FileBayUploadRequest =
            serde_json::from_str(r#"{"artifact_ids":["a1","a2"]}"#).unwrap();
        assert_eq!(
            parsed.artifact_ids,
            vec!["a1".to_string(), "a2".to_string()]
        );
        // Unknown/forbidden client-supplied fields (remote_path, token, url,
        // local file path) are simply ignored by serde's default behaviour —
        // the type itself has no field to receive them.
        let parsed: FileBayUploadRequest = serde_json::from_str(
            r#"{"artifact_ids":["a1"],"remote_path":"masked/x.md","token":"leak","url":"https://evil"}"#,
        )
        .unwrap();
        assert_eq!(parsed.artifact_ids, vec!["a1".to_string()]);
    }

    #[test]
    fn filebay_upload_item_reports_partial_failure_without_faking_success() {
        let item = FileBayUploadItem {
            artifact_id: "a1".into(),
            remote_path: "masked/a1-report.md".into(),
            success: false,
            url: None,
            error_code: Some("FILEBAY_UPLOAD_FAILED".into()),
        };
        assert!(!item.success);
        assert!(item.url.is_none());
    }
}
