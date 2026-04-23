use tauri::{AppHandle, Manager};
use std::path::PathBuf;
use std::process::Command;

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

/// AI 模型配置
pub struct AiModelConfig {
    pub model_name: String,
    pub model_size: String,
    pub ollama_version: String,
}

impl Default for AiModelConfig {
    fn default() -> Self {
        Self {
            model_name: "qwen2.5:1.5b".to_string(),
            model_size: "1GB".to_string(),
            ollama_version: "0.1.26".to_string(), // 使用稳定版本
        }
    }
}

/// 获取 AI 模型安装目录
/// 路径: AppData/com.cheersai.vault/ai-models/
fn get_ai_model_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let app_data_dir = app.path().app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {}", e))?;
    
    let model_dir = app_data_dir.join("ai-models");
    Ok(model_dir)
}

/// 获取内置 Ollama 可执行文件路径
/// 路径: AppData/com.cheersai.vault/ollama/ollama.exe
fn get_bundled_ollama_path(app: &AppHandle) -> Result<PathBuf, String> {
    let app_data_dir = app.path().app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {}", e))?;
    
    let ollama_dir = app_data_dir.join("ollama");
    
    #[cfg(target_os = "windows")]
    let ollama_exe = ollama_dir.join("ollama.exe");
    
    #[cfg(not(target_os = "windows"))]
    let ollama_exe = ollama_dir.join("ollama");
    
    Ok(ollama_exe)
}

