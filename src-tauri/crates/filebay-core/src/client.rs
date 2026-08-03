use crate::endpoint::Endpoint;
use crate::error::FileBayError;
use crate::path::validate_remote_path;
use crate::token::Token;
use crate::transport::Transport;
use base64::{engine::general_purpose, Engine as _};

/// A validated repository target: an [`Endpoint`] plus an owner/repo pair.
/// Callers are expected to have already run both through
/// [`crate::identity::validate_identity`].
pub struct RepositoryTarget<'a> {
    pub endpoint: &'a Endpoint,
    pub owner: &'a str,
    pub repo: &'a str,
}

#[derive(Debug)]
pub struct UploadOutcome {
    pub remote_path: String,
    pub url: Option<String>,
}

/// The single production FileBay HTTP client. Generic over [`Transport`]
/// so hosts (desktop Tauri, Runtime) inject a real [`crate::transport::ReqwestTransport`]
/// and tests inject [`crate::testing::FakeTransport`] — there is exactly one
/// implementation of the upload/check/create wire protocol in the whole
/// product.
pub struct FileBayClient<T: Transport> {
    transport: T,
}

impl<T: Transport> FileBayClient<T> {
    pub fn new(transport: T) -> Self {
        Self { transport }
    }

    pub async fn check_repository_exists(
        &self,
        target: &RepositoryTarget<'_>,
        token: &Token,
    ) -> Result<bool, FileBayError> {
        let url = format!(
            "{}/api/v1/repos/{}/{}",
            target.endpoint.as_str(),
            target.owner,
            target.repo
        );
        let response = self.transport.get(&url, Some(token)).await?;
        match response.status {
            200..=299 => Ok(true),
            404 => Ok(false),
            401 | 403 => Err(FileBayError::AuthFailed),
            _ => Err(FileBayError::ConnectionFailed),
        }
    }

    /// Unconditionally attempts to create the repository (no existence
    /// pre-check). `private` is a host-controlled flag — the Runtime
    /// browser adapter must always pass `true` and never let the browser
    /// choose; only the desktop host (which already exposed this choice
    /// pre-adapter) may pass a caller-selected value.
    pub async fn create_repository(
        &self,
        target: &RepositoryTarget<'_>,
        private: bool,
        token: &Token,
    ) -> Result<(), FileBayError> {
        let url = format!("{}/api/v1/user/repos", target.endpoint.as_str());
        let body = serde_json::json!({
            "name": target.repo,
            "private": private,
            "auto_init": true,
            "description": "CheersAI Vault - 脱敏文件存储仓库",
        });
        let response = self.transport.post_json(&url, Some(token), body).await?;
        if (200..=299).contains(&response.status) {
            Ok(())
        } else {
            Err(FileBayError::from_create_status(response.status))
        }
    }

    /// Idempotent: if the repository already exists this is a no-op
    /// success; otherwise creates it via [`Self::create_repository`].
    pub async fn ensure_repository(
        &self,
        target: &RepositoryTarget<'_>,
        private: bool,
        token: &Token,
    ) -> Result<(), FileBayError> {
        if self.check_repository_exists(target, token).await? {
            return Ok(());
        }
        self.create_repository(target, private, token).await
    }

    async fn get_file_sha(
        &self,
        target: &RepositoryTarget<'_>,
        remote_path: &str,
        token: &Token,
    ) -> Result<Option<String>, FileBayError> {
        let url = format!(
            "{}/api/v1/repos/{}/{}/contents/{}",
            target.endpoint.as_str(),
            target.owner,
            target.repo,
            remote_path
        );
        let response = self.transport.get(&url, Some(token)).await?;
        match response.status {
            200..=299 => Ok(response
                .json
                .as_ref()
                .and_then(|json| json.get("sha"))
                .and_then(|sha| sha.as_str())
                .map(|sha| sha.to_string())),
            404 => Ok(None),
            401 | 403 => Err(FileBayError::AuthFailed),
            _ => Err(FileBayError::ConnectionFailed),
        }
    }

