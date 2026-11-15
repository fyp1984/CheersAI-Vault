//! Runtime HTTP adapter for the browser FileBay uploader (`/api/v1/filebay*`).
//!
//! [`FileBaySession`] is the only place Runtime-specific policy (env-var
//! parsing, "browser may never choose `private`", the fixed remote-path
//! construction) wraps around the shared `filebay_core::FileBayClient` — the
//! HTTP wire protocol itself lives entirely in `filebay-core`, not here.
//! This module owns only: reading the four admin environment variables once
//! at startup, HTTP routing/request shaping, and orchestrating the store's
//! own controlled-read/candidate/log methods so a browser request can never
//! reach FileBay with an unverified artifact.
//!
//! Only three actions are ever allowed to reach FileBay: the explicit
//! connectivity test, private-repository creation, and confirmed upload.
//! `status` and `candidates` never touch the network.

use filebay_core::{
    build_remote_path, sanitize_stem, validate_identity, Endpoint, FileBayClient, FileBayError,
    RepositoryTarget, Token,
};
use service_contracts::{
    FileBayCandidate, FileBayCandidatesResponse, FileBayConfigStatus, FileBayRepositoryResponse,
    FileBayRepositoryStatus, FileBayStatusResponse, FileBayTestResponse, FileBayUploadItem,
    FileBayUploadRequest, FileBayUploadResponse,
};
use std::convert::Infallible;
use warp::{http::StatusCode, Filter, Rejection, Reply};

use crate::Runtime;

/// Request bodies here are tiny (at most 100 short artifact-id strings) —
/// capped well below any legitimate payload so an oversized body is
/// rejected before JSON parsing (安全约束/D §6).
const FILEBAY_BODY_LIMIT_BYTES: u64 = 16 * 1024;
const FILEBAY_MAX_UPLOAD_IDS: usize = 100;

#[cfg(not(test))]
type SessionTransport = filebay_core::ReqwestTransport;
#[cfg(test)]
type SessionTransport = std::sync::Arc<filebay_core::testing::FakeTransport>;

#[cfg(not(test))]
fn default_session_transport() -> SessionTransport {
    filebay_core::ReqwestTransport::new()
}
#[cfg(test)]
fn default_session_transport() -> SessionTransport {
    std::sync::Arc::new(filebay_core::testing::FakeTransport::new())
}

/// Runtime-side wrapping around the shared [`FileBayClient`]: owns the
/// admin-environment-derived configuration (read exactly once, at startup)
/// and the single client instance the browser adapter uses. There is
/// exactly one of these per Runtime process.
pub(crate) struct FileBaySession {
    status: FileBayConfigStatus,
    endpoint: Option<Endpoint>,
    owner: Option<String>,
    repo: Option<String>,
    token: Option<Token>,
    /// 生产 FileBay HTTP client 只在完整有效配置（`Configured`）时持有；
    /// 未配置/配置无效会话不创建 `ReqwestTransport`，避免无关的 macOS
    /// 系统代理发现 panic。测试 fake 会话同样只在 `Configured` 时持有。
    client: Option<FileBayClient<SessionTransport>>,
}

impl FileBaySession {
    /// Reads `VAULT_FILEBAY_URL`/`_TOKEN`/`_OWNER`/`_REPO` exactly once.
    /// Never re-read after this call — there is no API or browser action
    /// that can change Runtime's FileBay configuration (B §4).
    pub(crate) fn from_env() -> Self {
        let url = non_empty_env("VAULT_FILEBAY_URL");
        let token_raw = non_empty_env("VAULT_FILEBAY_TOKEN");
        let owner_raw = non_empty_env("VAULT_FILEBAY_OWNER");
        let repo_raw = non_empty_env("VAULT_FILEBAY_REPO");

        let present = [
            url.is_some(),
            token_raw.is_some(),
            owner_raw.is_some(),
            repo_raw.is_some(),
        ];
        let present_count = present.iter().filter(|value| **value).count();

        if present_count == 0 {
            return Self::unconfigured();
        }
        if present_count < 4 {
            return Self::invalid();
        }

        // All four are `Some` at this point (present_count == 4).
        let (Some(url), Some(token_raw), Some(owner_raw), Some(repo_raw)) =
            (url, token_raw, owner_raw, repo_raw)
        else {
            return Self::invalid();
        };

        let endpoint = Endpoint::parse(&url).ok();
        let owner_ok = validate_identity(&owner_raw).is_ok();
        let repo_ok = validate_identity(&repo_raw).is_ok();

        match endpoint {
            Some(endpoint) if owner_ok && repo_ok => Self {
                status: FileBayConfigStatus::Configured,
                endpoint: Some(endpoint),
                owner: Some(owner_raw),
                repo: Some(repo_raw),
                token: Some(Token::new(token_raw)),
                client: Some(FileBayClient::new(default_session_transport())),
            },
            _ => Self::invalid(),
        }
    }