/// 获取 Ollama 可执行文件路径（优先使用内置版本）
fn get_ollama_path(app: &AppHandle) -> Result<PathBuf, String> {
    // 1. 优先使用内置的 Ollama
    let bundled_path = get_bundled_ollama_path(app)?;
    if bundled_path.exists() {
        return Ok(bundled_path);
    }
    
    // 2. 检查系统中是否安装了 Ollama
    #[cfg(target_os = "windows")]
    {
        // 2.1 使用 PowerShell Get-Command 查找（最可靠）
        if let Ok(output) = std::process::Command::new("powershell")
            .arg("-NoProfile")
            .arg("-Command")
            .arg("Get-Command ollama -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Source")
            .output()
        {
            if output.status.success() {
                let path_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !path_str.is_empty() {
                    let path = PathBuf::from(&path_str);
                    if path.exists() {
                        println!("Found Ollama using PowerShell Get-Command: {:?}", path);
                        return Ok(path);
                    }
                }
            }
        }
        
        // 2.2 尝试从 PATH 环境变量中查找
        if let Ok(path_var) = std::env::var("PATH") {
            for path_dir in path_var.split(';') {
                let ollama_exe = PathBuf::from(path_dir).join("ollama.exe");
                if ollama_exe.exists() {
                    println!("Found Ollama in PATH: {:?}", ollama_exe);
                    return Ok(ollama_exe);
                }
            }
        }
        
        // 2.3 尝试使用 where 命令查找
        if let Ok(output) = std::process::Command::new("where")
            .arg("ollama")
            .output()
        {
            if output.status.success() {
                let path_str = String::from_utf8_lossy(&output.stdout);
                let first_path = path_str.lines().next().unwrap_or("").trim();
                if !first_path.is_empty() {
                    let path = PathBuf::from(first_path);
                    if path.exists() {
                        println!("Found Ollama using 'where' command: {:?}", path);
                        return Ok(path);
                    }
                }
            }
        }
        
        // 2.4 检查常见安装路径
        let possible_paths = vec![
            PathBuf::from(std::env::var("LOCALAPPDATA").unwrap_or_default()).join("Programs\\Ollama\\ollama.exe"),
            PathBuf::from(std::env::var("PROGRAMFILES").unwrap_or_default()).join("Ollama\\ollama.exe"),
            PathBuf::from(std::env::var("PROGRAMFILES(X86)").unwrap_or_default()).join("Ollama\\ollama.exe"),
            PathBuf::from(std::env::var("APPDATA").unwrap_or_default()).join("Ollama\\ollama.exe"),
            PathBuf::from(std::env::var("USERPROFILE").unwrap_or_default()).join("AppData\\Local\\Programs\\Ollama\\ollama.exe"),
        ];
        
        for path in possible_paths {
            if path.exists() {
                println!("Found Ollama in common path: {:?}", path);
                return Ok(path);
            }
        }
        
        // 2.5 扫描所有盘符的常见安装位置
        let drives = vec!["C:", "D:", "E:", "F:", "G:"];
        let common_locations = vec![
            "\\Program Files\\Ollama\\ollama.exe",
            "\\Program Files (x86)\\Ollama\\ollama.exe",
            "\\Ollama\\ollama.exe",
            "\\Tools\\Ollama\\ollama.exe",
            "\\Software\\Ollama\\ollama.exe",
        ];
        
        for drive in drives {
            for location in &common_locations {
                let path = PathBuf::from(format!("{}{}", drive, location));
                if path.exists() {
                    println!("Found Ollama by scanning drives: {:?}", path);
                    return Ok(path);
                }
            }
        }
    }
    
    #[cfg(target_os = "macos")]
    {
        // 尝试从 PATH 环境变量中查找
        if let Ok(path_var) = std::env::var("PATH") {
            for path_dir in path_var.split(':') {
                let ollama_exe = PathBuf::from(path_dir).join("ollama");
                if ollama_exe.exists() {
                    return Ok(ollama_exe);
                }
            }
        }
        
        // 检查常见路径
        let possible_paths = vec![
            PathBuf::from("/usr/local/bin/ollama"),
            PathBuf::from("/opt/homebrew/bin/ollama"),
            PathBuf::from("/usr/bin/ollama"),
        ];
        
        for path in possible_paths {
            if path.exists() {
                return Ok(path);
            }
        }
        
        // 尝试使用 which 命令查找
        if let Ok(output) = std::process::Command::new("which")
            .arg("ollama")
            .output()
        {
            if output.status.success() {
                let path_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !path_str.is_empty() {
                    let path = PathBuf::from(path_str);
                    if path.exists() {
                        return Ok(path);
                    }
                }
            }
        }
    }
    
    #[cfg(target_os = "linux")]
    {
        // 尝试从 PATH 环境变量中查找
        if let Ok(path_var) = std::env::var("PATH") {
            for path_dir in path_var.split(':') {
                let ollama_exe = PathBuf::from(path_dir).join("ollama");
                if ollama_exe.exists() {
                    return Ok(ollama_exe);
                }
            }
        }
        
        // 检查常见路径
        let possible_paths = vec![
            PathBuf::from("/usr/local/bin/ollama"),
            PathBuf::from("/usr/bin/ollama"),
            PathBuf::from("/bin/ollama"),
        ];
        
        for path in possible_paths {
            if path.exists() {
                return Ok(path);
            }
        }
        
        // 尝试使用 which 命令查找
        if let Ok(output) = std::process::Command::new("which")
            .arg("ollama")
            .output()
        {
            if output.status.success() {
                let path_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !path_str.is_empty() {
                    let path = PathBuf::from(path_str);
                    if path.exists() {
                        return Ok(path);
                    }
                }
            }
        }
    }
    
    Err("Ollama 未安装".to_string())
}

