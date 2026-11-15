use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use hkdf::Hkdf;
use rand::RngCore;
use sha2::{Digest, Sha256};
use zeroize::{Zeroize, ZeroizeOnDrop};

const MAGIC: &[u8] = b"CMAP\x01";
pub const ECMAP_MAGIC: &[u8] = b"ECMAP\x02";
pub const ENCSRC_MAGIC: &[u8] = b"VAULT_ENCSRC\x01";
const SALT_LEN: usize = 32;
const NONCE_LEN: usize = 12;

#[derive(Debug, Clone, Zeroize, ZeroizeOnDrop)]
pub enum EncSourcePassMode {
    SandboxReused,
    SecondaryPhrase { phrase: String },
    DeviceKey,
}

#[derive(Debug, Clone, Copy)]
pub enum KeyDomain {
    CmapV1,
    EcmapV1,
    EncsrcV1,
}

impl KeyDomain {
    pub fn to_bytes(self) -> &'static [u8] {
        match self {
            KeyDomain::CmapV1 => b"CMAP_V1\0",
            KeyDomain::EcmapV1 => b"ECMAP_V1\0",
            KeyDomain::EncsrcV1 => b"ENCSRC_V1\0",
        }
    }
}

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
            &mut key,
        )
        .map_err(|e| format!("PBKDF2 error: {}", e))?;

        Ok(key)
    }

    /// Encrypt plaintext to .cmap format: MAGIC + SALT(32) + NONCE(12) + CIPHERTEXT+TAG
    pub fn encrypt(plaintext: &[u8], passphrase: &str) -> Result<Vec<u8>, String> {
        let mut salt = [0u8; SALT_LEN];
        rand::thread_rng().fill_bytes(&mut salt);

        let mut nonce_bytes = [0u8; NONCE_LEN];
        rand::thread_rng().fill_bytes(&mut nonce_bytes);

        let mut key = Self::derive_key(passphrase, &salt)?;

        let cipher =
            Aes256Gcm::new_from_slice(&key).map_err(|e| format!("Cipher init error: {}", e))?;
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

        let cipher =
            Aes256Gcm::new_from_slice(&key).map_err(|e| format!("Cipher init error: {}", e))?;
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
        "apple", "brave", "cloud", "dance", "eagle", "flame", "grace", "heart", "light", "magic",
        "ocean", "peace", "quick", "river", "storm", "trust", "unity", "voice", "water", "youth",
        "zebra", "angel", "bloom", "charm", "dream", "earth", "frost", "giant", "happy", "ivory",
        "jewel", "karma",
    ];

    // 选择4个随机单词
    let indices: Vec<usize> = (0..4)
        .map(|i| (bytes[i * 4] as usize) % words.len())
        .collect();

    indices
        .iter()
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
    fs::write(path, json.as_bytes()).map_err(|e| format!("Failed to write mapping file: {}", e))?;
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        use std::path::Path;
        const CREATE_NO_WINDOW: u32 = 0x08000000;

        let win_path = Path::new(path).to_string_lossy().replace("/", "\\");
        let _ = std::process::Command::new("attrib")
            .creation_flags(CREATE_NO_WINDOW)
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

    fs::write(path, encrypted).map_err(|e| format!("Failed to write mapping file: {}", e))?;

    // Windows: 确保 .cmap 文件不被隐藏
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        use std::path::Path;
        const CREATE_NO_WINDOW: u32 = 0x08000000;

        let win_path = Path::new(path).to_string_lossy().replace("/", "\\");
        let result = std::process::Command::new("attrib")
            .creation_flags(CREATE_NO_WINDOW)
            .args(["-h", "-s", &win_path])
            .output();
        if let Err(e) = result {}
    }

    Ok(())
}

/// Load and decrypt mapping from file（自动兼容加密和明文 JSON 两种格式）
pub fn load_encrypted_mapping(
    path: &str,
    passphrase: &str,
) -> Result<Vec<crate::core::masking_engine::MappingEntry>, String> {
    use std::fs;

    let data = fs::read(path).map_err(|e| format!("Failed to read mapping file: {}", e))?;

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
        let json =
            String::from_utf8(data).map_err(|e| format!("Invalid UTF-8 in mapping file: {}", e))?;
        let mappings: Vec<crate::core::masking_engine::MappingEntry> = serde_json::from_str(&json)
            .map_err(|e| format!("Failed to deserialize plain mappings: {}", e))?;
        Ok(mappings)
    }
}

