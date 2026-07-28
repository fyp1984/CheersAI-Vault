use std::path::{Component, Path, PathBuf};

use chrono::Utc;
use service_contracts::{
    BatchDetail, BatchFile, BatchStatus, BatchSummary, CreateBatchResponse, CreatedFile,
    FileStatus, RetryResponse,
};
use sqlx::{sqlite::SqlitePoolOptions, Row, SqlitePool};
use tokio::fs;
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("storage operation failed")]
    Storage,
    #[error("record not found")]
    NotFound,
    #[error("file is not eligible for retry")]
    RetryConflict,
    #[error("stored state is invalid")]
    InvalidState,
}

impl From<sqlx::Error> for StoreError {
    fn from(_: sqlx::Error) -> Self {
        Self::Storage
    }
}

impl From<std::io::Error> for StoreError {
    fn from(_: std::io::Error) -> Self {
        Self::Storage
    }
}

#[derive(Debug)]
pub struct NewUpload {
    pub display_name: String,
    pub input_format: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug)]
pub struct PendingJob {
    pub batch_id: String,
    pub file_id: String,
    pub display_name: String,
    pub input_format: String,
    pub input_object_key: String,
    pub restore_mode: String,
    pub rules: Vec<String>,
}

#[derive(Debug)]
pub struct ArtifactRecord {
    pub object_key: String,
}

#[derive(Clone)]
pub struct Store {
    pool: SqlitePool,
    root: PathBuf,
}

impl Store {
    pub async fn open(root: PathBuf) -> Result<Self, StoreError> {
        fs::create_dir_all(root.join("input")).await?;
        fs::create_dir_all(root.join("output")).await?;
        fs::create_dir_all(root.join("mapping")).await?;
        let tmp_dir = root.join("tmp");
        fs::create_dir_all(&tmp_dir).await?;
        let mut tmp_entries = fs::read_dir(&tmp_dir).await?;
        while let Some(entry) = tmp_entries.next_entry().await? {
            if entry.file_type().await?.is_file() {
                fs::remove_file(entry.path()).await?;
            }
        }

        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(
                sqlx::sqlite::SqliteConnectOptions::new()
                    .filename(root.join("vault-pro.db"))
                    .create_if_missing(true)
                    .foreign_keys(true),
            )
            .await?;
        let store = Self { pool, root };
        store.initialize_schema().await?;
        Ok(store)
    }

    async fn initialize_schema(&self) -> Result<(), StoreError> {
        for statement in [
            "CREATE TABLE IF NOT EXISTS batches (id TEXT PRIMARY KEY, status TEXT NOT NULL, rules_json TEXT NOT NULL, restore_mode TEXT NOT NULL DEFAULT 'disabled', created_at TEXT NOT NULL, updated_at TEXT NOT NULL)",
            "CREATE TABLE IF NOT EXISTS batch_files (id TEXT PRIMARY KEY, batch_id TEXT NOT NULL REFERENCES batches(id) ON DELETE CASCADE, display_name TEXT NOT NULL, input_format TEXT NOT NULL, input_object_key TEXT NOT NULL, mapping_object_key TEXT, status TEXT NOT NULL, attempt INTEGER NOT NULL DEFAULT 1, masked_entity_count INTEGER, artifact_id TEXT, error_code TEXT, error_message TEXT, created_at TEXT NOT NULL, updated_at TEXT NOT NULL)",
            "CREATE INDEX IF NOT EXISTS idx_batch_files_pending ON batch_files(status, created_at)",
            "CREATE TABLE IF NOT EXISTS artifacts (id TEXT PRIMARY KEY, batch_id TEXT NOT NULL REFERENCES batches(id) ON DELETE CASCADE, file_id TEXT NOT NULL UNIQUE REFERENCES batch_files(id) ON DELETE CASCADE, object_key TEXT NOT NULL UNIQUE, size_bytes INTEGER NOT NULL, created_at TEXT NOT NULL)",
            "CREATE TABLE IF NOT EXISTS job_events (id TEXT PRIMARY KEY, batch_id TEXT NOT NULL, file_id TEXT, event_type TEXT NOT NULL, status TEXT NOT NULL, error_code TEXT, created_at TEXT NOT NULL)",
            "CREATE TABLE IF NOT EXISTS restore_events (id TEXT PRIMARY KEY, event_type TEXT NOT NULL, status TEXT NOT NULL, error_code TEXT, restored_entity_count INTEGER, timestamp TEXT NOT NULL)",
        ] {
            sqlx::query(statement).execute(&self.pool).await?;
        }
        // Idempotent column migrations for upgrading from older schema
        for statement in [
            "ALTER TABLE batches ADD COLUMN restore_mode TEXT NOT NULL DEFAULT 'disabled'",
            "ALTER TABLE batch_files ADD COLUMN mapping_object_key TEXT",
        ] {
            let _ = sqlx::query(statement).execute(&self.pool).await;
        }
        Ok(())
    }

