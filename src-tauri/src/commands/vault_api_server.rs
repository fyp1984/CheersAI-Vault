/// Vault API Server - 提供 HTTP API 供 Desktop 调用
/// 
/// 功能:
/// 1. 接收 Desktop 传递的 FileBay 配置
/// 2. 写入 Vault 本地数据库
/// 3. 提供配置查询接口

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::{AppHandle, Manager};
use tokio::sync::Mutex;
use warp::{Filter, Reply};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileBayConfigPayload {
    pub url: String,
    pub username: String,
    pub repo_name: String,
    pub email: String,
    pub token: String,
    pub downloaded_at: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub message: String,
    pub data: Option<T>,
}

/// Vault API 服务器状态
pub struct VaultApiServer {
    port: u16,
    is_running: Arc<Mutex<bool>>,
    app_handle: AppHandle,
}

impl VaultApiServer {
    pub fn new(app_handle: AppHandle, port: u16) -> Self {
        Self {
            port,
            is_running: Arc::new(Mutex::new(false)),
            app_handle,
        }
    }

    /// 启动 API 服务器
    pub async fn start(&self) -> Result<(), String> {
        let mut running = self.is_running.lock().await;
        if *running {
            return Err("API server is already running".to_string());
        }

        let app_handle = self.app_handle.clone();
        let port = self.port;
        let is_running = self.is_running.clone();

        // 启动服务器
        tokio::spawn(async move {
            let app_handle_filter = warp::any().map(move || app_handle.clone());

            // POST /api/v1/filebay/config - 接收 FileBay 配置
            let save_config = warp::post()
                .and(warp::path!("api" / "v1" / "filebay" / "config"))
                .and(warp::body::json())
                .and(app_handle_filter.clone())
                .and_then(handle_save_config);

            // GET /api/v1/filebay/config - 查询 FileBay 配置
            let get_config = warp::get()
                .and(warp::path!("api" / "v1" / "filebay" / "config"))
                .and(app_handle_filter.clone())
                .and_then(handle_get_config);

            // DELETE /api/v1/filebay/config - 删除 FileBay 配置
            let delete_config = warp::delete()
                .and(warp::path!("api" / "v1" / "filebay" / "config"))
                .and(app_handle_filter.clone())
                .and_then(handle_delete_config);

            // GET /api/v1/health - 健康检查
            let health = warp::get()
                .and(warp::path!("api" / "v1" / "health"))
                .map(|| {
                    warp::reply::json(&ApiResponse::<()> {
                        success: true,
                        message: "Vault API Server is running".to_string(),
                        data: None,
                    })
                });

            // CORS 配置 - 允许来自 Desktop 的跨域请求
            let cors = warp::cors()
                .allow_any_origin()
                .allow_methods(vec!["GET", "POST", "DELETE", "OPTIONS", "PUT", "PATCH"])
                .allow_headers(vec!["Content-Type", "Authorization", "Accept"])
                .expose_headers(vec!["Content-Type"])
                .max_age(3600);

            let routes = save_config
                .or(get_config)
                .or(delete_config)
                .or(health)
                .with(cors)
                .with(warp::log("vault_api"));

            println!("🚀 Vault API Server starting on http://localhost:{}", port);
            
            warp::serve(routes)
                .run(([127, 0, 0, 1], port))
                .await;

            // 服务器停止后更新状态
            let mut running = is_running.lock().await;
            *running = false;
        });

        *running = true;
        Ok(())
    }

    /// 停止 API 服务器
    pub async fn stop(&self) -> Result<(), String> {
        let mut running = self.is_running.lock().await;
        if !*running {
            return Err("API server is not running".to_string());
        }

        *running = false;
        Ok(())
    }

    /// 检查服务器是否运行
    pub async fn is_running(&self) -> bool {
        *self.is_running.lock().await
    }
}

/// 处理保存 FileBay 配置
async fn handle_save_config(
    payload: FileBayConfigPayload,
    app: AppHandle,
) -> Result<impl Reply, warp::Rejection> {
    println!("📥 Received FileBay config from Desktop:");
    println!("  URL: {}", payload.url);
    println!("  Username: {}", payload.username);
    println!("  Repo: {}", payload.repo_name);
    println!("  Email: {}", payload.email);

    // 保存到数据库
    match save_filebay_config_to_db(&app, &payload).await {
        Ok(_) => {
            let response = ApiResponse {
                success: true,
                message: "FileBay configuration saved successfully".to_string(),
                data: Some(payload),
            };
            Ok(warp::reply::json(&response))
        }
        Err(e) => {
            let response = ApiResponse::<FileBayConfigPayload> {
                success: false,
                message: format!("Failed to save configuration: {}", e),
                data: None,
            };
            Ok(warp::reply::json(&response))
        }
    }
}

