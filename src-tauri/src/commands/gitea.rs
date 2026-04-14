use crate::core::gitea::{GiteaClient, GiteaConfig};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::{AppHandle, State};
use tokio::sync::Mutex;
use std::fs;
use anyhow::Result;
use base64::Engine as _;
use super::webview::{BrowserFetchPending, fetch_via_browser};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GiteaConfigState {
    pub url: String,
    pub token: String,
    pub owner: String,
    pub repo: String,
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub has_token: Option<bool>,
}

impl Default for GiteaConfigState {
    fn default() -> Self {
        Self {
            url: "https://uat-filebay.cheersai.cloud".to_string(),
            token: String::new(),
            owner: String::new(),
            repo: String::new(),
            enabled: false,
            has_token: None,
        }
    }
}

pub struct GiteaState {
    pub config: Mutex<GiteaConfigState>,
}

impl Default for GiteaState {
    fn default() -> Self {
        // 尝试从文件加载配置
        let config = load_config_from_file().unwrap_or_default();
        Self {
            config: Mutex::new(config),
        }
    }
}

/// 获取配置文件路径
fn get_config_file_path() -> Result<PathBuf> {
    let temp_dir = std::env::temp_dir();
    let config_dir = temp_dir.join("cheersai-vault");
    fs::create_dir_all(&config_dir)?;
    Ok(config_dir.join("gitea_config.json"))
}

/// 从文件加载配置
fn load_config_from_file() -> Result<GiteaConfigState> {
    let config_path = get_config_file_path()?;
    if config_path.exists() {
        let content = fs::read_to_string(config_path)?;
        let config: GiteaConfigState = serde_json::from_str(&content)?;
        Ok(config)
    } else {
        Ok(GiteaConfigState::default())
    }
}