    /// Uploads (creating or updating) in-memory bytes at `remote_path`.
    /// Never accepts a local filesystem path — the host is responsible for
    /// resolving and reading a whitelisted artifact into `bytes` before
    /// calling this.
    pub async fn upload_bytes(
        &self,
        target: &RepositoryTarget<'_>,
        remote_path: &str,
        bytes: &[u8],
        message: &str,
        token: &Token,
    ) -> Result<UploadOutcome, FileBayError> {
        validate_remote_path(remote_path)?;
        let existing_sha = self.get_file_sha(target, remote_path, token).await?;
        let content_b64 = general_purpose::STANDARD.encode(bytes);
        let mut body = serde_json::json!({
            "content": content_b64,
            "message": message,
        });
        if let Some(sha) = &existing_sha {
            body["sha"] = serde_json::json!(sha);
        }
        let url = format!(
            "{}/api/v1/repos/{}/{}/contents/{}",
            target.endpoint.as_str(),
            target.owner,
            target.repo,
            remote_path
        );
        let response = self.transport.post_json(&url, Some(token), body).await?;
        if (200..=299).contains(&response.status) {
            let html_url = response
                .json
                .as_ref()
                .and_then(|json| json.get("content"))
                .and_then(|content| content.get("html_url"))
                .and_then(|value| value.as_str())
                .map(|value| value.to_string());
            Ok(UploadOutcome {
                remote_path: remote_path.to_string(),
                url: html_url,
            })
        } else {
            Err(FileBayError::from_upload_status(response.status))
        }
    }

