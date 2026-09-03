use crate::core::database::Database;
use crate::core::filebay_credentials;
use crate::core::gitea::{GiteaClient, GiteaConfig};
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use tauri::State;
use tokio::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GiteaConfigState {
    pub url: String,
    pub owner: String,
    pub repo: String,
    pub enabled: bool,
    #[serde(default)]
    pub has_token: bool,
}

impl Default for GiteaConfigState {
    fn default() -> Self {
        Self {
            url: "https://uat-filebay.cheersai.cloud".to_string(),
            owner: String::new(),
            repo: String::new(),
            enabled: false,
            has_token: false,
        }
    }
}

pub struct GiteaState {
    pub config: Mutex<GiteaConfigState>,
}

impl Default for GiteaState {
    fn default() -> Self {
        Self {
            config: Mutex::new(read_safe_config().unwrap_or_default()),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GiteaStatusResponse {
    pub enabled: bool,
    pub configured: bool,
    pub repo_exists: Option<bool>,
    pub config: GiteaConfigState,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UploadItemResult {
    pub history_id: String,
    pub remote_path: String,
    pub success: bool,
    pub url: Option<String>,
    pub error_code: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UploadResult {
    pub success: bool,
    pub urls: Vec<String>,
    pub message: String,
    pub items: Vec<UploadItemResult>,
}

#[derive(Debug, Deserialize)]
struct LegacyConfig {
    url: Option<String>,
    token: Option<String>,
    owner: Option<String>,
    repo: Option<String>,
    enabled: Option<bool>,
}

#[derive(Debug, Serialize)]
struct SafeConfigFile<'a> {
    url: &'a str,
    owner: &'a str,
    repo: &'a str,
    enabled: bool,
}

fn config_path() -> Result<PathBuf> {
    let dir = std::env::temp_dir().join("cheersai-vault");
    fs::create_dir_all(&dir).map_err(|_| anyhow!("FILEBAY_CONFIG_STORAGE_FAILED"))?;
    Ok(dir.join("gitea_config.json"))
}

fn read_safe_config() -> Result<GiteaConfigState> {
    let path = config_path()?;
    if !path.exists() {
        return Ok(GiteaConfigState::default());
    }
    let content = fs::read_to_string(&path).map_err(|_| anyhow!("FILEBAY_CONFIG_READ_FAILED"))?;
    let legacy: LegacyConfig =
        serde_json::from_str(&content).map_err(|_| anyhow!("FILEBAY_CONFIG_READ_FAILED"))?;
    Ok(GiteaConfigState {
        url: legacy
            .url
            .unwrap_or_else(|| GiteaConfigState::default().url),
        owner: legacy.owner.unwrap_or_default(),
        repo: legacy.repo.unwrap_or_default(),
        enabled: legacy.enabled.unwrap_or(false),
        has_token: false,
    })
}

fn read_legacy_file_token() -> Result<Option<String>> {
    let path = config_path()?;
    if !path.exists() {
        return Ok(None);
    }
    let content = fs::read_to_string(path).map_err(|_| anyhow!("FILEBAY_CONFIG_READ_FAILED"))?;
    let legacy: LegacyConfig =
        serde_json::from_str(&content).map_err(|_| anyhow!("FILEBAY_CONFIG_READ_FAILED"))?;
    Ok(legacy.token.filter(|token| !token.trim().is_empty()))
}

fn save_safe_config(config: &GiteaConfigState) -> Result<()> {
    let path = config_path()?;
    let tmp = path.with_extension("json.tmp");
    let content = serde_json::to_vec_pretty(&SafeConfigFile {
        url: &config.url,
        owner: &config.owner,
        repo: &config.repo,
        enabled: config.enabled,
    })?;
    fs::write(&tmp, content).map_err(|_| anyhow!("FILEBAY_CONFIG_STORAGE_FAILED"))?;
    fs::rename(tmp, path).map_err(|_| anyhow!("FILEBAY_CONFIG_STORAGE_FAILED"))
}

async fn migrate_legacy_credentials() -> Result<()> {
    if filebay_credentials::has_token()? {
        let db = Database::new()
            .await
            .map_err(|_| anyhow!("FILEBAY_CREDENTIAL_MIGRATION_FAILED"))?;
        db.clear_legacy_filebay_token()
            .await
            .map_err(|_| anyhow!("FILEBAY_CREDENTIAL_MIGRATION_FAILED"))?;
        let mut safe = read_safe_config()?;
        safe.has_token = true;
        save_safe_config(&safe)?;
        return Ok(());
    }

    let db = Database::new().await?;
    let legacy = db
        .get_legacy_filebay_token()
        .await?
        .or(read_legacy_file_token()?);
    if let Some(token) = legacy {
        filebay_credentials::set_token(&token)?;
        let verified = filebay_credentials::get_token()?
            .filter(|value| value == &token)
            .is_some();
        if !verified {
            return Err(anyhow!("FILEBAY_CREDENTIAL_MIGRATION_FAILED"));
        }
        db.clear_legacy_filebay_token().await?;
    }
    let mut safe = read_safe_config()?;
    safe.has_token = filebay_credentials::has_token()?;
    save_safe_config(&safe)?;
    Ok(())
}

async fn context(state: &State<'_, GiteaState>) -> Result<(GiteaConfigState, String)> {
    if let Err(error) = migrate_legacy_credentials().await {
        if let Ok(db) = Database::new().await {
            let _ = db.update_filebay_enabled(false).await;
        }
        let mut config = state.config.lock().await;
        config.enabled = false;
        let _ = error;
        return Err(anyhow!("FILEBAY_CREDENTIAL_MIGRATION_FAILED"));
    }
    let db = Database::new().await?;
    let db_config = db.get_filebay_config().await?;
    let mut config = state.config.lock().await;
    if let Some(db_config) = db_config {
        config.url = db_config.url;
        config.owner = db_config.owner;
        config.repo = db_config.repo;
        config.enabled = db_config.enabled;
    }
    config.has_token = filebay_credentials::has_token()?;
    let snapshot = config.clone();
    drop(config);
    let token =
        filebay_credentials::get_token()?.ok_or_else(|| anyhow!("FILEBAY_TOKEN_REQUIRED"))?;
    Ok((snapshot, token))
}

fn validate_https(url: &str) -> Result<()> {
    let parsed = reqwest::Url::parse(url).map_err(|_| anyhow!("FILEBAY_HTTPS_REQUIRED"))?;
    if parsed.scheme() != "https" || parsed.host_str().is_none() {
        return Err(anyhow!("FILEBAY_HTTPS_REQUIRED"));
    }
    Ok(())
}

fn validate_remote_path(path: &str) -> Result<()> {
    if !path.starts_with("masked/")
        || path.len() <= "masked/".len()
        || path.contains("..")
        || path.contains('\\')
        || path.contains("//")
        || path.chars().any(|c| c.is_control() || c == '?' || c == '#')
    {
        return Err(anyhow!("FILEBAY_UPLOAD_DENIED"));
    }
    Ok(())
}

fn validate_artifact(status: &str, input: &Path, output: &Path) -> Result<(PathBuf, String)> {
    if status != "success" {
        return Err(anyhow!("FILEBAY_UPLOAD_DENIED"));
    }
    let metadata = fs::symlink_metadata(output).map_err(|_| anyhow!("FILEBAY_UPLOAD_DENIED"))?;
    if !metadata.file_type().is_file()
        || metadata.file_type().is_symlink()
        || output
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.eq_ignore_ascii_case("md"))
            != Some(true)
    {
        return Err(anyhow!("FILEBAY_UPLOAD_DENIED"));
    }
    let canonical_output =
        fs::canonicalize(output).map_err(|_| anyhow!("FILEBAY_UPLOAD_DENIED"))?;
    let canonical_input = fs::canonicalize(input).ok();
    if canonical_input.as_ref() == Some(&canonical_output) {
        return Err(anyhow!("FILEBAY_UPLOAD_DENIED"));
    }
    // 唯一允许的正式产物位置是 commands/sandbox.rs::list_sandbox_files 定义的
    // 默认沙箱输出目录 temp_dir()/cheersai-vault/output/——脱敏产物真实落在这里。
    // 严格按白名单放行该目录（及其子路径），目录之外的一切路径一律拒绝：
    // 无论是与数据库/配置/PIN 同级的 temp_dir()/cheersai-vault/ 下其它内容，
    // 还是系统临时目录之外任何工作目录、下载目录中的普通 .md，都不构成
    // “正式成功产物”，即便其它检查（终态、扩展名、非符号链接、非原输入）都合法。
    let canonical_temp =
        fs::canonicalize(std::env::temp_dir()).unwrap_or_else(|_| std::env::temp_dir());
    let sandbox_output_root = canonical_temp.join("cheersai-vault").join("output");
    if !canonical_output.starts_with(&sandbox_output_root) {
        return Err(anyhow!("FILEBAY_UPLOAD_DENIED"));
    }
    let display_name = output
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("artifact.md")
        .to_string();
    Ok((canonical_output, display_name))
}

async fn resolve_history(id: &str) -> Result<(PathBuf, String)> {
    if id.is_empty()
        || id.len() > 128
        || !id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(anyhow!("FILEBAY_UPLOAD_DENIED"));
    }
    let db = Database::new().await?;
    let history = db
        .get_processing_history_by_id(id)
        .await?
        .ok_or_else(|| anyhow!("FILEBAY_UPLOAD_DENIED"))?;
    validate_artifact(
        &history.status,
        Path::new(&history.file_path),
        Path::new(&history.output_path),
    )
}

fn safe_error(error: &anyhow::Error) -> String {
    let text = error.to_string();
    if text.starts_with("FILEBAY_") {
        text
    } else if text.contains("request")
        || text.contains("connect")
        || text.contains("dns")
        || text.contains("timed out")
    {
        "FILEBAY_CONNECTION_FAILED".to_string()
    } else {
        "FILEBAY_OPERATION_FAILED".to_string()
    }
}

#[tauri::command]
pub async fn get_gitea_status(state: State<'_, GiteaState>) -> Result<GiteaStatusResponse, String> {
    let (config, token) = match context(&state).await {
        Ok(value) => value,
        Err(error) if error.to_string() == "FILEBAY_TOKEN_REQUIRED" => {
            let config = state.config.lock().await.clone();
            return Ok(GiteaStatusResponse {
                enabled: config.enabled,
                configured: false,
                repo_exists: None,
                config,
            });
        }
        Err(error) => return Err(safe_error(&error)),
    };
    let configured = !config.url.is_empty() && !config.owner.is_empty() && !config.repo.is_empty();
    let repo_exists = if configured && config.enabled && validate_https(&config.url).is_ok() {
        let client = GiteaClient::new(GiteaConfig {
            url: config.url.clone(),
            token,
            owner: config.owner.clone(),
            repo: config.repo.clone(),
        });
        client.check_repo_exists().await.ok()
    } else {
        None
    };
    Ok(GiteaStatusResponse {
        enabled: config.enabled,
        configured,
        repo_exists,
        config,
    })
}

#[tauri::command]
pub async fn update_gitea_config(
    state: State<'_, GiteaState>,
    url: Option<String>,
    token: Option<String>,
    owner: Option<String>,
    repo: Option<String>,
    enabled: Option<bool>,
) -> Result<String, String> {
    let mut config = state.config.lock().await.clone();
    if let Some(value) = url {
        validate_https(&value).map_err(|e| safe_error(&e))?;
        config.url = value;
    }
    if let Some(value) = owner {
        config.owner = value;
    }
    if let Some(value) = repo {
        config.repo = value;
    }
    if let Some(value) = enabled {
        config.enabled = value;
    }
    validate_https(&config.url).map_err(|e| safe_error(&e))?;
    if let Some(value) = token {
        filebay_credentials::set_token(&value).map_err(|e| safe_error(&e))?;
        let verified = filebay_credentials::get_token()
            .map_err(|e| safe_error(&e))?
            .filter(|saved| saved == &value)
            .is_some();
        if !verified {
            return Err("FILEBAY_CREDENTIAL_MIGRATION_FAILED".to_string());
        }
    }
    let db = Database::new()
        .await
        .map_err(|_| "FILEBAY_DATABASE_FAILED".to_string())?;
    db.save_filebay_config(&config.url, "", &config.owner, &config.repo, config.enabled)
        .await
        .map_err(|_| "FILEBAY_DATABASE_FAILED".to_string())?;
    config.has_token = filebay_credentials::has_token().map_err(|e| safe_error(&e))?;
    save_safe_config(&config).map_err(|e| safe_error(&e))?;
    *state.config.lock().await = config;
    Ok("FILEBAY_CONFIG_SAVED".to_string())
}

#[tauri::command]
pub async fn test_gitea_connection(state: State<'_, GiteaState>) -> Result<String, String> {
    let (config, token) = context(&state).await.map_err(|e| safe_error(&e))?;
    validate_https(&config.url).map_err(|e| safe_error(&e))?;
    let client = GiteaClient::new(GiteaConfig {
        url: config.url,
        token,
        owner: config.owner,
        repo: config.repo,
    });
    client
        .check_repo_exists()
        .await
        .map(|_| "FILEBAY_CONNECTION_OK".to_string())
        .map_err(|e| safe_error(&e))
}

#[tauri::command]
pub async fn create_gitea_repo(
    state: State<'_, GiteaState>,
    private: bool,
) -> Result<String, String> {
    let (config, token) = context(&state).await.map_err(|e| safe_error(&e))?;
    if !config.enabled {
        return Err("FILEBAY_DISABLED".to_string());
    }
    validate_https(&config.url).map_err(|e| safe_error(&e))?;
    let client = GiteaClient::new(GiteaConfig {
        url: config.url,
        token,
        owner: config.owner,
        repo: config.repo,
    });
    match client.check_repo_exists().await {
        Ok(true) => Ok("FILEBAY_REPOSITORY_READY".to_string()),
        Ok(false) => client
            .create_repo(private)
            .await
            .map(|_| "FILEBAY_REPOSITORY_CREATED".to_string())
            .map_err(|e| safe_error(&e)),
        Err(e) => Err(safe_error(&e)),
    }
}

/// 给定一个规范化本机路径，只有当它恰好等于某条成功历史记录、经与
/// `validate_artifact` 相同规则校验后的产物路径时才返回该历史 ID；
/// 用于让后端而非文件名字符串比较来确认候选文件身份（F5）。
async fn resolve_history_id_for_output_path(canonical_target: &Path) -> Result<Option<String>> {
    let db = Database::new().await?;
    let histories = db.get_processing_history(None, None).await?;
    for history in histories
        .into_iter()
        .filter(|history| history.status == "success")
    {
        if let Ok((canonical_output, _)) = validate_artifact(
            &history.status,
            Path::new(&history.file_path),
            Path::new(&history.output_path),
        ) {
            if canonical_output == *canonical_target {
                return Ok(Some(history.id));
            }
        }
    }
    Ok(None)
}

#[tauri::command]
pub async fn confirm_filebay_upload_candidates(
    file_paths: Vec<String>,
) -> Result<std::collections::HashMap<String, String>, String> {
    let mut confirmed = std::collections::HashMap::new();
    for file_path in file_paths {
        let Ok(canonical) = fs::canonicalize(&file_path) else {
            continue;
        };
        if let Ok(Some(history_id)) = resolve_history_id_for_output_path(&canonical).await {
            confirmed.insert(file_path, history_id);
        }
    }
    Ok(confirmed)
}

/// 写入最小化上传事件：只包含事件类型、时间、历史 ID/安全文件名（非完整路径）、
/// 目标域名、owner/repo、状态和固定错误码；不写 Token、Authorization、完整路径、
/// 正文或响应 body。日志写入失败被有意吞掉，不得反过来影响已计算出的上传结果（F4）。
async fn record_upload_event(
    config: &GiteaConfigState,
    history_id: &str,
    safe_name: &str,
    success: bool,
    error_code: Option<&str>,
) {
    let Ok(db) = Database::new().await else {
        return;
    };
    let domain = reqwest::Url::parse(&config.url)
        .ok()
        .and_then(|url| url.host_str().map(|host| host.to_string()))
        .unwrap_or_default();
    let details = serde_json::json!({
        "history_id": history_id,
        "domain": domain,
        "owner": config.owner,
        "repo": config.repo,
        "status": if success { "success" } else { "failed" },
        "error_code": error_code,
    })
    .to_string();
    let entry = crate::core::database::LogEntry {
        id: uuid::Uuid::new_v4().to_string(),
        timestamp: chrono::Utc::now(),
        level: if success {
            "info".to_string()
        } else {
            "error".to_string()
        },
        message: if success {
            "FILEBAY_UPLOAD_EVENT_SUCCESS".to_string()
        } else {
            "FILEBAY_UPLOAD_EVENT_FAILED".to_string()
        },
        details: Some(details),
        file_path: Some(safe_name.to_string()),
        operation_type: Some("filebay_upload".to_string()),
        user_id: None,
    };
    let _ = db.add_log(&entry).await;
}

async fn upload_one(
    config: &GiteaConfigState,
    token: &str,
    history_id: &str,
    remote_path: &str,
    message: &str,
) -> Result<(String, String)> {
    validate_remote_path(remote_path)?;
    let (path, display_name) = resolve_history(history_id).await?;
    validate_https(&config.url)?;
    let client = GiteaClient::new(GiteaConfig {
        url: config.url.clone(),
        token: token.to_string(),
        owner: config.owner.clone(),
        repo: config.repo.clone(),
    });
    let url = client
        .upload_file(&path, remote_path, message)
        .await?
        .unwrap_or_else(|| client.get_download_url(remote_path));
    Ok((url, display_name))
}

fn remote_path_basename(remote_path: &str) -> String {
    remote_path
        .rsplit('/')
        .next()
        .unwrap_or(remote_path)
        .to_string()
}

#[tauri::command]
pub async fn upload_to_gitea(
    state: State<'_, GiteaState>,
    history_id: String,
    remote_path: String,
    message: Option<String>,
) -> Result<UploadResult, String> {
    let (config, token) = context(&state).await.map_err(|e| safe_error(&e))?;
    if !config.enabled {
        return Err("FILEBAY_DISABLED".to_string());
    }
    match upload_one(
        &config,
        &token,
        &history_id,
        &remote_path,
        message
            .as_deref()
            .unwrap_or("CheersAI Desktop deidentified artifact upload"),
    )
    .await
    {
        Ok((url, display_name)) => {
            record_upload_event(&config, &history_id, &display_name, true, None).await;
            Ok(UploadResult {
                success: true,
                urls: vec![url.clone()],
                message: "FILEBAY_UPLOAD_OK".to_string(),
                items: vec![UploadItemResult {
                    history_id,
                    remote_path,
                    success: true,
                    url: Some(url),
                    error_code: None,
                }],
            })
        }
        Err(error) => {
            let code = safe_error(&error);
            record_upload_event(
                &config,
                &history_id,
                &remote_path_basename(&remote_path),
                false,
                Some(&code),
            )
            .await;
            Err(code)
        }
    }
}

#[tauri::command]
pub async fn upload_batch_to_gitea(
    state: State<'_, GiteaState>,
    files: Vec<(String, String)>,
    message: Option<String>,
) -> Result<UploadResult, String> {
    let (config, token) = context(&state).await.map_err(|e| safe_error(&e))?;
    if !config.enabled {
        return Err("FILEBAY_DISABLED".to_string());
    }
    let message = message
        .as_deref()
        .unwrap_or("CheersAI Vault deidentified artifact upload");
    let mut items = Vec::with_capacity(files.len());
    let mut urls = Vec::new();
    for (history_id, remote_path) in files {
        match upload_one(&config, &token, &history_id, &remote_path, message).await {
            Ok((url, display_name)) => {
                record_upload_event(&config, &history_id, &display_name, true, None).await;
                urls.push(url.clone());
                items.push(UploadItemResult {
                    history_id,
                    remote_path,
                    success: true,
                    url: Some(url),
                    error_code: None,
                });
            }
            Err(error) => {
                let code = safe_error(&error);
                record_upload_event(
                    &config,
                    &history_id,
                    &remote_path_basename(&remote_path),
                    false,
                    Some(&code),
                )
                .await;
                items.push(UploadItemResult {
                    history_id,
                    remote_path,
                    success: false,
                    url: None,
                    error_code: Some(code),
                });
            }
        }
    }
    let success = items.iter().all(|item| item.success);
    Ok(UploadResult {
        success,
        urls,
        message: if success {
            "FILEBAY_UPLOAD_OK"
        } else {
            "FILEBAY_UPLOAD_PARTIAL_FAILURE"
        }
        .to_string(),
        items,
    })
}

#[tauri::command]
pub async fn delete_from_gitea(
    state: State<'_, GiteaState>,
    remote_path: String,
    message: Option<String>,
) -> Result<String, String> {
    let (config, token) = context(&state).await.map_err(|e| safe_error(&e))?;
    if !config.enabled {
        return Err("FILEBAY_DISABLED".to_string());
    }
    validate_remote_path(&remote_path).map_err(|e| safe_error(&e))?;
    validate_https(&config.url).map_err(|e| safe_error(&e))?;
    let client = GiteaClient::new(GiteaConfig {
        url: config.url,
        token,
        owner: config.owner,
        repo: config.repo,
    });
    client
        .delete_file(
            &remote_path,
            message
                .as_deref()
                .unwrap_or("CheersAI Desktop deidentified artifact delete"),
        )
        .await
        .map(|_| "FILEBAY_DELETE_OK".to_string())
        .map_err(|e| safe_error(&e))
}

#[tauri::command]
pub async fn sync_filebay_config_from_desktop(
    state: State<'_, GiteaState>,
    url: String,
    token: String,
    owner: String,
    repo: String,
) -> Result<String, String> {
    validate_https(&url).map_err(|e| safe_error(&e))?;
    filebay_credentials::set_token(&token).map_err(|e| safe_error(&e))?;
    let verified = filebay_credentials::get_token()
        .map_err(|e| safe_error(&e))?
        .filter(|saved| saved == &token)
        .is_some();
    if !verified {
        return Err("FILEBAY_CREDENTIAL_MIGRATION_FAILED".to_string());
    }
    let db = Database::new()
        .await
        .map_err(|_| "FILEBAY_DATABASE_FAILED".to_string())?;
    db.save_filebay_config(&url, "", &owner, &repo, true)
        .await
        .map_err(|_| "FILEBAY_DATABASE_FAILED".to_string())?;
    let config = GiteaConfigState {
        url,
        owner,
        repo,
        enabled: true,
        has_token: true,
    };
    save_safe_config(&config).map_err(|e| safe_error(&e))?;
    *state.config.lock().await = config;
    Ok("FILEBAY_CONFIG_SYNCED".to_string())
}

#[cfg(test)]
mod tests {
    use super::{validate_artifact, validate_https, validate_remote_path};
    use std::fs;
    use std::path::PathBuf;

    #[test]
    fn rejects_non_https_filebay_endpoints() {
        assert!(validate_https("http://example.invalid").is_err());
        assert!(validate_https("https://example.invalid").is_ok());
    }

    #[test]
    fn only_masked_remote_paths_are_allowed() {
        assert!(validate_remote_path("masked/report.md").is_ok());
        assert!(validate_remote_path("raw/report.md").is_err());
        assert!(validate_remote_path("masked/../secret.md").is_err());
        assert!(validate_remote_path("masked\\report.md").is_err());
    }

    #[test]
    fn rejects_artifacts_outside_sandbox_output_directory_even_when_otherwise_valid() {
        // F3-R3：只有正式沙箱输出目录 temp_dir()/cheersai-vault/output/ 下的产物可以
        // 上传，目录之外的一切路径一律拒绝——即便终态、扩展名、非符号链接、非原输入
        // 等其它检查全部合法。用 target/ 下的独立目录模拟任意工作目录/自定义目录，
        // 证明它不再被当作正例放行（原先这里曾错误地断言 .is_ok()）。
        let root = std::env::current_dir()
            .unwrap()
            .join("target")
            .join(format!("filebay-artifact-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let input = root.join("input.csv");
        let output = root.join("masked.md");
        fs::write(&input, "fake input").unwrap();
        fs::write(&output, "fake deidentified output").unwrap();
        assert_eq!(
            validate_artifact("success", &input, &output)
                .unwrap_err()
                .to_string(),
            "FILEBAY_UPLOAD_DENIED"
        );
        assert_eq!(
            validate_artifact("failed", &input, &output)
                .unwrap_err()
                .to_string(),
            "FILEBAY_UPLOAD_DENIED"
        );
        assert!(validate_artifact("success", &input, &root.join("not-md.txt")).is_err());
        assert!(validate_artifact("success", &output, &output).is_err());
        #[cfg(unix)]
        {
            let link = root.join("link.md");
            std::os::unix::fs::symlink(&output, &link).unwrap();
            assert!(validate_artifact("success", &input, &link).is_err());
        }
        let _ = fs::remove_dir_all(PathBuf::from(root));
    }

    #[test]
    fn rejects_artifacts_stored_under_system_temp_directory() {
        // 应用自身的 SQLite 数据库、gitea_config.json 与 PIN 文件都放在系统临时目录下；
        // 本用例复现 Review F3 指出的“配置/日志/数据库旁边伪装 .md”场景。
        let root =
            std::env::temp_dir().join(format!("cheers-filebay-temp-denied-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let input = root.join("input.csv");
        let output = root.join("masked.md");
        fs::write(&input, "fake input").unwrap();
        fs::write(
            &output,
            "fake deidentified output disguised next to sensitive files",
        )
        .unwrap();
        assert_eq!(
            validate_artifact("success", &input, &output)
                .unwrap_err()
                .to_string(),
            "FILEBAY_UPLOAD_DENIED"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn accepts_default_sandbox_output_directory_while_still_denying_its_temp_dir_siblings() {
        // Review F3-R2：temp_dir()/cheersai-vault/output/ 是 commands/sandbox.rs::list_sandbox_files
        // 定义的正式默认沙箱输出目录，必须放行；但同一 cheersai-vault/ 目录下、不在
        // output/ 子目录内的路径（与真实数据库/配置同级）必须继续拒绝。只写入本用例
        // 自己的唯一命名文件，不清空或改动 output/ 目录中可能存在的其它真实产物。
        let app_dir = std::env::temp_dir().join("cheersai-vault");
        let output_dir = app_dir.join("output");
        fs::create_dir_all(&output_dir).unwrap();
        let unique = format!(
            "filebay-sandbox-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let input = output_dir.join(format!("{unique}-input.csv"));
        let output = output_dir.join(format!("{unique}-masked.md"));
        fs::write(&input, "fake input").unwrap();
        fs::write(
            &output,
            "fake deidentified output in default sandbox directory",
        )
        .unwrap();
        assert!(validate_artifact("success", &input, &output).is_ok());

        let sibling_output = app_dir.join(format!("{unique}-sibling.md"));
        fs::write(
            &sibling_output,
            "disguised next to real config/db, outside output/",
        )
        .unwrap();
        assert_eq!(
            validate_artifact("success", &input, &sibling_output)
                .unwrap_err()
                .to_string(),
            "FILEBAY_UPLOAD_DENIED"
        );

        let _ = fs::remove_file(&input);
        let _ = fs::remove_file(&output);
        let _ = fs::remove_file(&sibling_output);
    }

    #[test]
    fn same_named_files_in_different_directories_have_distinct_canonical_identity() {
        // F5 的候选绑定依赖 validate_artifact 返回的 canonical 路径具有唯一性；
        // 本用例在正式沙箱输出目录（F3-R3 起唯一放行的位置）下的两个不同子目录里
        // 各放一个同名 report.md，证明两个同名但位于不同目录的 .md 文件不会被
        // 当成同一产物冒用。
        let sandbox_root = std::env::temp_dir().join("cheersai-vault").join("output");
        let unique = format!(
            "filebay-identity-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let root_a = sandbox_root.join(format!("{unique}-a"));
        let root_b = sandbox_root.join(format!("{unique}-b"));
        let _ = fs::remove_dir_all(&root_a);
        let _ = fs::remove_dir_all(&root_b);
        fs::create_dir_all(&root_a).unwrap();
        fs::create_dir_all(&root_b).unwrap();
        let input_a = root_a.join("input.csv");
        let output_a = root_a.join("report.md");
        let input_b = root_b.join("input.csv");
        let output_b = root_b.join("report.md");
        fs::write(&input_a, "fake input a").unwrap();
        fs::write(&output_a, "genuine masked output for history H").unwrap();
        fs::write(&input_b, "fake input b").unwrap();
        fs::write(
            &output_b,
            "unrelated plain file that happens to share a name",
        )
        .unwrap();
        let (canonical_a, _) = validate_artifact("success", &input_a, &output_a).unwrap();
        let (canonical_b, _) = validate_artifact("success", &input_b, &output_b).unwrap();
        assert_ne!(
            canonical_a, canonical_b,
            "同名但不同目录的产物必须拥有不同身份，不能被互相冒用"
        );
        let _ = fs::remove_dir_all(&root_a);
        let _ = fs::remove_dir_all(&root_b);
    }

    #[test]
    fn failed_upload_result_is_not_success() {
        let item = super::UploadItemResult {
            history_id: "fake-history".into(),
            remote_path: "masked/report.md".into(),
            success: false,
            url: None,
            error_code: Some("FILEBAY_UPLOAD_FAILED".into()),
        };
        let result = super::UploadResult {
            success: false,
            urls: vec![],
            message: "FILEBAY_UPLOAD_PARTIAL_FAILURE".into(),
            items: vec![item],
        };
        assert!(!result.success);
        assert!(!result.items[0].success);
    }
}