impl CryptoEngine {
    pub fn derive_key_scoped(
        passphrase: &str,
        salt: &[u8],
        domain: KeyDomain,
    ) -> Result<[u8; 32], String> {
        let domain_tag = domain.to_bytes();
        let scoped_input = [domain_tag, passphrase.as_bytes()].concat();
        let mut key = [0u8; 32];
        pbkdf2::pbkdf2::<hmac::Hmac<Sha256>>(&scoped_input, salt, 200_000, &mut key)
            .map_err(|e| format!("PBKDF2 scoped error: {}", e))?;
        Ok(key)
    }

    pub fn encrypt_domain(
        plaintext: &[u8],
        passphrase: &str,
        domain: KeyDomain,
    ) -> Result<Vec<u8>, String> {
        let mut salt = [0u8; SALT_LEN];
        rand::thread_rng().fill_bytes(&mut salt);
        let mut nonce_bytes = [0u8; NONCE_LEN];
        rand::thread_rng().fill_bytes(&mut nonce_bytes);

        let mut key = Self::derive_key_scoped(passphrase, &salt, domain)?;
        let cipher =
            Aes256Gcm::new_from_slice(&key).map_err(|e| format!("Cipher init error: {}", e))?;
        key.zeroize();

        let nonce = Nonce::from_slice(&nonce_bytes);
        let ciphertext = cipher
            .encrypt(nonce, plaintext)
            .map_err(|e| format!("Encrypt domain error: {}", e))?;

        let mut output = Vec::with_capacity(MAGIC.len() + SALT_LEN + NONCE_LEN + ciphertext.len());
        output.extend_from_slice(domain.to_bytes());
        output.extend_from_slice(&salt);
        output.extend_from_slice(&nonce_bytes);
        output.extend_from_slice(&ciphertext);
        Ok(output)
    }

    pub fn decrypt_domain(
        data: &[u8],
        passphrase: &str,
        domain: KeyDomain,
    ) -> Result<Vec<u8>, String> {
        let tag = domain.to_bytes();
        let min_len = tag.len() + SALT_LEN + NONCE_LEN + 16;
        if data.len() < min_len {
            return Err("Scoped data too short".to_string());
        }
        if &data[..tag.len()] != tag {
            return Err("Scoped domain tag mismatch".to_string());
        }
        let offset = tag.len();
        let salt = &data[offset..offset + SALT_LEN];
        let nonce_bytes = &data[offset + SALT_LEN..offset + SALT_LEN + NONCE_LEN];
        let ciphertext = &data[offset + SALT_LEN + NONCE_LEN..];

        let mut key = Self::derive_key_scoped(passphrase, salt, domain)?;
        let cipher =
            Aes256Gcm::new_from_slice(&key).map_err(|e| format!("Cipher init error: {}", e))?;
        key.zeroize();

        let nonce = Nonce::from_slice(nonce_bytes);
        let plaintext = cipher
            .decrypt(nonce, ciphertext)
            .map_err(|_| "Scoped decryption failed — wrong passphrase?".to_string())?;
        Ok(plaintext)
    }
}

pub fn domain_hint8(pass: &str, domain: KeyDomain) -> String {
    let mut hasher = Sha256::new();
    hasher.update(pass.as_bytes());
    hasher.update(b"HINT");
    hasher.update(domain.to_bytes());
    let out = hasher.finalize();
    let mut s = String::with_capacity(16);
    for b in &out[..8] {
        use std::fmt::Write;
        let _ = write!(s, "{:02x}", b);
    }
    s
}

fn passphrase_from_mode(
    mode: &EncSourcePassMode,
    fallback_sandbox: &str,
) -> Result<String, String> {
    match mode {
        EncSourcePassMode::SandboxReused => {
            if fallback_sandbox.trim().is_empty() {
                return Err("Sandbox passphrase must not be empty".to_string());
            }
            Ok(fallback_sandbox.to_string())
        }
        EncSourcePassMode::SecondaryPhrase { phrase } => Ok(phrase.clone()),
        EncSourcePassMode::DeviceKey => {
            let entry = keyring::Entry::new("cheersai-vault-device-master", "device")
                .map_err(|e| format!("keyring entry error: {}", e))?;
            match entry.get_password() {
                Ok(pw) if !pw.is_empty() => Ok(pw),
                _ => {
                    let mut bytes = [0u8; 32];
                    rand::thread_rng().fill_bytes(&mut bytes);
                    let hex: String = bytes.iter().map(|b| format!("{:02x}", b)).collect();
                    entry
                        .set_password(&hex)
                        .map_err(|e| format!("keyring set error: {}", e))?;
                    Ok(hex)
                }
            }
        }
    }
}

