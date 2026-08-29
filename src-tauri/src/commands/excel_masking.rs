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
    /// Explicit, optional, backward-compatible rule-contract marker. A
    /// canonical-adapter-generated rule always sets this to
    /// `RULE_MODE_CANONICAL` and is masked by `strategy_id`/`replacement`
    /// via `apply_strategy()`. A rule that omits it (any config written
    /// before this field existed) is masked with the exact pre-existing
    /// field-based algorithm (`pattern`/`mask_char`/`keep_prefix`/
    /// `keep_suffix`/`replacement`), regardless of which of those fields it
    /// happens to carry — field presence alone cannot disambiguate a
    /// legacy `replacement`-only rule from a canonical one, so this marker
    /// is the single source of truth instead of a heuristic. Any other
    /// value is rejected before masking starts.
    #[serde(default)]
    pub rule_mode: Option<String>,
}

pub const RULE_MODE_CANONICAL: &str = "CANONICAL";

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
    pub strategy_id: String,
    pub replacement: Option<String>,
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
    #[serde(rename = "originalSha256", alias = "original_sha256")]
    pub original_sha256: String,
    #[serde(rename = "maskedSha256", alias = "masked_sha256")]
    pub masked_sha256: String,
    #[serde(rename = "sourceEncryptionKeySource", alias = "source_encryption_key_source")]
    pub source_encryption_key_source: String,
    #[serde(rename = "passphraseDomainHint8", alias = "passphrase_domain_hint8")]
    pub passphrase_domain_hint8: String,
    #[serde(rename = "sourceRetained", alias = "source_retained", default)]
    pub source_retained: bool,
}

pub const KEY_SOURCE_SANDBOX: &str = "SANDBOX_PASSPHRASE_REUSED";
pub const KEY_SOURCE_SEPARATE: &str = "SEPARATE_PASSPHRASE";
pub const KEY_SOURCE_DEVICE: &str = "DEVICE_KEY";

pub(crate) fn canonical_key_source(dto: Option<&EncSourcePassModeDto>) -> &'static str {
    match dto {
        Some(EncSourcePassModeDto::DeviceKey) => KEY_SOURCE_DEVICE,
        Some(EncSourcePassModeDto::SecondaryPhrase(_)) => KEY_SOURCE_SEPARATE,
        _ => KEY_SOURCE_SANDBOX,
    }
}

/// Accepts both the current canonical enum values and the pre-fix values
/// written by older builds (Tauri and Runtime), so existing `.ecmap`
/// artifacts stay readable. Rejects anything else instead of guessing.
pub(crate) fn normalize_key_source(raw: &str) -> Result<&'static str, String> {
    match raw {
        "SANDBOX_PASSPHRASE_REUSED" | "SandboxReused" => Ok(KEY_SOURCE_SANDBOX),
        "SEPARATE_PASSPHRASE" | "SecondaryPassphrase" | "SecondaryPhrase" => {
            Ok(KEY_SOURCE_SEPARATE)
        }
        "DEVICE_KEY" | "DeviceKey" => Ok(KEY_SOURCE_DEVICE),
        other => Err(format!("未知的密钥来源枚举值: {other}")),
    }
}