    /// Deletes an existing remote file. Not part of the browser-facing
    /// adapter contract (deletion is out of scope for that surface) — this
    /// exists purely so the desktop host's pre-existing delete command can
    /// keep using the single shared HTTP client instead of a second one.
    pub async fn delete_file(
        &self,
        target: &RepositoryTarget<'_>,
        remote_path: &str,
        message: &str,
        token: &Token,
    ) -> Result<(), FileBayError> {
        validate_remote_path(remote_path)?;
        let sha = self
            .get_file_sha(target, remote_path, token)
            .await?
            .ok_or(FileBayError::RepositoryNotFound)?;
        let body = serde_json::json!({ "sha": sha, "message": message });
        let url = format!(
            "{}/api/v1/repos/{}/{}/contents/{}",
            target.endpoint.as_str(),
            target.owner,
            target.repo,
            remote_path
        );
        let response = self.transport.delete_json(&url, Some(token), body).await?;
        if (200..=299).contains(&response.status) {
            Ok(())
        } else {
            match response.status {
                401 | 403 => Err(FileBayError::AuthFailed),
                404 => Err(FileBayError::RepositoryNotFound),
                _ => Err(FileBayError::UploadFailed),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::FakeTransport;

    fn target<'a>(endpoint: &'a Endpoint) -> RepositoryTarget<'a> {
        RepositoryTarget {
            endpoint,
            owner: "acme",
            repo: "vault-artifacts",
        }
    }

    #[tokio::test]
    async fn check_repository_exists_maps_status_codes() {
        let endpoint = Endpoint::parse("https://filebay.example.com").unwrap();
        let token = Token::new("fake-token");

        let transport = FakeTransport::new();
        transport.stub("/api/v1/repos/", 200, None);
        let client = FileBayClient::new(transport);
        assert!(client
            .check_repository_exists(&target(&endpoint), &token)
            .await
            .unwrap());

        let transport = FakeTransport::new();
        transport.stub("/api/v1/repos/", 404, None);
        let client = FileBayClient::new(transport);
        assert!(!client
            .check_repository_exists(&target(&endpoint), &token)
            .await
            .unwrap());

        let transport = FakeTransport::new();
        transport.stub("/api/v1/repos/", 401, None);
        let client = FileBayClient::new(transport);
        assert_eq!(
            client
                .check_repository_exists(&target(&endpoint), &token)
                .await
                .unwrap_err(),
            FileBayError::AuthFailed
        );

        let transport = FakeTransport::new();
        transport.stub("/api/v1/repos/", 500, None);
        let client = FileBayClient::new(transport);
        assert_eq!(
            client
                .check_repository_exists(&target(&endpoint), &token)
                .await
                .unwrap_err(),
            FileBayError::ConnectionFailed
        );
    }

    #[tokio::test]
    async fn ensure_repository_is_idempotent_when_already_present() {
        let endpoint = Endpoint::parse("https://filebay.example.com").unwrap();
        let token = Token::new("fake-token");
        let transport = FakeTransport::new();
        transport.stub("/api/v1/repos/", 200, None);
        let client = FileBayClient::new(transport);
        client
            .ensure_repository(&target(&endpoint), true, &token)
            .await
            .unwrap();
        // Only the existence check should have fired; no create call.
        assert_eq!(client.transport.call_count(), 1);
    }

    #[tokio::test]
    async fn ensure_repository_creates_private_when_missing() {
        let endpoint = Endpoint::parse("https://filebay.example.com").unwrap();
        let token = Token::new("fake-token");
        let transport = FakeTransport::new();
        transport.stub("/api/v1/repos/", 404, None);
        transport.stub("/api/v1/user/repos", 201, None);
        let client = FileBayClient::new(transport);
        client
            .ensure_repository(&target(&endpoint), true, &token)
            .await
            .unwrap();
        assert_eq!(client.transport.call_count(), 2);
    }

    #[tokio::test]
    async fn upload_bytes_rejects_remote_path_before_any_transport_call() {
        let endpoint = Endpoint::parse("https://filebay.example.com").unwrap();
        let token = Token::new("fake-token");
        let transport = FakeTransport::new();
        let client = FileBayClient::new(transport);
        let result = client
            .upload_bytes(
                &target(&endpoint),
                "raw/escape.md",
                b"content",
                "msg",
                &token,
            )
            .await;
        assert_eq!(result.unwrap_err(), FileBayError::UploadDenied);
        assert_eq!(client.transport.call_count(), 0);
    }

    #[tokio::test]
    async fn delete_file_requires_an_existing_remote_file() {
        let endpoint = Endpoint::parse("https://filebay.example.com").unwrap();
        let token = Token::new("fake-token");
        let transport = FakeTransport::new();
        transport.stub("/contents/", 404, None);
        let client = FileBayClient::new(transport);
        assert_eq!(
            client
                .delete_file(&target(&endpoint), "masked/gone.md", "msg", &token)
                .await
                .unwrap_err(),
            FileBayError::RepositoryNotFound
        );
        assert_eq!(client.transport.call_count(), 1);
    }

    #[tokio::test]
    async fn delete_file_succeeds_after_finding_sha() {
        let endpoint = Endpoint::parse("https://filebay.example.com").unwrap();
        let token = Token::new("fake-token");
        let transport = FakeTransport::new();
        transport.stub(
            "/contents/",
            200,
            Some(serde_json::json!({"sha": "abc123"})),
        );
        transport.stub("/contents/", 200, None);
        let client = FileBayClient::new(transport);
        client
            .delete_file(&target(&endpoint), "masked/report.md", "msg", &token)
            .await
            .unwrap();
        assert_eq!(client.transport.call_count(), 2);
    }

    #[tokio::test]
    async fn upload_bytes_succeeds_and_returns_url() {
        let endpoint = Endpoint::parse("https://filebay.example.com").unwrap();
        let token = Token::new("fake-token");
        let transport = FakeTransport::new();
        transport.stub("/contents/", 404, None); // no existing file
        transport.stub(
            "/contents/",
            201,
            Some(serde_json::json!({"content": {"html_url": "https://filebay.example.com/acme/vault-artifacts/src/branch/main/masked/report.md"}})),
        );
        let client = FileBayClient::new(transport);
        let outcome = client
            .upload_bytes(
                &target(&endpoint),
                "masked/report.md",
                b"hello",
                "msg",
                &token,
            )
            .await
            .unwrap();
        assert_eq!(outcome.remote_path, "masked/report.md");
        assert!(outcome.url.unwrap().contains("masked/report.md"));
    }

    #[tokio::test]
    async fn upload_bytes_maps_auth_and_generic_failures() {
        let endpoint = Endpoint::parse("https://filebay.example.com").unwrap();
        let token = Token::new("fake-token");

        let transport = FakeTransport::new();
        transport.stub("/contents/", 404, None);
        transport.stub("/contents/", 401, None);
        let client = FileBayClient::new(transport);
        assert_eq!(
            client
                .upload_bytes(&target(&endpoint), "masked/a.md", b"x", "m", &token)
                .await
                .unwrap_err(),
            FileBayError::AuthFailed
        );

        let transport = FakeTransport::new();
        transport.stub("/contents/", 404, None);
        transport.stub("/contents/", 500, None);
        let client = FileBayClient::new(transport);
        assert_eq!(
            client
                .upload_bytes(&target(&endpoint), "masked/a.md", b"x", "m", &token)
                .await
                .unwrap_err(),
            FileBayError::UploadFailed
        );
    }
}
