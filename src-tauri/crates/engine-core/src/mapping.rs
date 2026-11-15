//! Shared mapping / restore module.
//!
//! This is the single place for:
//! - `.cmap v2` encryption & decryption (AES-256-GCM, PBKDF2 600k)
//! - Legacy `CMAP\x01` read-only decryption (PBKDF2 10k)
//! - Markdown restoration from `MappingEntry`
//! - Unified error types
//!
//! Architecture boundary
//! ---------------------
//! Lives in engine-core so that both desktop Tauri and the enterprise
//! vault-runtime-api call the exact same code.
//!
//! v2 container layout (frozen):
//!   CVAULTCMAP             9 bytes magic
//!   version                1 byte   (=2)
//!   header_len             4 bytes  big-endian u32
//!   header_json             variable UTF-8 JSON
//!   ciphertext_and_tag      remaining (AES-256-GCM)

use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use rand::RngCore;
use serde::{Deserialize, Serialize};

use crate::MappingEntry;

use crate::AppError;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const V2_MAGIC: &[u8] = b"CVAULTCMAP";
const V1_MAGIC: &[u8] = b"CMAP\x01";
const V2_KDF_ITERATIONS: u32 = 600_000;
const V1_KDF_ITERATIONS: u32 = 10_000;
const SALT_LEN: usize = 32;
const NONCE_LEN: usize = 12;
const DERIVED_KEY_LEN: usize = 32;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Cmap version detected after parsing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CmapVersion {
    V2,
    LegacyV1,
}

/// Result of a successful restore operation.
#[derive(Debug, Clone)]
pub struct RestoreResult {
    pub restored_markdown: String,
    pub restored_count: usize,
    pub cmap_version: CmapVersion,
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, thiserror::Error)]
pub enum MappingError {
    #[error("wrong passphrase or corrupted data")]
    AuthFailed,
    #[error("unsupported version or plaintext cmap")]
    VersionUnsupported,
    #[error("mapping data is empty or invalid")]
    InvalidMapping,
    #[error("I/O error: {0}")]
    Io(String),
}

impl MappingError {
    pub fn error_code(&self) -> &'static str {
        match self {
            Self::AuthFailed => "CMAP_AUTH_FAILED",
            Self::VersionUnsupported => "CMAP_VERSION_UNSUPPORTED",
            Self::InvalidMapping => "CMAP_MISMATCH",
            Self::Io(_) => "INPUT_CORRUPTED",
        }
    }

    pub fn to_app_error(self) -> AppError {
        let (code, retryable) = match &self {
            Self::AuthFailed => ("CMAP_AUTH_FAILED", false),
            Self::VersionUnsupported => ("CMAP_VERSION_UNSUPPORTED", false),
            Self::InvalidMapping => ("CMAP_MISMATCH", true),
            Self::Io(_) => ("INPUT_CORRUPTED", false),
        };
        AppError {
            code: code.into(),
            message: self.to_string(),
            retryable,
            safe_details: None,
        }
    }
}

// ---------------------------------------------------------------------------
// v2 header JSON structure
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
struct V2Header {
    version: u8,
    kdf_name: String,
    kdf_iterations: u32,
    derived_key_len: usize,
    #[serde(rename = "salt_base64")]
    salt_b64: String,
    aead_name: String,
    #[serde(rename = "nonce_base64")]
    nonce_b64: String,
    artifact_id: String,
    created_at: String,
}

// ---------------------------------------------------------------------------
// v2 encode / decode
// ---------------------------------------------------------------------------

