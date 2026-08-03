/// Fixed set of safe FileBay error codes. Never carries the underlying
/// transport error, response body, credential, or a full local/server path —
/// only a stable machine-readable code plus an optional caller-supplied
/// safe detail string (already scrubbed by the caller).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FileBayError {
    #[error("FILEBAY_NOT_CONFIGURED")]
    NotConfigured,
    #[error("FILEBAY_CONFIG_INVALID")]
    ConfigInvalid,
    #[error("FILEBAY_AUTH_FAILED")]
    AuthFailed,
    #[error("FILEBAY_CONNECTION_FAILED")]
    ConnectionFailed,
    #[error("FILEBAY_REPOSITORY_NOT_FOUND")]
    RepositoryNotFound,
    #[error("FILEBAY_REPOSITORY_CREATE_FAILED")]
    RepositoryCreateFailed,
    #[error("FILEBAY_UPLOAD_DENIED")]
    UploadDenied,
    #[error("FILEBAY_UPLOAD_FAILED")]
    UploadFailed,
    #[error("FILEBAY_REQUEST_INVALID")]
    RequestInvalid,
    #[error("FILEBAY_STORAGE_FAILED")]
    StorageFailed,
}

impl FileBayError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::NotConfigured => "FILEBAY_NOT_CONFIGURED",
            Self::ConfigInvalid => "FILEBAY_CONFIG_INVALID",
            Self::AuthFailed => "FILEBAY_AUTH_FAILED",
            Self::ConnectionFailed => "FILEBAY_CONNECTION_FAILED",
            Self::RepositoryNotFound => "FILEBAY_REPOSITORY_NOT_FOUND",
            Self::RepositoryCreateFailed => "FILEBAY_REPOSITORY_CREATE_FAILED",
            Self::UploadDenied => "FILEBAY_UPLOAD_DENIED",
            Self::UploadFailed => "FILEBAY_UPLOAD_FAILED",
            Self::RequestInvalid => "FILEBAY_REQUEST_INVALID",
            Self::StorageFailed => "FILEBAY_STORAGE_FAILED",
        }
    }

    /// Maps an HTTP status code from a repository-existence check to a
    /// safe error, or `None` when the status represents "repository not
    /// found" (a valid, non-error outcome for that specific call).
    pub fn from_check_status(status: u16) -> Option<Self> {
        match status {
            200..=299 => None,
            401 | 403 => Some(Self::AuthFailed),
            404 => None,
            _ => Some(Self::ConnectionFailed),
        }
    }

    pub fn from_upload_status(status: u16) -> Self {
        match status {
            401 | 403 => Self::AuthFailed,
            404 => Self::RepositoryNotFound,
            _ => Self::UploadFailed,
        }
    }

    pub fn from_create_status(status: u16) -> Self {
        match status {
            401 | 403 => Self::AuthFailed,
            _ => Self::RepositoryCreateFailed,
        }
    }
}
