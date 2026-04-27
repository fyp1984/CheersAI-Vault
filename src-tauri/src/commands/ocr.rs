use std::path::PathBuf;
use std::fs;
use std::io::Write;
use tauri::{AppHandle, Manager, Emitter};

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct OcrDownloadProgress {
    pub downloaded: u64,
    pub total: u64,
    pub percentage: f64,
    pub status: String,
}

/// 检查 OCR 是否已安装
#[tauri::command]
pub async fn check_ocr_installed(app: AppHandle) -> Result<bool, String> {
    // 只检查内置的 OCR 包，不检查系统 Python
    let exe_dir = app.path().app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {}", e))?;
    
    let ocr_python = exe_dir.join("ocr-package").join("python").join("python.exe");
    let ocr_script = exe_dir.join("ocr-package").join("pdf_ocr.py");
    
    println!("Checking OCR installation:");
    println!("  Python: {} (exists: {})", ocr_python.display(), ocr_python.exists());
    println!("  Script: {} (exists: {})", ocr_script.display(), ocr_script.exists());
    
    let is_installed = ocr_python.exists() && ocr_script.exists();
    println!("  Result: {}", if is_installed { "Installed" } else { "Not installed" });
    
    Ok(is_installed)
}

/// 获取 OCR 安装路径
#[tauri::command]
pub async fn get_ocr_install_path(app: AppHandle) -> Result<String, String> {
    let exe_dir = app.path().app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {}", e))?;
    
    Ok(exe_dir.join("ocr-package").to_string_lossy().to_string())
}

/// 下载 OCR 包（支持自定义路径）
#[tauri::command]
pub async fn download_ocr_package(
    app: AppHandle,
    window: tauri::Window,
    custom_path: Option<String>,
) -> Result<String, String> {
    println!("Starting OCR package download and setup");
    
    // 获取安装目录
    let ocr_dir = if let Some(path) = custom_path {
        PathBuf::from(path)
    } else {
        // 默认使用应用数据目录
        let app_data_dir = app.path().app_data_dir()
            .map_err(|e| format!("Failed to get app data dir: {}", e))?;
        app_data_dir.join("ocr-package")
    };
    
    println!("OCR installation directory: {:?}", ocr_dir);
    
    let python_dir = ocr_dir.join("python");
    let temp_dir = ocr_dir.join("temp");
    
    // 创建目录
    fs::create_dir_all(&temp_dir)
        .map_err(|e| format!("Failed to create temp directory: {}", e))?;
    fs::create_dir_all(&python_dir)
        .map_err(|e| format!("Failed to create python directory: {}", e))?;
    
    // 发送开始下载事件
    let _ = window.emit("ocr-download-progress", OcrDownloadProgress {
        downloaded: 0,
        total: 0,
        percentage: 0.0,
        status: "正在下载 Python...".to_string(),
    });
    
    // 1. 下载 Python 嵌入式版本
    let python_url = "https://www.python.org/ftp/python/3.11.9/python-3.11.9-embed-amd64.zip";
    let python_zip = temp_dir.join("python.zip");
    
    download_file(&window, python_url, &python_zip, "Python 运行时").await?;
    
    // 解压 Python
    let _ = window.emit("ocr-download-progress", OcrDownloadProgress {
        downloaded: 0,
        total: 0,
        percentage: 20.0,
        status: "正在安装 Python...".to_string(),
    });
    
    extract_zip(&python_zip, &python_dir)?;
    let _ = fs::remove_file(&python_zip);
    
    // 2. 下载并安装 pip
    let _ = window.emit("ocr-download-progress", OcrDownloadProgress {
        downloaded: 0,
        total: 0,
        percentage: 30.0,
        status: "正在下载 pip...".to_string(),
    });
    
    let get_pip_url = "https://bootstrap.pypa.io/get-pip.py";
    let get_pip_path = temp_dir.join("get-pip.py");
    
    download_file(&window, get_pip_url, &get_pip_path, "pip 安装器").await?;
    
    // 先启用 pip
    let _ = window.emit("ocr-download-progress", OcrDownloadProgress {
        downloaded: 0,
        total: 0,
        percentage: 35.0,
        status: "正在配置 pip...".to_string(),
    });
    
    enable_pip(&python_dir)?;
    
    // 安装 pip
    let _ = window.emit("ocr-download-progress", OcrDownloadProgress {
        downloaded: 0,
        total: 0,
        percentage: 40.0,
        status: "正在安装 pip...".to_string(),
    });
    
    install_pip(&python_dir, &get_pip_path)?;
    let _ = fs::remove_file(&get_pip_path);
    
    // 3. 安装 OCR 依赖（仅 PyMuPDF）
    let _ = window.emit("ocr-download-progress", OcrDownloadProgress {
        downloaded: 0,
        total: 0,
        percentage: 50.0,
        status: "正在安装 PyMuPDF（PDF 文本提取）...".to_string(),
    });
    
    install_ocr_dependencies(&python_dir)?;
    
    // 4. 复制 OCR 脚本
    let _ = window.emit("ocr-download-progress", OcrDownloadProgress {
        downloaded: 0,
        total: 0,
        percentage: 95.0,
        status: "正在配置 OCR 脚本...".to_string(),
    });
    
    copy_ocr_script(&ocr_dir)?;
    
    // 完成
    let _ = window.emit("ocr-download-progress", OcrDownloadProgress {
        downloaded: 0,
        total: 0,
        percentage: 100.0,
        status: "安装完成！".to_string(),
    });
    
    println!("OCR package setup completed at: {}", ocr_dir.display());
    Ok(ocr_dir.to_string_lossy().to_string())
}