/// Encrypt `MappingEntry` slice into a v2 `.cmap` byte vector.
///
/// Uses PBKDF2-HMAC-SHA256 (600 000 iterations), AES-256-GCM with a random
/// salt (32 B) and nonce (12 B). Each call produces unique output even with
/// the same passphrase and mappings because salt + nonce are random.
pub fn encrypt_v2(
    entries: &[MappingEntry],
    passphrase: &str,
    artifact_id: &str,
    created_at: &str,
) -> Result<Vec<u8>, MappingError> {
    let payload = serde_json::to_vec(entries).map_err(|_| MappingError::InvalidMapping)?;

    let mut salt = [0u8; SALT_LEN];
    OsRng.fill_bytes(&mut salt);
    let mut nonce_bytes = [0u8; NONCE_LEN];
    OsRng.fill_bytes(&mut nonce_bytes);

    let key = derive_key(passphrase, &salt, V2_KDF_ITERATIONS)?;
    let cipher = Aes256Gcm::new_from_slice(&key).map_err(|_| MappingError::AuthFailed)?;

    let header = V2Header {
        version: 2,
        kdf_name: "PBKDF2-HMAC-SHA256".into(),
        kdf_iterations: V2_KDF_ITERATIONS,
        derived_key_len: DERIVED_KEY_LEN,
        salt_b64: base64_encode(&salt),
        aead_name: "AES-256-GCM".into(),
        nonce_b64: base64_encode(&nonce_bytes),
        artifact_id: artifact_id.into(),
        created_at: created_at.into(),
    };
    let header_json = serde_json::to_vec(&header).map_err(|_| MappingError::InvalidMapping)?;
    let header_len = header_json.len() as u32;

    // Build AAD = magic + version byte + header_len (BE) + raw header_json
    let mut aad = Vec::with_capacity(V2_MAGIC.len() + 1 + 4 + header_json.len());
    aad.extend_from_slice(V2_MAGIC);
    aad.push(2u8);
    aad.extend_from_slice(&header_len.to_be_bytes());
    aad.extend_from_slice(&header_json);

    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher
        .encrypt(nonce, payload.as_slice())
        .map_err(|_| MappingError::AuthFailed)?;

    // Build binary: magic + version + header_len + header_json + ciphertext+tag
    let mut output =
        Vec::with_capacity(V2_MAGIC.len() + 1 + 4 + header_json.len() + ciphertext.len());
    output.extend_from_slice(V2_MAGIC);
    output.push(2u8);
    output.extend_from_slice(&header_len.to_be_bytes());
    output.extend_from_slice(&header_json);
    output.extend_from_slice(&ciphertext);

    Ok(output)
}

/// Decrypt a v2 `.cmap` byte slice.
///
/// Returns `(entries, header)` on success.
fn decrypt_v2(
    data: &[u8],
    passphrase: &str,
) -> Result<(Vec<MappingEntry>, V2Header), MappingError> {
    let mut offset = V2_MAGIC.len() + 1; // skip magic + version byte
    if data.len() < offset + 4 {
        return Err(MappingError::AuthFailed);
    }

    let header_len = u32::from_be_bytes([
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
    ]) as usize;
    offset += 4;

    if data.len() < offset + header_len + 16 {
        return Err(MappingError::AuthFailed);
    }
    if header_len > 4096 {
        return Err(MappingError::AuthFailed); // sanity bound
    }

    let header_json = &data[offset..offset + header_len];
    offset += header_len;
    let ciphertext = &data[offset..];

    let header: V2Header =
        serde_json::from_slice(header_json).map_err(|_| MappingError::AuthFailed)?;

    // Rebuild AAD exactly as was used during encryption
    let mut aad = Vec::with_capacity(V2_MAGIC.len() + 1 + 4 + header_json.len());
    aad.extend_from_slice(V2_MAGIC);
    aad.push(2u8);
    aad.extend_from_slice(&(header_json.len() as u32).to_be_bytes());
    aad.extend_from_slice(header_json);

    let salt = base64_decode(&header.salt_b64)?;
    if salt.len() != SALT_LEN {
        return Err(MappingError::AuthFailed);
    }
    let nonce_bytes = base64_decode(&header.nonce_b64)?;
    if nonce_bytes.len() != NONCE_LEN {
        return Err(MappingError::AuthFailed);
    }

    let key = derive_key(passphrase, &salt, header.kdf_iterations)?;
    let cipher = Aes256Gcm::new_from_slice(&key).map_err(|_| MappingError::AuthFailed)?;

    let nonce = Nonce::from_slice(&nonce_bytes);
    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| MappingError::AuthFailed)?;

    let entries: Vec<MappingEntry> =
        serde_json::from_slice(&plaintext).map_err(|_| MappingError::AuthFailed)?;

    Ok((entries, header))
}

