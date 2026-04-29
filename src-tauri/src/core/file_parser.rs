use anyhow::{Result, Context};
use std::fs;
use std::io::Read;

#[derive(Debug)]
pub enum FileFormat {
    Csv,
    Excel,
    Json,
    Text,
    Word,
    PowerPoint,
    Markdown,
    Pdf,
}

pub fn detect_format(path: &str) -> FileFormat {
    let lower = path.to_lowercase();
    if lower.ends_with(".csv") {
        FileFormat::Csv
    } else if lower.ends_with(".xlsx") || lower.ends_with(".xls") {
        FileFormat::Excel
    } else if lower.ends_with(".json") {
        FileFormat::Json
    } else if lower.ends_with(".docx") || lower.ends_with(".doc") {
        FileFormat::Word
    } else if lower.ends_with(".pptx") || lower.ends_with(".ppt") {
        FileFormat::PowerPoint
    } else if lower.ends_with(".md") || lower.ends_with(".markdown") {
        FileFormat::Markdown
    } else if lower.ends_with(".pdf") {
        FileFormat::Pdf
    } else {
        FileFormat::Text
    }
}

pub fn parse_csv(path: &str) -> Result<(Vec<String>, Vec<Vec<String>>)> {
    // 首先尝试直接读取
    match try_parse_csv_direct(path) {
        Ok(result) => return Ok(result),
        Err(_) => {
            // 如果失败，尝试不同编码
            if let Ok(content) = parse_text_file(path) {
                return try_parse_csv_from_string(&content);
            }
        }
    }
    
    Err(anyhow::anyhow!("Failed to parse CSV file: {}", path))
}

// Excel parsing using calamine
pub fn parse_excel(path: &str) -> Result<(Vec<String>, Vec<Vec<String>>)> {
    use calamine::{Reader, open_workbook_auto, Data};
    
    let mut workbook = open_workbook_auto(path)
        .context("Failed to open Excel file")?;
    
    // Get the first worksheet
    let sheet_names = workbook.sheet_names().to_owned();
    if sheet_names.is_empty() {
        return Err(anyhow::anyhow!("Excel file has no sheets"));
    }
    
    let sheet_name = &sheet_names[0];
    let range = workbook.worksheet_range(sheet_name)
        .map_err(|e| anyhow::anyhow!("Worksheet error: {}", e))?;
    
    let mut headers = Vec::new();
    let mut rows = Vec::new();
    
    for (idx, row) in range.rows().enumerate() {
        let row_data: Vec<String> = row.iter()
            .map(|cell| match cell {
                Data::Int(i) => i.to_string(),
                Data::Float(f) => f.to_string(),
                Data::String(s) => s.clone(),
                Data::Bool(b) => b.to_string(),
                Data::DateTime(dt) => dt.to_string(),
                Data::DateTimeIso(s) => s.clone(),
                Data::DurationIso(s) => s.clone(),
                Data::Error(e) => format!("Error: {:?}", e),
                Data::Empty => String::new(),
            })
            .collect();
        
        if idx == 0 {
            headers = row_data;
        } else {
            rows.push(row_data);
        }
    }
    
    Ok((headers, rows))
}

fn try_parse_csv_direct(path: &str) -> Result<(Vec<String>, Vec<Vec<String>>)> {
    let mut reader = csv::Reader::from_path(path)?;
    let headers: Vec<String> = reader.headers()?.iter().map(|s| s.to_string()).collect();
    let mut rows = Vec::new();
    for result in reader.records() {
        let record = result?;
        rows.push(record.iter().map(|s| s.to_string()).collect());
    }
    Ok((headers, rows))
}

fn try_parse_csv_from_string(content: &str) -> Result<(Vec<String>, Vec<Vec<String>>)> {
    let mut reader = csv::Reader::from_reader(content.as_bytes());
    let headers: Vec<String> = reader.headers()?.iter().map(|s| s.to_string()).collect();
    let mut rows = Vec::new();
    for result in reader.records() {
        let record = result?;
        rows.push(record.iter().map(|s| s.to_string()).collect());
    }
    Ok((headers, rows))
}

