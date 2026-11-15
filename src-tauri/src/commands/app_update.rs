use serde::Serialize;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateBackupSummary {
    pub backup_path: String,
    pub source_path: String,
    pub copied_files: usize,
    pub created_at: String,
}

fn app_data_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map_err(|e| format!("无法获取应用数据目录: {e}"))
}

fn backup_root(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("update-backups")
}

fn should_skip(source: &Path, backup_root: &Path) -> bool {
    source == backup_root || source.starts_with(backup_root)
}

fn copy_tree(source: &Path, target: &Path, backup_root: &Path) -> Result<usize, String> {
    if should_skip(source, backup_root) || !source.exists() {
        return Ok(0);
    }

    if source.is_file() {
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("创建备份目录失败: {e}"))?;
        }
        std::fs::copy(source, target)
            .map_err(|e| format!("复制文件失败 {} -> {}: {e}", source.display(), target.display()))?;
        return Ok(1);
    }

    std::fs::create_dir_all(target)
        .map_err(|e| format!("创建目录失败 {}: {e}", target.display()))?;

    let mut copied_files = 0usize;
    for entry in std::fs::read_dir(source)
        .map_err(|e| format!("读取目录失败 {}: {e}", source.display()))?
    {
        let entry = entry.map_err(|e| format!("读取目录项失败: {e}"))?;
        let child_source = entry.path();
        if should_skip(&child_source, backup_root) {
            continue;
        }
        let child_target = target.join(entry.file_name());
        copied_files += copy_tree(&child_source, &child_target, backup_root)?;
    }

    Ok(copied_files)
}

fn timestamp_string() -> String {
    chrono::Utc::now().format("%Y%m%dT%H%M%SZ").to_string()
}

#[tauri::command]
pub async fn prepare_update_backup(app: AppHandle) -> Result<UpdateBackupSummary, String> {
    let source_dir = app_data_dir(&app)?;
    std::fs::create_dir_all(&source_dir)
        .map_err(|e| format!("初始化应用数据目录失败: {e}"))?;

    let backup_root = backup_root(&source_dir);
    std::fs::create_dir_all(&backup_root)
        .map_err(|e| format!("初始化更新备份目录失败: {e}"))?;

    let backup_dir = backup_root.join(timestamp_string());
    let data_target = backup_dir.join("app-data");
    let copied_files = copy_tree(&source_dir, &data_target, &backup_root)?;

    let manifest = serde_json::json!({
        "createdAt": chrono::Utc::now().to_rfc3339(),
        "sourcePath": source_dir.to_string_lossy(),
        "backupPath": data_target.to_string_lossy(),
        "copiedFiles": copied_files
    });
    std::fs::write(
        backup_dir.join("manifest.json"),
        serde_json::to_vec_pretty(&manifest).map_err(|e| format!("序列化备份清单失败: {e}"))?,
    )
    .map_err(|e| format!("写入备份清单失败: {e}"))?;

    Ok(UpdateBackupSummary {
        backup_path: data_target.to_string_lossy().to_string(),
        source_path: source_dir.to_string_lossy().to_string(),
        copied_files,
        created_at: chrono::Utc::now().to_rfc3339(),
    })
}

#[tauri::command]
pub async fn restart_app(app: AppHandle) -> Result<(), String> {
    app.restart();
}

#[cfg(test)]
mod tests {
    use super::{backup_root, copy_tree};

    fn temp_path(label: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("cheersai-vault-{label}-{}", uuid::Uuid::new_v4()))
    }

    #[test]
    fn copy_tree_skips_existing_backup_root() {
        let source_dir = temp_path("update-backup-source");
        let target_dir = temp_path("update-backup-target");
        let backups_dir = backup_root(&source_dir);

        std::fs::create_dir_all(source_dir.join("nested")).unwrap();
        std::fs::create_dir_all(backups_dir.join("old")).unwrap();
        std::fs::write(source_dir.join("settings.json"), b"{}").unwrap();
        std::fs::write(source_dir.join("nested").join("db.sqlite"), b"sqlite").unwrap();
        std::fs::write(backups_dir.join("old").join("should-skip.txt"), b"skip").unwrap();

        let copied_files = copy_tree(&source_dir, &target_dir, &backups_dir).unwrap();

        assert_eq!(copied_files, 2);
        assert!(target_dir.join("settings.json").exists());
        assert!(target_dir.join("nested").join("db.sqlite").exists());
        assert!(!target_dir.join("update-backups").exists());

        let _ = std::fs::remove_dir_all(&source_dir);
        let _ = std::fs::remove_dir_all(&target_dir);
    }
}
