use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Serialize, Deserialize)]
pub struct SandboxFile {
    pub name: String,
    pub path: String,
    pub size: u64,
    pub modified: String,
}

use crate::core::dpapi;

/// 检查是否已设置 PIN（用于前端判断显示"设置PIN"还是"输入PIN"）
#[tauri::command]
pub async fn has_pin() -> Result<bool, String> {
    Ok(dpapi::has_pin())
}

/// 验证 PIN（使用 Windows DPAPI 解密后比对）
#[tauri::command]
pub async fn verify_pin(pin: String) -> Result<bool, String> {
    if !dpapi::has_pin() {
        return Err("尚未设置 PIN，请先设置".to_string());
    }
    dpapi::verify_pin(&pin)
}

/// 设置 PIN（使用 Windows DPAPI 加密后持久化存储）
#[tauri::command]
pub async fn set_pin(pin: String) -> Result<(), String> {
    if pin.len() < 4 {
        return Err("PIN 至少需要 4 位".to_string());
    }
    dpapi::save_pin(&pin)
}

/// 清除 PIN
#[tauri::command]
pub async fn clear_pin() -> Result<(), String> {
    dpapi::clear_pin()
}

#[tauri::command]
pub async fn list_sandbox_files() -> Result<Vec<SandboxFile>, String> {
    // 获取沙箱输出目录
    let temp_dir = std::env::temp_dir();
    let output_dir = temp_dir.join("cheersai-vault").join("output");
    
    // 如果目录不存在，创建它
    if !output_dir.exists() {
        std::fs::create_dir_all(&output_dir).map_err(|e| e.to_string())?;
        return Ok(vec![]);
    }
    
    let mut files = Vec::new();
    
    match std::fs::read_dir(&output_dir) {
        Ok(entries) => {
            for entry in entries {
                match entry {
                    Ok(entry) => {
                        let path = entry.path();
                        if path.is_file() {
                            let name = path.file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or("unknown")
                                .to_string();
                            
                            let size = match entry.metadata() {
                                Ok(metadata) => metadata.len(),
                                Err(_) => 0,
                            };
                            
                            let modified = match entry.metadata()
                                .and_then(|m| m.modified()) {
                                Ok(time) => {
                                    let datetime: chrono::DateTime<chrono::Utc> = time.into();
                                    datetime.to_rfc3339()
                                },
                                Err(_) => chrono::Utc::now().to_rfc3339(),
                            };
                            
                            files.push(SandboxFile {
                                name,
                                path: path.to_string_lossy().to_string(),
                                size,
                                modified,
                            });
                        }
                    },
                    Err(_) => continue,
                }
            }
        },
        Err(e) => return Err(format!("Failed to read directory: {}", e)),
    }
    
    // 按修改时间倒序排序（最新的在前面）
    files.sort_by(|a, b| b.modified.cmp(&a.modified));
    
    Ok(files)
}

#[tauri::command]
pub async fn list_files_in_directory(directory: String) -> Result<Vec<SandboxFile>, String> {
    let dir_path = Path::new(&directory);
    
    if !dir_path.exists() {
        return Ok(vec![]);
    }
    
    if !dir_path.is_dir() {
        return Err("Path is not a directory".to_string());
    }
    
    let mut files = Vec::new();
    
    match std::fs::read_dir(dir_path) {
        Ok(entries) => {
            for entry in entries {
                match entry {
                    Ok(entry) => {
                        let path = entry.path();
                        if path.is_file() {
                            let name = path.file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or("unknown")
                                .to_string();
                            
                            let size = match entry.metadata() {
                                Ok(metadata) => metadata.len(),
                                Err(_) => 0,
                            };
                            
                            let modified = match entry.metadata()
                                .and_then(|m| m.modified()) {
                                Ok(time) => {
                                    match time.duration_since(std::time::UNIX_EPOCH) {
                                        Ok(duration) => {
                                            let timestamp = duration.as_secs();
                                            format_timestamp(timestamp)
                                        },
                                        Err(_) => "Unknown".to_string(),
                                    }
                                },
                                Err(_) => "Unknown".to_string(),
                            };
                            
                            files.push(SandboxFile {
                                name,
                                path: path.to_string_lossy().to_string(),
                                size,
                                modified,
                            });
                        }
                    },
                    Err(_) => continue,
                }
            }
        },
        Err(e) => return Err(format!("Failed to read directory: {}", e)),
    }
    
    // 按名称排序
    files.sort_by(|a, b| a.name.cmp(&b.name));
    
    Ok(files)
}

#[tauri::command]
pub async fn export_sandbox(_file_name: String, _dest_path: String, _passphrase: String) -> Result<(), String> {
    // TODO: 实现文件导出功能
    Err("Export functionality not implemented yet".to_string())
}

#[tauri::command]
pub async fn import_sandbox(_src_path: String, _passphrase: String) -> Result<SandboxFile, String> {
    // TODO: 实现文件导入功能
    Err("Import functionality not implemented yet".to_string())
}

/// 删除沙箱文件
#[tauri::command]
pub async fn delete_sandbox_file(file_path: String) -> Result<String, String> {
    let path = Path::new(&file_path);
    
    if path.exists() {
        std::fs::remove_file(path).map_err(|e| format!("删除文件失败: {}", e))?;
        Ok("文件已删除".to_string())
    } else {
        Err("文件不存在".to_string())
    }
}

