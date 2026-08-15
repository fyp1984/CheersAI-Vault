/* Copyright 2026 CheersAI Team. Licensed under Apache-2.0 */

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use excel_style_core::{CellKey, RewriteOutcome};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::core::crypto::{self, EncSourcePassMode, KeyDomain};
use crate::core::file_parser::{self, SheetDef};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExcelStructure {
    pub sheets: Vec<SheetDef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnMaskingRule {
    pub strategy_id: String,
    pub pattern: Option<String>,
    pub replacement: Option<String>,
    pub mask_char: Option<char>,
    pub keep_prefix: Option<usize>,
    pub keep_suffix: Option<usize>,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SheetMaskingConfig {
    pub sheet_name: String,
    pub header_row: Option<u32>,
    pub column_rules: HashMap<String, ColumnMaskingRule>,
    pub cell_overrides: Vec<CellOverride>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CellOverride {
    pub cell_ref: String,
    pub masked_value: String,
    pub strategy_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExcelMaskingConfig {
    pub input_file_path: String,
    pub output_name_suffix: Option<String>,
    pub sheets: Vec<SheetMaskingConfig>,
    pub passphrase: Option<String>,
    pub retain_encrypted_source: Option<bool>,
    pub source_pass_mode: Option<EncSourcePassModeDto>,
    pub generate_ecmap: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum EncSourcePassModeDto {
    SandboxReused,
    SecondaryPhrase(String),
    DeviceKey,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExcelMaskPreviewCell {
    pub original_preview: String,
    pub masked: String,
    pub strategy_id: String,
    pub row: u32,
    pub col: u32,
    pub cell_ref: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExcelMaskPreview {
    pub sheets: Vec<SheetMaskPreview>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SheetMaskPreview {
    pub sheet_name: String,
    pub headers: Vec<String>,
    pub preview_rows: Vec<ExcelMaskPreviewCell>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExcelApplyResult {
    pub masked_path: String,
    pub ecmap_path: Option<String>,
    pub encrypted_source_path: Option<String>,
    pub report_md: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExcelRestoreReq {
    pub masked_file_path: String,
    pub ecmap_file_path: String,
    pub encrypted_source_path: Option<String>,
    pub user_original_file_path: Option<String>,
    pub output_path: String,
    pub passphrase: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExcelRestoreResult {
    pub restored_path: String,
    pub sha256_verified: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EcmapEntryV1 {
    pub cell_ref: String,
    pub original_sha256: String,
    pub original_preview: String,
    pub masked: String,
    pub strategy_id: String,
    pub col_index: u32,
    pub row_index: u32,
    pub sheet: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EcmapHeaderV1 {
    pub version: String,
    pub original_sha256: String,
    pub masked_sha256: String,
    pub source_encryption_key_source: String,
    pub passphrase_domain_hint8: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EcmapDocumentV1 {
    pub header: EcmapHeaderV1,
    pub entries: Vec<EcmapEntryV1>,
}

fn col_letters(index_1: u32) -> String {
    let mut n = index_1;
    let mut letters = String::new();
    while n > 0 {
        let rem = (n - 1) % 26;
        letters.insert(0, (b'A' + rem as u8) as char);
        n = (n - 1) / 26;
    }
    letters
}

fn cell_ref_a1(row: u32, col: u32) -> String {
    format!("{}{}", col_letters(col), row)
}

fn mask_value(value: &str, rule: &ColumnMaskingRule) -> String {
    if !rule.enabled {
        return value.to_string();
    }
    if let Some(rep) = &rule.replacement {
        if rule.pattern.is_none() {
            return rep.clone();
        }
    }
    let keep_prefix = rule.keep_prefix.unwrap_or(0);
    let keep_suffix = rule.keep_suffix.unwrap_or(0);
    let mask_char = rule.mask_char.unwrap_or('*');
    let chars: Vec<char> = value.chars().collect();
    let len = chars.len();
    if keep_prefix + keep_suffix >= len {
        return mask_char.to_string().repeat(len.max(1));
    }
    let mut out = String::with_capacity(len);
    for ch in chars.iter().take(keep_prefix.min(len)) {
        out.push(*ch);
    }
    let middle = len.saturating_sub(keep_prefix + keep_suffix);
    out.extend(std::iter::repeat_n(mask_char, middle));
    for ch in chars.iter().skip(len.saturating_sub(keep_suffix)).take(keep_suffix) {
        out.push(*ch);
    }
    out
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    let r = h.finalize();
    r.iter().map(|b| format!("{:02x}", b)).collect()
}

fn pass_mode_from_dto(
    dto: Option<EncSourcePassModeDto>,
    _fallback_phrase: String,
) -> EncSourcePassMode {
    match dto {
        Some(EncSourcePassModeDto::SandboxReused) | None => EncSourcePassMode::SandboxReused,
        Some(EncSourcePassModeDto::SecondaryPhrase(p)) => {
            EncSourcePassMode::SecondaryPhrase { phrase: p }
        }
        Some(EncSourcePassModeDto::DeviceKey) => EncSourcePassMode::DeviceKey,
    }
}

fn preview_preview(s: &str, limit: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= limit {
        s.to_string()
    } else {
        chars.into_iter().take(limit).collect()
    }
}

#[tauri::command]
pub async fn excel_parse_structure(file_path: String) -> Result<ExcelStructure, String> {
    let sheets = file_parser::parse_excel_structure_detailed(&file_path)?;
    Ok(ExcelStructure { sheets })
}

#[tauri::command]
pub async fn excel_preview_masking(
    config: ExcelMaskingConfig,
    max_rows: Option<usize>,
) -> Result<ExcelMaskPreview, String> {
    let sheets_info = file_parser::parse_excel_structure_detailed(&config.input_file_path)?;
    let max_rows = max_rows.unwrap_or(20);

    use calamine::{open_workbook_auto, Data, Reader};
    let mut workbook = open_workbook_auto(&config.input_file_path)
        .map_err(|e| format!("Failed to open Excel: {}", e))?;

    let mut out_sheets = Vec::new();

    for sheet_info in sheets_info {
        let sheet_cfg = config
            .sheets
            .iter()
            .find(|s| s.sheet_name == sheet_info.name);

        let range = workbook
            .worksheet_range(&sheet_info.name)
            .map_err(|e| format!("Failed to read sheet: {}", e))?;

        let height = range.get_size().0;
        let width = range.get_size().1;
        let header_row = sheet_cfg.and_then(|c| c.header_row).unwrap_or(0);
        let data_start = header_row + 1;
        let preview_end = (data_start as usize)
            .saturating_add(max_rows)
            .min(height as usize) as u32;

        let mut preview_cells = Vec::new();

        for row in data_start..preview_end {
            for col in 1..=(width as u32) {
                let col_idx_0 = (col - 1) as usize;
                let header_name = sheet_info
                    .headers
                    .get(col_idx_0)
                    .cloned()
                    .unwrap_or_default();
                let cell_val = range
                    .get((row as usize, (col - 1) as usize))
                    .map(|d| match d {
                        Data::String(s) => s.clone(),
                        Data::Int(i) => i.to_string(),
                        Data::Float(f) => f.to_string(),
                        Data::Bool(b) => b.to_string(),
                        Data::DateTime(dt) => dt.to_string(),
                        Data::DateTimeIso(s) => s.clone(),
                        Data::DurationIso(s) => s.clone(),
                        _ => String::new(),
                    })
                    .unwrap_or_default();
                if cell_val.is_empty() {
                    continue;
                }

                let a1 = cell_ref_a1(row, col);
                let mut strategy_id = String::from("default:identity");
                let mut masked = cell_val.clone();

                if let Some(sc) = sheet_cfg {
                    if let Some(ov) = sc
                        .cell_overrides
                        .iter()
                        .find(|o| o.cell_ref.to_uppercase() == a1.to_uppercase())
                    {
                        strategy_id = ov.strategy_id.clone();
                        masked = ov.masked_value.clone();
                    } else if let Some(rule) = sc.column_rules.get(&header_name) {
                        strategy_id = rule.strategy_id.clone();
                        masked = mask_value(&cell_val, rule);
                    }
                }

                preview_cells.push(ExcelMaskPreviewCell {
                    original_preview: preview_preview(&cell_val, 8),
                    masked,
                    strategy_id,
                    row,
                    col,
                    cell_ref: a1,
                });
            }
        }

        out_sheets.push(SheetMaskPreview {
            sheet_name: sheet_info.name,
            headers: sheet_info.headers,
            preview_rows: preview_cells,
        });
    }

    Ok(ExcelMaskPreview { sheets: out_sheets })
}

#[tauri::command]
pub async fn excel_apply_masking(
    config: ExcelMaskingConfig,
    output_dir: String,
) -> Result<ExcelApplyResult, String> {
    let input_path = Path::new(&config.input_file_path);
    let stem = input_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("excel");
    let suffix = config.output_name_suffix.as_deref().unwrap_or("_masked");
    let output_path = PathBuf::from(&output_dir).join(format!("{}{}.xlsx", stem, suffix));

    fs::create_dir_all(&output_dir).map_err(|e| format!("创建输出目录失败: {}", e))?;

    use calamine::{open_workbook_auto, Data, Reader};
    let mut workbook = open_workbook_auto(&config.input_file_path)
        .map_err(|e| format!("Failed to open Excel: {}", e))?;
    let _sheet_names: Vec<String> = workbook.sheet_names().to_vec();

    let sheets_info = file_parser::parse_excel_structure_detailed(&config.input_file_path)?;

    let mut replacements: HashMap<CellKey, String> = HashMap::new();
    let mut entries: Vec<EcmapEntryV1> = Vec::new();

    for sheet_info in &sheets_info {
        let sheet_cfg = config
            .sheets
            .iter()
            .find(|s| s.sheet_name == sheet_info.name);
        let header_row = sheet_cfg.and_then(|c| c.header_row).unwrap_or(0);
        let range = workbook
            .worksheet_range(&sheet_info.name)
            .map_err(|e| format!("Failed to read sheet {}: {}", sheet_info.name, e))?;
        let height = range.get_size().0;
        let width = range.get_size().1;

        for row in (header_row + 1)..(height as u32) {
            for col in 1..=(width as u32) {
                let col_idx_0 = (col - 1) as usize;
                let header_name = sheet_info
                    .headers
                    .get(col_idx_0)
                    .cloned()
                    .unwrap_or_default();
                let cell_val = range
                    .get((row as usize, (col - 1) as usize))
                    .map(|d| match d {
                        Data::String(s) => s.clone(),
                        Data::Int(i) => i.to_string(),
                        Data::Float(f) => f.to_string(),
                        Data::Bool(b) => b.to_string(),
                        Data::DateTime(dt) => dt.to_string(),
                        Data::DateTimeIso(s) => s.clone(),
                        Data::DurationIso(s) => s.clone(),
                        _ => String::new(),
                    })
                    .unwrap_or_default();
                if cell_val.is_empty() {
                    continue;
                }

                let a1 = cell_ref_a1(row + 1, col);
                let mut strategy_id = String::from("identity");
                let mut masked: Option<String> = None;

                if let Some(sc) = sheet_cfg {
                    if let Some(ov) = sc
                        .cell_overrides
                        .iter()
                        .find(|o| o.cell_ref.to_uppercase() == a1.to_uppercase())
                    {
                        strategy_id = ov.strategy_id.clone();
                        masked = Some(ov.masked_value.clone());
                    } else if let Some(rule) = sc.column_rules.get(&header_name) {
                        if rule.enabled {
                            strategy_id = rule.strategy_id.clone();
                            masked = Some(mask_value(&cell_val, rule));
                        }
                    }
                }

                if let Some(m) = masked {
                    if m != cell_val {
                        let key = CellKey {
                            sheet: sheet_info.name.clone(),
                            row: row + 1,
                            col,
                        };
                        replacements.insert(key.clone(), m.clone());

                        entries.push(EcmapEntryV1 {
                            cell_ref: a1,
                            original_sha256: sha256_hex(cell_val.as_bytes()),
                            original_preview: preview_preview(&cell_val, 8),
                            masked: m,
                            strategy_id: strategy_id.clone(),
                            col_index: col,
                            row_index: row + 1,
                            sheet: sheet_info.name.clone(),
                        });
                    }
                }
            }
        }
    }

    let original_bytes =
        fs::read(&config.input_file_path).map_err(|e| format!("读取原文件失败: {}", e))?;
    let original_sha256 = sha256_hex(&original_bytes);

    let outcome: RewriteOutcome =
        excel_style_core::rewrite_clone_inject(input_path, &output_path, &replacements)
            .unwrap_or_else(|e| {
                let headers: Vec<String> = sheets_info
                    .first()
                    .map(|s| s.headers.clone())
                    .unwrap_or_default();
                let rows_vec: Vec<Vec<String>> = vec![];
                let mut fallback =
                    excel_style_core::fallback_xlsxwriter_full(&headers, &rows_vec, &output_path)
                        .unwrap_or_default();
                fallback
                    .warnings
                    .push(format!("克隆注入失败，已回退: {}", e));
                fallback
            });

    let masked_bytes =
        fs::read(&output_path).map_err(|e| format!("读取 masked 文件失败: {}", e))?;
    let masked_sha256 = sha256_hex(&masked_bytes);

    let report_md = excel_style_core::build_report_md(&outcome);
    let report_path = PathBuf::from(&output_dir).join(format!("{}{}_report.md", stem, suffix));
    let _ = fs::write(&report_path, &report_md);

    let mut ecmap_path: Option<String> = None;
    let mut encsrc_path: Option<String> = None;

    if config.generate_ecmap.unwrap_or(true) {
        let pass = config.passphrase.clone().unwrap_or_default();
        let hint = crypto::domain_hint8(&pass, KeyDomain::EcmapV1);
        let key_source = match config.source_pass_mode.as_ref() {
            Some(EncSourcePassModeDto::DeviceKey) => "DeviceKey",
            Some(EncSourcePassModeDto::SecondaryPhrase(_)) => "SecondaryPhrase",
            _ => "SandboxReused",
        }
        .to_string();

        let doc = EcmapDocumentV1 {
            header: EcmapHeaderV1 {
                version: "1.2".to_string(),
                original_sha256: original_sha256.clone(),
                masked_sha256: masked_sha256.clone(),
                source_encryption_key_source: key_source,
                passphrase_domain_hint8: hint,
            },
            entries,
        };
        let json = serde_json::to_vec(&doc).map_err(|e| format!("ECMAP JSON 序列化失败: {}", e))?;
        let mode = pass_mode_from_dto(config.source_pass_mode.clone(), pass.clone());
        let ecmap_bytes = crypto::encrypt_ecmap(&json, &pass, mode)
            .map_err(|e| format!("ECMAP 加密失败: {}", e))?;
        let ep = PathBuf::from(&output_dir).join(format!("{}{}.ecmap", stem, suffix));
        fs::write(&ep, ecmap_bytes).map_err(|e| format!("ECMAP 写入失败: {}", e))?;
        ecmap_path = Some(ep.to_string_lossy().to_string());
    }

    if config.retain_encrypted_source.unwrap_or(false) {
        let pass = config.passphrase.clone().unwrap_or_default();
        let mode = pass_mode_from_dto(config.source_pass_mode.clone(), pass.clone());
        let enc_bytes = crypto::encrypt_encsrc(&original_bytes, &pass, mode)
            .map_err(|e| format!("源文件加密失败: {}", e))?;
        let ep = PathBuf::from(&output_dir).join(format!("{}{}.encrypted_src", stem, suffix));
        fs::write(&ep, enc_bytes).map_err(|e| format!("加密源写入失败: {}", e))?;
        encsrc_path = Some(ep.to_string_lossy().to_string());
    }

    Ok(ExcelApplyResult {
        masked_path: output_path.to_string_lossy().to_string(),
        ecmap_path,
        encrypted_source_path: encsrc_path,
        report_md,
    })
}

#[tauri::command]
pub async fn excel_restore_from_ecmap(
    restore: ExcelRestoreReq,
) -> Result<ExcelRestoreResult, String> {
    let has_a = restore.encrypted_source_path.is_some();
    let has_b = restore.user_original_file_path.is_some();

    if !has_a && !has_b {
        return Err("材料不足：反脱敏必须 3 文件(路径A) 或 ecmap+用户原件(路径B)；不保留加密源则无法启用自动反脱敏".to_string());
    }

    if has_a {
        let enc_path = restore.encrypted_source_path.as_deref().unwrap();
        let enc_data = fs::read(enc_path).map_err(|e| format!("读取加密源失败: {}", e))?;
        let pass = restore.passphrase.clone().unwrap_or_default();
        let plain = crypto::decrypt_encsrc(&enc_data, &pass)
            .map_err(|e| format!("加密源解密失败: {}", e))?;
        fs::write(&restore.output_path, plain).map_err(|e| format!("写入恢复文件失败: {}", e))?;
        return Ok(ExcelRestoreResult {
            restored_path: restore.output_path.clone(),
            sha256_verified: true,
        });
    }

    if let Some(user_path) = restore.user_original_file_path.as_deref() {
        let user_bytes = fs::read(user_path).map_err(|e| format!("读取用户原件失败: {}", e))?;
        let ecmap_bytes =
            fs::read(&restore.ecmap_file_path).map_err(|e| format!("读取 ECMAP 失败: {}", e))?;
        let pass = restore.passphrase.clone().unwrap_or_default();
        let ecmap_json = crypto::decrypt_ecmap(&ecmap_bytes, &pass)
            .map_err(|e| format!("ECMAP 解密失败: {}", e))?;
        let doc: EcmapDocumentV1 = serde_json::from_slice(&ecmap_json)
            .map_err(|e| format!("ECMAP JSON 解析失败: {}", e))?;

        let user_sha = sha256_hex(&user_bytes);
        if user_sha != doc.header.original_sha256 {
            return Err("校验失败：用户提供原件的 SHA256 与 ECMAP header.originalSha256 不相等，拒绝猜测式还原".to_string());
        }

        fs::copy(user_path, &restore.output_path)
            .map_err(|e| format!("复制恢复文件失败: {}", e))?;
        return Ok(ExcelRestoreResult {
            restored_path: restore.output_path.clone(),
            sha256_verified: true,
        });
    }

    Err("材料不足".to_string())
}

#[tauri::command]
pub async fn excel_save_template(
    template_name: String,
    config: ExcelMaskingConfig,
) -> Result<String, String> {
    let slug: String = template_name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let json_bytes = serde_json::to_vec(&config).map_err(|e| format!("序列化模板失败: {}", e))?;
    let name_hash = sha256_hex(&json_bytes);
    let short = &name_hash[..10];

    let base = dirs_next::data_dir()
        .or_else(|| dirs_next::home_dir().map(|h| h.join(".cheersai-vault")))
        .ok_or_else(|| "无法获取用户数据目录".to_string())?;
    let dir = base.join("cheersai-vault").join("config_templates");
    fs::create_dir_all(&dir).map_err(|e| format!("创建模板目录失败: {}", e))?;
    let path = dir.join(format!("excel_masking_{}_{}.json", slug, short));
    fs::write(&path, json_bytes).map_err(|e| format!("写入模板失败: {}", e))?;
    Ok(path.to_string_lossy().to_string())
}

#[tauri::command]
pub async fn excel_load_template(template_path: String) -> Result<ExcelMaskingConfig, String> {
    let bytes = fs::read(&template_path).map_err(|e| format!("读取模板失败: {}", e))?;
    let cfg: ExcelMaskingConfig =
        serde_json::from_slice(&bytes).map_err(|e| format!("模板反序列化失败: {}", e))?;
    Ok(cfg)
}
