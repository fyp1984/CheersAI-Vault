use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaskingRule {
    pub id: String,
    pub name: String,
    pub pattern: String,
    pub replacement_template: String,
    pub enabled: bool,
    pub builtin: bool,
    /// true = 追加自增序号（适合 PII 如姓名）；false = 固定文本（适合公司名/项目代号等）
    pub use_counter: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MappingEntry {
    pub original: String,
    pub masked: String,
    pub rule_id: String,
}

static BUILTIN_RULES: Lazy<Vec<MaskingRule>> = Lazy::new(|| {
    vec![
        MaskingRule {
            id: "id_card".to_string(),
            name: "身份证号".to_string(),
            pattern: r"[1-9]\d{5}(18|19|20)\d{2}(0[1-9]|1[0-2])(0[1-9]|[12]\d|3[01])\d{3}[\dXx]".to_string(),
            replacement_template: "***IDCARD***".to_string(),
            enabled: true,
            builtin: true,
            use_counter: true,
        },
        MaskingRule {
            id: "phone".to_string(),
            name: "手机号".to_string(),
            pattern: r"1[3-9]\d{9}".to_string(),
            replacement_template: "***PHONE***".to_string(),
            enabled: true,
            builtin: true,
            use_counter: true,
        },
        MaskingRule {
            id: "email".to_string(),
            name: "电子邮箱".to_string(),
            pattern: r"[A-Za-z0-9._%+\-]+@[A-Za-z0-9.\-]+\.[A-Za-z]{2,}".to_string(),
            replacement_template: "***EMAIL***".to_string(),
            enabled: true,
            builtin: true,
            use_counter: true,
        },
        MaskingRule {
            id: "bank_card".to_string(),
            name: "银行卡号".to_string(),
            pattern: r"[1-9]\d{15,18}".to_string(),
            replacement_template: "***BANKCARD***".to_string(),
            enabled: true,
            builtin: true,
            use_counter: true,
        },
        MaskingRule {
            id: "ipv4".to_string(),
            name: "IPv4地址".to_string(),
            pattern: r"(?:(?:25[0-5]|2[0-4]\d|[01]?\d\d?)\.){3}(?:25[0-5]|2[0-4]\d|[01]?\d\d?)".to_string(),
            replacement_template: "***IP***".to_string(),
            enabled: true,
            builtin: true,
            use_counter: true,
        },
        MaskingRule {
            id: "passport".to_string(),
            name: "护照号".to_string(),
            pattern: r"[A-Za-z][0-9]{8}".to_string(),
            replacement_template: "***PASSPORT***".to_string(),
            enabled: true,
            builtin: true,
            use_counter: true,
        },
        MaskingRule {
            id: "chinese_name".to_string(),
            name: "中文姓名".to_string(),
            pattern: r"[\u4e00-\u9fa5]{2,4}".to_string(),
            replacement_template: "姓名".to_string(),
            enabled: false,
            builtin: true,
            use_counter: true,
        },
    ]
});

pub fn get_builtin_rules() -> &'static Vec<MaskingRule> {
    &BUILTIN_RULES
}

