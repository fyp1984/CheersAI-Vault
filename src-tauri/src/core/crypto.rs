use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use rand::RngCore;
use zeroize::Zeroize;

const MAGIC: &[u8] = b"CMAP\x01";
const SALT_LEN: usize = 32;
const NONCE_LEN: usize = 12;

pub struct CryptoEngine;

impl CryptoEngine {
    /// Derive a 32-byte key from passphrase + salt using PBKDF2-SHA256
    fn derive_key(passphrase: &str, salt: &[u8]) -> Result<[u8; 32], String> {
        use sha2::Sha256;
        
        // 使用简单的 PBKDF2 实现
        let mut key = [0u8; 32];
        pbkdf2::pbkdf2::<hmac::Hmac<Sha256>>(
            passphrase.as_bytes(),
            salt,
            10000, // 迭代次数
            &mut key
        ).map_err(|e| format!("PBKDF2 error: {}", e))?;
        
        Ok(key)
    }

    /// Encrypt plaintext to .cmap format: MAGIC + SALT(32) + NONCE(12) + CIPHERTEXT+TAG
    pub fn encrypt(plaintext: &[u8], passphrase: &str) -> Result<Vec<u8>, String> {
        let mut salt = [0u8; SALT_LEN];
        rand::thread_rng().fill_bytes(&mut salt);

        let mut nonce_bytes = [0u8; NONCE_LEN];
        rand::thread_rng().fill_bytes(&mut nonce_bytes);

        let mut key = Self::derive_key(passphrase, &salt)?;

        let cipher = Aes256Gcm::new_from_slice(&key)
            .map_err(|e| format!("Cipher init error: {}", e))?;
        key.zeroize();

        let nonce = Nonce::from_slice(&nonce_bytes);
        let ciphertext = cipher
            .encrypt(nonce, plaintext)
            .map_err(|e| format!("Encrypt error: {}", e))?;

        let mut output = Vec::with_capacity(MAGIC.len() + SALT_LEN + NONCE_LEN + ciphertext.len());
        output.extend_from_slice(MAGIC);
        output.extend_from_slice(&salt);
        output.extend_from_slice(&nonce_bytes);
        output.extend_from_slice(&ciphertext);

        Ok(output)
    }

    /// Decrypt .cmap formatted bytes
    pub fn decrypt(data: &[u8], passphrase: &str) -> Result<Vec<u8>, String> {
        if data.len() < MAGIC.len() + SALT_LEN + NONCE_LEN + 16 {
            return Err("Data too short".to_string());
        }
        if &data[..MAGIC.len()] != MAGIC {
            return Err("Invalid magic bytes".to_string());
        }

        let offset = MAGIC.len();
        let salt = &data[offset..offset + SALT_LEN];
        let nonce_bytes = &data[offset + SALT_LEN..offset + SALT_LEN + NONCE_LEN];
        let ciphertext = &data[offset + SALT_LEN + NONCE_LEN..];

        let mut key = Self::derive_key(passphrase, salt)?;

        let cipher = Aes256Gcm::new_from_slice(&key)
            .map_err(|e| format!("Cipher init error: {}", e))?;
        key.zeroize();

        let nonce = Nonce::from_slice(nonce_bytes);
        let plaintext = cipher
            .decrypt(nonce, ciphertext)
            .map_err(|_| "Decryption failed — wrong passphrase?".to_string())?;

        Ok(plaintext)
    }
}

/// Generate a random BIP39-style passphrase (word list fallback: hex)
pub fn generate_passphrase_words() -> String {
    let mut bytes = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut bytes);
    
    // 生成更友好的口令格式
    let words = [
        "apple", "brave", "cloud", "dance", "eagle", "flame", "grace", "heart",
        "light", "magic", "ocean", "peace", "quick", "river", "storm", "trust",
        "unity", "voice", "water", "youth", "zebra", "angel", "bloom", "charm",
        "dream", "earth", "frost", "giant", "happy", "ivory", "jewel", "karma"
    ];
    
    // 选择4个随机单词
    let indices: Vec<usize> = (0..4)
        .map(|i| (bytes[i * 4] as usize) % words.len())
        .collect();
    
    indices.iter()
        .map(|&i| words[i])
        .collect::<Vec<_>>()
        .join("-")
}

/// Save plain JSON mapping to file（无 passphrase 时使用）
pub fn save_plain_mapping(
    path: &str,
    mappings: &[crate::core::masking_engine::MappingEntry],
) -> Result<(), String> {
    use std::fs;
    let json = serde_json::to_string_pretty(mappings)
        .map_err(|e| format!("Failed to serialize mappings: {}", e))?;
    fs::write(path, json.as_bytes())
        .map_err(|e| format!("Failed to write mapping file: {}", e))?;
    #[cfg(target_os = "windows")]
    {
        use std::path::Path;
        let win_path = Path::new(path).to_string_lossy().replace("/", "\\");
        let _ = std::process::Command::new("attrib")
            .args(["-h", "-s", &win_path])
            .output();
    }
    Ok(())
}

/// Save encrypted mapping to file
pub fn save_encrypted_mapping(
    path: &str,
    mappings: &[crate::core::masking_engine::MappingEntry],
    passphrase: &str,
) -> Result<(), String> {
    use std::fs;

    let json = serde_json::to_string(mappings)
        .map_err(|e| format!("Failed to serialize mappings: {}", e))?;

    let encrypted = CryptoEngine::encrypt(json.as_bytes(), passphrase)?;

    fs::write(path, encrypted)
        .map_err(|e| format!("Failed to write mapping file: {}", e))?;

    // Windows: 确保 .cmap 文件不被隐藏
    #[cfg(target_os = "windows")]
    {
        use std::path::Path;
        let win_path = Path::new(path).to_string_lossy().replace("/", "\\");
        let result = std::process::Command::new("attrib")
            .args(["-h", "-s", &win_path])
            .output();
        if let Err(e) = result {
            eprintln!("Warning: Failed to set file attributes for {}: {}", win_path, e);
        }
    }

    Ok(())
}

/// Load and decrypt mapping from file（自动兼容加密和明文 JSON 两种格式）
pub fn load_encrypted_mapping(
    path: &str,
    passphrase: &str,
) -> Result<Vec<crate::core::masking_engine::MappingEntry>, String> {
    use std::fs;

    let data = fs::read(path)
        .map_err(|e| format!("Failed to read mapping file: {}", e))?;

    // 检测是否为加密格式（以 MAGIC bytes 开头）
    if data.starts_with(MAGIC) {
        let decrypted = CryptoEngine::decrypt(&data, passphrase)?;
        let json = String::from_utf8(decrypted)
            .map_err(|e| format!("Invalid UTF-8 in decrypted data: {}", e))?;
        let mappings: Vec<crate::core::masking_engine::MappingEntry> = serde_json::from_str(&json)
            .map_err(|e| format!("Failed to deserialize mappings: {}", e))?;
        Ok(mappings)
    } else {
        // 明文 JSON 格式（无 passphrase 时生成）
        let json = String::from_utf8(data)
            .map_err(|e| format!("Invalid UTF-8 in mapping file: {}", e))?;
        let mappings: Vec<crate::core::masking_engine::MappingEntry> = serde_json::from_str(&json)
            .map_err(|e| format!("Failed to deserialize plain mappings: {}", e))?;
        Ok(mappings)
    }
}