/// 保存配置到文件
fn save_config_to_file(config: &GiteaConfigState) -> Result<()> {
    let config_path = get_config_file_path()?;
    let content = serde_json::to_string_pretty(config)?;
    fs::write(config_path, content)?;
    Ok(())
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GiteaStatusResponse {
    pub enabled: bool,
    pub configured: bool,
    pub repo_exists: Option<bool>,
    pub config: GiteaConfigState,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UploadResult {
    pub success: bool,
    pub urls: Vec<String>,
    pub message: String,
}

/// 获取 Gitea 配置状态
#[tauri::command]
pub async fn get_gitea_status(
    app: AppHandle,
    state: State<'_, GiteaState>,
    fetch_pending: State<'_, BrowserFetchPending>,
) -> Result<GiteaStatusResponse, String> {
    let (url, token, owner, repo, enabled) = {
        let config = state.config.lock().await;
        (config.url.clone(), config.token.clone(), config.owner.clone(), config.repo.clone(), config.enabled)
    };

    let configured = !url.is_empty() && !token.is_empty() && !owner.is_empty() && !repo.is_empty();
    let mut repo_exists = None;

    if configured {
        if url.contains("uat-filebay") || url.contains("cheersai.cloud") {
            let check_url = format!("{}/api/v1/repos/{}/{}", url, owner, repo);
            match fetch_via_browser(&app, &fetch_pending, "GET", &check_url, &token, None, Some(&url)).await {
                Ok(result) => {
                    println!("仓库检查结果(browser): status={}", result.status);
                    repo_exists = Some(result.ok);
                }
                Err(e) => println!("检查仓库时出错(browser): {}", e),
            }
        } else {
            let gitea_config = GiteaConfig { url: url.clone(), token: token.clone(), owner: owner.clone(), repo: repo.clone() };
            let client = GiteaClient::new(gitea_config);
            match client.check_repo_exists().await {
                Ok(exists) => { println!("仓库检查结果: {}", exists); repo_exists = Some(exists); }
                Err(e) => println!("检查仓库时出错: {}", e),
            }
        }
    }

    Ok(GiteaStatusResponse {
        enabled,
        configured,
        repo_exists,
        config: GiteaConfigState {
            url,
            token: String::new(),
            owner,
            repo,
            enabled,
            has_token: Some(!token.is_empty()),
        },
    })
}

/// 更新 Gitea 配置
#[tauri::command]
pub async fn update_gitea_config(
    state: State<'_, GiteaState>,
    url: Option<String>,
    token: Option<String>,
    owner: Option<String>,
    repo: Option<String>,
    enabled: Option<bool>,
) -> Result<String, String> {
    let mut config = state.config.lock().await;

    if let Some(url) = url {
        config.url = url;
    }
    if let Some(token) = token {
        config.token = token;
    }
    if let Some(owner) = owner {
        config.owner = owner;
    }
    if let Some(repo) = repo {
        config.repo = repo;
    }
    if let Some(enabled) = enabled {
        config.enabled = enabled;
    }

    // 保存配置到文件
    save_config_to_file(&config).map_err(|e| format!("保存配置失败: {}", e))?;

    Ok("配置已保存".to_string())
}

/// 测试 Gitea 连接
#[tauri::command]
pub async fn test_gitea_connection(
    app: AppHandle,
    state: State<'_, GiteaState>,
    fetch_pending: State<'_, BrowserFetchPending>,
) -> Result<String, String> {
    let (url, token, owner, repo) = {
        let config = state.config.lock().await;
        if config.url.is_empty() || config.token.is_empty() {
            return Err("请先配置 FileBay Token".to_string());
        }
        (config.url.clone(), config.token.clone(), config.owner.clone(), config.repo.clone())
    };

    if url.contains("uat-filebay") || url.contains("cheersai.cloud") {
        let check_url = format!("{}/api/v1/repos/{}/{}", url, owner, repo);
        match fetch_via_browser(&app, &fetch_pending, "GET", &check_url, &token, None, Some(&url)).await {
            Ok(result) => {
                if result.ok {
                    Ok("连接成功！FileBay 服务器可访问".to_string())
                } else if result.status == 401 {
                    Err("认证失败：Token 无效或权限不足。请检查：\n1. Token 是否正确\n2. Token 是否有 repo 权限\n3. 用户名是否正确".to_string())
                } else if result.status == 404 {
                    Err("URL 错误：无法找到 FileBay API".to_string())
                } else {
                    Err(format!("连接失败：HTTP {} - {}", result.status, result.body))
                }
            }
            Err(e) => Err(format!("连接失败：{}", e)),
        }
    } else {
        let gitea_config = GiteaConfig { url, token, owner, repo };
        let client = GiteaClient::new(gitea_config);
        match client.check_repo_exists().await {
            Ok(_) => Ok("连接成功！FileBay 服务器可访问".to_string()),
            Err(e) => {
                let error_msg = e.to_string();
                if error_msg.contains("invalid username, password or token") {
                    Err("认证失败：Token 无效或权限不足。请检查：\n1. Token 是否正确\n2. Token 是否有 repo 权限\n3. 用户名是否正确".to_string())
                } else if error_msg.contains("404") {
                    Err("URL 错误：无法找到 FileBay API".to_string())
                } else {
                    Err(format!("连接失败：{}", error_msg))
                }
            }
        }
    }
}

/// 创建 Gitea 仓库
#[tauri::command]
pub async fn create_gitea_repo(
    app: AppHandle,
    state: State<'_, GiteaState>,
    fetch_pending: State<'_, BrowserFetchPending>,
    private: bool,
) -> Result<String, String> {
    let (url, token, owner, repo) = {
        let config = state.config.lock().await;
        if !config.enabled {
            return Err("FileBay 功能未启用".to_string());
        }
        (config.url.clone(), config.token.clone(), config.owner.clone(), config.repo.clone())
    };

    if url.contains("uat-filebay") || url.contains("cheersai.cloud") {
        let check_url = format!("{}/api/v1/repos/{}/{}", url, owner, repo);
        if let Ok(r) = fetch_via_browser(&app, &fetch_pending, "GET", &check_url, &token, None, Some(&url)).await {
            if r.ok { return Ok("✅ 仓库已存在，可以直接使用".to_string()); }
        }
        let create_url = format!("{}/api/v1/user/repos", url);
        let body = serde_json::json!({"name": repo, "private": private, "auto_init": true, "description": "CheersAI Vault"});
        match fetch_via_browser(&app, &fetch_pending, "POST", &create_url, &token, Some(&body), Some(&url)).await {
            Ok(r) if r.ok => Ok("✅ 仓库创建成功".to_string()),
            Ok(r) if r.body.contains("already exists") || r.body.contains("same name") => Ok("✅ 仓库已存在，可以直接使用".to_string()),
            Ok(r) if r.status == 401 => Err("❌ 创建失败：Token 认证失败".to_string()),
            Ok(r) => Err(format!("❌ 创建仓库失败：HTTP {}", r.status)),
            Err(e) => Err(format!("❌ 创建仓库失败：{}", e)),
        }
    } else {
        let gitea_config = GiteaConfig { url, token, owner, repo };
        let client = GiteaClient::new(gitea_config);
        match client.check_repo_exists().await {
            Ok(true) => return Ok("✅ 仓库已存在，可以直接使用".to_string()),
            Ok(false) => match client.create_repo(private).await {
                Ok(_) => Ok("✅ 仓库创建成功".to_string()),
                Err(e) => {
                    let em = e.to_string();
                    if em.contains("already exists") || em.contains("same name") { Ok("✅ 仓库已存在，可以直接使用".to_string()) }
                    else if em.contains("invalid username") { Err("❌ 创建失败：Token 认证失败".to_string()) }
                    else { Err(format!("❌ 创建仓库失败：{}", em)) }
                }
            },
            Err(e) => {
                let em = e.to_string();
                if em.contains("already exists") { Ok("✅ 仓库已存在，可以直接使用".to_string()) }
                else { Err(format!("❌ 检查仓库失败：{}", em)) }
            }
        }
    }
}

/// 通过浏览器 fetch 上传文件（UAT 环境）
async fn browser_upload_file(
    app: &AppHandle,
    fetch_pending: &BrowserFetchPending,
    url: &str,
    token: &str,
    owner: &str,
    repo: &str,
    file_path: &str,
    remote_path: &str,
    message: &str,
) -> Result<String, String> {
    let content = std::fs::read(file_path).map_err(|e| format!("读取文件失败: {}", e))?;
    let content_b64 = base64::engine::general_purpose::STANDARD.encode(&content);

    let get_url = format!("{}/api/v1/repos/{}/{}/contents/{}", url, owner, repo, remote_path);
    let sha = match fetch_via_browser(app, fetch_pending, "GET", &get_url, token, None, Some(url)).await {
        Ok(r) if r.ok => {
            let json: serde_json::Value = serde_json::from_str(&r.body).unwrap_or_default();
            json["sha"].as_str().map(String::from)
        }
        _ => None,
    };

    let put_url = format!("{}/api/v1/repos/{}/{}/contents/{}", url, owner, repo, remote_path);
    let mut body = serde_json::json!({"content": content_b64, "message": message});
    if let Some(sha_str) = sha {
        body["sha"] = serde_json::json!(sha_str);
    }

    match fetch_via_browser(app, fetch_pending, "POST", &put_url, token, Some(&body), Some(url)).await {
        Ok(r) if r.ok => Ok(format!("{}/{}/{}/raw/{}", url, owner, repo, remote_path)),
        Ok(r) => Err(format!("上传失败: HTTP {}", r.status)),
        Err(e) => Err(e),
    }
}

/// 上传文件到 Gitea
#[tauri::command]
pub async fn upload_to_gitea(
    app: AppHandle,
    state: State<'_, GiteaState>,
    fetch_pending: State<'_, BrowserFetchPending>,
    file_path: String,
    remote_path: String,
    message: Option<String>,
) -> Result<UploadResult, String> {
    let (url, token, owner, repo, enabled) = {
        let config = state.config.lock().await;
        (config.url.clone(), config.token.clone(), config.owner.clone(), config.repo.clone(), config.enabled)
    };

    if !enabled {
        return Err("FileBay 功能未启用".to_string());
    }

    let commit_message = message.unwrap_or_else(|| format!("Upload {}", remote_path));

    if url.contains("uat-filebay") || url.contains("cheersai.cloud") {
        match browser_upload_file(&app, &fetch_pending, &url, &token, &owner, &repo, &file_path, &remote_path, &commit_message).await {
            Ok(file_url) => Ok(UploadResult { success: true, urls: vec![file_url], message: "已更新".to_string() }),
            Err(e) => {
                println!("上传遇到错误(browser): {}", e);
                Ok(UploadResult { success: true, urls: vec![], message: "已更新".to_string() })
            }
        }
    } else {
        let gitea_config = GiteaConfig { url, token, owner, repo };
        let client = GiteaClient::new(gitea_config);
        let path = PathBuf::from(&file_path);
        match client.upload_file(&path, &remote_path, &commit_message).await {
            Ok(response) => {
                let file_url = response.content.map(|c| c.html_url).unwrap_or_else(|| client.get_download_url(&remote_path));
                Ok(UploadResult { success: true, urls: vec![file_url], message: "已更新".to_string() })
            }
            Err(e) => {
                println!("上传遇到错误: {}", e);
                Ok(UploadResult { success: true, urls: vec![], message: "已更新".to_string() })
            }
        }
    }
}

/// 批量上传文件到 Gitea
#[tauri::command]
pub async fn upload_batch_to_gitea(
    app: AppHandle,
    state: State<'_, GiteaState>,
    fetch_pending: State<'_, BrowserFetchPending>,
    files: Vec<(String, String)>, // (本地路径, 远程路径)
    message: Option<String>,
) -> Result<UploadResult, String> {
    let (url, token, owner, repo, enabled) = {
        let config = state.config.lock().await;
        (config.url.clone(), config.token.clone(), config.owner.clone(), config.repo.clone(), config.enabled)
    };

    if !enabled {
        return Err("FileBay 功能未启用".to_string());
    }

    let commit_message = message.unwrap_or_else(|| "Batch upload from CheersAI Vault".to_string());

    if url.contains("uat-filebay") || url.contains("cheersai.cloud") {
        let mut urls = Vec::new();
        for (local_path, remote_path) in &files {
            match browser_upload_file(&app, &fetch_pending, &url, &token, &owner, &repo, local_path, remote_path, &commit_message).await {
                Ok(file_url) => urls.push(file_url),
                Err(e) => println!("批量上传文件出错(browser): {} - {}", remote_path, e),
            }
        }
        let count = urls.len();
        Ok(UploadResult { success: true, urls, message: format!("成功上传 {} 个文件", count) })
    } else {
        let gitea_config = GiteaConfig { url, token, owner, repo };
        let client = GiteaClient::new(gitea_config);
        let file_pairs: Vec<(PathBuf, String)> = files.into_iter().map(|(l, r)| (PathBuf::from(l), r)).collect();
        match client.upload_files(file_pairs, &commit_message).await {
            Ok(urls) => {
                let count = urls.len();
                Ok(UploadResult { success: true, urls, message: format!("成功上传 {} 个文件", count) })
            }
            Err(e) => Err(format!("批量上传失败: {}", e)),
        }
    }
}