pub fn write_csv(path: &str, headers: &[String], rows: &[Vec<String>]) -> Result<()> {
    let mut writer = csv::Writer::from_path(path)?;
    writer.write_record(headers)?;
    for row in rows {
        writer.write_record(row)?;
    }
    writer.flush()?;
    Ok(())
}

// Word document parsing (DOCX) - simplified version
// Extracts text content from DOCX using zip + XML parsing
pub fn parse_word(path: &str) -> Result<String> {
    use zip::ZipArchive;
    use quick_xml::Reader;
    use quick_xml::events::Event;

    // 检查文件是否存在
    if !std::path::Path::new(path).exists() {
        return Err(anyhow::anyhow!("Word file not found: {}", path));
    }

    // 检查文件扩展名
    let lower_path = path.to_lowercase();
    if lower_path.ends_with(".doc") {
        return Err(anyhow::anyhow!("旧版 .doc 格式暂不支持，请使用 .docx 格式"));
    }

    let file = fs::File::open(path)
        .with_context(|| format!("无法打开 Word 文件: {}", path))?;
    
    let mut archive = ZipArchive::new(file)
        .with_context(|| format!("无法读取 DOCX 文件，可能文件已损坏或不是有效的 DOCX 格式: {}", path))?;

    let mut text = String::new();

    // Try to read document.xml which contains the main content
    match archive.by_name("word/document.xml") {
        Ok(mut doc_file) => {
            let mut content = String::new();
            doc_file.read_to_string(&mut content)
                .context("无法读取文档内容")?;

            // Parse XML and extract text
            let mut reader = Reader::from_str(&content);
            reader.trim_text(true);

            let mut buf = Vec::new();
            loop {
                match reader.read_event_into(&mut buf) {
                    Ok(Event::Text(e)) => {
                        if let Ok(txt) = e.unescape() {
                            text.push_str(&txt);
                            text.push('\n');
                        }
                    }
                    Ok(Event::Eof) => break,
                    Err(e) => return Err(anyhow::anyhow!("XML 解析错误: {}", e)),
                    _ => {}
                }
                buf.clear();
            }
        }
        Err(e) => {
            return Err(anyhow::anyhow!("DOCX 文件结构异常，找不到 word/document.xml: {}", e));
        }
    }

    if text.is_empty() {
        text = "（文档为空或无法提取文本内容）".to_string();
    }

    Ok(text)
}

pub fn write_word(path: &str, content: &str) -> Result<()> {
    // For simplicity, write as plain text with .docx extension
    // Users can open in Word and it will be imported as text
    fs::write(path, content).context("Failed to write Word file")?;
    Ok(())
}

