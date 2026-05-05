use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::io::{BufRead, BufReader, Write};
use tauri::{AppHandle, Manager, Emitter};
use serde::{Deserialize, Serialize};

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

// 内嵌 Python 脚本
const INSTALL_OCR_SCRIPT: &str = include_str!("../../../scripts/install_ocr.py");
const INSTALL_OLLAMA_SCRIPT: &str = include_str!("../../../scripts/install_ollama.py");

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct InstallerProgress {
    pub percentage: f64,
    pub status: String,
    pub log: String,
}

/// 获取 Python 可执行文件路径
fn get_python_exe() -> Result<String, String> {
    // 尝试查找系统 Python
    #[cfg(target_os = "windows")]
    {
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        
        // 尝试 python3
        if let Ok(output) = Command::new("where")
            .creation_flags(CREATE_NO_WINDOW)
            .arg("python3")
            .output()
        {
            if output.status.success() {
                let path = String::from_utf8_lossy(&output.stdout);
                let first_path = path.lines().next().unwrap_or("").trim();
                if !first_path.is_empty() {
                    return Ok(first_path.to_string());
                }
            }
        }
        
        // 尝试 python
        if let Ok(output) = Command::new("where")
            .creation_flags(CREATE_NO_WINDOW)
            .arg("python")
            .output()
        {
            if output.status.success() {
                let path = String::from_utf8_lossy(&output.stdout);
                let first_path = path.lines().next().unwrap_or("").trim();
                if !first_path.is_empty() {
                    return Ok(first_path.to_string());
                }
            }
        }
        
        // 尝试 py launcher
        if let Ok(output) = Command::new("py")
            .creation_flags(CREATE_NO_WINDOW)
            .arg("--version")
            .output()
        {
            if output.status.success() {
                return Ok("py".to_string());
            }
        }
    }
    
    #[cfg(not(target_os = "windows"))]
    {
        // 尝试 python3
        if let Ok(output) = Command::new("which")
            .arg("python3")
            .output()
        {
            if output.status.success() {
                let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !path.is_empty() {
                    return Ok(path);
                }
            }
        }
        
        // 尝试 python
        if let Ok(output) = Command::new("which")
            .arg("python")
            .output()
        {
            if output.status.success() {
                let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !path.is_empty() {
                    return Ok(path);
                }
            }
        }
    }
    
    Err("Python 未安装。请先安装 Python 3.7 或更高版本。".to_string())
}

/// 获取或创建安装脚本
fn get_or_create_script(script_name: &str) -> Result<PathBuf, String> {
    println!("Getting script: {}", script_name);
    
    // 获取脚本内容
    let script_content = match script_name {
        "install_ocr.py" => INSTALL_OCR_SCRIPT,
        "install_ollama.py" => INSTALL_OLLAMA_SCRIPT,
        _ => return Err(format!("Unknown script: {}", script_name)),
    };
    
    // 创建临时目录
    let temp_dir = std::env::temp_dir().join("cheersai_scripts");
    std::fs::create_dir_all(&temp_dir)
        .map_err(|e| format!("Failed to create temp dir: {}", e))?;
    
    // 写入脚本文件
    let script_path = temp_dir.join(script_name);
    std::fs::write(&script_path, script_content)
        .map_err(|e| format!("Failed to write script: {}", e))?;
    
    println!("  ✓ Script created at: {:?}", script_path);
    Ok(script_path)
}