    fn unconfigured() -> Self {
        Self {
            status: FileBayConfigStatus::Unconfigured,
            endpoint: None,
            owner: None,
            repo: None,
            token: None,
            client: None,
        }
    }

    fn invalid() -> Self {
        Self {
            status: FileBayConfigStatus::Invalid,
            endpoint: None,
            owner: None,
            repo: None,
            token: None,
            client: None,
        }
    }

    #[cfg(test)]
    pub(crate) fn new_for_test(
        status: FileBayConfigStatus,
        endpoint: Option<Endpoint>,
        owner: Option<String>,
        repo: Option<String>,
        token: Option<Token>,
        transport: std::sync::Arc<filebay_core::testing::FakeTransport>,
    ) -> Self {
        Self {
            status,
            endpoint,
            owner,
            repo,
            token,
            // 测试 fake 会话同样只在 `Configured` 时持有 client，与生产
            // `unconfigured()`/`invalid()` 语义一致。
            client: (status == FileBayConfigStatus::Configured)
                .then(|| FileBayClient::new(transport)),
        }
    }

    fn target(&self) -> Result<RepositoryTarget<'_>, FileBayOpError> {
        match (&self.endpoint, &self.owner, &self.repo) {
            (Some(endpoint), Some(owner), Some(repo)) => Ok(RepositoryTarget {
                endpoint,
                owner,
                repo,
            }),
            _ => Err(self.not_ready_error()),
        }
    }

    fn token(&self) -> Result<&Token, FileBayOpError> {
        self.token.as_ref().ok_or_else(|| self.not_ready_error())
    }

    /// 取得已构造的生产 client；未配置/无效会话不持有 client，返回既有
    /// not-ready 错误，绝不为占位 client 触发网络/系统代理访问。
    fn client(&self) -> Result<&FileBayClient<SessionTransport>, FileBayOpError> {
        self.client.as_ref().ok_or_else(|| self.not_ready_error())
    }

    fn not_ready_error(&self) -> FileBayOpError {
        match self.status {
            FileBayConfigStatus::Unconfigured => FileBayOpError::NotConfigured,
            _ => FileBayOpError::ConfigInvalid,
        }
    }

    fn status_response(&self) -> FileBayStatusResponse {
        FileBayStatusResponse {
            status: self.status,
            configured: self.status == FileBayConfigStatus::Configured,
            has_token: self.token.is_some(),
            target_origin: self.endpoint.as_ref().map(|e| e.as_str().to_string()),
            owner: self.owner.clone(),
            repo: self.repo.clone(),
        }
    }

    async fn test_connection(&self) -> Result<FileBayTestResponse, FileBayOpError> {
        let target = self.target()?;
        let token = self.token()?;
        let client = self.client()?;
        let exists = client
            .check_repository_exists(&target, token)
            .await
            .map_err(FileBayOpError::from)?;
        Ok(FileBayTestResponse {
            repository_exists: exists,
        })
    }

    /// Always a **private** repository — the browser can never choose
    /// otherwise (A §3/§6, unlike the desktop command which still exposes
    /// that legacy choice to its own caller).
    async fn ensure_private_repository(&self) -> Result<FileBayRepositoryResponse, FileBayOpError> {
        let target = self.target()?;
        let token = self.token()?;
        let client = self.client()?;
        if client
            .check_repository_exists(&target, token)
            .await
            .map_err(FileBayOpError::from)?
        {
            return Ok(FileBayRepositoryResponse {
                status: FileBayRepositoryStatus::Ready,
            });
        }
        client
            .create_repository(&target, true, token)
            .await
            .map_err(FileBayOpError::from)?;
        Ok(FileBayRepositoryResponse {
            status: FileBayRepositoryStatus::Created,
        })
    }

    /// Uploads one already-store-verified artifact. Never called for an
    /// artifact that failed [`crate::store::Store::filebay_verified_artifact`]
    /// — that rejection happens before this, and before any transport call.
    async fn upload_one(
        &self,
        remote_path: &str,
        bytes: &[u8],
    ) -> Result<Option<String>, FileBayOpError> {
        let target = self.target()?;
        let token = self.token()?;
        let client = self.client()?;
        let outcome = client
            .upload_bytes(
                &target,
                remote_path,
                bytes,
                "CheersAI Vault deidentified artifact upload",
                token,
            )
            .await
            .map_err(FileBayOpError::from)?;
        Ok(outcome.url)
    }

    fn target_domain(&self) -> String {
        self.endpoint
            .as_ref()
            .map(|endpoint| endpoint.as_str().to_string())
            .unwrap_or_default()
    }
}