/// 下载 Ollama
#[tauri::command]
pub async fn download_ollama(app: AppHandle) -> Result<String, String> {
    let app_data_dir = app.path().app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {}", e))?;
    
    let ollama_dir = app_data_dir.join("ollama");
    std::fs::create_dir_all(&ollama_dir)
        .map_err(|e| format!("Failed to create ollama directory: {}", e))?;
    
    #[cfg(target_os = "windows")]
    {
        let ollama_url = "https://github.com/ollama/ollama/releases/download/v0.1.26/ollama-windows-amd64.zip";
        let zip_path = ollama_dir.join("ollama.zip");
        let ollama_exe = ollama_dir.join("ollama.exe");
        
        println!("Downloading Ollama from: {}", ollama_url);
        
        // 使用 reqwest 下载文件
        let client = reqwest::Client::new();
        let response = client.get(ollama_url)
            .send()
            .await
            .map_err(|e| format!("Failed to download Ollama: {}", e))?;
        
        let bytes = response.bytes()
            .await
            .map_err(|e| format!("Failed to read response: {}", e))?;
        
        std::fs::write(&zip_path, &bytes)
            .map_err(|e| format!("Failed to write zip file: {}", e))?;
        
        // 解压文件
        let file = std::fs::File::open(&zip_path)
            .map_err(|e| format!("Failed to open zip file: {}", e))?;
        
        let mut archive = zip::ZipArchive::new(file)
            .map_err(|e| format!("Failed to read zip archive: {}", e))?;
        
        archive.extract(&ollama_dir)
            .map_err(|e| format!("Failed to extract zip: {}", e))?;
        
        // 删除 zip 文件
        let _ = std::fs::remove_file(&zip_path);
        
        if !ollama_exe.exists() {
            return Err("Ollama 可执行文件未找到".to_string());
        }
        
        Ok(format!("Ollama 已安装到: {:?}", ollama_exe))
    }
    
    #[cfg(not(target_os = "windows"))]
    {
        Err("当前平台暂不支持自动安装 Ollama，请手动安装: https://ollama.com/download".to_string())
    }
}

/// 检查 Ollama 是否已安装
#[tauri::command]
pub async fn check_ollama_installed(app: AppHandle) -> Result<bool, String> {
    println!("=== Checking Ollama installation ===");
    match get_ollama_path(&app) {
        Ok(path) => {
            println!("✓ Ollama found at: {:?}", path);
            Ok(true)
        },
        Err(e) => {
            println!("✗ Ollama not found: {}", e);
            Ok(false)
        },
    }
}

/// 启动 Ollama 服务
#[tauri::command]
pub async fn start_ollama_service(app: AppHandle) -> Result<String, String> {
    println!("=== Starting Ollama service ===");
    
    let ollama_path = get_ollama_path(&app)?;
    println!("Using Ollama at: {:?}", ollama_path);
    
    // 启动 Ollama 服务（后台运行）
    #[cfg(target_os = "windows")]
    {
        match std::process::Command::new(&ollama_path)
            .arg("serve")
            .creation_flags(0x08000000) // CREATE_NO_WINDOW
            .spawn()
        {
            Ok(_) => {
                println!("✓ Ollama service started");
                Ok("Ollama 服务已启动".to_string())
            },
            Err(e) => {
                println!("✗ Failed to start Ollama: {}", e);
                Err(format!("启动 Ollama 失败: {}", e))
            }
        }
    }
    
    #[cfg(not(target_os = "windows"))]
    {
        match std::process::Command::new(&ollama_path)
            .arg("serve")
            .spawn()
        {
            Ok(_) => {
                println!("✓ Ollama service started");
                Ok("Ollama 服务已启动".to_string())
            },
            Err(e) => {
                println!("✗ Failed to start Ollama: {}", e);
                Err(format!("启动 Ollama 失败: {}", e))
            }
        }
    }
}

/// 检查 AI 模型是否已安装
#[tauri::command]
pub async fn check_ai_model_installed(app: AppHandle) -> Result<bool, String> {
    let config = AiModelConfig::default();
    
    println!("=== Checking AI model installation ===");
    
    // 检查 Ollama 是否安装
    let ollama_path = match get_ollama_path(&app) {
        Ok(path) => {
            println!("✓ Ollama found, checking for model: {}", config.model_name);
            path
        },
        Err(e) => {
            println!("✗ Ollama not found: {}", e);
            return Ok(false);
        }
    };
    
    // 使用 ollama list 命令检查模型是否存在
    let output = match Command::new(&ollama_path)
        .arg("list")
        .output()
    {
        Ok(output) => output,
        Err(e) => {
            println!("✗ Failed to execute ollama list: {}", e);
            return Ok(false);
        }
    };
    
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        println!("✗ ollama list command failed: {}", stderr);
        
        // 检查是否是因为服务未运行
        if stderr.contains("connect") || stderr.contains("refused") || stderr.contains("dial") {
            println!("  Reason: Ollama service is not running");
        }
        
        return Ok(false);
    }
    
    let output_str = String::from_utf8_lossy(&output.stdout);
    let found = output_str.contains(&config.model_name);
    
    if found {
        println!("✓ AI model {} is installed", config.model_name);
    } else {
        println!("✗ AI model {} not found", config.model_name);
        println!("  Available models:\n{}", output_str);
    }
    
    Ok(found)
}

