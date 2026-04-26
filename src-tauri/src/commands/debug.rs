use std::env;
use std::path::PathBuf;

#[tauri::command]
pub async fn get_working_directory() -> Result<String, String> {
    match env::current_dir() {
        Ok(dir) => Ok(dir.to_string_lossy().to_string()),
        Err(e) => Err(format!("Failed to get working directory: {}", e)),
    }
}

#[tauri::command]
pub async fn check_file_exists(path: String) -> Result<bool, String> {
    let file_path = PathBuf::from(&path);
    println!("Checking file existence: {}", path);
    println!("Absolute path: {}", file_path.canonicalize().unwrap_or(file_path.clone()).display());
    Ok(file_path.exists())
}