use anyhow::{Context, Result};
use base64::{engine::general_purpose, Engine as _};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GiteaConfig {
    pub url: String,
    pub token: String,
    pub owner: String,
    pub repo: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GiteaFileResponse {
    pub content: Option<GiteaFileContent>,
    pub commit: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GiteaFileContent {
    pub name: String,
    pub path: String,
    pub sha: String,
    pub size: i64,
    pub url: String,
    pub html_url: String,
    pub git_url: String,
    pub download_url: String,
}

pub struct GiteaClient {
    config: GiteaConfig,
    client: reqwest::Client,
}

impl GiteaClient {
    pub fn new(config: GiteaConfig) -> Self {
        // 为 UAT 环境配置接受不安全的证书
        let client = if config.url.contains("uat-filebay") {
            reqwest::Client::builder()
                .danger_accept_invalid_certs(true)
                .build()
                .unwrap_or_else(|_| reqwest::Client::new())
        } else {
            reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .connect_timeout(std::time::Duration::from_secs(10))
                .build()
                .unwrap_or_else(|_| reqwest::Client::new())
        };

        Self {
            config,
            client,
        }
    }

    /// 检查仓库是否存在
    pub async fn check_repo_exists(&self) -> Result<bool> {
        let url = format!(
            "{}/api/v1/repos/{}/{}",
            self.config.url, self.config.owner, self.config.repo
        );

        let response = self
            .client
            .get(&url)
            .header("Authorization", format!("token {}", self.config.token))
            .send()
            .await?;

        Ok(response.status().is_success())
    }

    /// 创建仓库
    pub async fn create_repo(&self, private: bool) -> Result<()> {
        let url = format!("{}/api/v1/user/repos", self.config.url);

        let body = serde_json::json!({
            "name": self.config.repo,
            "private": private,
            "auto_init": true,
            "description": "CheersAI Vault - 脱敏文件存储仓库"
        });

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("token {}", self.config.token))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow::anyhow!("Failed to create repo: {}", error_text));
        }

        Ok(())
    }

    /// 获取文件内容（用于检查文件是否存在并获取 SHA）
    pub async fn get_file(&self, path: &str) -> Result<Option<GiteaFileContent>> {
        let url = format!(
            "{}/api/v1/repos/{}/{}/contents/{}",
            self.config.url, self.config.owner, self.config.repo, path
        );

        let response = self
            .client
            .get(&url)
            .header("Authorization", format!("token {}", self.config.token))
            .send()
            .await?;

        if response.status() == 404 {
            return Ok(None);
        }

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow::anyhow!("Failed to get file: {}", error_text));
        }

        let file_content: GiteaFileContent = response.json().await?;
        Ok(Some(file_content))
    }

    /// 上传或更新文件到 Gitea
    pub async fn upload_file(
        &self,
        file_path: &Path,
        remote_path: &str,
        message: &str,
    ) -> Result<GiteaFileResponse> {
        // 读取文件内容
        let content = std::fs::read(file_path)
            .with_context(|| format!("Failed to read file: {}", file_path.display()))?;

        // Base64 编码
        let content_b64 = general_purpose::STANDARD.encode(&content);

        // 检查文件是否已存在
        let existing_file = self.get_file(remote_path).await?;
        let is_update = existing_file.is_some();
        
        let operation = if is_update { "更新" } else { "创建" };
        println!("文件{}，将执行{}操作: {}", if is_update { "已存在" } else { "不存在" }, operation, remote_path);

        let url = format!(
            "{}/api/v1/repos/{}/{}/contents/{}",
            self.config.url, self.config.owner, self.config.repo, remote_path
        );

        let mut body = serde_json::json!({
            "content": content_b64,
            "message": message,
        });

        // 如果文件已存在，需要提供 SHA 来更新
        if let Some(existing) = existing_file {
            body["sha"] = serde_json::json!(existing.sha);
            println!("使用 SHA 进行更新: {}", existing.sha);
        }

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("token {}", self.config.token))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            
            println!("上传失败 - 状态码: {}, 响应: {}", status, error_text);
            
            // 将技术错误转换为用户友好的消息
            let user_message = if error_text.contains("already exists") {
                // 这个错误不应该发生，因为我们已经检查并提供了 SHA
                format!("文件同步失败，请重试")
            } else if status == 401 || error_text.contains("unauthorized") {
                format!("认证失败，请检查 Token 配置")
            } else if status == 404 {
                format!("仓库不存在，请先创建仓库")
            } else {
                format!("网络错误，请检查 Gitea 服务是否正常")
            };
            
            return Err(anyhow::anyhow!("{}", user_message));
        }

        let file_response: GiteaFileResponse = response.json().await?;
        println!("文件上传成功: {}", remote_path);
        Ok(file_response)
    }

    /// 批量上传文件
    pub async fn upload_files(
        &self,
        files: Vec<(std::path::PathBuf, String)>, // (本地路径, 远程路径)
        base_message: &str,
    ) -> Result<Vec<String>> {
        let mut uploaded_urls = Vec::new();

        for (local_path, remote_path) in files {
            let message = format!("{} - {}", base_message, remote_path);
            
            match self.upload_file(&local_path, &remote_path, &message).await {
                Ok(response) => {
                    if let Some(content) = response.content {
                        let url = content.html_url.clone();
                        uploaded_urls.push(content.html_url);
                        println!("Uploaded: {} -> {}", local_path.display(), url);
                    }
                }
                Err(e) => {
                    eprintln!("Failed to upload {}: {}", local_path.display(), e);
                    return Err(e);
                }
            }
        }

        Ok(uploaded_urls)
    }

    /// 获取文件的下载 URL
    pub fn get_download_url(&self, remote_path: &str) -> String {
        format!(
            "{}/{}/{}/raw/branch/main/{}",
            self.config.url, self.config.owner, self.config.repo, remote_path
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore] // 需要真实的 Gitea 服务器才能运行
    async fn test_gitea_client() {
        let config = GiteaConfig {
            url: "http://localhost:3001".to_string(),
            token: "your-token".to_string(),
            owner: "test-user".to_string(),
            repo: "test-repo".to_string(),
        };

        let client = GiteaClient::new(config);
        
        // 测试检查仓库
        let exists = client.check_repo_exists().await.unwrap();
        println!("Repo exists: {}", exists);
    }
}
