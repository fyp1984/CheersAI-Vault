use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::commands::batch::BatchJobOptions;
use crate::commands::masking::{MaskFileOptions, mask_file};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum BatchJobStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchJob {
    pub id: String,
    pub total: usize,
    pub completed: usize,
    pub failed: usize,
    pub status: BatchJobStatus,
    pub current_file: Option<String>,
    pub error: Option<String>,
}

static JOBS: Lazy<Arc<Mutex<HashMap<String, BatchJob>>>> =
    Lazy::new(|| Arc::new(Mutex::new(HashMap::new())));

pub fn create_job(total: usize) -> String {
    let id = Uuid::new_v4().to_string();
    let job = BatchJob {
        id: id.clone(),
        total,
        completed: 0,
        failed: 0,
        status: BatchJobStatus::Pending,
        current_file: None,
        error: None,
    };
    JOBS.lock().unwrap().insert(id.clone(), job);
    id
}

pub fn get_job(id: &str) -> Option<BatchJob> {
    JOBS.lock().unwrap().get(id).cloned()
}

pub fn cancel_job(id: &str) -> bool {
    let mut jobs = JOBS.lock().unwrap();
    if let Some(job) = jobs.get_mut(id) {
        job.status = BatchJobStatus::Cancelled;
        true
    } else {
        false
    }
}

fn update_job<F>(id: &str, updater: F) 
where 
    F: FnOnce(&mut BatchJob),
{
    let mut jobs = JOBS.lock().unwrap();
    if let Some(job) = jobs.get_mut(id) {
        updater(job);
    }
}