    pub async fn recover_interrupted(&self) -> Result<usize, StoreError> {
        let rows = sqlx::query("SELECT id, batch_id FROM batch_files WHERE status = ?")
            .bind(FileStatus::Processing.as_str())
            .fetch_all(&self.pool)
            .await?;
        if rows.is_empty() {
            return Ok(0);
        }

        let now = Utc::now().to_rfc3339();
        let mut tx = self.pool.begin().await?;
        for row in &rows {
            let file_id: String = row.get("id");
            let batch_id: String = row.get("batch_id");
            sqlx::query("UPDATE batch_files SET status = ?, error_code = ?, error_message = ?, updated_at = ? WHERE id = ?")
                .bind(FileStatus::Failed.as_str())
                .bind("PROCESS_INTERRUPTED")
                .bind("Processing was interrupted; retry is available")
                .bind(&now)
                .bind(&file_id)
                .execute(&mut *tx)
                .await?;
            insert_event(
                &mut tx,
                &batch_id,
                Some(&file_id),
                "RecoveredInterrupted",
                FileStatus::Failed.as_str(),
                Some("PROCESS_INTERRUPTED"),
                &now,
            )
            .await?;
        }
        tx.commit().await?;

        let recovered_count = rows.len();
        for row in rows {
            let batch_id: String = row.get("batch_id");
            self.refresh_batch(&batch_id).await?;
        }
        Ok(recovered_count)
    }

    pub async fn create_batch(
        &self,
        uploads: Vec<NewUpload>,
        rules: Vec<String>,
        restore_mode: &str,
    ) -> Result<CreateBatchResponse, StoreError> {
        let batch_id = Uuid::new_v4().to_string();
        let batch_dir = self.root.join("input").join(&batch_id);
        fs::create_dir_all(&batch_dir).await?;
        let now = Utc::now().to_rfc3339();
        let mut created = Vec::with_capacity(uploads.len());
        let mut stored = Vec::with_capacity(uploads.len());

        for upload in uploads {
            let file_id = Uuid::new_v4().to_string();
            let object_key = format!("input/{batch_id}/{file_id}.input");
            let path = self.controlled_path(&object_key)?;
            fs::write(path, upload.bytes).await?;
            created.push(CreatedFile {
                file_id: file_id.clone(),
                display_name: upload.display_name.clone(),
            });
            stored.push((file_id, upload.display_name, upload.input_format, object_key));
        }

        let write_result = async {
            let mut tx = self.pool.begin().await?;
            sqlx::query("INSERT INTO batches (id, status, rules_json, restore_mode, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)")
                .bind(&batch_id)
                .bind(BatchStatus::Running.as_str())
                .bind(serde_json::to_string(&rules).map_err(|_| StoreError::Storage)?)
                .bind(restore_mode)
                .bind(&now)
                .bind(&now)
                .execute(&mut *tx)
                .await?;
            for (file_id, display_name, input_format, object_key) in &stored {
                sqlx::query("INSERT INTO batch_files (id, batch_id, display_name, input_format, input_object_key, status, attempt, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, 1, ?, ?)")
                    .bind(file_id)
                    .bind(&batch_id)
                    .bind(display_name)
                    .bind(input_format)
                    .bind(object_key)
                    .bind(FileStatus::Pending.as_str())
                    .bind(&now)
                    .bind(&now)
                    .execute(&mut *tx)
                    .await?;
                insert_event(
                    &mut tx,
                    &batch_id,
                    Some(file_id),
                    "Queued",
                    FileStatus::Pending.as_str(),
                    None,
                    &now,
                )
                .await?;
            }
            tx.commit().await?;
            Ok::<(), StoreError>(())
        }
        .await;

        if write_result.is_err() {
            let _ = fs::remove_dir_all(batch_dir).await;
            write_result?;
        }

        Ok(CreateBatchResponse {
            batch_id,
            files: created,
        })
    }