/// 下载文件
async fn download_file(
    window: &tauri::Window,
    url: &str,
    dest: &PathBuf,
    name: &str,
) -> Result<(), String> {
    use futures_util::StreamExt;
    
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(600))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;
    
    let response = client.get(url)
        .send()
        .await
        .map_err(|e| format!("下载 {} 失败: {}。请检查网络连接。", name, e))?;
    
    if !response.status().is_success() {
        return Err(format!("下载 {} 失败，状态码: {}", name, response.status()));
    }
    
    let total_size = response.content_length().unwrap_or(0);
    let mut file = fs::File::create(dest)
        .map_err(|e| format!("Failed to create file: {}", e))?;
    
    let mut downloaded: u64 = 0;
    let mut stream = response.bytes_stream();
    
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("Failed to read chunk: {}", e))?;
        file.write_all(&chunk)
            .map_err(|e| format!("Failed to write chunk: {}", e))?;
        
        downloaded += chunk.len() as u64;
        
        if total_size > 0 && downloaded % (512 * 1024) == 0 {
            let _ = window.emit("ocr-download-progress", OcrDownloadProgress {
                downloaded,
                total: total_size,
                percentage: 0.0,
                status: format!("正在下载 {}... {:.1} MB / {:.1} MB", 
                    name,
                    downloaded as f64 / 1024.0 / 1024.0,
                    total_size as f64 / 1024.0 / 1024.0),
            });
        }
    }
    
    Ok(())
}

/// 启用 pip
fn enable_pip(python_dir: &PathBuf) -> Result<(), String> {
    println!("Enabling pip for embedded Python...");
    
    let pth_file = python_dir.join("python311._pth");
    
    println!("  Looking for: {:?}", pth_file);
    
    if !pth_file.exists() {
        return Err(format!("Python ._pth file not found at: {:?}", pth_file));
    }
    
    let content = fs::read_to_string(&pth_file)
        .map_err(|e| format!("Failed to read _pth file: {}", e))?;
    
    println!("  Current content:\n{}", content);
    
    // 检查是否已经启用
    if content.lines().any(|line| {
        let trimmed = line.trim();
        trimmed == "import site" && !trimmed.starts_with('#')
    }) {
        println!("  ✓ pip already enabled");
        return Ok(());
    }
    
    // 重新构建内容：保留非注释行，移除 #import site，添加 import site
    let mut new_lines: Vec<String> = content
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            // 保留非空行，但跳过 #import site 和 # Uncomment... 注释
            !trimmed.is_empty() 
                && !trimmed.starts_with("#import site")
                && !trimmed.contains("Uncomment to run site.main()")
        })
        .map(|s| s.to_string())
        .collect();
    
    // 添加 import site（如果还没有）
    if !new_lines.iter().any(|line| line.trim() == "import site") {
        new_lines.push("import site".to_string());
    }
    
    let new_content = new_lines.join("\n") + "\n";
    
    println!("  New content:\n{}", new_content);
    
    fs::write(&pth_file, new_content)
        .map_err(|e| format!("Failed to write _pth file: {}", e))?;
    
    println!("  ✓ pip enabled successfully");
    Ok(())
}

