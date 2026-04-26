use anyhow::Result;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use super::database::Database;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ManagedFile {
    pub id: String,
    pub original_name: String,
    pub masked_name: String,
    pub file_path: String,
    pub file_size: i64,
    pub file_type: String,
    pub masked_count: i64,
    pub gitea_uploaded: bool,
    pub gitea_url: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl ManagedFile {
    pub fn new(
        original_name: String,
        masked_name: String,
        file_path: String,
        file_size: i64,
        file_type: String,
        masked_count: i64,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            original_name,
            masked_name,
            file_path,
            file_size,
            file_type,
            masked_count,
            gitea_uploaded: false,
            gitea_url: None,
            created_at: chrono::Utc::now().to_rfc3339(),
            updated_at: chrono::Utc::now().to_rfc3339(),
        }
    }
}

impl Database {
    /// 添加文件到管理列表
    pub async fn add_managed_file(&self, file: &ManagedFile) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO managed_files (
                id, original_name, masked_name, file_path, file_size, 
                file_type, masked_count, gitea_uploaded, gitea_url, 
                created_at, updated_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&file.id)
        .bind(&file.original_name)
        .bind(&file.masked_name)
        .bind(&file.file_path)
        .bind(file.file_size)
        .bind(&file.file_type)
        .bind(file.masked_count)
        .bind(file.gitea_uploaded)
        .bind(&file.gitea_url)
        .bind(&file.created_at)
        .bind(&file.updated_at)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// 获取所有管理的文件
    pub async fn get_managed_files(&self, limit: i64, offset: i64) -> Result<Vec<ManagedFile>> {
        let files = sqlx::query_as::<_, ManagedFile>(
            r#"
            SELECT id, original_name, masked_name, file_path, file_size, 
                   file_type, masked_count, gitea_uploaded, gitea_url, 
                   created_at, updated_at
            FROM managed_files
            ORDER BY created_at DESC
            LIMIT ? OFFSET ?
            "#,
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        Ok(files)
    }

    /// 获取文件总数
    pub async fn get_managed_files_count(&self) -> Result<i64> {
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM managed_files")
            .fetch_one(&self.pool)
            .await?;

        Ok(count.0)
    }

    /// 根据 ID 获取文件
    pub async fn get_managed_file_by_id(&self, id: &str) -> Result<Option<ManagedFile>> {
        let file = sqlx::query_as::<_, ManagedFile>(
            r#"
            SELECT id, original_name, masked_name, file_path, file_size, 
                   file_type, masked_count, gitea_uploaded, gitea_url, 
                   created_at, updated_at
            FROM managed_files
            WHERE id = ?
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(file)
    }

    /// 更新文件信息
    pub async fn update_managed_file(&self, file: &ManagedFile) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE managed_files
            SET original_name = ?, masked_name = ?, file_path = ?, 
                file_size = ?, file_type = ?, masked_count = ?, 
                gitea_uploaded = ?, gitea_url = ?, updated_at = ?
            WHERE id = ?
            "#,
        )
        .bind(&file.original_name)
        .bind(&file.masked_name)
        .bind(&file.file_path)
        .bind(file.file_size)
        .bind(&file.file_type)
        .bind(file.masked_count)
        .bind(file.gitea_uploaded)
        .bind(&file.gitea_url)
        .bind(chrono::Utc::now().to_rfc3339())
        .bind(&file.id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// 标记文件已上传到 Gitea
    pub async fn mark_file_uploaded_to_gitea(&self, id: &str, gitea_url: &str) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE managed_files
            SET gitea_uploaded = 1, gitea_url = ?, updated_at = ?
            WHERE id = ?
            "#,
        )
        .bind(gitea_url)
        .bind(chrono::Utc::now().to_rfc3339())
        .bind(id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// 删除文件记录
    pub async fn delete_managed_file(&self, id: &str) -> Result<()> {
        sqlx::query("DELETE FROM managed_files WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    /// 批量删除文件记录
    pub async fn delete_managed_files(&self, ids: &[String]) -> Result<()> {
        for id in ids {
            self.delete_managed_file(id).await?;
        }
        Ok(())
    }

    /// 搜索文件
    pub async fn search_managed_files(&self, query: &str, limit: i64) -> Result<Vec<ManagedFile>> {
        let search_pattern = format!("%{}%", query);
        
        let files = sqlx::query_as::<_, ManagedFile>(
            r#"
            SELECT id, original_name, masked_name, file_path, file_size, 
                   file_type, masked_count, gitea_uploaded, gitea_url, 
                   created_at, updated_at
            FROM managed_files
            WHERE original_name LIKE ? OR masked_name LIKE ?
            ORDER BY created_at DESC
            LIMIT ?
            "#,
        )
        .bind(&search_pattern)
        .bind(&search_pattern)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(files)
    }
}