    pub async fn claim_next_pending(&self) -> Result<Option<PendingJob>, StoreError> {
        let mut tx = self.pool.begin().await?;
        let row = sqlx::query(
            "SELECT f.id, f.batch_id, f.display_name, f.input_format, f.input_object_key, b.restore_mode, b.rules_json FROM batch_files f JOIN batches b ON b.id = f.batch_id WHERE f.status = ? ORDER BY f.created_at, f.id LIMIT 1",
        )
        .bind(FileStatus::Pending.as_str())
        .fetch_optional(&mut *tx)
        .await?;
        let Some(row) = row else {
            tx.commit().await?;
            return Ok(None);
        };

        let file_id: String = row.get("id");
        let batch_id: String = row.get("batch_id");
        let now = Utc::now().to_rfc3339();
        let changed = sqlx::query("UPDATE batch_files SET status = ?, error_code = NULL, error_message = NULL, updated_at = ? WHERE id = ? AND status = ?")
            .bind(FileStatus::Processing.as_str())
            .bind(&now)
            .bind(&file_id)
            .bind(FileStatus::Pending.as_str())
            .execute(&mut *tx)
            .await?
            .rows_affected();
        if changed == 0 {
            tx.rollback().await?;
            return Ok(None);
        }
        insert_event(
            &mut tx,
            &batch_id,
            Some(&file_id),
            "ProcessingStarted",
            FileStatus::Processing.as_str(),
            None,
            &now,
        )
        .await?;
        tx.commit().await?;

        let rules_json: String = row.get("rules_json");
        Ok(Some(PendingJob {
            batch_id,
            file_id,
            display_name: row.get("display_name"),
            input_format: row.get("input_format"),
            input_object_key: row.get("input_object_key"),
            restore_mode: row.get("restore_mode"),
            rules: serde_json::from_str(&rules_json)
                .map_err(|_| StoreError::InvalidState)?,
        }))
    }

    pub async fn read_input(&self, object_key: &str) -> Result<Vec<u8>, StoreError> {
        Ok(fs::read(self.controlled_path(object_key)?).await?)
    }

