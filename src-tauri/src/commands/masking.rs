use serde::{Deserialize, Serialize};
use crate::core::{masking_engine, file_parser, ner, crypto, database};

/// 从数据库加载启用的敏感词
async fn load_sensitive_terms() -> Result<Vec<database::SensitiveTerm>, String> {
    let db = database::Database::new().await
        .map_err(|e| format!("Failed to initialize database: {}", e))?;
    
    // 只加载启用的敏感词
    db.get_sensitive_terms(None, true).await
        .map_err(|e| format!("Failed to load sensitive terms: {}", e))
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
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PreviewResult {
    pub original_rows: Vec<Vec<String>>,
    pub masked_rows: Vec<Vec<String>>,
    pub headers: Vec<String>,
    pub detected_entities: Option<Vec<ner::RowEntities>>,
}

#[tauri::command]
pub async fn mask_file(options: MaskFileOptions) -> Result<MaskResult, String> {
    println!("=== MASK_FILE START ===");
    println!("File path: {}", options.file_path);
    println!("Output path: {}", options.output_path);
    println!("Rule IDs: {:?}", options.rule_ids);
    
    let format = file_parser::detect_format(&options.file_path);
    println!("Detected format: {:?}", format);
    
    let mut mapping = std::collections::HashMap::new();
    let mut counter = 0usize;
    // 无论有无 passphrase 都生成 .cmap 路径
    let mapping_path = Some(format!("{}.cmap", options.output_path));

    // 1. 检查是否启用敏感词库
    let use_sensitive_terms = options.rule_ids.contains(&"use_sensitive_terms".to_string());
    let sensitive_term_rules: Vec<masking_engine::MaskingRule> = if use_sensitive_terms {
        // 加载敏感词库
        let sensitive_terms = load_sensitive_terms().await?;
        println!("Loaded {} sensitive terms from database", sensitive_terms.len());
        
        // 将敏感词转换为脱敏规则
        sensitive_terms
            .iter()
            .map(|term| {
                // 使用精确匹配的正则表达式（转义特殊字符）
                let escaped_term = regex::escape(&term.term);
                // 对于中文，不使用词边界；对于英文，使用词边界
                let pattern = if term.term.chars().any(|c| c > '\u{4E00}' && c < '\u{9FA5}') {
                    // 包含中文字符，不使用词边界
                    escaped_term
                } else {
                    // 纯英文或数字，使用词边界
                    format!(r"\b{}\b", escaped_term)
                };
                
                masking_engine::MaskingRule {
                    id: format!("sensitive_term_{}", term.id),
                    name: format!("{} ({})", term.term, term.category),
                    pattern,
                    replacement_template: format!("[{}]", term.category),
                    use_counter: false, // 敏感词使用固定替换
                    enabled: term.enabled,
                    builtin: false,
                }
            })
            .collect()
    } else {
        println!("Sensitive terms disabled by user");
        Vec::new()
    };
    
    println!("Converted {} sensitive terms to masking rules", sensitive_term_rules.len());

    // 2. 合并 builtin + custom + sensitive_term 规则
    let builtin = masking_engine::get_builtin_rules();
    let mut custom_masking_rules: Vec<masking_engine::MaskingRule> = options.custom_rules
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
        .map(|r| { let mut rule = r.clone(); rule.enabled = true; rule })
        .collect();
    
    println!("=== Masking Debug Info ===");
    println!("Total available rules: {}", all_combined.len());
    println!("Requested rule IDs: {:?}", options.rule_ids);
    println!("Active rules count: {}", active_rules.len());
    for rule in &active_rules {
        println!("  - {} ({}): pattern={}", rule.name, rule.id, rule.pattern);
    }
    println!("========================");
    
    // 创建 NER 检测器（根据选项决定是否启用 AI 检测）
    let use_ai = options.use_ai_validation.unwrap_or(false);
    let ner_detector = if use_ai {
        println!("AI detection enabled for multi-method entity detection");
        ner::NERDetector::new_with_ai_detection(true)
    } else {
        ner::NERDetector::new()
    };

    match format {
        file_parser::FileFormat::Csv => {
            println!("Processing CSV file...");
            let (headers, rows) = file_parser::parse_csv(&options.file_path)
                .map_err(|e| {
                    println!("CSV parse error: {}", e);
                    format!("Failed to parse CSV: {}", e)
                })?;

            println!("CSV parsed successfully. Headers: {:?}, Rows: {}", headers, rows.len());

            // 批量处理：收集所有单元格
            let all_cells: Vec<String> = rows.iter()
                .flat_map(|row| row.iter())
                .cloned()
                .collect();
            
            println!("Collected {} cells for batch processing", all_cells.len());
            
            // 批量检测实体
            let batch_entities = ner_detector.detect_entities_batch(&all_cells);
            println!("Batch detection completed");
            
            // 应用脱敏
            let mut masked_rows = Vec::new();
            let mut cell_idx = 0;
            
            for (row_idx, row) in rows.iter().enumerate() {
                println!("Processing row {}/{}", row_idx + 1, rows.len());
                let mut masked_row = Vec::new();
                
                for cell in row {
                    let entities = &batch_entities[cell_idx];
                    
                    // 应用实体脱敏
                    let masked = if entities.is_empty() {
                        // 没有检测到实体，使用正则表达式脱敏
                        masking_engine::mask_value(&cell, &active_rules, &mut mapping, &mut counter)
                    } else {
                        // 有检测到实体，应用实体脱敏
                        masking_engine::apply_entities_to_text(&cell, entities, &mut mapping, &mut counter)
                    };
                    
                    masked_row.push(masked);
                    cell_idx += 1;
                }
                
                masked_rows.push(masked_row);
            }

            println!("Writing CSV output...");
            file_parser::write_csv(&options.output_path, &headers, &masked_rows)
                .map_err(|e| {
                    println!("CSV write error: {}", e);
                    format!("Failed to write CSV: {}", e)
                })?;

            if let Some(ref map_path) = mapping_path {
                let mappings: Vec<_> = mapping.values().cloned().collect();
                if let Some(passphrase) = &options.passphrase {
                    println!("Saving encrypted mapping to: {}", map_path);
                    crypto::save_encrypted_mapping(map_path, &mappings, passphrase)
                        .map_err(|e| { println!("Mapping save error: {}", e); format!("Failed to save mapping: {}", e) })?;
                } else {
                    println!("Saving plain JSON mapping to: {}", map_path);
                    crypto::save_plain_mapping(map_path, &mappings)
                        .map_err(|e| { println!("Mapping save error: {}", e); format!("Failed to save mapping: {}", e) })?;
                }
            }
        }
        file_parser::FileFormat::Word => {
            println!("Processing Word file...");
            let content = file_parser::parse_word(&options.file_path)
                .map_err(|e| {
                    println!("Word parse error: {}", e);
                    format!("Failed to parse Word: {}", e)
                })?;

            let masked_content = masking_engine::mask_value_with_ner(&content, &active_rules, &ner_detector, &mut mapping, &mut counter);

            // Output as .txt file instead of .docx for simplicity
            let txt_output = options.output_path.replace(".docx", ".txt").replace(".doc", ".txt");
            println!("Writing Word output to: {}", txt_output);
            file_parser::write_markdown(&txt_output, &masked_content)
                .map_err(|e| {
                    println!("Word write error: {}", e);
                    format!("Failed to write Word: {}", e)
                })?;

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
            println!("Processing PowerPoint file...");
            let content = file_parser::parse_powerpoint(&options.file_path)
                .map_err(|e| {
                    println!("PowerPoint parse error: {}", e);
                    format!("Failed to parse PowerPoint: {}", e)
                })?;

            let masked_content = masking_engine::mask_value_with_ner(&content, &active_rules, &ner_detector, &mut mapping, &mut counter);

            // Output as .txt file instead of .pptx for simplicity
            let txt_output = options.output_path.replace(".pptx", ".txt").replace(".ppt", ".txt");
            println!("Writing PowerPoint output to: {}", txt_output);
            file_parser::write_markdown(&txt_output, &masked_content)
                .map_err(|e| {
                    println!("PowerPoint write error: {}", e);
                    format!("Failed to write PowerPoint: {}", e)
                })?;

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
            println!("Processing PDF file with batch detection...");
            let content = file_parser::parse_pdf(&options.file_path)
                .map_err(|e| {
                    println!("PDF parse error: {}", e);
                    format!("Failed to parse PDF: {}", e)
                })?;

            // 按行分割
            let lines: Vec<String> = content.lines()
                .map(|s| s.to_string())
                .collect();
            
            println!("PDF parsed into {} lines", lines.len());
            
            // 批量检测所有行
            let batch_entities = ner_detector.detect_entities_batch(&lines);
            println!("Batch detection completed for {} lines", lines.len());
            
            // 对每行应用脱敏
            let masked_lines: Vec<String> = lines
                .iter()
                .zip(batch_entities.iter())
                .map(|(line, entities)| {
                    if entities.is_empty() {
                        // 没有检测到实体，使用正则表达式脱敏
                        masking_engine::mask_value(line, &active_rules, &mut mapping, &mut counter)
                    } else {
                        // 有检测到实体，应用实体脱敏
                        masking_engine::apply_entities_to_text(line, entities, &mut mapping, &mut counter)
                    }
                })
                .collect();
            
            let masked_content = masked_lines.join("\n");
            
            // Output as .txt file instead of .pdf for simplicity
            let txt_output = options.output_path.replace(".pdf", ".txt");
            println!("Writing PDF output to: {}", txt_output);
            file_parser::write_markdown(&txt_output, &masked_content)
                .map_err(|e| {
                    println!("PDF write error: {}", e);
                    format!("Failed to write PDF: {}", e)
                })?;

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
            println!("Processing Markdown/Text file...");
            let content = file_parser::parse_markdown(&options.file_path)
                .map_err(|e| {
                    println!("Markdown/Text parse error: {}", e);
                    format!("Failed to read file: {}", e)
                })?;

            let masked_content = masking_engine::mask_value_with_ner(&content, &active_rules, &ner_detector, &mut mapping, &mut counter);

            println!("Writing Markdown/Text output...");
            file_parser::write_markdown(&options.output_path, &masked_content)
                .map_err(|e| {
                    println!("Markdown/Text write error: {}", e);
                    format!("Failed to write file: {}", e)
                })?;

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
            println!("Unsupported file format: {:?}", format);
            return Err("Unsupported file format".to_string());
        }
    }

    println!("=== MASK_FILE SUCCESS ===");
    println!("Masked count: {}", counter);
    println!("Output path: {}", options.output_path);

    Ok(MaskResult {
        output_path: options.output_path,
        masked_count: counter,
        mapping_path,
    })
}

#[tauri::command]
pub async fn preview_masking(options: PreviewOptions) -> Result<PreviewResult, String> {
    let format = file_parser::detect_format(&options.file_path);
    let max_rows = options.max_rows.unwrap_or(10);
    
    // 1. 检查是否启用敏感词库
    let use_sensitive_terms = options.rule_ids.contains(&"use_sensitive_terms".to_string());
    let sensitive_term_rules: Vec<masking_engine::MaskingRule> = if use_sensitive_terms {
        // 加载敏感词库
        let sensitive_terms = load_sensitive_terms().await?;
        println!("Loaded {} sensitive terms for preview", sensitive_terms.len());
        
        // 将敏感词转换为脱敏规则
        sensitive_terms
            .iter()
            .map(|term| {
                let escaped_term = regex::escape(&term.term);
                // 对于中文，不使用词边界；对于英文，使用词边界
                let pattern = if term.term.chars().any(|c| c > '\u{4E00}' && c < '\u{9FA5}') {
                    // 包含中文字符，不使用词边界
                    escaped_term
                } else {
                    // 纯英文或数字，使用词边界
                    format!(r"\b{}\b", escaped_term)
                };
                
                masking_engine::MaskingRule {
                    id: format!("sensitive_term_{}", term.id),
                    name: format!("{} ({})", term.term, term.category),
                    pattern,
                    replacement_template: format!("[{}]", term.category),
                    use_counter: false,
                    enabled: term.enabled,
                    builtin: false,
                }
            })
            .collect()
    } else {
        println!("Sensitive terms disabled by user for preview");
        Vec::new()
    };
    
    // 2. 合并 builtin + custom + sensitive_term 规则
    let builtin = masking_engine::get_builtin_rules();
    let mut custom_masking_rules: Vec<masking_engine::MaskingRule> = options.custom_rules
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
        .map(|r| { let mut rule = r.clone(); rule.enabled = true; rule })
        .collect();
    
    // Create NER detector (with AI detection if enabled)
    let use_ai = options.use_ai_validation.unwrap_or(false);
    let ner_detector = if use_ai {
        println!("AI detection enabled for multi-method entity detection (preview)");
        ner::NERDetector::new_with_ai_detection(true)
    } else {
        ner::NERDetector::new()
    };

    match format {
        file_parser::FileFormat::Csv => {
            let (headers, rows) = file_parser::parse_csv(&options.file_path)
                .map_err(|e| format!("Failed to parse CSV: {}", e))?;

            let preview_rows: Vec<_> = rows.into_iter().take(max_rows).collect();
            let mut mapping = std::collections::HashMap::new();
            let mut counter = 0usize;

            let masked_rows: Vec<Vec<String>> = preview_rows
                .iter()
                .map(|row| {
                    row.iter()
                        .map(|cell| masking_engine::mask_value_with_ner(cell, &active_rules, &ner_detector, &mut mapping, &mut counter))
                        .collect()
                })
                .collect();

            // Detect entities in original rows
            let detected_entities = Some(ner_detector.detect_in_rows(&preview_rows));

            Ok(PreviewResult {
                original_rows: preview_rows,
                masked_rows,
                headers,
                detected_entities,
            })
        }
        file_parser::FileFormat::Excel => {
            let (headers, rows) = file_parser::parse_excel(&options.file_path)
                .map_err(|e| format!("Failed to parse Excel: {}", e))?;

            let preview_rows: Vec<_> = rows.into_iter().take(max_rows).collect();
            let mut mapping = std::collections::HashMap::new();
            let mut counter = 0usize;

            let masked_rows: Vec<Vec<String>> = preview_rows
                .iter()
                .map(|row| {
                    row.iter()
                        .map(|cell| masking_engine::mask_value_with_ner(cell, &active_rules, &ner_detector, &mut mapping, &mut counter))
                        .collect()
                })
                .collect();

            // Detect entities in original rows
            let detected_entities = Some(ner_detector.detect_in_rows(&preview_rows));

            Ok(PreviewResult {
                original_rows: preview_rows,
                masked_rows,
                headers,
                detected_entities,
            })
        }
        file_parser::FileFormat::Word => {
            // 读取 Word 文档内容
            let content = file_parser::parse_word(&options.file_path)
                .map_err(|e| format!("Failed to parse Word: {}", e))?;
            
            // 按行分割
            let lines: Vec<String> = content.lines()
                .map(|s| s.to_string())
                .collect();
            
            let mut mapping = std::collections::HashMap::new();
            let mut counter = 0usize;
            
            // 对每行进行脱敏
            let masked_lines: Vec<String> = lines
                .iter()
                .map(|line| masking_engine::mask_value_with_ner(line, &active_rules, &ner_detector, &mut mapping, &mut counter))
                .collect();
            
            // 将文本行转换为表格格式（单列）
            let original_rows: Vec<Vec<String>> = lines.iter()
                .map(|line| vec![line.clone()])
                .collect();
            
            let masked_rows: Vec<Vec<String>> = masked_lines.iter()
                .map(|line| vec![line.clone()])
                .collect();
            
            // Detect entities in original rows
            let detected_entities = Some(ner_detector.detect_in_rows(&original_rows));
            
            Ok(PreviewResult {
                original_rows,
                masked_rows,
                headers: vec!["内容".to_string()],
                detected_entities,
            })
        }
        file_parser::FileFormat::PowerPoint => {
            // 读取 PowerPoint 内容
            let content = file_parser::parse_powerpoint(&options.file_path)
                .map_err(|e| format!("Failed to parse PowerPoint: {}", e))?;
            
            // 按行分割
            let lines: Vec<String> = content.lines()
                .map(|s| s.to_string())
                .collect();
            
            let mut mapping = std::collections::HashMap::new();
            let mut counter = 0usize;
            
            // 对每行进行脱敏
            let masked_lines: Vec<String> = lines
                .iter()
                .map(|line| masking_engine::mask_value_with_ner(line, &active_rules, &ner_detector, &mut mapping, &mut counter))
                .collect();
            
            // 将文本行转换为表格格式（单列）
            let original_rows: Vec<Vec<String>> = lines.iter()
                .map(|line| vec![line.clone()])
                .collect();
            
            let masked_rows: Vec<Vec<String>> = masked_lines.iter()
                .map(|line| vec![line.clone()])
                .collect();
            
            // Detect entities in original rows
            let detected_entities = Some(ner_detector.detect_in_rows(&original_rows));
            
            Ok(PreviewResult {
                original_rows,
                masked_rows,
                headers: vec!["内容".to_string()],
                detected_entities,
            })
        }
        file_parser::FileFormat::Pdf => {
            // 读取 PDF 内容
            let content = file_parser::parse_pdf(&options.file_path)
                .map_err(|e| format!("Failed to parse PDF: {}", e))?;
            
            // 按行分割
            let lines: Vec<String> = content.lines()
                .map(|s| s.to_string())
                .collect();
            
            println!("PDF parsed into {} lines", lines.len());
            
            let mut mapping = std::collections::HashMap::new();
            let mut counter = 0usize;
            
            // 批量检测所有行
            let batch_entities = ner_detector.detect_entities_batch(&lines);
            println!("Batch detection completed for PDF lines");
            
            // 对每行应用脱敏
            let masked_lines: Vec<String> = lines
                .iter()
                .zip(batch_entities.iter())
                .map(|(line, entities)| {
                    if entities.is_empty() {
                        masking_engine::mask_value(line, &active_rules, &mut mapping, &mut counter)
                    } else {
                        masking_engine::apply_entities_to_text(line, entities, &mut mapping, &mut counter)
                    }
                })
                .collect();
            
            // 将文本行转换为表格格式（单列）
            let original_rows: Vec<Vec<String>> = lines.iter()
                .map(|line| vec![line.clone()])
                .collect();
            
            let masked_rows: Vec<Vec<String>> = masked_lines.iter()
                .map(|line| vec![line.clone()])
                .collect();
            
            // Detect entities in original rows
            let detected_entities = Some(ner_detector.detect_in_rows(&original_rows));
            
            Ok(PreviewResult {
                original_rows,
                masked_rows,
                headers: vec!["内容".to_string()],
                detected_entities,
            })
        }
        file_parser::FileFormat::Markdown | file_parser::FileFormat::Text => {
            // 读取文本内容
            let content = std::fs::read_to_string(&options.file_path)
                .map_err(|e| format!("Failed to read file: {}", e))?;
            
            // 按行分割（读取全部内容，不限制行数）
            let lines: Vec<String> = content.lines()
                .map(|s| s.to_string())
                .collect();
            
            let mut mapping = std::collections::HashMap::new();
            let mut counter = 0usize;
            
            // 对每行进行脱敏
            let masked_lines: Vec<String> = lines
                .iter()
                .map(|line| masking_engine::mask_value_with_ner(line, &active_rules, &ner_detector, &mut mapping, &mut counter))
                .collect();
            
            // 将文本行转换为表格格式（单列）
            let original_rows: Vec<Vec<String>> = lines.iter()
                .map(|line| vec![line.clone()])
                .collect();
            
            let masked_rows: Vec<Vec<String>> = masked_lines.iter()
                .map(|line| vec![line.clone()])
                .collect();
            
            // Detect entities in original rows
            let detected_entities = Some(ner_detector.detect_in_rows(&original_rows));
            
            Ok(PreviewResult {
                original_rows,
                masked_rows,
                headers: vec!["内容".to_string()],
                detected_entities,
            })
        }
        _ => Err("预览功能目前支持 CSV、Excel、Word、PowerPoint、PDF、Markdown 和 TXT 文件".to_string()),
    }
}

