use crate::core::filebay_credentials;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

#[derive(Debug, Serialize, Deserialize)]
pub struct SyncConfigRequest {
    pub url: String,
    pub username: String,
    pub repo_name: String,
    pub email: String,
    pub token: Option<String>,
    pub user_id: Option<String>,
}

fn require_https(url: &str) -> Result<(), String> {
    let parsed = reqwest::Url::parse(url).map_err(|_| "FILEBAY_HTTPS_REQUIRED")?;
    if parsed.scheme() != "https" || parsed.host_str().is_none() {
        return Err("FILEBAY_HTTPS_REQUIRED".to_string());
    }
    Ok(())
}

#[tauri::command]
pub async fn sync_config_from_desktop(
    app: AppHandle,
    config: SyncConfigRequest,
) -> Result<String, String> {
    require_https(&config.url)?;
    if let Some(token) = config
        .token
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        filebay_credentials::set_token(token).map_err(|_| "FILEBAY_CREDENTIAL_MIGRATION_FAILED")?;
        if filebay_credentials::get_token()
            .map_err(|_| "FILEBAY_CREDENTIAL_MIGRATION_FAILED")?
            .as_deref()
            != Some(token)
        {
            return Err("FILEBAY_CREDENTIAL_MIGRATION_FAILED".to_string());
        }
    }

    let vault_path = get_vault_db_path();
    if vault_path.exists() {
        let pool = sqlx::sqlite::SqlitePool::connect(&format!(
            "sqlite://{}",
            vault_path.to_string_lossy()
        ))
        .await
        .map_err(|_| "FILEBAY_DATABASE_FAILED")?;
        let existing: Option<(String,)> =
            sqlx::query_as("SELECT user_id FROM filebay_configs WHERE email = ? OR username = ?")
                .bind(&config.email)
                .bind(&config.username)
                .fetch_optional(&pool)
                .await
                .map_err(|_| "FILEBAY_DATABASE_FAILED")?;
        if let Some((user_id,)) = existing {
            sqlx::query("UPDATE filebay_configs SET url = ?, username = ?, repo_name = ?, email = ?, token = '', updated_at = datetime('now') WHERE user_id = ?")
                .bind(&config.url).bind(&config.username).bind(&config.repo_name).bind(&config.email).bind(user_id).execute(&pool).await.map_err(|_| "FILEBAY_DATABASE_FAILED")?;
        } else {
            let user_id = config
                .user_id
                .clone()
                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
            sqlx::query("INSERT INTO filebay_configs (user_id, url, username, repo_name, email, token, updated_at) VALUES (?, ?, ?, ?, ?, '', datetime('now'))")
                .bind(user_id).bind(&config.url).bind(&config.username).bind(&config.repo_name).bind(&config.email).execute(&pool).await.map_err(|_| "FILEBAY_DATABASE_FAILED")?;
        }
        pool.close().await;
    }

    let gitea_path = get_gitea_config_path()?;
    let gitea_config = serde_json::json!({ "url": config.url, "owner": config.username, "repo": config.repo_name, "enabled": true });
    std::fs::write(
        gitea_path,
        serde_json::to_vec_pretty(&gitea_config).map_err(|_| "FILEBAY_CONFIG_STORAGE_FAILED")?,
    )
    .map_err(|_| "FILEBAY_CONFIG_STORAGE_FAILED")?;

    let filebay_path = get_filebay_config_path(&app)?;
    if let Some(parent) = filebay_path.parent() {
        std::fs::create_dir_all(parent).map_err(|_| "FILEBAY_CONFIG_STORAGE_FAILED")?;
    }
    let filebay_config = serde_json::json!({
        "url": config.url, "username": config.username, "repoName": config.repo_name,
        "email": config.email, "hasToken": filebay_credentials::has_token().map_err(|_| "FILEBAY_CREDENTIAL_STORE_UNAVAILABLE")?,
        "downloadedAt": chrono::Utc::now().to_rfc3339(), "version": "1.0.0"
    });
    std::fs::write(
        filebay_path,
        serde_json::to_vec_pretty(&filebay_config).map_err(|_| "FILEBAY_CONFIG_STORAGE_FAILED")?,
    )
    .map_err(|_| "FILEBAY_CONFIG_STORAGE_FAILED")?;
    Ok("FILEBAY_CONFIG_SYNCED".to_string())
}

fn get_vault_db_path() -> PathBuf {
    dirs_next::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".cheersai")
        .join("vault.db")
}

fn get_gitea_config_path() -> Result<PathBuf, String> {
    let dir = std::env::temp_dir().join("cheersai-vault");
    std::fs::create_dir_all(&dir).map_err(|_| "FILEBAY_CONFIG_STORAGE_FAILED")?;
    Ok(dir.join("gitea_config.json"))
}

fn get_filebay_config_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app
        .path()
        .app_data_dir()
        .map_err(|_| "FILEBAY_CONFIG_STORAGE_FAILED")?
        .join("downloads")
        .join("filebay-config.json"))
}
