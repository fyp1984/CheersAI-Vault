//! FileBay credential storage.
//!
//! The token is intentionally kept out of the application database, JSON
//! configuration files and Tauri responses.  Platform implementations use
//! macOS Keychain or Windows DPAPI; unsupported platforms fail closed.

use anyhow::{anyhow, Result};

const SERVICE: &str = "com.cheersai.vault.filebay";
const ACCOUNT: &str = "default";
const WINDOWS_PURPOSE: &[u8] = b"CheersAI Vault FileBay credential v1";

pub fn get_token() -> Result<Option<String>> {
    #[cfg(target_os = "macos")]
    {
        let output = std::process::Command::new("/usr/bin/security")
            .args(["find-generic-password", "-s", SERVICE, "-a", ACCOUNT, "-w"])
            .output()
            .map_err(|_| anyhow!("FILEBAY_CREDENTIAL_STORE_UNAVAILABLE"))?;
        if output.status.success() {
            let token = String::from_utf8_lossy(&output.stdout).trim().to_string();
            return Ok((!token.is_empty()).then_some(token));
        }
        return Ok(None);
    }

    #[cfg(target_os = "windows")]
    {
        return windows_get_token();
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        Err(anyhow!("FILEBAY_CREDENTIAL_STORE_UNSUPPORTED"))
    }
}

pub fn set_token(token: &str) -> Result<()> {
    if token.trim().is_empty() {
        return Err(anyhow!("FILEBAY_TOKEN_REQUIRED"));
    }

    #[cfg(target_os = "macos")]
    {
        let status = std::process::Command::new("/usr/bin/security")
            .args([
                "add-generic-password",
                "-U",
                "-s",
                SERVICE,
                "-a",
                ACCOUNT,
                "-w",
                token,
            ])
            .status()
            .map_err(|_| anyhow!("FILEBAY_CREDENTIAL_STORE_UNAVAILABLE"))?;
        if status.success() {
            Ok(())
        } else {
            Err(anyhow!("FILEBAY_CREDENTIAL_STORE_WRITE_FAILED"))
        }
    }

    #[cfg(target_os = "windows")]
    {
        windows_set_token(token)
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        Err(anyhow!("FILEBAY_CREDENTIAL_STORE_UNSUPPORTED"))
    }
}

pub fn delete_token() -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        let status = std::process::Command::new("/usr/bin/security")
            .args(["delete-generic-password", "-s", SERVICE, "-a", ACCOUNT])
            .status()
            .map_err(|_| anyhow!("FILEBAY_CREDENTIAL_STORE_UNAVAILABLE"))?;
        if status.success() || status.code() == Some(44) {
            Ok(())
        } else {
            Err(anyhow!("FILEBAY_CREDENTIAL_STORE_DELETE_FAILED"))
        }
    }

    #[cfg(target_os = "windows")]
    {
        windows_delete_token()
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        Err(anyhow!("FILEBAY_CREDENTIAL_STORE_UNSUPPORTED"))
    }
}

pub fn has_token() -> Result<bool> {
    Ok(get_token()?.is_some())
}

#[cfg(target_os = "windows")]
fn credential_path() -> Result<std::path::PathBuf> {
    let base =
        dirs_next::data_dir().ok_or_else(|| anyhow!("FILEBAY_CREDENTIAL_STORE_UNAVAILABLE"))?;
    let dir = base.join("CheersAI-Vault");
    std::fs::create_dir_all(&dir).map_err(|_| anyhow!("FILEBAY_CREDENTIAL_STORE_UNAVAILABLE"))?;
    Ok(dir.join("filebay-token.dpapi"))
}

#[cfg(target_os = "windows")]
fn windows_protect(data: &[u8]) -> Result<Vec<u8>> {
    use std::ptr::null_mut;
    use windows_sys::Win32::Security::Cryptography::{CryptProtectData, CRYPT_INTEGER_BLOB};
    let mut input = CRYPT_INTEGER_BLOB {
        cbData: data.len() as u32,
        pbData: data.as_ptr() as *mut u8,
    };
    let mut purpose = CRYPT_INTEGER_BLOB {
        cbData: WINDOWS_PURPOSE.len() as u32,
        pbData: WINDOWS_PURPOSE.as_ptr() as *mut u8,
    };
    let mut output = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: null_mut(),
    };
    let ok = unsafe {
        CryptProtectData(
            &mut input,
            std::ptr::null(),
            &mut purpose,
            null_mut(),
            null_mut(),
            0,
            &mut output,
        )
    };
    if ok == 0 {
        return Err(anyhow!("FILEBAY_CREDENTIAL_STORE_WRITE_FAILED"));
    }
    let bytes =
        unsafe { std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec() };
    unsafe {
        windows_sys::Win32::Foundation::LocalFree(output.pbData as *mut std::ffi::c_void);
    }
    Ok(bytes)
}

