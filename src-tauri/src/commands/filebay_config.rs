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
    pub token: String,
    #[serde(rename = "downloadedAt")]
    pub downloaded_at: String,
    pub version: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FileBayConfigStatus {
    pub exists: bool,
    pub config: Option<FileBayConfig>,
    pub file_path: Option<String>,
    pub last_modified: Option<String>,
}

/// 获取 downloads 文件夹中的 FileBay 配置文件路径
fn get_filebay_config_path_from_downloads(app: &AppHandle) -> Result<PathBuf, String> {
    let app_data_dir = app.path().app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {}", e))?;
    
    let downloads_dir = app_data_dir.join("downloads");
    Ok(downloads_dir.join("filebay-config.json"))
}

/// 获取沙箱中的 FileBay 配置文件路径（已弃用，保留用于兼容）
fn get_filebay_config_path() -> Result<PathBuf, String> {
    use crate::core::sandbox::SandboxManager;
    
    let sandbox = SandboxManager::init()
        .map_err(|e| format!("Failed to initialize sandbox: {}", e))?;
    
    Ok(sandbox.get_file_path("filebay-config.json"))
}

/// 读取 FileBay 配置文件（优先从 downloads 文件夹读取，其次从浏览器 Downloads 文件夹）
#[tauri::command]
pub async fn read_filebay_config(app: AppHandle) -> Result<FileBayConfigStatus, String> {
    // 优先从 app downloads 文件夹读取
    let config_path = get_filebay_config_path_from_downloads(&app)?;
    
    // 如果 app downloads 文件夹中不存在，尝试从浏览器 Downloads 文件夹读取
    let final_path = if !config_path.exists() {
        // 尝试从浏览器 Downloads 文件夹读取
        let user_downloads = dirs_next::download_dir()
            .ok_or_else(|| "无法获取用户 Downloads 文件夹".to_string())?;
        let browser_config_path = user_downloads.join("filebay-config.json");
        
        if browser_config_path.exists() {

            browser_config_path
        } else {
            return Ok(FileBayConfigStatus {
                exists: false,
                config: None,
                file_path: None,
                last_modified: None,
            });
        }
    } else {
        config_path
    };
    
    // 读取文件内容
    let content = std::fs::read_to_string(&final_path)
        .map_err(|e| format!("Failed to read config file: {}", e))?;
    
    // 解析 JSON
    let config: FileBayConfig = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse config JSON: {}", e))?;
    
    // 获取文件修改时间
    let metadata = std::fs::metadata(&final_path)
        .map_err(|e| format!("Failed to get file metadata: {}", e))?;
    
    let last_modified = metadata.modified()
        .map(|time| {
            use std::time::UNIX_EPOCH;
            let duration = time.duration_since(UNIX_EPOCH).unwrap_or_default();
            chrono::DateTime::from_timestamp(duration.as_secs() as i64, 0)
                .unwrap_or_default()
                .format("%Y-%m-%d %H:%M:%S")
                .to_string()
        })
        .unwrap_or_default();
    
    Ok(FileBayConfigStatus {
        exists: true,
        config: Some(config),
        file_path: Some(final_path.to_string_lossy().to_string()),
        last_modified: Some(last_modified),
    })
}

/// 检查 FileBay 配置文件是否存在（从 downloads 文件夹检查）
#[tauri::command]
pub async fn check_filebay_config_exists(app: AppHandle) -> Result<bool, String> {
    let config_path = get_filebay_config_path_from_downloads(&app)?;
    Ok(config_path.exists())
}

/// 删除 FileBay 配置文件（从 downloads 文件夹删除）
#[tauri::command]
pub async fn delete_filebay_config(app: AppHandle) -> Result<String, String> {
    let config_path = get_filebay_config_path_from_downloads(&app)?;
    
    if !config_path.exists() {
        return Err("配置文件不存在".to_string());
    }
    
    std::fs::remove_file(&config_path)
        .map_err(|e| format!("删除配置文件失败: {}", e))?;
    
    Ok("FileBay 配置文件已删除".to_string())
}

/// 验证 FileBay 配置文件格式
#[tauri::command]
pub async fn validate_filebay_config_file(file_path: String) -> Result<FileBayConfig, String> {


    let path = PathBuf::from(&file_path);
    
    if !path.exists() {

        return Err("文件不存在".to_string());
    }
    
    // 检查文件扩展名
    if let Some(extension) = path.extension() {
        if extension != "json" {

            return Err("文件必须是 JSON 格式 (.json)".to_string());
        }
    } else {

        return Err("文件必须有 .json 扩展名".to_string());
    }
    
    // 读取并解析文件
    let content = std::fs::read_to_string(&path)
        .map_err(|e| format!("读取文件失败: {}", e))?;
    

    let config: FileBayConfig = serde_json::from_str(&content)
        .map_err(|e| {

            format!("JSON 格式错误: {}", e)
        })?;
    
    // 验证必需字段
    if config.url.is_empty() {

        return Err("配置文件缺少 URL 字段".to_string());
    }
    
    if config.username.is_empty() {

        return Err("配置文件缺少用户名字段".to_string());
    }
    
    if config.repo_name.is_empty() {

        return Err("配置文件缺少仓库名字段".to_string());
    }




    Ok(config)
}

/// 导入 FileBay 配置文件到 downloads 文件夹
#[tauri::command]
pub async fn import_filebay_config(app: AppHandle, source_path: String) -> Result<String, String> {
    // 首先验证文件
    let config = validate_filebay_config_file(source_path.clone()).await?;
    
    // 获取目标路径（downloads 文件夹）
    let target_path = get_filebay_config_path_from_downloads(&app)?;
    
    // 确保 downloads 目录存在
    if let Some(parent) = target_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("创建 downloads 目录失败: {}", e))?;
    }
    
    // 复制文件到 downloads 文件夹
    std::fs::copy(&source_path, &target_path)
        .map_err(|e| format!("复制文件到 downloads 文件夹失败: {}", e))?;
    
    Ok(format!(
        "FileBay 配置已导入成功\n服务器: {}\n用户: {}\n仓库: {}\n邮箱: {}",
        config.url,
        config.username,
        config.repo_name,
        config.email
    ))
}