/// 批量删除沙箱文件
#[tauri::command]
pub async fn delete_sandbox_files(file_paths: Vec<String>) -> Result<String, String> {
    let mut deleted_count = 0;
    let mut errors = Vec::new();
    
    for file_path in file_paths {
        let path = Path::new(&file_path);
        
        if path.exists() {
            match std::fs::remove_file(path) {
                Ok(_) => deleted_count += 1,
                Err(e) => errors.push(format!("删除失败: {}", e)),
            }
        }
    }
    
    if errors.is_empty() {
        Ok(format!("成功删除 {} 个文件", deleted_count))
    } else {
        Ok(format!("删除了 {} 个文件，{} 个失败", deleted_count, errors.len()))
    }
}

/// 获取沙箱目录路径
#[tauri::command]
pub async fn get_sandbox_dir_path() -> Result<String, String> {
    let temp_dir = std::env::temp_dir();
    let output_dir = temp_dir.join("cheersai-vault").join("output");
    
    // 确保目录存在
    if !output_dir.exists() {
        std::fs::create_dir_all(&output_dir).map_err(|e| e.to_string())?;
    }
    
    Ok(output_dir.to_string_lossy().to_string())
}

/// 打开沙箱目录
#[tauri::command]
pub async fn open_sandbox_dir() -> Result<String, String> {
    let temp_dir = std::env::temp_dir();
    let output_dir = temp_dir.join("cheersai-vault").join("output");
    
    // 确保目录存在
    if !output_dir.exists() {
        std::fs::create_dir_all(&output_dir).map_err(|e| e.to_string())?;
    }
    
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(&output_dir)
            .spawn()
            .map_err(|e| format!("无法打开文件夹: {}", e))?;
    }
    
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(&output_dir)
            .spawn()
            .map_err(|e| format!("无法打开文件夹: {}", e))?;
    }
    
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(&output_dir)
            .spawn()
            .map_err(|e| format!("无法打开文件夹: {}", e))?;
    }
    
    Ok("已打开沙箱目录".to_string())
}

/// 清空沙箱目录
#[tauri::command]
pub async fn clear_sandbox_dir() -> Result<String, String> {
    let temp_dir = std::env::temp_dir();
    let output_dir = temp_dir.join("cheersai-vault").join("output");
    
    if output_dir.exists() {
        let entries = std::fs::read_dir(&output_dir).map_err(|e| e.to_string())?;
        let mut deleted_count = 0;
        
        for entry in entries {
            let entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path();
            
            if path.is_file() {
                std::fs::remove_file(path).map_err(|e| e.to_string())?;
                deleted_count += 1;
            }
        }
        
        Ok(format!("已清空沙箱目录，删除了 {} 个文件", deleted_count))
    } else {
        Ok("沙箱目录不存在".to_string())
    }
}

/// 锁定沙箱文件：将指定目录中的所有文件设为隐藏+系统属性
#[tauri::command]
pub async fn lock_sandbox_files(directory: String) -> Result<String, String> {
    let dir_path = Path::new(&directory);
    if !dir_path.exists() {
        return Ok("目录不存在".to_string());
    }

    let mut count = 0;
    #[cfg(target_os = "windows")]
    {
        if let Ok(entries) = std::fs::read_dir(dir_path) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    let _ = std::process::Command::new("attrib")
                        .args(["+h", "+s", &path.to_string_lossy()])
                        .output();
                    count += 1;
                }
            }
        }
    }

    Ok(format!("已隐藏 {} 个文件", count))
}

/// 解锁沙箱文件：将指定目录中的所有文件取消隐藏+系统属性
#[tauri::command]
pub async fn unlock_sandbox_files(directory: String) -> Result<String, String> {
    let dir_path = Path::new(&directory);
    if !dir_path.exists() {
        return Ok("目录不存在".to_string());
    }

    let mut count = 0;
    #[cfg(target_os = "windows")]
    {
        if let Ok(entries) = std::fs::read_dir(dir_path) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    let _ = std::process::Command::new("attrib")
                        .args(["-h", "-s", &path.to_string_lossy()])
                        .output();
                    count += 1;
                }
            }
        }
    }

    Ok(format!("已显示 {} 个文件", count))
}

/// 格式化时间戳为可读格式
fn format_timestamp(timestamp: u64) -> String {
    use std::time::UNIX_EPOCH;
    
    let system_time = UNIX_EPOCH + std::time::Duration::from_secs(timestamp);
    
    // 简单的时间格式化（实际应用中可能需要更复杂的格式化）
    match system_time.elapsed() {
        Ok(elapsed) => {
            let days = elapsed.as_secs() / 86400;
            if days > 0 {
                format!("{} 天前", days)
            } else {
                let hours = elapsed.as_secs() / 3600;
                if hours > 0 {
                    format!("{} 小时前", hours)
                } else {
                    let minutes = elapsed.as_secs() / 60;
                    if minutes > 0 {
                        format!("{} 分钟前", minutes)
                    } else {
                        "刚刚".to_string()
                    }
                }
            }
        },
        Err(_) => {
            // 时间戳在未来，直接显示日期
            "未来时间".to_string()
        }
    }
}
