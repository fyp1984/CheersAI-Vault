use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleConfig {
    pub rules: Vec<RuleEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleEntry {
    pub id: String,
    pub name: String,
    pub pattern: String,
    pub replacement: String,
    pub enabled: bool,
    pub builtin: bool,
}

pub fn load_rules(path: &str) -> Result<RuleConfig> {
    let content = std::fs::read_to_string(path)?;
    let config: RuleConfig = serde_json::from_str(&content)?;
    Ok(config)
}

pub fn save_rules(path: &str, config: &RuleConfig) -> Result<()> {
    let content = serde_json::to_string_pretty(config)?;
    std::fs::write(path, content)?;
    Ok(())
}
