use std::path::PathBuf;
use std::fs;
use std::io::Write;
use tauri::{AppHandle, Manager, Emitter};
use tokio::process::Command;

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct OcrDownloadProgress {
    pub downloaded: u64,
    pub total: u64,
    pub percentage: f64,
    pub status: String,
}

fn get_ocr_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map(|dir| dir.join("ocr-package"))
        .map_err(|e| format!("Failed to get app data dir: {}", e))
}

fn get_ocr_dir_from_env() -> PathBuf {
    dirs_next::data_dir()
        .map(|dir| dir.join("com.cheersai.vault").join("ocr-package"))
        .unwrap_or_else(|| PathBuf::from(".").join("ocr-package"))
}

fn get_ocr_python_candidates(ocr_dir: &PathBuf) -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    #[cfg(target_os = "windows")]
    {
        candidates.push(ocr_dir.join("python").join("python.exe"));
    }

    #[cfg(not(target_os = "windows"))]
    {
        candidates.push(ocr_dir.join("venv").join("bin").join("python3"));
        candidates.push(ocr_dir.join("venv").join("bin").join("python"));
        candidates.push(ocr_dir.join("python").join("bin").join("python3"));
        candidates.push(ocr_dir.join("python").join("bin").join("python"));
    }

    candidates
}

fn find_system_python() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    let candidates = vec!["python", "python3", "py"];
    #[cfg(not(target_os = "windows"))]
    let candidates = vec!["python3", "python"];

    for candidate in candidates {
        if let Ok(output) = std::process::Command::new(candidate).arg("--version").output() {
            if output.status.success() {
                return Some(PathBuf::from(candidate));
            }
        }
    }

    None
}

fn find_existing_ocr_python(ocr_dir: &PathBuf) -> Option<PathBuf> {
    get_ocr_python_candidates(ocr_dir)
        .into_iter()
        .find(|path| path.exists())
}

pub(crate) fn resolve_ocr_runtime_from_env() -> Option<(PathBuf, PathBuf)> {
    let ocr_dir = get_ocr_dir_from_env();
    let script_path = ocr_dir.join("pdf_ocr.py");

    if let Some(python) = find_existing_ocr_python(&ocr_dir) {
        if script_path.exists() {
            return Some((python, script_path));
        }
    }

    None
}

fn python_supports_pdf_extraction(python_cmd: &PathBuf) -> bool {
    std::process::Command::new(python_cmd)
        .arg("-c")
        .arg("import fitz; print('OK')")
        .output()
        .map(|output| output.status.success() && String::from_utf8_lossy(&output.stdout).contains("OK"))
        .unwrap_or(false)
}

/// 检查 OCR 是否已安装
#[tauri::command]
pub async fn check_ocr_installed(app: AppHandle) -> Result<bool, String> {
    let ocr_dir = get_ocr_dir(&app)?;
    let script_path = ocr_dir.join("pdf_ocr.py");

    if !script_path.exists() {
        return Ok(false);
    }

    if let Some(python) = find_existing_ocr_python(&ocr_dir) {
        return Ok(python_supports_pdf_extraction(&python));
    }

    if let Some(system_python) = find_system_python() {
        return Ok(python_supports_pdf_extraction(&system_python));
    }

    Ok(false)
}

