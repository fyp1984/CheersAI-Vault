//! Thin desktop wrapper around the shared `filebay-core` HTTP client.
//!
//! This module intentionally contains **no** independent reqwest usage —
//! every wire request goes through `filebay_core::FileBayClient`, the same
//! client the Runtime browser adapter uses. Only Tauri-specific plumbing
//! (config shape expected by `commands/gitea.rs`, `anyhow::Error` mapping)
//! lives here.

use anyhow::{anyhow, Result};
use filebay_core::{Endpoint, FileBayClient, RepositoryTarget, ReqwestTransport, Token};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GiteaConfig {
    pub url: String,
    pub token: String,
    pub owner: String,
    pub repo: String,
}

fn to_anyhow(error: filebay_core::FileBayError) -> anyhow::Error {
    anyhow!(error.code().to_string())
}

pub struct GiteaClient {
    config: GiteaConfig,
    inner: FileBayClient<ReqwestTransport>,
}

impl GiteaClient {
    pub fn new(config: GiteaConfig) -> Self {
        Self {
            config,
            inner: FileBayClient::new(ReqwestTransport::new()),
        }
    }

    fn endpoint(&self) -> Result<Endpoint> {
        // The desktop's pre-existing config flow already only ever calls
        // this after `validate_https` accepted the URL (see
        // `commands/gitea.rs`); `Endpoint::parse` re-validates with the
        // shared crate's stricter root-origin rule, which every real
        // desktop config in use satisfies.
        Endpoint::parse(&self.config.url).map_err(to_anyhow)
    }

    fn target<'a>(&'a self, endpoint: &'a Endpoint) -> RepositoryTarget<'a> {
        RepositoryTarget {
            endpoint,
            owner: &self.config.owner,
            repo: &self.config.repo,
        }
    }

    fn token(&self) -> Token {
        Token::new(self.config.token.clone())
    }

    /// 检查仓库是否存在
    pub async fn check_repo_exists(&self) -> Result<bool> {
        let endpoint = self.endpoint()?;
        self.inner
            .check_repository_exists(&self.target(&endpoint), &self.token())
            .await
            .map_err(to_anyhow)
    }

    /// 创建仓库（无预先存在性检查，保持既有调用方 check→create 的两段式语义）
    pub async fn create_repo(&self, private: bool) -> Result<()> {
        let endpoint = self.endpoint()?;
        self.inner
            .create_repository(&self.target(&endpoint), private, &self.token())
            .await
            .map_err(to_anyhow)
    }

    /// 上传或更新文件到 Gitea，返回远端 URL（若响应携带）
    pub async fn upload_file(
        &self,
        file_path: &Path,
        remote_path: &str,
        message: &str,
    ) -> Result<Option<String>> {
        let content = std::fs::read(file_path).map_err(|_| anyhow!("FILEBAY_UPLOAD_FAILED"))?;
        let endpoint = self.endpoint()?;
        let outcome = self
            .inner
            .upload_bytes(
                &self.target(&endpoint),
                remote_path,
                &content,
                message,
                &self.token(),
            )
            .await
            .map_err(to_anyhow)?;
        Ok(outcome.url)
    }

    /// 删除文件
    pub async fn delete_file(&self, remote_path: &str, message: &str) -> Result<()> {
        let endpoint = self.endpoint()?;
        self.inner
            .delete_file(&self.target(&endpoint), remote_path, message, &self.token())
            .await
            .map_err(to_anyhow)
    }

    /// 获取文件的下载 URL
    pub fn get_download_url(&self, remote_path: &str) -> String {
        format!(
            "{}/{}/{}/raw/branch/main/{}",
            self.config.url, self.config.owner, self.config.repo, remote_path
        )
    }
}