/// 使用 NER + 规则进行脱敏
pub fn mask_value_with_ner(
    value: &str,
    rules: &[MaskingRule],
    ner_detector: &crate::core::ner::NERDetector,
    mapping: &mut HashMap<String, MappingEntry>,
    counter: &mut usize,
) -> String {
    let mut result = value.to_string();
    let original_value = value.to_string();
    
    // 1. 先使用规则进行脱敏（正则表达式匹配）
    result = mask_value(&result, rules, mapping, counter);
    
    // 2. 再使用 NER 检测原始文本中的实体（NER 在原始文本上工作）
    let entities = ner_detector.detect_entities(value);
    
    if !entities.is_empty() {
        println!("NER detected {} entities in original text", entities.len());
        
        // 3. 对于 NER 检测到的实体，如果还没有被脱敏，则进行脱敏
        // 需要在当前结果中查找这些实体
        for entity in entities {
            let original = entity.text.clone();
            
            // 检查这个实体是否还在结果中（如果不在，说明已经被规则脱敏了）
            if !result.contains(&original) {
                // 已经被规则脱敏了，跳过
                println!("NER entity already masked by rules: {}", original);
                continue;
            }
            
            // 检查是否已经有映射
            if let Some(entry) = mapping.values().find(|e| e.original == original) {
                println!("NER entity found in existing mapping: {} -> {}", original, entry.masked);
                result = result.replace(&original, &entry.masked);
                continue;
            }
            
            // 根据实体类型生成脱敏值
            let (masked, rule_id) = match entity.entity_type.as_str() {
                "身份证号" => {
                    *counter += 1;
                    (format!("***IDCARD{}***", counter), "id_card_ner".to_string())
                },
                "手机号" => {
                    *counter += 1;
                    (format!("***PHONE{}***", counter), "phone_ner".to_string())
                },
                "邮箱" => {
                    *counter += 1;
                    (format!("***EMAIL{}***", counter), "email_ner".to_string())
                },
                "银行卡号" => {
                    *counter += 1;
                    (format!("***BANKCARD{}***", counter), "bank_card_ner".to_string())
                },
                "IP地址" => {
                    *counter += 1;
                    (format!("***IP{}***", counter), "ipv4_ner".to_string())
                },
                "护照号" => {
                    *counter += 1;
                    (format!("***PASSPORT{}***", counter), "passport_ner".to_string())
                },
                "姓名" | "中文姓名" => {
                    *counter += 1;
                    (format!("姓名{}", counter), "chinese_name_ner".to_string())
                },
                _ => {
                    println!("NER detected unknown entity type: {} ({})", entity.entity_type, original);
                    continue; // 跳过未知类型
                }
            };
            
            println!("NER masking entity: {} -> {} (type: {})", original, masked, entity.entity_type);
            
            // 保存映射
            let map_key = format!("{}-{}", rule_id, counter);
            mapping.insert(
                map_key,
                MappingEntry {
                    original: original.clone(),
                    masked: masked.clone(),
                    rule_id,
                },
            );
            
            // 替换文本
            result = result.replace(&original, &masked);
        }
    }
    
    if original_value != result {
        println!("mask_value_with_ner: content changed from {} chars to {} chars", original_value.len(), result.len());
    } else {
        println!("mask_value_with_ner: NO changes made (input {} chars)", original_value.len());
    }
    
    result
}

pub fn mask_value(
    value: &str,
    rules: &[MaskingRule],
    mapping: &mut HashMap<String, MappingEntry>,
    counter: &mut usize,
) -> String {
    let mut result = value.to_string();
    let original_value = value.to_string();
    
    if value.len() > 0 && value.len() < 200 {
        println!("mask_value input: '{}'", value);
    } else if value.len() > 0 {
        println!("mask_value input: {} chars (too long to display)", value.len());
    }
    
    println!("mask_value: Processing with {} rules", rules.len());
    if rules.is_empty() {
        println!("WARNING: No rules provided to mask_value!");
    }

    for rule in rules {
        if !rule.enabled {
            println!("Skipping disabled rule: {}", rule.id);
            continue;
        }
        println!("Trying rule: {} (pattern: {})", rule.id, rule.pattern);
        let re = match Regex::new(&rule.pattern) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("Invalid regex pattern for rule {}: {}", rule.id, e);
                continue;
            }
        };

        let before = result.clone();
        result = re
            .replace_all(&result, |caps: &regex::Captures| {
                let original = caps[0].to_string();
                if let Some(entry) = mapping.values().find(|e| e.original == original) {
                    return entry.masked.clone();
                }
                let masked = if rule.use_counter {
                    *counter += 1;
                    format!("{}{}", rule.replacement_template, counter)
                } else {
                    rule.replacement_template.clone()
                };
                println!("New mapping: {} -> {} (rule: {})", original, masked, rule.id);
                let map_key = if rule.use_counter {
                    format!("{}-{}", rule.id, counter)
                } else {
                    format!("{}-{}", rule.id, original)
                };
                mapping.insert(
                    map_key,
                    MappingEntry {
                        original: original.clone(),
                        masked: masked.clone(),
                        rule_id: rule.id.clone(),
                    },
                );
                masked
            })
            .to_string();
        
        if before != result {
            println!("Rule {} matched and replaced content", rule.id);
        }
    }

    if original_value != result {
        println!("mask_value: content changed from {} chars to {} chars", original_value.len(), result.len());
    } else {
        println!("mask_value: NO changes made (input {} chars)", original_value.len());
    }

    result
}
