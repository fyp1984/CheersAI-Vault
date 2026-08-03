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
    parse_word_with_range(path, None)
}

// Word document parsing with page range support
pub fn parse_word_with_range(path: &str, page_range: Option<(usize, usize)>) -> Result<String> {
    use zip::ZipArchive;

    // 检查文件是否存在
    if !std::path::Path::new(path).exists() {
        return Err(anyhow::anyhow!("Word file not found: {}", path));
    }

    // 检查文件扩展名
    let lower_path = path.to_lowercase();
    if lower_path.ends_with(".doc") {
        return Err(anyhow::anyhow!("旧版 .doc 格式暂不支持，请使用 .docx 格式"));
    }

    if page_range.is_none() {
        let bytes = fs::read(path).context("无法读取 Word 文件")?;
        return engine_core::parse_document(&bytes, engine_core::DocumentFormat::Docx)
            .map(|parsed| parsed.markdown)
            .map_err(|error| anyhow::anyhow!("{}: {}", error.code, error.message));
    }

    let file = fs::File::open(path)
        .with_context(|| format!("无法打开 Word 文件: {}", path))?;
    
    let mut archive = ZipArchive::new(file)
        .with_context(|| format!("无法读取 DOCX 文件，可能文件已损坏或不是有效的 DOCX 格式: {}", path))?;

    let app_page_count = match archive.by_name("docProps/app.xml") {
        Ok(mut app_file) => {
            let mut app_content = String::new();
            app_file.read_to_string(&mut app_content).ok();
            extract_docx_app_page_count(&app_content).ok().flatten()
        }
        Err(_) => None,
    };

    // Try to read document.xml which contains the main content
    let mut content = String::new();
    match archive.by_name("word/document.xml") {
        Ok(mut doc_file) => {
            doc_file.read_to_string(&mut content)
                .context("无法读取文档内容")?;
        }
        Err(e) => {
            return Err(anyhow::anyhow!("DOCX 文件结构异常，找不到 word/document.xml: {}", e));
        }
    }

    // 如果没有指定页码范围，提取全文
    if page_range.is_none() {
        return extract_word_text_simple(&content);
    }

    // 指定了页码范围，按分页符分割
    let (start_page, end_page) = page_range.unwrap();
    extract_word_text_by_pages(&content, start_page, end_page, app_page_count)
}

fn extract_docx_app_page_count(xml_content: &str) -> Result<Option<usize>> {
    use quick_xml::events::Event;
    use quick_xml::Reader;

    let mut reader = Reader::from_str(xml_content);
    reader.trim_text(true);

    let mut buf = Vec::new();
    let mut in_pages = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                in_pages = e.name().as_ref() == b"Pages";
            }
            Ok(Event::Text(e)) if in_pages => {
                if let Ok(txt) = e.unescape() {
                    if let Ok(page_count) = txt.trim().parse::<usize>() {
                        if page_count > 0 {
                            return Ok(Some(page_count));
                        }
                    }
                }
            }
            Ok(Event::End(e)) => {
                if e.name().as_ref() == b"Pages" {
                    in_pages = false;
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(anyhow::anyhow!("XML 解析错误: {}", e)),
            _ => {}
        }
        buf.clear();
    }

    Ok(None)
}

// 简单提取 Word 文本（不考虑分页）
fn extract_word_text_simple(xml_content: &str) -> Result<String> {
    use quick_xml::Reader;
    use quick_xml::events::Event;

    let mut text = String::new();
    let mut reader = Reader::from_str(xml_content);
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

    if text.is_empty() {
        text = "（文档为空或无法提取文本内容）".to_string();
    }

    Ok(text)
}