enum FileBayOpError {
    NotConfigured,
    ConfigInvalid,
    AuthFailed,
    ConnectionFailed,
    RepositoryNotFound,
    RepositoryCreateFailed,
    UploadFailed,
    RequestInvalid,
}

impl From<FileBayError> for FileBayOpError {
    fn from(error: FileBayError) -> Self {
        match error {
            FileBayError::NotConfigured => Self::NotConfigured,
            FileBayError::ConfigInvalid => Self::ConfigInvalid,
            FileBayError::AuthFailed => Self::AuthFailed,
            FileBayError::ConnectionFailed => Self::ConnectionFailed,
            FileBayError::RepositoryNotFound => Self::RepositoryNotFound,
            FileBayError::RepositoryCreateFailed => Self::RepositoryCreateFailed,
            FileBayError::UploadDenied => Self::RequestInvalid,
            FileBayError::UploadFailed => Self::UploadFailed,
            FileBayError::RequestInvalid => Self::RequestInvalid,
            FileBayError::StorageFailed => Self::UploadFailed,
        }
    }
}

impl FileBayOpError {
    fn code(&self) -> &'static str {
        match self {
            Self::NotConfigured => "FILEBAY_NOT_CONFIGURED",
            Self::ConfigInvalid => "FILEBAY_CONFIG_INVALID",
            Self::AuthFailed => "FILEBAY_AUTH_FAILED",
            Self::ConnectionFailed => "FILEBAY_CONNECTION_FAILED",
            Self::RepositoryNotFound => "FILEBAY_REPOSITORY_NOT_FOUND",
            Self::RepositoryCreateFailed => "FILEBAY_REPOSITORY_CREATE_FAILED",
            Self::UploadFailed => "FILEBAY_UPLOAD_FAILED",
            Self::RequestInvalid => "FILEBAY_REQUEST_INVALID",
        }
    }

    fn status(&self) -> StatusCode {
        match self {
            Self::NotConfigured => StatusCode::CONFLICT,
            Self::ConfigInvalid => StatusCode::CONFLICT,
            Self::AuthFailed => StatusCode::UNAUTHORIZED,
            Self::ConnectionFailed => StatusCode::BAD_GATEWAY,
            Self::RepositoryNotFound => StatusCode::NOT_FOUND,
            Self::RepositoryCreateFailed => StatusCode::BAD_GATEWAY,
            Self::UploadFailed => StatusCode::BAD_GATEWAY,
            Self::RequestInvalid => StatusCode::BAD_REQUEST,
        }
    }

    fn retryable(&self) -> bool {
        matches!(
            self,
            Self::ConnectionFailed | Self::RepositoryCreateFailed | Self::UploadFailed
        )
    }
}