/// 运行安装脚本并实时输出进度
async fn run_installer_script(
    _app: AppHandle,
    window: tauri::Window,
    script_name: &str,
    args: Vec<&str>,
    event_name: &str,
) -> Result<String, String> {
    let python_exe = get_python_exe()?;
    let script_path = get_or_create_script(script_name)?;
    
    println!("Running installer script:");
    println!("  Python: {}", python_exe);
    println!("  Script: {:?}", script_path);
    println!("  Args: {:?}", args);
    
    // 构建命令
    #[cfg(target_os = "windows")]
    let mut cmd = {
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        let mut cmd = Command::new(&python_exe);
        cmd.creation_flags(CREATE_NO_WINDOW);
        cmd
    };
    
    #[cfg(not(target_os = "windows"))]
    let mut cmd = Command::new(&python_exe);
    
    cmd.arg(&script_path);
    for arg in args {
        cmd.arg(arg);
    }
    
    cmd.stdout(Stdio::piped())
        .stderr(Stdio::piped());
    
    // 启动进程
    let mut child = cmd.spawn()
        .map_err(|e| format!("Failed to start installer: {}", e))?;
    
    // 读取输出
    let stdout = child.stdout.take()
        .ok_or("Failed to capture stdout")?;
    let stderr = child.stderr.take()
        .ok_or("Failed to capture stderr")?;
    
    let window_clone = window.clone();
    let event_name_clone = event_name.to_string();
    
    // 在单独的线程中读取 stdout
    let stdout_handle = std::thread::spawn(move || {
        let reader = BufReader::new(stdout);
        for line in reader.lines() {
            if let Ok(line) = line {
                println!("{}", line);
                
                // 解析进度信息
                let progress = parse_installer_log(&line);
                let _ = window_clone.emit(&event_name_clone, progress);
            }
        }
    });
    
    // 在单独的线程中读取 stderr
    let stderr_handle = std::thread::spawn(move || {
        let reader = BufReader::new(stderr);
        let mut stderr_lines = Vec::new();
        for line in reader.lines() {
            if let Ok(line) = line {
                eprintln!("{}", line);
                stderr_lines.push(line);
            }
        }
        stderr_lines
    });
    
    // 等待进程完成
    let status = child.wait()
        .map_err(|e| format!("Failed to wait for installer: {}", e))?;
    
    // 等待输出线程完成
    stdout_handle.join().ok();
    let stderr_lines = stderr_handle.join().unwrap_or_default();
    
    if !status.success() {
        let error_msg = stderr_lines.join("\n");
        return Err(format!("安装失败:\n{}", error_msg));
    }
    
    Ok("安装完成".to_string())
}

/// 解析安装脚本的日志输出
fn parse_installer_log(line: &str) -> InstallerProgress {
    // 解析多种格式:
    // 1. [2024-01-01 12:00:00] [INFO] 下载进度: 50.0% (5.60 MB / 11.20 MB)
    // 2. pulling 183715c43589: 50% ▕████████▏ 500 MB
    // 3. downloading: 75.5%
    
    let mut percentage = 0.0;
    let mut status = line.to_string();
    
    // 提取百分比 - 支持多种格式
    if let Some(pos) = line.find('%') {
        // 向前查找数字开始位置
        let mut start = pos;
        while start > 0 {
            let ch = line.chars().nth(start - 1).unwrap_or(' ');
            if ch.is_numeric() || ch == '.' {
                start -= 1;
            } else {
                break;
            }
        }
        
        if let Ok(pct) = line[start..pos].trim().parse::<f64>() {
            percentage = pct;
        }
    }
    
    // 提取状态信息
    // 优先处理带时间戳的日志格式
    if let Some(pos) = line.find("] [") {
        if let Some(end) = line[pos..].find(']') {
            status = line[pos + end + 2..].trim().to_string();
        }
    } else {
        // 处理 Ollama 的输出格式
        // 清理 ANSI 转义序列和特殊字符
        let cleaned = line
            .replace("\x1b[?2026h", "")
            .replace("\x1b[?25l", "")
            .replace("\x1b[?25h", "")
            .replace("\x1b[?2026l", "")
            .replace("\x1b[1G", "")
            .replace("[K", "");
        
        status = cleaned.trim().to_string();
        
        // 如果是 pulling 行，提取更友好的状态
        if status.starts_with("pulling") {
            // 提取文件名和进度
            if let Some(colon_pos) = status.find(':') {
                let parts: Vec<&str> = status[..colon_pos].split_whitespace().collect();
                if parts.len() >= 2 {
                    let file_id = &parts[1][..8.min(parts[1].len())]; // 取前8个字符
                    if percentage > 0.0 {
                        status = format!("下载模型文件 {}: {:.1}%", file_id, percentage);
                    } else {
                        status = format!("下载模型文件 {}", file_id);
                    }
                }
            }
        } else if status.contains("pulling manifest") {
            status = "正在获取模型清单...".to_string();
            percentage = 5.0; // 给一个小的进度值
        } else if status.contains("verifying") {
            status = "正在验证模型文件...".to_string();
            percentage = 95.0;
        } else if status.contains("writing manifest") {
            status = "正在写入模型清单...".to_string();
            percentage = 98.0;
        } else if status.contains("success") {
            status = "模型下载完成".to_string();
            percentage = 100.0;
        }
    }
    
    InstallerProgress {
        percentage,
        status: status.clone(),
        log: line.to_string(),
    }
}

