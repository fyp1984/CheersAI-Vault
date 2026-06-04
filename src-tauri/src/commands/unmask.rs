use serde::{Deserialize, Serialize};
use crate::core::{crypto, file_parser};
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize, Deserialize)]
pub struct UnmaskFileOptions {
    pub masked_file_path: String,
    pub mapping_file_path: String,
    pub passphrase: String,
    pub output_path: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UnmaskResult {
    pub output_path: String,
    pub restored_count: usize,
}

#[tauri::command]
pub async fn unmask_file(options: UnmaskFileOptions) -> Result<UnmaskResult, String> {
    if options.masked_file_path.trim().is_empty() {
        return Err("请选择需要反脱敏的文件".to_string());
    }
    if options.mapping_file_path.trim().is_empty() {
        return Err("请选择对应的 .cmap 对照文件".to_string());
    }
    if options.output_path.trim().is_empty() {
        return Err("请选择反脱敏输出路径".to_string());
    }
    if !Path::new(&options.masked_file_path).is_file() {
        return Err(format!("已脱敏文件不存在: {}", options.masked_file_path));
    }
    if !Path::new(&options.mapping_file_path).is_file() {
        return Err(format!("对照文件不存在: {}", options.mapping_file_path));
    }

    // 1. 解密/读取对照文件。失败只返回错误，不让 Tauri 进程退出。
    let mappings = crypto::load_encrypted_mapping(&options.mapping_file_path, &options.passphrase)
        .map_err(|e| normalize_mapping_error(&e))?;
    

    // 2. 读取已脱敏的文件内容
    // 注意：当前脱敏结果统一保存为 Markdown。为了避免 Word/PDF/PPT 等原生解析器
    // 在反脱敏失败场景中触发进程级崩溃，这里只对 CSV/Excel 走表格解析，
    // 其他格式全部按文本读取。
    let format = file_parser::detect_format(&options.masked_file_path);

    let mut restored_count = 0usize;

    match format {
        file_parser::FileFormat::Csv => {

            let (headers, rows) = file_parser::parse_csv(&options.masked_file_path)
                .map_err(|e| format!("Failed to parse CSV: {}", e))?;

            let mut restored_rows = Vec::new();
            for row in rows {
                let mut restored_row = Vec::new();
                for cell in row {
                    let (restored, count) = restore_value(&cell, &mappings);
                    restored_count += count;
                    restored_row.push(restored);
                }
                restored_rows.push(restored_row);
            }

            file_parser::write_csv(&options.output_path, &headers, &restored_rows)
                .map_err(|e| format!("Failed to write CSV: {}", e))?;
        }
        file_parser::FileFormat::Excel => {

            let (headers, rows) = file_parser::parse_excel(&options.masked_file_path)
                .map_err(|e| format!("Failed to parse Excel: {}", e))?;

            let mut restored_rows = Vec::new();
            for row in rows {
                let mut restored_row = Vec::new();
                for cell in row {
                    let (restored, count) = restore_value(&cell, &mappings);
                    restored_count += count;
                    restored_row.push(restored);
                }
                restored_rows.push(restored_row);
            }

            // Excel 文件反脱敏后保存为 CSV（因为没有 Excel 写入库）
            file_parser::write_csv(&options.output_path, &headers, &restored_rows)
                .map_err(|e| format!("Failed to write CSV: {}", e))?;
        }
        file_parser::FileFormat::Markdown
        | file_parser::FileFormat::Text
        | file_parser::FileFormat::Word
        | file_parser::FileFormat::PowerPoint
        | file_parser::FileFormat::Pdf
        | file_parser::FileFormat::Json => {
            let content = file_parser::parse_markdown(&options.masked_file_path)
                .map_err(|e| format!("Failed to read file: {}", e))?;

            let (restored_content, count) = restore_value(&content, &mappings);
            restored_count = count;

            file_parser::write_markdown(&options.output_path, &restored_content)
                .map_err(|e| format!("Failed to write file: {}", e))?;
        }
    }

    let final_output_path = restore_output_filename(&options.output_path, &mappings)?;

    Ok(UnmaskResult {
        output_path: final_output_path,
        restored_count,
    })
}

/// 将文本中的脱敏值替换回原始值
fn restore_value(
    masked_text: &str,
    mappings: &[crate::core::masking_engine::MappingEntry],
) -> (String, usize) {
    let mut result = masked_text.to_string();
    let mut count = 0usize;

    // 按照 masked 值的长度降序排序，避免短的替换影响长的
    let mut sorted_mappings = mappings.to_vec();
    sorted_mappings.sort_by(|a, b| b.masked.len().cmp(&a.masked.len()));

    for entry in sorted_mappings {
        if result.contains(&entry.masked) {
            let occurrences = result.matches(&entry.masked).count();
            result = result.replace(&entry.masked, &entry.original);
            count += occurrences;
        }
    }

    (result, count)
}

fn restore_output_filename(
    output_path: &str,
    mappings: &[crate::core::masking_engine::MappingEntry],
) -> Result<String, String> {
    let output_path_obj = Path::new(output_path);
    let parent_dir = output_path_obj.parent().unwrap_or_else(|| Path::new("."));
    let filename = output_path_obj.file_name().unwrap_or_default().to_string_lossy();
    let filename_str = filename.as_ref();

    let (name_part, ext_part) = if let Some(dot_pos) = filename_str.rfind('.') {
        (&filename_str[..dot_pos], &filename_str[dot_pos..])
    } else {
        (filename_str, "")
    };

    let restored_name = sanitize_restored_filename(&restore_filename(name_part, mappings));
    let final_filename = format!("{}{}", restored_name, ext_part);
    let final_output_path_buf: PathBuf = parent_dir.join(&final_filename);
    let final_output_path = final_output_path_buf.to_string_lossy().to_string();

    if final_output_path != output_path {
        std::fs::rename(output_path, &final_output_path)
            .map_err(|e| format!("Failed to rename file: {}", e))?;
    }

    Ok(final_output_path)
}

fn sanitize_restored_filename(filename: &str) -> String {
    let sanitized: String = filename
        .chars()
        .map(|ch| match ch {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            ch if ch.is_control() => '_',
            ch => ch,
        })
        .collect();

    let trimmed = sanitized.trim().trim_matches('.').to_string();
    if trimmed.is_empty() {
        "restored".to_string()
    } else {
        trimmed
    }
}

fn normalize_mapping_error(error: &str) -> String {
    if error.contains("Decryption failed") || error.contains("wrong passphrase") {
        "解密失败：口令不正确，请确认输入的口令与创建对照文件时一致".to_string()
    } else if error.contains("Invalid magic bytes")
        || error.contains("Data too short")
        || error.contains("deserialize")
        || error.contains("Invalid UTF-8")
    {
        "对照文件格式错误或已损坏，请确认选择了正确的 .cmap 文件".to_string()
    } else {
        error.to_string()
    }
}

/// 还原文件名中的脱敏标签
fn restore_filename(
    masked_filename: &str,
    mappings: &[crate::core::masking_engine::MappingEntry],
) -> String {
    let mut result = masked_filename.to_string();

    // 按照 masked 值的长度降序排序
    let mut sorted_mappings = mappings.to_vec();
    sorted_mappings.sort_by(|a, b| b.masked.len().cmp(&a.masked.len()));

    for entry in sorted_mappings {
        if result.contains(&entry.masked) {
            result = result.replace(&entry.masked, &entry.original);
        }
    }

    result
}