    pub async fn write_completed(
        &self,
        job: &PendingJob,
        markdown: &[u8],
        masked_entity_count: usize,
        artifact_id: &str,
        mapping_bytes: Option<&[u8]>,
    ) -> Result<String, StoreError> {
        // 1. Write markdown atomically to output/{batch}/{artifact_id}.md
        let object_key = format!("output/{}/{artifact_id}.md", job.batch_id);
        let output_path = self.controlled_path(&object_key)?;
        let output_dir = output_path.parent().ok_or(StoreError::Storage)?;
        fs::create_dir_all(output_dir).await?;
        let tmp_key = format!("tmp/{}.tmp", Uuid::new_v4());
        let tmp_path = self.controlled_path(&tmp_key)?;
        fs::write(&tmp_path, markdown).await?;
        if let Err(error) = fs::rename(&tmp_path, &output_path).await {
            let _ = fs::remove_file(&tmp_path).await;
            return Err(error.into());
        }

        // 2. Write mapping atomically to private mapping/{batch}/{artifact_id}.cmap (if provided)
        let mapping_key = if let Some(bytes) = mapping_bytes {
            let key = format!("mapping/{}/{artifact_id}.cmap", job.batch_id);
            let path = self.controlled_path(&key)?;
            let dir = path.parent().ok_or(StoreError::Storage)?;
            fs::create_dir_all(dir).await?;
            let tmp_key = format!("tmp/{}.cmap.tmp", Uuid::new_v4());
            let tmp_path = self.controlled_path(&tmp_key)?;
            fs::write(&tmp_path, bytes).await?;
            if let Err(e) = fs::rename(&tmp_path, &path).await {
                let _ = fs::remove_file(&tmp_path).await;
                let _ = fs::remove_file(&output_path).await; // rollback markdown
                return Err(e.into());
            }
            Some(key)
        } else {
            None
        };

        // 3. Single DB transaction: insert artifact + update batch_files with mapping
        let now = Utc::now().to_rfc3339();
        let result = async {
            let mut tx = self.pool.begin().await?;
            sqlx::query("INSERT INTO artifacts (id, batch_id, file_id, object_key, size_bytes, created_at) VALUES (?, ?, ?, ?, ?, ?)")
                .bind(artifact_id)
                .bind(&job.batch_id)
                .bind(&job.file_id)
                .bind(&object_key)
                .bind(markdown.len() as i64)
                .bind(&now)
                .execute(&mut *tx)
                .await?;
            sqlx::query("UPDATE batch_files SET status = ?, masked_entity_count = ?, artifact_id = ?, mapping_object_key = ?, error_code = NULL, error_message = NULL, updated_at = ? WHERE id = ? AND status = ?")
                .bind(FileStatus::Completed.as_str())
                .bind(masked_entity_count as i64)
                .bind(artifact_id)
                .bind(&mapping_key)
                .bind(&now)
                .bind(&job.file_id)
                .bind(FileStatus::Processing.as_str())
                .execute(&mut *tx)
                .await?;
            insert_event(
                &mut tx,
                &job.batch_id,
                Some(&job.file_id),
                "Completed",
                FileStatus::Completed.as_str(),
                None,
                &now,
            )
            .await?;
            tx.commit().await?;
            Ok::<(), StoreError>(())
        }
        .await;
        if result.is_err() {
            let _ = fs::remove_file(output_path).await;
            if let Some(ref key) = mapping_key {
                if let Ok(path) = self.controlled_path(key) {
                    let _ = fs::remove_file(path).await;
                }
            }
            result?;
        }
        self.refresh_batch(&job.batch_id).await?;
        Ok(artifact_id.to_string())
    }

    /// Read markdown + mapping for a completed artifact (restore).
    pub async fn artifact_with_mapping(&self, artifact_id: &str) -> Result<(Vec<u8>, Vec<u8>), StoreError> {
        let row = sqlx::query(
            "SELECT a.object_key, f.mapping_object_key FROM artifacts a JOIN batch_files f ON f.id = a.file_id WHERE a.id = ? AND f.status = ?",
        )
        .bind(artifact_id)
        .bind(FileStatus::Completed.as_str())
        .fetch_optional(&self.pool)
        .await?
        .ok_or(StoreError::NotFound)?;
        let object_key: String = row.get("object_key");
        let mapping_key: Option<String> = row.get("mapping_object_key");
        let markdown = fs::read(self.controlled_path(&object_key)?).await?;
        match mapping_key {
            Some(key) => {
                let mapping = fs::read(self.controlled_path(&key)?).await?;
                Ok((markdown, mapping))
            }
            None => Err(StoreError::NotFound),
        }
    }