/// 获取 OCR 安装路径
#[tauri::command]
pub async fn get_ocr_install_path(app: AppHandle) -> Result<String, String> {
    Ok(get_ocr_dir(&app)?.to_string_lossy().to_string())
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
        get_ocr_dir(&app)?
    };
    
    println!("OCR installation directory: {:?}", ocr_dir);
    
    let temp_dir = ocr_dir.join("temp");
    
    // 创建目录
    fs::create_dir_all(&temp_dir)
        .map_err(|e| format!("Failed to create temp directory: {}", e))?;
    
    #[cfg(target_os = "windows")]
    let python_dir = ocr_dir.join("python");

    #[cfg(target_os = "windows")]
    {
        fs::create_dir_all(&python_dir)
            .map_err(|e| format!("Failed to create python directory: {}", e))?;

        let _ = window.emit("ocr-download-progress", OcrDownloadProgress {
            downloaded: 0,
            total: 0,
            percentage: 0.0,
            status: "正在下载 Python...".to_string(),
        });

        let python_url = "https://www.python.org/ftp/python/3.11.9/python-3.11.9-embed-amd64.zip";
        let python_zip = temp_dir.join("python.zip");

        download_file(&window, python_url, &python_zip, "Python 运行时").await?;

        let _ = window.emit("ocr-download-progress", OcrDownloadProgress {
            downloaded: 0,
            total: 0,
            percentage: 20.0,
            status: "正在安装 Python...".to_string(),
        });

        extract_zip(&python_zip, &python_dir)?;
        let _ = fs::remove_file(&python_zip);

        let _ = window.emit("ocr-download-progress", OcrDownloadProgress {
            downloaded: 0,
            total: 0,
            percentage: 30.0,
            status: "正在下载 pip...".to_string(),
        });

        let get_pip_url = "https://bootstrap.pypa.io/get-pip.py";
        let get_pip_path = temp_dir.join("get-pip.py");

        download_file(&window, get_pip_url, &get_pip_path, "pip 安装器").await?;

        let _ = window.emit("ocr-download-progress", OcrDownloadProgress {
            downloaded: 0,
            total: 0,
            percentage: 35.0,
            status: "正在配置 pip...".to_string(),
        });

        enable_pip(&python_dir)?;

        let _ = window.emit("ocr-download-progress", OcrDownloadProgress {
            downloaded: 0,
            total: 0,
            percentage: 40.0,
            status: "正在安装 pip...".to_string(),
        });

        install_pip(&python_dir, &get_pip_path)?;
        let _ = fs::remove_file(&get_pip_path);

        let _ = window.emit("ocr-download-progress", OcrDownloadProgress {
            downloaded: 0,
            total: 0,
            percentage: 50.0,
            status: "正在安装 OCR 依赖（PyMuPDF + PaddleOCR）...".to_string(),
        });

        install_ocr_dependencies(&python_dir).await?;
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = window.emit("ocr-download-progress", OcrDownloadProgress {
            downloaded: 0,
            total: 0,
            percentage: 10.0,
            status: "正在创建 Python 虚拟环境...".to_string(),
        });

        create_python_venv(&ocr_dir)?;

        let _ = window.emit("ocr-download-progress", OcrDownloadProgress {
            downloaded: 0,
            total: 0,
            percentage: 55.0,
            status: "正在安装 OCR 依赖（PyMuPDF + PaddleOCR）...".to_string(),
        });

        install_ocr_dependencies(&ocr_dir.join("venv")).await?;
    }
    
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

