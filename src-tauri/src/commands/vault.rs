/// Vault 集成 - 从 Vault Bridge 数据库读取 FileBay 配置
/// 
/// 功能:
/// 1. 读取 ~/.cheersai/vault.db 数据库
/// 2. 列出所有可用的 FileBay 配置
/// 3. 通过用户 ID 或邮箱获取配置

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct VaultFileBayConfig {
    pub user_id: String,
    pub url: String,
    pub username: String,
    pub repo_name: String,
    pub email: String,
    pub token: String,
    pub updated_at: String,
}

/// 获取 Vault 数据库路径
fn get_vault_db_path() -> PathBuf {
    let home = dirs_next::home_dir().expect("Failed to get home directory");
    home.join(".cheersai").join("vault.db")
}

/// 列出所有可用的 FileBay 配置
#[tauri::command]
pub async fn list_vault_configs() -> Result<Vec<VaultFileBayConfig>, String> {
    use crate::core::database::Database;

    // 首先尝试从本地数据库读取（Vault API Server 同步的配置）
    match Database::new().await {
        Ok(db) => {

            match db.get_setting("filebay_config").await {
                Ok(Some(config_json)) => {

                    // 解析配置
                    match serde_json::from_str::<serde_json::Value>(&config_json) {
                        Ok(config_value) => {

                            // 构建 VaultFileBayConfig
                            let config = VaultFileBayConfig {
                                user_id: config_value.get("user_id")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("auto_synced")
                                    .to_string(),
                                url: config_value.get("url")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string(),
                                username: config_value.get("username")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string(),
                                repo_name: config_value.get("repo_name")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string(),
                                email: config_value.get("email")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string(),
                                token: config_value.get("token")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string(),
                                updated_at: config_value.get("downloaded_at")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string(),
                            };

                            return Ok(vec![config]);
                        }
                        Err(e) => {

                        }
                    }
                }
                Ok(None) => {

                }
                Err(e) => {

                }
            }
        }
        Err(e) => {

        }
    }

    // 如果本地数据库没有配置，尝试从 Vault Bridge 数据库读取
    use sqlx::sqlite::SqlitePool;
    
    let db_path = get_vault_db_path();
    
    if !db_path.exists() {
        return Err(format!(
            "未找到任何配置\n\n请先在 Desktop 的 FileBay 设置页面配置，配置会自动同步到本地。\n\n或者在 Vault 系统中登录:\nhttp://localhost:3000/signin\n然后访问:\nhttp://localhost:3000/sync-config"
        ));
    }
    
    let db_url = format!("sqlite://{}", db_path.to_string_lossy());
    let pool = SqlitePool::connect(&db_url)
        .await
        .map_err(|e| format!("无法打开数据库: {}", e))?;
    
    let configs = sqlx::query_as::<_, (String, String, String, String, String, String, String)>(
        "SELECT user_id, url, username, repo_name, email, token, updated_at 
         FROM filebay_configs 
         ORDER BY updated_at DESC"
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| format!("查询失败: {}", e))?;
    
    pool.close().await;
    
    if configs.is_empty() {
        return Err(
            "数据库中没有配置\n\n请先在 Desktop 的 FileBay 设置页面配置，配置会自动同步到本地。\n\n或者在 Vault 系统中登录并同步配置:\nhttp://localhost:3000/sync-config".to_string()
        );
    }
    
    let result: Vec<VaultFileBayConfig> = configs
        .into_iter()
        .map(|(user_id, url, username, repo_name, email, token, updated_at)| {
            VaultFileBayConfig {
                user_id,
                url,
                username,
                repo_name,
                email,
                token,
                updated_at,
            }
        })
        .collect();
    
    Ok(result)
}