    pub async fn mark_failed(
        &self,
        job: &PendingJob,
        error_code: &str,
        error_message: &str,
    ) -> Result<(), StoreError> {
        let now = Utc::now().to_rfc3339();
        let mut tx = self.pool.begin().await?;
        sqlx::query("UPDATE batch_files SET status = ?, masked_entity_count = NULL, artifact_id = NULL, error_code = ?, error_message = ?, updated_at = ? WHERE id = ?")
            .bind(FileStatus::Failed.as_str())
            .bind(error_code)
            .bind(error_message)
            .bind(&now)
            .bind(&job.file_id)
            .execute(&mut *tx)
            .await?;
        insert_event(
            &mut tx,
            &job.batch_id,
            Some(&job.file_id),
            "Failed",
            FileStatus::Failed.as_str(),
            Some(error_code),
            &now,
        )
        .await?;
        tx.commit().await?;
        self.refresh_batch(&job.batch_id).await
    }

    pub async fn list_batches(&self) -> Result<Vec<BatchSummary>, StoreError> {
        let rows = sqlx::query(
            "SELECT b.id, b.status, b.created_at, b.updated_at, COUNT(f.id) file_count, SUM(CASE WHEN f.status = ? THEN 1 ELSE 0 END) completed_count, SUM(CASE WHEN f.status = ? THEN 1 ELSE 0 END) failed_count, COALESCE(SUM(f.masked_entity_count), 0) masked_entity_count FROM batches b LEFT JOIN batch_files f ON f.batch_id = b.id GROUP BY b.id ORDER BY b.created_at DESC",
        )
        .bind(FileStatus::Completed.as_str())
        .bind(FileStatus::Failed.as_str())
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter().map(summary_from_row).collect()
    }

    pub async fn batch_detail(&self, batch_id: &str) -> Result<BatchDetail, StoreError> {
        let summary_row = sqlx::query(
            "SELECT b.id, b.status, b.created_at, b.updated_at, COUNT(f.id) file_count, SUM(CASE WHEN f.status = ? THEN 1 ELSE 0 END) completed_count, SUM(CASE WHEN f.status = ? THEN 1 ELSE 0 END) failed_count, COALESCE(SUM(f.masked_entity_count), 0) masked_entity_count FROM batches b LEFT JOIN batch_files f ON f.batch_id = b.id WHERE b.id = ? GROUP BY b.id",
        )
        .bind(FileStatus::Completed.as_str())
        .bind(FileStatus::Failed.as_str())
        .bind(batch_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(StoreError::NotFound)?;
        let batch = summary_from_row(summary_row)?;
        let rows = sqlx::query("SELECT id, display_name, input_format, status, attempt, masked_entity_count, artifact_id, mapping_object_key, error_code, error_message FROM batch_files WHERE batch_id = ? ORDER BY created_at, id")
            .bind(batch_id)
            .fetch_all(&self.pool)
            .await?;
        let files = rows
            .into_iter()
            .map(|row| {
                let status_value: String = row.get("status");
                let mapping_key: Option<String> = row.get("mapping_object_key");
                Ok(BatchFile {
                    file_id: row.get("id"),
                    display_name: row.get("display_name"),
                    input_format: row.get("input_format"),
                    status: FileStatus::parse(&status_value).ok_or(StoreError::InvalidState)?,
                    attempt: row.get::<i64, _>("attempt") as usize,
                    masked_entity_count: row
                        .get::<Option<i64>, _>("masked_entity_count")
                        .map(|value| value as usize),
                    artifact_id: row.get("artifact_id"),
                    error_code: row.get("error_code"),
                    error_message: row.get("error_message"),
                    restore_available: mapping_key.is_some(),
                })
            })
            .collect::<Result<Vec<_>, StoreError>>()?;
        Ok(BatchDetail { batch, files })
    }

    pub async fn retry(&self, file_id: &str) -> Result<RetryResponse, StoreError> {
        let now = Utc::now().to_rfc3339();
        let mut tx = self.pool.begin().await?;
        let updated = sqlx::query("UPDATE batch_files SET status = ?, attempt = attempt + 1, masked_entity_count = NULL, artifact_id = NULL, error_code = NULL, error_message = NULL, updated_at = ? WHERE id = ? AND status = ?")
            .bind(FileStatus::Pending.as_str())
            .bind(&now)
            .bind(file_id)
            .bind(FileStatus::Failed.as_str())
            .execute(&mut *tx)
            .await?;
        if updated.rows_affected() != 1 {
            let exists = sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM batch_files WHERE id = ?",
            )
            .bind(file_id)
            .fetch_one(&mut *tx)
            .await?
                == 1;
            tx.rollback().await?;
            return Err(if exists {
                StoreError::RetryConflict
            } else {
                StoreError::NotFound
            });
        }
        let row = sqlx::query("SELECT batch_id, attempt FROM batch_files WHERE id = ?")
            .bind(file_id)
            .fetch_one(&mut *tx)
            .await?;
        let batch_id: String = row.get("batch_id");
        let attempt = row.get::<i64, _>("attempt");
        sqlx::query("UPDATE batches SET status = ?, updated_at = ? WHERE id = ?")
            .bind(BatchStatus::Running.as_str())
            .bind(&now)
            .bind(&batch_id)
            .execute(&mut *tx)
            .await?;
        insert_event(
            &mut tx,
            &batch_id,
            Some(file_id),
            "RetryQueued",
            FileStatus::Pending.as_str(),
            None,
            &now,
        )
        .await?;
        tx.commit().await?;
        Ok(RetryResponse {
            file_id: file_id.to_string(),
            status: FileStatus::Pending,
            attempt: attempt as usize,
        })
    }

