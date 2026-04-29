use serde::Serialize;
use std::path::PathBuf;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlatformContext {
    pub os: String,
    pub path_separator: String,
    pub default_documents_dir: String,
    pub app_data_dir: String,
    pub cache_dir: String,
    pub temp_dir: String,
    pub pin_storage_mode: String,
    pub ocr_strategy: String,
    pub ollama_strategy: String,
}

fn stringify_path(path: PathBuf) -> String {
    path.to_string_lossy().to_string()
}

fn default_documents_dir() -> PathBuf {
    dirs_next::document_dir()
        .or_else(dirs_next::home_dir)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("CheersAI Vault")
}

fn app_data_dir() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        dirs_next::data_dir()
            .unwrap_or_else(|| PathBuf::from(std::env::var("APPDATA").unwrap_or_else(|_| ".".to_string())))
            .join("CheersAI-Vault")
    }

    #[cfg(target_os = "macos")]
    {
        dirs_next::data_dir()
            .unwrap_or_else(|| PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".to_string())).join("Library/Application Support"))
            .join("CheersAI-Vault")
    }

    #[cfg(target_os = "linux")]
    {
        dirs_next::config_dir()
            .unwrap_or_else(|| PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".to_string())).join(".config"))
            .join("CheersAI-Vault")
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        PathBuf::from("./CheersAI-Vault")
    }
}

fn cache_dir() -> PathBuf {
    dirs_next::cache_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("CheersAI-Vault")
}

fn temp_dir() -> PathBuf {
    std::env::temp_dir().join("cheersai-vault")
}

#[tauri::command]
pub async fn get_platform_context() -> Result<PlatformContext, String> {
    #[cfg(target_os = "windows")]
    let os = "windows";
    #[cfg(target_os = "macos")]
    let os = "macos";
    #[cfg(target_os = "linux")]
    let os = "linux";
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    let os = "unknown";

    let pin_storage_mode = match os {
        "windows" => "windows_dpapi",
        "macos" => "macos_keychain",
        _ => "fallback_file",
    };

    let ocr_strategy = match os {
        "windows" => "embedded_python",
        "macos" => "system_python_venv",
        "linux" => "system_python",
        _ => "system_python",
    };

    let ollama_strategy = match os {
        "macos" => "binary_check_plus_app_launch",
        "windows" => "binary_check_plus_background_serve",
        _ => "binary_check_plus_cli_serve",
    };

    Ok(PlatformContext {
        os: os.to_string(),
        path_separator: if os == "windows" { "\\" } else { "/" }.to_string(),
        default_documents_dir: stringify_path(default_documents_dir()),
        app_data_dir: stringify_path(app_data_dir()),
        cache_dir: stringify_path(cache_dir()),
        temp_dir: stringify_path(temp_dir()),
        pin_storage_mode: pin_storage_mode.to_string(),
        ocr_strategy: ocr_strategy.to_string(),
        ollama_strategy: ollama_strategy.to_string(),
    })
}