/// 安装 AI 模型
#[tauri::command]
pub async fn install_ai_model(app: AppHandle) -> Result<String, String> {
    let config = AiModelConfig::default();
    
    // 检查 Ollama 是否安装
    let ollama_path = get_ollama_path(&app)?;
    
    // 确保模型目录存在
    let model_dir = get_ai_model_dir(&app)?;
    std::fs::create_dir_all(&model_dir)
        .map_err(|e| format!("Failed to create model directory: {}", e))?;
    
    println!("Installing AI model: {}", config.model_name);
    println!("Model directory: {:?}", model_dir);
    
    // 设置 OLLAMA_MODELS 环境变量，指定模型存储位置
    let output = Command::new(ollama_path)
        .arg("pull")
        .arg(&config.model_name)
        .env("OLLAMA_MODELS", &model_dir)
        .output()
        .map_err(|e| format!("Failed to pull model: {}", e))?;
    
    if !output.status.success() {
        let error_msg = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Failed to install model: {}", error_msg));
    }
    
    Ok(format!("AI 模型 {} 安装成功", config.model_name))
}

/// 卸载 AI 模型
#[tauri::command]
pub async fn uninstall_ai_model(app: AppHandle) -> Result<String, String> {
    let config = AiModelConfig::default();
    
    // 检查 Ollama 是否安装
    let ollama_path = get_ollama_path(&app)?;
    
    println!("Uninstalling AI model: {}", config.model_name);
    
    // 使用 ollama rm 命令删除模型
    let output = Command::new(ollama_path)
        .arg("rm")
        .arg(&config.model_name)
        .output()
        .map_err(|e| format!("Failed to remove model: {}", e))?;
    
    if !output.status.success() {
        let error_msg = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Failed to uninstall model: {}", error_msg));
    }
    
    Ok(format!("AI 模型 {} 已卸载", config.model_name))
}

/// 调用 AI 模型进行推理
#[tauri::command]
pub async fn call_ai_model(app: AppHandle, prompt: String) -> Result<String, String> {
    let config = AiModelConfig::default();
    
    // 检查 Ollama 是否安装
    let ollama_path = get_ollama_path(&app)?;
    
    println!("Calling AI model with prompt: {}", prompt);
    
    // 使用 ollama run 命令调用模型
    let output = Command::new(ollama_path)
        .arg("run")
        .arg(&config.model_name)
        .arg(&prompt)
        .output()
        .map_err(|e| format!("Failed to run model: {}", e))?;
    
    if !output.status.success() {
        let error_msg = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Model inference failed: {}", error_msg));
    }
    
    let result = String::from_utf8_lossy(&output.stdout).to_string();
    Ok(result)
}

/// 获取 AI 模型信息
#[tauri::command]
pub async fn get_ai_model_info(app: AppHandle) -> Result<serde_json::Value, String> {
    let config = AiModelConfig::default();
    let model_dir = get_ai_model_dir(&app)?;
    let ollama_installed = check_ollama_installed(app.clone()).await?;
    let model_installed = if ollama_installed {
        check_ai_model_installed(app.clone()).await.unwrap_or(false)
    } else {
        false
    };
    
    let bundled_ollama = get_bundled_ollama_path(&app)?;
    let using_bundled = bundled_ollama.exists();
    
    Ok(serde_json::json!({
        "model_name": config.model_name,
        "model_size": config.model_size,
        "model_dir": model_dir.to_string_lossy(),
        "ollama_installed": ollama_installed,
        "model_installed": model_installed,
        "using_bundled_ollama": using_bundled,
        "ollama_path": if ollama_installed {
            get_ollama_path(&app).ok().map(|p| p.to_string_lossy().to_string())
        } else {
            None
        },
    }))
}