pub async fn process_batch_job(job_id: String, options: BatchJobOptions) {
    use crate::core::database::Database;
    
    // 更新状态为运行中
    update_job(&job_id, |job| {
        job.status = BatchJobStatus::Running;
    });

    // 记录批处理开始日志
    if let Ok(db) = Database::new().await {
        let log_entry = crate::core::database::LogEntry {
            id: uuid::Uuid::new_v4().to_string(),
            timestamp: chrono::Utc::now(),
            level: "info".to_string(),
            message: format!("开始批处理任务，共 {} 个文件", options.file_paths.len()),
            details: Some(format!("输出目录: {}", options.output_dir)),
            file_path: None,
            operation_type: Some("batch_start".to_string()),
            user_id: None,
        };
        let _ = db.add_log(&log_entry).await;
    }

    // 确保输出目录存在
    if let Err(e) = std::fs::create_dir_all(&options.output_dir) {
        update_job(&job_id, |job| {
            job.status = BatchJobStatus::Failed;
            job.error = Some(format!("无法创建输出目录: {}", e));
        });
        
        // 记录错误日志
        if let Ok(db) = Database::new().await {
            let log_entry = crate::core::database::LogEntry {
                id: uuid::Uuid::new_v4().to_string(),
                timestamp: chrono::Utc::now(),
                level: "error".to_string(),
                message: "批处理任务失败".to_string(),
                details: Some(format!("无法创建输出目录: {}", e)),
                file_path: None,
                operation_type: Some("batch_error".to_string()),
                user_id: None,
            };
            let _ = db.add_log(&log_entry).await;
        }
        return;
    }

    let start_time = std::time::Instant::now();
    let mut successful_files = 0;
    let mut failed_files = 0;

    for file_path in &options.file_paths {
        // 检查是否被取消
        if let Some(job) = get_job(&job_id) {
            if matches!(job.status, BatchJobStatus::Cancelled) {
                return;
            }
        }

        // 更新当前处理的文件
        update_job(&job_id, |job| {
            job.current_file = Some(file_path.clone());
        });

        let file_start_time = std::time::Instant::now();

        // 安全地生成输出文件路径
        let input_path = Path::new(file_path);
        let file_name = input_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");
        
        // 清理文件名，确保跨平台兼容
        let safe_file_name = sanitize_filename(&format!("masked_{}", file_name));
        
        let output_path = Path::new(&options.output_dir)
            .join(safe_file_name)
            .to_string_lossy()
            .to_string();

        // 获取文件大小
        let file_size = std::fs::metadata(file_path)
            .map(|m| m.len() as i64)
            .unwrap_or(0);

        // 处理单个文件
        let mask_options = MaskFileOptions {
            file_path: file_path.clone(),
            output_path: output_path.clone(),
            rule_ids: options.rule_ids.clone(),
            passphrase: options.passphrase.clone(),
            custom_rules: options.custom_rules.clone(),
        };

        println!("Starting to process file: {}", file_path);
        println!("File size: {} bytes", file_size);
        println!("Output path: {}", output_path);
        println!("Selected rules: {:?}", options.rule_ids);
        
        match mask_file(mask_options).await {
            Ok(result) => {
                let processing_time = file_start_time.elapsed().as_millis() as i64;
                successful_files += 1;
                
                update_job(&job_id, |job| {
                    job.completed += 1;
                });
                
                println!("Successfully processed file: {} -> {}", file_path, result.output_path);
                
                // 记录成功日志和处理历史
                if let Ok(db) = Database::new().await {
                    // 添加日志
                    let log_entry = crate::core::database::LogEntry {
                        id: uuid::Uuid::new_v4().to_string(),
                        timestamp: chrono::Utc::now(),
                        level: "success".to_string(),
                        message: format!("文件处理成功，脱敏 {} 项", result.masked_count),
                        details: Some(format!("处理时间: {}ms", processing_time)),
                        file_path: Some(file_path.clone()),
                        operation_type: Some("file_mask".to_string()),
                        user_id: None,
                    };
                    let _ = db.add_log(&log_entry).await;
                    
                    // 添加处理历史
                    let history = crate::core::database::ProcessingHistory {
                        id: uuid::Uuid::new_v4().to_string(),
                        file_path: file_path.clone(),
                        output_path: result.output_path,
                        rule_ids: serde_json::to_string(&options.rule_ids).unwrap_or_default(),
                        file_size,
                        masked_count: result.masked_count as i32,
                        processing_time_ms: processing_time,
                        status: "success".to_string(),
                        error_message: None,
                        created_at: chrono::Utc::now(),
                    };
                    let _ = db.add_processing_history(&history).await;
                }
            }
            Err(e) => {
                let processing_time = file_start_time.elapsed().as_millis() as i64;
                failed_files += 1;
                
                update_job(&job_id, |job| {
                    job.failed += 1;
                    job.error = Some(format!("处理文件 {} 失败: {}", file_path, e));
                });
                
                eprintln!("Failed to process file {}: {}", file_path, e);
                
                // 记录失败日志和处理历史
                if let Ok(db) = Database::new().await {
                    // 添加日志
                    let log_entry = crate::core::database::LogEntry {
                        id: uuid::Uuid::new_v4().to_string(),
                        timestamp: chrono::Utc::now(),
                        level: "error".to_string(),
                        message: "文件处理失败".to_string(),
                        details: Some(format!("错误: {}", e)),
                        file_path: Some(file_path.clone()),
                        operation_type: Some("file_mask".to_string()),
                        user_id: None,
                    };
                    let _ = db.add_log(&log_entry).await;
                    
                    // 添加处理历史
                    let history = crate::core::database::ProcessingHistory {
                        id: uuid::Uuid::new_v4().to_string(),
                        file_path: file_path.clone(),
                        output_path: "".to_string(),
                        rule_ids: serde_json::to_string(&options.rule_ids).unwrap_or_default(),
                        file_size,
                        masked_count: 0,
                        processing_time_ms: processing_time,
                        status: "failed".to_string(),
                        error_message: Some(format!("{}", e)),
                        created_at: chrono::Utc::now(),
                    };
                    let _ = db.add_processing_history(&history).await;
                }
            }
        }
    }

    let total_time = start_time.elapsed().as_millis() as i64;

    // 更新最终状态
    update_job(&job_id, |job| {
        job.current_file = None;
        job.status = if job.failed > 0 {
            BatchJobStatus::Failed
        } else {
            BatchJobStatus::Completed
        };
    });

    // 记录批处理完成日志
    if let Ok(db) = Database::new().await {
        let log_entry = crate::core::database::LogEntry {
            id: uuid::Uuid::new_v4().to_string(),
            timestamp: chrono::Utc::now(),
            level: if failed_files > 0 { "warning" } else { "success" }.to_string(),
            message: format!(
                "批处理任务完成，成功 {} 个，失败 {} 个", 
                successful_files, failed_files
            ),
            details: Some(format!("总处理时间: {}ms", total_time)),
            file_path: None,
            operation_type: Some("batch_complete".to_string()),
            user_id: None,
        };
        let _ = db.add_log(&log_entry).await;
    }
}

// 跨平台文件名清理函数
fn sanitize_filename(filename: &str) -> String {
    let mut sanitized = filename.to_string();
    
    // 移除或替换非法字符
    sanitized = sanitized
        .replace(['<', '>', ':', '"', '|', '?', '*'], "_")
        .replace(['/', '\\'], "_");
    
    // Windows 保留名称检查
    let reserved_names = [
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5",
        "COM6", "COM7", "COM8", "COM9", "LPT1", "LPT2", "LPT3", "LPT4",
        "LPT5", "LPT6", "LPT7", "LPT8", "LPT9"
    ];
    
    let base_name = sanitized.split('.').next().unwrap_or("").to_uppercase();
    if reserved_names.contains(&base_name.as_str()) {
        sanitized = format!("_{}", sanitized);
    }
    
    // 移除开头和结尾的空格和点
    sanitized = sanitized.trim_matches([' ', '.']).to_string();
    
    // 确保不为空
    if sanitized.is_empty() {
        sanitized = "untitled".to_string();
    }
    
    sanitized
}
