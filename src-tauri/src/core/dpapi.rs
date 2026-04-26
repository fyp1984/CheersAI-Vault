/// Windows DPAPI 加密模块
/// 使用 Windows Data Protection API 将数据绑定到当前 Windows 用户账户
/// 只有登录的同一用户才能解密数据

use std::fs;
use std::path::PathBuf;

/// 获取 PIN 存储文件路径
fn get_pin_file_path() -> Result<PathBuf, String> {
    let temp_dir = std::env::temp_dir();
    let config_dir = temp_dir.join("cheersai-vault");
    fs::create_dir_all(&config_dir).map_err(|e| format!("创建配置目录失败: {}", e))?;
    Ok(config_dir.join("pin.dpapi"))
}

/// 检查是否已设置 PIN
pub fn has_pin() -> bool {
    match get_pin_file_path() {
        Ok(path) => path.exists(),
        Err(_) => false,
    }
}

/// 使用 DPAPI 加密数据并保存到文件
pub fn save_pin(pin: &str) -> Result<(), String> {
    let encrypted = dpapi_encrypt(pin.as_bytes())?;
    let path = get_pin_file_path()?;
    fs::write(&path, &encrypted).map_err(|e| format!("保存 PIN 失败: {}", e))?;
    Ok(())
}

/// 从文件加载并使用 DPAPI 解密，验证 PIN
pub fn verify_pin(pin: &str) -> Result<bool, String> {
    let path = get_pin_file_path()?;
    if !path.exists() {
        return Err("尚未设置 PIN".to_string());
    }

    let encrypted = fs::read(&path).map_err(|e| format!("读取 PIN 文件失败: {}", e))?;
    let decrypted = dpapi_decrypt(&encrypted)?;
    let stored_pin = String::from_utf8(decrypted)
        .map_err(|_| "PIN 数据损坏".to_string())?;

    Ok(stored_pin == pin)
}

/// 清除已保存的 PIN
pub fn clear_pin() -> Result<(), String> {
    let path = get_pin_file_path()?;
    if path.exists() {
        fs::remove_file(&path).map_err(|e| format!("清除 PIN 失败: {}", e))?;
    }
    Ok(())
}

// ============ Windows DPAPI 实现 ============

#[cfg(target_os = "windows")]
fn dpapi_encrypt(data: &[u8]) -> Result<Vec<u8>, String> {
    use windows_sys::Win32::Security::Cryptography::{
        CryptProtectData, CRYPT_INTEGER_BLOB,
    };

    let mut input_blob = CRYPT_INTEGER_BLOB {
        cbData: data.len() as u32,
        pbData: data.as_ptr() as *mut u8,
    };

    let mut output_blob = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: std::ptr::null_mut(),
    };

    let description: Vec<u16> = "CheersAI Vault PIN\0".encode_utf16().collect();

    let result = unsafe {
        CryptProtectData(
            &mut input_blob,
            description.as_ptr(),
            std::ptr::null_mut(), // optional entropy
            std::ptr::null_mut(), // reserved
            std::ptr::null_mut(), // prompt struct
            0,                    // flags
            &mut output_blob,
        )
    };

    if result == 0 {
        return Err("DPAPI 加密失败".to_string());
    }

    let encrypted = unsafe {
        std::slice::from_raw_parts(output_blob.pbData, output_blob.cbData as usize).to_vec()
    };

    // 释放 DPAPI 分配的内存
    unsafe {
        windows_sys::Win32::Foundation::LocalFree(output_blob.pbData as *mut std::ffi::c_void);
    }

    Ok(encrypted)
}

#[cfg(target_os = "windows")]
fn dpapi_decrypt(data: &[u8]) -> Result<Vec<u8>, String> {
    use windows_sys::Win32::Security::Cryptography::{
        CryptUnprotectData, CRYPT_INTEGER_BLOB,
    };

    let mut input_blob = CRYPT_INTEGER_BLOB {
        cbData: data.len() as u32,
        pbData: data.as_ptr() as *mut u8,
    };

    let mut output_blob = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: std::ptr::null_mut(),
    };

    let result = unsafe {
        CryptUnprotectData(
            &mut input_blob,
            std::ptr::null_mut(), // description out
            std::ptr::null_mut(), // optional entropy
            std::ptr::null_mut(), // reserved
            std::ptr::null_mut(), // prompt struct
            0,                    // flags
            &mut output_blob,
        )
    };

    if result == 0 {
        return Err("DPAPI 解密失败，可能不是同一用户加密的数据".to_string());
    }

    let decrypted = unsafe {
        std::slice::from_raw_parts(output_blob.pbData, output_blob.cbData as usize).to_vec()
    };

    // 释放 DPAPI 分配的内存
    unsafe {
        windows_sys::Win32::Foundation::LocalFree(output_blob.pbData as *mut std::ffi::c_void);
    }

    Ok(decrypted)
}

// ============ 非 Windows 平台的 fallback ============

#[cfg(not(target_os = "windows"))]
fn dpapi_encrypt(data: &[u8]) -> Result<Vec<u8>, String> {
    // 非 Windows 平台使用简单的 base64 编码（仅作 fallback）
    Ok(base64::Engine::encode(&base64::engine::general_purpose::STANDARD, data).into_bytes())
}

#[cfg(not(target_os = "windows"))]
fn dpapi_decrypt(data: &[u8]) -> Result<Vec<u8>, String> {
    let encoded = String::from_utf8(data.to_vec())
        .map_err(|_| "数据格式错误".to_string())?;
    base64::Engine::decode(&base64::engine::general_purpose::STANDARD, &encoded)
        .map_err(|_| "解密失败".to_string())
}