/// 安装 pip
fn install_pip(python_dir: &PathBuf, get_pip_path: &PathBuf) -> Result<(), String> {
    use std::process::Command;
    
    #[cfg(target_os = "windows")]
    const CREATE_NO_WINDOW: u32 = 0x08000000;
    
    let python_exe = python_dir.join("python.exe");
    
    println!("Installing pip...");
    println!("  Python: {:?}", python_exe);
    println!("  get-pip.py: {:?}", get_pip_path);
    println!("  Working dir: {:?}", python_dir);
    
    // 验证文件存在
    if !python_exe.exists() {
        return Err(format!("Python executable not found: {:?}", python_exe));
    }
    if !get_pip_path.exists() {
        return Err(format!("get-pip.py not found: {:?}", get_pip_path));
    }
    
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        let output = Command::new(&python_exe)
            .creation_flags(CREATE_NO_WINDOW)
            .arg(get_pip_path)
            .current_dir(python_dir)
            .output()
            .map_err(|e| format!("Failed to run get-pip.py: {} (path may contain unsupported characters)", e))?;
        
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            return Err(format!("pip 安装失败:\nSTDERR: {}\nSTDOUT: {}", stderr, stdout));
        }
        
        println!("✓ pip installed successfully");
    }
    
    #[cfg(not(target_os = "windows"))]
    {
        let output = Command::new(&python_exe)
            .arg(get_pip_path)
            .current_dir(python_dir)
            .output()
            .map_err(|e| format!("Failed to run get-pip.py: {}", e))?;
        
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            return Err(format!("pip 安装失败:\nSTDERR: {}\nSTDOUT: {}", stderr, stdout));
        }
        
        println!("✓ pip installed successfully");
    }
    
    Ok(())
}

/// 安装 OCR 依赖
fn install_ocr_dependencies(python_dir: &PathBuf) -> Result<(), String> {
    use std::process::Command;
    
    #[cfg(target_os = "windows")]
    const CREATE_NO_WINDOW: u32 = 0x08000000;
    
    let python_exe = python_dir.join("python.exe");
    
    println!("Installing OCR dependencies...");
    println!("  Python: {:?}", python_exe);
    
    // 安装 OCR 依赖：PyMuPDF 用于 PDF 文本提取，easyocr 用于图片 OCR
    let packages = vec![
        "PyMuPDF",   // PDF 文本提取，约 20MB
        "easyocr",   // 图片 OCR，约 500MB+（包含模型）
    ];
    
    for package in packages {
        println!("  Installing {}...", package);
        
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            
            // 添加超时机制
            let start = std::time::Instant::now();
            let timeout = std::time::Duration::from_secs(300); // 5分钟超时
            
            let output = Command::new(&python_exe)
                .creation_flags(CREATE_NO_WINDOW)
                .args(&["-m", "pip", "install", "--no-warn-script-location", package])
                .current_dir(python_dir)
                .output()
                .map_err(|e| format!("Failed to install {}: {} (path may contain unsupported characters)", package, e))?;
            
            let elapsed = start.elapsed();
            println!("  Installation took: {:?}", elapsed);
            
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                let stdout = String::from_utf8_lossy(&output.stdout);
                return Err(format!("安装 {} 失败:\nSTDERR: {}\nSTDOUT: {}", package, stderr, stdout));
            }
            
            println!("  ✓ {} installed", package);
        }
        
        #[cfg(not(target_os = "windows"))]
        {
            let output = Command::new(&python_exe)
                .args(&["-m", "pip", "install", "--no-warn-script-location", package])
                .current_dir(python_dir)
                .output()
                .map_err(|e| format!("Failed to install {}: {}", package, e))?;
            
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                let stdout = String::from_utf8_lossy(&output.stdout);
                return Err(format!("安装 {} 失败:\nSTDERR: {}\nSTDOUT: {}", package, stderr, stdout));
            }
            
            println!("  ✓ {} installed", package);
        }
    }
    
    println!("✓ All OCR dependencies installed successfully");
    
    // 验证安装
    println!("Verifying installation...");
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        
        // 验证 PyMuPDF
        let verify_pymupdf = Command::new(&python_exe)
            .creation_flags(CREATE_NO_WINDOW)
            .args(&["-c", "import fitz; print('PyMuPDF OK')"])
            .current_dir(python_dir)
            .output();
        
        match verify_pymupdf {
            Ok(output) if output.status.success() => {
                println!("✓ PyMuPDF verified");
            },
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                println!("⚠ PyMuPDF verification warning: {}", stderr);
            },
            Err(e) => {
                println!("⚠ PyMuPDF verification failed: {}", e);
            }
        }
        
        // 验证 easyocr
        let verify_easyocr = Command::new(&python_exe)
            .creation_flags(CREATE_NO_WINDOW)
            .args(&["-c", "import easyocr; print('easyocr OK')"])
            .current_dir(python_dir)
            .output();
        
        match verify_easyocr {
            Ok(output) if output.status.success() => {
                println!("✓ easyocr verified");
            },
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                println!("⚠ easyocr verification warning: {}", stderr);
            },
            Err(e) => {
                println!("⚠ easyocr verification failed: {}", e);
            }
        }
    }
    
    #[cfg(not(target_os = "windows"))]
    {
        // 验证 PyMuPDF
        let verify_pymupdf = Command::new(&python_exe)
            .args(&["-c", "import fitz; print('PyMuPDF OK')"])
            .current_dir(python_dir)
            .output();
        
        match verify_pymupdf {
            Ok(output) if output.status.success() => {
                println!("✓ PyMuPDF verified");
            },
            _ => {
                println!("⚠ PyMuPDF verification failed");
            }
        }
        
        // 验证 easyocr
        let verify_easyocr = Command::new(&python_exe)
            .args(&["-c", "import easyocr; print('easyocr OK')"])
            .current_dir(python_dir)
            .output();
        
        match verify_easyocr {
            Ok(output) if output.status.success() => {
                println!("✓ easyocr verified");
            },
            _ => {
                println!("⚠ easyocr verification failed");
            }
        }
    }
    
    Ok(())
}