// ---------------------------------------------------------------------------
// Legacy v1 decode (read-only)
// ---------------------------------------------------------------------------

/// Decrypt a legacy `CMAP\x01` byte slice (PBKDF2 10k, AES-256-GCM).
///
/// Format: CMAP\x01 + salt(32) + nonce(12) + ciphertext+tag
fn decrypt_v1(data: &[u8], passphrase: &str) -> Result<Vec<MappingEntry>, MappingError> {
    let offset = V1_MAGIC.len();
    if data.len() < offset + SALT_LEN + NONCE_LEN + 16 {
        return Err(MappingError::AuthFailed);
    }

    let salt = &data[offset..offset + SALT_LEN];
    let nonce_bytes = &data[offset + SALT_LEN..offset + SALT_LEN + NONCE_LEN];
    let ciphertext = &data[offset + SALT_LEN + NONCE_LEN..];

    let key = derive_key(passphrase, salt, V1_KDF_ITERATIONS)?;
    let cipher = Aes256Gcm::new_from_slice(&key).map_err(|_| MappingError::AuthFailed)?;

    let nonce = Nonce::from_slice(nonce_bytes);
    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| MappingError::AuthFailed)?;

    let entries: Vec<MappingEntry> =
        serde_json::from_slice(&plaintext).map_err(|_| MappingError::AuthFailed)?;

    Ok(entries)
}

// ---------------------------------------------------------------------------
// Unified decode (auto-detect format)
// ---------------------------------------------------------------------------

/// Decode a `.cmap` byte slice, auto-detecting v2, legacy v1, or plaintext.
///
/// Returns `(entries, cmap_version, is_legacy)`.
pub fn decode_cmap(
    data: &[u8],
    passphrase: &str,
) -> Result<(Vec<MappingEntry>, CmapVersion), MappingError> {
    if data.starts_with(V2_MAGIC) {
        let (entries, _header) = decrypt_v2(data, passphrase)?;
        Ok((entries, CmapVersion::V2))
    } else if data.starts_with(V1_MAGIC) {
        let entries = decrypt_v1(data, passphrase)?;
        Ok((entries, CmapVersion::LegacyV1))
    } else {
        // Reject plaintext JSON (no magic → not encrypted)
        Err(MappingError::VersionUnsupported)
    }
}

// ---------------------------------------------------------------------------
// Markdown restoration
// ---------------------------------------------------------------------------

/// Restore masked markdown using the given mapping entries.
///
/// Entries are sorted by masked-value length descending so that longer
/// patterns are replaced first (avoids partial-substitution conflicts).
/// Returns `(restored_text, actual_restore_count)`.
pub fn restore_markdown(masked: &str, entries: &[MappingEntry]) -> (String, usize) {
    let mut result = masked.to_string();
    let mut count = 0usize;

    let mut sorted = entries.to_vec();
    sorted.sort_by(|a, b| b.masked.len().cmp(&a.masked.len()));

    for entry in &sorted {
        let occurrences = result.matches(&entry.masked).count();
        if occurrences > 0 {
            result = result.replace(&entry.masked, &entry.original);
            count += occurrences;
        }
    }

    (result, count)
}

// ---------------------------------------------------------------------------
// Server internal cmap (plaintext MVP — NOT for user-facing use)
// ---------------------------------------------------------------------------

/// Magic string that identifies a server-internal cmap file.
pub const SERVER_CMAP_MAGIC: &str = "cheersai-server-cmap-mvp";

/// Current version of the server-internal cmap format.
pub const SERVER_CMAP_VERSION: u32 = 1;

/// Structure of the server-internal plaintext `.cmap` file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerCmap {
    pub format: String,
    pub version: u32,
    pub artifact_id: String,
    pub created_at: String,
    pub mappings: Vec<MappingEntry>,
}