/// 安装 OCR 环境
#[tauri::command]
pub async fn install_ocr_with_script(
    app: AppHandle,
    window: tauri::Window,
) -> Result<String, String> {
    println!("=== Installing OCR using script ===");
    
    run_installer_script(
        app,
        window,
        "install_ocr.py",
        vec![],
        "ocr-install-progress",
    ).await
}

/// 卸载 OCR 环境
#[tauri::command]
pub async fn uninstall_ocr_with_script(
    app: AppHandle,
    window: tauri::Window,
) -> Result<String, String> {
    println!("=== Uninstalling OCR using script ===");
    
    run_installer_script(
        app,
        window,
        "install_ocr.py",
        vec!["uninstall"],
        "ocr-uninstall-progress",
    ).await
}

/// 安装 Ollama + AI 模型
#[tauri::command]
pub async fn install_ollama_with_script(
    app: AppHandle,
    window: tauri::Window,
) -> Result<String, String> {
    println!("=== Installing Ollama using script ===");
    
    run_installer_script(
        app,
        window,
        "install_ollama.py",
        vec![],
        "ollama-install-progress",
    ).await
}

/// 卸载 Ollama + AI 模型
#[tauri::command]
pub async fn uninstall_ollama_with_script(
    app: AppHandle,
    window: tauri::Window,
) -> Result<String, String> {
    println!("=== Uninstalling Ollama using script ===");
    
    run_installer_script(
        app,
        window,
        "install_ollama.py",
        vec!["uninstall"],
        "ollama-uninstall-progress",
    ).await
}

/// 检查 Python 是否可用
#[tauri::command]
pub async fn check_python_available() -> Result<bool, String> {
    match get_python_exe() {
        Ok(python) => {
            println!("✓ Python found: {}", python);
            Ok(true)
        },
        Err(e) => {
            println!("✗ Python not found: {}", e);
            Ok(false)
        }
    }
}

/// 获取 Ollama 安装程序路径（如果存在）
#[tauri::command]
pub async fn get_ollama_installer_path() -> Result<Option<String>, String> {
    let temp_dir = std::env::temp_dir();
    let installer_path = temp_dir.join("OllamaSetup.exe");
    
    if installer_path.exists() {
        Ok(Some(installer_path.to_string_lossy().to_string()))
    } else {
        Ok(None)
    }
}

/// 打开文件所在文件夹
#[tauri::command]
pub async fn open_installer_folder() -> Result<(), String> {
    let temp_dir = std::env::temp_dir();
    
    #[cfg(target_os = "windows")]
    {
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        
        std::process::Command::new("explorer")
            .creation_flags(CREATE_NO_WINDOW)
            .arg(temp_dir)
            .spawn()
            .map_err(|e| format!("无法打开文件夹: {}", e))?;
    }
    
    #[cfg(not(target_os = "windows"))]
    {
        std::process::Command::new("open")
            .arg(temp_dir)
            .spawn()
            .map_err(|e| format!("无法打开文件夹: {}", e))?;
    }
    
    Ok(())
}