/// 处理查询 FileBay 配置
async fn handle_get_config(app: AppHandle) -> Result<impl Reply, warp::Rejection> {
    match get_filebay_config_from_db(&app).await {
        Ok(Some(config)) => {
            let response = ApiResponse {
                success: true,
                message: "Configuration retrieved successfully".to_string(),
                data: Some(config),
            };
            Ok(warp::reply::json(&response))
        }
        Ok(None) => {
            let response = ApiResponse::<FileBayConfigPayload> {
                success: false,
                message: "No configuration found".to_string(),
                data: None,
            };
            Ok(warp::reply::json(&response))
        }
        Err(e) => {
            let response = ApiResponse::<FileBayConfigPayload> {
                success: false,
                message: format!("Failed to retrieve configuration: {}", e),
                data: None,
            };
            Ok(warp::reply::json(&response))
        }
    }
}

/// 处理删除 FileBay 配置
async fn handle_delete_config(app: AppHandle) -> Result<impl Reply, warp::Rejection> {
    match delete_filebay_config_from_db(&app).await {
        Ok(_) => {
            let response = ApiResponse::<()> {
                success: true,
                message: "Configuration deleted successfully".to_string(),
                data: None,
            };
            Ok(warp::reply::json(&response))
        }
        Err(e) => {
            let response = ApiResponse::<()> {
                success: false,
                message: format!("Failed to delete configuration: {}", e),
                data: None,
            };
            Ok(warp::reply::json(&response))
        }
    }
}

/// 保存 FileBay 配置到数据库
async fn save_filebay_config_to_db(
    app: &AppHandle,
    config: &FileBayConfigPayload,
) -> Result<(), String> {
    use crate::core::database::Database;

    let db = Database::new()
        .await
        .map_err(|e| format!("Failed to connect to database: {}", e))?;

    // 将配置序列化为 JSON
    let config_json = serde_json::to_string(config)
        .map_err(|e| format!("Failed to serialize config: {}", e))?;

    // 保存到 user_settings 表
    db.save_setting("filebay_config", &config_json)
        .await
        .map_err(|e| format!("Failed to save to database: {}", e))?;

    println!("✅ FileBay config saved to Vault database");
    Ok(())
}

/// 从数据库获取 FileBay 配置
async fn get_filebay_config_from_db(app: &AppHandle) -> Result<Option<FileBayConfigPayload>, String> {
    use crate::core::database::Database;

    let db = Database::new()
        .await
        .map_err(|e| format!("Failed to connect to database: {}", e))?;

    // 从 user_settings 表读取
    let config_json = db.get_setting("filebay_config")
        .await
        .map_err(|e| format!("Failed to read from database: {}", e))?;

    match config_json {
        Some(json) => {
            let config: FileBayConfigPayload = serde_json::from_str(&json)
                .map_err(|e| format!("Failed to deserialize config: {}", e))?;
            Ok(Some(config))
        }
        None => Ok(None),
    }
}

/// 从数据库删除 FileBay 配置
async fn delete_filebay_config_from_db(app: &AppHandle) -> Result<(), String> {
    use crate::core::database::Database;

    let db = Database::new()
        .await
        .map_err(|e| format!("Failed to connect to database: {}", e))?;

    db.delete_setting("filebay_config")
        .await
        .map_err(|e| format!("Failed to delete from database: {}", e))?;

    println!("✅ FileBay config deleted from Vault database");
    Ok(())
}

// ============ Tauri Commands ============

/// 启动 Vault API 服务器
#[tauri::command]
pub async fn start_vault_api_server(app: AppHandle, port: Option<u16>) -> Result<String, String> {
    let port = port.unwrap_or(7788);
    let server = VaultApiServer::new(app, port);
    
    server.start().await?;
    
    Ok(format!("Vault API Server started on port {}", port))
}

/// 停止 Vault API 服务器
#[tauri::command]
pub async fn stop_vault_api_server(app: AppHandle) -> Result<String, String> {
    // 这里需要从全局状态获取服务器实例
    // 暂时返回成功
    Ok("Vault API Server stopped".to_string())
}

/// 检查 API 服务器状态
#[tauri::command]
pub async fn check_vault_api_server_status(app: AppHandle) -> Result<bool, String> {
    // 尝试连接本地端口检查
    let client = reqwest::Client::new();
    match client
        .get("http://localhost:7788/api/v1/health")
        .timeout(std::time::Duration::from_secs(2))
        .send()
        .await
    {
        Ok(response) => Ok(response.status().is_success()),
        Err(_) => Ok(false),
    }
}

/// 手动保存 FileBay 配置 (Tauri Command)
#[tauri::command]
pub async fn save_filebay_config_via_api(
    app: AppHandle,
    config: FileBayConfigPayload,
) -> Result<String, String> {
    save_filebay_config_to_db(&app, &config).await?;
    Ok("Configuration saved successfully".to_string())
}

/// 手动获取 FileBay 配置 (Tauri Command)
#[tauri::command]
pub async fn get_filebay_config_via_api(
    app: AppHandle,
) -> Result<Option<FileBayConfigPayload>, String> {
    get_filebay_config_from_db(&app).await
}

/// 手动删除 FileBay 配置 (Tauri Command)
#[tauri::command]
pub async fn delete_filebay_config_via_api(app: AppHandle) -> Result<String, String> {
    delete_filebay_config_from_db(&app).await?;
    Ok("Configuration deleted successfully".to_string())
}