/// 启用 pip（仅 Windows 嵌入式 Python 使用）
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
    
    let python_exe = resolve_python_executable(python_dir)?;
    
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
async fn install_ocr_dependencies(python_dir: &PathBuf) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    const CREATE_NO_WINDOW: u32 = 0x08000000;
    
    let python_exe = resolve_python_executable(python_dir)?;
    
    println!("Installing OCR dependencies...");
    println!("  Python: {:?}", python_exe);

    #[cfg(not(target_os = "windows"))]
    ensure_python_packaging_toolchain(&python_exe, python_dir)?;
    
    // 安装完整版 OCR：PyMuPDF + EasyOCR
    // PyMuPDF 用于文本型 PDF，EasyOCR 用于扫描版 PDF
    let packages = vec![
        "PyMuPDF",      // PDF 文本提取，约 20MB
        "easyocr",      // OCR 引擎，约 100MB（包含依赖）
        "Pillow",       // 图片处理
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
                .await
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
            install_package_with_fallbacks(&python_exe, python_dir, package)?;
            println!("  ✓ {} installed", package);
        }
    }
    
    println!("✓ All OCR dependencies installed successfully");
    println!("ℹ️  OCR runtime is configured for PDF text extraction via PyMuPDF.");
    
    // 验证安装
    println!("Verifying installation...");
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        let verify_output = Command::new(&python_exe)
            .creation_flags(CREATE_NO_WINDOW)
            .args(&["-c", "import fitz; print('OK')"])
            .current_dir(python_dir)
            .output()
            .await;
        
        match verify_output {
            Ok(output) if output.status.success() => {
                println!("✓ Verification successful");
            },
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                println!("⚠ Verification warning: {}", stderr);
            },
            Err(e) => {
                println!("⚠ Verification failed: {}", e);
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

fn resolve_python_executable(python_dir: &PathBuf) -> Result<PathBuf, String> {
    let candidates = if cfg!(target_os = "windows") {
        vec![python_dir.join("python.exe")]
    } else {
        vec![
            python_dir.join("bin").join("python3"),
            python_dir.join("bin").join("python"),
            python_dir.join("python3"),
            python_dir.join("python"),
        ]
    };

    candidates
        .into_iter()
        .find(|path| path.exists())
        .ok_or_else(|| format!("Python executable not found in {:?}", python_dir))
}

#[cfg(not(target_os = "windows"))]
fn get_python_version(python_exe: &PathBuf, python_dir: &PathBuf) -> Option<(u32, u32)> {
    let output = std::process::Command::new(python_exe)
        .args(["-c", "import sys; print(f'{sys.version_info[0]}.{sys.version_info[1]}')"])
        .current_dir(python_dir)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let mut parts = version.split('.');
    let major = parts.next()?.parse::<u32>().ok()?;
    let minor = parts.next()?.parse::<u32>().ok()?;
    Some((major, minor))
}

#[cfg(not(target_os = "windows"))]
fn ensure_python_packaging_toolchain(python_exe: &PathBuf, python_dir: &PathBuf) -> Result<(), String> {
    use std::process::Command;

    println!("Ensuring pip/setuptools/wheel are available...");

    let ensurepip_output = Command::new(python_exe)
        .args(["-m", "ensurepip", "--upgrade"])
        .current_dir(python_dir)
        .env("PIP_DISABLE_PIP_VERSION_CHECK", "1")
        .output()
        .map_err(|e| format!("修复 pip 运行时失败: {}", e))?;

    if !ensurepip_output.status.success() {
        println!(
            "⚠ ensurepip returned non-zero: {}",
            String::from_utf8_lossy(&ensurepip_output.stderr)
        );
    }

    let upgrade_output = Command::new(python_exe)
        .args([
            "-m",
            "pip",
            "install",
            "--upgrade",
            "--no-warn-script-location",
            "pip",
            "setuptools",
            "wheel",
        ])
        .current_dir(python_dir)
        .env("PIP_DISABLE_PIP_VERSION_CHECK", "1")
        .output()
        .map_err(|e| format!("升级 pip 工具链失败: {}", e))?;

    if !upgrade_output.status.success() {
        let stderr = String::from_utf8_lossy(&upgrade_output.stderr);
        let stdout = String::from_utf8_lossy(&upgrade_output.stdout);
        return Err(format!("升级 pip 工具链失败:\nSTDERR: {}\nSTDOUT: {}", stderr, stdout));
    }

    println!("✓ pip/setuptools/wheel are ready");
    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn install_package_with_fallbacks(
    python_exe: &PathBuf,
    python_dir: &PathBuf,
    package: &str,
) -> Result<(), String> {
    use std::process::Command;

    let mut attempts: Vec<Vec<String>> = vec![
        vec![
            "-m".to_string(),
            "pip".to_string(),
            "install".to_string(),
            "--upgrade".to_string(),
            "--no-warn-script-location".to_string(),
            "--prefer-binary".to_string(),
            "--only-binary=:all:".to_string(),
            package.to_string(),
        ],
        vec![
            "-m".to_string(),
            "pip".to_string(),
            "install".to_string(),
            "--upgrade".to_string(),
            "--no-warn-script-location".to_string(),
            "--prefer-binary".to_string(),
            "--only-binary=:all:".to_string(),
            "--index-url".to_string(),
            "https://pypi.org/simple".to_string(),
            package.to_string(),
        ],
    ];

    if package == "PyMuPDF" {
        if let Some((major, minor)) = get_python_version(python_exe, python_dir) {
            if major == 3 && minor <= 9 {
                attempts.push(vec![
                    "-m".to_string(),
                    "pip".to_string(),
                    "install".to_string(),
                    "--upgrade".to_string(),
                    "--no-warn-script-location".to_string(),
                    "--prefer-binary".to_string(),
                    "--only-binary=:all:".to_string(),
                    "--index-url".to_string(),
                    "https://pypi.org/simple".to_string(),
                    "PyMuPDF==1.26.5".to_string(),
                ]);
            }
        }
    }

    let mut failure_logs = Vec::new();

    for args in attempts {
        println!("  Running pip command: {:?}", args);
        let output = Command::new(python_exe)
            .args(args.iter().map(|s| s.as_str()))
            .current_dir(python_dir)
            .env("PIP_DISABLE_PIP_VERSION_CHECK", "1")
            .env("PIP_DEFAULT_TIMEOUT", "120")
            .output()
            .map_err(|e| format!("Failed to install {}: {}", package, e))?;

        if output.status.success() {
            return Ok(());
        }

        failure_logs.push(format!(
            "STDERR: {}\nSTDOUT: {}",
            String::from_utf8_lossy(&output.stderr),
            String::from_utf8_lossy(&output.stdout)
        ));
    }

    Err(format!(
        "安装 {} 失败，已尝试升级 pip、使用官方索引及兼容版本回退。\n{}",
        package,
        failure_logs.join("\n\n---\n\n")
    ))
}

#[cfg(not(target_os = "windows"))]
fn create_python_venv(ocr_dir: &PathBuf) -> Result<(), String> {
    use std::process::Command;

    let system_python = find_system_python()
        .ok_or_else(|| "未找到可用的 python3，请先安装 Python 3 或 Command Line Tools".to_string())?;

    let venv_dir = ocr_dir.join("venv");
    fs::create_dir_all(ocr_dir).map_err(|e| format!("Failed to create OCR directory: {}", e))?;

    let output = Command::new(system_python)
        .args(["-m", "venv"])
        .arg(&venv_dir)
        .output()
        .map_err(|e| format!("创建 Python 虚拟环境失败: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "创建 Python 虚拟环境失败: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

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
