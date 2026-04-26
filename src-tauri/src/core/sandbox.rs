use std::path::PathBuf;
use anyhow::Result;

pub struct SandboxManager {
    pub sandbox_dir: PathBuf,
}

impl SandboxManager {
    pub fn init() -> Result<Self> {
        let app_data = get_cross_platform_app_data_dir()
            .join("sandbox");

        std::fs::create_dir_all(&app_data)?;

        Ok(SandboxManager {
            sandbox_dir: app_data,
        })
    }

    pub fn init_with_custom_dir(custom_dir: &str) -> Result<Self> {
        let sandbox_dir = PathBuf::from(custom_dir);
        std::fs::create_dir_all(&sandbox_dir)?;

        Ok(SandboxManager {
            sandbox_dir,
        })
    }

    pub fn list_files(&self) -> Result<Vec<PathBuf>> {
        let mut files = Vec::new();
        if self.sandbox_dir.exists() {
            for entry in std::fs::read_dir(&self.sandbox_dir)? {
                let entry = entry?;
                if entry.file_type()?.is_file() {
                    files.push(entry.path());
                }
            }
        }
        Ok(files)
    }

    pub fn get_file_path(&self, filename: &str) -> PathBuf {
        let safe_filename = sanitize_filename_cross_platform(filename);
        self.sandbox_dir.join(safe_filename)
    }
}

/// 获取跨平台应用数据目录
fn get_cross_platform_app_data_dir() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        dirs_next::data_dir()
            .unwrap_or_else(|| PathBuf::from(std::env::var("APPDATA").unwrap_or_else(|_| ".".to_string())))
            .join("CheersAI Vault")
    }
    
    #[cfg(target_os = "macos")]
    {
        dirs_next::data_dir()
            .unwrap_or_else(|| PathBuf::from("~/Library/Application Support"))
            .join("CheersAI Vault")
    }
    
    #[cfg(target_os = "linux")]
    {
        dirs_next::data_dir()
            .unwrap_or_else(|| PathBuf::from("~/.local/share"))
            .join("CheersAI Vault")
    }
    
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        PathBuf::from("./CheersAI Vault")
    }
}

/// 跨平台文件名清理
fn sanitize_filename_cross_platform(filename: &str) -> String {
    let mut sanitized = filename.to_string();
    
    // 移除或替换非法字符
    #[cfg(target_os = "windows")]
    {
        sanitized = sanitized
            .replace(['<', '>', ':', '"', '|', '?', '*'], "_")
            .replace(['/', '\\'], "_");
        
        // Windows 保留名称检查
        let reserved_names = [
            "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5",
            "COM6", "COM7", "COM8", "COM9", "LPT1", "LPT2", "LPT3", "LPT4",
            "LPT5", "LPT6", "LPT7", "LPT8", "LPT9"
        ];
        
        let base_name = sanitized.split('.').next().unwrap_or("").to_uppercase();
        if reserved_names.contains(&base_name.as_str()) {
            sanitized = format!("_{}", sanitized);
        }
    }
    
    #[cfg(not(target_os = "windows"))]
    {
        // Unix-like 系统
        sanitized = sanitized
            .replace(['/', '\0'], "_");
    }
    
    // 移除开头和结尾的空格和点
    sanitized = sanitized.trim_matches([' ', '.']).to_string();
    
    // 确保不为空
    if sanitized.is_empty() {
        sanitized = "untitled".to_string();
    }
    
    sanitized
}