pub fn encrypt_ecmap(
    json_bytes: &[u8],
    pass: &str,
    pass_mode: EncSourcePassMode,
) -> Result<Vec<u8>, String> {
    let mut mode = pass_mode;
    let effective_pass = passphrase_from_mode(&mode, pass)?;

    let mut salt = [0u8; SALT_LEN];
    rand::thread_rng().fill_bytes(&mut salt);
    let mut nonce_bytes = [0u8; NONCE_LEN];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);

    let mut key = CryptoEngine::derive_key_scoped(&effective_pass, &salt, KeyDomain::EcmapV1)?;
    let cipher =
        Aes256Gcm::new_from_slice(&key).map_err(|e| format!("Cipher init error: {}", e))?;
    key.zeroize();
    if let EncSourcePassMode::SecondaryPhrase { ref mut phrase } = mode {
        phrase.zeroize();
    }

    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher
        .encrypt(nonce, json_bytes)
        .map_err(|e| format!("Encrypt ECMAP error: {}", e))?;

    let mut output =
        Vec::with_capacity(ECMAP_MAGIC.len() + SALT_LEN + NONCE_LEN + ciphertext.len());
    output.extend_from_slice(ECMAP_MAGIC);
    output.extend_from_slice(&salt);
    output.extend_from_slice(&nonce_bytes);
    output.extend_from_slice(&ciphertext);
    Ok(output)
}

pub fn decrypt_ecmap(data: &[u8], pass: &str) -> Result<Vec<u8>, String> {
    let min_len = ECMAP_MAGIC.len() + SALT_LEN + NONCE_LEN + 16;
    if data.len() < min_len {
        return Err("ECMAP data too short".to_string());
    }
    if !data.starts_with(ECMAP_MAGIC) {
        return Err("Invalid ECMAP magic bytes".to_string());
    }
    let offset = ECMAP_MAGIC.len();
    let salt = &data[offset..offset + SALT_LEN];
    let nonce_bytes = &data[offset + SALT_LEN..offset + SALT_LEN + NONCE_LEN];
    let ciphertext = &data[offset + SALT_LEN + NONCE_LEN..];

    let mut key = CryptoEngine::derive_key_scoped(pass, salt, KeyDomain::EcmapV1)?;
    let cipher =
        Aes256Gcm::new_from_slice(&key).map_err(|e| format!("Cipher init error: {}", e))?;
    key.zeroize();

    let nonce = Nonce::from_slice(nonce_bytes);
    cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| "ECMAP decryption failed — wrong passphrase?".to_string())
}

pub fn encrypt_encsrc(
    original_bytes: &[u8],
    pass: &str,
    pass_mode: EncSourcePassMode,
) -> Result<Vec<u8>, String> {
    let mut mode = pass_mode;
    let (effective_pass, use_hkdf_per_file) = match &mode {
        EncSourcePassMode::DeviceKey => {
            let entry = keyring::Entry::new("cheersai-vault-device-master", "device")
                .map_err(|e| format!("keyring entry error: {}", e))?;
            let master = match entry.get_password() {
                Ok(pw) if !pw.is_empty() => pw,
                _ => {
                    let mut bytes = [0u8; 32];
                    rand::thread_rng().fill_bytes(&mut bytes);
                    let hex: String = bytes.iter().map(|b| format!("{:02x}", b)).collect();
                    entry
                        .set_password(&hex)
                        .map_err(|e| format!("keyring set error: {}", e))?;
                    hex
                }
            };
            (master, true)
        }
        _ => (passphrase_from_mode(&mode, pass)?, false),
    };

    let mut salt = [0u8; SALT_LEN];
    rand::thread_rng().fill_bytes(&mut salt);
    let mut nonce_bytes = [0u8; NONCE_LEN];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);

    let mut key = if use_hkdf_per_file {
        let mut base_key =
            CryptoEngine::derive_key_scoped(&effective_pass, &salt, KeyDomain::EncsrcV1)?;
        let file_hash = {
            let mut h = Sha256::new();
            h.update(original_bytes);
            let r = h.finalize();
            let mut o = [0u8; 32];
            o.copy_from_slice(&r);
            o
        };
        let info = &file_hash[..16];
        let hk =
            Hkdf::<Sha256>::from_prk(&base_key).map_err(|e| format!("HKDF PRK error: {}", e))?;
        base_key.zeroize();
        let mut per_file = [0u8; 32];
        hk.expand(info, &mut per_file)
            .map_err(|e| format!("HKDF expand error: {}", e))?;
        per_file
    } else {
        CryptoEngine::derive_key_scoped(&effective_pass, &salt, KeyDomain::EncsrcV1)?
    };

    let cipher =
        Aes256Gcm::new_from_slice(&key).map_err(|e| format!("Cipher init error: {}", e))?;
    key.zeroize();
    if let EncSourcePassMode::SecondaryPhrase { ref mut phrase } = mode {
        phrase.zeroize();
    }

    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher
        .encrypt(nonce, original_bytes)
        .map_err(|e| format!("Encrypt ENCSRC error: {}", e))?;

    let mut output =
        Vec::with_capacity(ENCSRC_MAGIC.len() + SALT_LEN + NONCE_LEN + ciphertext.len());
    output.extend_from_slice(ENCSRC_MAGIC);
    output.extend_from_slice(&salt);
    output.extend_from_slice(&nonce_bytes);
    output.extend_from_slice(&ciphertext);
    Ok(output)
}