fn filebay_rejection(error: FileBayOpError) -> Rejection {
    crate::api_error(
        error.status(),
        error.code(),
        error.code(),
        error.retryable(),
    )
}

fn non_empty_env(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .filter(|value| !value.trim().is_empty())
}

pub fn routes<F>(
    runtime_filter: F,
) -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone
where
    F: Filter<Extract = (Runtime,), Error = Infallible> + Clone + Send + Sync + 'static,
{
    let status = warp::path!("api" / "v1" / "filebay" / "status")
        .and(warp::get())
        .and(runtime_filter.clone())
        .and_then(status_handler);

    let test = warp::path!("api" / "v1" / "filebay" / "test")
        .and(warp::post())
        .and(warp::body::content_length_limit(FILEBAY_BODY_LIMIT_BYTES))
        .and(runtime_filter.clone())
        .and_then(test_handler);

    let repository = warp::path!("api" / "v1" / "filebay" / "repository")
        .and(warp::post())
        .and(warp::body::content_length_limit(FILEBAY_BODY_LIMIT_BYTES))
        .and(runtime_filter.clone())
        .and_then(repository_handler);

    let candidates = warp::path!("api" / "v1" / "filebay" / "batches" / String / "candidates")
        .and(warp::get())
        .and(runtime_filter.clone())
        .and_then(candidates_handler);

    let uploads = warp::path!("api" / "v1" / "filebay" / "uploads")
        .and(warp::post())
        .and(warp::body::content_length_limit(FILEBAY_BODY_LIMIT_BYTES))
        .and(warp::body::json())
        .and(runtime_filter)
        .and_then(uploads_handler);

    status.or(test).or(repository).or(candidates).or(uploads)
}

async fn status_handler(runtime: Runtime) -> Result<impl Reply, Rejection> {
    Ok(warp::reply::json(&runtime.filebay.status_response()))
}

async fn test_handler(runtime: Runtime) -> Result<impl Reply, Rejection> {
    let response = runtime
        .filebay
        .test_connection()
        .await
        .map_err(filebay_rejection)?;
    Ok(warp::reply::json(&response))
}

async fn repository_handler(runtime: Runtime) -> Result<impl Reply, Rejection> {
    let response = runtime
        .filebay
        .ensure_private_repository()
        .await
        .map_err(filebay_rejection)?;
    Ok(warp::reply::json(&response))
}

async fn candidates_handler(batch_id: String, runtime: Runtime) -> Result<impl Reply, Rejection> {
    let rows = runtime
        .store
        .filebay_candidates(&batch_id)
        .await
        .map_err(|_| {
            crate::api_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "FILEBAY_STORAGE_FAILED",
                "FILEBAY_STORAGE_FAILED",
                true,
            )
        })?;
    let candidates = rows
        .into_iter()
        .map(|(artifact_id, display_name)| {
            let remote_path = build_remote_path(&artifact_id, &sanitize_stem(&display_name));
            FileBayCandidate {
                artifact_id,
                display_name,
                remote_path,
            }
        })
        .collect();
    Ok(warp::reply::json(&FileBayCandidatesResponse { candidates }))
}

/// Body-shape and count validation. Runs entirely before any store lookup
/// or transport call (D §6).
fn validate_upload_request(request: &FileBayUploadRequest) -> Result<(), Rejection> {
    if request.artifact_ids.is_empty() || request.artifact_ids.len() > FILEBAY_MAX_UPLOAD_IDS {
        return Err(crate::api_error(
            StatusCode::BAD_REQUEST,
            "FILEBAY_REQUEST_INVALID",
            "FILEBAY_REQUEST_INVALID",
            false,
        ));
    }
    let mut seen = std::collections::HashSet::with_capacity(request.artifact_ids.len());
    for id in &request.artifact_ids {
        if id.is_empty() || id.len() > 128 || !seen.insert(id.as_str()) {
            return Err(crate::api_error(
                StatusCode::BAD_REQUEST,
                "FILEBAY_REQUEST_INVALID",
                "FILEBAY_REQUEST_INVALID",
                false,
            ));
        }
    }
    Ok(())
}

