use serde::{Deserialize, Serialize};
use chrono::Utc;
use uuid::Uuid;
use crate::core::database::{Database, SensitiveTerm};
use std::sync::LazyLock;
use tokio::sync::Mutex;

// 全局数据库实例（与 database.rs 共享）
static DATABASE: LazyLock<Mutex<Option<Database>>> = LazyLock::new(|| Mutex::new(None));

/// 获取数据库实例
async fn get_database() -> Result<Database, String> {
    let mut db_guard = DATABASE.lock().await;
    
    if db_guard.is_none() {
        let db = Database::new().await.map_err(|e| format!("Failed to initialize database: {}", e))?;
        *db_guard = Some(db);
    }
    
    match db_guard.as_ref() {
        Some(db) => Ok(db.clone()),
        None => Err("Database not initialized".to_string()),
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AddSensitiveTermRequest {
    pub term: String,
    pub category: String,
    pub description: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateSensitiveTermRequest {
    pub id: String,
    pub term: Option<String>,
    pub category: Option<String>,
    pub description: Option<String>,
    pub enabled: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ImportSensitiveTermsRequest {
    pub terms: Vec<AddSensitiveTermRequest>,
}

/// 添加敏感词
#[tauri::command]
pub async fn add_sensitive_term(
    request: AddSensitiveTermRequest,
) -> Result<SensitiveTerm, String> {
    let db = get_database().await?;
    let now = Utc::now();
    let term = SensitiveTerm {
        id: Uuid::new_v4().to_string(),
        term: request.term,
        category: request.category,
        description: request.description,
        enabled: true,
        created_at: now,
        updated_at: now,
    };
    
    db.add_sensitive_term(&term)
        .await
        .map_err(|e| format!("Failed to add sensitive term: {}", e))?;
    
    Ok(term)
}

/// 批量添加敏感词
#[tauri::command]
pub async fn add_sensitive_terms_batch(
    requests: Vec<AddSensitiveTermRequest>,
) -> Result<Vec<SensitiveTerm>, String> {
    let db = get_database().await?;
    let now = Utc::now();
    let terms: Vec<SensitiveTerm> = requests.into_iter().map(|req| {
        SensitiveTerm {
            id: Uuid::new_v4().to_string(),
            term: req.term,
            category: req.category,
            description: req.description,
            enabled: true,
            created_at: now,
            updated_at: now,
        }
    }).collect();
    
    db.add_sensitive_terms_batch(&terms)
        .await
        .map_err(|e| format!("Failed to add sensitive terms: {}", e))?;
    
    Ok(terms)
}

/// 更新敏感词
#[tauri::command]
pub async fn update_sensitive_term(
    request: UpdateSensitiveTermRequest,
) -> Result<SensitiveTerm, String> {
    let db = get_database().await?;
    
    // 先获取现有的敏感词
    let terms = db.get_sensitive_terms(None, false).await
        .map_err(|e| format!("Failed to get sensitive term: {}", e))?;
    
    let existing = terms.iter().find(|t| t.id == request.id)
        .ok_or_else(|| "Sensitive term not found".to_string())?;
    
    // 使用新值或保留旧值
    let term = request.term.unwrap_or_else(|| existing.term.clone());
    let category = request.category.unwrap_or_else(|| existing.category.clone());
    let description = request.description.or_else(|| existing.description.clone());
    let enabled = request.enabled.unwrap_or(existing.enabled);
    
    db.update_sensitive_term(
        &request.id,
        &term,
        &category,
        description.as_deref(),
        enabled,
    )
    .await
    .map_err(|e| format!("Failed to update sensitive term: {}", e))?;
    
    // 返回更新后的敏感词
    Ok(SensitiveTerm {
        id: request.id,
        term,
        category,
        description,
        enabled,
        created_at: existing.created_at,
        updated_at: Utc::now(),
    })
}

/// 删除敏感词
#[tauri::command]
pub async fn delete_sensitive_term(
    id: String,
) -> Result<(), String> {
    let db = get_database().await?;
    db.delete_sensitive_term(&id)
        .await
        .map_err(|e| format!("Failed to delete sensitive term: {}", e))
}

/// 批量删除敏感词
#[tauri::command]
pub async fn delete_sensitive_terms_batch(
    ids: Vec<String>,
) -> Result<(), String> {
    let db = get_database().await?;
    db.delete_sensitive_terms_batch(&ids)
        .await
        .map_err(|e| format!("Failed to delete sensitive terms: {}", e))
}

/// 获取所有敏感词
#[tauri::command]
pub async fn get_sensitive_terms(
    category: Option<String>,
    enabled_only: Option<bool>,
) -> Result<Vec<SensitiveTerm>, String> {
    let db = get_database().await?;
    db.get_sensitive_terms(category.as_deref(), enabled_only.unwrap_or(false))
        .await
        .map_err(|e| format!("Failed to get sensitive terms: {}", e))
}

/// 获取敏感词分类列表
#[tauri::command]
pub async fn get_sensitive_term_categories() -> Result<Vec<String>, String> {
    let db = get_database().await?;
    db.get_sensitive_term_categories()
        .await
        .map_err(|e| format!("Failed to get categories: {}", e))
}

/// 搜索敏感词
#[tauri::command]
pub async fn search_sensitive_terms(
    query: String,
) -> Result<Vec<SensitiveTerm>, String> {
    let db = get_database().await?;
    db.search_sensitive_terms(&query)
        .await
        .map_err(|e| format!("Failed to search sensitive terms: {}", e))
}

/// 获取敏感词统计
#[tauri::command]
pub async fn get_sensitive_terms_stats() -> Result<serde_json::Value, String> {
    let db = get_database().await?;
    db.get_sensitive_terms_stats()
        .await
        .map_err(|e| format!("Failed to get stats: {}", e))
}

/// 导出敏感词为CSV
#[tauri::command]
pub async fn export_sensitive_terms_csv(
    output_path: String,
) -> Result<String, String> {
    let db = get_database().await?;
    let terms = db.get_sensitive_terms(None, false)
        .await
        .map_err(|e| format!("Failed to get sensitive terms: {}", e))?;
    
    let mut csv = String::from("分类,敏感词,描述,状态\n");
    for term in terms {
        let status = if term.enabled { "启用" } else { "禁用" };
        let description = term.description.unwrap_or_default();
        csv.push_str(&format!("{},{},{},{}\n", term.category, term.term, description, status));
    }
    
    // 写入文件
    std::fs::write(&output_path, csv)
        .map_err(|e| format!("Failed to write CSV file: {}", e))?;
    
    Ok(output_path)
}

/// 从CSV导入敏感词
#[tauri::command]
pub async fn import_sensitive_terms_csv(
    file_path: String,
) -> Result<usize, String> {
    let db = get_database().await?;
    
    // 读取文件
    let csv_content = std::fs::read_to_string(&file_path)
        .map_err(|e| format!("Failed to read CSV file: {}", e))?;
    
    let mut terms = Vec::new();
    let now = Utc::now();
    
    for (index, line) in csv_content.lines().enumerate() {
        // 跳过标题行
        if index == 0 {
            continue;
        }
        
        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() < 2 {
            continue;
        }
        
        let category = parts[0].trim().to_string();
        let term = parts[1].trim().to_string();
        let description = if parts.len() > 2 && !parts[2].trim().is_empty() {
            Some(parts[2].trim().to_string())
        } else {
            None
        };
        let enabled = if parts.len() > 3 {
            parts[3].trim() == "启用"
        } else {
            true
        };
        
        if !term.is_empty() && !category.is_empty() {
            terms.push(SensitiveTerm {
                id: Uuid::new_v4().to_string(),
                term,
                category,
                description,
                enabled,
                created_at: now,
                updated_at: now,
            });
        }
    }
    
    let count = terms.len();
    
    if !terms.is_empty() {
        db.add_sensitive_terms_batch(&terms)
            .await
            .map_err(|e| format!("Failed to import sensitive terms: {}", e))?;
    }
    
    Ok(count)
}
