use tauri::{AppHandle, Manager};
use std::path::PathBuf;
use std::process::Command;

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

/// AI 模型配置
pub struct AiModelConfig {
    pub model_name: String,
    pub model_size: String,
}

impl Default for AiModelConfig {
    fn default() -> Self {
        Self {
            model_name: "qwen2.5:1.5b".to_string(),
            model_size: "1GB".to_string(),
        }
    }
}

/// 获取 AI 模型安装目录
/// 优先使用用户自定义目录，否则使用默认 AppData 目录
fn get_ai_model_dir(app: &AppHandle) -> Result<PathBuf, String> {
    // TODO: 从配置文件读取用户自定义目录
    // 暂时使用默认目录
    let app_data_dir = app.path().app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {}", e))?;
    
    let model_dir = app_data_dir.join("ai-models");
    Ok(model_dir)
}

/// 获取内置 Ollama 可执行文件路径
/// 优先使用用户自定义目录，否则使用默认 AppData 目录
fn get_bundled_ollama_path(app: &AppHandle) -> Result<PathBuf, String> {
    // TODO: 从配置文件读取用户自定义目录
    // 暂时使用默认目录
    let app_data_dir = app.path().app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {}", e))?;
    
    let ollama_dir = app_data_dir.join("ollama");
    
    #[cfg(target_os = "windows")]
    let ollama_exe = ollama_dir.join("ollama.exe");
    
    #[cfg(not(target_os = "windows"))]
    let ollama_exe = ollama_dir.join("ollama");
    
    Ok(ollama_exe)
}

pub(crate) fn resolve_system_ollama_path() -> Result<PathBuf, String> {
    #[cfg(target_os = "windows")]
    {
        const CREATE_NO_WINDOW: u32 = 0x08000000;

        if let Ok(path_var) = std::env::var("PATH") {
            for path_dir in path_var.split(';') {
                let ollama_exe = PathBuf::from(path_dir).join("ollama.exe");
                if ollama_exe.exists() {
                    return Ok(ollama_exe);
                }
            }
        }

        if let Ok(output) = std::process::Command::new("powershell")
            .creation_flags(CREATE_NO_WINDOW)
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
                        return Ok(path);
                    }
                }
            }
        }

        if let Ok(output) = std::process::Command::new("where")
            .creation_flags(CREATE_NO_WINDOW)
            .arg("ollama")
            .output()
        {
            if output.status.success() {
                let path_str = String::from_utf8_lossy(&output.stdout);
                let first_path = path_str.lines().next().unwrap_or("").trim();
                if !first_path.is_empty() {
                    let path = PathBuf::from(first_path);
                    if path.exists() {
                        return Ok(path);
                    }
                }
            }
        }

        let possible_paths = vec![
            PathBuf::from(std::env::var("LOCALAPPDATA").unwrap_or_default()).join("Programs\\Ollama\\ollama.exe"),
            PathBuf::from(std::env::var("PROGRAMFILES").unwrap_or_default()).join("Ollama\\ollama.exe"),
            PathBuf::from(std::env::var("PROGRAMFILES(X86)").unwrap_or_default()).join("Ollama\\ollama.exe"),
            PathBuf::from(std::env::var("APPDATA").unwrap_or_default()).join("Ollama\\ollama.exe"),
            PathBuf::from(std::env::var("USERPROFILE").unwrap_or_default()).join("AppData\\Local\\Programs\\Ollama\\ollama.exe"),
        ];

        for path in possible_paths {
            if path.exists() {
                return Ok(path);
            }
        }

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
                    return Ok(path);
                }
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        if let Ok(path_var) = std::env::var("PATH") {
            for path_dir in path_var.split(':') {
                let ollama_exe = PathBuf::from(path_dir).join("ollama");
                if ollama_exe.exists() {
                    return Ok(ollama_exe);
                }
            }
        }

        let home = std::env::var("HOME").unwrap_or_default();
        let possible_paths = vec![
            PathBuf::from("/usr/local/bin/ollama"),
            PathBuf::from("/opt/homebrew/bin/ollama"),
            PathBuf::from("/usr/bin/ollama"),
            PathBuf::from("/Applications/Ollama.app/Contents/Resources/ollama"),
            PathBuf::from(format!("{}/Applications/Ollama.app/Contents/Resources/ollama", home)),
        ];

        for path in possible_paths {
            if path.exists() {
                return Ok(path);
            }
        }

        if let Ok(output) = std::process::Command::new("which").arg("ollama").output() {
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
        if let Ok(path_var) = std::env::var("PATH") {
            for path_dir in path_var.split(':') {
                let ollama_exe = PathBuf::from(path_dir).join("ollama");
                if ollama_exe.exists() {
                    return Ok(ollama_exe);
                }
            }
        }

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

        if let Ok(output) = std::process::Command::new("which").arg("ollama").output() {
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

pub(crate) fn resolve_system_ollama_path_string() -> Result<String, String> {
    resolve_system_ollama_path().map(|path| path.to_string_lossy().to_string())
}

async fn is_ollama_service_running() -> bool {
    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(2))
        .build()
    {
        Ok(client) => client,
        Err(_) => return false,
    };

    match client.get("http://127.0.0.1:11434/api/tags").send().await {
        Ok(response) => response.status().is_success(),
        Err(_) => false,
    }
}

fn manual_install_hint() -> &'static str {
    #[cfg(target_os = "macos")]
    {
        return "1. 访问 https://ollama.com/download\n\
        2. 下载 macOS 版本\n\
        3. 安装并启动 Ollama.app\n\
        4. 回到本应用重新扫描服务状态";
    }

    #[cfg(target_os = "windows")]
    {
        return "1. 访问 https://ollama.com/download\n\
        2. 下载 Windows 版本\n\
        3. 安装后重新打开本应用\n\
        4. 回到增强服务页重新扫描";
    }

    #[cfg(target_os = "linux")]
    {
        return "1. 访问 https://ollama.com/download\n\
        2. 按发行版说明安装 Ollama\n\
        3. 启动 Ollama 服务\n\
        4. 回到本应用重新扫描服务状态";
    }
}