    pub async fn artifact(&self, artifact_id: &str) -> Result<(ArtifactRecord, Vec<u8>), StoreError> {
        let row = sqlx::query("SELECT a.object_key FROM artifacts a JOIN batch_files f ON f.id = a.file_id WHERE a.id = ? AND f.status = ?")
            .bind(artifact_id)
            .bind(FileStatus::Completed.as_str())
            .fetch_optional(&self.pool)
            .await?
            .ok_or(StoreError::NotFound)?;
        let record = ArtifactRecord {
            object_key: row.get("object_key"),
        };
        let bytes = fs::read(self.controlled_path(&record.object_key)?).await?;
        Ok((record, bytes))
    }

    async fn refresh_batch(&self, batch_id: &str) -> Result<(), StoreError> {
        let row = sqlx::query("SELECT COUNT(*) total, SUM(CASE WHEN status = ? THEN 1 ELSE 0 END) pending_count, SUM(CASE WHEN status = ? THEN 1 ELSE 0 END) processing_count, SUM(CASE WHEN status = ? THEN 1 ELSE 0 END) completed_count, SUM(CASE WHEN status = ? THEN 1 ELSE 0 END) failed_count FROM batch_files WHERE batch_id = ?")
            .bind(FileStatus::Pending.as_str())
            .bind(FileStatus::Processing.as_str())
            .bind(FileStatus::Completed.as_str())
            .bind(FileStatus::Failed.as_str())
            .bind(batch_id)
            .fetch_one(&self.pool)
            .await?;
        let total: i64 = row.get("total");
        let pending: i64 = row.get("pending_count");
        let processing: i64 = row.get("processing_count");
        let completed: i64 = row.get("completed_count");
        let failed: i64 = row.get("failed_count");
        let status = if pending > 0 || processing > 0 {
            BatchStatus::Running
        } else if total > 0 && completed == total {
            BatchStatus::Completed
        } else if completed > 0 && failed > 0 {
            BatchStatus::CompletedWithErrors
        } else {
            BatchStatus::Failed
        };
        sqlx::query("UPDATE batches SET status = ?, updated_at = ? WHERE id = ?")
            .bind(status.as_str())
            .bind(Utc::now().to_rfc3339())
            .bind(batch_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    fn controlled_path(&self, object_key: &str) -> Result<PathBuf, StoreError> {
        let relative = Path::new(object_key);
        if relative.is_absolute()
            || relative
                .components()
                .any(|component| !matches!(component, Component::Normal(_)))
        {
            return Err(StoreError::InvalidState);
        }
        Ok(self.root.join(relative))
    }

    pub async fn log_restore_event(
        &self,
        event_type: &str,
        status: &str,
        error_code: Option<&str>,
        restored_entity_count: Option<usize>,
    ) -> Result<(), StoreError> {
        let now = Utc::now().to_rfc3339();
        sqlx::query("INSERT INTO restore_events (id, event_type, status, error_code, restored_entity_count, timestamp) VALUES (?, ?, ?, ?, ?, ?)")
            .bind(Uuid::new_v4().to_string())
            .bind(event_type)
            .bind(status)
            .bind(error_code)
            .bind(restored_entity_count.map(|c| c as i64))
            .bind(&now)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    #[cfg(test)]
    pub async fn force_processing(&self, file_id: &str) -> Result<(), StoreError> {
        sqlx::query("UPDATE batch_files SET status = ? WHERE id = ?")
            .bind(FileStatus::Processing.as_str())
            .bind(file_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    #[cfg(test)]
    pub async fn force_failed(&self, file_id: &str) -> Result<(), StoreError> {
        sqlx::query("UPDATE batch_files SET status = ? WHERE id = ?")
            .bind(FileStatus::Failed.as_str())
            .bind(file_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    #[cfg(test)]
    pub async fn record_counts(&self) -> Result<(usize, usize), StoreError> {
        let batches = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM batches")
            .fetch_one(&self.pool)
            .await?;
        let files = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM batch_files")
            .fetch_one(&self.pool)
            .await?;
        Ok((batches as usize, files as usize))
    }

    #[cfg(test)]
    pub fn database_path(&self) -> PathBuf {
        self.root.join("vault-pro.db")
    }

    #[cfg(test)]
    pub async fn event_count(&self, file_id: &str, event_type: &str) -> Result<usize, StoreError> {
        let row = sqlx::query("SELECT COUNT(*) count FROM job_events WHERE file_id = ? AND event_type = ?")
            .bind(file_id)
            .bind(event_type)
            .fetch_one(&self.pool)
            .await?;
        Ok(row.get::<i64, _>("count") as usize)
    }
}

fn summary_from_row(row: sqlx::sqlite::SqliteRow) -> Result<BatchSummary, StoreError> {
    let status_value: String = row.get("status");
    Ok(BatchSummary {
        batch_id: row.get("id"),
        status: BatchStatus::parse(&status_value).ok_or(StoreError::InvalidState)?,
        file_count: row.get::<i64, _>("file_count") as usize,
        completed_count: row.get::<i64, _>("completed_count") as usize,
        failed_count: row.get::<i64, _>("failed_count") as usize,
        masked_entity_count: row.get::<i64, _>("masked_entity_count") as usize,
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

async fn insert_event(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    batch_id: &str,
    file_id: Option<&str>,
    event_type: &str,
    status: &str,
    error_code: Option<&str>,
    created_at: &str,
) -> Result<(), StoreError> {
    sqlx::query("INSERT INTO job_events (id, batch_id, file_id, event_type, status, error_code, created_at) VALUES (?, ?, ?, ?, ?, ?, ?)")
        .bind(Uuid::new_v4().to_string())
        .bind(batch_id)
        .bind(file_id)
        .bind(event_type)
        .bind(status)
        .bind(error_code)
        .bind(created_at)
        .execute(&mut **tx)
        .await?;
    Ok(())
}