// 按分页符提取 Word 文本；没有显式分页符时，按 Word 保存的总页数近似分段。
fn extract_word_text_by_pages(
    xml_content: &str,
    start_page: usize,
    end_page: usize,
    expected_page_count: Option<usize>,
) -> Result<String> {
    use quick_xml::Reader;
    use quick_xml::events::Event;

    // 验证页码范围
    if start_page < 1 {
        return Err(anyhow::anyhow!("起始页必须 >= 1"));
    }
    
    if start_page > end_page {
        return Err(anyhow::anyhow!("起始页不能大于结束页"));
    }

    let mut reader = Reader::from_str(xml_content);
    reader.trim_text(true);

    let mut pages: Vec<String> = Vec::new();
    let mut current_page_text = String::new();
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                // 检测分页符：<w:br w:type="page"/>
                if e.name().as_ref() == b"w:br" {
                    for attr in e.attributes() {
                        if let Ok(attr) = attr {
                            if attr.key.as_ref() == b"w:type" && attr.value.as_ref() == b"page" {
                                // 保存当前页内容
                                pages.push(current_page_text.clone());
                                current_page_text.clear();
                                break;
                            }
                        }
                    }
                }
            }
            Ok(Event::Empty(e)) => {
                // Word often writes page breaks as a self-closing <w:br w:type="page"/>.
                if e.name().as_ref() == b"w:br" {
                    for attr in e.attributes() {
                        if let Ok(attr) = attr {
                            if attr.key.as_ref() == b"w:type" && attr.value.as_ref() == b"page" {
                                pages.push(current_page_text.clone());
                                current_page_text.clear();
                                break;
                            }
                        }
                    }
                }
            }
            Ok(Event::Text(e)) => {
                if let Ok(txt) = e.unescape() {
                    current_page_text.push_str(&txt);
                    current_page_text.push('\n');
                }
            }
            Ok(Event::Eof) => {
                // 保存最后一页
                if !current_page_text.is_empty() || pages.is_empty() {
                    pages.push(current_page_text);
                }
                break;
            }
            Err(e) => return Err(anyhow::anyhow!("XML 解析错误: {}", e)),
            _ => {}
        }
        buf.clear();
    }

    // 如果没有检测到任何正文，将整个文档作为一页再走一次全文提取兜底。
    if pages.iter().all(|page| page.trim().is_empty()) {
        return extract_word_text_simple(xml_content);
    }

    if pages.len() == 1 {
        if let Some(expected_pages) = expected_page_count {
            if expected_pages > 1 {
                let full_text = extract_word_text_simple(xml_content)?;
                pages = split_text_into_estimated_pages(&full_text, expected_pages);
            }
        }
    }

    let total_pages = pages.len();
    
    // 调整页码范围
    let actual_end = end_page.min(total_pages);
    
    if start_page > total_pages {
        return Err(anyhow::anyhow!(
            "起始页 {} 超出文档总页数 {}",
            start_page,
            total_pages
        ));
    }

    // 提取指定页码范围的内容
    let mut result = String::new();
    for page_num in start_page..=actual_end {
        let page_index = page_num - 1; // 转换为 0 索引
        if let Some(page_content) = pages.get(page_index) {
            result.push_str(&format!("--- 第 {} 页 ---\n", page_num));
            result.push_str(page_content);
            result.push('\n');
        }
    }

    if result.is_empty() {
        result = "（指定页码范围内无内容）".to_string();
    }

    Ok(result)
}

fn split_text_into_estimated_pages(text: &str, total_pages: usize) -> Vec<String> {
    let lines: Vec<&str> = text.lines().collect();
    if total_pages <= 1 || lines.is_empty() {
        return vec![text.to_string()];
    }

    let mut pages = Vec::with_capacity(total_pages);
    for page_index in 0..total_pages {
        let start = page_index * lines.len() / total_pages;
        let end = (page_index + 1) * lines.len() / total_pages;
        let page_text = if start < end {
            lines[start..end].join("\n")
        } else {
            String::new()
        };
        pages.push(page_text);
    }

    pages
}

pub fn write_word(path: &str, content: &str) -> Result<()> {
    // For simplicity, write as plain text with .docx extension
    // Users can open in Word and it will be imported as text
    fs::write(path, content).context("Failed to write Word file")?;
    Ok(())
}

// PowerPoint parsing (PPTX)
pub fn parse_powerpoint(path: &str) -> Result<String> {
    parse_powerpoint_with_range(path, None)
}