async fn uploads_handler(
    request: FileBayUploadRequest,
    runtime: Runtime,
) -> Result<impl Reply, Rejection> {
    validate_upload_request(&request)?;
    if runtime.filebay.status != FileBayConfigStatus::Configured {
        return Err(filebay_rejection(runtime.filebay.not_ready_error()));
    }

    let mut items = Vec::with_capacity(request.artifact_ids.len());
    let target_domain = runtime.filebay.target_domain();
    let owner = runtime.filebay.owner.clone().unwrap_or_default();
    let repo = runtime.filebay.repo.clone().unwrap_or_default();

    // Sequential, not concurrent (C §5 "顺序受控上传") — one artifact at a
    // time, so a slow or failing upload can never race another's log event.
    for artifact_id in &request.artifact_ids {
        let verified = runtime.store.filebay_verified_artifact(artifact_id).await;
        let (batch_id, display_name, bytes) = match verified {
            Ok(value) => value,
            Err(_) => {
                // Rejected by our own whitelist — no transport call, no log
                // event (nothing was actually attempted against FileBay).
                items.push(FileBayUploadItem {
                    artifact_id: artifact_id.clone(),
                    remote_path: String::new(),
                    success: false,
                    url: None,
                    error_code: Some("FILEBAY_UPLOAD_DENIED".to_string()),
                });
                continue;
            }
        };
        let remote_path = build_remote_path(artifact_id, &sanitize_stem(&display_name));
        match runtime.filebay.upload_one(&remote_path, &bytes).await {
            Ok(url) => {
                let _ = runtime
                    .store
                    .log_filebay_upload_event(
                        Some(&batch_id),
                        artifact_id,
                        &display_name,
                        &target_domain,
                        &owner,
                        &repo,
                        &remote_path,
                        true,
                        None,
                    )
                    .await;
                items.push(FileBayUploadItem {
                    artifact_id: artifact_id.clone(),
                    remote_path,
                    success: true,
                    url,
                    error_code: None,
                });
            }
            Err(error) => {
                let code = error.code();
                let _ = runtime
                    .store
                    .log_filebay_upload_event(
                        Some(&batch_id),
                        artifact_id,
                        &display_name,
                        &target_domain,
                        &owner,
                        &repo,
                        &remote_path,
                        false,
                        Some(code),
                    )
                    .await;
                items.push(FileBayUploadItem {
                    artifact_id: artifact_id.clone(),
                    remote_path,
                    success: false,
                    url: None,
                    error_code: Some(code.to_string()),
                });
            }
        }
    }

    Ok(warp::reply::json(&FileBayUploadResponse { items }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use filebay_core::{testing::FakeTransport, Endpoint, Token};
    use std::sync::Arc;

    fn fake_target() -> (Endpoint, String, String, Token) {
        (
            Endpoint::parse("https://filebay.example.com").unwrap(),
            "test-owner".to_string(),
            "test-repo".to_string(),
            Token::new("fake-token-for-tests-never-a-real-credential"),
        )
    }

    fn fake_transport() -> Arc<FakeTransport> {
        Arc::new(FakeTransport::new())
    }

    /// 未配置/无效会话不得持有生产 client（本任务核心语义：不得为了占位
    /// client 触发 reqwest/system-configuration 系统代理发现）。
    #[test]
    fn unconfigured_and_invalid_sessions_hold_no_client() {
        let unconfigured = FileBaySession::new_for_test(
            FileBayConfigStatus::Unconfigured,
            None,
            None,
            None,
            None,
            fake_transport(),
        );
        assert!(
            unconfigured.client.is_none(),
            "Unconfigured 会话不得创建 FileBay HTTP client"
        );

        let invalid = FileBaySession::new_for_test(
            FileBayConfigStatus::Invalid,
            None,
            None,
            None,
            None,
            fake_transport(),
        );
        assert!(
            invalid.client.is_none(),
            "Invalid 会话不得创建 FileBay HTTP client"
        );
    }

    /// 完整有效配置的 fake 会话持有 client，且能正常通过 fake transport
    /// 完成连接测试（fake transport 不回退）。
    #[test]
    fn configured_fake_session_holds_a_client() {
        let (endpoint, owner, repo, token) = fake_target();
        let configured = FileBaySession::new_for_test(
            FileBayConfigStatus::Configured,
            Some(endpoint),
            Some(owner),
            Some(repo),
            Some(token),
            fake_transport(),
        );
        assert!(
            configured.client.is_some(),
            "Configured 会话必须持有 FileBay HTTP client"
        );
    }

    /// 未配置状态下的 test/repository/upload 网络动作返回既有
    /// `FILEBAY_NOT_CONFIGURED`（HTTP 409、retryable=false），不 panic、
    /// 不伪成功、不访问 client。
    #[tokio::test]
    async fn unconfigured_network_actions_return_not_configured_without_client() {
        let unconfigured = FileBaySession::new_for_test(
            FileBayConfigStatus::Unconfigured,
            None,
            None,
            None,
            None,
            fake_transport(),
        );

        let err = unconfigured.test_connection().await.unwrap_err();
        assert!(matches!(err, FileBayOpError::NotConfigured));
        assert_eq!(err.code(), "FILEBAY_NOT_CONFIGURED");
        assert_eq!(err.status(), StatusCode::CONFLICT);
        assert!(!err.retryable());

        let err = unconfigured.ensure_private_repository().await.unwrap_err();
        assert!(matches!(err, FileBayOpError::NotConfigured));
        assert_eq!(err.code(), "FILEBAY_NOT_CONFIGURED");

        let err = unconfigured
            .upload_one("test-owner/test-repo/remote.md", b"x")
            .await
            .unwrap_err();
        assert!(matches!(err, FileBayOpError::NotConfigured));
        assert_eq!(err.code(), "FILEBAY_NOT_CONFIGURED");
    }

    /// 无效配置状态下的网络动作返回既有 `FILEBAY_CONFIG_INVALID`
    /// （HTTP 409、retryable=false），同样不 panic、不伪成功、不访问 client。
    #[tokio::test]
    async fn invalid_network_actions_return_config_invalid_without_client() {
        let invalid = FileBaySession::new_for_test(
            FileBayConfigStatus::Invalid,
            None,
            None,
            None,
            None,
            fake_transport(),
        );

        let err = invalid.test_connection().await.unwrap_err();
        assert!(matches!(err, FileBayOpError::ConfigInvalid));
        assert_eq!(err.code(), "FILEBAY_CONFIG_INVALID");
        assert_eq!(err.status(), StatusCode::CONFLICT);
        assert!(!err.retryable());

        let err = invalid.ensure_private_repository().await.unwrap_err();
        assert!(matches!(err, FileBayOpError::ConfigInvalid));

        let err = invalid
            .upload_one("test-owner/test-repo/remote.md", b"x")
            .await
            .unwrap_err();
        assert!(matches!(err, FileBayOpError::ConfigInvalid));
    }

    /// Configured fake 会话通过 fake transport 完成连接测试并记录一次调用，
    /// 证明生产 client 语义与 fake transport 复用不回退。
    #[tokio::test]
    async fn configured_fake_session_uses_fake_transport_for_test_connection() {
        let (endpoint, owner, repo, token) = fake_target();
        let transport = fake_transport();
        let session = FileBaySession::new_for_test(
            FileBayConfigStatus::Configured,
            Some(endpoint),
            Some(owner),
            Some(repo),
            Some(token),
            transport.clone(),
        );
        // 默认 200 → check_repository_exists 判定仓库存在。
        let response = match session.test_connection().await {
            Ok(response) => response,
            Err(error) => panic!("配置会话不应失败，错误码 {}", error.code()),
        };
        assert!(response.repository_exists);
        assert_eq!(transport.call_count(), 1);
    }
}