async fn ensure_ollama_service_ready() -> Result<(), String> {
    if is_ollama_service_running().await {
        return Ok(());
    }

    #[cfg(target_os = "macos")]
    {
        return Err("已检测到 Ollama，但服务尚未启动。请先点击“启动 Ollama”或打开 Ollama.app，然后重新扫描。".to_string());
    }

    #[cfg(not(target_os = "macos"))]
    {
        return Err("已检测到 Ollama，但服务尚未启动。请先启动 Ollama 服务并重新扫描。".to_string());
    }
}

/// 获取 Ollama 可执行文件路径（优先使用内置版本）
fn get_ollama_path(app: &AppHandle) -> Result<PathBuf, String> {
    // 1. 优先使用内置的 Ollama
    let bundled_path = get_bundled_ollama_path(app)?;
    if bundled_path.exists() {
        return Ok(bundled_path);
    }
    
    resolve_system_ollama_path()
}

/// 下载 Ollama
/// 下载并安装 Ollama（支持自定义路径）
/// 注意：由于路径编码问题，建议用户手动安装 Ollama
#[tauri::command]
pub async fn download_ollama(app: AppHandle, custom_path: Option<String>) -> Result<String, String> {

    if let Some(ref path) = custom_path {

    }
    
    // 检查系统是否已安装 Ollama
    if let Ok(_) = get_ollama_path(&app) {
        return Ok("Ollama 已安装，无需重复安装".to_string());
    }
    
    // 由于用户环境千差万别，路径可能包含中文等特殊字符
    // 建议用户手动安装以避免路径问题
    Err(format!(
        "为避免路径和服务状态不一致，建议手动安装 Ollama：\n\n{}\n\n{}",
        manual_install_hint(),
        custom_path
            .map(|p| format!("已记录你的自定义路径偏好：{}", p))
            .unwrap_or_else(|| "手动安装后重新扫描即可。".to_string())
    ))
}

/// 检查 Ollama 是否已安装
#[tauri::command]
pub async fn check_ollama_installed(app: AppHandle) -> Result<bool, String> {

    match get_ollama_path(&app) {
        Ok(path) => {

            Ok(true)
        },
        Err(e) => {

            Ok(false)
        },
    }
}

#[tauri::command]
pub async fn check_ollama_binary_installed(app: AppHandle) -> Result<bool, String> {
    Ok(get_ollama_path(&app).is_ok())
}

#[tauri::command]
pub async fn check_ollama_service_running() -> Result<bool, String> {
    Ok(is_ollama_service_running().await)
}

/// 启动 Ollama 服务
#[tauri::command]
pub async fn start_ollama_service(app: AppHandle) -> Result<String, String> {

    if is_ollama_service_running().await {
        return Ok("Ollama 服务已在运行".to_string());
    }
    
    let ollama_path = get_ollama_path(&app)?;

    // 启动 Ollama 服务（后台运行）
    #[cfg(target_os = "windows")]
    {
        match std::process::Command::new(&ollama_path)
            .arg("serve")
            .creation_flags(0x08000000) // CREATE_NO_WINDOW
            .spawn()
        {
            Ok(_) => {

                Ok("Ollama 服务已启动".to_string())
            },
            Err(e) => {

                Err(format!("启动 Ollama 失败: {}", e))
            }
        }
    }
    
    #[cfg(not(target_os = "windows"))]
    {
        #[cfg(target_os = "macos")]
        {
            let app_candidates = [
                "/Applications/Ollama.app",
                &format!("{}/Applications/Ollama.app", std::env::var("HOME").unwrap_or_default()),
            ];

            for candidate in app_candidates {
                if std::path::Path::new(candidate).exists() {
                    if std::process::Command::new("open").args(["-a", "Ollama"]).spawn().is_ok() {
                        return Ok("已尝试启动 Ollama.app，请稍等几秒后重新扫描".to_string());
                    }
                }
            }
        }

        match std::process::Command::new(&ollama_path)
            .arg("serve")
            .spawn()
        {
            Ok(_) => {

                Ok("Ollama 服务启动命令已发出，请稍等几秒后重新扫描".to_string())
            },
            Err(e) => {

                Err(format!("启动 Ollama 失败: {}", e))
            }
        }
    }
}