pub fn decrypt_encsrc(data: &[u8], pass: &str) -> Result<Vec<u8>, String> {
    let min_len = ENCSRC_MAGIC.len() + SALT_LEN + NONCE_LEN + 16;
    if data.len() < min_len {
        return Err("ENCSRC data too short".to_string());
    }
    if !data.starts_with(ENCSRC_MAGIC) {
        return Err("Invalid ENCSRC magic bytes".to_string());
    }
    let offset = ENCSRC_MAGIC.len();
    let salt = &data[offset..offset + SALT_LEN];
    let nonce_bytes = &data[offset + SALT_LEN..offset + SALT_LEN + NONCE_LEN];
    let ciphertext = &data[offset + SALT_LEN + NONCE_LEN..];

    let entry = keyring::Entry::new("cheersai-vault-device-master", "device").ok();
    let master = entry.as_ref().and_then(|e| e.get_password().ok());
    let try_keys: Vec<[u8; 32]> = vec![
        CryptoEngine::derive_key_scoped(pass, salt, KeyDomain::EncsrcV1).ok(),
        master
            .as_ref()
            .and_then(|m| CryptoEngine::derive_key_scoped(m, salt, KeyDomain::EncsrcV1).ok()),
    ]
    .into_iter()
    .flatten()
    .collect();

    let nonce = Nonce::from_slice(nonce_bytes);
    for mut k in try_keys {
        let cipher =
            Aes256Gcm::new_from_slice(&k).map_err(|e| format!("Cipher init error: {}", e))?;
        k.zeroize();
        if let Ok(plain) = cipher.decrypt(nonce, ciphertext) {
            return Ok(plain);
        }
    }
    Err("ENCSRC decryption failed — wrong passphrase/device?".to_string())
}

#[cfg(test)]
mod tests {
    use super::{encrypt_ecmap, encrypt_encsrc, passphrase_from_mode, EncSourcePassMode};

    #[test]
    fn sandbox_reused_rejects_empty_and_whitespace_only_fallbacks() {
        for fallback in ["", "   ", "\t\n"] {
            let result = passphrase_from_mode(&EncSourcePassMode::SandboxReused, fallback);
            assert_eq!(result.unwrap_err(), "Sandbox passphrase must not be empty");
        }
    }

    #[test]
    fn sandbox_reused_preserves_non_empty_fallback_bytes() {
        let fallback = "  fixture sandbox passphrase  ";
        let result = passphrase_from_mode(&EncSourcePassMode::SandboxReused, fallback);
        assert_eq!(result.as_deref(), Ok(fallback));
    }

    #[test]
    fn encryption_boundaries_fail_closed_for_empty_sandbox_fallbacks() {
        for fallback in ["", "   ", "\t\n"] {
            assert_eq!(
                encrypt_ecmap(b"{}", fallback, EncSourcePassMode::SandboxReused).unwrap_err(),
                "Sandbox passphrase must not be empty"
            );
            assert_eq!(
                encrypt_encsrc(b"fixture", fallback, EncSourcePassMode::SandboxReused).unwrap_err(),
                "Sandbox passphrase must not be empty"
            );
        }
    }
}
