use serde::{Deserialize, Serialize};
use crate::core::masking_engine;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaskRule {
    pub id: String,
    pub name: String,
    pub pattern: String,
    pub replacement: String,
    pub enabled: bool,
    pub builtin: bool,
}

#[tauri::command]
pub async fn get_rules() -> Result<Vec<MaskRule>, String> {
    let builtin_rules = masking_engine::get_builtin_rules();
    let rules: Vec<MaskRule> = builtin_rules
        .iter()
        .map(|r| MaskRule {
            id: r.id.clone(),
            name: r.name.clone(),
            pattern: r.pattern.clone(),
            replacement: r.replacement_template.clone(),
            enabled: r.enabled,
            builtin: r.builtin,
        })
        .collect();
    
    Ok(rules)
}

#[tauri::command]
pub async fn save_rules(_rules: Vec<MaskRule>) -> Result<(), String> {
    // For now, we only support built-in rules
    // In the future, this could save custom rules to a config file
    Err("Saving custom rules is not implemented yet".to_string())
}