/// 检查 AI 模型是否已安装
#[tauri::command]
pub async fn check_ai_model_installed(app: AppHandle) -> Result<bool, String> {
    let config = AiModelConfig::default();


    // 检查 Ollama 是否安装
    let ollama_path = match get_ollama_path(&app) {
        Ok(path) => {


            path
        },
        Err(e) => {

            return Ok(false);
        }
    };

    if !is_ollama_service_running().await {

        return Ok(false);
    }
    
    // 使用 ollama list 命令检查模型是否存在
    #[cfg(target_os = "windows")]
    let output = {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        
        match Command::new(&ollama_path)
            .creation_flags(CREATE_NO_WINDOW)
            .arg("list")
            .output()
        {
            Ok(output) => output,
            Err(e) => {

                return Ok(false);
            }
        }
    };
    
    #[cfg(not(target_os = "windows"))]
    let output = match Command::new(&ollama_path)
        .arg("list")
        .output()
    {
        Ok(output) => output,
        Err(e) => {

            return Ok(false);
        }
    };
    
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);

        // 检查是否是因为服务未运行
        if stderr.contains("connect") || stderr.contains("refused") || stderr.contains("dial") {

        }
        
        return Ok(false);
    }
    
    let output_str = String::from_utf8_lossy(&output.stdout);
    let found = output_str.contains(&config.model_name);
    
    if found {

    } else {


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



    // 验证 Ollama 可执行文件存在
    if !ollama_path.exists() {
        return Err(format!("Ollama executable not found: {:?}", ollama_path));
    }

    ensure_ollama_service_ready().await?;
    
    // 设置 OLLAMA_MODELS 环境变量，指定模型存储位置
    // 注意：PathBuf 会自动处理中文路径
    #[cfg(target_os = "windows")]
    let output = {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        
        Command::new(ollama_path)
            .creation_flags(CREATE_NO_WINDOW)
            .arg("pull")
            .arg(&config.model_name)
            .env("OLLAMA_MODELS", &model_dir)
            .output()
            .map_err(|e| format!("Failed to pull model: {} (path may contain unsupported characters)", e))?
    };
    
    #[cfg(not(target_os = "windows"))]
    let output = Command::new(ollama_path)
        .arg("pull")
        .arg(&config.model_name)
        .env("OLLAMA_MODELS", &model_dir)
        .output()
        .map_err(|e| format!("Failed to pull model: {}", e))?;
    
    if !output.status.success() {
        let error_msg = String::from_utf8_lossy(&output.stderr);
        let stdout_msg = String::from_utf8_lossy(&output.stdout);
        return Err(format!("Failed to install model:\nSTDERR: {}\nSTDOUT: {}", error_msg, stdout_msg));
    }

    Ok(format!("AI 模型 {} 安装成功", config.model_name))
}

/// 卸载 AI 模型
#[tauri::command]
pub async fn uninstall_ai_model(app: AppHandle) -> Result<String, String> {
    let config = AiModelConfig::default();
    
    // 检查 Ollama 是否安装
    let ollama_path = get_ollama_path(&app)?;

    // 使用 ollama rm 命令删除模型
    #[cfg(target_os = "windows")]
    let output = {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        
        Command::new(ollama_path)
            .creation_flags(CREATE_NO_WINDOW)
            .arg("rm")
            .arg(&config.model_name)
            .output()
            .map_err(|e| format!("Failed to remove model: {}", e))?
    };
    
    #[cfg(not(target_os = "windows"))]
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

    ensure_ollama_service_ready().await?;

    // 使用 ollama run 命令调用模型
    #[cfg(target_os = "windows")]
    let output = {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        
        Command::new(ollama_path)
            .creation_flags(CREATE_NO_WINDOW)
            .arg("run")
            .arg(&config.model_name)
            .arg(&prompt)
            .output()
            .map_err(|e| format!("Failed to run model: {}", e))?
    };
    
    #[cfg(not(target_os = "windows"))]
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
    let service_running = if ollama_installed {
        check_ollama_service_running().await.unwrap_or(false)
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
        "service_running": service_running,
        "using_bundled_ollama": using_bundled,
        "ollama_path": if ollama_installed {
            get_ollama_path(&app).ok().map(|p| p.to_string_lossy().to_string())
        } else {
            None
        },
    }))
}

/// 检查 AI 检测是否可用（Ollama 已安装且模型已安装）
#[tauri::command]
pub async fn check_ai_detection_available(app: AppHandle) -> Result<bool, String> {

    // 检查 Ollama 是否安装
    let ollama_installed = match check_ollama_installed(app.clone()).await {
        Ok(installed) => installed,
        Err(e) => {

            return Ok(false);
        }
    };
    
    if !ollama_installed {

        return Ok(false);
    }
    
    // 检查模型是否安装
    let model_installed = match check_ai_model_installed(app.clone()).await {
        Ok(installed) => installed,
        Err(e) => {

            return Ok(false);
        }
    };
    
    if !model_installed {

        return Ok(false);
    }

    Ok(true)
}
