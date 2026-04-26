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
    println!("=== UNMASK_FILE START ===");
    println!("Masked file: {}", options.masked_file_path);
    println!("Mapping file: {}", options.mapping_file_path);
    println!("Output path: {}", options.output_path);

    // 1. 解密对照文件
    println!("Loading and decrypting mapping file...");
    let mappings = crypto::load_encrypted_mapping(&options.mapping_file_path, &options.passphrase)
        .map_err(|e| {
            println!("Failed to decrypt mapping: {}", e);
            e
        })?;
    
    println!("Loaded {} mapping entries", mappings.len());

    // 2. 读取已脱敏的文件内容
    let format = file_parser::detect_format(&options.masked_file_path);
    println!("Detected format: {:?}", format);

    let mut restored_count = 0usize;

    match format {
        file_parser::FileFormat::Csv => {
            println!("Processing CSV file...");
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
        file_parser::FileFormat::Markdown | file_parser::FileFormat::Text => {
            println!("Processing Text/Markdown file...");
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

    println!("=== UNMASK_FILE SUCCESS ===");
    println!("Restored count: {}", restored_count);

    Ok(UnmaskResult {
        output_path: options.output_path,
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
