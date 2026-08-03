use crate::core::{database::Database, filebay_credentials};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FileBayConfig {
    pub url: String,
    pub username: String,
    #[serde(rename = "repoName")]
    pub repo_name: String,
    pub email: String,
    #[serde(rename = "hasToken")]
    pub has_token: bool,
    #[serde(rename = "downloadedAt", default)]
    pub downloaded_at: String,
    #[serde(default)]
    pub version: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FileBayConfigStatus {
    pub exists: bool,
    pub config: Option<FileBayConfig>,
    pub last_modified: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
struct LegacyFileBayConfig {
    url: String,
    username: String,
    #[serde(rename = "repoName")]
    repo_name: String,
    #[serde(default)]
    email: String,
    token: Option<String>,
    #[serde(rename = "downloadedAt", default)]
    downloaded_at: String,
    #[serde(default)]
    version: String,
}

fn get_filebay_config_path_from_downloads(app: &AppHandle) -> Result<PathBuf, String> {
    let app_data_dir = app.path().app_data_dir().map_err(|_| "FILEBAY_CONFIG_STORAGE_FAILED")?;
    Ok(app_data_dir.join("downloads").join("filebay-config.json"))
}

fn safe_config(legacy: &LegacyFileBayConfig) -> FileBayConfig {
    FileBayConfig {
        url: legacy.url.clone(), username: legacy.username.clone(), repo_name: legacy.repo_name.clone(),
        email: legacy.email.clone(), has_token: legacy.token.as_ref().is_some_and(|value| !value.trim().is_empty()),
        downloaded_at: legacy.downloaded_at.clone(), version: legacy.version.clone(),
    }
}

fn save_safe_config(path: &PathBuf, config: &FileBayConfig) -> Result<(), String> {
    let tmp = path.with_extension("json.tmp");
    let bytes = serde_json::to_vec_pretty(config).map_err(|_| "FILEBAY_CONFIG_STORAGE_FAILED")?;
    std::fs::write(&tmp, bytes).map_err(|_| "FILEBAY_CONFIG_STORAGE_FAILED")?;
    std::fs::rename(tmp, path).map_err(|_| "FILEBAY_CONFIG_STORAGE_FAILED".to_string())
}

async fn migrate_legacy_config(path: &PathBuf) -> Result<FileBayConfig, String> {
    let content = std::fs::read_to_string(path).map_err(|_| "FILEBAY_CONFIG_READ_FAILED")?;
    let legacy: LegacyFileBayConfig = serde_json::from_str(&content).map_err(|_| "FILEBAY_CONFIG_READ_FAILED")?;
    if let Some(token) = legacy.token.as_deref().filter(|value| !value.trim().is_empty()) {
        filebay_credentials::set_token(token).map_err(|_| "FILEBAY_CREDENTIAL_MIGRATION_FAILED")?;
        let verified = filebay_credentials::get_token().map_err(|_| "FILEBAY_CREDENTIAL_MIGRATION_FAILED")?.as_deref() == Some(token);
        if !verified { return Err("FILEBAY_CREDENTIAL_MIGRATION_FAILED".to_string()); }
    }
    let mut safe = safe_config(&legacy);
    safe.has_token = filebay_credentials::has_token().map_err(|_| "FILEBAY_CREDENTIAL_STORE_UNAVAILABLE")?;
    save_safe_config(path, &safe)?;
    Ok(safe)
}

#[tauri::command]
pub async fn read_filebay_config(app: AppHandle) -> Result<FileBayConfigStatus, String> {
    let app_path = get_filebay_config_path_from_downloads(&app)?;
    let path = if app_path.exists() {
        app_path
    } else if let Some(downloads) = dirs_next::download_dir() {
        let browser_path = downloads.join("filebay-config.json");
        if browser_path.exists() { browser_path } else { return Ok(FileBayConfigStatus { exists: false, config: None, last_modified: None }); }
    } else {
        return Ok(FileBayConfigStatus { exists: false, config: None, last_modified: None });
    };
    let config = migrate_legacy_config(&path).await?;
    let modified = std::fs::metadata(&path).ok().and_then(|metadata| metadata.modified().ok()).and_then(|time| {
        time.duration_since(std::time::UNIX_EPOCH).ok().and_then(|duration| chrono::DateTime::from_timestamp(duration.as_secs() as i64, 0)).map(|date| date.format("%Y-%m-%d %H:%M:%S").to_string())
    });
    Ok(FileBayConfigStatus { exists: true, config: Some(config), last_modified: modified })
}

#[tauri::command]
pub async fn check_filebay_config_exists(app: AppHandle) -> Result<bool, String> {
    Ok(get_filebay_config_path_from_downloads(&app)?.exists())
}

#[tauri::command]
pub async fn delete_filebay_config(app: AppHandle) -> Result<String, String> {
    let path = get_filebay_config_path_from_downloads(&app)?;
    if path.exists() { std::fs::remove_file(path).map_err(|_| "FILEBAY_CONFIG_DELETE_FAILED")?; }
    let _ = filebay_credentials::delete_token();
    if let Ok(db) = Database::new().await { let _ = db.delete_filebay_config().await; }
    Ok("FILEBAY_CONFIG_DELETED".to_string())
}

async fn validate_internal(path: &PathBuf) -> Result<(LegacyFileBayConfig, FileBayConfig), String> {
    if !path.exists() || path.extension().and_then(|ext| ext.to_str()) != Some("json") { return Err("FILEBAY_CONFIG_INVALID".to_string()); }
    let content = std::fs::read_to_string(path).map_err(|_| "FILEBAY_CONFIG_READ_FAILED")?;
    let legacy: LegacyFileBayConfig = serde_json::from_str(&content).map_err(|_| "FILEBAY_CONFIG_INVALID")?;
    if legacy.url.is_empty() || legacy.username.is_empty() || legacy.repo_name.is_empty() { return Err("FILEBAY_CONFIG_INVALID".to_string()); }
    Ok((legacy.clone(), safe_config(&legacy)))
}

#[tauri::command]
pub async fn validate_filebay_config_file(file_path: String) -> Result<FileBayConfig, String> {
    let (_, config) = validate_internal(&PathBuf::from(file_path)).await?;
    Ok(config)
}

#[tauri::command]
pub async fn import_filebay_config(app: AppHandle, source_path: String) -> Result<String, String> {
    let source = PathBuf::from(source_path);
    let (legacy, mut config) = validate_internal(&source).await?;
    let had_token = legacy.token.as_deref().is_some_and(|value| !value.trim().is_empty());
    if let Some(token) = legacy.token.as_deref().filter(|value| !value.trim().is_empty()) {
        filebay_credentials::set_token(token).map_err(|_| "FILEBAY_CREDENTIAL_MIGRATION_FAILED")?;
        if filebay_credentials::get_token().map_err(|_| "FILEBAY_CREDENTIAL_MIGRATION_FAILED")?.as_deref() != Some(token) { return Err("FILEBAY_CREDENTIAL_MIGRATION_FAILED".to_string()); }
    }
    config.has_token = filebay_credentials::has_token().map_err(|_| "FILEBAY_CREDENTIAL_STORE_UNAVAILABLE")?;
    let target = get_filebay_config_path_from_downloads(&app)?;
    if let Some(parent) = target.parent() { std::fs::create_dir_all(parent).map_err(|_| "FILEBAY_CONFIG_STORAGE_FAILED")?; }
    save_safe_config(&target, &config)?;
    // 迁移成功后，用验证过的无 Token 配置就地覆盖用户导入的源文件，
    // 避免旧明文 Token 继续留在该文件里可被重新读取（F2）。
    if had_token {
        save_safe_config(&source, &config).map_err(|_| "FILEBAY_CREDENTIAL_MIGRATION_FAILED".to_string())?;
    }
    let db = Database::new().await.map_err(|_| "FILEBAY_DATABASE_FAILED")?;
    db.save_filebay_config(&config.url, "", &config.username, &config.repo_name, true).await.map_err(|_| "FILEBAY_DATABASE_FAILED")?;
    Ok("FILEBAY_CONFIG_IMPORTED".to_string())
}
