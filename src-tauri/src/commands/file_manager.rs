use crate::core::database::Database;
use crate::core::file_manager::ManagedFile;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Serialize, Deserialize)]
pub struct FileListResponse {
    pub files: Vec<ManagedFile>,
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
}

/// 添加文件到管理列表
#[tauri::command]
pub async fn add_managed_file(
    original_name: String,
    masked_name: String,
    file_path: String,
    file_size: i64,
    file_type: String,
    masked_count: i64,
) -> Result<String, String> {
    let db = Database::new().await.map_err(|e| e.to_string())?;
    
    let file = ManagedFile::new(
        original_name,
        masked_name,
        file_path,
        file_size,
        file_type,
        masked_count,
    );
    
    db.add_managed_file(&file).await.map_err(|e| e.to_string())?;
    
    Ok(file.id)
}

/// 获取文件列表
#[tauri::command]
pub async fn get_managed_files(
    page: i64,
    page_size: i64,
) -> Result<FileListResponse, String> {
    let db = Database::new().await.map_err(|e| e.to_string())?;
    
    let offset = (page - 1) * page_size;
    let files = db.get_managed_files(page_size, offset).await.map_err(|e| e.to_string())?;
    let total = db.get_managed_files_count().await.map_err(|e| e.to_string())?;
    
    Ok(FileListResponse {
        files,
        total,
        page,
        page_size,
    })
}

/// 获取单个文件信息
#[tauri::command]
pub async fn get_managed_file(id: String) -> Result<Option<ManagedFile>, String> {
    let db = Database::new().await.map_err(|e| e.to_string())?;
    db.get_managed_file_by_id(&id).await.map_err(|e| e.to_string())
}

/// 更新文件信息
#[tauri::command]
pub async fn update_managed_file(file: ManagedFile) -> Result<String, String> {
    let db = Database::new().await.map_err(|e| e.to_string())?;
    db.update_managed_file(&file).await.map_err(|e| e.to_string())?;
    Ok("文件信息已更新".to_string())
}

/// 删除文件
#[tauri::command]
pub async fn delete_managed_file(id: String, delete_physical: bool) -> Result<String, String> {
    let db = Database::new().await.map_err(|e| e.to_string())?;
    
    // 如果需要删除物理文件，先获取文件路径
    if delete_physical {
        if let Some(file) = db.get_managed_file_by_id(&id).await.map_err(|e| e.to_string())? {
            let path = Path::new(&file.file_path);
            if path.exists() {
                std::fs::remove_file(path).map_err(|e| format!("删除文件失败: {}", e))?;
            }
        }
    }
    
    // 删除数据库记录
    db.delete_managed_file(&id).await.map_err(|e| e.to_string())?;
    
    Ok("文件已删除".to_string())
}

/// 批量删除文件
#[tauri::command]
pub async fn delete_managed_files(ids: Vec<String>, delete_physical: bool) -> Result<String, String> {
    let db = Database::new().await.map_err(|e| e.to_string())?;
    
    // 如果需要删除物理文件
    if delete_physical {
        for id in &ids {
            if let Some(file) = db.get_managed_file_by_id(id).await.map_err(|e| e.to_string())? {
                let path = Path::new(&file.file_path);
                if path.exists() {
                    if let Err(e) = std::fs::remove_file(path) {
                        eprintln!("Failed to delete file {}: {}", file.file_path, e);
                    }
                }
            }
        }
    }
    
    // 删除数据库记录
    db.delete_managed_files(&ids).await.map_err(|e| e.to_string())?;
    
    Ok(format!("已删除 {} 个文件", ids.len()))
}

/// 标记文件已上传到 Gitea
#[tauri::command]
pub async fn mark_file_uploaded(id: String, gitea_url: String) -> Result<String, String> {
    let db = Database::new().await.map_err(|e| e.to_string())?;
    db.mark_file_uploaded_to_gitea(&id, &gitea_url).await.map_err(|e| e.to_string())?;
    Ok("已标记为已上传".to_string())
}

/// 搜索文件
#[tauri::command]
pub async fn search_managed_files(query: String, limit: i64) -> Result<Vec<ManagedFile>, String> {
    let db = Database::new().await.map_err(|e| e.to_string())?;
    db.search_managed_files(&query, limit).await.map_err(|e| e.to_string())
}

/// 获取文件统计信息
#[tauri::command]
pub async fn get_file_statistics() -> Result<serde_json::Value, String> {
    let db = Database::new().await.map_err(|e| e.to_string())?;
    
    let total = db.get_managed_files_count().await.map_err(|e| e.to_string())?;
    
    // 获取已上传到 Gitea 的文件数
    let uploaded: (i64,) = sqlx::query_as::<_, (i64,)>(
        "SELECT COUNT(*) FROM managed_files WHERE gitea_uploaded = 1"
    )
    .fetch_one(&db.pool)
    .await
    .map_err(|e: sqlx::Error| e.to_string())?;
    
    // 获取总文件大小
    let total_size: (Option<i64>,) = sqlx::query_as::<_, (Option<i64>,)>(
        "SELECT SUM(file_size) FROM managed_files"
    )
    .fetch_one(&db.pool)
    .await
    .map_err(|e: sqlx::Error| e.to_string())?;
    
    Ok(serde_json::json!({
        "total_files": total,
        "uploaded_to_gitea": uploaded.0,
        "total_size": total_size.0.unwrap_or(0),
    }))
}