// PowerPoint parsing (PPTX)
pub fn parse_powerpoint(path: &str) -> Result<String> {
    use zip::ZipArchive;
    use quick_xml::Reader;
    use quick_xml::events::Event;

    // 检查文件是否存在
    if !std::path::Path::new(path).exists() {
        return Err(anyhow::anyhow!("PowerPoint file not found: {}", path));
    }

    // 检查文件扩展名
    let lower_path = path.to_lowercase();
    if lower_path.ends_with(".ppt") {
        return Err(anyhow::anyhow!("旧版 .ppt 格式暂不支持，请使用 .pptx 格式"));
    }

    let file = fs::File::open(path)
        .with_context(|| format!("无法打开 PowerPoint 文件: {}", path))?;
    
    let mut archive = ZipArchive::new(file)
        .with_context(|| format!("无法读取 PPTX 文件，可能文件已损坏或不是有效的 PPTX 格式: {}", path))?;

    let mut text = String::new();
    let mut slide_count = 0;

    // Iterate through all slides
    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let name = file.name().to_string();

        // Only process slide XML files
        if name.starts_with("ppt/slides/slide") && name.ends_with(".xml") {
            slide_count += 1;
            text.push_str(&format!("--- 幻灯片 {} ---\n", slide_count));
            
            let mut content = String::new();
            file.read_to_string(&mut content)?;

            // Parse XML and extract text
            let mut reader = Reader::from_str(&content);
            reader.trim_text(true);

            let mut buf = Vec::new();
            loop {
                match reader.read_event_into(&mut buf) {
                    Ok(Event::Text(e)) => {
                        if let Ok(txt) = e.unescape() {
                            let trimmed = txt.trim();
                            if !trimmed.is_empty() {
                                text.push_str(trimmed);
                                text.push('\n');
                            }
                        }
                    }
                    Ok(Event::Eof) => break,
                    Err(e) => return Err(anyhow::anyhow!("XML 解析错误: {}", e)),
                    _ => {}
                }
                buf.clear();
            }
            text.push('\n');
        }
    }

    if text.is_empty() {
        text = "（演示文稿为空或无法提取文本内容）".to_string();
    }

    Ok(text)
}

pub fn write_powerpoint(path: &str, content: &str) -> Result<()> {
    // For now, write as text file with .pptx extension
    // Full PPTX generation would require complex XML structure
    fs::write(path, content).context("Failed to write PowerPoint file")?;
    Ok(())
}

// Markdown parsing
pub fn parse_markdown(path: &str) -> Result<String> {
    parse_text_file(path).context("Failed to read Markdown file")
}

pub fn write_markdown(path: &str, content: &str) -> Result<()> {
    fs::write(path, content).context("Failed to write Markdown file")
}

// 通用文本文件解析函数，支持多种编码
fn parse_text_file(path: &str) -> Result<String> {
    use std::path::Path;
    
    // 打印当前工作目录和文件路径信息
    let current_dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    println!("Current working directory: {}", current_dir.display());
    println!("Attempting to read file: {}", path);
    
    // 标准化路径处理
    let file_path = Path::new(path);
    
    // 如果是绝对路径，直接使用
    let final_path = if file_path.is_absolute() {
        file_path.to_path_buf()
    } else {
        // 相对路径，尝试多种解析方式
        let possible_paths = vec![
            current_dir.join(path),                                    // 相对于当前目录
            current_dir.parent().unwrap_or(&current_dir).join(path),   // 相对于父目录（项目根目录）
        ];
        
        let mut found_path = None;
        for candidate in &possible_paths {
            println!("Trying path: {}", candidate.display());
            if candidate.exists() && candidate.is_file() {
                println!("Found valid file at: {}", candidate.display());
                found_path = Some(candidate.clone());
                break;
            }
        }
        
        match found_path {
            Some(path) => path,
            None => {
                let error_msg = format!(
                    "File not found: '{}'. Tried paths:\n{}",
                    path,
                    possible_paths.iter()
                        .map(|p| format!("  - {}", p.display()))
                        .collect::<Vec<_>>()
                        .join("\n")
                );
                return Err(anyhow::anyhow!("{}", error_msg));
            }
        }
    };

    println!("Using final path: {}", final_path.display());

    // 尝试读取文件，提供更详细的错误信息
    match std::fs::read_to_string(&final_path) {
        Ok(content) => {
            println!("Successfully read file: {} ({} bytes)", final_path.display(), content.len());
            Ok(content)
        },
        Err(e) => {
            println!("Failed to read file as UTF-8: {}, trying alternative encodings...", e);
            // 尝试以字节方式读取，然后转换为字符串
            match std::fs::read(&final_path) {
                Ok(bytes) => {
                    println!("Read {} bytes from file: {}", bytes.len(), final_path.display());
                    // 尝试不同的编码
                    if let Ok(content) = String::from_utf8(bytes.clone()) {
                        println!("Successfully decoded as UTF-8");
                        Ok(content)
                    } else {
                        println!("UTF-8 decode failed, trying GBK...");
                        // 尝试 GBK 编码（中文 Windows 常用）
                        match encoding_rs::GBK.decode(&bytes) {
                            (content, _, false) => {
                                println!("Successfully decoded as GBK");
                                Ok(content.into_owned())
                            },
                            _ => {
                                println!("GBK decode failed, using lossy UTF-8...");
                                // 如果都失败，使用 UTF-8 lossy 转换
                                Ok(String::from_utf8_lossy(&bytes).into_owned())
                            }
                        }
                    }
                }
                Err(read_err) => {
                    Err(anyhow::anyhow!(
                        "Failed to read file '{}': {} (original UTF-8 error: {})", 
                        final_path.display(), read_err, e
                    ))
                }
            }
        }
    }
}

