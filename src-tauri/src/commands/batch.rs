use serde::{Deserialize, Serialize};
use crate::core::batch;
use crate::commands::masking::CustomRule;

#[derive(Debug, Serialize, Deserialize)]
pub struct BatchJobOptions {
    pub file_paths: Vec<String>,
    pub output_dir: String,
    pub rule_ids: Vec<String>,
    pub passphrase: Option<String>,
    pub custom_rules: Option<Vec<CustomRule>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BatchStatus {
    pub job_id: String,
    pub total: usize,
    pub completed: usize,
    pub failed: usize,
    pub status: String,
    pub current_file: Option<String>,
    pub error: Option<String>,
}

#[tauri::command]
pub async fn start_batch_job(options: BatchJobOptions) -> Result<String, String> {
    let job_id = batch::create_job(options.file_paths.len());
    
    // 启动后台任务处理文件
    let job_id_clone = job_id.clone();
    let options_clone = options;
    
    tokio::spawn(async move {
        batch::process_batch_job(job_id_clone, options_clone).await;
    });
    
    Ok(job_id)
}

#[tauri::command]
pub async fn get_batch_status(job_id: String) -> Result<BatchStatus, String> {
    match batch::get_job(&job_id) {
        Some(job) => Ok(BatchStatus {
            job_id: job_id.clone(),
            total: job.total,
            completed: job.completed,
            failed: job.failed,
            status: format!("{:?}", job.status),
            current_file: job.current_file.clone(),
            error: job.error.clone(),
        }),
        None => Err("Job not found".to_string()),
    }
}

#[tauri::command]
pub async fn cancel_batch_job(job_id: String) -> Result<(), String> {
    if batch::cancel_job(&job_id) {
        Ok(())
    } else {
        Err("Job not found or already completed".to_string())
    }
}
