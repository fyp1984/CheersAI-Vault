use std::path::{Component, Path, PathBuf};
use std::time::Duration;

use chrono::Utc;
use engine_core::SensitiveTermDefinition;
use service_contracts::{
    BatchDetail, BatchFile, BatchStatus, BatchSummary, CreateBatchResponse, CreatePreviewResponse,
    CreatedFile, CreatedPreviewFile, FileStatus, OperationLogEntry, OperationLogLevel,
    OperationLogStatistics, OperationLogStorageStatus, PreviewDetail, PreviewFile,
    PreviewFileStatus, PreviewSessionStatus, RetryResponse, SensitiveTerm, SensitiveTermsStats,
};
use sqlx::{sqlite::SqlitePoolOptions, Row, SqlitePool};
use tokio::fs;
use uuid::Uuid;

/// Special `rule_ids` entry (never a real engine-core rule id) that opts a
/// batch/preview into the enabled sensitive-term library. `pub(crate)` so
/// `lib.rs`'s `/rules` metadata and validation share this single literal.
pub(crate) const SENSITIVE_TERMS_RULE_ID: &str = "use_sensitive_terms";

pub const SENSITIVE_TERM_MAX_CHARS: usize = 256;
pub const SENSITIVE_TERM_CATEGORY_MAX_CHARS: usize = 64;
pub const SENSITIVE_TERM_DESCRIPTION_MAX_CHARS: usize = 512;
pub const SENSITIVE_TERMS_MAX_COUNT: usize = 10_000;
pub const SENSITIVE_TERMS_IMPORT_MAX_BYTES: usize = 5 * 1024 * 1024;
pub const SENSITIVE_TERMS_IMPORT_MAX_ROWS: usize = 10_000;

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
    #[error("preview session is already confirmed")]
    PreviewAlreadyConfirmed,
    #[error("{0}")]
    SensitiveTermInvalid(String),
    #[error("sensitive term already exists")]
    SensitiveTermDuplicate,
    #[error("sensitive term not found")]
    SensitiveTermNotFound,
    #[error("{0}")]
    SensitiveTermsImportInvalid(String),
    #[error("input exceeds the configured limit")]
    InputLimitExceeded,
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
    pub display_name: String,
}

#[derive(Debug)]
pub struct PendingPreviewJob {
    pub preview_id: String,
    pub file_id: String,
    pub display_name: String,
    pub input_format: String,
    pub input_object_key: String,
    pub rules: Vec<String>,
}

