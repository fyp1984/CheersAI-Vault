use serde::{Deserialize, Serialize};
use crate::core::crypto;

#[derive(Debug, Serialize, Deserialize)]
pub struct EncryptOptions {
    pub mapping_json: String,
    pub passphrase: String,
    pub output_path: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DecryptOptions {
    pub cmap_path: String,
    pub passphrase: String,
}

#[tauri::command]
pub async fn generate_passphrase() -> Result<String, String> {
    Ok(crypto::generate_passphrase_words())
}

#[tauri::command]
pub async fn encrypt_mapping(options: EncryptOptions) -> Result<String, String> {
    // Parse the JSON mapping
    let mappings: Vec<crate::core::masking_engine::MappingEntry> =
        serde_json::from_str(&options.mapping_json)
            .map_err(|e| format!("Failed to parse mapping JSON: {}", e))?;

    // Save encrypted mapping
    crypto::save_encrypted_mapping(&options.output_path, &mappings, &options.passphrase)?;

    Ok(options.output_path)
}

#[tauri::command]
pub async fn decrypt_mapping(options: DecryptOptions) -> Result<String, String> {
    // Load and decrypt mapping
    let mappings = crypto::load_encrypted_mapping(&options.cmap_path, &options.passphrase)?;

    // Convert to JSON
    let json = serde_json::to_string(&mappings)
        .map_err(|e| format!("Failed to serialize mappings: {}", e))?;

    Ok(json)
}