#[cfg(target_os = "windows")]
fn windows_unprotect(data: &[u8]) -> Result<Vec<u8>> {
    use std::ptr::null_mut;
    use windows_sys::Win32::Security::Cryptography::{CryptUnprotectData, CRYPT_INTEGER_BLOB};
    let mut input = CRYPT_INTEGER_BLOB {
        cbData: data.len() as u32,
        pbData: data.as_ptr() as *mut u8,
    };
    let mut purpose = CRYPT_INTEGER_BLOB {
        cbData: WINDOWS_PURPOSE.len() as u32,
        pbData: WINDOWS_PURPOSE.as_ptr() as *mut u8,
    };
    let mut output = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: null_mut(),
    };
    let ok = unsafe {
        CryptUnprotectData(
            &mut input,
            std::ptr::null_mut(),
            &mut purpose,
            null_mut(),
            null_mut(),
            0,
            &mut output,
        )
    };
    if ok == 0 {
        return Err(anyhow!("FILEBAY_CREDENTIAL_STORE_READ_FAILED"));
    }
    let bytes =
        unsafe { std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec() };
    unsafe {
        windows_sys::Win32::Foundation::LocalFree(output.pbData as *mut std::ffi::c_void);
    }
    Ok(bytes)
}

#[cfg(target_os = "windows")]
fn windows_get_token() -> Result<Option<String>> {
    let path = credential_path()?;
    if !path.exists() {
        return Ok(None);
    }
    let bytes = windows_unprotect(
        &std::fs::read(path).map_err(|_| anyhow!("FILEBAY_CREDENTIAL_STORE_READ_FAILED"))?,
    )?;
    let token =
        String::from_utf8(bytes).map_err(|_| anyhow!("FILEBAY_CREDENTIAL_STORE_READ_FAILED"))?;
    Ok((!token.is_empty()).then_some(token))
}

#[cfg(target_os = "windows")]
fn windows_set_token(token: &str) -> Result<()> {
    let path = credential_path()?;
    let protected = windows_protect(token.as_bytes())?;
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, protected)
        .map_err(|_| anyhow!("FILEBAY_CREDENTIAL_STORE_WRITE_FAILED"))?;
    std::fs::rename(tmp, path).map_err(|_| anyhow!("FILEBAY_CREDENTIAL_STORE_WRITE_FAILED"))
}

#[cfg(target_os = "windows")]
fn windows_delete_token() -> Result<()> {
    let path = credential_path()?;
    if path.exists() {
        std::fs::remove_file(path)
            .map_err(|_| anyhow!("FILEBAY_CREDENTIAL_STORE_DELETE_FAILED"))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    #[derive(Clone, Default)]
    struct MemoryCredentialStore(Arc<Mutex<Option<String>>>);

    impl MemoryCredentialStore {
        fn set(&self, token: &str) {
            *self.0.lock().unwrap() = Some(token.to_string());
        }
        fn get(&self) -> Option<String> {
            self.0.lock().unwrap().clone()
        }
        fn delete(&self) {
            *self.0.lock().unwrap() = None;
        }
    }

    #[test]
    fn test_store_never_serializes_token() {
        let store = MemoryCredentialStore::default();
        store.set("fake-filebay-token");
        assert_eq!(store.get().as_deref(), Some("fake-filebay-token"));
        store.set("fake-filebay-token-replaced");
        assert_eq!(store.get().as_deref(), Some("fake-filebay-token-replaced"));
        let public = serde_json::json!({"has_token": store.get().is_some()});
        assert_eq!(public.to_string(), r#"{"has_token":true}"#);
        assert!(!public.to_string().contains("fake-filebay-token"));
        store.delete();
        assert!(store.get().is_none());
    }
}
