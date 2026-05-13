use serde::{Deserialize, Serialize};
use crate::core::{crypto, file_parser};

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




    // 1. 解密对照文件

    let mappings = crypto::load_encrypted_mapping(&options.mapping_file_path, &options.passphrase)
        .map_err(|e| {

            e
        })?;
    

    // 2. 读取已脱敏的文件内容
    // 注意：脱敏保存时，Word/PDF/PowerPoint 都被保存为文本或 CSV 格式
    // 所以反脱敏时应该按照实际保存的格式来解析，而不是原始格式
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
        file_parser::FileFormat::Word => {

            let content = file_parser::parse_word(&options.masked_file_path)
                .map_err(|e| format!("Failed to parse Word: {}", e))?;

            let (restored_content, count) = restore_value(&content, &mappings);
            restored_count = count;

            // Word 文件反脱敏后保存为文本格式
            file_parser::write_markdown(&options.output_path, &restored_content)
                .map_err(|e| format!("Failed to write file: {}", e))?;
        }
        file_parser::FileFormat::Markdown | file_parser::FileFormat::Text | file_parser::FileFormat::Pdf => {
            // PDF 在脱敏时已经被保存为 .txt 文本格式，所以这里按文本处理

            let content = file_parser::parse_markdown(&options.masked_file_path)
                .map_err(|e| format!("Failed to read file: {}", e))?;

            let (restored_content, count) = restore_value(&content, &mappings);
            restored_count = count;

            file_parser::write_markdown(&options.output_path, &restored_content)
                .map_err(|e| format!("Failed to write file: {}", e))?;
        }
        _ => {
            return Err("Unsupported file format for unmasking".to_string());
        }
    }


    // 还原输出文件名中的脱敏标签
    let output_path_obj = std::path::Path::new(&options.output_path);
    let parent_dir = output_path_obj.parent().unwrap_or_else(|| std::path::Path::new("."));
    let filename = output_path_obj.file_name().unwrap_or_default().to_string_lossy();
    
    // 从文件名中分离扩展名
    let filename_str = filename.as_ref();
    let (name_part, ext_part) = if let Some(dot_pos) = filename_str.rfind('.') {
        (&filename_str[..dot_pos], &filename_str[dot_pos..])
    } else {
        (filename_str, "")
    };
    
    // 还原文件名中的脱敏标签
    let restored_name = restore_filename(name_part, &mappings);
    let final_filename = format!("{}{}", restored_name, ext_part);
    let final_output_path = parent_dir.join(&final_filename).to_string_lossy().to_string();
    
    // 如果还原后的文件名不同，重命名文件
    if final_output_path != options.output_path {

        std::fs::rename(&options.output_path, &final_output_path)
            .map_err(|e| format!("Failed to rename file: {}", e))?;
    }

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