/// 复制 OCR 脚本
fn copy_ocr_script(ocr_dir: &PathBuf) -> Result<(), String> {
    // 从应用资源或嵌入的脚本复制
    let script_content = include_str!("../../scripts/pdf_ocr.py");
    let script_path = ocr_dir.join("pdf_ocr.py");
    
    fs::write(&script_path, script_content)
        .map_err(|e| format!("Failed to write OCR script: {}", e))?;
    
    Ok(())
}

/// 解压 ZIP 文件
fn extract_zip(zip_path: &PathBuf, extract_dir: &PathBuf) -> Result<(), String> {
    use zip::ZipArchive;
    
    let file = fs::File::open(zip_path)
        .map_err(|e| format!("Failed to open zip file: {}", e))?;
    
    let mut archive = ZipArchive::new(file)
        .map_err(|e| format!("Failed to read zip archive: {}", e))?;
    
    // 创建目标目录
    fs::create_dir_all(extract_dir)
        .map_err(|e| format!("Failed to create extract directory: {}", e))?;
    
    for i in 0..archive.len() {
        let mut file = archive.by_index(i)
            .map_err(|e| format!("Failed to read file from archive: {}", e))?;
        
        let outpath = match file.enclosed_name() {
            Some(path) => extract_dir.join(path),
            None => continue,
        };
        
        if file.name().ends_with('/') {
            // 目录
            fs::create_dir_all(&outpath)
                .map_err(|e| format!("Failed to create directory: {}", e))?;
        } else {
            // 文件
            if let Some(parent) = outpath.parent() {
                fs::create_dir_all(parent)
                    .map_err(|e| format!("Failed to create parent directory: {}", e))?;
            }
            
            let mut outfile = fs::File::create(&outpath)
                .map_err(|e| format!("Failed to create file: {}", e))?;
            
            std::io::copy(&mut file, &mut outfile)
                .map_err(|e| format!("Failed to extract file: {}", e))?;
        }
        
        // 设置文件权限（Unix）
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Some(mode) = file.unix_mode() {
                fs::set_permissions(&outpath, fs::Permissions::from_mode(mode))
                    .map_err(|e| format!("Failed to set permissions: {}", e))?;
            }
        }
    }
    
    Ok(())
}

/// 删除 OCR 包
#[tauri::command]
pub async fn uninstall_ocr_package(app: AppHandle) -> Result<(), String> {
    let app_data_dir = app.path().app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {}", e))?;
    
    let ocr_dir = app_data_dir.join("ocr-package");
    
    println!("Uninstalling OCR package from: {:?}", ocr_dir);
    
    if ocr_dir.exists() {
        println!("OCR directory exists, removing...");
        
        // 列出要删除的内容
        if let Ok(entries) = fs::read_dir(&ocr_dir) {
            for entry in entries.flatten() {
                println!("  - Removing: {:?}", entry.path());
            }
        }
        
        fs::remove_dir_all(&ocr_dir)
            .map_err(|e| format!("Failed to remove OCR package: {}", e))?;
        
        // 验证删除
        if ocr_dir.exists() {
            return Err("OCR directory still exists after removal".to_string());
        }
        
        println!("✓ OCR package uninstalled successfully");
    } else {
        println!("OCR directory does not exist, nothing to uninstall");
    }
    
    Ok(())
}