// PDF parsing with OCR fallback
pub fn parse_pdf(path: &str) -> Result<String> {
    use pdf_extract::extract_text;
    use std::panic;
    
    println!("Attempting to parse PDF: {}", path);
    
    // 使用 catch_unwind 捕获 panic
    let result = panic::catch_unwind(|| {
        extract_text(path)
    });
    
    match result {
        Ok(Ok(text)) => {
            println!("Successfully extracted {} characters from PDF", text.len());
            
            if text.trim().is_empty() {
                println!("Warning: PDF contains no extractable text, trying OCR...");
                // 文本为空，尝试 OCR
                parse_pdf_with_python_ocr(path)
            } else {
                println!("First 200 chars: {}", &text.chars().take(200).collect::<String>());
                Ok(text)
            }
        }
        Ok(Err(e)) => {
            println!("Failed to extract text from PDF: {:?}, trying OCR...", e);
            // 提取失败，尝试 OCR
            parse_pdf_with_python_ocr(path)
        }
        Err(panic_err) => {
            println!("PDF parsing panicked: {:?}, trying OCR...", panic_err);
            // Panic 发生，尝试 OCR
            parse_pdf_with_python_ocr(path)
        }
    }
}

// OCR-based PDF parsing using Python script or bundled executable
fn parse_pdf_with_python_ocr(path: &str) -> Result<String> {
    println!("Starting OCR processing for PDF: {}", path);
    println!("Note: OCR processing may take 30-60 seconds");
    
    // 获取应用目录和应用数据目录
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| std::path::PathBuf::from("."));

    if let Some((python_runtime, script_path)) = crate::commands::ocr::resolve_ocr_runtime_from_env() {
        println!("Using OCR runtime: {}", python_runtime.display());
        return run_ocr_command(
            &python_runtime.to_string_lossy(),
            &[&script_path.to_string_lossy(), path],
            300  // 5 分钟超时
        );
    }

    // 方案 2: 使用打包的 exe（仅 Windows）
    #[cfg(target_os = "windows")]
    {
        let bundled_exe = exe_dir.join("pdf_ocr.exe");
        if bundled_exe.exists() {
            println!("Using bundled OCR executable: {}", bundled_exe.display());
            return run_ocr_command(&bundled_exe.to_string_lossy(), &[path], 300);
        }
    }

    // 方案 3: 开发环境，使用项目中的脚本
    let dev_script = if cfg!(debug_assertions) {
        std::path::PathBuf::from("scripts/pdf_ocr.py")
    } else {
        exe_dir.join("scripts").join("pdf_ocr.py")
    };
    
    if dev_script.exists() {
        println!("Using development OCR script: {}", dev_script.display());
        
        // 尝试系统 Python
        let python_commands = vec!["python", "python3", "py"];
        
        for python_cmd in &python_commands {
            println!("Trying Python command: {}", python_cmd);
            
            match run_ocr_command(python_cmd, &[&dev_script.to_string_lossy(), path], 300) {  // 5 分钟超时
                Ok(text) => return Ok(text),
                Err(e) => {
                    println!("Failed with {}: {}", python_cmd, e);
                }
            }
        }
        
        // 系统 Python 失败 - 提示下载 OCR 包
        return Err(anyhow::anyhow!(
            "⚠️ 检测到扫描版 PDF，需要 OCR 功能\n\n\
            OCR 依赖未安装。请选择以下方式之一：\n\n\
            方法 1：自动安装 PDF 提取运行时（推荐）\n\
            • 点击下载 OCR 依赖按钮\n\
            • Windows 会下载嵌入式 Python\n\
            • macOS 会创建本地 Python venv 并安装 PyMuPDF\n\n\
            方法 2：手动安装 Python 环境\n\
            1. 安装 Python 3.9+ (https://www.python.org/)\n\
            2. 运行命令: pip install PyMuPDF\n\
            3. 重新处理文件\n\n\
            当前轻量运行时优先支持 PDF 文本提取；若是纯图片型 PDF，请先准备更完整的 OCR 环境。\
            "
        ));
    }
    
    // 所有方案都不可用 - 提示下载
    Err(anyhow::anyhow!(
        "⚠️ OCR 功能未安装\n\n\
        检测到扫描版 PDF，需要下载 OCR 依赖。\n\n\
        解决方案：\n\n\
        方法 1：自动安装 OCR 依赖（推荐）\n\
        • 应用会按当前平台自动准备运行时\n\
        • 下载或初始化完成后即可重新处理文件\n\n\
        方法 2：手动安装 Python OCR 环境\n\
        1. 安装 Python 3.9+ (https://www.python.org/)\n\
        2. 运行命令: pip install PyMuPDF\n\
        3. 重新尝试处理文件\n\n\
        当前轻量运行时优先支持 PDF 文本提取；若仍无法提取内容，请补充图片型 OCR 环境后再试。\
        "
    ))
}