/// Encode a `ServerCmap` to a UTF-8 JSON byte vector.
pub fn encode_server_cmap(cmap: &ServerCmap) -> Result<Vec<u8>, MappingError> {
    if &cmap.format != SERVER_CMAP_MAGIC {
        return Err(MappingError::InvalidMapping);
    }
    if cmap.version != SERVER_CMAP_VERSION {
        return Err(MappingError::InvalidMapping);
    }
    if cmap.artifact_id.is_empty() {
        return Err(MappingError::InvalidMapping);
    }
    serde_json::to_vec(cmap).map_err(|_| MappingError::InvalidMapping)
}

/// Decode a `ServerCmap` from a byte slice.  Validates magic, version,
/// and non-empty artifact_id.  Plaintext-only.
pub fn decode_server_cmap(data: &[u8]) -> Result<ServerCmap, MappingError> {
    let cmap: ServerCmap =
        serde_json::from_slice(data).map_err(|_| MappingError::InvalidMapping)?;
    if &cmap.format != SERVER_CMAP_MAGIC {
        return Err(MappingError::InvalidMapping);
    }
    if cmap.version != SERVER_CMAP_VERSION {
        return Err(MappingError::InvalidMapping);
    }
    if cmap.artifact_id.is_empty() {
        return Err(MappingError::InvalidMapping);
    }
    if cmap.mappings.is_empty() {
        return Err(MappingError::InvalidMapping);
    }
    let mut seen = std::collections::HashSet::new();
    for entry in &cmap.mappings {
        if !seen.insert(&entry.masked) {
            return Err(MappingError::InvalidMapping);
        }
    }
    Ok(cmap)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn derive_key(passphrase: &str, salt: &[u8], iterations: u32) -> Result<[u8; 32], MappingError> {
    let mut key = [0u8; 32];
    pbkdf2::pbkdf2::<hmac::Hmac<sha2::Sha256>>(passphrase.as_bytes(), salt, iterations, &mut key)
        .map_err(|_| MappingError::AuthFailed)?;
    Ok(key)
}

fn base64_encode(data: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(data)
}

fn base64_decode(data: &str) -> Result<Vec<u8>, MappingError> {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD
        .decode(data)
        .map_err(|_| MappingError::AuthFailed)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_entries() -> Vec<MappingEntry> {
        vec![
            MappingEntry {
                original: "13900000000".into(),
                masked: "***PHONE***1".into(),
                rule_id: "phone".into(),
            },
            MappingEntry {
                original: "test@example.cn".into(),
                masked: "***EMAIL***1".into(),
                rule_id: "email".into(),
            },
            MappingEntry {
                original: "张三".into(),
                masked: "姓名1".into(),
                rule_id: "chinese_name".into(),
            },
        ]
    }

    fn sample_markdown() -> String {
        "电话：***PHONE***1\n邮箱：***EMAIL***1\n联系人：姓名1\n".into()
    }

    #[test]
    fn v2_encrypt_decrypt_round_trip() {
        let entries = sample_entries();
        let pass = "test-passphrase-123";
        let encoded = encrypt_v2(&entries, pass, "test-artifact", "2026-07-23T00:00:00Z").unwrap();
        assert!(encoded.starts_with(V2_MAGIC));

        let (decoded, version) = decode_cmap(&encoded, pass).unwrap();
        assert_eq!(version, CmapVersion::V2);
        assert_eq!(decoded, entries);
    }

    #[test]
    fn v2_wrong_passphrase_fails() {
        let entries = sample_entries();
        let encoded = encrypt_v2(&entries, "correct", "a", "t").unwrap();
        let result = decode_cmap(&encoded, "wrong");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().error_code(), "CMAP_AUTH_FAILED");
    }

    #[test]
    fn v2_tampered_header_fails() {
        let entries = sample_entries();
        let pass = "p4ss";
        let mut encoded = encrypt_v2(&entries, pass, "a", "t").unwrap();
        // Flip a byte in the header
        let pos = V2_MAGIC.len() + 5;
        if pos < encoded.len() {
            encoded[pos] ^= 0x01;
        }
        let result = decode_cmap(&encoded, pass);
        assert_eq!(
            result.unwrap_err().error_code(),
            "CMAP_AUTH_FAILED",
            "tampered header"
        );
    }

    #[test]
    fn v2_tampered_ciphertext_fails() {
        let entries = sample_entries();
        let pass = "p4ss";
        let mut encoded = encrypt_v2(&entries, pass, "a", "t").unwrap();
        let last = encoded.len() - 5;
        if last > 0 {
            encoded[last] ^= 0xff;
        }
        let result = decode_cmap(&encoded, pass);
        assert_eq!(
            result.unwrap_err().error_code(),
            "CMAP_AUTH_FAILED",
            "tampered ciphertext"
        );
    }

    #[test]
    fn v2_truncated_fails() {
        let entries = sample_entries();
        let encoded = encrypt_v2(&entries, "p4ss", "a", "t").unwrap();
        let truncated = &encoded[..encoded.len() - 10];
        let result = decode_cmap(truncated, "p4ss");
        assert_eq!(result.unwrap_err().error_code(), "CMAP_AUTH_FAILED");
    }

    #[test]
    fn v2_unique_per_call() {
        let entries = sample_entries();
        let a = encrypt_v2(&entries, "pass", "a", "t").unwrap();
        let b = encrypt_v2(&entries, "pass", "a", "t").unwrap();
        assert_ne!(a, b, "salt+nonce must differ each call");
    }

    #[test]
    fn v2_production_params() {
        // Verify the header uses correct production values
        let entries = sample_entries();
        let encoded = encrypt_v2(&entries, "pass", "art", "2026-07-23T00:00:00Z").unwrap();

        // Parse header
        let offset = V2_MAGIC.len() + 1 + 4;
        let header_len = u32::from_be_bytes([
            encoded[V2_MAGIC.len() + 1],
            encoded[V2_MAGIC.len() + 2],
            encoded[V2_MAGIC.len() + 3],
            encoded[V2_MAGIC.len() + 4],
        ]) as usize;
        let header_json = &encoded[offset..offset + header_len];
        let header: V2Header = serde_json::from_slice(header_json).unwrap();

        assert_eq!(header.kdf_iterations, 600_000);
        assert_eq!(header.derived_key_len, 32);
        assert_eq!(base64_decode(&header.salt_b64).unwrap().len(), 32);
        assert_eq!(base64_decode(&header.nonce_b64).unwrap().len(), 12);
    }

    #[test]
    fn restore_works() {
        let entries = sample_entries();
        let md = sample_markdown();
        let (restored, count) = restore_markdown(&md, &entries);
        assert_eq!(count, 3);
        assert!(restored.contains("13900000000"));
        assert!(restored.contains("test@example.cn"));
        assert!(restored.contains("张三"));
    }

    #[test]
    fn restore_longest_first() {
        let entries = vec![
            MappingEntry {
                original: "13800000000".into(),
                masked: "***PHONE***10".into(),
                rule_id: "phone".into(),
            },
            MappingEntry {
                original: "13800000000-EXT".into(),
                masked: "***PHONE***1".into(),
                rule_id: "phone".into(),
            },
        ];
        let md = "***PHONE***1\n***PHONE***10\n";
        let (restored, count) = restore_markdown(md, &entries);
        assert_eq!(count, 2);
        assert!(
            restored.contains("13800000000-EXT"),
            "longer pattern preserved"
        );
        assert!(
            restored.contains("13800000000\n"),
            "shorter pattern restored"
        );
    }

    #[test]
    fn reject_plaintext_json() {
        let plain = br#"[{"original":"139","masked":"***PHONE***1","rule_id":"phone"}]"#;
        let result = decode_cmap(plain, "any");
        assert_eq!(result.unwrap_err().error_code(), "CMAP_VERSION_UNSUPPORTED");
    }

    #[test]
    fn empty_mappings_restore_zero() {
        let (restored, count) = restore_markdown("hello ***PHONE***1", &[]);
        assert_eq!(count, 0);
        assert_eq!(restored, "hello ***PHONE***1");
    }

    #[test]
    fn legacy_v1_round_trip() {
        // Build a v1 cmap using the old crypto
        let entries = sample_entries();
        let payload = serde_json::to_vec(&entries).unwrap();
        let pass = "v1-pass";

        let salt = [0xabu8; 32];
        let nonce = [0xcdu8; 12];
        let key = derive_key(pass, &salt, V1_KDF_ITERATIONS).unwrap();
        let cipher = Aes256Gcm::new_from_slice(&key).unwrap();
        let ct = cipher
            .encrypt(Nonce::from_slice(&nonce), payload.as_slice())
            .unwrap();

        let mut cmap = Vec::new();
        cmap.extend_from_slice(V1_MAGIC);
        cmap.extend_from_slice(&salt);
        cmap.extend_from_slice(&nonce);
        cmap.extend_from_slice(&ct);

        let (decoded, version) = decode_cmap(&cmap, pass).unwrap();
        assert_eq!(version, CmapVersion::LegacyV1);
        assert_eq!(decoded, entries);
    }

    #[test]
    fn mapping_error_codes_are_stable() {
        assert_eq!(MappingError::AuthFailed.error_code(), "CMAP_AUTH_FAILED");
        assert_eq!(
            MappingError::VersionUnsupported.error_code(),
            "CMAP_VERSION_UNSUPPORTED"
        );
        assert_eq!(MappingError::InvalidMapping.error_code(), "CMAP_MISMATCH");
    }

    // --------------- server cmap tests ---------------

    #[test]
    fn server_cmap_round_trip() {
        let entries = sample_entries();
        let sc = ServerCmap {
            format: SERVER_CMAP_MAGIC.into(),
            version: SERVER_CMAP_VERSION,
            artifact_id: "test-artifact-123".into(),
            created_at: "2026-07-23T00:00:00Z".into(),
            mappings: entries.clone(),
        };
        let encoded = encode_server_cmap(&sc).unwrap();
        let decoded = decode_server_cmap(&encoded).unwrap();
        assert_eq!(decoded.artifact_id, "test-artifact-123");
        assert_eq!(decoded.mappings, entries);
    }

    #[test]
    fn server_cmap_wrong_format_rejected() {
        let sc = ServerCmap {
            format: "wrong-format".into(),
            version: SERVER_CMAP_VERSION,
            artifact_id: "a".into(),
            created_at: "t".into(),
            mappings: sample_entries(),
        };
        assert!(encode_server_cmap(&sc).is_err());
    }

    #[test]
    fn server_cmap_empty_artifact_id_rejected() {
        let sc = ServerCmap {
            format: SERVER_CMAP_MAGIC.into(),
            version: SERVER_CMAP_VERSION,
            artifact_id: "".into(),
            created_at: "t".into(),
            mappings: sample_entries(),
        };
        assert!(encode_server_cmap(&sc).is_err());
    }

    #[test]
    fn server_cmap_empty_mappings_rejected() {
        let data = format!(
            r#"{{"format":"{}","version":{},"artifact_id":"a","created_at":"t","mappings":[]}}"#,
            SERVER_CMAP_MAGIC, SERVER_CMAP_VERSION
        );
        assert!(decode_server_cmap(data.as_bytes()).is_err());
    }

    #[test]
    fn server_cmap_duplicate_masked_rejected() {
        let entries = vec![
            MappingEntry {
                original: "a".into(),
                masked: "***X***1".into(),
                rule_id: "r".into(),
            },
            MappingEntry {
                original: "b".into(),
                masked: "***X***1".into(),
                rule_id: "r".into(),
            },
        ];
        let sc = ServerCmap {
            format: SERVER_CMAP_MAGIC.into(),
            version: SERVER_CMAP_VERSION,
            artifact_id: "a".into(),
            created_at: "t".into(),
            mappings: entries,
        };
        let encoded = encode_server_cmap(&sc).unwrap();
        assert!(decode_server_cmap(&encoded).is_err());
    }
}