// PowerPoint parsing with page range support (slides)
pub fn parse_powerpoint_with_range(path: &str, page_range: Option<(usize, usize)>) -> Result<String> {
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

    // 收集所有幻灯片
    let mut slides: Vec<(usize, String)> = Vec::new();
    
    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let name = file.name().to_string();

        // 只处理幻灯片 XML 文件
        if name.starts_with("ppt/slides/slide") && name.ends_with(".xml") {
            // 提取幻灯片编号
            let slide_num = name
                .trim_start_matches("ppt/slides/slide")
                .trim_end_matches(".xml")
                .parse::<usize>()
                .unwrap_or(0);
            
            let mut content = String::new();
            file.read_to_string(&mut content)?;

            // 解析 XML 并提取文本
            let mut slide_text = String::new();
            let mut reader = Reader::from_str(&content);
            reader.trim_text(true);

            let mut buf = Vec::new();
            loop {
                match reader.read_event_into(&mut buf) {
                    Ok(Event::Text(e)) => {
                        if let Ok(txt) = e.unescape() {
                            let trimmed = txt.trim();
                            if !trimmed.is_empty() {
                                slide_text.push_str(trimmed);
                                slide_text.push('\n');
                            }
                        }
                    }
                    Ok(Event::Eof) => break,
                    Err(e) => return Err(anyhow::anyhow!("XML 解析错误: {}", e)),
                    _ => {}
                }
                buf.clear();
            }
            
            slides.push((slide_num, slide_text));
        }
    }

    // 按幻灯片编号排序
    slides.sort_by_key(|(num, _)| *num);

    if slides.is_empty() {
        return Ok("（演示文稿为空或无法提取文本内容）".to_string());
    }

    // 如果没有指定页码范围，返回所有幻灯片
    if page_range.is_none() {
        let mut text = String::new();
        for (slide_num, slide_text) in slides {
            text.push_str(&format!("--- 幻灯片 {} ---\n", slide_num));
            text.push_str(&slide_text);
            text.push('\n');
        }
        return Ok(text);
    }

    // 指定了页码范围，提取指定幻灯片
    let (start_page, end_page) = page_range.unwrap();
    
    // 验证页码范围
    if start_page < 1 {
        return Err(anyhow::anyhow!("起始页必须 >= 1"));
    }
    
    if start_page > end_page {
        return Err(anyhow::anyhow!("起始页不能大于结束页"));
    }

    let total_slides = slides.len();
    let actual_end = end_page.min(total_slides);
    
    if start_page > total_slides {
        return Err(anyhow::anyhow!(
            "起始页 {} 超出演示文稿总页数 {}",
            start_page,
            total_slides
        ));
    }

    // 提取指定范围的幻灯片
    let mut text = String::new();
    for i in (start_page - 1)..actual_end {
        if let Some((slide_num, slide_text)) = slides.get(i) {
            text.push_str(&format!("--- 幻灯片 {} ---\n", slide_num));
            text.push_str(slide_text);
            text.push('\n');
        }
    }

    if text.is_empty() {
        text = "（指定页码范围内无内容）".to_string();
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
            if candidate.exists() && candidate.is_file() {
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


    // 尝试读取文件，提供更详细的错误信息
    match std::fs::read_to_string(&final_path) {
        Ok(content) => {
            Ok(content)
        },
        Err(e) => {

            // 尝试以字节方式读取，然后转换为字符串
            match std::fs::read(&final_path) {
                Ok(bytes) => {
                    // 尝试不同的编码
                    if let Ok(content) = String::from_utf8(bytes.clone()) {

                        Ok(content)
                    } else {

                        // 尝试 GBK 编码（中文 Windows 常用）
                        match encoding_rs::GBK.decode(&bytes) {
                            (content, _, false) => {

                                Ok(content.into_owned())
                            },
                            _ => {

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
    parse_pdf_with_range(path, None)
}

// 获取文件的总页数
pub fn get_page_count(path: &str) -> Result<usize> {
    let format = detect_format(path);
    
    match format {
        FileFormat::Pdf => {
            use lopdf::Document;
            
            match Document::load(path) {
                Ok(doc) => {
                    let page_count = doc.get_pages().len();
                    Ok(page_count)
                }
                Err(e) => {
                    // 如果 lopdf 失败，尝试使用 pdf-extract
                    Err(anyhow::anyhow!("无法读取 PDF 页数: {}", e))
                }
            }
        }
        FileFormat::Word => {
            use zip::ZipArchive;
            use quick_xml::Reader;
            use quick_xml::events::Event;

            let file = fs::File::open(path)
                .context("无法打开 Word 文件")?;
            
            let mut archive = ZipArchive::new(file)
                .context("无法读取 DOCX 文件")?;

            if let Ok(mut app_file) = archive.by_name("docProps/app.xml") {
                let mut app_content = String::new();
                if app_file.read_to_string(&mut app_content).is_ok() {
                    if let Some(page_count) = extract_docx_app_page_count(&app_content)? {
                        return Ok(page_count);
                    }
                }
            }

            let mut content = String::new();
            match archive.by_name("word/document.xml") {
                Ok(mut doc_file) => {
                    doc_file.read_to_string(&mut content)
                        .context("无法读取文档内容")?;
                }
                Err(e) => {
                    return Err(anyhow::anyhow!("DOCX 文件结构异常: {}", e));
                }
            }

            // 计算分页符数量
            let mut reader = Reader::from_str(&content);
            reader.trim_text(true);
            
            let mut page_count = 1; // 至少有一页
            let mut buf = Vec::new();
            
            loop {
                match reader.read_event_into(&mut buf) {
                    Ok(Event::Start(e)) => {
                        if e.name().as_ref() == b"w:br" {
                            for attr in e.attributes() {
                                if let Ok(attr) = attr {
                                    if attr.key.as_ref() == b"w:type" && attr.value.as_ref() == b"page" {
                                        page_count += 1;
                                        break;
                                    }
                                }
                            }
                        }
                    }
                    Ok(Event::Empty(e)) => {
                        if e.name().as_ref() == b"w:br" {
                            for attr in e.attributes() {
                                if let Ok(attr) = attr {
                                    if attr.key.as_ref() == b"w:type" && attr.value.as_ref() == b"page" {
                                        page_count += 1;
                                        break;
                                    }
                                }
                            }
                        }
                    }
                    Ok(Event::Eof) => break,
                    Err(e) => return Err(anyhow::anyhow!("XML 解析错误: {}", e)),
                    _ => {}
                }
                buf.clear();
            }
            
            Ok(page_count)
        }
        FileFormat::PowerPoint => {
            use zip::ZipArchive;

            let file = fs::File::open(path)
                .context("无法打开 PowerPoint 文件")?;
            
            let mut archive = ZipArchive::new(file)
                .context("无法读取 PPTX 文件")?;

            let mut slide_count = 0;
            
            for i in 0..archive.len() {
                let file = archive.by_index(i)?;
                let name = file.name();
                
                if name.starts_with("ppt/slides/slide") && name.ends_with(".xml") {
                    slide_count += 1;
                }
            }
            
            Ok(slide_count)
        }
        _ => {
            // 不支持分页的格式
            Err(anyhow::anyhow!("该文件格式不支持分页"))
        }
    }
}

// PDF parsing with page range support
pub fn parse_pdf_with_range(path: &str, page_range: Option<(usize, usize)>) -> Result<String> {
    // 调试日志
    eprintln!("🔍 parse_pdf_with_range called with page_range: {:?}", page_range);

    // 如果指定了页码范围，优先使用 Python/PyMuPDF 提取。
    // lopdf 在部分中文 PDF 字体编码上可能触发原生访问违规，导致整个 Tauri 进程崩溃。
    if let Some((start_page, end_page)) = page_range {
        if start_page < 1 {
            return Err(anyhow::anyhow!("起始页必须 >= 1"));
        }

        if start_page > end_page {
            return Err(anyhow::anyhow!("起始页不能大于结束页"));
        }

        eprintln!("✅ Using PyMuPDF for page range: {}-{}", start_page, end_page);
        return parse_pdf_with_python_ocr_range(path, Some((start_page, end_page)));
    }
    
    eprintln!("⚠️ No page range specified, extracting full document");
    let bytes = fs::read(path).context("无法读取 PDF 文件")?;
    match engine_core::parse_document(&bytes, engine_core::DocumentFormat::Pdf) {
        Ok(parsed) => Ok(parsed.markdown),
        Err(error) if error.code == "OCR_COMPONENT_REQUIRED" => parse_pdf_with_python_ocr(path),
        Err(error) => Err(anyhow::anyhow!("{}: {}", error.code, error.message)),
    }
}

// 使用 lopdf 按页提取 PDF 内容
fn parse_pdf_pages_with_lopdf(path: &str, start_page: usize, end_page: usize) -> Result<String> {
    use lopdf::Document;
    
    // 加载 PDF 文档
    let doc = Document::load(path)
        .context("无法加载 PDF 文档")?;
    
    // 获取页数
    let page_count = doc.get_pages().len();
    
    // 验证页码范围
    if start_page < 1 {
        return Err(anyhow::anyhow!("起始页必须 >= 1"));
    }
    
    if start_page > end_page {
        return Err(anyhow::anyhow!("起始页不能大于结束页"));
    }
    
    // 调整页码范围（如果超出实际页数）
    let actual_end = end_page.min(page_count);
    
    if start_page > page_count {
        return Err(anyhow::anyhow!(
            "起始页 {} 超出文档总页数 {}",
            start_page,
            page_count
        ));
    }
    
    
    // 提取指定页码范围的文本
    let mut content = String::new();
    let mut has_content = false;
    
    for page_num in start_page..=actual_end {
        // lopdf 的页码从 1 开始
        match doc.extract_text(&[page_num as u32]) {
            Ok(text) => {
                let trimmed = text.trim();
                // 检查是否包含 "Unimplemented" 错误
                if !trimmed.is_empty() && !trimmed.contains("Unimplemented") {
                    content.push_str(&format!("--- 第 {} 页 ---\n", page_num));
                    content.push_str(&text);
                    content.push('\n');
                    has_content = true;
                } else {
                    eprintln!("⚠️ 第 {} 页提取失败或包含不支持的编码", page_num);
                }
            }
            Err(e) => {
                eprintln!("⚠️ 无法提取第 {} 页的文本: {}", page_num, e);
                // 继续处理其他页
            }
        }
    }
    
    // 如果没有提取到有效内容，返回错误让系统尝试其他方法
    if !has_content {
        eprintln!("⚠️ lopdf 无法提取有效内容（可能是中文编码问题）");
        return Err(anyhow::anyhow!("lopdf 无法提取有效内容，可能包含不支持的字体编码"));
    }
    
    Ok(content)
}

// OCR-based PDF parsing using the shared component-runtime crate.
fn parse_pdf_with_python_ocr(path: &str) -> Result<String> {
    parse_pdf_with_python_ocr_range(path, None)
}

// OCR-based PDF parsing with page range support, via component-runtime.
//
// This is the only OCR entry point on the desktop side: it uses exactly the
// same shared resolver (`ocr_config_from_env` -> `component_runtime::resolve_ocr_config`)
// as the enterprise Runtime. There is deliberately no second Python/script
// lookup here — a missing or invalid shared configuration, or a failed
// `run_ocr_blocking` call, is reported to the caller as-is instead of being
// retried against some other interpreter, so desktop and Runtime can never
// disagree about which installation actually processed a file.
fn parse_pdf_with_python_ocr_range(path: &str, page_range: Option<(usize, usize)>) -> Result<String> {
    use std::fs;

    let Some(config) = crate::commands::ocr::ocr_config_from_env() else {
        return Err(anyhow::anyhow!(
            "⚠️ OCR 功能未安装\n\n\
            检测到扫描版 PDF，需要下载 OCR 依赖。\n\n\
            请通过「增强服务」页面安装 OCR 组件后重试。"
        ));
    };

    eprintln!("🔍 OCR using configured runtime");

    let pdf_bytes = fs::read(path)
        .map_err(|e| anyhow::anyhow!("无法读取 PDF 文件: {}", e))?;

    match component_runtime::run_ocr_blocking(&config, &pdf_bytes, page_range) {
        Ok(ocr_result) => {
            let markdown = engine_core::ocr_result_to_markdown(&ocr_result);
            eprintln!("✅ OCR succeeded: {} chars, {} pages",
                markdown.len(), ocr_result.pages.len());
            Ok(markdown)
        }
        Err(ocr_error) => {
            let msg = ocr_error.to_string();
            let code = ocr_error.error_code();
            eprintln!("❌ OCR failed [{}]: {}", code, msg);
            Err(anyhow::anyhow!(
                "⚠️ 检测到扫描版 PDF，需要 OCR 功能。OCR 处理失败 [{}]: {}",
                code, msg
            ))
        }
    }
}