/// Outcome of [`Store::confirm_preview`]. Distinguishes a fresh confirm from
/// an idempotent repeat of an already-confirmed preview (E4) and from a
/// concurrent in-flight confirm, without ever producing a second batch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfirmOutcome {
    Confirmed { batch_id: String },
    AlreadyConfirmed { batch_id: String },
    InFlight,
    NotReady,
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
            "CREATE INDEX IF NOT EXISTS idx_job_events_file_created ON job_events(file_id, created_at)",
            "CREATE INDEX IF NOT EXISTS idx_job_events_batch_created ON job_events(batch_id, created_at)",
            "CREATE TABLE IF NOT EXISTS restore_events (id TEXT PRIMARY KEY, event_type TEXT NOT NULL, status TEXT NOT NULL, error_code TEXT, restored_entity_count INTEGER, timestamp TEXT NOT NULL)",
            "CREATE TABLE IF NOT EXISTS previews (id TEXT PRIMARY KEY, status TEXT NOT NULL, rules_json TEXT NOT NULL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL, expires_at TEXT NOT NULL, confirming INTEGER NOT NULL DEFAULT 0, confirmed_batch_id TEXT)",
            "CREATE TABLE IF NOT EXISTS preview_files (id TEXT PRIMARY KEY, preview_id TEXT NOT NULL REFERENCES previews(id) ON DELETE CASCADE, display_name TEXT NOT NULL, input_format TEXT NOT NULL, input_object_key TEXT NOT NULL, status TEXT NOT NULL, masked_entity_count INTEGER, artifact_id TEXT, markdown_object_key TEXT, mapping_object_key TEXT, error_code TEXT, error_message TEXT, created_at TEXT NOT NULL, updated_at TEXT NOT NULL)",
            "CREATE INDEX IF NOT EXISTS idx_preview_files_pending ON preview_files(status, created_at)",
            "CREATE INDEX IF NOT EXISTS idx_previews_expiry ON previews(expires_at, status)",
            "CREATE TABLE IF NOT EXISTS sensitive_terms (id TEXT PRIMARY KEY, term TEXT NOT NULL, category TEXT NOT NULL, description TEXT, enabled INTEGER NOT NULL DEFAULT 1, created_at TEXT NOT NULL, updated_at TEXT NOT NULL)",
            "CREATE INDEX IF NOT EXISTS idx_sensitive_terms_category ON sensitive_terms(category)",
            "CREATE INDEX IF NOT EXISTS idx_sensitive_terms_enabled ON sensitive_terms(enabled)",
            // FileBay upload events (D3/C5): only ever written after an
            // actual attempted FileBay call (never for a request rejected
            // by our own whitelist before any transport call). Never a
            // column for token, Authorization, full local path, request
            // body or remote response body.
            "CREATE TABLE IF NOT EXISTS filebay_events (id TEXT PRIMARY KEY, batch_id TEXT, file_id TEXT, artifact_id TEXT NOT NULL, display_name TEXT NOT NULL, target_domain TEXT NOT NULL, owner TEXT NOT NULL, repo TEXT NOT NULL, remote_path TEXT NOT NULL, status TEXT NOT NULL, error_code TEXT, created_at TEXT NOT NULL)",
            "CREATE INDEX IF NOT EXISTS idx_filebay_events_created ON filebay_events(created_at)",
        ] {
            sqlx::query(statement).execute(&self.pool).await?;
        }
        // Idempotent column migrations for upgrading from older schema. A
        // pre-existing `batches`/`previews` row upgraded through this path
        // has no snapshot column value (NULL), which every reader below
        // treats as an empty snapshot (6.5) — never a hard migration error.
        for statement in [
            "ALTER TABLE batches ADD COLUMN restore_mode TEXT NOT NULL DEFAULT 'disabled'",
            "ALTER TABLE batch_files ADD COLUMN mapping_object_key TEXT",
            "ALTER TABLE batches ADD COLUMN sensitive_terms_snapshot_json TEXT",
            "ALTER TABLE previews ADD COLUMN sensitive_terms_snapshot_json TEXT",
        ] {
            let _ = sqlx::query(statement).execute(&self.pool).await;
        }
        Ok(())
    }

    /// Read the currently-enabled sensitive terms inside an already-open
    /// transaction, so the snapshot taken at batch/preview creation is
    /// atomic with the insert itself (6.1) — a concurrent edit can never
    /// land between "read enabled terms" and "commit the new batch/preview".
    async fn snapshot_enabled_sensitive_terms_tx(
        tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    ) -> Result<Vec<SensitiveTermDefinition>, StoreError> {
        let rows = sqlx::query(
            "SELECT id, term, category FROM sensitive_terms WHERE enabled = 1 ORDER BY term",
        )
        .fetch_all(&mut **tx)
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| SensitiveTermDefinition {
                id: row.get("id"),
                term: row.get("term"),
                category: row.get("category"),
                enabled: true,
            })
            .collect())
    }

    /// Compute the `sensitive_terms_snapshot_json` column value for a new
    /// batch/preview: `None` (stored as SQL NULL) unless the caller actually
    /// requested `use_sensitive_terms`, in which case it is the JSON-encoded
    /// snapshot (possibly an empty array, e.g. when no term is enabled).
    async fn snapshot_json_for_rules(
        tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
        rules: &[String],
    ) -> Result<Option<String>, StoreError> {
        if !rules.iter().any(|rule| rule == SENSITIVE_TERMS_RULE_ID) {
            return Ok(None);
        }
        let snapshot = Self::snapshot_enabled_sensitive_terms_tx(tx).await?;
        Ok(Some(
            serde_json::to_string(&snapshot).map_err(|_| StoreError::Storage)?,
        ))
    }

    /// Expand a claimed job's stored rule ids with its frozen sensitive-term
    /// snapshot (if any) into the single opaque channel `processing::process_input`
    /// already accepts (`rule_ids: &[String]`) — see `processing::decode_sensitive_terms_snapshot`
    /// for why this indirection exists instead of a dedicated field.
    fn expand_rules_with_snapshot(
        mut rules: Vec<String>,
        snapshot_json: Option<String>,
    ) -> Vec<String> {
        if let Some(snapshot_json) = snapshot_json {
            if let Some(entry) =
                crate::processing::encode_sensitive_terms_snapshot_entry(&snapshot_json)
            {
                rules.push(entry);
            }
        }
        rules
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
            stored.push((
                file_id,
                upload.display_name,
                upload.input_format,
                object_key,
            ));
        }

        let write_result = async {
            let mut tx = self.pool.begin().await?;
            // Snapshot enabled sensitive terms inside this same transaction
            // (6.1) before the batch row is even inserted, so the frozen set
            // is atomic with batch creation.
            let snapshot_json = Self::snapshot_json_for_rules(&mut tx, &rules).await?;
            sqlx::query("INSERT INTO batches (id, status, rules_json, restore_mode, sensitive_terms_snapshot_json, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?)")
                .bind(&batch_id)
                .bind(BatchStatus::Running.as_str())
                .bind(serde_json::to_string(&rules).map_err(|_| StoreError::Storage)?)
                .bind(restore_mode)
                .bind(&snapshot_json)
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
            "SELECT f.id, f.batch_id, f.display_name, f.input_format, f.input_object_key, b.restore_mode, b.rules_json, b.sensitive_terms_snapshot_json FROM batch_files f JOIN batches b ON b.id = f.batch_id WHERE f.status = ? ORDER BY f.created_at, f.id LIMIT 1",
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
        let snapshot_json: Option<String> = row.get("sensitive_terms_snapshot_json");
        let rules: Vec<String> =
            serde_json::from_str(&rules_json).map_err(|_| StoreError::InvalidState)?;
        Ok(Some(PendingJob {
            batch_id,
            file_id,
            display_name: row.get("display_name"),
            input_format: row.get("input_format"),
            input_object_key: row.get("input_object_key"),
            restore_mode: row.get("restore_mode"),
            rules: Self::expand_rules_with_snapshot(rules, snapshot_json),
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

    /// Read markdown + mapping + display name for a completed artifact (restore).
    pub async fn artifact_with_mapping(
        &self,
        artifact_id: &str,
    ) -> Result<(String, Vec<u8>, Vec<u8>), StoreError> {
        let row = sqlx::query(
            "SELECT a.object_key, f.mapping_object_key, f.display_name FROM artifacts a JOIN batch_files f ON f.id = a.file_id WHERE a.id = ? AND f.status = ?",
        )
        .bind(artifact_id)
        .bind(FileStatus::Completed.as_str())
        .fetch_optional(&self.pool)
        .await?
        .ok_or(StoreError::NotFound)?;
        let object_key: String = row.get("object_key");
        let mapping_key: Option<String> = row.get("mapping_object_key");
        let display_name: String = row.get("display_name");
        let markdown = fs::read(self.controlled_path(&object_key)?).await?;
        match mapping_key {
            Some(key) => {
                let mapping = fs::read(self.controlled_path(&key)?).await?;
                Ok((display_name, markdown, mapping))
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
            let exists =
                sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM batch_files WHERE id = ?")
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

    pub async fn artifact(
        &self,
        artifact_id: &str,
    ) -> Result<(ArtifactRecord, Vec<u8>), StoreError> {
        let row = sqlx::query("SELECT a.object_key, f.display_name FROM artifacts a JOIN batch_files f ON f.id = a.file_id WHERE a.id = ? AND f.status = ?")
            .bind(artifact_id)
            .bind(FileStatus::Completed.as_str())
            .fetch_optional(&self.pool)
            .await?
            .ok_or(StoreError::NotFound)?;
        let record = ArtifactRecord {
            object_key: row.get("object_key"),
            display_name: row.get("display_name"),
        };
        let bytes = fs::read(self.controlled_path(&record.object_key)?).await?;
        Ok((record, bytes))
    }

    // -----------------------------------------------------------------
    // FileBay candidates, controlled read, and upload event logging
    // -----------------------------------------------------------------

    /// Every `Completed` Markdown artifact belonging to `batch_id`, safe to
    /// offer as an upload candidate. Deliberately reuses the same
    /// `artifacts JOIN batch_files ... status = Completed` predicate as
    /// [`Self::artifact`] — never a separate, potentially-divergent query
    /// over arbitrary `object_key`s.
    pub async fn filebay_candidates(
        &self,
        batch_id: &str,
    ) -> Result<Vec<(String, String)>, StoreError> {
        let rows = sqlx::query(
            "SELECT a.id AS artifact_id, f.display_name AS display_name \
             FROM artifacts a JOIN batch_files f ON f.id = a.file_id \
             WHERE a.batch_id = ? AND f.status = ? ORDER BY a.created_at",
        )
        .bind(batch_id)
        .bind(FileStatus::Completed.as_str())
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| (row.get("artifact_id"), row.get("display_name")))
            .collect())
    }

    /// Re-validates that `artifact_id` is still a `Completed` artifact
    /// (same predicate as [`Self::artifact`]), resolves it through the same
    /// [`Self::controlled_path`] whitelist, and additionally rejects
    /// symlinks, non-regular files, non-`.md` extensions and non-UTF-8
    /// content before ever handing bytes to the FileBay upload path.
    /// Returns `(batch_id, display_name, bytes)`.
    pub async fn filebay_verified_artifact(
        &self,
        artifact_id: &str,
    ) -> Result<(String, String, Vec<u8>), StoreError> {
        let row = sqlx::query(
            "SELECT a.batch_id AS batch_id, a.object_key AS object_key, f.display_name AS display_name \
             FROM artifacts a JOIN batch_files f ON f.id = a.file_id \
             WHERE a.id = ? AND f.status = ?",
        )
        .bind(artifact_id)
        .bind(FileStatus::Completed.as_str())
        .fetch_optional(&self.pool)
        .await?
        .ok_or(StoreError::NotFound)?;
        let batch_id: String = row.get("batch_id");
        let object_key: String = row.get("object_key");
        let display_name: String = row.get("display_name");
        let path = self.controlled_path(&object_key)?;
        let metadata = fs::symlink_metadata(&path)
            .await
            .map_err(|_| StoreError::NotFound)?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(StoreError::InvalidState);
        }
        let is_markdown = path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.eq_ignore_ascii_case("md"))
            .unwrap_or(false);
        if !is_markdown {
            return Err(StoreError::InvalidState);
        }
        let bytes = fs::read(&path).await?;
        if std::str::from_utf8(&bytes).is_err() {
            return Err(StoreError::InvalidState);
        }
        Ok((batch_id, display_name, bytes))
    }

    /// Persists a minimal FileBay upload event. Only called after an
    /// actual attempted call to FileBay (never for a request this Runtime
    /// rejected itself before any transport call) — success or failure.
    /// `file_id` is best-effort (looked up from `artifact_id`) and may be
    /// `None` if the artifact/file record was concurrently removed between
    /// the upload attempt and this call; that never turns a real upload
    /// outcome into a different one, it only affects which file the event
    /// links back to in the log view.
    #[allow(clippy::too_many_arguments)]
    pub async fn log_filebay_upload_event(
        &self,
        batch_id: Option<&str>,
        artifact_id: &str,
        display_name: &str,
        target_domain: &str,
        owner: &str,
        repo: &str,
        remote_path: &str,
        success: bool,
        error_code: Option<&str>,
    ) -> Result<(), StoreError> {
        let file_id: Option<String> =
            sqlx::query_scalar("SELECT file_id FROM artifacts WHERE id = ?")
                .bind(artifact_id)
                .fetch_optional(&self.pool)
                .await
                .unwrap_or(None);
        sqlx::query(
            "INSERT INTO filebay_events (id, batch_id, file_id, artifact_id, display_name, target_domain, owner, repo, remote_path, status, error_code, created_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(Uuid::new_v4().to_string())
        .bind(batch_id)
        .bind(file_id)
        .bind(artifact_id)
        .bind(display_name)
        .bind(target_domain)
        .bind(owner)
        .bind(repo)
        .bind(remote_path)
        .bind(if success { "success" } else { "failed" })
        .bind(error_code)
        .bind(Utc::now().to_rfc3339())
        .execute(&self.pool)
        .await?;
        Ok(())
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

    // -----------------------------------------------------------------
    // Preview sessions (two-phase browser preview/confirm)
    // -----------------------------------------------------------------

    /// Create a temporary preview session. Never touches `batches`,
    /// `batch_files`, `artifacts`, `job_events` or `restore_events` (C1).
    /// All object keys are rooted under `preview/{preview_id}/...`, itself
    /// composed only of server-generated UUIDs.
    pub async fn create_preview(
        &self,
        uploads: Vec<NewUpload>,
        rules: Vec<String>,
        ttl: Duration,
    ) -> Result<CreatePreviewResponse, StoreError> {
        let preview_id = Uuid::new_v4().to_string();
        let preview_dir = self.root.join("preview").join(&preview_id);
        let input_dir = preview_dir.join("input");
        fs::create_dir_all(&input_dir).await?;
        let now = Utc::now();
        let now_str = now.to_rfc3339();
        let expires_at =
            (now + chrono::Duration::from_std(ttl).map_err(|_| StoreError::Storage)?).to_rfc3339();

        let mut created = Vec::with_capacity(uploads.len());
        let mut stored = Vec::with_capacity(uploads.len());
        for upload in uploads {
            let file_id = Uuid::new_v4().to_string();
            let object_key = format!("preview/{preview_id}/input/{file_id}.input");
            let path = self.controlled_path(&object_key)?;
            fs::write(path, upload.bytes).await?;
            created.push(CreatedPreviewFile {
                file_id: file_id.clone(),
                display_name: upload.display_name.clone(),
            });
            stored.push((
                file_id,
                upload.display_name,
                upload.input_format,
                object_key,
            ));
        }

        let write_result = async {
            let mut tx = self.pool.begin().await?;
            // Snapshot enabled sensitive terms inside this same transaction
            // (6.1), atomic with preview creation — same rationale as
            // `create_batch`.
            let snapshot_json = Self::snapshot_json_for_rules(&mut tx, &rules).await?;
            sqlx::query("INSERT INTO previews (id, status, rules_json, sensitive_terms_snapshot_json, created_at, updated_at, expires_at, confirming, confirmed_batch_id) VALUES (?, ?, ?, ?, ?, ?, ?, 0, NULL)")
                .bind(&preview_id)
                .bind(PreviewSessionStatus::Processing.as_str())
                .bind(serde_json::to_string(&rules).map_err(|_| StoreError::Storage)?)
                .bind(&snapshot_json)
                .bind(&now_str)
                .bind(&now_str)
                .bind(&expires_at)
                .execute(&mut *tx)
                .await?;
            for (file_id, display_name, input_format, object_key) in &stored {
                sqlx::query("INSERT INTO preview_files (id, preview_id, display_name, input_format, input_object_key, status, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?)")
                    .bind(file_id)
                    .bind(&preview_id)
                    .bind(display_name)
                    .bind(input_format)
                    .bind(object_key)
                    .bind(PreviewFileStatus::Pending.as_str())
                    .bind(&now_str)
                    .bind(&now_str)
                    .execute(&mut *tx)
                    .await?;
            }
            tx.commit().await?;
            Ok::<(), StoreError>(())
        }
        .await;

        if write_result.is_err() {
            let _ = fs::remove_dir_all(&preview_dir).await;
            write_result?;
        }

        Ok(CreatePreviewResponse {
            preview_id,
            files: created,
            expires_at,
        })
    }

    pub async fn claim_next_pending_preview_file(
        &self,
    ) -> Result<Option<PendingPreviewJob>, StoreError> {
        let mut tx = self.pool.begin().await?;
        let row = sqlx::query(
            "SELECT f.id, f.preview_id, f.display_name, f.input_format, f.input_object_key, p.rules_json, p.sensitive_terms_snapshot_json FROM preview_files f JOIN previews p ON p.id = f.preview_id WHERE f.status = ? ORDER BY f.created_at, f.id LIMIT 1",
        )
        .bind(PreviewFileStatus::Pending.as_str())
        .fetch_optional(&mut *tx)
        .await?;
        let Some(row) = row else {
            tx.commit().await?;
            return Ok(None);
        };

        let file_id: String = row.get("id");
        let preview_id: String = row.get("preview_id");
        let now = Utc::now().to_rfc3339();
        let changed = sqlx::query(
            "UPDATE preview_files SET status = ?, updated_at = ? WHERE id = ? AND status = ?",
        )
        .bind(PreviewFileStatus::Processing.as_str())
        .bind(&now)
        .bind(&file_id)
        .bind(PreviewFileStatus::Pending.as_str())
        .execute(&mut *tx)
        .await?
        .rows_affected();
        if changed == 0 {
            tx.rollback().await?;
            return Ok(None);
        }
        tx.commit().await?;
        self.refresh_preview_status(&preview_id).await?;

        let rules_json: String = row.get("rules_json");
        let snapshot_json: Option<String> = row.get("sensitive_terms_snapshot_json");
        let rules: Vec<String> =
            serde_json::from_str(&rules_json).map_err(|_| StoreError::InvalidState)?;
        Ok(Some(PendingPreviewJob {
            preview_id,
            file_id,
            display_name: row.get("display_name"),
            input_format: row.get("input_format"),
            input_object_key: row.get("input_object_key"),
            rules: Self::expand_rules_with_snapshot(rules, snapshot_json),
        }))
    }

    pub async fn mark_preview_file_failed(
        &self,
        preview_id: &str,
        file_id: &str,
        error_code: &str,
        error_message: &str,
    ) -> Result<(), StoreError> {
        let now = Utc::now().to_rfc3339();
        let changed = sqlx::query("UPDATE preview_files SET status = ?, error_code = ?, error_message = ?, updated_at = ? WHERE id = ? AND preview_id = ?")
            .bind(PreviewFileStatus::Failed.as_str())
            .bind(error_code)
            .bind(error_message)
            .bind(&now)
            .bind(file_id)
            .bind(preview_id)
            .execute(&self.pool)
            .await?
            .rows_affected();
        if changed == 0 {
            // Preview (or this file) was cancelled/deleted mid-flight (D4):
            // there is nothing left to mark, and no artifact was produced.
            return Ok(());
        }
        self.refresh_preview_status(preview_id).await
    }

    pub async fn write_preview_file_ready(
        &self,
        preview_id: &str,
        file_id: &str,
        markdown: &[u8],
        mapping_bytes: &[u8],
        masked_entity_count: usize,
        artifact_id: &str,
    ) -> Result<(), StoreError> {
        // If the preview (or this file) was cancelled mid-flight, discard the
        // result instead of writing a confirmable artifact for a session that
        // no longer exists (D4).
        let exists: Option<i64> =
            sqlx::query_scalar("SELECT 1 FROM preview_files WHERE id = ? AND preview_id = ?")
                .bind(file_id)
                .bind(preview_id)
                .fetch_optional(&self.pool)
                .await?;
        if exists.is_none() {
            return Ok(());
        }

        let markdown_key = format!("preview/{preview_id}/markdown/{artifact_id}.md");
        let markdown_path = self.controlled_path(&markdown_key)?;
        fs::create_dir_all(markdown_path.parent().ok_or(StoreError::Storage)?).await?;
        let tmp_md = self.controlled_path(&format!("tmp/{}.tmp", Uuid::new_v4()))?;
        fs::write(&tmp_md, markdown).await?;
        if let Err(e) = fs::rename(&tmp_md, &markdown_path).await {
            let _ = fs::remove_file(&tmp_md).await;
            return Err(e.into());
        }

        let mapping_key = format!("preview/{preview_id}/mapping/{artifact_id}.cmap");
        let mapping_path = self.controlled_path(&mapping_key)?;
        fs::create_dir_all(mapping_path.parent().ok_or(StoreError::Storage)?).await?;
        let tmp_map = self.controlled_path(&format!("tmp/{}.cmap.tmp", Uuid::new_v4()))?;
        fs::write(&tmp_map, mapping_bytes).await?;
        if let Err(e) = fs::rename(&tmp_map, &mapping_path).await {
            let _ = fs::remove_file(&tmp_map).await;
            let _ = fs::remove_file(&markdown_path).await;
            return Err(e.into());
        }

        let now = Utc::now().to_rfc3339();
        let changed = sqlx::query("UPDATE preview_files SET status = ?, masked_entity_count = ?, artifact_id = ?, markdown_object_key = ?, mapping_object_key = ?, error_code = NULL, error_message = NULL, updated_at = ? WHERE id = ? AND preview_id = ? AND status = ?")
            .bind(PreviewFileStatus::Ready.as_str())
            .bind(masked_entity_count as i64)
            .bind(artifact_id)
            .bind(&markdown_key)
            .bind(&mapping_key)
            .bind(&now)
            .bind(file_id)
            .bind(preview_id)
            .bind(PreviewFileStatus::Processing.as_str())
            .execute(&self.pool)
            .await?
            .rows_affected();
        if changed == 0 {
            // Cancelled between the existence check above and this write.
            let _ = fs::remove_file(&markdown_path).await;
            let _ = fs::remove_file(&mapping_path).await;
            return Ok(());
        }
        self.refresh_preview_status(preview_id).await
    }

    async fn compute_preview_status(
        &self,
        preview_id: &str,
    ) -> Result<PreviewSessionStatus, StoreError> {
        let row = sqlx::query("SELECT COUNT(*) total, SUM(CASE WHEN status = ? THEN 1 ELSE 0 END) pending_count, SUM(CASE WHEN status = ? THEN 1 ELSE 0 END) processing_count, SUM(CASE WHEN status = ? THEN 1 ELSE 0 END) ready_count, SUM(CASE WHEN status = ? THEN 1 ELSE 0 END) failed_count FROM preview_files WHERE preview_id = ?")
            .bind(PreviewFileStatus::Pending.as_str())
            .bind(PreviewFileStatus::Processing.as_str())
            .bind(PreviewFileStatus::Ready.as_str())
            .bind(PreviewFileStatus::Failed.as_str())
            .bind(preview_id)
            .fetch_one(&self.pool)
            .await?;
        let total: i64 = row.get("total");
        let pending: i64 = row.get("pending_count");
        let processing: i64 = row.get("processing_count");
        let ready: i64 = row.get("ready_count");
        let failed: i64 = row.get("failed_count");
        Ok(if pending > 0 || processing > 0 {
            PreviewSessionStatus::Processing
        } else if total > 0 && ready == total {
            PreviewSessionStatus::Ready
        } else if ready > 0 && failed > 0 {
            PreviewSessionStatus::ReadyWithErrors
        } else {
            PreviewSessionStatus::Failed
        })
    }

    /// Recompute and persist the aggregate preview status from its files.
    /// Never overwrites `Confirming`/`Confirmed`, which are exclusively
    /// owned by [`Self::confirm_preview`]. No-ops silently if the preview no
    /// longer exists (cancelled mid-flight).
    async fn refresh_preview_status(&self, preview_id: &str) -> Result<(), StoreError> {
        let current: Option<String> =
            sqlx::query_scalar("SELECT status FROM previews WHERE id = ?")
                .bind(preview_id)
                .fetch_optional(&self.pool)
                .await?;
        let Some(current) = current else {
            return Ok(());
        };
        if current == PreviewSessionStatus::Confirming.as_str()
            || current == PreviewSessionStatus::Confirmed.as_str()
        {
            return Ok(());
        }
        let status = self.compute_preview_status(preview_id).await?;
        sqlx::query("UPDATE previews SET status = ?, updated_at = ? WHERE id = ?")
            .bind(status.as_str())
            .bind(Utc::now().to_rfc3339())
            .bind(preview_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn preview_detail(&self, preview_id: &str) -> Result<PreviewDetail, StoreError> {
        let row = sqlx::query("SELECT status, created_at, expires_at FROM previews WHERE id = ?")
            .bind(preview_id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or(StoreError::NotFound)?;
        let status_value: String = row.get("status");
        let status = PreviewSessionStatus::parse(&status_value).ok_or(StoreError::InvalidState)?;
        let created_at: String = row.get("created_at");
        let expires_at: String = row.get("expires_at");

        let file_rows = sqlx::query("SELECT id, display_name, input_format, status, masked_entity_count, markdown_object_key, error_code, error_message FROM preview_files WHERE preview_id = ? ORDER BY created_at, id")
            .bind(preview_id)
            .fetch_all(&self.pool)
            .await?;
        let mut ready_count = 0usize;
        let mut failed_count = 0usize;
        let mut masked_entity_count = 0usize;
        let files = file_rows
            .into_iter()
            .map(|row| {
                let file_status_value: String = row.get("status");
                let file_status =
                    PreviewFileStatus::parse(&file_status_value).ok_or(StoreError::InvalidState)?;
                let markdown_key: Option<String> = row.get("markdown_object_key");
                let count: Option<i64> = row.get("masked_entity_count");
                if file_status == PreviewFileStatus::Ready {
                    ready_count += 1;
                    masked_entity_count += count.unwrap_or(0) as usize;
                }
                if file_status == PreviewFileStatus::Failed {
                    failed_count += 1;
                }
                Ok(PreviewFile {
                    file_id: row.get("id"),
                    display_name: row.get("display_name"),
                    input_format: row.get("input_format"),
                    status: file_status,
                    masked_entity_count: count.map(|value| value as usize),
                    error_code: row.get("error_code"),
                    error_message: row.get("error_message"),
                    content_available: markdown_key.is_some(),
                })
            })
            .collect::<Result<Vec<_>, StoreError>>()?;

        Ok(PreviewDetail {
            preview_id: preview_id.to_string(),
            status,
            file_count: files.len(),
            ready_count,
            failed_count,
            masked_entity_count,
            created_at,
            expires_at,
            files,
        })
    }

    /// Read masked Markdown for one preview file. Only serves files that
    /// belong to the requested `preview_id` AND are `Ready` (C2/C4/D5) — a
    /// cross-session, Pending/Processing/Failed, or unknown `file_id` is
    /// rejected before any file read happens.
    pub async fn preview_file_content(
        &self,
        preview_id: &str,
        file_id: &str,
    ) -> Result<String, StoreError> {
        let row = sqlx::query(
            "SELECT status, markdown_object_key FROM preview_files WHERE id = ? AND preview_id = ?",
        )
        .bind(file_id)
        .bind(preview_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(StoreError::NotFound)?;
        let status_value: String = row.get("status");
        if status_value != PreviewFileStatus::Ready.as_str() {
            return Err(StoreError::NotFound);
        }
        let key: Option<String> = row.get("markdown_object_key");
        let key = key.ok_or(StoreError::NotFound)?;
        let bytes = fs::read(self.controlled_path(&key)?).await?;
        String::from_utf8(bytes).map_err(|_| StoreError::InvalidState)
    }

    /// Cancel an unconfirmed preview session: deletes its DB rows (cascading
    /// to `preview_files`) and its temp directory. Row deletion itself is
    /// the cancellation signal a still-running worker task re-checks before
    /// writing any result (D1/D4/D8) — no separate "cancelled" flag needed.
    /// Rejects deleting an already-`Confirmed` session so a real batch can
    /// never be removed through this path.
    pub async fn delete_preview(&self, preview_id: &str) -> Result<(), StoreError> {
        let status: Option<String> = sqlx::query_scalar("SELECT status FROM previews WHERE id = ?")
            .bind(preview_id)
            .fetch_optional(&self.pool)
            .await?;
        let Some(status) = status else {
            return Err(StoreError::NotFound);
        };
        if status == PreviewSessionStatus::Confirmed.as_str() {
            return Err(StoreError::PreviewAlreadyConfirmed);
        }
        sqlx::query("DELETE FROM previews WHERE id = ?")
            .bind(preview_id)
            .execute(&self.pool)
            .await?;
        let dir = self.root.join("preview").join(preview_id);
        let _ = fs::remove_dir_all(dir).await;
        Ok(())
    }

    /// Delete every preview session whose `expires_at` has passed, skipping
    /// any session currently `Confirming` or already `Confirmed` (D2).
    pub async fn sweep_expired_previews(&self) -> Result<usize, StoreError> {
        let now = Utc::now().to_rfc3339();
        let rows =
            sqlx::query("SELECT id FROM previews WHERE expires_at < ? AND status NOT IN (?, ?)")
                .bind(&now)
                .bind(PreviewSessionStatus::Confirming.as_str())
                .bind(PreviewSessionStatus::Confirmed.as_str())
                .fetch_all(&self.pool)
                .await?;
        let mut count = 0usize;
        for row in rows {
            let id: String = row.get("id");
            sqlx::query("DELETE FROM previews WHERE id = ?")
                .bind(&id)
                .execute(&self.pool)
                .await?;
            let dir = self.root.join("preview").join(&id);
            let _ = fs::remove_dir_all(dir).await;
            count += 1;
        }
        Ok(count)
    }

    /// Unconditionally wipe every preview session and the entire `preview/`
    /// directory. Called once at Runtime startup (D3) — preview sessions
    /// never survive a restart, regardless of status or TTL.
    pub async fn wipe_all_previews(&self) -> Result<(), StoreError> {
        sqlx::query("DELETE FROM previews")
            .execute(&self.pool)
            .await?;
        let dir = self.root.join("preview");
        if fs::try_exists(&dir).await.unwrap_or(false) {
            fs::remove_dir_all(&dir).await?;
        }
        Ok(())
    }

    /// Atomically confirm a preview session into a real batch (E1-E6).
    ///
    /// The `confirming` flag is flipped `0 -> 1` by a single `UPDATE ...
    /// WHERE confirming = 0`, which SQLite serializes — only one concurrent
    /// caller can ever win that race, so at most one batch is ever created
    /// (E4). Callers that lose the race, or call again after a successful
    /// confirm, get [`ConfirmOutcome::InFlight`] or
    /// [`ConfirmOutcome::AlreadyConfirmed`] instead of a second batch.
    pub async fn confirm_preview(&self, preview_id: &str) -> Result<ConfirmOutcome, StoreError> {
        let now = Utc::now().to_rfc3339();
        let mut tx = self.pool.begin().await?;
        let changed = sqlx::query("UPDATE previews SET confirming = 1, status = ?, updated_at = ? WHERE id = ? AND confirming = 0 AND status IN (?, ?)")
            .bind(PreviewSessionStatus::Confirming.as_str())
            .bind(&now)
            .bind(preview_id)
            .bind(PreviewSessionStatus::Ready.as_str())
            .bind(PreviewSessionStatus::ReadyWithErrors.as_str())
            .execute(&mut *tx)
            .await?
            .rows_affected();
        if changed == 0 {
            tx.rollback().await?;
            let row = sqlx::query("SELECT status, confirmed_batch_id FROM previews WHERE id = ?")
                .bind(preview_id)
                .fetch_optional(&self.pool)
                .await?
                .ok_or(StoreError::NotFound)?;
            let status: String = row.get("status");
            let confirmed_batch_id: Option<String> = row.get("confirmed_batch_id");
            if let Some(batch_id) = confirmed_batch_id {
                return Ok(ConfirmOutcome::AlreadyConfirmed { batch_id });
            }
            if status == PreviewSessionStatus::Confirming.as_str() {
                return Ok(ConfirmOutcome::InFlight);
            }
            return Ok(ConfirmOutcome::NotReady);
        }
        tx.commit().await?;

        match self.confirm_preview_locked(preview_id).await {
            Ok(batch_id) => Ok(ConfirmOutcome::Confirmed { batch_id }),
            Err(err) => {
                // No half-finished batch/artifact/mapping may remain (E5);
                // any files already copied into the real batch dirs are
                // rolled back inside confirm_preview_locked itself. Here we
                // only need to release the confirming lock so a retry (or a
                // still-open browser tab) can try again.
                let _ = self.revert_confirming(preview_id).await;
                Err(err)
            }
        }
    }

    async fn revert_confirming(&self, preview_id: &str) -> Result<(), StoreError> {
        let status = self.compute_preview_status(preview_id).await?;
        sqlx::query("UPDATE previews SET status = ?, confirming = 0, updated_at = ? WHERE id = ?")
            .bind(status.as_str())
            .bind(Utc::now().to_rfc3339())
            .bind(preview_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn confirm_preview_locked(&self, preview_id: &str) -> Result<String, StoreError> {
        let preview_row = sqlx::query(
            "SELECT rules_json, sensitive_terms_snapshot_json FROM previews WHERE id = ?",
        )
        .bind(preview_id)
        .fetch_one(&self.pool)
        .await?;
        let rules_json: String = preview_row.get("rules_json");
        // Carry over the preview's own frozen snapshot verbatim (C4): confirm
        // must never take a fresh snapshot of the live sensitive-term library.
        let snapshot_json: Option<String> = preview_row.get("sensitive_terms_snapshot_json");
        let file_rows = sqlx::query("SELECT id, display_name, input_format, input_object_key, status, masked_entity_count, artifact_id, markdown_object_key, mapping_object_key, error_code, error_message FROM preview_files WHERE preview_id = ? ORDER BY created_at, id")
            .bind(preview_id)
            .fetch_all(&self.pool)
            .await?;

        let batch_id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        let mut copied_paths: Vec<PathBuf> = Vec::new();

        struct RealFile {
            file_id: String,
            display_name: String,
            input_format: String,
            input_object_key: String,
            status: FileStatus,
            masked_entity_count: Option<i64>,
            artifact_id: Option<String>,
            mapping_object_key: Option<String>,
            output_object_key: Option<String>,
            error_code: Option<String>,
            error_message: Option<String>,
        }

        let mut real_files: Vec<RealFile> = Vec::with_capacity(file_rows.len());
        let copy_result: Result<(), StoreError> = async {
            for row in &file_rows {
                let file_id: String = row.get("id");
                let display_name: String = row.get("display_name");
                let input_format: String = row.get("input_format");
                let preview_input_key: String = row.get("input_object_key");
                let status_value: String = row.get("status");
                let masked_entity_count: Option<i64> = row.get("masked_entity_count");
                let artifact_id: Option<String> = row.get("artifact_id");
                let preview_markdown_key: Option<String> = row.get("markdown_object_key");
                let preview_mapping_key: Option<String> = row.get("mapping_object_key");
                let error_code: Option<String> = row.get("error_code");
                let error_message: Option<String> = row.get("error_message");

                // Always copy the original input into the real batch's input
                // dir so Failed files keep working with the existing retry
                // endpoint after the preview/ temp copy is gone.
                let real_input_key = format!("input/{batch_id}/{file_id}.input");
                let real_input_path = self.controlled_path(&real_input_key)?;
                fs::create_dir_all(real_input_path.parent().ok_or(StoreError::Storage)?).await?;
                fs::copy(self.controlled_path(&preview_input_key)?, &real_input_path).await?;
                copied_paths.push(real_input_path);

                let (real_status, real_mapping_key, real_output_key) =
                    if status_value == PreviewFileStatus::Ready.as_str() {
                        let artifact_id = artifact_id.clone().ok_or(StoreError::InvalidState)?;
                        let markdown_key = preview_markdown_key
                            .clone()
                            .ok_or(StoreError::InvalidState)?;
                        let mapping_key = preview_mapping_key
                            .clone()
                            .ok_or(StoreError::InvalidState)?;

                        let real_output_key = format!("output/{batch_id}/{artifact_id}.md");
                        let real_output_path = self.controlled_path(&real_output_key)?;
                        fs::create_dir_all(real_output_path.parent().ok_or(StoreError::Storage)?)
                            .await?;
                        fs::copy(self.controlled_path(&markdown_key)?, &real_output_path).await?;
                        copied_paths.push(real_output_path);

                        let real_mapping_key = format!("mapping/{batch_id}/{artifact_id}.cmap");
                        let real_mapping_path = self.controlled_path(&real_mapping_key)?;
                        fs::create_dir_all(real_mapping_path.parent().ok_or(StoreError::Storage)?)
                            .await?;
                        fs::copy(self.controlled_path(&mapping_key)?, &real_mapping_path).await?;
                        copied_paths.push(real_mapping_path);

                        (
                            FileStatus::Completed,
                            Some(real_mapping_key),
                            Some(real_output_key),
                        )
                    } else {
                        (FileStatus::Failed, None, None)
                    };

                real_files.push(RealFile {
                    file_id,
                    display_name,
                    input_format,
                    input_object_key: real_input_key,
                    status: real_status,
                    masked_entity_count,
                    artifact_id,
                    mapping_object_key: real_mapping_key,
                    output_object_key: real_output_key,
                    error_code,
                    error_message,
                });
            }
            Ok(())
        }
        .await;

        if let Err(err) = copy_result {
            for path in &copied_paths {
                let _ = fs::remove_file(path).await;
            }
            return Err(err);
        }

        let tx_result: Result<(), StoreError> = async {
            let mut tx = self.pool.begin().await?;
            sqlx::query("INSERT INTO batches (id, status, rules_json, restore_mode, sensitive_terms_snapshot_json, created_at, updated_at) VALUES (?, ?, ?, 'server_cmap', ?, ?, ?)")
                .bind(&batch_id)
                .bind(BatchStatus::Running.as_str())
                .bind(&rules_json)
                .bind(&snapshot_json)
                .bind(&now)
                .bind(&now)
                .execute(&mut *tx)
                .await?;
            for file in &real_files {
                sqlx::query("INSERT INTO batch_files (id, batch_id, display_name, input_format, input_object_key, mapping_object_key, status, attempt, masked_entity_count, artifact_id, error_code, error_message, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, 1, ?, ?, ?, ?, ?, ?)")
                    .bind(&file.file_id)
                    .bind(&batch_id)
                    .bind(&file.display_name)
                    .bind(&file.input_format)
                    .bind(&file.input_object_key)
                    .bind(&file.mapping_object_key)
                    .bind(file.status.as_str())
                    .bind(file.masked_entity_count)
                    .bind(&file.artifact_id)
                    .bind(&file.error_code)
                    .bind(&file.error_message)
                    .bind(&now)
                    .bind(&now)
                    .execute(&mut *tx)
                    .await?;
                // Operation-log projection (B1/E1): a confirmed preview file
                // was already fully processed during the preview phase, so
                // there is no real "start" timestamp to invent here — only
                // its genuine terminal outcome is recorded, at the real
                // moment it becomes a persisted batch file. This is the only
                // event this file will ever get from this code path; it is
                // deliberately not paired into the average-processing-time
                // statistic (§6), which only counts real ProcessingStarted→
                // terminal rounds and correctly skips this one rather than
                // inventing a duration for it (非目标 6).
                insert_event(
                    &mut tx,
                    &batch_id,
                    Some(&file.file_id),
                    file.status.as_str(),
                    file.status.as_str(),
                    file.error_code.as_deref(),
                    &now,
                )
                .await?;
                if file.status == FileStatus::Completed {
                    let artifact_id = file.artifact_id.as_ref().ok_or(StoreError::InvalidState)?;
                    let output_key = file.output_object_key.as_ref().ok_or(StoreError::InvalidState)?;
                    let size_bytes = self
                        .controlled_path(output_key)
                        .ok()
                        .and_then(|path| std::fs::metadata(path).ok())
                        .map(|meta| meta.len() as i64)
                        .unwrap_or(0);
                    sqlx::query("INSERT INTO artifacts (id, batch_id, file_id, object_key, size_bytes, created_at) VALUES (?, ?, ?, ?, ?, ?)")
                        .bind(artifact_id)
                        .bind(&batch_id)
                        .bind(&file.file_id)
                        .bind(output_key)
                        .bind(size_bytes)
                        .bind(&now)
                        .execute(&mut *tx)
                        .await?;
                }
            }
            sqlx::query("UPDATE previews SET status = ?, confirming = 0, confirmed_batch_id = ?, updated_at = ? WHERE id = ?")
                .bind(PreviewSessionStatus::Confirmed.as_str())
                .bind(&batch_id)
                .bind(&now)
                .bind(preview_id)
                .execute(&mut *tx)
                .await?;
            tx.commit().await?;
            Ok::<(), StoreError>(())
        }
        .await;

        if let Err(err) = tx_result {
            for path in &copied_paths {
                let _ = fs::remove_file(path).await;
            }
            let _ = fs::remove_dir_all(self.root.join("input").join(&batch_id)).await;
            let _ = fs::remove_dir_all(self.root.join("output").join(&batch_id)).await;
            let _ = fs::remove_dir_all(self.root.join("mapping").join(&batch_id)).await;
            return Err(err);
        }

        self.refresh_batch(&batch_id).await?;

        // Real data now lives permanently under input/output/mapping; the
        // preview's own temp copies are no longer needed (D1/E6).
        sqlx::query("DELETE FROM preview_files WHERE preview_id = ?")
            .bind(preview_id)
            .execute(&self.pool)
            .await?;
        let preview_dir = self.root.join("preview").join(preview_id);
        let _ = fs::remove_dir_all(preview_dir).await;

        Ok(batch_id)
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

    /// Browser operation-log projection (FR-007 minimal-exposure mode):
    /// a `UNION ALL` of `job_events` (left-joined with `batch_files` for the
    /// already-controlled `display_name`/`input_format`/`masked_entity_count`
    /// fields) and `restore_events` (which have no batch/file association in
    /// this schema, so those columns are `NULL` rather than guessed). `level`
    /// is computed here via the single fixed mapping (task §5) so an unknown
    /// historical `event_type` degrades to `'info'` instead of breaking the
    /// query (A5) — never stored, never left to the frontend to derive.
    const OPERATION_LOG_EVENTS_SQL: &'static str = "
        SELECT
            je.id AS event_id,
            je.event_type AS event_type,
            je.created_at AS timestamp,
            je.batch_id AS batch_id,
            je.file_id AS file_id,
            bf.display_name AS display_name,
            bf.input_format AS input_format,
            je.status AS status,
            bf.masked_entity_count AS masked_entity_count,
            je.error_code AS error_code,
            NULL AS restored_entity_count,
            CASE je.event_type
                WHEN 'Queued' THEN 'info'
                WHEN 'ProcessingStarted' THEN 'info'
                WHEN 'Completed' THEN 'success'
                WHEN 'RetryQueued' THEN 'warning'
                WHEN 'RecoveredInterrupted' THEN 'warning'
                WHEN 'Failed' THEN 'error'
                ELSE 'info'
            END AS level
        FROM job_events je
        LEFT JOIN batch_files bf ON bf.id = je.file_id
        UNION ALL
        SELECT
            re.id AS event_id,
            re.event_type AS event_type,
            re.timestamp AS timestamp,
            NULL AS batch_id,
            NULL AS file_id,
            NULL AS display_name,
            NULL AS input_format,
            re.status AS status,
            NULL AS masked_entity_count,
            re.error_code AS error_code,
            re.restored_entity_count AS restored_entity_count,
            CASE re.event_type
                WHEN 'RestoreSucceeded' THEN 'success'
                WHEN 'RestoreFailed' THEN 'error'
                ELSE 'info'
            END AS level
        FROM restore_events re
        UNION ALL
        SELECT
            fe.id AS event_id,
            'FileBayUpload' AS event_type,
            fe.created_at AS timestamp,
            fe.batch_id AS batch_id,
            fe.file_id AS file_id,
            fe.display_name AS display_name,
            NULL AS input_format,
            fe.status AS status,
            NULL AS masked_entity_count,
            fe.error_code AS error_code,
            NULL AS restored_entity_count,
            CASE fe.status
                WHEN 'success' THEN 'success'
                WHEN 'failed' THEN 'error'
                ELSE 'info'
            END AS level
        FROM filebay_events fe
    ";

    /// Stable, safety-net-bound paginated + filtered read of the operation
    /// log. Caller (the HTTP layer) has already validated `page`/`page_size`
    /// bounds, `level` against the known enum and the `batch_id` prefix
    /// charset (INVALID_QUERY, 8.4) — this method only binds parameters, it
    /// never concatenates untrusted input into SQL (safety 3).
    pub async fn list_operation_logs(
        &self,
        page: usize,
        page_size: usize,
        level: Option<&str>,
        status: Option<&str>,
        batch_id_prefix: Option<&str>,
    ) -> Result<(Vec<OperationLogEntry>, usize), StoreError> {
        let mut clauses: Vec<&str> = Vec::new();
        if level.is_some() {
            clauses.push("level = ?");
        }
        if status.is_some() {
            clauses.push("status = ?");
        }
        if batch_id_prefix.is_some() {
            clauses.push("batch_id LIKE ?");
        }
        let where_sql = if clauses.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", clauses.join(" AND "))
        };

        let count_sql = format!(
            "SELECT COUNT(*) FROM ({}) e {where_sql}",
            Self::OPERATION_LOG_EVENTS_SQL
        );
        let mut count_query = sqlx::query_scalar::<_, i64>(&count_sql);
        if let Some(level) = level {
            count_query = count_query.bind(level);
        }
        if let Some(status) = status {
            count_query = count_query.bind(status);
        }
        if let Some(prefix) = batch_id_prefix {
            count_query = count_query.bind(format!("{prefix}%"));
        }
        let total_count = count_query.fetch_one(&self.pool).await?;

        let data_sql = format!(
            "SELECT * FROM ({}) e {where_sql} ORDER BY timestamp DESC, event_id DESC LIMIT ? OFFSET ?",
            Self::OPERATION_LOG_EVENTS_SQL
        );
        let mut data_query = sqlx::query(&data_sql);
        if let Some(level) = level {
            data_query = data_query.bind(level);
        }
        if let Some(status) = status {
            data_query = data_query.bind(status);
        }
        if let Some(prefix) = batch_id_prefix {
            data_query = data_query.bind(format!("{prefix}%"));
        }
        let offset = page.saturating_sub(1) * page_size;
        data_query = data_query.bind(page_size as i64).bind(offset as i64);
        let rows = data_query.fetch_all(&self.pool).await?;

        let entries = rows
            .into_iter()
            .map(|row| {
                let level_str: String = row.get("level");
                let masked_entity_count: Option<i64> = row.get("masked_entity_count");
                let restored_entity_count: Option<i64> = row.get("restored_entity_count");
                OperationLogEntry {
                    event_id: row.get("event_id"),
                    event_type: row.get("event_type"),
                    timestamp: row.get("timestamp"),
                    level: OperationLogLevel::parse(&level_str).unwrap_or(OperationLogLevel::Info),
                    batch_id: row.get("batch_id"),
                    file_id: row.get("file_id"),
                    display_name: row.get("display_name"),
                    input_format: row.get("input_format"),
                    status: row.get("status"),
                    masked_entity_count: masked_entity_count.map(|value| value as usize),
                    error_code: row.get("error_code"),
                    restored_entity_count: restored_entity_count.map(|value| value as usize),
                }
            })
            .collect();

        Ok((entries, total_count as usize))
    }

    /// Processing statistics computed directly from `batches`/`batch_files`/
    /// `job_events` at query time (task §6) — never cached, never derived
    /// from the currently-loaded log page, so it naturally reflects a
    /// cleared event log (average time returns to 0, C4) without any
    /// special-cased "cleared" branch.
    pub async fn operation_log_statistics(&self) -> Result<OperationLogStatistics, StoreError> {
        let total_files: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM batch_files")
            .fetch_one(&self.pool)
            .await?;
        let successful_files: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM batch_files WHERE status = ?")
                .bind(FileStatus::Completed.as_str())
                .fetch_one(&self.pool)
                .await?;
        let failed_files: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM batch_files WHERE status = ?")
                .bind(FileStatus::Failed.as_str())
                .fetch_one(&self.pool)
                .await?;
        let total_masked_items: i64 = sqlx::query_scalar(
            "SELECT COALESCE(SUM(masked_entity_count), 0) FROM batch_files WHERE status = ?",
        )
        .bind(FileStatus::Completed.as_str())
        .fetch_one(&self.pool)
        .await?;
        let seven_days_ago = (Utc::now() - chrono::Duration::days(7)).to_rfc3339();
        let recent_files_7days: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM batch_files WHERE created_at >= ?")
                .bind(&seven_days_ago)
                .fetch_one(&self.pool)
                .await?;

        let terminal = successful_files + failed_files;
        let success_rate = if terminal > 0 {
            (successful_files as f64 / terminal as f64) * 100.0
        } else {
            0.0
        };

        // Pair each round's `ProcessingStarted` with the *immediately
        // following* job_event for that same file (via `LEAD`), then keep
        // only pairs whose next event is a genuine terminal `Completed`/
        // `Failed` (task §6: "retry 以每轮开始和其后首个终态事件配对").
        // A `RecoveredInterrupted` landing immediately after a start is
        // deliberately excluded — pairing it would count Runtime downtime
        // as processing time, which is exactly the "队列等待时间或批次总
        // 时长冒充" the task forbids.
        let rows = sqlx::query(
            "SELECT created_at, next_event_time FROM (
                SELECT event_type, created_at,
                       LEAD(event_type) OVER (PARTITION BY file_id ORDER BY created_at, id) AS next_event_type,
                       LEAD(created_at) OVER (PARTITION BY file_id ORDER BY created_at, id) AS next_event_time
                FROM job_events
            ) WHERE event_type = 'ProcessingStarted' AND next_event_type IN ('Completed', 'Failed')",
        )
        .fetch_all(&self.pool)
        .await?;

        let mut total_ms: i64 = 0;
        let mut sample_count: i64 = 0;
        for row in rows {
            let start: String = row.get("created_at");
            let end: String = row.get("next_event_time");
            if let (Ok(start_dt), Ok(end_dt)) = (
                chrono::DateTime::parse_from_rfc3339(&start),
                chrono::DateTime::parse_from_rfc3339(&end),
            ) {
                let delta = end_dt.timestamp_millis() - start_dt.timestamp_millis();
                if delta >= 0 {
                    total_ms += delta;
                    sample_count += 1;
                }
            }
        }
        let average_processing_time_ms = if sample_count > 0 {
            (total_ms / sample_count) as u64
        } else {
            0
        };

        Ok(OperationLogStatistics {
            total_files: total_files as usize,
            successful_files: successful_files as usize,
            failed_files: failed_files as usize,
            total_masked_items: total_masked_items as usize,
            success_rate,
            recent_files_7days: recent_files_7days as usize,
            average_processing_time_ms,
        })
    }

    /// Safe storage-status projection (task §7): only a ready/error flag,
    /// the current combined event count and the Runtime's own version —
    /// never a database path, table name or host username.
    pub async fn operation_log_storage_status(
        &self,
    ) -> Result<OperationLogStorageStatus, StoreError> {
        let job_events: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM job_events")
            .fetch_one(&self.pool)
            .await?;
        let restore_events: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM restore_events")
            .fetch_one(&self.pool)
            .await?;
        Ok(OperationLogStorageStatus {
            status: "ready".to_string(),
            event_count: (job_events + restore_events) as usize,
            runtime_version: crate::VERSION.to_string(),
        })
    }

    /// Clears only the two event-log tables in a single transaction (task
    /// §8/C5): a failure here rolls back automatically (the transaction is
    /// simply dropped without `commit()`), so callers can never observe a
    /// partial clear or a false "cleared" success. Never touches `batches`,
    /// `batch_files`, `artifacts`, `previews`, `preview_files` or
    /// `sensitive_terms` (C3).
    pub async fn clear_operation_logs(&self) -> Result<(usize, usize), StoreError> {
        let mut tx = self.pool.begin().await?;
        let deleted_job_events = sqlx::query("DELETE FROM job_events")
            .execute(&mut *tx)
            .await?
            .rows_affected();
        let deleted_restore_events = sqlx::query("DELETE FROM restore_events")
            .execute(&mut *tx)
            .await?
            .rows_affected();
        tx.commit().await?;
        Ok((deleted_job_events as usize, deleted_restore_events as usize))
    }

    #[cfg(test)]
    pub async fn drop_operation_log_tables_for_test(&self) -> Result<(), StoreError> {
        sqlx::query("DROP TABLE job_events")
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
        let row = sqlx::query(
            "SELECT COUNT(*) count FROM job_events WHERE file_id = ? AND event_type = ?",
        )
        .bind(file_id)
        .bind(event_type)
        .fetch_one(&self.pool)
        .await?;
        Ok(row.get::<i64, _>("count") as usize)
    }

    /// Test-only: insert a `job_events` row with an explicit, caller-chosen
    /// `event_type`/`status`/`created_at` — used to build deterministic
    /// multi-round processing-time fixtures and unknown-event-type (A5)
    /// fixtures without racing real background-worker timing.
    #[cfg(test)]
    pub async fn insert_raw_job_event(
        &self,
        batch_id: &str,
        file_id: &str,
        event_type: &str,
        status: &str,
        created_at: &str,
    ) -> Result<(), StoreError> {
        sqlx::query("INSERT INTO job_events (id, batch_id, file_id, event_type, status, error_code, created_at) VALUES (?, ?, ?, ?, ?, NULL, ?)")
            .bind(Uuid::new_v4().to_string())
            .bind(batch_id)
            .bind(file_id)
            .bind(event_type)
            .bind(status)
            .bind(created_at)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Test-only: read a batch's frozen `sensitive_terms_snapshot_json`
    /// column directly, to prove immutability (C2/C3) at the persistence
    /// layer without needing a full worker retry cycle.
    #[cfg(test)]
    pub async fn sensitive_terms_snapshot_json_for_batch(
        &self,
        batch_id: &str,
    ) -> Result<Option<String>, StoreError> {
        let value: Option<String> =
            sqlx::query_scalar("SELECT sensitive_terms_snapshot_json FROM batches WHERE id = ?")
                .bind(batch_id)
                .fetch_one(&self.pool)
                .await?;
        Ok(value)
    }

    // -----------------------------------------------------------------
    // Sensitive-term library (browser CRUD + CSV import/export)
    // -----------------------------------------------------------------

    pub async fn create_sensitive_term(
        &self,
        term: &str,
        category: &str,
        description: Option<&str>,
    ) -> Result<SensitiveTerm, StoreError> {
        let (term, category, description) =
            validate_sensitive_term_fields(term, category, description)?;
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();

        let mut tx = self.pool.begin().await?;
        let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sensitive_terms")
            .fetch_one(&mut *tx)
            .await?;
        if total as usize >= SENSITIVE_TERMS_MAX_COUNT {
            tx.rollback().await?;
            return Err(StoreError::InputLimitExceeded);
        }
        let duplicate: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM sensitive_terms WHERE term = ?")
                .bind(&term)
                .fetch_one(&mut *tx)
                .await?;
        if duplicate > 0 {
            tx.rollback().await?;
            return Err(StoreError::SensitiveTermDuplicate);
        }
        sqlx::query("INSERT INTO sensitive_terms (id, term, category, description, enabled, created_at, updated_at) VALUES (?, ?, ?, ?, 1, ?, ?)")
            .bind(&id)
            .bind(&term)
            .bind(&category)
            .bind(&description)
            .bind(&now)
            .bind(&now)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;

        Ok(SensitiveTerm {
            id,
            term,
            category,
            description,
            enabled: true,
            created_at: now.clone(),
            updated_at: now,
        })
    }

    /// Partial update: `None` for `term`/`category`/`description`/`enabled`
    /// leaves that field unchanged, matching the desktop `update_sensitive_term`
    /// command's semantics exactly (4.1).
    pub async fn update_sensitive_term(
        &self,
        id: &str,
        term: Option<&str>,
        category: Option<&str>,
        description: Option<&str>,
        enabled: Option<bool>,
    ) -> Result<SensitiveTerm, StoreError> {
        let mut tx = self.pool.begin().await?;
        let row = sqlx::query(
            "SELECT id, term, category, description, enabled, created_at, updated_at FROM sensitive_terms WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?;
        let Some(row) = row else {
            tx.rollback().await?;
            return Err(StoreError::SensitiveTermNotFound);
        };
        let existing = sensitive_term_from_row(row);

        let new_term = match term {
            Some(value) => normalize_sensitive_term_key(value),
            None => existing.term.clone(),
        };
        let new_category = match category {
            Some(value) => normalize_sensitive_term_key(value),
            None => existing.category.clone(),
        };
        let new_description = match description {
            Some(value) => Some(value.trim().to_string()),
            None => existing.description.clone(),
        };
        let new_enabled = enabled.unwrap_or(existing.enabled);

        if let Err(error) =
            validate_sensitive_term_lengths(&new_term, &new_category, new_description.as_deref())
        {
            tx.rollback().await?;
            return Err(error);
        }

        let duplicate: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM sensitive_terms WHERE term = ? AND id != ?")
                .bind(&new_term)
                .bind(id)
                .fetch_one(&mut *tx)
                .await?;
        if duplicate > 0 {
            tx.rollback().await?;
            return Err(StoreError::SensitiveTermDuplicate);
        }

        let now = Utc::now().to_rfc3339();
        sqlx::query("UPDATE sensitive_terms SET term = ?, category = ?, description = ?, enabled = ?, updated_at = ? WHERE id = ?")
            .bind(&new_term)
            .bind(&new_category)
            .bind(&new_description)
            .bind(new_enabled)
            .bind(&now)
            .bind(id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;

        Ok(SensitiveTerm {
            id: id.to_string(),
            term: new_term,
            category: new_category,
            description: new_description,
            enabled: new_enabled,
            created_at: existing.created_at,
            updated_at: now,
        })
    }

    pub async fn delete_sensitive_term(&self, id: &str) -> Result<(), StoreError> {
        let deleted = sqlx::query("DELETE FROM sensitive_terms WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?
            .rows_affected();
        if deleted == 0 {
            return Err(StoreError::SensitiveTermNotFound);
        }
        Ok(())
    }

    pub async fn list_sensitive_terms(
        &self,
        category: Option<&str>,
        query: Option<&str>,
        enabled_only: bool,
    ) -> Result<Vec<SensitiveTerm>, StoreError> {
        let query = query.map(str::trim).filter(|value| !value.is_empty());
        let mut sql = String::from(
            "SELECT id, term, category, description, enabled, created_at, updated_at FROM sensitive_terms WHERE 1 = 1",
        );
        if category.is_some() {
            sql.push_str(" AND category = ?");
        }
        if query.is_some() {
            sql.push_str(" AND (term LIKE ? ESCAPE '\\' OR category LIKE ? ESCAPE '\\')");
        }
        if enabled_only {
            sql.push_str(" AND enabled = 1");
        }
        sql.push_str(" ORDER BY created_at DESC, id");

        let mut builder = sqlx::query(&sql);
        if let Some(category) = category {
            builder = builder.bind(category);
        }
        let like_pattern = query.map(|value| format!("%{}%", escape_like(value)));
        if let Some(pattern) = &like_pattern {
            builder = builder.bind(pattern).bind(pattern);
        }

        let rows = builder.fetch_all(&self.pool).await?;
        Ok(rows.into_iter().map(sensitive_term_from_row).collect())
    }

    pub async fn sensitive_term_categories(&self) -> Result<Vec<String>, StoreError> {
        let categories = sqlx::query_scalar::<_, String>(
            "SELECT DISTINCT category FROM sensitive_terms ORDER BY category",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(categories)
    }

    pub async fn sensitive_terms_stats(&self) -> Result<SensitiveTermsStats, StoreError> {
        let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sensitive_terms")
            .fetch_one(&self.pool)
            .await?;
        let enabled: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM sensitive_terms WHERE enabled = 1")
                .fetch_one(&self.pool)
                .await?;
        let categories: i64 =
            sqlx::query_scalar("SELECT COUNT(DISTINCT category) FROM sensitive_terms")
                .fetch_one(&self.pool)
                .await?;
        Ok(SensitiveTermsStats {
            total: total as usize,
            enabled: enabled as usize,
            disabled: (total - enabled) as usize,
            categories: categories as usize,
        })
    }

    /// Transactional bulk import: `rows` is already parsed and CSV-decoded by
    /// the caller (`sensitive_terms.rs`, which owns the `csv` crate usage).
    /// Any duplicate or invalid row fails the *entire* import — no partial
    /// success (3.7).
    pub async fn import_sensitive_terms(
        &self,
        rows: Vec<(String, String, Option<String>, bool)>,
    ) -> Result<usize, StoreError> {
        if rows.is_empty() {
            return Err(StoreError::SensitiveTermsImportInvalid(
                "CSV 中没有可导入的数据行".to_string(),
            ));
        }
        if rows.len() > SENSITIVE_TERMS_IMPORT_MAX_ROWS {
            return Err(StoreError::InputLimitExceeded);
        }

        let mut normalized = Vec::with_capacity(rows.len());
        let mut seen = std::collections::HashSet::new();
        for (category, term, description, enabled) in rows {
            let (term, category, description) =
                validate_sensitive_term_fields(&term, &category, description.as_deref()).map_err(
                    |error| match error {
                        StoreError::SensitiveTermInvalid(message) => {
                            StoreError::SensitiveTermsImportInvalid(message)
                        }
                        other => other,
                    },
                )?;
            if !seen.insert(term.clone()) {
                return Err(StoreError::SensitiveTermsImportInvalid(format!(
                    "CSV 中存在重复敏感词「{term}」"
                )));
            }
            normalized.push((term, category, description, enabled));
        }

        let mut tx = self.pool.begin().await?;
        let existing_total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sensitive_terms")
            .fetch_one(&mut *tx)
            .await?;
        if existing_total as usize + normalized.len() > SENSITIVE_TERMS_MAX_COUNT {
            tx.rollback().await?;
            return Err(StoreError::InputLimitExceeded);
        }
        for (term, _, _, _) in &normalized {
            let duplicate: i64 =
                sqlx::query_scalar("SELECT COUNT(*) FROM sensitive_terms WHERE term = ?")
                    .bind(term)
                    .fetch_one(&mut *tx)
                    .await?;
            if duplicate > 0 {
                tx.rollback().await?;
                return Err(StoreError::SensitiveTermsImportInvalid(format!(
                    "敏感词「{term}」已存在于词库中"
                )));
            }
        }

        let now = Utc::now().to_rfc3339();
        for (term, category, description, enabled) in &normalized {
            let id = Uuid::new_v4().to_string();
            sqlx::query("INSERT INTO sensitive_terms (id, term, category, description, enabled, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?)")
                .bind(&id)
                .bind(term)
                .bind(category)
                .bind(description)
                .bind(*enabled)
                .bind(&now)
                .bind(&now)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(normalized.len())
    }
}

fn normalize_sensitive_term_key(value: &str) -> String {
    value.trim().to_string()
}

fn validate_sensitive_term_lengths(
    term: &str,
    category: &str,
    description: Option<&str>,
) -> Result<(), StoreError> {
    if term.is_empty() || category.is_empty() {
        return Err(StoreError::SensitiveTermInvalid(
            "敏感词和分类不能为空".to_string(),
        ));
    }
    if term.chars().count() > SENSITIVE_TERM_MAX_CHARS {
        return Err(StoreError::SensitiveTermInvalid(format!(
            "敏感词长度不能超过 {SENSITIVE_TERM_MAX_CHARS} 个字符"
        )));
    }
    if category.chars().count() > SENSITIVE_TERM_CATEGORY_MAX_CHARS {
        return Err(StoreError::SensitiveTermInvalid(format!(
            "分类长度不能超过 {SENSITIVE_TERM_CATEGORY_MAX_CHARS} 个字符"
        )));
    }
    if let Some(description) = description {
        if description.chars().count() > SENSITIVE_TERM_DESCRIPTION_MAX_CHARS {
            return Err(StoreError::SensitiveTermInvalid(format!(
                "描述长度不能超过 {SENSITIVE_TERM_DESCRIPTION_MAX_CHARS} 个字符"
            )));
        }
    }
    Ok(())
}

/// Trim + validate a freshly-submitted term/category/description triple
/// (create path and CSV import path). Update uses
/// [`validate_sensitive_term_lengths`] directly since it must merge with
/// existing field values first.
fn validate_sensitive_term_fields(
    term: &str,
    category: &str,
    description: Option<&str>,
) -> Result<(String, String, Option<String>), StoreError> {
    let term = normalize_sensitive_term_key(term);
    let category = normalize_sensitive_term_key(category);
    let description = description.map(|value| value.trim().to_string());
    validate_sensitive_term_lengths(&term, &category, description.as_deref())?;
    Ok((term, category, description))
}

fn escape_like(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

fn sensitive_term_from_row(row: sqlx::sqlite::SqliteRow) -> SensitiveTerm {
    SensitiveTerm {
        id: row.get("id"),
        term: row.get("term"),
        category: row.get("category"),
        description: row.get("description"),
        enabled: row.get::<i64, _>("enabled") != 0,
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
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