/// Whether the raw (decrypted) ECMAP JSON actually contains a
/// `sourceRetained`/`source_retained` key on its header, distinct from a
/// deserialized `false` — which `#[serde(default)]` also produces for
/// headers written before this field existed. Used to tell "explicitly not
/// retained" (a real contradiction with restore path A) apart from "this
/// header predates the field" (unknown, not a contradiction by itself).
pub(crate) fn ecmap_header_declares_source_retained(raw_json: &[u8]) -> bool {
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(raw_json) else {
        return false;
    };
    value
        .get("header")
        .and_then(|header| header.as_object())
        .map(|header| {
            header.contains_key("sourceRetained") || header.contains_key("source_retained")
        })
        .unwrap_or(false)
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

fn mask_middle(value: &str, keep_prefix: usize, keep_suffix: usize) -> String {
    let chars: Vec<char> = value.chars().collect();
    let len = chars.len();
    if len == 0 {
        return String::new();
    }
    if keep_prefix + keep_suffix >= len {
        return "*".repeat(len);
    }
    let mut out = String::with_capacity(len);
    out.extend(chars.iter().take(keep_prefix));
    out.extend(std::iter::repeat_n('*', len - keep_prefix - keep_suffix));
    out.extend(chars.iter().skip(len - keep_suffix));
    out
}

fn mask_email(value: &str) -> String {
    let Some((user, domain)) = value.split_once('@') else {
        return mask_middle(value, 1, 1);
    };
    let user_len = user.chars().count();
    let masked_user = if user_len <= 2 {
        "*".repeat(user_len.max(1))
    } else {
        mask_middle(user, 1, 1)
    };
    format!("{masked_user}@{domain}")
}

/// 仅当值恰好是 11 位 ASCII 数字时才脱敏（保留前 3、后 4）；其他任何情况
/// （空值、长度不符、含字母/符号等）一律保持原值不变。
fn mask_phone(value: &str) -> String {
    let is_11_ascii_digits =
        value.len() == 11 && value.chars().all(|c| c.is_ascii_digit());
    if is_11_ascii_digits {
        mask_middle(value, 3, 4)
    } else {
        value.to_string()
    }
}

/// 18 位保留前 6、后 4；15 位保留前 3、后 4；其他长度（含空值）保持原值不变。
pub(crate) fn mask_idcard(value: &str) -> String {
    match value.chars().count() {
        18 => mask_middle(value, 6, 4),
        15 => mask_middle(value, 3, 4),
        _ => value.to_string(),
    }
}

pub(crate) fn apply_strategy(value: &str, strategy_id: &str, replacement: Option<&str>) -> String {
    match strategy_id {
        "FULL_MASK" | "BANK_CARD" | "EMAIL" | "ADDRESS" | "COMPLIANCE_ID" => replacement
            .map(str::to_string)
            .unwrap_or_else(|| mask_middle(value, 0, 0)),
        "PHONE_MID4" => mask_phone(value),
        "IDCARD_MID10" => mask_idcard(value),
        "BANKCARD_LAST4" => mask_middle(value, 0, 4),
        "EMAIL_USER_MASK" => mask_email(value),
        "DEFAULT_VALUE" => replacement.unwrap_or("[MASKED]").to_string(),
        "CLEAR_COL" => String::new(),
        _ => replacement
            .map(str::to_string)
            .unwrap_or_else(|| mask_middle(value, 0, 0)),
    }
}

/// R7 (second pass): field presence alone cannot disambiguate a legacy
/// `replacement`-only rule from a canonical `replacement`-only rule — both
/// shapes are valid with every other field empty. Dispatch is therefore
/// driven solely by the explicit `rule_mode` marker: `Some(RULE_MODE_
/// CANONICAL)` (always set by the canonical adapter) masks by
/// `strategy_id`/`replacement` via `apply_strategy()`; anything else (in
/// practice only `None`, since `validate_column_rule_modes()` rejects any
/// other value before this function is ever called) is masked with the
/// exact pre-existing field-based algorithm, regardless of which legacy
/// fields it happens to carry.
fn mask_value(value: &str, rule: &ColumnMaskingRule) -> String {
    if !rule.enabled {
        return value.to_string();
    }
    if rule.rule_mode.as_deref() == Some(RULE_MODE_CANONICAL) {
        apply_strategy(value, &rule.strategy_id, rule.replacement.as_deref())
    } else {
        mask_value_legacy_fields(value, rule)
    }
}

/// Rejects any `rule_mode` other than `None` or `RULE_MODE_CANONICAL`
/// before masking starts, so an unrecognized contract-version marker fails
/// the whole command instead of being silently treated as legacy or
/// canonical.
fn validate_column_rule_modes(config: &ExcelMaskingConfig) -> Result<(), String> {
    for sheet in &config.sheets {
        for (header, rule) in &sheet.column_rules {
            if let Some(mode) = &rule.rule_mode {
                if mode != RULE_MODE_CANONICAL {
                    return Err(format!(
                        "工作表 '{}' 列规则 '{}' 使用未知的规则模式标记 '{}'，拒绝执行",
                        sheet.sheet_name, header, mode
                    ));
                }
            }
        }
    }
    Ok(())
}

/// UI-STATE-004 (TASK-EXCEL-OUTPUT-RECOVERY-CONSISTENCY-CLOSEOUT-001):
/// 先校验口令、后写文件——在写出任何脱敏产物之前确认将要用于加密的口令
/// 可用，避免应用失败后残留不完整的 masked/report 半成品（空沙箱口令场景
/// 曾真实复现：`masked.xlsx` 与报告已写出、`.ecmap` 加密失败）。
///
/// - `SANDBOX_REUSED`（默认）：沙箱口令必须非空白；
/// - `SECONDARY_PASSPHRASE`：独立二级口令必须非空白；
/// - `DEVICE_KEY`：真实密钥在加密阶段经 keyring 解析，此处不做预先校验
///   （隔离测试不覆盖真实 macOS Keychain ACL，页面/契约测试已覆盖选择）。
fn validate_effective_passphrase(config: &ExcelMaskingConfig) -> Result<(), String> {
    match &config.source_pass_mode {
        Some(EncSourcePassModeDto::SecondaryPhrase(phrase)) => {
            if phrase.trim().is_empty() {
                return Err("独立二级口令不能为空".to_string());
            }
        }
        Some(EncSourcePassModeDto::SandboxReused) | None => {
            if config.passphrase.as_deref().unwrap_or("").trim().is_empty() {
                return Err("沙箱口令不能为空".to_string());
            }
        }
        Some(EncSourcePassModeDto::DeviceKey) => {}
    }
    Ok(())
}

fn mask_value_legacy_fields(value: &str, rule: &ColumnMaskingRule) -> String {
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
    for ch in chars
        .iter()
        .skip(len.saturating_sub(keep_suffix))
        .take(keep_suffix)
    {
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
    validate_column_rule_modes(&config)?;
    let sheets_info = file_parser::parse_excel_structure_detailed(&config.input_file_path)?;
    let max_rows = max_rows.unwrap_or(20);
    let input_path = Path::new(&config.input_file_path);
    let is_csv = file_parser::is_csv_path(input_path);
    let is_xls = file_parser::is_xls_path(input_path);

    let mut out_sheets = Vec::new();

    for sheet_info in sheets_info {
        let sheet_cfg = config
            .sheets
            .iter()
            .find(|s| s.sheet_name == sheet_info.name);
        let header_row = sheet_cfg.and_then(|c| c.header_row).unwrap_or(0);

        // Bounded fast path: CSV (single implicit sheet, header always row 1)
        // or xlsx with the default header_row=0, which is the only value the
        // frontend ever sends and matches table_reader's fixed "row 1 is the
        // header" convention. `.xls` (not a ZIP archive, R1) and a
        // non-default header_row (a still-supported, if currently
        // unexercised, part of the public config contract) both fall back
        // to the exact previous calamine full-range logic so neither
        // capability is silently dropped.
        let preview_cells = if is_csv {
            let preview = excel_style_core::table_reader::read_csv_preview(input_path, max_rows)
                .map_err(|e| format!("Failed to read CSV file: {}", e))?;
            build_preview_cells_from_table_preview(&preview, sheet_cfg)
        } else if header_row == 0 && !is_xls {
            let preview =
                excel_style_core::table_reader::read_xlsx_preview(input_path, &sheet_info.name, max_rows)
                    .map_err(|e| format!("Failed to read sheet: {}", e))?;
            build_preview_cells_from_table_preview(&preview, sheet_cfg)
        } else {
            build_preview_cells_via_calamine(input_path, &sheet_info, sheet_cfg, header_row, max_rows)?
        };

        out_sheets.push(SheetMaskPreview {
            sheet_name: sheet_info.name,
            headers: sheet_info.headers,
            preview_rows: preview_cells,
        });
    }

    Ok(ExcelMaskPreview { sheets: out_sheets })
}

/// Builds preview cells from a bounded `table_reader::TablePreview`,
/// reproducing the exact `row`/`cell_ref` numbering the previous
/// calamine-based loop produced for the `header_row=0` case: the pre-existing
/// convention used the 0-based range row index (1 for the first data row,
/// matching `data_start = header_row + 1`) rather than the 1-based file row,
/// so `table_reader`'s 1-based `row_number` (2 for the first data row) is
/// translated back by subtracting 1 to avoid changing this response's
/// existing numbering.
fn build_preview_cells_from_table_preview(
    preview: &excel_style_core::table_reader::TablePreview,
    sheet_cfg: Option<&SheetMaskingConfig>,
) -> Vec<ExcelMaskPreviewCell> {
    let mut preview_cells = Vec::new();
    for row in &preview.rows {
        let legacy_row = row.row_number - 1;
        for (col_idx_0, cell_val) in row.values.iter().enumerate() {
            if cell_val.is_empty() {
                continue;
            }
            let col = (col_idx_0 as u32) + 1;
            let header_name = preview.headers.get(col_idx_0).cloned().unwrap_or_default();
            let a1 = cell_ref_a1(legacy_row, col);
            let mut strategy_id = String::from("default:identity");
            let mut masked = cell_val.clone();

            if let Some(sc) = sheet_cfg {
                if let Some(ov) = sc
                    .cell_overrides
                    .iter()
                    .find(|o| o.cell_ref.to_uppercase() == a1.to_uppercase())
                {
                    strategy_id = ov.strategy_id.clone();
                    masked = apply_strategy(cell_val, &ov.strategy_id, ov.replacement.as_deref());
                } else if let Some(rule) = sc.column_rules.get(&header_name) {
                    strategy_id = rule.strategy_id.clone();
                    masked = mask_value(cell_val, rule);
                }
            }

            preview_cells.push(ExcelMaskPreviewCell {
                original_preview: preview_preview(cell_val, 8),
                masked,
                strategy_id,
                row: legacy_row,
                col,
                cell_ref: a1,
            });
        }
    }
    preview_cells
}

/// The exact pre-existing calamine full-range preview logic, preserved
/// verbatim (only extracted into its own function) as the fallback path for
/// a non-default `header_row`, which `table_reader` does not model.
fn build_preview_cells_via_calamine(
    input_path: &Path,
    sheet_info: &SheetDef,
    sheet_cfg: Option<&SheetMaskingConfig>,
    header_row: u32,
    max_rows: usize,
) -> Result<Vec<ExcelMaskPreviewCell>, String> {
    use calamine::{open_workbook_auto, Data, Reader};
    let mut workbook = open_workbook_auto(input_path)
        .map_err(|e| format!("Failed to open Excel: {}", e))?;
    let range = workbook
        .worksheet_range(&sheet_info.name)
        .map_err(|e| format!("Failed to read sheet: {}", e))?;

    let height = range.get_size().0;
    let width = range.get_size().1;
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
                    masked = apply_strategy(&cell_val, &ov.strategy_id, ov.replacement.as_deref());
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

    Ok(preview_cells)
}

#[tauri::command]
pub async fn excel_apply_masking(
    config: ExcelMaskingConfig,
    output_dir: String,
) -> Result<ExcelApplyResult, String> {
    validate_column_rule_modes(&config)?;
    // UI-STATE-004 (TASK-EXCEL-OUTPUT-RECOVERY-CONSISTENCY-CLOSEOUT-001):
    // 先校验口令、后写文件。只有在确实需要加密（生成 .ecmap 或保留加密源）
    // 时才校验口令；口令错误（如空沙箱口令）必须在创建输出目录或写出任何
    // 脱敏产物之前失败，避免应用失败后残留不完整的 masked/report 半成品。
    let will_encrypt =
        config.generate_ecmap.unwrap_or(true) || config.retain_encrypted_source.unwrap_or(false);
    if will_encrypt {
        validate_effective_passphrase(&config)?;
    }
    let input_path = Path::new(&config.input_file_path);
    let stem = input_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("excel");
    let suffix = config.output_name_suffix.as_deref().unwrap_or("_masked");
    let output_path = PathBuf::from(&output_dir).join(format!("{}{}.xlsx", stem, suffix));

    fs::create_dir_all(&output_dir).map_err(|e| format!("创建输出目录失败: {}", e))?;

    let sheets_info = file_parser::parse_excel_structure_detailed(&config.input_file_path)?;

    let mut replacements: HashMap<CellKey, String> = HashMap::new();
    let mut entries: Vec<EcmapEntryV1> = Vec::new();

    // CSV has no OOXML to clone-inject into (and no calamine support at
    // all), so its masked output is always built via
    // `fallback_xlsxwriter_full`, reusing the same column-rule/cell-override
    // masking logic as `.xlsx`. `.xlsx`/`.xls` keep the exact previous
    // calamine-based full-range path unchanged.
    let outcome: RewriteOutcome = if file_parser::is_csv_path(input_path) {
        let (header, rows) = excel_style_core::table_reader::read_csv_all_rows(input_path)
            .map_err(|e| format!("Failed to read CSV file: {}", e))?;
        let sheet_name = sheets_info
            .first()
            .map(|s| s.name.clone())
            .unwrap_or_else(|| "Sheet1".to_string());
        let sheet_cfg = config.sheets.iter().find(|s| s.sheet_name == sheet_name);

        let mut masked_rows: Vec<Vec<String>> = Vec::with_capacity(rows.len());
        for (i, row) in rows.iter().enumerate() {
            // row_number is the 1-based file row (header is file row 1),
            // mirroring the xlsx branch's `row + 1` convention below.
            let row_number = (i as u32) + 2;
            let mut masked_row = Vec::with_capacity(row.len());
            for (col_idx_0, cell_val) in row.iter().enumerate() {
                let col = (col_idx_0 as u32) + 1;
                if cell_val.is_empty() {
                    masked_row.push(String::new());
                    continue;
                }
                let header_name = header.get(col_idx_0).cloned().unwrap_or_default();
                let a1 = cell_ref_a1(row_number, col);
                let mut strategy_id = String::from("identity");
                let mut masked: Option<String> = None;

                if let Some(sc) = sheet_cfg {
                    if let Some(ov) = sc
                        .cell_overrides
                        .iter()
                        .find(|o| o.cell_ref.to_uppercase() == a1.to_uppercase())
                    {
                        strategy_id = ov.strategy_id.clone();
                        masked = Some(apply_strategy(
                            cell_val,
                            &ov.strategy_id,
                            ov.replacement.as_deref(),
                        ));
                    } else if let Some(rule) = sc.column_rules.get(&header_name) {
                        if rule.enabled {
                            strategy_id = rule.strategy_id.clone();
                            masked = Some(mask_value(cell_val, rule));
                        }
                    }
                }

                let final_val = masked.clone().unwrap_or_else(|| cell_val.clone());
                masked_row.push(final_val);

                if let Some(m) = masked {
                    if m != *cell_val {
                        replacements.insert(
                            CellKey {
                                sheet: sheet_name.clone(),
                                row: row_number,
                                col,
                            },
                            m.clone(),
                        );
                        entries.push(EcmapEntryV1 {
                            cell_ref: a1,
                            original_sha256: sha256_hex(cell_val.as_bytes()),
                            original_preview: preview_preview(cell_val, 8),
                            masked: m,
                            strategy_id: strategy_id.clone(),
                            col_index: col,
                            row_index: row_number,
                            sheet: sheet_name.clone(),
                        });
                    }
                }
            }
            masked_rows.push(masked_row);
        }

        let mut csv_outcome =
            excel_style_core::fallback_xlsxwriter_full(&header, &masked_rows, &output_path)
                .map_err(|e| format!("写入 masked CSV 输出失败: {}", e))?;
        // R-closeout (TASK-EXCEL-OUTPUT-RECOVERY-CONSISTENCY-CLOSEOUT-001):
        // `fallback_xlsxwriter_full` writes every cell and therefore reports
        // hits=0; the "实际发生替换的单元格数" is exactly the number of
        // `.ecmap` entries collected above. Align the report with the
        // `.ecmap` entries and the produced workbook so the old "报告统计为 0"
        // defect cannot recur on the desktop CSV path either.
        csv_outcome.hits = entries.len() as u64;
        csv_outcome
    } else if file_parser::is_xls_path(input_path) {
        // R3: `.xls` is legacy OLE, not an OOXML ZIP, so it can never go
        // through `rewrite_clone_inject` — the shared
        // `rewrite_legacy_xls_with_mask` reads every sheet in full, masks
        // every data cell via the same header-text/cell-override lookup
        // used by the `.xlsx` branch below, and returns the exact list of
        // changed cells the output workbook was built from; `.ecmap`
        // entries are built from that same list, not a second independent
        // pass over the file.
        let (legacy_outcome, changes) = excel_style_core::rewrite_legacy_xls_with_mask(
            input_path,
            &output_path,
            |sheet_name, row_idx, col_idx, original| {
                if original.is_empty() {
                    return String::new();
                }
                let header_name = sheets_info
                    .iter()
                    .find(|s| s.name == sheet_name)
                    .and_then(|s| s.headers.get(col_idx))
                    .cloned()
                    .unwrap_or_default();
                let a1 = cell_ref_a1((row_idx as u32) + 1, (col_idx as u32) + 1);
                let Some(sc) = config.sheets.iter().find(|s| s.sheet_name == sheet_name) else {
                    return original.to_string();
                };
                if let Some(ov) = sc
                    .cell_overrides
                    .iter()
                    .find(|o| o.cell_ref.to_uppercase() == a1.to_uppercase())
                {
                    return apply_strategy(original, &ov.strategy_id, ov.replacement.as_deref());
                }
                if let Some(rule) = sc.column_rules.get(&header_name) {
                    if rule.enabled {
                        return mask_value(original, rule);
                    }
                }
                original.to_string()
            },
        )
        .map_err(|e| format!("Failed to process legacy xls file: {}", e))?;

        for change in changes {
            let row_number = (change.row_idx as u32) + 1;
            let col_number = (change.col_idx as u32) + 1;
            let a1 = cell_ref_a1(row_number, col_number);
            let header_name = sheets_info
                .iter()
                .find(|s| s.name == change.sheet)
                .and_then(|s| s.headers.get(change.col_idx))
                .cloned()
                .unwrap_or_default();
            let strategy_id = config
                .sheets
                .iter()
                .find(|s| s.sheet_name == change.sheet)
                .and_then(|sc| {
                    sc.cell_overrides
                        .iter()
                        .find(|o| o.cell_ref.to_uppercase() == a1.to_uppercase())
                        .map(|ov| ov.strategy_id.clone())
                        .or_else(|| sc.column_rules.get(&header_name).map(|r| r.strategy_id.clone()))
                })
                .unwrap_or_else(|| "identity".to_string());
            entries.push(EcmapEntryV1 {
                cell_ref: a1,
                original_sha256: sha256_hex(change.original.as_bytes()),
                original_preview: preview_preview(&change.original, 8),
                masked: change.masked,
                strategy_id,
                col_index: col_number,
                row_index: row_number,
                sheet: change.sheet,
            });
        }

        legacy_outcome
    } else {
        use calamine::{open_workbook_auto, Data, Reader};
        let mut workbook = open_workbook_auto(&config.input_file_path)
            .map_err(|e| format!("Failed to open Excel: {}", e))?;
        let _sheet_names: Vec<String> = workbook.sheet_names().to_vec();

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
                            masked = Some(apply_strategy(
                                &cell_val,
                                &ov.strategy_id,
                                ov.replacement.as_deref(),
                            ));
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
            })
    };

    let original_bytes =
        fs::read(&config.input_file_path).map_err(|e| format!("读取原文件失败: {}", e))?;
    let original_sha256 = sha256_hex(&original_bytes);

    let masked_bytes =
        fs::read(&output_path).map_err(|e| format!("读取 masked 文件失败: {}", e))?;
    let masked_sha256 = sha256_hex(&masked_bytes);

    let report_md = excel_style_core::build_report_md(&outcome);
    let report_path = PathBuf::from(&output_dir).join(format!("{}{}_report.md", stem, suffix));
    let _ = fs::write(&report_path, &report_md);

    let mut ecmap_path: Option<String> = None;
    let mut encsrc_path: Option<String> = None;

    let will_retain_source = config.retain_encrypted_source.unwrap_or(false);

    if config.generate_ecmap.unwrap_or(true) {
        let pass = config.passphrase.clone().unwrap_or_default();
        // R-closeout (工作包 D): the header hint must be for the passphrase
        // the .ecmap is actually encrypted with. For SECONDARY_PASSPHRASE
        // that is the secondary phrase, never the sandbox fallback; for
        // SandboxReused it stays the sandbox passphrase.
        let effective_hint_pass: String = match &config.source_pass_mode {
            Some(EncSourcePassModeDto::SecondaryPhrase(phrase))
                if !phrase.trim().is_empty() =>
            {
                phrase.clone()
            }
            _ => pass.clone(),
        };
        let hint = crypto::domain_hint8(&effective_hint_pass, KeyDomain::EcmapV1);
        let key_source = canonical_key_source(config.source_pass_mode.as_ref()).to_string();

        let doc = EcmapDocumentV1 {
            header: EcmapHeaderV1 {
                version: "1.2".to_string(),
                original_sha256: original_sha256.clone(),
                masked_sha256: masked_sha256.clone(),
                source_encryption_key_source: key_source,
                passphrase_domain_hint8: hint,
                source_retained: will_retain_source,
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

    if will_retain_source {
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

const LEGACY_XLS_MAGIC: &[u8; 8] = &[0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];

fn restored_excel_extension(bytes: &[u8]) -> Result<&'static str, String> {
    if bytes.starts_with(LEGACY_XLS_MAGIC) {
        return Ok("xls");
    }

    // OOXML workbooks are ZIP archives. The local-file header is sufficient
    // here because the bytes have already been authenticated by the ecmap
    // SHA-256 check; this branch only chooses the matching user-facing suffix.
    if bytes.starts_with(b"PK\x03\x04") {
        return Ok("xlsx");
    }

    // CSV is a supported Excel input and may be retained/restored as its
    // original text bytes. Keep the detection deliberately conservative so a
    // corrupted binary payload is never silently written with a .csv suffix.
    if !bytes.is_empty()
        && !bytes.iter().any(|byte| *byte == 0)
        && bytes.iter().any(|byte| matches!(*byte, b',' | b'\n' | b'\r'))
    {
        return Ok("csv");
    }

    Err("恢复文件格式无法识别，已拒绝写出".to_string())
}

/// Normalize the selected restore path to the extension that matches the
/// authenticated bytes. The file contents are never converted or rewritten;
/// only a mismatched suffix is corrected before the final write.
fn normalize_excel_restore_output_path(
    output_path: &Path,
    bytes: &[u8],
) -> Result<PathBuf, String> {
    let expected_extension = restored_excel_extension(bytes)?;
    let already_matches = output_path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case(expected_extension));

    if already_matches {
        return Ok(output_path.to_path_buf());
    }

    let mut normalized = output_path.to_path_buf();
    normalized.set_extension(expected_extension);
    Ok(normalized)
}

/// Resolve the final restore path without bypassing the save dialog's
/// overwrite confirmation. When byte-based extension normalization changes
/// the path, an existing directory entry at that new path was not selected
/// by the user and must therefore be rejected before any write. Use
/// `symlink_metadata` so even a broken symlink is treated as occupied; other
/// metadata errors fail closed rather than being mistaken for absence.
fn resolve_excel_restore_output_path(
    output_path: &Path,
    bytes: &[u8],
) -> Result<PathBuf, String> {
    let normalized = normalize_excel_restore_output_path(output_path, bytes)?;
    if normalized == output_path {
        return Ok(normalized);
    }

    match fs::symlink_metadata(&normalized) {
        Ok(_) => Err("恢复目标路径已存在，请重新选择文件名".to_string()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(normalized),
        Err(_) => Err("无法确认恢复目标路径状态，已拒绝写出".to_string()),
    }
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
        let pass = restore.passphrase.clone().unwrap_or_default();

        // R6: 路径 A 在写出结果前必须先用 .ecmap header 校验材料一致性，
        // 不能只信任 .encrypted_src 自身能解密就判定为可信恢复。
        let ecmap_bytes =
            fs::read(&restore.ecmap_file_path).map_err(|e| format!("读取 ECMAP 失败: {}", e))?;
        let ecmap_json = crypto::decrypt_ecmap(&ecmap_bytes, &pass)
            .map_err(|e| format!("ECMAP 解密失败: {}", e))?;
        let doc: EcmapDocumentV1 = serde_json::from_slice(&ecmap_json)
            .map_err(|e| format!("ECMAP JSON 解析失败: {}", e))?;
        normalize_key_source(&doc.header.source_encryption_key_source)
            .map_err(|e| format!("ECMAP header 校验失败: {}", e))?;

        // 旧 header（早于 sourceRetained 字段引入）无法证明当时的留存意图；
        // 缺失时不静默当作一致，而是不使用这一项信号，改由下方强制的 SHA
        // 校验作为唯一的一致性证明。只有 header 明确写了 sourceRetained
        // 且为 false 时，才判定为"header 与传入的 .encrypted_src 材料矛盾"。
        if ecmap_header_declares_source_retained(&ecmap_json) && !doc.header.source_retained {
            return Err("校验失败：ECMAP header 标明未留存加密源（sourceRetained=false），与传入的 .encrypted_src 材料矛盾，拒绝猜测式还原".to_string());
        }

        let enc_data = fs::read(enc_path).map_err(|e| format!("读取加密源失败: {}", e))?;
        let plain = crypto::decrypt_encsrc(&enc_data, &pass)
            .map_err(|e| format!("加密源解密失败: {}", e))?;
        let decrypted_sha = sha256_hex(&plain);
        if decrypted_sha != doc.header.original_sha256 {
            return Err("校验失败：解密后的加密源 SHA256 与 ECMAP header.originalSha256 不相等，拒绝猜测式还原".to_string());
        }

        let masked_bytes = fs::read(&restore.masked_file_path)
            .map_err(|e| format!("读取 masked 文件失败: {}", e))?;
        let masked_sha = sha256_hex(&masked_bytes);
        if masked_sha != doc.header.masked_sha256 {
            return Err("校验失败：传入的 masked 文件 SHA256 与 ECMAP header.maskedSha256 不相等，拒绝猜测式还原".to_string());
        }

        let final_output_path = resolve_excel_restore_output_path(
            Path::new(&restore.output_path),
            &plain,
        )?;
        fs::write(&final_output_path, &plain)
            .map_err(|e| format!("写入恢复文件失败: {}", e))?;
        return Ok(ExcelRestoreResult {
            restored_path: final_output_path.to_string_lossy().to_string(),
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
        normalize_key_source(&doc.header.source_encryption_key_source)
            .map_err(|e| format!("ECMAP header 校验失败: {}", e))?;

        let user_sha = sha256_hex(&user_bytes);
        if user_sha != doc.header.original_sha256 {
            return Err("校验失败：用户提供原件的 SHA256 与 ECMAP header.originalSha256 不相等，拒绝猜测式还原".to_string());
        }

        let final_output_path = resolve_excel_restore_output_path(
            Path::new(&restore.output_path),
            &user_bytes,
        )?;
        fs::write(&final_output_path, &user_bytes)
            .map_err(|e| format!("写入恢复文件失败: {}", e))?;
        return Ok(ExcelRestoreResult {
            restored_path: final_output_path.to_string_lossy().to_string(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    fn unique_temp_dir(tag: &str) -> PathBuf {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "cheersai-excel-masking-test-{tag}-{nanos}-{n}-{}",
            std::process::id()
        ));
        fs::create_dir_all(&dir).expect("create temp test dir");
        dir
    }

    fn write_fixture_xlsx(dir: &Path, headers: &[&str], rows: &[Vec<&str>]) -> PathBuf {
        let path = dir.join("fixture.xlsx");
        let headers_owned: Vec<String> = headers.iter().map(|s| s.to_string()).collect();
        let rows_owned: Vec<Vec<String>> = rows
            .iter()
            .map(|row| row.iter().map(|s| s.to_string()).collect())
            .collect();
        excel_style_core::fallback_xlsxwriter_full(&headers_owned, &rows_owned, &path)
            .expect("write fixture xlsx");
        path
    }

    // --- R4: ID card masking must branch on exact char length, not a
    // generic mask_middle(value, 4, 4). ---

    #[test]
    fn idcard_strategy_masks_18_digit_values_as_6_plus_8_plus_4() {
        assert_eq!(
            mask_idcard("123456789012345678"),
            "123456********5678"
        );
        assert_eq!(
            apply_strategy("123456789012345678", "IDCARD_MID10", None),
            "123456********5678"
        );
    }

    #[test]
    fn idcard_strategy_masks_15_digit_values_as_3_plus_8_plus_4() {
        assert_eq!(mask_idcard("123456789012345"), "123********2345");
    }

    #[test]
    fn idcard_strategy_keeps_empty_and_other_lengths_unchanged() {
        assert_eq!(mask_idcard(""), "");
        assert_eq!(mask_idcard("1234567890"), "1234567890");
        assert_eq!(mask_idcard("1234567890123456789"), "1234567890123456789");
    }

    #[test]
    fn other_strategies_still_behave_as_before() {
        assert_eq!(apply_strategy("13900001234", "PHONE_MID4", None), "139****1234");
        assert_eq!(
            apply_strategy("alice@example.com", "EMAIL_USER_MASK", None),
            "a***e@example.com"
        );
        assert_eq!(apply_strategy("anything", "CLEAR_COL", None), "");
        assert_eq!(
            apply_strategy("anything", "DEFAULT_VALUE", None),
            "[MASKED]"
        );
        assert_eq!(
            apply_strategy("anything", "DEFAULT_VALUE", Some("REDACTED")),
            "REDACTED"
        );
    }

    #[test]
    fn phone_mid4_only_masks_exactly_11_ascii_digit_values() {
        assert_eq!(apply_strategy("13900001234", "PHONE_MID4", None), "139****1234");
        assert_eq!(apply_strategy("1390000123", "PHONE_MID4", None), "1390000123");
        assert_eq!(apply_strategy("139000012345", "PHONE_MID4", None), "139000012345");
        assert_eq!(apply_strategy("1390000123a", "PHONE_MID4", None), "1390000123a");
        assert_eq!(apply_strategy("", "PHONE_MID4", None), "");
        assert_eq!(apply_strategy("一三900001234", "PHONE_MID4", None), "一三900001234");
    }

    fn write_fixture_csv(dir: &Path, bytes: &[u8]) -> PathBuf {
        let path = dir.join("fixture.csv");
        fs::write(&path, bytes).expect("write fixture csv");
        path
    }

    /// A real, existing, valid legacy OLE `.xls` fixture (not a fabricated
    /// or product/PII file) shared with the `engine-core` crate's own
    /// tests, reused read-only here rather than duplicated.
    fn sample_xls_bytes() -> Vec<u8> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("crates/engine-core/tests/fixtures/sample.xls");
        fs::read(&path).expect("read sample.xls fixture")
    }

    fn write_fixture_xls(dir: &Path) -> PathBuf {
        let path = dir.join("fixture.xls");
        fs::write(&path, sample_xls_bytes()).expect("write fixture xls");
        path
    }

    fn phone_column_rules() -> HashMap<String, ColumnMaskingRule> {
        let mut column_rules = HashMap::new();
        column_rules.insert(
            "手机号".to_string(),
            ColumnMaskingRule {
                strategy_id: "PHONE_MID4".to_string(),
                pattern: None,
                replacement: None,
                mask_char: None,
                keep_prefix: None,
                keep_suffix: None,
                enabled: true,
                rule_mode: Some(RULE_MODE_CANONICAL.to_string()),
            },
        );
        column_rules
    }

    #[tokio::test]
    async fn csv_utf8_bom_structure_preview_and_apply_produce_masked_workbook() {
        let dir = unique_temp_dir("csv-utf8-bom");
        let mut bytes: Vec<u8> = vec![0xEF, 0xBB, 0xBF];
        bytes.extend_from_slice("姓名,手机号\n张三,13900001234\n李四,139\n".as_bytes());
        let input = write_fixture_csv(&dir, &bytes);
        let input_path_str = input.to_string_lossy().to_string();

        let structure = file_parser::parse_excel_structure_detailed(&input_path_str)
            .expect("parse csv structure");
        assert_eq!(structure.len(), 1);
        assert_eq!(structure[0].name, "Sheet1");
        assert_eq!(structure[0].headers, vec!["姓名", "手机号"]);
        assert_eq!(structure[0].max_row, 3);
        assert_eq!(structure[0].max_col, 2);

        let config = ExcelMaskingConfig {
            input_file_path: input_path_str.clone(),
            output_name_suffix: None,
            sheets: vec![SheetMaskingConfig {
                sheet_name: "Sheet1".to_string(),
                header_row: Some(0),
                column_rules: phone_column_rules(),
                cell_overrides: vec![],
            }],
            passphrase: Some("test-sandbox-pass".to_string()),
            retain_encrypted_source: Some(false),
            source_pass_mode: Some(EncSourcePassModeDto::SandboxReused),
            generate_ecmap: Some(true),
        };

        let preview = excel_preview_masking(config.clone(), Some(20))
            .await
            .expect("csv preview must succeed");
        assert_eq!(preview.sheets.len(), 1);
        let masked_by_cell_ref: HashMap<String, String> = preview.sheets[0]
            .preview_rows
            .iter()
            .map(|c| (c.cell_ref.clone(), c.masked.clone()))
            .collect();
        assert_eq!(masked_by_cell_ref.get("B1").map(String::as_str), Some("139****1234"));
        assert_eq!(masked_by_cell_ref.get("B2").map(String::as_str), Some("139"));

        let output_dir = dir.join("out");
        let result = excel_apply_masking(config, output_dir.to_string_lossy().to_string())
            .await
            .expect("csv apply masking must succeed");

        use calamine::{open_workbook_auto, Reader};
        let mut workbook =
            open_workbook_auto(&result.masked_path).expect("open csv-derived masked workbook");
        let range = workbook
            .worksheet_range("Sheet1")
            .expect("Sheet1 must exist");
        assert_eq!(cell_to_string_for_test(&range, 0, 0), "姓名");
        assert_eq!(cell_to_string_for_test(&range, 0, 1), "手机号");
        assert_eq!(cell_to_string_for_test(&range, 1, 0), "张三");
        assert_eq!(cell_to_string_for_test(&range, 1, 1), "139****1234");
        assert_eq!(cell_to_string_for_test(&range, 2, 0), "李四");
        assert_eq!(cell_to_string_for_test(&range, 2, 1), "139");
    }

    fn cell_to_string_for_test(range: &calamine::Range<calamine::Data>, r: usize, c: usize) -> String {
        use calamine::Data;
        match range.get((r, c)) {
            Some(Data::String(s)) => s.clone(),
            Some(other) => format!("{other:?}"),
            None => String::new(),
        }
    }

    /// R-closeout (工作包 B): a CSV with exactly 12 maskable phones must
    /// report 12 in report.md on the desktop path too — `fallback_xlsxwriter
    /// _full` cannot know which cells changed, so the CSV branch must align
    /// the report hits with the `.ecmap` entries it actually wrote.
    #[tokio::test]
    async fn csv_12_hit_report_counts_real_replacements_not_zero() {
        let dir = unique_temp_dir("csv-12hit");
        let mut content = String::from("姓名,手机号\n");
        for i in 1..=12u32 {
            content.push_str(&format!("用户{i},13{:09}\n", 900_000_000 + i));
        }
        content.push_str("无效A,139\n无效B,not-a-phone\n");
        let input = write_fixture_csv(&dir, content.as_bytes());
        let input_path_str = input.to_string_lossy().to_string();

        let config = ExcelMaskingConfig {
            input_file_path: input_path_str.clone(),
            output_name_suffix: None,
            sheets: vec![SheetMaskingConfig {
                sheet_name: "Sheet1".to_string(),
                header_row: Some(0),
                column_rules: phone_column_rules(),
                cell_overrides: vec![],
            }],
            passphrase: Some("test-sandbox-pass".to_string()),
            retain_encrypted_source: Some(false),
            source_pass_mode: Some(EncSourcePassModeDto::SandboxReused),
            generate_ecmap: Some(true),
        };

        let output_dir = dir.join("out");
        let result = excel_apply_masking(config, output_dir.to_string_lossy().to_string())
            .await
            .expect("csv apply masking must succeed");

        let report_path = output_dir.join("fixture_masked_report.md");
        let report = std::fs::read_to_string(&report_path).expect("read report.md");
        assert!(
            report.contains("**命中单元格数:** 12"),
            "report must count 12 real replacements, got: {report}"
        );
        assert!(
            !report.contains("**命中单元格数:** 0"),
            "the old 0-hit defect must not reappear"
        );

        let ecmap_path = result.ecmap_path.expect("ecmap must be generated");
        let ecmap_bytes = fs::read(&ecmap_path).expect("read ecmap");
        let plain = crypto::decrypt_ecmap(&ecmap_bytes, "test-sandbox-pass").expect("decrypt ecmap");
        let doc: EcmapDocumentV1 = serde_json::from_slice(&plain).expect("parse ecmap document");
        assert_eq!(doc.entries.len(), 12, "ecmap must carry exactly 12 entries");
    }

    #[tokio::test]
    async fn csv_gb18030_gbk_encoded_bytes_parse_correctly() {
        let dir = unique_temp_dir("csv-gbk");
        // GBK bytes for "姓名,手机号\n张三,13900001234\n李四,139\n" (no BOM,
        // not valid UTF-8); computed with Python's `str.encode("gbk")`.
        let bytes: Vec<u8> = vec![
            0xD0, 0xD5, 0xC3, 0xFB, 0x2C, 0xCA, 0xD6, 0xBB, 0xFA, 0xBA, 0xC5, 0x0A, 0xD5, 0xC5,
            0xC8, 0xFD, 0x2C, 0x31, 0x33, 0x39, 0x30, 0x30, 0x30, 0x30, 0x31, 0x32, 0x33, 0x34,
            0x0A, 0xC0, 0xEE, 0xCB, 0xC4, 0x2C, 0x31, 0x33, 0x39, 0x0A,
        ];
        assert!(std::str::from_utf8(&bytes).is_err(), "fixture must not be valid UTF-8");
        let input = write_fixture_csv(&dir, &bytes);
        let structure = file_parser::parse_excel_structure_detailed(&input.to_string_lossy())
            .expect("parse gbk csv structure");
        assert_eq!(structure[0].headers, vec!["姓名", "手机号"]);
        assert_eq!(
            structure[0].column_samples[0],
            vec!["张三".to_string(), "李四".to_string()]
        );
    }

    #[tokio::test]
    async fn csv_malformed_unterminated_quote_fails_without_producing_an_artifact() {
        let dir = unique_temp_dir("csv-malformed");
        let input = write_fixture_csv(&dir, b"header_a,header_b\n\"unterminated,value\n");

        let structure_err =
            file_parser::parse_excel_structure_detailed(&input.to_string_lossy()).unwrap_err();
        assert!(structure_err.contains("Failed to read CSV file"));

        let config = ExcelMaskingConfig {
            input_file_path: input.to_string_lossy().to_string(),
            output_name_suffix: None,
            sheets: vec![],
            passphrase: Some("test-sandbox-pass".to_string()),
            retain_encrypted_source: Some(false),
            source_pass_mode: Some(EncSourcePassModeDto::SandboxReused),
            generate_ecmap: Some(true),
        };
        let output_dir = dir.join("out");
        let apply_err = excel_apply_masking(config, output_dir.to_string_lossy().to_string())
            .await
            .unwrap_err();
        assert!(apply_err.contains("Failed to read CSV file"));
        assert!(
            !output_dir.exists() || fs::read_dir(&output_dir).unwrap().next().is_none(),
            "no masked artifact should be produced for a corrupted CSV"
        );
    }

    fn phone_rule() -> ColumnMaskingRule {
        ColumnMaskingRule {
            strategy_id: "PHONE_MID4".to_string(),
            pattern: None,
            replacement: None,
            mask_char: None,
            keep_prefix: None,
            keep_suffix: None,
            enabled: true,
            rule_mode: Some(RULE_MODE_CANONICAL.to_string()),
        }
    }

    fn email_rule() -> ColumnMaskingRule {
        ColumnMaskingRule {
            strategy_id: "EMAIL_USER_MASK".to_string(),
            pattern: None,
            replacement: None,
            mask_char: None,
            keep_prefix: None,
            keep_suffix: None,
            enabled: true,
            rule_mode: Some(RULE_MODE_CANONICAL.to_string()),
        }
    }

    /// R1 (structure/preview/apply status) + R3 (apply must produce a
    /// correct, multi-sheet, fully-populated masked workbook) of
    /// TASK-EXCEL-P0-DYNAMIC-FAILURES-CLOSEOUT-001: a valid legacy OLE
    /// `.xls` must go through the real Tauri commands exactly like `.xlsx`,
    /// and the produced workbook must contain both worksheets, every data
    /// row, every empty cell and unmatched value preserved, only the
    /// targeted cells masked, and `.ecmap` entries individually verifiable
    /// against the produced file — the exact regression the second-round
    /// architect Review found (200 status, but only a header row and a
    /// missing second sheet).
    #[tokio::test]
    async fn xls_structure_preview_apply_and_masked_workbook_are_fully_correct() {
        let dir = unique_temp_dir("xls-r3-full");
        let input = write_fixture_xls(&dir);
        let input_path_str = input.to_string_lossy().to_string();

        let structure = file_parser::parse_excel_structure_detailed(&input_path_str)
            .expect("xls structure must parse");
        assert_eq!(structure.len(), 2, "sample.xls has exactly two sheets");
        assert_eq!(structure[0].name, "Sheet1");
        assert_eq!(structure[0].headers, vec!["Name", "Phone", "Email"]);
        assert_eq!(structure[1].name, "Sheet2");
        assert_eq!(structure[1].headers, vec!["Phone"]);

        let mut sheet1_rules = HashMap::new();
        sheet1_rules.insert("Phone".to_string(), phone_rule());
        sheet1_rules.insert("Email".to_string(), email_rule());
        let mut sheet2_rules = HashMap::new();
        sheet2_rules.insert("Phone".to_string(), phone_rule());

        let config = ExcelMaskingConfig {
            input_file_path: input_path_str.clone(),
            output_name_suffix: None,
            sheets: vec![
                SheetMaskingConfig {
                    sheet_name: "Sheet1".to_string(),
                    header_row: Some(0),
                    column_rules: sheet1_rules,
                    cell_overrides: vec![],
                },
                SheetMaskingConfig {
                    sheet_name: "Sheet2".to_string(),
                    header_row: Some(0),
                    column_rules: sheet2_rules,
                    cell_overrides: vec![],
                },
            ],
            passphrase: Some("xls-r3-test-pass".to_string()),
            retain_encrypted_source: Some(false),
            source_pass_mode: Some(EncSourcePassModeDto::SandboxReused),
            generate_ecmap: Some(true),
        };

        let preview = excel_preview_masking(config.clone(), Some(20))
            .await
            .expect("xls preview must succeed");
        let sheet1_preview = preview
            .sheets
            .iter()
            .find(|s| s.sheet_name == "Sheet1")
            .expect("Sheet1 preview must be present");
        let sheet2_preview = preview
            .sheets
            .iter()
            .find(|s| s.sheet_name == "Sheet2")
            .expect("Sheet2 preview must be present (not dropped)");
        // Tauri's preview is a flat per-non-empty-cell list (unlike
        // Runtime's per-row array), so it includes every non-empty cell —
        // masked or passed through unchanged — not just the masked ones:
        // row2/row3 contribute 3 cells each (Name+Phone+Email), and row4
        // (empty Phone/Email) contributes only its non-empty Name cell.
        assert_eq!(
            sheet1_preview.preview_rows.len(),
            7,
            "Sheet1: 3+3+1 non-empty cells across the 3 data rows, second sheet not dropped"
        );
        assert_eq!(sheet2_preview.preview_rows.len(), 1);

        let output_dir = dir.join("out");
        let result = excel_apply_masking(config, output_dir.to_string_lossy().to_string())
            .await
            .expect("xls apply masking must succeed");

        use calamine::{open_workbook_auto, Data, Reader};
        let mut workbook =
            open_workbook_auto(&result.masked_path).expect("open xls-derived masked workbook");
        assert_eq!(
            workbook.sheet_names().to_vec(),
            vec!["Sheet1".to_string(), "Sheet2".to_string()],
            "both original sheets must survive, in order"
        );

        let sheet1 = workbook.worksheet_range("Sheet1").expect("Sheet1 must exist");
        assert_eq!(cell_to_string_for_test(&sheet1, 0, 0), "Name");
        assert_eq!(cell_to_string_for_test(&sheet1, 1, 0), "Alice");
        assert_eq!(cell_to_string_for_test(&sheet1, 1, 1), "139****0000");
        assert_eq!(cell_to_string_for_test(&sheet1, 1, 2), "a***e@example.invalid");
        assert_eq!(cell_to_string_for_test(&sheet1, 2, 0), "Bob");
        assert_eq!(cell_to_string_for_test(&sheet1, 2, 1), "138****0000");
        assert_eq!(cell_to_string_for_test(&sheet1, 2, 2), "b*b@example.invalid");
        assert_eq!(
            cell_to_string_for_test(&sheet1, 3, 0),
            "中文",
            "the unmatched name in the row with empty Phone/Email must be preserved"
        );

        let sheet2 = workbook.worksheet_range("Sheet2").expect(
            "Sheet2 must exist in the masked workbook (this is the exact R3 regression)",
        );
        assert_eq!(cell_to_string_for_test(&sheet2, 0, 0), "Phone");
        assert_eq!(cell_to_string_for_test(&sheet2, 1, 0), "139****0000");

        let ecmap_bytes = fs::read(result.ecmap_path.expect("ecmap must be generated"))
            .expect("read ecmap file");
        let ecmap_json =
            crypto::decrypt_ecmap(&ecmap_bytes, "xls-r3-test-pass").expect("decrypt ecmap");
        let doc: EcmapDocumentV1 =
            serde_json::from_slice(&ecmap_json).expect("parse ecmap document");
        assert_eq!(
            doc.entries.len(),
            5,
            "exactly 5 real masked cells: Sheet1 rows 2-3 phone+email, Sheet2 row 2 phone"
        );
        let sheets_by_name: HashMap<&str, calamine::Range<Data>> =
            [("Sheet1", sheet1), ("Sheet2", sheet2)].into_iter().collect();
        for entry in &doc.entries {
            let range = sheets_by_name
                .get(entry.sheet.as_str())
                .unwrap_or_else(|| panic!("ecmap references unknown sheet {}", entry.sheet));
            let actual = cell_to_string_for_test(
                range,
                (entry.row_index - 1) as usize,
                (entry.col_index - 1) as usize,
            );
            assert_eq!(
                actual, entry.masked,
                "ecmap entry {}!R{}C{} must match the workbook",
                entry.sheet, entry.row_index, entry.col_index
            );
        }

        let report = std::fs::read_to_string(
            output_dir.join(format!(
                "{}_masked_report.md",
                Path::new(&input_path_str).file_stem().unwrap().to_string_lossy()
            )),
        )
        .expect("read report.md");
        assert!(
            report.contains(".xls") && report.contains("样式"),
            "report must explicitly state the .xls -> .xlsx pure-data downgrade, got: {report}"
        );
    }

    /// R-closeout (工作包 A, 双 Sheet .xls): 逐 Sheet 配置语义。只配置
    /// Sheet1 时，Sheet2 的所有单元格必须原样保留（不脱敏、不丢失），
    /// `.ecmap` 不得出现任何 Sheet2 条目；显式配置 Sheet2 后 Sheet2 才按
    /// 规则脱敏（既有 `xls_structure_preview_apply_and_masked_workbook_are_
    /// fully_correct` 已覆盖双 Sheet 都配置的路径）。
    #[tokio::test]
    async fn xls_sheet2_stays_unchanged_when_only_sheet1_is_configured() {
        let dir = unique_temp_dir("xls-sheet2-untouched");
        let input = write_fixture_xls(&dir);
        let input_path_str = input.to_string_lossy().to_string();

        let mut sheet1_rules = HashMap::new();
        sheet1_rules.insert("Phone".to_string(), phone_rule());
        sheet1_rules.insert("Email".to_string(), email_rule());

        let config = ExcelMaskingConfig {
            input_file_path: input_path_str.clone(),
            output_name_suffix: None,
            sheets: vec![SheetMaskingConfig {
                sheet_name: "Sheet1".to_string(),
                header_row: Some(0),
                column_rules: sheet1_rules,
                cell_overrides: vec![],
            }],
            passphrase: Some("xls-sheet2-test-pass".to_string()),
            retain_encrypted_source: Some(false),
            source_pass_mode: Some(EncSourcePassModeDto::SandboxReused),
            generate_ecmap: Some(true),
        };

        let output_dir = dir.join("out");
        let result = excel_apply_masking(config, output_dir.to_string_lossy().to_string())
            .await
            .expect("xls apply with only Sheet1 configured must succeed");

        use calamine::{open_workbook_auto, Data, Reader};
        let mut workbook =
            open_workbook_auto(&result.masked_path).expect("open xls-derived masked workbook");
        assert_eq!(
            workbook.sheet_names().to_vec(),
            vec!["Sheet1".to_string(), "Sheet2".to_string()],
            "both sheets must survive"
        );

        let sheet1 = workbook.worksheet_range("Sheet1").expect("Sheet1");
        assert_eq!(cell_to_string_for_test(&sheet1, 1, 1), "139****0000");

        // Sheet2 has no configured rules: every cell stays byte-identical.
        let sheet2 = workbook.worksheet_range("Sheet2").expect("Sheet2 must exist");
        assert_eq!(cell_to_string_for_test(&sheet2, 0, 0), "Phone");
        assert_eq!(
            cell_to_string_for_test(&sheet2, 1, 0),
            "13900000000",
            "Sheet2 phone must stay unmasked"
        );

        // The ecmap contains zero Sheet2 entries.
        let ecmap_bytes = fs::read(result.ecmap_path.expect("ecmap must be generated"))
            .expect("read ecmap");
        let plain = crypto::decrypt_ecmap(&ecmap_bytes, "xls-sheet2-test-pass")
            .expect("decrypt ecmap");
        let doc: EcmapDocumentV1 =
            serde_json::from_slice(&plain).expect("parse ecmap document");
        assert!(
            doc.entries.iter().all(|entry| entry.sheet == "Sheet1"),
            "no Sheet2 entry may exist when only Sheet1 is configured: {:?}",
            doc.entries
                .iter()
                .map(|entry| (&entry.sheet, &entry.cell_ref))
                .collect::<Vec<_>>()
        );
    }

    /// R1: a corrupted/disguised `.xls` (garbage bytes, not real OLE) must
    /// fail closed instead of a panic or false success, confirming the
    /// format-routing fix still validates real content, not just the
    /// extension.
    #[tokio::test]
    async fn xls_corrupted_bytes_fail_closed() {
        let dir = unique_temp_dir("xls-corrupted");
        let path = dir.join("fake.xls");
        fs::write(&path, b"this is not a real OLE .xls file, just plain bytes")
            .expect("write corrupted xls fixture");

        let err = file_parser::parse_excel_structure_detailed(&path.to_string_lossy())
            .expect_err("corrupted xls must fail closed, not succeed");
        assert!(err.contains("Failed to open Excel file"));
    }

    // --- R3: canonical key-source enum + legacy compatibility ---

    #[test]
    fn canonical_key_source_uses_new_enum_values() {
        assert_eq!(canonical_key_source(None), KEY_SOURCE_SANDBOX);
        assert_eq!(
            canonical_key_source(Some(&EncSourcePassModeDto::SandboxReused)),
            KEY_SOURCE_SANDBOX
        );
        assert_eq!(
            canonical_key_source(Some(&EncSourcePassModeDto::SecondaryPhrase(
                "x".to_string()
            ))),
            KEY_SOURCE_SEPARATE
        );
        assert_eq!(
            canonical_key_source(Some(&EncSourcePassModeDto::DeviceKey)),
            KEY_SOURCE_DEVICE
        );
    }

    #[test]
    fn normalize_key_source_accepts_new_and_legacy_values_and_rejects_unknown() {
        for (raw, expected) in [
            ("SANDBOX_PASSPHRASE_REUSED", KEY_SOURCE_SANDBOX),
            ("SandboxReused", KEY_SOURCE_SANDBOX),
            ("SEPARATE_PASSPHRASE", KEY_SOURCE_SEPARATE),
            ("SecondaryPassphrase", KEY_SOURCE_SEPARATE),
            ("SecondaryPhrase", KEY_SOURCE_SEPARATE),
            ("DEVICE_KEY", KEY_SOURCE_DEVICE),
            ("DeviceKey", KEY_SOURCE_DEVICE),
        ] {
            assert_eq!(normalize_key_source(raw).unwrap(), expected, "raw={raw}");
        }
        assert!(normalize_key_source("SomethingElse").is_err());
    }

    #[test]
    fn ecmap_header_reads_legacy_snake_case_and_defaults_missing_source_retained() {
        let legacy_json = r#"{
            "version": "1.1",
            "original_sha256": "aa",
            "masked_sha256": "bb",
            "source_encryption_key_source": "SandboxReused",
            "passphrase_domain_hint8": "deadbeef"
        }"#;
        let header: EcmapHeaderV1 =
            serde_json::from_str(legacy_json).expect("legacy header must deserialize");
        assert_eq!(header.original_sha256, "aa");
        assert_eq!(header.masked_sha256, "bb");
        assert_eq!(header.source_encryption_key_source, "SandboxReused");
        assert_eq!(header.passphrase_domain_hint8, "deadbeef");
        assert!(!header.source_retained);

        let new_json = r#"{
            "version": "1.2",
            "originalSha256": "cc",
            "maskedSha256": "dd",
            "sourceEncryptionKeySource": "SANDBOX_PASSPHRASE_REUSED",
            "passphraseDomainHint8": "cafebabe",
            "sourceRetained": true
        }"#;
        let header2: EcmapHeaderV1 =
            serde_json::from_str(new_json).expect("new header must deserialize");
        assert!(header2.source_retained);
        let serialized = serde_json::to_string(&header2).unwrap();
        assert!(serialized.contains("\"sourceRetained\":true"));
        assert!(serialized.contains("\"sourceEncryptionKeySource\""));
        assert!(!serialized.contains("source_encryption_key_source"));
    }

    // --- End-to-end: retain=false / retain=true artifact sets + header +
    // cell-override strategy application, via the real Tauri commands. ---

    #[tokio::test]
    async fn apply_masking_retain_false_produces_no_encrypted_source_and_header_matches() {
        let dir = unique_temp_dir("retain-false");
        let input = write_fixture_xlsx(
            &dir,
            &["姓名", "身份证号"],
            &[
                vec!["张三", "123456789012345678"],
                vec!["李四", "123456789012345"],
            ],
        );

        let mut column_rules = HashMap::new();
        column_rules.insert(
            "身份证号".to_string(),
            ColumnMaskingRule {
                strategy_id: "IDCARD_MID10".to_string(),
                pattern: None,
                replacement: None,
                mask_char: None,
                keep_prefix: None,
                keep_suffix: None,
                enabled: true,
                rule_mode: Some(RULE_MODE_CANONICAL.to_string()),
            },
        );

        let config = ExcelMaskingConfig {
            input_file_path: input.to_string_lossy().to_string(),
            output_name_suffix: None,
            sheets: vec![SheetMaskingConfig {
                sheet_name: "Sheet1".to_string(),
                header_row: Some(0),
                column_rules,
                cell_overrides: vec![],
            }],
            passphrase: Some("test-sandbox-pass".to_string()),
            retain_encrypted_source: Some(false),
            source_pass_mode: Some(EncSourcePassModeDto::SandboxReused),
            generate_ecmap: Some(true),
        };

        let output_dir = dir.join("out");
        let result = excel_apply_masking(config.clone(), output_dir.to_string_lossy().to_string())
            .await
            .expect("apply masking must succeed");

        assert!(result.encrypted_source_path.is_none());
        let ecmap_path = result.ecmap_path.expect("ecmap must be generated");
        let ecmap_bytes = fs::read(&ecmap_path).expect("read ecmap");
        let plain = crypto::decrypt_ecmap(&ecmap_bytes, "test-sandbox-pass")
            .expect("ecmap must decrypt with the sandbox passphrase");
        let doc: EcmapDocumentV1 = serde_json::from_slice(&plain).expect("parse ecmap json");
        assert!(!doc.header.source_retained);
        assert_eq!(doc.header.source_encryption_key_source, KEY_SOURCE_SANDBOX);

        let masked_entry = doc
            .entries
            .iter()
            .find(|e| e.cell_ref == "B2")
            .expect("masked entry for the 18-digit id card cell");
        assert_eq!(masked_entry.masked, "123456********5678");
    }

    #[tokio::test]
    async fn apply_masking_retain_true_produces_encrypted_source_and_header_matches() {
        let dir = unique_temp_dir("retain-true");
        let input = write_fixture_xlsx(
            &dir,
            &["姓名", "身份证号"],
            &[vec!["王五", "123456789012345"]],
        );

        let mut column_rules = HashMap::new();
        column_rules.insert(
            "身份证号".to_string(),
            ColumnMaskingRule {
                strategy_id: "IDCARD_MID10".to_string(),
                pattern: None,
                replacement: None,
                mask_char: None,
                keep_prefix: None,
                keep_suffix: None,
                enabled: true,
                rule_mode: Some(RULE_MODE_CANONICAL.to_string()),
            },
        );

        let config = ExcelMaskingConfig {
            input_file_path: input.to_string_lossy().to_string(),
            output_name_suffix: None,
            sheets: vec![SheetMaskingConfig {
                sheet_name: "Sheet1".to_string(),
                header_row: Some(0),
                column_rules,
                cell_overrides: vec![],
            }],
            passphrase: Some("test-sandbox-pass-2".to_string()),
            retain_encrypted_source: Some(true),
            source_pass_mode: Some(EncSourcePassModeDto::SandboxReused),
            generate_ecmap: Some(true),
        };

        let output_dir = dir.join("out");
        let result = excel_apply_masking(config, output_dir.to_string_lossy().to_string())
            .await
            .expect("apply masking must succeed");

        let encsrc_path = result
            .encrypted_source_path
            .expect(".encrypted_src must be generated when retain=true");
        assert!(fs::metadata(&encsrc_path).is_ok());

        let ecmap_bytes = fs::read(result.ecmap_path.unwrap()).expect("read ecmap");
        let plain = crypto::decrypt_ecmap(&ecmap_bytes, "test-sandbox-pass-2")
            .expect("ecmap must decrypt");
        let doc: EcmapDocumentV1 = serde_json::from_slice(&plain).expect("parse ecmap json");
        assert!(doc.header.source_retained);
    }

    /// UI-STATE-004: 空/空白沙箱口令必须在使用前失败（先校验口令、后写文件），
    /// 且不得残留任何 masked/report 半成品（曾真实复现：`masked.xlsx` 与报告
    /// 已写出、`.ecmap` 加密失败）。
    #[tokio::test]
    async fn apply_masking_with_empty_sandbox_passphrase_fails_with_zero_artifacts() {
        for passphrase in ["", "   ", "\t\n"] {
            let dir = unique_temp_dir("empty-sandbox-apply");
            let input = write_fixture_xlsx(
                &dir,
                &["姓名", "手机号"],
                &[vec!["张三", "13900001234"]],
            );
            let config = ExcelMaskingConfig {
                input_file_path: input.to_string_lossy().to_string(),
                output_name_suffix: None,
                sheets: vec![SheetMaskingConfig {
                    sheet_name: "Sheet1".to_string(),
                    header_row: Some(0),
                    column_rules: phone_column_rules(),
                    cell_overrides: vec![],
                }],
                passphrase: Some(passphrase.to_string()),
                retain_encrypted_source: Some(false),
                source_pass_mode: Some(EncSourcePassModeDto::SandboxReused),
                generate_ecmap: Some(true),
            };
            let output_dir = dir.join("out");
            let err = excel_apply_masking(config, output_dir.to_string_lossy().to_string())
                .await
                .expect_err("empty sandbox passphrase must fail before writing any artifact");
            assert!(err.contains("沙箱口令不能为空"), "got: {err}");
            assert!(
                !output_dir.exists() || fs::read_dir(&output_dir).unwrap().next().is_none(),
                "no masked/report artifact may be left behind for passphrase={passphrase:?}"
            );
        }
    }

    /// UI-STATE-004: 空独立二级口令同样在写任何产物前失败且零残留。
    #[tokio::test]
    async fn apply_masking_with_empty_secondary_passphrase_fails_with_zero_artifacts() {
        let dir = unique_temp_dir("empty-secondary-apply");
        let input = write_fixture_xlsx(
            &dir,
            &["姓名", "手机号"],
            &[vec!["张三", "13900001234"]],
        );
        let config = ExcelMaskingConfig {
            input_file_path: input.to_string_lossy().to_string(),
            output_name_suffix: None,
            sheets: vec![SheetMaskingConfig {
                sheet_name: "Sheet1".to_string(),
                header_row: Some(0),
                column_rules: phone_column_rules(),
                cell_overrides: vec![],
            }],
            passphrase: Some("sandbox-fallback".to_string()),
            retain_encrypted_source: Some(true),
            source_pass_mode: Some(EncSourcePassModeDto::SecondaryPhrase("".to_string())),
            generate_ecmap: Some(true),
        };
        let output_dir = dir.join("out");
        let err = excel_apply_masking(config, output_dir.to_string_lossy().to_string())
            .await
            .expect_err("empty secondary passphrase must fail before writing any artifact");
        assert!(err.contains("独立二级口令不能为空"), "got: {err}");
        assert!(
            !output_dir.exists() || fs::read_dir(&output_dir).unwrap().next().is_none(),
            "no masked/report artifact may be left behind"
        );
    }

    /// R-closeout (工作包 D): SECONDARY_PASSPHRASE 完整闭环。脱敏用独立二级
    /// 口令加密 `.ecmap`/`.encrypted_src`，header hint 必须是该二级口令的
    /// hint（而不是沙箱口令的）；路径 A / 路径 B 用正确二级口令恢复成功，
    /// 错误口令安全失败。
    #[tokio::test]
    async fn secondary_passphrase_masks_restores_via_path_a_and_b_and_rejects_wrong_passphrase() {
        let dir = unique_temp_dir("secondary-passphrase");
        let input = write_fixture_xlsx(
            &dir,
            &["姓名", "手机号"],
            &[vec!["张三", "13900001234"]],
        );

        let config = ExcelMaskingConfig {
            input_file_path: input.to_string_lossy().to_string(),
            output_name_suffix: None,
            sheets: vec![SheetMaskingConfig {
                sheet_name: "Sheet1".to_string(),
                header_row: Some(0),
                column_rules: phone_column_rules(),
                cell_overrides: vec![],
            }],
            passphrase: Some("sandbox-fallback-pass".to_string()),
            retain_encrypted_source: Some(true),
            source_pass_mode: Some(EncSourcePassModeDto::SecondaryPhrase(
                "fixture-secondary-pass".to_string(),
            )),
            generate_ecmap: Some(true),
        };

        let output_dir = dir.join("out");
        let result = excel_apply_masking(config, output_dir.to_string_lossy().to_string())
            .await
            .expect("secondary passphrase masking must succeed");
        assert!(
            result.encrypted_source_path.is_some(),
            "retain=true must produce .encrypted_src"
        );

        let ecmap_path = result.ecmap_path.clone().expect("ecmap must be generated");
        let ecmap_bytes = fs::read(&ecmap_path).expect("read ecmap");
        // The ecmap is encrypted with the secondary phrase, not the sandbox
        // fallback.
        let sandbox_decrypt = crypto::decrypt_ecmap(&ecmap_bytes, "sandbox-fallback-pass");
        assert!(
            sandbox_decrypt.is_err(),
            "the sandbox passphrase must NOT decrypt a secondary-passphrase ecmap"
        );
        let plain =
            crypto::decrypt_ecmap(&ecmap_bytes, "fixture-secondary-pass")
                .expect("decrypt ecmap with the secondary phrase");
        let doc: EcmapDocumentV1 = serde_json::from_slice(&plain).expect("parse ecmap json");
        assert_eq!(doc.header.source_encryption_key_source, KEY_SOURCE_SEPARATE);
        assert!(doc.header.source_retained);
        let expected_hint =
            crypto::domain_hint8("fixture-secondary-pass", KeyDomain::EcmapV1);
        assert_eq!(
            doc.header.passphrase_domain_hint8, expected_hint,
            "the header hint must match the effective (secondary) passphrase"
        );
        assert_ne!(
            doc.header.passphrase_domain_hint8,
            crypto::domain_hint8("sandbox-fallback-pass", KeyDomain::EcmapV1),
            "the header hint must not be the sandbox passphrase hint"
        );

        // Path A: correct secondary passphrase restores the original bytes.
        let original_bytes = fs::read(&input).expect("read original");
        let restore_a = ExcelRestoreReq {
            masked_file_path: result.masked_path.clone(),
            ecmap_file_path: ecmap_path.clone(),
            encrypted_source_path: result.encrypted_source_path.clone(),
            user_original_file_path: None,
            output_path: dir.join("restored-a.xlsx").to_string_lossy().to_string(),
            passphrase: Some("fixture-secondary-pass".to_string()),
        };
        let restored_a = excel_restore_from_ecmap(restore_a)
            .await
            .expect("path A restore with the secondary phrase must succeed");
        assert!(restored_a.sha256_verified);
        assert_eq!(
            fs::read(&restored_a.restored_path).expect("read restored-a"),
            original_bytes
        );

        // Wrong secondary passphrase fails closed with a safe message.
        let restore_bad = ExcelRestoreReq {
            masked_file_path: result.masked_path.clone(),
            ecmap_file_path: ecmap_path.clone(),
            encrypted_source_path: result.encrypted_source_path.clone(),
            user_original_file_path: None,
            output_path: dir.join("restored-bad.xlsx").to_string_lossy().to_string(),
            passphrase: Some("wrong-secondary-pass".to_string()),
        };
        let err = excel_restore_from_ecmap(restore_bad)
            .await
            .expect_err("a wrong secondary passphrase must fail safely");
        assert!(err.contains("ECMAP 解密失败"), "got: {err}");
        assert!(
            !dir.join("restored-bad.xlsx").exists(),
            "no restored artifact may be produced on failure"
        );

        // Path B: user original + correct secondary passphrase.
        let restore_b = ExcelRestoreReq {
            masked_file_path: result.masked_path.clone(),
            ecmap_file_path: ecmap_path.clone(),
            encrypted_source_path: None,
            user_original_file_path: Some(input.to_string_lossy().to_string()),
            output_path: dir.join("restored-b.xlsx").to_string_lossy().to_string(),
            passphrase: Some("fixture-secondary-pass".to_string()),
        };
        let restored_b = excel_restore_from_ecmap(restore_b)
            .await
            .expect("path B restore with the secondary phrase must succeed");
        assert!(restored_b.sha256_verified);
        assert_eq!(
            fs::read(&restored_b.restored_path).expect("read restored-b"),
            original_bytes
        );
    }

    // --- Cell overrides now carry a strategy + optional replacement,
    // matching the Runtime host, instead of a pre-computed literal value. ---

    #[tokio::test]
    async fn cell_override_applies_its_own_strategy_at_mask_time() {
        let dir = unique_temp_dir("cell-override");
        let input = write_fixture_xlsx(
            &dir,
            &["姓名", "备注"],
            &[vec!["赵六", "13900001234"]],
        );

        let config = ExcelMaskingConfig {
            input_file_path: input.to_string_lossy().to_string(),
            output_name_suffix: None,
            sheets: vec![SheetMaskingConfig {
                sheet_name: "Sheet1".to_string(),
                header_row: Some(0),
                column_rules: HashMap::new(),
                cell_overrides: vec![CellOverride {
                    cell_ref: "B2".to_string(),
                    strategy_id: "PHONE_MID4".to_string(),
                    replacement: None,
                }],
            }],
            passphrase: Some("test-sandbox-pass-3".to_string()),
            retain_encrypted_source: Some(false),
            source_pass_mode: Some(EncSourcePassModeDto::SandboxReused),
            generate_ecmap: Some(true),
        };

        let output_dir = dir.join("out");
        let result = excel_apply_masking(config, output_dir.to_string_lossy().to_string())
            .await
            .expect("apply masking must succeed");

        let ecmap_bytes = fs::read(result.ecmap_path.unwrap()).expect("read ecmap");
        let plain = crypto::decrypt_ecmap(&ecmap_bytes, "test-sandbox-pass-3")
            .expect("ecmap must decrypt");
        let doc: EcmapDocumentV1 = serde_json::from_slice(&plain).expect("parse ecmap json");
        let entry = doc
            .entries
            .iter()
            .find(|e| e.cell_ref == "B2")
            .expect("overridden cell must be masked");
        assert_eq!(entry.masked, "139****1234");
        assert_eq!(entry.strategy_id, "PHONE_MID4");
    }

    // --- Restore path B must reject a user-provided file whose SHA-256
    // does not match the ecmap header, never guess. ---

    #[tokio::test]
    async fn restore_path_b_rejects_mismatched_user_original_sha256() {
        let dir = unique_temp_dir("restore-b");
        let input = write_fixture_xlsx(&dir, &["姓名"], &[vec!["张三"]]);

        let config = ExcelMaskingConfig {
            input_file_path: input.to_string_lossy().to_string(),
            output_name_suffix: None,
            sheets: vec![SheetMaskingConfig {
                sheet_name: "Sheet1".to_string(),
                header_row: Some(0),
                column_rules: HashMap::new(),
                cell_overrides: vec![],
            }],
            passphrase: Some("test-sandbox-pass-4".to_string()),
            retain_encrypted_source: Some(false),
            source_pass_mode: Some(EncSourcePassModeDto::SandboxReused),
            generate_ecmap: Some(true),
        };

        let output_dir = dir.join("out");
        let result = excel_apply_masking(config, output_dir.to_string_lossy().to_string())
            .await
            .expect("apply masking must succeed");

        // A different file (wrong SHA-256) must not be accepted as "the original".
        let wrong_original = dir.join("wrong_original.xlsx");
        fs::write(&wrong_original, b"not the real original bytes").unwrap();

        let restore_req = ExcelRestoreReq {
            masked_file_path: result.masked_path,
            ecmap_file_path: result.ecmap_path.unwrap(),
            encrypted_source_path: None,
            user_original_file_path: Some(wrong_original.to_string_lossy().to_string()),
            output_path: dir.join("restored.xlsx").to_string_lossy().to_string(),
            passphrase: Some("test-sandbox-pass-4".to_string()),
        };

        let err = excel_restore_from_ecmap(restore_req)
            .await
            .expect_err("mismatched SHA-256 must be rejected, never guessed");
        assert!(err.contains("拒绝猜测式还原"));
    }

    // --- R6: restore path A must validate the .ecmap header and material
    // consistency before trusting the decrypted .encrypted_src, not just
    // that it decrypts successfully. ---

    #[tokio::test]
    async fn restore_path_a_succeeds_with_correct_new_header_and_matching_materials() {
        let dir = unique_temp_dir("restore-a-ok");
        let input = write_fixture_xlsx(&dir, &["姓名"], &[vec!["张三"]]);
        let config = ExcelMaskingConfig {
            input_file_path: input.to_string_lossy().to_string(),
            output_name_suffix: None,
            sheets: vec![SheetMaskingConfig {
                sheet_name: "Sheet1".to_string(),
                header_row: Some(0),
                column_rules: HashMap::new(),
                cell_overrides: vec![],
            }],
            passphrase: Some("test-restore-a-pass".to_string()),
            retain_encrypted_source: Some(true),
            source_pass_mode: Some(EncSourcePassModeDto::SandboxReused),
            generate_ecmap: Some(true),
        };
        let output_dir = dir.join("out");
        let apply_result =
            excel_apply_masking(config, output_dir.to_string_lossy().to_string())
                .await
                .expect("apply must succeed");

        let restore_req = ExcelRestoreReq {
            masked_file_path: apply_result.masked_path.clone(),
            ecmap_file_path: apply_result.ecmap_path.clone().unwrap(),
            encrypted_source_path: apply_result.encrypted_source_path.clone(),
            user_original_file_path: None,
            output_path: dir.join("restored.xlsx").to_string_lossy().to_string(),
            passphrase: Some("test-restore-a-pass".to_string()),
        };
        let result = excel_restore_from_ecmap(restore_req)
            .await
            .expect("path A restore with a correct new header must succeed");
        assert!(result.sha256_verified);
        let restored_bytes = fs::read(&result.restored_path).unwrap();
        let original_bytes = fs::read(&input).unwrap();
        assert_eq!(sha256_hex(&restored_bytes), sha256_hex(&original_bytes));
    }

    #[tokio::test]
    async fn restore_path_a_rejects_when_header_declares_source_not_retained() {
        let dir = unique_temp_dir("restore-a-not-retained");
        let input = write_fixture_xlsx(&dir, &["姓名"], &[vec!["李四"]]);
        let config = ExcelMaskingConfig {
            input_file_path: input.to_string_lossy().to_string(),
            output_name_suffix: None,
            sheets: vec![SheetMaskingConfig {
                sheet_name: "Sheet1".to_string(),
                header_row: Some(0),
                column_rules: HashMap::new(),
                cell_overrides: vec![],
            }],
            passphrase: Some("test-restore-a-notretained".to_string()),
            retain_encrypted_source: Some(false),
            source_pass_mode: Some(EncSourcePassModeDto::SandboxReused),
            generate_ecmap: Some(true),
        };
        let output_dir = dir.join("out");
        let apply_result =
            excel_apply_masking(config, output_dir.to_string_lossy().to_string())
                .await
                .expect("apply must succeed");
        assert!(apply_result.encrypted_source_path.is_none());

        // Craft a plausible-looking .encrypted_src that a caller is (incorrectly)
        // presenting alongside a header that explicitly says nothing was retained.
        let fake_encsrc = crypto::encrypt_encsrc(
            b"not the real original file",
            "test-restore-a-notretained",
            EncSourcePassMode::SandboxReused,
        )
        .unwrap();
        let fake_encsrc_path = dir.join("fake.encrypted_src");
        fs::write(&fake_encsrc_path, &fake_encsrc).unwrap();

        let restore_req = ExcelRestoreReq {
            masked_file_path: apply_result.masked_path.clone(),
            ecmap_file_path: apply_result.ecmap_path.clone().unwrap(),
            encrypted_source_path: Some(fake_encsrc_path.to_string_lossy().to_string()),
            user_original_file_path: None,
            output_path: dir.join("restored.xlsx").to_string_lossy().to_string(),
            passphrase: Some("test-restore-a-notretained".to_string()),
        };
        let err = excel_restore_from_ecmap(restore_req)
            .await
            .expect_err("sourceRetained=false must contradict a presented .encrypted_src");
        assert!(err.contains("拒绝猜测式还原"));
        assert!(err.contains("矛盾"));
    }

    #[tokio::test]
    async fn restore_path_a_rejects_unknown_key_source_in_header() {
        let dir = unique_temp_dir("restore-a-unknown-keysource");
        let bad_doc_json = serde_json::json!({
            "header": {
                "version": "1.2",
                "originalSha256": "aa",
                "maskedSha256": "bb",
                "sourceEncryptionKeySource": "TOTALLY_UNKNOWN_MODE",
                "passphraseDomainHint8": "deadbeef",
                "sourceRetained": true
            },
            "entries": []
        });
        let ecmap_bytes = crypto::encrypt_ecmap(
            bad_doc_json.to_string().as_bytes(),
            "test-restore-a-unknown",
            EncSourcePassMode::SandboxReused,
        )
        .unwrap();
        let ecmap_path = dir.join("unknown_keysource.ecmap");
        fs::write(&ecmap_path, &ecmap_bytes).unwrap();

        let restore_req = ExcelRestoreReq {
            masked_file_path: dir.join("does_not_matter.xlsx").to_string_lossy().to_string(),
            ecmap_file_path: ecmap_path.to_string_lossy().to_string(),
            encrypted_source_path: Some(
                dir.join("does_not_matter.encrypted_src")
                    .to_string_lossy()
                    .to_string(),
            ),
            user_original_file_path: None,
            output_path: dir.join("restored.xlsx").to_string_lossy().to_string(),
            passphrase: Some("test-restore-a-unknown".to_string()),
        };
        let err = excel_restore_from_ecmap(restore_req)
            .await
            .expect_err("unknown key source must be rejected before touching the encrypted source");
        assert!(err.contains("ECMAP header 校验失败"));
    }

    #[tokio::test]
    async fn restore_path_a_rejects_masked_file_sha_mismatch() {
        let dir = unique_temp_dir("restore-a-masked-mismatch");
        let input = write_fixture_xlsx(&dir, &["姓名"], &[vec!["王五"]]);
        let config = ExcelMaskingConfig {
            input_file_path: input.to_string_lossy().to_string(),
            output_name_suffix: None,
            sheets: vec![SheetMaskingConfig {
                sheet_name: "Sheet1".to_string(),
                header_row: Some(0),
                column_rules: HashMap::new(),
                cell_overrides: vec![],
            }],
            passphrase: Some("test-restore-a-maskedmismatch".to_string()),
            retain_encrypted_source: Some(true),
            source_pass_mode: Some(EncSourcePassModeDto::SandboxReused),
            generate_ecmap: Some(true),
        };
        let output_dir = dir.join("out");
        let apply_result =
            excel_apply_masking(config, output_dir.to_string_lossy().to_string())
                .await
                .expect("apply must succeed");

        let wrong_masked = dir.join("wrong_masked.xlsx");
        fs::write(&wrong_masked, b"not the real masked bytes").unwrap();

        let restore_req = ExcelRestoreReq {
            masked_file_path: wrong_masked.to_string_lossy().to_string(),
            ecmap_file_path: apply_result.ecmap_path.clone().unwrap(),
            encrypted_source_path: apply_result.encrypted_source_path.clone(),
            user_original_file_path: None,
            output_path: dir.join("restored.xlsx").to_string_lossy().to_string(),
            passphrase: Some("test-restore-a-maskedmismatch".to_string()),
        };
        let err = excel_restore_from_ecmap(restore_req)
            .await
            .expect_err("masked file SHA mismatch must be rejected");
        assert!(err.contains("maskedSha256"));
        assert!(err.contains("拒绝猜测式还原"));
    }

    #[tokio::test]
    async fn restore_path_a_rejects_decrypted_source_sha_mismatch() {
        let dir = unique_temp_dir("restore-a-source-mismatch");
        let input = write_fixture_xlsx(&dir, &["姓名"], &[vec!["赵六"]]);
        let config = ExcelMaskingConfig {
            input_file_path: input.to_string_lossy().to_string(),
            output_name_suffix: None,
            sheets: vec![SheetMaskingConfig {
                sheet_name: "Sheet1".to_string(),
                header_row: Some(0),
                column_rules: HashMap::new(),
                cell_overrides: vec![],
            }],
            passphrase: Some("test-restore-a-srcmismatch".to_string()),
            retain_encrypted_source: Some(true),
            source_pass_mode: Some(EncSourcePassModeDto::SandboxReused),
            generate_ecmap: Some(true),
        };
        let output_dir = dir.join("out");
        let apply_result =
            excel_apply_masking(config, output_dir.to_string_lossy().to_string())
                .await
                .expect("apply must succeed");

        // A different, correctly-encrypted .encrypted_src using the same
        // passphrase: it decrypts fine, but its plaintext SHA does not match
        // the real header.originalSha256.
        let wrong_encsrc = crypto::encrypt_encsrc(
            b"a completely different original file",
            "test-restore-a-srcmismatch",
            EncSourcePassMode::SandboxReused,
        )
        .unwrap();
        let wrong_encsrc_path = dir.join("wrong.encrypted_src");
        fs::write(&wrong_encsrc_path, &wrong_encsrc).unwrap();

        let restore_req = ExcelRestoreReq {
            masked_file_path: apply_result.masked_path.clone(),
            ecmap_file_path: apply_result.ecmap_path.clone().unwrap(),
            encrypted_source_path: Some(wrong_encsrc_path.to_string_lossy().to_string()),
            user_original_file_path: None,
            output_path: dir.join("restored.xlsx").to_string_lossy().to_string(),
            passphrase: Some("test-restore-a-srcmismatch".to_string()),
        };
        let err = excel_restore_from_ecmap(restore_req)
            .await
            .expect_err("decrypted source SHA mismatch must be rejected");
        assert!(err.contains("originalSha256"));
        assert!(err.contains("拒绝猜测式还原"));
    }

    #[tokio::test]
    async fn restore_path_a_succeeds_with_legacy_header_missing_source_retained() {
        let dir = unique_temp_dir("restore-a-legacy-header");
        // The compatibility fixture must still be a recognizable legacy Excel
        // payload now that restore validates the final suffix against bytes.
        let mut original_bytes = LEGACY_XLS_MAGIC.to_vec();
        original_bytes.extend_from_slice(b"legacy original file bytes");
        let pass = "test-restore-a-legacy";

        let encsrc_bytes =
            crypto::encrypt_encsrc(&original_bytes, pass, EncSourcePassMode::SandboxReused)
                .unwrap();
        let encsrc_path = dir.join("legacy.encrypted_src");
        fs::write(&encsrc_path, &encsrc_bytes).unwrap();

        let masked_bytes = b"legacy masked file bytes".to_vec();
        let masked_path = dir.join("legacy_masked.xlsx");
        fs::write(&masked_path, &masked_bytes).unwrap();

        // A legacy-shaped header: old snake_case field names, old enum
        // value, and (crucially) no sourceRetained key at all.
        let legacy_doc_json = serde_json::json!({
            "header": {
                "version": "1.1",
                "original_sha256": sha256_hex(&original_bytes),
                "masked_sha256": sha256_hex(&masked_bytes),
                "source_encryption_key_source": "SandboxReused",
                "passphrase_domain_hint8": "deadbeef"
            },
            "entries": []
        });
        let ecmap_bytes = crypto::encrypt_ecmap(
            legacy_doc_json.to_string().as_bytes(),
            pass,
            EncSourcePassMode::SandboxReused,
        )
        .unwrap();
        let ecmap_path = dir.join("legacy.ecmap");
        fs::write(&ecmap_path, &ecmap_bytes).unwrap();

        let restore_req = ExcelRestoreReq {
            masked_file_path: masked_path.to_string_lossy().to_string(),
            ecmap_file_path: ecmap_path.to_string_lossy().to_string(),
            encrypted_source_path: Some(encsrc_path.to_string_lossy().to_string()),
            user_original_file_path: None,
            output_path: dir.join("restored.xlsx").to_string_lossy().to_string(),
            passphrase: Some(pass.to_string()),
        };
        let result = excel_restore_from_ecmap(restore_req)
            .await
            .expect("a legacy header without sourceRetained, with matching SHAs, must still succeed");
        let restored_bytes = fs::read(&result.restored_path).unwrap();
        assert_eq!(restored_bytes, original_bytes);
    }

    // --- R7 (second pass): field presence alone cannot disambiguate a
    // legacy replacement-only rule from a canonical replacement-only rule.
    // Dispatch is driven solely by the explicit `rule_mode` marker. ---

    #[test]
    fn legacy_column_rule_fields_take_precedence_when_rule_mode_is_absent() {
        let rule = ColumnMaskingRule {
            strategy_id: "IDCARD_MID10".to_string(),
            pattern: Some("ignored-pattern".to_string()),
            replacement: Some("SHOULD_NOT_BE_USED_LITERALLY".to_string()),
            mask_char: Some('#'),
            keep_prefix: Some(1),
            keep_suffix: Some(1),
            enabled: true,
            rule_mode: None,
        };
        // Strategy-based masking of an 18-digit id would give
        // "123456********5678"; without a canonical rule_mode marker the
        // legacy keep_prefix=1/keep_suffix=1/mask_char='#' fields take over
        // instead and strategy_id is ignored.
        assert_eq!(
            mask_value("123456789012345678", &rule),
            format!("1{}8", "#".repeat(16))
        );
    }

    #[test]
    fn legacy_replacement_only_rule_without_rule_mode_short_circuits_like_before() {
        let rule = ColumnMaskingRule {
            strategy_id: "PHONE_MID4".to_string(),
            pattern: None,
            replacement: Some("LEGACY_REPLACED".to_string()),
            mask_char: None,
            keep_prefix: None,
            keep_suffix: None,
            enabled: true,
            rule_mode: None,
        };
        // Only `replacement` is set and rule_mode is absent — by field
        // presence alone this is indistinguishable from the canonical
        // replacement-only rule below; the missing marker is what routes it
        // to the legacy short-circuit (pattern is None + replacement is Some
        // -> return the literal replacement verbatim, exactly as before R1).
        assert_eq!(mask_value("13900001234", &rule), "LEGACY_REPLACED");
    }

    #[test]
    fn canonical_replacement_only_rule_with_rule_mode_dispatches_by_strategy() {
        let rule = ColumnMaskingRule {
            strategy_id: "FULL_MASK".to_string(),
            pattern: None,
            replacement: Some("CANON_REPLACED".to_string()),
            mask_char: None,
            keep_prefix: None,
            keep_suffix: None,
            enabled: true,
            rule_mode: Some(RULE_MODE_CANONICAL.to_string()),
        };
        // Identical field shape to the legacy replacement-only rule above
        // (only `replacement` set), except for the explicit rule_mode
        // marker: this must go through apply_strategy() — FULL_MASK honors
        // `replacement` as its literal output too, but via strategy
        // dispatch, not the legacy short-circuit.
        assert_eq!(mask_value("13900001234", &rule), "CANON_REPLACED");
    }

    #[test]
    fn legacy_column_rule_disabled_returns_original_value() {
        let rule = ColumnMaskingRule {
            strategy_id: "FULL_MASK".to_string(),
            pattern: None,
            replacement: None,
            mask_char: None,
            keep_prefix: Some(0),
            keep_suffix: Some(0),
            enabled: false,
            rule_mode: None,
        };
        assert_eq!(mask_value("keep-me", &rule), "keep-me");
    }

    #[test]
    fn column_rule_with_canonical_rule_mode_dispatches_by_strategy() {
        let rule = ColumnMaskingRule {
            strategy_id: "IDCARD_MID10".to_string(),
            pattern: None,
            replacement: None,
            mask_char: None,
            keep_prefix: None,
            keep_suffix: None,
            enabled: true,
            rule_mode: Some(RULE_MODE_CANONICAL.to_string()),
        };
        assert_eq!(
            mask_value("123456789012345678", &rule),
            "123456********5678"
        );
    }

    #[tokio::test]
    async fn unknown_rule_mode_is_rejected_before_masking_starts() {
        let dir = unique_temp_dir("unknown-rule-mode");
        let input = write_fixture_xlsx(&dir, &["姓名"], &[vec!["张三"]]);

        let mut column_rules = HashMap::new();
        column_rules.insert(
            "姓名".to_string(),
            ColumnMaskingRule {
                strategy_id: "FULL_MASK".to_string(),
                pattern: None,
                replacement: None,
                mask_char: None,
                keep_prefix: None,
                keep_suffix: None,
                enabled: true,
                rule_mode: Some("SOME_FUTURE_MODE".to_string()),
            },
        );
        let config = ExcelMaskingConfig {
            input_file_path: input.to_string_lossy().to_string(),
            output_name_suffix: None,
            sheets: vec![SheetMaskingConfig {
                sheet_name: "Sheet1".to_string(),
                header_row: Some(0),
                column_rules,
                cell_overrides: vec![],
            }],
            passphrase: Some("test-unknown-mode".to_string()),
            retain_encrypted_source: Some(false),
            source_pass_mode: Some(EncSourcePassModeDto::SandboxReused),
            generate_ecmap: Some(true),
        };

        let output_dir = dir.join("out");
        let err = excel_apply_masking(config, output_dir.to_string_lossy().to_string())
            .await
            .expect_err("an unrecognized rule_mode must reject the whole command");
        assert!(err.contains("未知的规则模式标记"));
    }

    // --- R6: an explicit command-level regression test proving restore
    // path A also safely fails on a wrong passphrase (it already does, via
    // ECMAP decryption failing first). ---

    #[tokio::test]
    async fn restore_path_a_rejects_wrong_passphrase() {
        let dir = unique_temp_dir("restore-a-wrong-pass");
        let input = write_fixture_xlsx(&dir, &["姓名"], &[vec!["孙七"]]);
        let config = ExcelMaskingConfig {
            input_file_path: input.to_string_lossy().to_string(),
            output_name_suffix: None,
            sheets: vec![SheetMaskingConfig {
                sheet_name: "Sheet1".to_string(),
                header_row: Some(0),
                column_rules: HashMap::new(),
                cell_overrides: vec![],
            }],
            passphrase: Some("the-correct-passphrase".to_string()),
            retain_encrypted_source: Some(true),
            source_pass_mode: Some(EncSourcePassModeDto::SandboxReused),
            generate_ecmap: Some(true),
        };
        let output_dir = dir.join("out");
        let apply_result = excel_apply_masking(config, output_dir.to_string_lossy().to_string())
            .await
            .expect("apply must succeed");

        let restore_req = ExcelRestoreReq {
            masked_file_path: apply_result.masked_path.clone(),
            ecmap_file_path: apply_result.ecmap_path.clone().unwrap(),
            encrypted_source_path: apply_result.encrypted_source_path.clone(),
            user_original_file_path: None,
            output_path: dir.join("restored.xlsx").to_string_lossy().to_string(),
            passphrase: Some("a-completely-wrong-passphrase".to_string()),
        };
        let err = excel_restore_from_ecmap(restore_req)
            .await
            .expect_err("a wrong passphrase must fail safely, not restore anything");
        assert!(err.contains("ECMAP 解密失败"));
    }

    #[test]
    fn restore_output_extension_follows_actual_excel_bytes() {
        let legacy_ole = [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];
        let normalized_xls = normalize_excel_restore_output_path(
            Path::new("/tmp/fixture/employee_已还原.xlsx"),
            &legacy_ole,
        )
        .expect("legacy OLE bytes must be recognized as .xls");
        assert_eq!(
            normalized_xls,
            PathBuf::from("/tmp/fixture/employee_已还原.xls")
        );

        let ooxml_zip = b"PK\x03\x04fixture";
        let normalized_xlsx = normalize_excel_restore_output_path(
            Path::new("/tmp/fixture/employee_已还原.xls"),
            ooxml_zip,
        )
        .expect("OOXML ZIP bytes must be recognized as .xlsx");
        assert_eq!(
            normalized_xlsx,
            PathBuf::from("/tmp/fixture/employee_已还原.xlsx")
        );
    }

    #[tokio::test]
    async fn restore_paths_a_and_b_preserve_legacy_xls_bytes_and_suffix() {
        let dir = unique_temp_dir("restore-legacy-xls-format");
        let input = write_fixture_xls(&dir);
        let original_bytes = fs::read(&input).expect("read legacy xls fixture");
        assert!(original_bytes.starts_with(LEGACY_XLS_MAGIC));

        let config = ExcelMaskingConfig {
            input_file_path: input.to_string_lossy().to_string(),
            output_name_suffix: None,
            sheets: vec![SheetMaskingConfig {
                sheet_name: "Sheet1".to_string(),
                header_row: Some(0),
                column_rules: HashMap::new(),
                cell_overrides: vec![],
            }],
            passphrase: Some("restore-legacy-xls-pass".to_string()),
            retain_encrypted_source: Some(true),
            source_pass_mode: Some(EncSourcePassModeDto::SandboxReused),
            generate_ecmap: Some(true),
        };
        let result = excel_apply_masking(config, dir.join("out").to_string_lossy().to_string())
            .await
            .expect("legacy xls apply must succeed");

        let restore_a = excel_restore_from_ecmap(ExcelRestoreReq {
            masked_file_path: result.masked_path.clone(),
            ecmap_file_path: result.ecmap_path.clone().expect("ecmap path"),
            encrypted_source_path: result.encrypted_source_path.clone(),
            user_original_file_path: None,
            output_path: dir.join("path-a.xlsx").to_string_lossy().to_string(),
            passphrase: Some("restore-legacy-xls-pass".to_string()),
        })
        .await
        .expect("Path A legacy xls restore must succeed");
        assert!(restore_a.restored_path.ends_with("path-a.xls"));
        let restored_a = fs::read(&restore_a.restored_path).expect("read Path A restore");
        assert!(restored_a.starts_with(LEGACY_XLS_MAGIC));
        assert_eq!(restored_a, original_bytes);

        let restore_b = excel_restore_from_ecmap(ExcelRestoreReq {
            masked_file_path: result.masked_path,
            ecmap_file_path: result.ecmap_path.expect("ecmap path"),
            encrypted_source_path: None,
            user_original_file_path: Some(input.to_string_lossy().to_string()),
            output_path: dir.join("path-b.xlsx").to_string_lossy().to_string(),
            passphrase: Some("restore-legacy-xls-pass".to_string()),
        })
        .await
        .expect("Path B legacy xls restore must succeed");
        assert!(restore_b.restored_path.ends_with("path-b.xls"));
        let restored_b = fs::read(&restore_b.restored_path).expect("read Path B restore");
        assert!(restored_b.starts_with(LEGACY_XLS_MAGIC));
        assert_eq!(restored_b, original_bytes);
    }

    #[tokio::test]
    async fn restore_paths_reject_existing_normalized_target_without_overwrite() {
        let dir = unique_temp_dir("restore-normalized-target-exists");
        let input = write_fixture_xls(&dir);
        let config = ExcelMaskingConfig {
            input_file_path: input.to_string_lossy().to_string(),
            output_name_suffix: None,
            sheets: vec![SheetMaskingConfig {
                sheet_name: "Sheet1".to_string(),
                header_row: Some(0),
                column_rules: HashMap::new(),
                cell_overrides: vec![],
            }],
            passphrase: Some("restore-existing-target-pass".to_string()),
            retain_encrypted_source: Some(true),
            source_pass_mode: Some(EncSourcePassModeDto::SandboxReused),
            generate_ecmap: Some(true),
        };
        let result = excel_apply_masking(config, dir.join("out").to_string_lossy().to_string())
            .await
            .expect("legacy xls apply must succeed");
        let ecmap_path = result.ecmap_path.clone().expect("ecmap path");
        let encrypted_source_path = result
            .encrypted_source_path
            .clone()
            .expect("encrypted source path");

        let sentinel_a = b"existing Path A target must remain unchanged";
        let requested_a = dir.join("path-a.xlsx");
        let normalized_a = dir.join("path-a.xls");
        fs::write(&normalized_a, sentinel_a).expect("seed existing Path A target");
        let error_a = excel_restore_from_ecmap(ExcelRestoreReq {
            masked_file_path: result.masked_path.clone(),
            ecmap_file_path: ecmap_path.clone(),
            encrypted_source_path: Some(encrypted_source_path.clone()),
            user_original_file_path: None,
            output_path: requested_a.to_string_lossy().to_string(),
            passphrase: Some("restore-existing-target-pass".to_string()),
        })
        .await
        .expect_err("Path A must reject an occupied normalized target");
        assert_eq!(error_a, "恢复目标路径已存在，请重新选择文件名");
        assert_eq!(fs::read(&normalized_a).unwrap(), sentinel_a);
        assert!(!requested_a.exists(), "the unconfirmed requested path must stay absent");

        let original = fs::read(&input).expect("read original Path B bytes");
        let sentinel_b = b"existing Path B target must remain unchanged";
        let requested_b = dir.join("path-b.xlsx");
        let normalized_b = dir.join("path-b.xls");
        fs::write(&normalized_b, sentinel_b).expect("seed existing Path B target");
        let error_b = excel_restore_from_ecmap(ExcelRestoreReq {
            masked_file_path: result.masked_path,
            ecmap_file_path: ecmap_path,
            encrypted_source_path: None,
            user_original_file_path: Some(input.to_string_lossy().to_string()),
            output_path: requested_b.to_string_lossy().to_string(),
            passphrase: Some("restore-existing-target-pass".to_string()),
        })
        .await
        .expect_err("Path B must reject an occupied normalized target");
        assert_eq!(error_b, "恢复目标路径已存在，请重新选择文件名");
        assert_eq!(fs::read(&normalized_b).unwrap(), sentinel_b);
        assert!(!requested_b.exists(), "the unconfirmed requested path must stay absent");
        assert!(original.starts_with(LEGACY_XLS_MAGIC));
    }
}