/// 通过用户 ID 获取配置
#[tauri::command]
pub async fn get_vault_config_by_user_id(user_id: String) -> Result<VaultFileBayConfig, String> {
    use sqlx::sqlite::SqlitePool;
    
    let db_path = get_vault_db_path();
    
    if !db_path.exists() {
        return Err("Vault 数据库不存在".to_string());
    }
    
    let db_url = format!("sqlite://{}", db_path.to_string_lossy());
    let pool = SqlitePool::connect(&db_url)
        .await
        .map_err(|e| format!("无法打开数据库: {}", e))?;
    
    let config = sqlx::query_as::<_, (String, String, String, String, String, String, String)>(
        "SELECT user_id, url, username, repo_name, email, token, updated_at 
         FROM filebay_configs 
         WHERE user_id = ?"
    )
    .bind(&user_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| format!("查询失败: {}", e))?;
    
    pool.close().await;
    
    match config {
        Some((user_id, url, username, repo_name, email, token, updated_at)) => {
            Ok(VaultFileBayConfig {
                user_id,
                url,
                username,
                repo_name,
                email,
                token,
                updated_at,
            })
        }
        None => Err(format!("未找到用户 ID 为 {} 的配置", user_id)),
    }
}

/// 通过邮箱获取配置
#[tauri::command]
pub async fn get_vault_config_by_email(email: String) -> Result<VaultFileBayConfig, String> {
    use sqlx::sqlite::SqlitePool;
    
    let db_path = get_vault_db_path();
    
    if !db_path.exists() {
        return Err("Vault 数据库不存在".to_string());
    }
    
    let db_url = format!("sqlite://{}", db_path.to_string_lossy());
    let pool = SqlitePool::connect(&db_url)
        .await
        .map_err(|e| format!("无法打开数据库: {}", e))?;
    
    let config = sqlx::query_as::<_, (String, String, String, String, String, String, String)>(
        "SELECT user_id, url, username, repo_name, email, token, updated_at 
         FROM filebay_configs 
         WHERE email = ?"
    )
    .bind(&email)
    .fetch_optional(&pool)
    .await
    .map_err(|e| format!("查询失败: {}", e))?;
    
    pool.close().await;
    
    match config {
        Some((user_id, url, username, repo_name, email, token, updated_at)) => {
            Ok(VaultFileBayConfig {
                user_id,
                url,
                username,
                repo_name,
                email,
                token,
                updated_at,
            })
        }
        None => Err(format!("未找到邮箱为 {} 的配置", email)),
    }
}

/// 检查 Vault 数据库是否存在
#[tauri::command]
pub async fn check_vault_db_exists() -> Result<bool, String> {
    let db_path = get_vault_db_path();
    Ok(db_path.exists())
}

/// 获取 Vault 数据库路径
#[tauri::command]
pub async fn get_vault_db_path_string() -> Result<String, String> {
    let db_path = get_vault_db_path();
    Ok(db_path.to_string_lossy().to_string())
}

/// 获取 Vault 数据库统计信息
#[tauri::command]
pub async fn get_vault_db_stats() -> Result<VaultDbStats, String> {
    use sqlx::sqlite::SqlitePool;
    
    let db_path = get_vault_db_path();
    
    if !db_path.exists() {
        return Ok(VaultDbStats {
            exists: false,
            path: db_path.to_string_lossy().to_string(),
            config_count: 0,
            last_updated: None,
        });
    }
    
    let db_url = format!("sqlite://{}", db_path.to_string_lossy());
    let pool = SqlitePool::connect(&db_url)
        .await
        .map_err(|e| format!("无法打开数据库: {}", e))?;
    
    // 获取配置数量
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM filebay_configs")
        .fetch_one(&pool)
        .await
        .map_err(|e| format!("查询失败: {}", e))?;
    
    // 获取最后更新时间
    let last_updated: Option<(String,)> = sqlx::query_as(
        "SELECT updated_at FROM filebay_configs ORDER BY updated_at DESC LIMIT 1"
    )
    .fetch_optional(&pool)
    .await
    .map_err(|e| format!("查询失败: {}", e))?;
    
    pool.close().await;
    
    Ok(VaultDbStats {
        exists: true,
        path: db_path.to_string_lossy().to_string(),
        config_count: count.0 as usize,
        last_updated: last_updated.map(|(time,)| time),
    })
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VaultDbStats {
    pub exists: bool,
    pub path: String,
    pub config_count: usize,
    pub last_updated: Option<String>,
}
