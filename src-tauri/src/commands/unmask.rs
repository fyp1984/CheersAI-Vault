use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::fs;

use engine_core::{decode_cmap, restore_markdown, MappingEntry};

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

    // 1. Read cmap bytes and decode via engine-core (shared)
    let cmap_bytes = fs::read(&options.mapping_file_path)
        .map_err(|e| format!("读取对照文件失败: {}", e))?;
    let (mappings, _version) = decode_cmap(&cmap_bytes, &options.passphrase)
        .map_err(|e| normalize_mapping_error(e.error_code()))?;

    // 2. Read masked file
    let content = fs::read_to_string(&options.masked_file_path)
        .map_err(|e| format!("读取已脱敏文件失败: {}", e))?;

    // 3. Restore using shared engine-core function
    let (restored_content, restored_count) = restore_markdown(&content, &mappings);

    // 4. Write restored file
    fs::write(&options.output_path, &restored_content)
        .map_err(|e| format!("写入恢复文件失败: {}", e))?;

    let final_output_path = restore_output_filename(&options.output_path, &mappings)?;

    Ok(UnmaskResult {
        output_path: final_output_path,
        restored_count,
    })
}

fn restore_output_filename(
    output_path: &str,
    mappings: &[MappingEntry],
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

fn normalize_mapping_error(code: &str) -> String {
    match code {
        "CMAP_AUTH_FAILED" => "解密失败：口令不正确或对照文件已损坏".to_string(),
        "CMAP_VERSION_UNSUPPORTED" => "对照文件版本不支持或已损坏".to_string(),
        _ => format!("对照文件处理失败: {}", code),
    }
}

fn restore_filename(
    masked_filename: &str,
    mappings: &[MappingEntry],
) -> String {
    let (restored, _) = restore_markdown(masked_filename, mappings);
    restored
}