// 运行 OCR 命令，支持实时输出和超时控制
fn run_ocr_command(command: &str, args: &[&str], timeout_secs: u64) -> Result<String> {
    use std::process::{Command, Stdio};
    use std::time::Duration;
    use std::thread;
    use std::sync::mpsc;
    use std::io::{BufRead, BufReader};
    
    let (tx, rx) = mpsc::channel();
    let command_clone = command.to_string();
    let args_clone: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    
    // 在新线程中执行命令
    thread::spawn(move || {
        let mut child = match Command::new(&command_clone)
            .args(&args_clone)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
        {
            Ok(c) => c,
            Err(e) => {
                let _ = tx.send(Err(anyhow::anyhow!("Failed to spawn process: {}", e)));
                return;
            }
        };
        
        // 读取 stderr（进度信息）
        if let Some(stderr) = child.stderr.take() {
            let reader = BufReader::new(stderr);
            thread::spawn(move || {
                for line in reader.lines() {
                    if let Ok(line) = line {
                        println!("OCR: {}", line);
                    }
                }
            });
        }
        
        // 等待进程完成
        match child.wait_with_output() {
            Ok(output) => {
                if output.status.success() {
                    let text = String::from_utf8_lossy(&output.stdout).to_string();
                    if text.trim().is_empty() {
                        let _ = tx.send(Err(anyhow::anyhow!("OCR returned empty result")));
                    } else {
                        println!("OCR completed: {} characters extracted", text.len());
                        let _ = tx.send(Ok(text));
                    }
                } else {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    let _ = tx.send(Err(anyhow::anyhow!("OCR failed: {}", stderr)));
                }
            }
            Err(e) => {
                let _ = tx.send(Err(anyhow::anyhow!("Failed to wait for process: {}", e)));
            }
        }
    });
    
    // 等待结果，带超时
    match rx.recv_timeout(Duration::from_secs(timeout_secs)) {
        Ok(result) => result,
        Err(_) => Err(anyhow::anyhow!("OCR processing timed out after {} seconds", timeout_secs)),
    }
}
