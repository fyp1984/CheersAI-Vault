use std::collections::HashMap;

pub use engine_core::{get_builtin_rules, MappingEntry, MaskingRule};
use engine_core::{DeterministicFinding, MaskingSession};

fn to_findings(entities: &[crate::core::ner::EntityMatch]) -> Vec<DeterministicFinding> {
    entities
        .iter()
        .map(|entity| DeterministicFinding {
            text: entity.text.clone(),
            entity_type: entity.entity_type.clone(),
            start: entity.start,
            end: entity.end,
        })
        .collect()
}

fn session_from_legacy_state(
    rules: &[MaskingRule],
    mapping: &HashMap<String, MappingEntry>,
    counter: usize,
) -> MaskingSession {
    let mut mappings: Vec<_> = mapping.values().cloned().collect();
    mappings.sort_by(|left, right| left.masked.cmp(&right.masked));
    MaskingSession::with_state(rules.to_vec(), mappings, counter)
}

fn sync_legacy_state(
    session: &MaskingSession,
    mapping: &mut HashMap<String, MappingEntry>,
    counter: &mut usize,
) {
    mapping.clear();
    for (index, entry) in session.mappings().iter().cloned().enumerate() {
        mapping.insert(format!("{}-{}", entry.rule_id, index + 1), entry);
    }
    *counter = session.placeholder_counter();
}

/// 桌面 Adapter：NER 检测仍留在当前进程，规则、映射与替换由 engine-core 执行。
pub fn mask_value_with_ner(
    value: &str,
    rules: &[MaskingRule],
    ner_detector: &crate::core::ner::NERDetector,
    mapping: &mut HashMap<String, MappingEntry>,
    counter: &mut usize,
) -> String {
    let findings = to_findings(&ner_detector.detect_entities(value));
    let mut session = session_from_legacy_state(rules, mapping, *counter);
    let masked = session.mask_fragment(value, &findings);
    sync_legacy_state(&session, mapping, counter);
    masked
}

/// 桌面批处理 Adapter：使用已经检测到的实体，不在共享核心中启动任何外部能力。
pub fn apply_entities_to_text(
    value: &str,
    entities: &[crate::core::ner::EntityMatch],
    mapping: &mut HashMap<String, MappingEntry>,
    counter: &mut usize,
) -> String {
    let findings = to_findings(entities);
    let mut session = session_from_legacy_state(&[], mapping, *counter);
    let masked = session.apply_findings_fragment_unchecked(value, &findings);
    sync_legacy_state(&session, mapping, counter);
    masked
}

/// 兼容现有非 TXT/Markdown Adapter；算法实现只存在于 engine-core。
pub fn mask_value(
    value: &str,
    rules: &[MaskingRule],
    mapping: &mut HashMap<String, MappingEntry>,
    counter: &mut usize,
) -> String {
    let mut session = session_from_legacy_state(rules, mapping, *counter);
    let masked = session.mask_fragment(value, &[]);
    sync_legacy_state(&session, mapping, counter);
    masked
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::ner::EntityMatch;

    fn entity(text: &str, entity_type: &str) -> EntityMatch {
        EntityMatch {
            text: text.to_string(),
            entity_type: entity_type.to_string(),
            start: 0,
            end: text.len(),
            confidence: 1.0,
            source: "test".to_string(),
        }
    }

    #[test]
    fn legacy_entity_adapter_preserves_name_placeholder() {
        let mut mapping = HashMap::new();
        let mut counter = 0;
        let masked = apply_entities_to_text(
            "测试姓名甲",
            &[entity("测试姓名甲", "姓名")],
            &mut mapping,
            &mut counter,
        );

        assert_eq!(masked, "***NAME1***");
        assert_eq!(counter, 1);
        assert_eq!(mapping.values().next().unwrap().rule_id, "name_ner");
    }

    #[test]
    fn legacy_entity_adapter_preserves_generic_sensitive_placeholder() {
        let mut mapping = HashMap::new();
        let mut counter = 0;
        let masked = apply_entities_to_text(
            "2099-12-31",
            &[entity("2099-12-31", "日期")],
            &mut mapping,
            &mut counter,
        );

        assert_eq!(masked, "***SENSITIVE1***");
        assert_eq!(counter, 1);
        assert_eq!(mapping.values().next().unwrap().rule_id, "unknown_ner");
    }
}
