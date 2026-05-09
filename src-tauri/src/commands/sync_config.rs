/// 从 Desktop 在线工作区同步配置到本地
/// 
/// 这个命令可以从 Desktop 的 localStorage/cookies 中读取当前用户的配置
/// 并同步到本地的 Vault 数据库和配置文件

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use std::path::PathBuf;
use std::fs;

#[derive(Debug, Serialize, Deserialize)]
pub struct SyncConfigRequest {
    pub url: String,
    pub username: String,
    pub repo_name: String,
    pub email: String,
    pub token: String,
    pub user_id: Option<String>,
}

/// 从 Desktop 在线工作区同步配置
/// 
/// 这个命令接收从 Desktop 前端传递的配置信息，并同步到：
/// 1. Vault Bridge 数据库 (~/.cheersai/vault.db)
/// 2. Gitea 配置文件 (gitea_config.json)
/// 3. FileBay 配置文件 (filebay-config.json)
#[tauri::command]
pub async fn sync_config_from_desktop(
    app: AppHandle,
    config: SyncConfigRequest,
) -> Result<String, String> {
    use sqlx::sqlite::SqlitePool;
    
    println!("=== Syncing config from Desktop ===");
    println!("URL: {}", config.url);
    println!("Username: {}", config.username);
    println!("Email: {}", config.email);
    println!("Repo: {}", config.repo_name);
    
    // 1. 更新 Vault Bridge 数据库
    let vault_db_path = get_vault_db_path();
    if vault_db_path.exists() {
        println!("Updating Vault database: {:?}", vault_db_path);
        
        let db_url = format!("sqlite://{}", vault_db_path.to_string_lossy());
        let pool = SqlitePool::connect(&db_url)
            .await
            .map_err(|e| format!("无法打开 Vault 数据库: {}", e))?;
        
        // 检查是否已存在配置
        let existing: Option<(String,)> = sqlx::query_as(
            "SELECT user_id FROM filebay_configs WHERE email = ? OR username = ?"
        )
        .bind(&config.email)
        .bind(&config.username)
        .fetch_optional(&pool)
        .await
        .map_err(|e| format!("查询失败: {}", e))?;
        
        if let Some((user_id,)) = existing {
            // 更新现有配置
            println!("Updating existing config for user_id: {}", user_id);
            sqlx::query(
                "UPDATE filebay_configs 
                 SET url = ?, username = ?, repo_name = ?, email = ?, token = ?, updated_at = datetime('now')
                 WHERE user_id = ?"
            )
            .bind(&config.url)
            .bind(&config.username)
            .bind(&config.repo_name)
            .bind(&config.email)
            .bind(&config.token)
            .bind(&user_id)
            .execute(&pool)
            .await
            .map_err(|e| format!("更新配置失败: {}", e))?;
        } else {
            // 插入新配置
            let user_id = config.user_id.clone().unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
            println!("Inserting new config with user_id: {}", user_id);
            sqlx::query(
                "INSERT INTO filebay_configs (user_id, url, username, repo_name, email, token, updated_at)
                 VALUES (?, ?, ?, ?, ?, ?, datetime('now'))"
            )
            .bind(&user_id)
            .bind(&config.url)
            .bind(&config.username)
            .bind(&config.repo_name)
            .bind(&config.email)
            .bind(&config.token)
            .execute(&pool)
            .await
            .map_err(|e| format!("插入配置失败: {}", e))?;
        }
        
        pool.close().await;
        println!("✓ Vault database updated");
    } else {
        println!("⚠ Vault database does not exist, skipping");
    }
    
    // 2. 更新 Gitea 配置文件
    let gitea_config_path = get_gitea_config_path()?;
    let gitea_config = serde_json::json!({
        "url": config.url,
        "token": config.token,
        "owner": config.username,
        "repo": config.repo_name,
        "enabled": true
    });
    
    std::fs::write(&gitea_config_path, serde_json::to_string_pretty(&gitea_config).unwrap())
        .map_err(|e| format!("写入 Gitea 配置失败: {}", e))?;
    println!("✓ Gitea config updated: {:?}", gitea_config_path);
    
    // 3. 更新 FileBay 配置文件
    let filebay_config_path = get_filebay_config_path(&app)?;
    let filebay_config = serde_json::json!({
        "url": config.url,
        "username": config.username,
        "repoName": config.repo_name,
        "email": config.email,
        "token": config.token,
        "downloadedAt": chrono::Utc::now().to_rfc3339(),
        "version": "1.0.0"
    });
    
    // 确保目录存在
    if let Some(parent) = filebay_config_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("创建目录失败: {}", e))?;
    }
    
    std::fs::write(&filebay_config_path, serde_json::to_string_pretty(&filebay_config).unwrap())
        .map_err(|e| format!("写入 FileBay 配置失败: {}", e))?;
    println!("✓ FileBay config updated: {:?}", filebay_config_path);
    
    Ok(format!(
        "配置同步成功！\n\n用户: {} ({})\n仓库: {}\n服务器: {}",
        config.username, config.email, config.repo_name, config.url
    ))
}

fn get_vault_db_path() -> PathBuf {
    let home = dirs_next::home_dir().expect("Failed to get home directory");
    home.join(".cheersai").join("vault.db")
}

fn get_gitea_config_path() -> Result<PathBuf, String> {
    let temp_dir = std::env::temp_dir();
    let config_dir = temp_dir.join("cheersai-vault");
    std::fs::create_dir_all(&config_dir)
        .map_err(|e| format!("创建配置目录失败: {}", e))?;
    Ok(config_dir.join("gitea_config.json"))
}

fn get_filebay_config_path(app: &AppHandle) -> Result<PathBuf, String> {
    let app_data_dir = app.path().app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {}", e))?;
    
    let downloads_dir = app_data_dir.join("downloads");
    Ok(downloads_dir.join("filebay-config.json"))
}
