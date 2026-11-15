use crate::core::{crypto, database, file_parser, masking_engine, ner};
use engine_core::{DeterministicFinding, MaskingSession, SensitiveTermDefinition};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 获取文件的总页数
#[tauri::command]
pub async fn get_file_page_count(file_path: String) -> Result<usize, String> {
    file_parser::get_page_count(&file_path).map_err(|e| format!("Failed to get page count: {}", e))
}

/// 从数据库加载启用的敏感词
async fn load_sensitive_terms() -> Result<Vec<database::SensitiveTerm>, String> {
    let db = database::Database::new()
        .await
        .map_err(|e| format!("Failed to initialize database: {}", e))?;

    // 只加载启用的敏感词
    db.get_sensitive_terms(None, true)
        .await
        .map_err(|e| format!("Failed to load sensitive terms: {}", e))
}

/// 把桌面数据库记录投影为 engine-core 的纯数据定义，供唯一的共享规则构造
/// 函数 `engine_core::sensitive_term_rules` 使用；不在本文件重复构造规则。
fn sensitive_term_definitions(terms: &[database::SensitiveTerm]) -> Vec<SensitiveTermDefinition> {
    terms
        .iter()
        .map(|term| SensitiveTermDefinition {
            id: term.id.clone(),
            term: term.term.clone(),
            category: term.category.clone(),
            enabled: term.enabled,
        })
        .collect()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomRule {
    pub id: String,
    pub name: String,
    pub pattern: String,
    pub replacement_template: String,
    /// false = 直接使用 replacement_template 作为固定文本（不追加计数器）
    pub use_counter: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MaskFileOptions {
    pub file_path: String,
    pub output_path: String,
    pub rule_ids: Vec<String>,
    pub passphrase: Option<String>,
    pub custom_rules: Option<Vec<CustomRule>>,
    pub use_ai_validation: Option<bool>, // 是否使用 AI 验证姓名
    pub page_range: Option<(usize, usize)>, // 页码范围 (起始页, 结束页)，从 1 开始
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MaskResult {
    pub output_path: String,
    pub masked_count: usize,
    pub mapping_path: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PreviewOptions {
    pub file_path: String,
    pub rule_ids: Vec<String>,
    pub max_rows: Option<usize>,
    pub custom_rules: Option<Vec<CustomRule>>,
    pub use_ai_validation: Option<bool>,
    pub page_range: Option<(usize, usize)>, // 页码范围 (起始页, 结束页)，从 1 开始
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SavePreviewOptions {
    pub file_path: String,
    pub output_dir: String,
    pub masked_rows: Vec<Vec<String>>,
    pub headers: Option<Vec<String>>,
    pub passphrase: Option<String>,
    pub mapping: Option<Vec<masking_engine::MappingEntry>>,
    pub masked_file_stem: Option<String>,
    pub rule_ids: Option<Vec<String>>,
    pub custom_rules: Option<Vec<CustomRule>>,
    pub page_range: Option<(usize, usize)>, // 页码范围 (起始页, 结束页)，从 1 开始
    pub masked_entity_count: Option<usize>,
    pub masked_markdown: Option<String>,
}

fn sanitize_output_file_stem(file_stem: &str) -> String {
    let mut sanitized: String = file_stem
        .chars()
        .map(|ch| match ch {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' => '_',
            ch if ch.is_control() => '_',
            ch => ch,
        })
        .collect();

    sanitized = sanitized.trim_matches([' ', '.']).to_string();

    if sanitized.is_empty() {
        return "masked_file".to_string();
    }

    let reserved_names = [
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
        "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ];
    let base_name = sanitized
        .split('.')
        .next()
        .unwrap_or("")
        .to_ascii_uppercase();

    if reserved_names.contains(&base_name.as_str()) {
        sanitized = format!("_{}", sanitized);
    }

    sanitized
}

fn format_output_create_error(path: &str, error: impl std::fmt::Display) -> String {
    format!("创建输出文件失败：{}。请检查输出目录是否存在、文件名是否合法，以及文件是否正在被其他程序占用。原始错误：{}", path, error)
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PreviewResult {
    pub original_rows: Vec<Vec<String>>,
    pub masked_rows: Vec<Vec<String>>,
    pub headers: Vec<String>,
    pub original_file_stem: String,
    pub original_file_name: String,
    pub masked_file_stem: String,
    pub masked_file_name: String,
    pub detected_entities: Option<Vec<ner::RowEntities>>,
    pub mapping: Option<Vec<masking_engine::MappingEntry>>,
    pub masked_entity_count: Option<usize>,
    pub masked_markdown: Option<String>,
}

fn deterministic_findings(detector: &ner::NERDetector, content: &str) -> Vec<DeterministicFinding> {
    detector
        .detect_entities(content)
        .into_iter()
        .map(|entity| DeterministicFinding {
            text: entity.text,
            entity_type: entity.entity_type,
            start: entity.start,
            end: entity.end,
        })
        .collect()
}

fn sorted_mappings(
    mapping: &std::collections::HashMap<String, masking_engine::MappingEntry>,
) -> Vec<masking_engine::MappingEntry> {
    let mut entries: Vec<_> = mapping.values().cloned().collect();
    entries.sort_by(|left, right| left.masked.cmp(&right.masked));
    entries
}

fn mask_text_through_shared_core(
    content: &str,
    rules: &[masking_engine::MaskingRule],
    detector: &ner::NERDetector,
    initial_mappings: Vec<masking_engine::MappingEntry>,
    placeholder_counter: usize,
) -> engine_core::MaskingResult {
    let findings = deterministic_findings(detector, content);
    let mut session =
        MaskingSession::with_state(rules.to_vec(), initial_mappings, placeholder_counter);
    let markdown = session.mask_document(content, &findings);
    session.finish(markdown)
}

fn mask_filename_stem_through_shared_core(
    original_file_stem: &str,
    rules: &[masking_engine::MaskingRule],
    detector: &ner::NERDetector,
) -> engine_core::FilenameMaskingResult {
    let mut findings = engine_core::collect_filename_findings(original_file_stem, rules);
    findings.extend(deterministic_findings(detector, original_file_stem));
    engine_core::mask_filename(original_file_stem, rules, &findings)
}

fn output_extension_for_format(_format: &file_parser::FileFormat) -> &'static str {
    "md"
}

fn escape_markdown_table_cell(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('|', "\\|")
        .replace('\r', "")
        .replace('\n', "<br>")
}

fn rows_to_markdown(headers: Option<&[String]>, rows: &[Vec<String>]) -> String {
    let header_len = headers.map(|h| h.len()).unwrap_or(0);
    let max_cols = rows
        .iter()
        .map(|row| row.len())
        .max()
        .unwrap_or(0)
        .max(header_len);

    if max_cols <= 1 {
        let mut content = rows
            .iter()
            .map(|row| row.first().cloned().unwrap_or_default())
            .collect::<Vec<_>>()
            .join("\n");
        if !content.ends_with('\n') {
            content.push('\n');
        }
        return content;
    }

    let mut table_headers = headers.map(|h| h.to_vec()).unwrap_or_default();
    if table_headers.is_empty() {
        table_headers = (1..=max_cols).map(|idx| format!("列{}", idx)).collect();
    }
    table_headers.resize(max_cols, String::new());

    let mut content = String::new();
    content.push('|');
    for header in &table_headers {
        content.push(' ');
        content.push_str(&escape_markdown_table_cell(header));
        content.push_str(" |");
    }
    content.push('\n');

    content.push('|');
    for _ in 0..max_cols {
        content.push_str(" --- |");
    }
    content.push('\n');

    for row in rows {
        content.push('|');
        for col_idx in 0..max_cols {
            content.push(' ');
            content.push_str(&escape_markdown_table_cell(
                row.get(col_idx).map(String::as_str).unwrap_or(""),
            ));
            content.push_str(" |");
        }
        content.push('\n');
    }

    content
}

fn preview_content_for_save(
    format: &file_parser::FileFormat,
    masked_markdown: Option<&str>,
    headers: Option<&[String]>,
    masked_rows: &[Vec<String>],
) -> Result<String, String> {
    if matches!(
        format,
        file_parser::FileFormat::Text
            | file_parser::FileFormat::Markdown
            | file_parser::FileFormat::Word
            | file_parser::FileFormat::Pdf
    ) {
        return masked_markdown
            .map(str::to_owned)
            .ok_or_else(|| "预览缺少共享核心完整 Markdown，拒绝保存".to_string());
    }

    Ok(rows_to_markdown(headers, masked_rows))
}

#[cfg(test)]
mod preview_save_content_tests {
    use super::*;
    use std::fs;
    use uuid::Uuid;

    #[test]
    fn text_and_markdown_preserve_canonical_line_endings_exactly() {
        let cases = [
            (
                file_parser::FileFormat::Text,
                "alpha",
                "no trailing newline",
            ),
            (
                file_parser::FileFormat::Text,
                "alpha\n",
                "one trailing newline",
            ),
            (
                file_parser::FileFormat::Text,
                "alpha\n\n",
                "two trailing newlines",
            ),
            (
                file_parser::FileFormat::Markdown,
                "# alpha\n\n\n",
                "multiple trailing newlines",
            ),
            (
                file_parser::FileFormat::Markdown,
                "# alpha\r\nline\r\n",
                "CRLF content",
            ),
        ];
        let rows = vec![vec!["reconstructed".to_string()]];

        for (format, canonical, label) in cases {
            let actual = preview_content_for_save(&format, Some(canonical), None, &rows).unwrap();
            assert_eq!(actual.as_bytes(), canonical.as_bytes(), "{label}");
        }
    }

    #[test]
    fn shared_core_formats_require_canonical_markdown() {
        let rows = vec![vec!["must not be used".to_string()]];

        for format in [
            file_parser::FileFormat::Text,
            file_parser::FileFormat::Markdown,
            file_parser::FileFormat::Word,
            file_parser::FileFormat::Pdf,
        ] {
            let error = preview_content_for_save(&format, None, None, &rows).unwrap_err();
            assert_eq!(error, "预览缺少共享核心完整 Markdown，拒绝保存");
        }
    }

    #[test]
    fn non_text_formats_keep_rows_to_markdown_behavior() {
        let headers = vec!["列".to_string()];
        let rows = vec![vec!["alpha".to_string()], vec!["beta".to_string()]];

        let actual = preview_content_for_save(
            &file_parser::FileFormat::Csv,
            Some("must not be used"),
            Some(&headers),
            &rows,
        )
        .unwrap();

        assert_eq!(actual, rows_to_markdown(Some(&headers), &rows));
        assert_eq!(actual, "alpha\nbeta\n");
    }

    #[tokio::test]
    async fn save_preview_result_persists_manual_replacements_to_output_and_mapping() {
        let temp_dir = std::env::temp_dir().join(format!("vault-manual-replace-{}", Uuid::new_v4()));
        fs::create_dir_all(&temp_dir).unwrap();

        let source_path = temp_dir.join("manual-replace-source.md");
        fs::write(
            &source_path,
            "# 手工替换回归测试\n联系人：金金\n手机：13800138000\n",
        )
        .unwrap();

        let output_dir = temp_dir.join("output");
        fs::create_dir_all(&output_dir).unwrap();

        let result = save_preview_result(SavePreviewOptions {
            file_path: source_path.to_string_lossy().to_string(),
            output_dir: output_dir.to_string_lossy().to_string(),
            masked_rows: vec![vec!["联系人：甲方".to_string()], vec!["手机：联系电话".to_string()]],
            headers: None,
            passphrase: None,
            mapping: Some(vec![
                masking_engine::MappingEntry {
                    original: "金金".to_string(),
                    masked: "甲方".to_string(),
                    rule_id: "chinese_name".to_string(),
                },
                masking_engine::MappingEntry {
                    original: "13800138000".to_string(),
                    masked: "联系电话".to_string(),
                    rule_id: "phone".to_string(),
                },
            ]),
            masked_file_stem: Some("manual_replace_regression_result".to_string()),
            rule_ids: Some(vec!["chinese_name".to_string(), "phone".to_string()]),
            custom_rules: None,
            page_range: None,
            masked_entity_count: Some(2),
            masked_markdown: Some("# 手工替换回归测试\n联系人：甲方\n手机：联系电话\n".to_string()),
        })
        .await
        .unwrap();

        let saved_content = fs::read_to_string(&result.output_path).unwrap();
        assert_eq!(saved_content, "# 手工替换回归测试\n联系人：甲方\n手机：联系电话\n");

        let mapping_path = result.mapping_path.expect("mapping path should exist");
        let mappings = crypto::load_encrypted_mapping(&mapping_path, "").unwrap();
        assert_eq!(mappings.len(), 2);
        assert_eq!(mappings[0].masked, "甲方");
        assert_eq!(mappings[1].masked, "联系电话");

        let _ = fs::remove_dir_all(&temp_dir);
    }
}

fn build_masked_file_stem(
    file_path: &str,
    active_rules: &[masking_engine::MaskingRule],
    ner_detector: &ner::NERDetector,
    mapping: &mut std::collections::HashMap<String, masking_engine::MappingEntry>,
    counter: &mut usize,
    page_range: Option<(usize, usize)>,
) -> String {
    let original_file_name = std::path::Path::new(file_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("文件");

    let masked_result =
        mask_filename_stem_through_shared_core(original_file_name, active_rules, ner_detector);

    for entry in masked_result.mappings {
        mapping.entry(entry.original.clone()).or_insert(entry);
    }
    *counter += masked_result.masked_entity_count;

    let with_suffix = if let Some((start, end)) = page_range {
        format!("{}_脱敏_p{}-{}", masked_result.masked, start, end)
    } else {
        format!("{}_脱敏", masked_result.masked)
    };

    sanitize_output_file_stem(&with_suffix)
}

#[tauri::command]
pub async fn mask_file(options: MaskFileOptions) -> Result<MaskResult, String> {
    // 开始计时
    let start_time = std::time::Instant::now();

    let format = file_parser::detect_format(&options.file_path);

    let mut mapping = std::collections::HashMap::new();
    let mut counter = 0usize;
    let mut shared_masked_entity_count = None;

    // 1. 检查是否启用敏感词库；转换为规则统一调用共享构造函数（B1）
    let use_sensitive_terms = options
        .rule_ids
        .contains(&"use_sensitive_terms".to_string());
    let sensitive_term_rules: Vec<masking_engine::MaskingRule> = if use_sensitive_terms {
        let sensitive_terms = load_sensitive_terms().await?;
        engine_core::sensitive_term_rules(&sensitive_term_definitions(&sensitive_terms))
    } else {
        Vec::new()
    };

    // 2. 合并 builtin + custom + sensitive_term 规则
    let builtin = masking_engine::get_builtin_rules();
    let mut custom_masking_rules: Vec<masking_engine::MaskingRule> = options
        .custom_rules
        .as_deref()
        .unwrap_or(&[])
        .iter()
        .map(|cr| masking_engine::MaskingRule {
            id: cr.id.clone(),
            name: cr.name.clone(),
            pattern: cr.pattern.clone(),
            replacement_template: cr.replacement_template.clone(),
            use_counter: cr.use_counter.unwrap_or(true),
            enabled: true,
            builtin: false,
        })
        .collect();

    let mut all_combined: Vec<masking_engine::MaskingRule> = builtin.to_vec();
    all_combined.append(&mut custom_masking_rules);
    all_combined.extend(sensitive_term_rules); // 添加敏感词规则

    let active_rules: Vec<_> = all_combined
        .iter()
        .filter(|r| {
            // 内置规则和自定义规则需要在 rule_ids 中
            // 敏感词规则只要启用就自动生效（前提是 use_sensitive_terms 为 true）
            if r.id.starts_with("sensitive_term_") {
                r.enabled
            } else {
                options.rule_ids.contains(&r.id)
            }
        })
        .map(|r| {
            let mut rule = r.clone();
            rule.enabled = true;
            rule
        })
        .collect();

    for rule in &active_rules {}

    // 创建 NER 检测器（根据选项决定是否启用 AI 检测）
    let use_ai = options.use_ai_validation.unwrap_or(false);
    let ner_detector = if use_ai {
        ner::NERDetector::new_with_ai_detection(true)
    } else {
        ner::NERDetector::new()
    };

    // 3. 对文件名进行脱敏
    let original_file_name = std::path::Path::new(&options.file_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("file");

    // 对文件名应用脱敏规则
    let masked_file_name = masking_engine::mask_value_with_ner(
        original_file_name,
        &active_rules,
        &ner_detector,
        &mut mapping,
        &mut counter,
    );

    // 构建最终文件名：脱敏后的文件名 + "_脱敏" 后缀 + 页码标识（如果有）
    let final_file_name = if masked_file_name.is_empty()
        || masked_file_name.chars().all(|c| c == '*' || c.is_numeric())
    {
        // 文件名被完全脱敏（只剩占位符），使用占位符加后缀
        if let Some((start, end)) = options.page_range {
            format!("{}_脱敏_p{}-{}", masked_file_name, start, end)
        } else {
            format!("{}_脱敏", masked_file_name)
        }
    } else {
        // 文件名部分脱敏，在脱敏后的文件名后添加后缀
        if let Some((start, end)) = options.page_range {
            format!("{}_脱敏_p{}-{}", masked_file_name, start, end)
        } else {
            format!("{}_脱敏", masked_file_name)
        }
    };
    let final_file_name = sanitize_output_file_stem(&final_file_name);

    // 构建新的输出路径（使用脱敏后的文件名）
    let output_dir = std::path::Path::new(&options.output_path)
        .parent()
        .and_then(|p| p.to_str())
        .unwrap_or(".");

    let final_output_path = format!(
        "{}/{}.{}",
        output_dir,
        final_file_name,
        output_extension_for_format(&format)
    );

    // 无论有无 passphrase 都生成 .cmap 路径
    let mapping_path = Some(format!("{}.cmap", final_output_path));

    match format {
        file_parser::FileFormat::Csv => {
            let (headers, rows) = file_parser::parse_csv(&options.file_path)
                .map_err(|e| format!("Failed to parse CSV: {}", e))?;

            // 批量处理：收集所有单元格
            let all_cells: Vec<String> = rows.iter().flat_map(|row| row.iter()).cloned().collect();

            // 批量检测实体
            let batch_entities = ner_detector.detect_entities_batch(&all_cells);

            // 应用脱敏
            let mut masked_rows = Vec::new();
            let mut cell_idx = 0;

            for (row_idx, row) in rows.iter().enumerate() {
                let mut masked_row = Vec::new();

                for cell in row {
                    let entities = &batch_entities[cell_idx];

                    // 应用实体脱敏
                    let masked = if entities.is_empty() {
                        // 没有检测到实体，使用正则表达式脱敏
                        masking_engine::mask_value(&cell, &active_rules, &mut mapping, &mut counter)
                    } else {
                        // 有检测到实体，应用实体脱敏
                        masking_engine::apply_entities_to_text(
                            &cell,
                            entities,
                            &mut mapping,
                            &mut counter,
                        )
                    };

                    masked_row.push(masked);
                    cell_idx += 1;
                }

                masked_rows.push(masked_row);
            }

            let masked_content = rows_to_markdown(Some(&headers), &masked_rows);
            file_parser::write_markdown(&final_output_path, &masked_content)
                .map_err(|e| format!("Failed to write Markdown: {}", e))?;

            if let Some(ref map_path) = mapping_path {
                let mappings: Vec<_> = mapping.values().cloned().collect();
                if let Some(passphrase) = &options.passphrase {
                    crypto::save_encrypted_mapping(map_path, &mappings, passphrase)
                        .map_err(|e| format!("Failed to save mapping: {}", e))?;
                } else {
                    crypto::save_plain_mapping(map_path, &mappings)
                        .map_err(|e| format!("Failed to save mapping: {}", e))?;
                }
            }
        }
        file_parser::FileFormat::Excel => {
            let (headers, rows) = file_parser::parse_excel(&options.file_path)
                .map_err(|e| format!("Failed to parse Excel: {}", e))?;

            let all_cells: Vec<String> = rows.iter().flat_map(|row| row.iter()).cloned().collect();
            let batch_entities = ner_detector.detect_entities_batch(&all_cells);

            let mut masked_rows = Vec::new();
            let mut cell_idx = 0;

            for row in rows.iter() {
                let mut masked_row = Vec::new();

                for cell in row {
                    let entities = &batch_entities[cell_idx];
                    let masked = if entities.is_empty() {
                        masking_engine::mask_value(cell, &active_rules, &mut mapping, &mut counter)
                    } else {
                        masking_engine::apply_entities_to_text(
                            cell,
                            entities,
                            &mut mapping,
                            &mut counter,
                        )
                    };

                    masked_row.push(masked);
                    cell_idx += 1;
                }

                masked_rows.push(masked_row);
            }

            let masked_content = rows_to_markdown(Some(&headers), &masked_rows);
            file_parser::write_markdown(&final_output_path, &masked_content)
                .map_err(|e| format!("Failed to write Markdown: {}", e))?;

            if let Some(ref map_path) = mapping_path {
                let mappings: Vec<_> = mapping.values().cloned().collect();
                if let Some(passphrase) = &options.passphrase {
                    crypto::save_encrypted_mapping(map_path, &mappings, passphrase)
                        .map_err(|e| format!("Failed to save mapping: {}", e))?;
                } else {
                    crypto::save_plain_mapping(map_path, &mappings)
                        .map_err(|e| format!("Failed to save mapping: {}", e))?;
                }
            }
        }
        file_parser::FileFormat::Word => {
            let content =
                file_parser::parse_word_with_range(&options.file_path, options.page_range)
                    .map_err(|e| format!("Failed to parse Word: {}", e))?;

            let core_result = mask_text_through_shared_core(
                &content,
                &active_rules,
                &ner_detector,
                sorted_mappings(&mapping),
                counter,
            );
            let masked_content = core_result.markdown;
            shared_masked_entity_count = Some(core_result.masked_entity_count);
            mapping.clear();
            for (index, entry) in core_result.mappings.into_iter().enumerate() {
                mapping.insert(format!("{}-{}", entry.rule_id, index + 1), entry);
            }

            file_parser::write_markdown(&final_output_path, &masked_content)
                .map_err(|e| format!("Failed to write Word: {}", e))?;

            if let Some(ref map_path) = mapping_path {
                let mappings: Vec<_> = mapping.values().cloned().collect();
                if let Some(passphrase) = &options.passphrase {
                    crypto::save_encrypted_mapping(map_path, &mappings, passphrase)
                        .map_err(|e| format!("Failed to save mapping: {}", e))?;
                } else {
                    crypto::save_plain_mapping(map_path, &mappings)
                        .map_err(|e| format!("Failed to save mapping: {}", e))?;
                }
            }
        }
        file_parser::FileFormat::PowerPoint => {
            let content =
                file_parser::parse_powerpoint_with_range(&options.file_path, options.page_range)
                    .map_err(|e| format!("Failed to parse PowerPoint: {}", e))?;

            let masked_content = masking_engine::mask_value_with_ner(
                &content,
                &active_rules,
                &ner_detector,
                &mut mapping,
                &mut counter,
            );

            file_parser::write_markdown(&final_output_path, &masked_content)
                .map_err(|e| format!("Failed to write PowerPoint: {}", e))?;

            if let Some(ref map_path) = mapping_path {
                let mappings: Vec<_> = mapping.values().cloned().collect();
                if let Some(passphrase) = &options.passphrase {
                    crypto::save_encrypted_mapping(map_path, &mappings, passphrase)
                        .map_err(|e| format!("Failed to save mapping: {}", e))?;
                } else {
                    crypto::save_plain_mapping(map_path, &mappings)
                        .map_err(|e| format!("Failed to save mapping: {}", e))?;
                }
            }
        }
        file_parser::FileFormat::Pdf => {
            // parse_pdf_with_range 已经内置了 OCR 回退逻辑
            let content = file_parser::parse_pdf_with_range(&options.file_path, options.page_range)
                .map_err(|e| format!("Failed to parse PDF: {}", e))?;

            let core_result = mask_text_through_shared_core(
                &content,
                &active_rules,
                &ner_detector,
                sorted_mappings(&mapping),
                counter,
            );
            let masked_content = core_result.markdown;
            shared_masked_entity_count = Some(core_result.masked_entity_count);
            mapping.clear();
            for (index, entry) in core_result.mappings.into_iter().enumerate() {
                mapping.insert(format!("{}-{}", entry.rule_id, index + 1), entry);
            }

            file_parser::write_markdown(&final_output_path, &masked_content)
                .map_err(|e| format!("Failed to write PDF: {}", e))?;

            if let Some(ref map_path) = mapping_path {
                let mappings: Vec<_> = mapping.values().cloned().collect();
                if let Some(passphrase) = &options.passphrase {
                    crypto::save_encrypted_mapping(map_path, &mappings, passphrase)
                        .map_err(|e| format!("Failed to save mapping: {}", e))?;
                } else {
                    crypto::save_plain_mapping(map_path, &mappings)
                        .map_err(|e| format!("Failed to save mapping: {}", e))?;
                }
            }
        }
        file_parser::FileFormat::Markdown | file_parser::FileFormat::Text => {
            let content = file_parser::parse_markdown(&options.file_path)
                .map_err(|e| format!("Failed to read file: {}", e))?;

            let core_result = mask_text_through_shared_core(
                &content,
                &active_rules,
                &ner_detector,
                sorted_mappings(&mapping),
                counter,
            );
            let masked_content = core_result.markdown;
            shared_masked_entity_count = Some(core_result.masked_entity_count);
            mapping.clear();
            for (index, entry) in core_result.mappings.into_iter().enumerate() {
                mapping.insert(format!("{}-{}", entry.rule_id, index + 1), entry);
            }

            file_parser::write_markdown(&final_output_path, &masked_content)
                .map_err(|e| format!("Failed to write file: {}", e))?;

            if let Some(ref map_path) = mapping_path {
                let mappings: Vec<_> = mapping.values().cloned().collect();
                if let Some(passphrase) = &options.passphrase {
                    crypto::save_encrypted_mapping(map_path, &mappings, passphrase)
                        .map_err(|e| format!("Failed to save mapping: {}", e))?;
                } else {
                    crypto::save_plain_mapping(map_path, &mappings)
                        .map_err(|e| format!("Failed to save mapping: {}", e))?;
                }
            }
        }
        _ => {
            return Err("Unsupported file format".to_string());
        }
    }

    // 记录处理历史到数据库
    let processing_time_ms = start_time.elapsed().as_millis() as i64;
    let history = database::ProcessingHistory {
        id: Uuid::new_v4().to_string(),
        file_path: options.file_path.clone(),
        output_path: final_output_path.clone(),
        rule_ids: "[]".to_string(), // 空的规则ID数组
        file_size: std::fs::metadata(&options.file_path)
            .map(|m| m.len() as i64)
            .unwrap_or(0),
        masked_count: shared_masked_entity_count.unwrap_or(counter) as i32,
        processing_time_ms,
        status: "success".to_string(),
        error_message: None,
        created_at: chrono::Utc::now(),
    };

    // 异步记录到数据库（不阻塞主流程）
    if let Ok(db) = database::Database::new().await {
        let _ = db.add_processing_history(&history).await;
    }

    Ok(MaskResult {
        output_path: final_output_path,
        masked_count: shared_masked_entity_count.unwrap_or(counter),
        mapping_path,
    })
}

#[tauri::command]
pub async fn preview_masking(options: PreviewOptions) -> Result<PreviewResult, String> {
    let format = file_parser::detect_format(&options.file_path);
    let max_rows = options.max_rows.unwrap_or(10);

    // 1. 检查是否启用敏感词库；转换为规则统一调用共享构造函数（B1）
    let use_sensitive_terms = options
        .rule_ids
        .contains(&"use_sensitive_terms".to_string());

    let sensitive_term_rules: Vec<masking_engine::MaskingRule> = if use_sensitive_terms {
        let sensitive_terms = load_sensitive_terms().await?;
        engine_core::sensitive_term_rules(&sensitive_term_definitions(&sensitive_terms))
    } else {
        Vec::new()
    };

    // 2. 合并 builtin + custom + sensitive_term 规则
    let builtin = masking_engine::get_builtin_rules();
    let mut custom_masking_rules: Vec<masking_engine::MaskingRule> = options
        .custom_rules
        .as_deref()
        .unwrap_or(&[])
        .iter()
        .map(|cr| masking_engine::MaskingRule {
            id: cr.id.clone(),
            name: cr.name.clone(),
            pattern: cr.pattern.clone(),
            replacement_template: cr.replacement_template.clone(),
            use_counter: cr.use_counter.unwrap_or(true),
            enabled: true,
            builtin: false,
        })
        .collect();

    let mut all_combined: Vec<masking_engine::MaskingRule> = builtin.to_vec();
    all_combined.append(&mut custom_masking_rules);
    all_combined.extend(sensitive_term_rules);

    let active_rules: Vec<_> = all_combined
        .iter()
        .filter(|r| {
            // 敏感词规则只要启用就自动生效（前提是 use_sensitive_terms 为 true）
            if r.id.starts_with("sensitive_term_") {
                r.enabled
            } else {
                options.rule_ids.contains(&r.id)
            }
        })
        .map(|r| {
            let mut rule = r.clone();
            rule.enabled = true;
            rule
        })
        .collect();

    // Create NER detector (with AI detection if enabled)
    let use_ai = options.use_ai_validation.unwrap_or(false);
    let ner_detector = if use_ai {
        ner::NERDetector::new_with_ai_detection(true)
    } else {
        ner::NERDetector::new()
    };
    let mut filename_mapping = std::collections::HashMap::new();
    let mut filename_counter = 0usize;
    let masked_file_stem = build_masked_file_stem(
        &options.file_path,
        &active_rules,
        &ner_detector,
        &mut filename_mapping,
        &mut filename_counter,
        options.page_range,
    );
    let masked_file_name = format!(
        "{}.{}",
        masked_file_stem,
        output_extension_for_format(&format)
    );
    let original_file_stem = std::path::Path::new(&options.file_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("文件")
        .to_string();
    let original_file_name = std::path::Path::new(&options.file_path)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("文件")
        .to_string();

    match format {
        file_parser::FileFormat::Csv => {
            let (headers, rows) = file_parser::parse_csv(&options.file_path)
                .map_err(|e| format!("Failed to parse CSV: {}", e))?;

            let preview_rows: Vec<_> = rows.into_iter().take(max_rows).collect();
            let mut mapping = filename_mapping.clone();
            let mut counter = filename_counter;

            let masked_rows: Vec<Vec<String>> = preview_rows
                .iter()
                .map(|row| {
                    row.iter()
                        .map(|cell| {
                            masking_engine::mask_value_with_ner(
                                cell,
                                &active_rules,
                                &ner_detector,
                                &mut mapping,
                                &mut counter,
                            )
                        })
                        .collect()
                })
                .collect();

            // Detect entities in original rows
            let detected_entities = Some(ner_detector.detect_in_rows(&preview_rows));

            // 转换映射为 MappingEntry 向量
            let mapping_entries: Vec<masking_engine::MappingEntry> =
                mapping.values().cloned().collect();

            Ok(PreviewResult {
                original_rows: preview_rows,
                masked_rows,
                headers,
                original_file_stem: original_file_stem.clone(),
                original_file_name: original_file_name.clone(),
                masked_file_stem: masked_file_stem.clone(),
                masked_file_name: masked_file_name.clone(),
                detected_entities,
                mapping: Some(mapping_entries),
                masked_entity_count: None,
                masked_markdown: None,
            })
        }
        file_parser::FileFormat::Excel => {
            let (headers, rows) = file_parser::parse_excel(&options.file_path)
                .map_err(|e| format!("Failed to parse Excel: {}", e))?;

            let preview_rows: Vec<_> = rows.into_iter().take(max_rows).collect();
            let mut mapping = filename_mapping.clone();
            let mut counter = filename_counter;

            let masked_rows: Vec<Vec<String>> = preview_rows
                .iter()
                .map(|row| {
                    row.iter()
                        .map(|cell| {
                            masking_engine::mask_value_with_ner(
                                cell,
                                &active_rules,
                                &ner_detector,
                                &mut mapping,
                                &mut counter,
                            )
                        })
                        .collect()
                })
                .collect();

            // Detect entities in original rows
            let detected_entities = Some(ner_detector.detect_in_rows(&preview_rows));

            // 转换映射为 MappingEntry 向量
            let mapping_entries: Vec<masking_engine::MappingEntry> =
                mapping.values().cloned().collect();

            Ok(PreviewResult {
                original_rows: preview_rows,
                masked_rows,
                headers,
                original_file_stem: original_file_stem.clone(),
                original_file_name: original_file_name.clone(),
                masked_file_stem: masked_file_stem.clone(),
                masked_file_name: masked_file_name.clone(),
                detected_entities,
                mapping: Some(mapping_entries),
                masked_entity_count: None,
                masked_markdown: None,
            })
        }
        file_parser::FileFormat::Word => {
            // 读取 Word 文档内容
            let content =
                file_parser::parse_word_with_range(&options.file_path, options.page_range)
                    .map_err(|e| format!("Failed to parse Word: {}", e))?;

            // 按行分割
            let lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();

            let core_result = mask_text_through_shared_core(
                &content,
                &active_rules,
                &ner_detector,
                sorted_mappings(&filename_mapping),
                filename_counter,
            );
            let masked_markdown = core_result.markdown;
            let masked_lines: Vec<String> = masked_markdown.lines().map(str::to_string).collect();

            // 将文本行转换为表格格式（单列）
            let original_rows: Vec<Vec<String>> =
                lines.iter().map(|line| vec![line.clone()]).collect();

            let masked_rows: Vec<Vec<String>> =
                masked_lines.iter().map(|line| vec![line.clone()]).collect();

            // Detect entities in original rows
            let detected_entities = Some(ner_detector.detect_in_rows(&original_rows));

            // 转换映射为 MappingEntry 向量
            let mapping_entries = core_result.mappings;

            Ok(PreviewResult {
                original_rows,
                masked_rows,
                headers: vec!["内容".to_string()],
                original_file_stem: original_file_stem.clone(),
                original_file_name: original_file_name.clone(),
                masked_file_stem: masked_file_stem.clone(),
                masked_file_name: masked_file_name.clone(),
                detected_entities,
                mapping: Some(mapping_entries),
                masked_entity_count: Some(core_result.masked_entity_count),
                masked_markdown: Some(masked_markdown),
            })
        }
        file_parser::FileFormat::PowerPoint => {
            // 读取 PowerPoint 内容
            let content =
                file_parser::parse_powerpoint_with_range(&options.file_path, options.page_range)
                    .map_err(|e| format!("Failed to parse PowerPoint: {}", e))?;

            // 按行分割
            let lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();

            let mut mapping = filename_mapping.clone();
            let mut counter = filename_counter;

            // 对每行进行脱敏
            let masked_lines: Vec<String> = lines
                .iter()
                .map(|line| {
                    masking_engine::mask_value_with_ner(
                        line,
                        &active_rules,
                        &ner_detector,
                        &mut mapping,
                        &mut counter,
                    )
                })
                .collect();

            // 将文本行转换为表格格式（单列）
            let original_rows: Vec<Vec<String>> =
                lines.iter().map(|line| vec![line.clone()]).collect();

            let masked_rows: Vec<Vec<String>> =
                masked_lines.iter().map(|line| vec![line.clone()]).collect();

            // Detect entities in original rows
            let detected_entities = Some(ner_detector.detect_in_rows(&original_rows));

            // 转换映射为 MappingEntry 向量
            let mapping_entries: Vec<masking_engine::MappingEntry> =
                mapping.values().cloned().collect();

            Ok(PreviewResult {
                original_rows,
                masked_rows,
                headers: vec!["内容".to_string()],
                original_file_stem: original_file_stem.clone(),
                original_file_name: original_file_name.clone(),
                masked_file_stem: masked_file_stem.clone(),
                masked_file_name: masked_file_name.clone(),
                detected_entities,
                mapping: Some(mapping_entries),
                masked_entity_count: None,
                masked_markdown: None,
            })
        }
        file_parser::FileFormat::Pdf => {
            // parse_pdf_with_range 已经内置了 OCR 回退逻辑
            let content = file_parser::parse_pdf_with_range(&options.file_path, options.page_range)
                .map_err(|e| format!("Failed to parse PDF: {}", e))?;

            // 按行分割
            let lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();

            let core_result = mask_text_through_shared_core(
                &content,
                &active_rules,
                &ner_detector,
                sorted_mappings(&filename_mapping),
                filename_counter,
            );
            let masked_markdown = core_result.markdown;
            let masked_lines: Vec<String> = masked_markdown.lines().map(str::to_string).collect();

            // 将文本行转换为表格格式（单列）
            let original_rows: Vec<Vec<String>> =
                lines.iter().map(|line| vec![line.clone()]).collect();

            let masked_rows: Vec<Vec<String>> =
                masked_lines.iter().map(|line| vec![line.clone()]).collect();

            // Detect entities in original rows
            let detected_entities = Some(ner_detector.detect_in_rows(&original_rows));

            // 转换映射为 MappingEntry 向量
            let mapping_entries = core_result.mappings;

            Ok(PreviewResult {
                original_rows,
                masked_rows,
                headers: vec!["内容".to_string()],
                original_file_stem: original_file_stem.clone(),
                original_file_name: original_file_name.clone(),
                masked_file_stem: masked_file_stem.clone(),
                masked_file_name: masked_file_name.clone(),
                detected_entities,
                mapping: Some(mapping_entries),
                masked_entity_count: Some(core_result.masked_entity_count),
                masked_markdown: Some(masked_markdown),
            })
        }
        file_parser::FileFormat::Markdown | file_parser::FileFormat::Text => {
            // 读取文本内容
            let content = std::fs::read_to_string(&options.file_path)
                .map_err(|e| format!("Failed to read file: {}", e))?;

            // 按行分割（读取全部内容，不限制行数）
            let lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();

            let core_result = mask_text_through_shared_core(
                &content,
                &active_rules,
                &ner_detector,
                sorted_mappings(&filename_mapping),
                filename_counter,
            );
            let masked_markdown = core_result.markdown;
            let masked_lines: Vec<String> = masked_markdown.lines().map(str::to_string).collect();

            // 将文本行转换为表格格式（单列）
            let original_rows: Vec<Vec<String>> =
                lines.iter().map(|line| vec![line.clone()]).collect();

            let masked_rows: Vec<Vec<String>> =
                masked_lines.iter().map(|line| vec![line.clone()]).collect();

            // Detect entities in original rows
            let detected_entities = Some(ner_detector.detect_in_rows(&original_rows));

            // 转换映射为 MappingEntry 向量
            let mapping_entries = core_result.mappings;

            Ok(PreviewResult {
                original_rows,
                masked_rows,
                headers: vec!["内容".to_string()],
                original_file_stem: original_file_stem.clone(),
                original_file_name: original_file_name.clone(),
                masked_file_stem: masked_file_stem.clone(),
                masked_file_name: masked_file_name.clone(),
                detected_entities,
                mapping: Some(mapping_entries),
                masked_entity_count: Some(core_result.masked_entity_count),
                masked_markdown: Some(masked_markdown),
            })
        }
        _ => Err(
            "预览功能目前支持 CSV、Excel、Word、PowerPoint、PDF、Markdown 和 TXT 文件".to_string(),
        ),
    }
}

#[cfg(test)]
mod shared_core_adapter_tests {
    use super::*;
    use engine_core::{InputFormat, MaskingRequest, MaskingService};

    #[test]
    fn text_adapter_matches_direct_shared_core() {
        let rules: Vec<_> = masking_engine::get_builtin_rules()
            .iter()
            .filter(|rule| ["phone", "email"].contains(&rule.id.as_str()))
            .cloned()
            .map(|mut rule| {
                rule.enabled = true;
                rule
            })
            .collect();
        let content = "13900000000 unit.test@example.invalid 13900000000";
        let direct = MaskingService::mask(MaskingRequest {
            input_format: InputFormat::Text,
            content: content.into(),
            rules: rules.clone(),
            deterministic_findings: vec![],
        })
        .unwrap();
        let adapter =
            mask_text_through_shared_core(content, &rules, &ner::NERDetector::new(), vec![], 0);

        assert_eq!(adapter.markdown, direct.markdown);
        assert_eq!(adapter.mappings, direct.mappings);
        assert_eq!(adapter.masked_entity_count, direct.masked_entity_count);
    }

    #[test]
    fn text_adapter_masks_chinese_name_via_ner_when_rule_enabled() {
        let rules: Vec<_> = masking_engine::get_builtin_rules()
            .iter()
            .filter(|rule| ["chinese_name", "phone", "email"].contains(&rule.id.as_str()))
            .cloned()
            .map(|mut rule| {
                rule.enabled = true;
                rule
            })
            .collect();
        let content = "客户姓名：张三\n联系电话：13900000000\n电子邮箱：zhangsan@example.com\n";

        let adapter = mask_text_through_shared_core(
            content,
            &rules,
            &ner::NERDetector::new(),
            vec![],
            0,
        );

        assert!(adapter.markdown.contains("客户姓名：姓名1"));
        assert!(adapter.markdown.contains("联系电话：***PHONE***2"));
        assert!(adapter.markdown.contains("电子邮箱：***EMAIL***3"));
        assert!(!adapter.markdown.contains("客户姓名：张三"));
        assert!(!adapter.markdown.contains("13900000000"));
        assert!(!adapter.markdown.contains("zhangsan@example.com"));
    }

    #[test]
    fn build_masked_file_stem_masks_sensitive_filename_for_desktop_flow() {
        let rules: Vec<_> = masking_engine::get_builtin_rules()
            .iter()
            .filter(|rule| ["chinese_name", "phone"].contains(&rule.id.as_str()))
            .cloned()
            .map(|mut rule| {
                rule.enabled = true;
                rule
            })
            .collect();
        let mut mapping = std::collections::HashMap::new();
        let mut counter = 0usize;

        let stem = build_masked_file_stem(
            "/tmp/张三-13900000000-测试文档.md",
            &rules,
            &ner::NERDetector::new(),
            &mut mapping,
            &mut counter,
            None,
        );

        assert!(stem.ends_with("_脱敏"));
        assert!(!stem.contains("张三"));
        assert!(!stem.contains("13900000000"));
        assert!(stem.contains("张*"));
        assert!(stem.contains("139****0000"));
    }
}

#[tauri::command]
pub async fn save_preview_result(options: SavePreviewOptions) -> Result<MaskResult, String> {
    // 共享核心格式传入真实命中数；旧格式保持按变更行数记录的口径。
    let masked_count = options
        .masked_entity_count
        .unwrap_or(options.masked_rows.len());

    // 开始计时
    let start_time = std::time::Instant::now();

    let format = file_parser::detect_format(&options.file_path);
    let original_file_name = std::path::Path::new(&options.file_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("文件");

    // 优先使用预览阶段已经生成并清洗过的文件名，避免 macOS 保存前仍暴露原始文件名。
    let masked_file_name = if let Some(preview_file_stem) = options.masked_file_stem.as_deref() {
        preview_file_stem.to_string()
    } else if let Some(rule_ids) = &options.rule_ids {
        // 1. 检查是否启用敏感词库；转换为规则统一调用共享构造函数（B1）
        let use_sensitive_terms = rule_ids.contains(&"use_sensitive_terms".to_string());
        let sensitive_term_rules: Vec<masking_engine::MaskingRule> = if use_sensitive_terms {
            let sensitive_terms = load_sensitive_terms().await?;
            engine_core::sensitive_term_rules(&sensitive_term_definitions(&sensitive_terms))
        } else {
            Vec::new()
        };

        // 2. 合并 builtin + custom + sensitive_term 规则
        let builtin = masking_engine::get_builtin_rules();
        let mut custom_masking_rules: Vec<masking_engine::MaskingRule> = options
            .custom_rules
            .as_deref()
            .unwrap_or(&[])
            .iter()
            .map(|cr| masking_engine::MaskingRule {
                id: cr.id.clone(),
                name: cr.name.clone(),
                pattern: cr.pattern.clone(),
                replacement_template: cr.replacement_template.clone(),
                use_counter: cr.use_counter.unwrap_or(true),
                enabled: true,
                builtin: false,
            })
            .collect();

        let mut all_combined: Vec<masking_engine::MaskingRule> = builtin.to_vec();
        all_combined.append(&mut custom_masking_rules);
        all_combined.extend(sensitive_term_rules);

        let active_rules: Vec<_> = all_combined
            .iter()
            .filter(|r| {
                if r.id.starts_with("sensitive_term_") {
                    r.enabled
                } else {
                    rule_ids.contains(&r.id)
                }
            })
            .map(|r| {
                let mut rule = r.clone();
                rule.enabled = true;
                rule
            })
            .collect();

        // 创建 NER 检测器
        let ner_detector = ner::NERDetector::new();

        // 对文件名应用脱敏规则
        let mut temp_mapping = std::collections::HashMap::new();
        let mut temp_counter = 0usize;
        let masked = build_masked_file_stem(
            &options.file_path,
            &active_rules,
            &ner_detector,
            &mut temp_mapping,
            &mut temp_counter,
            options.page_range,
        );

        // 添加页码标识（如果有）
        if let Some((start, end)) = options.page_range {
            format!("{}_p{}-{}", masked, start, end)
        } else {
            masked
        }
    } else {
        // 没有提供 rule_ids，只添加后缀
        if let Some((start, end)) = options.page_range {
            format!("{}_脱敏_p{}-{}", original_file_name, start, end)
        } else {
            format!("{}_脱敏", original_file_name)
        }
    };
    let masked_file_name = sanitize_output_file_stem(&masked_file_name);

    let extension = output_extension_for_format(&format);

    let output_path = format!("{}/{}.{}", options.output_dir, masked_file_name, extension);

    if !matches!(
        format,
        file_parser::FileFormat::Csv
            | file_parser::FileFormat::Excel
            | file_parser::FileFormat::Word
            | file_parser::FileFormat::PowerPoint
            | file_parser::FileFormat::Pdf
            | file_parser::FileFormat::Markdown
            | file_parser::FileFormat::Text
    ) {
        return Err(format!(
            "Unsupported format for save_preview_result: {:?}",
            format
        ));
    }

    let content = preview_content_for_save(
        &format,
        options.masked_markdown.as_deref(),
        options.headers.as_deref(),
        &options.masked_rows,
    )?;
    std::fs::write(&output_path, content)
        .map_err(|e| format_output_create_error(&output_path, e))?;

    // 创建映射文件（使用传入的映射数据，如果没有则创建空映射）
    let mapping_path = format!("{}.cmap", output_path);
    let mapping_to_save = options.mapping.unwrap_or_default();

    if let Some(passphrase) = &options.passphrase {
        crypto::save_encrypted_mapping(&mapping_path, &mapping_to_save, passphrase)
            .map_err(|e| format!("Failed to save encrypted mapping: {}", e))?;
    } else {
        crypto::save_plain_mapping(&mapping_path, &mapping_to_save)
            .map_err(|e| format!("Failed to save plain mapping: {}", e))?;
    }

    // 记录处理历史到数据库
    let processing_time_ms = start_time.elapsed().as_millis() as i64;
    let history = database::ProcessingHistory {
        id: Uuid::new_v4().to_string(),
        file_path: options.file_path.clone(),
        output_path: output_path.clone(),
        rule_ids: "[]".to_string(),
        file_size: std::fs::metadata(&options.file_path)
            .map(|m| m.len() as i64)
            .unwrap_or(0),
        masked_count: masked_count as i32,
        processing_time_ms,
        status: "success".to_string(),
        error_message: None,
        created_at: chrono::Utc::now(),
    };

    // 异步记录到数据库（不阻塞主流程）
    if let Ok(db) = database::Database::new().await {
        let _ = db.add_processing_history(&history).await;
    }

    Ok(MaskResult {
        output_path,
        masked_count,
        mapping_path: Some(mapping_path),
    })
}
