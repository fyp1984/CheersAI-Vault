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
        [Self::Pending, Self::Processing, Self::Completed, Self::Failed]
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BatchFile {
    pub file_id: String,
    pub display_name: String,
    pub input_format: String,
    pub status: FileStatus,
    pub attempt: usize,
    pub masked_entity_count: Option<usize>,
    pub artifact_id: Option<String>,
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
}
