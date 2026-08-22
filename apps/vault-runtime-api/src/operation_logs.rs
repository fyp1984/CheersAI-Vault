//! Runtime HTTP read/clear API for the browser operation log
//! (`/api/v1/operation-logs*`). This module owns only HTTP routing, query
//! validation and response shaping; the projection off `job_events`/
//! `restore_events`, the statistics SQL and the transactional clear all live
//! in `store.rs`. There is no parallel log table here — every response is a
//! direct read of the Runtime's existing event/batch data.

use std::convert::Infallible;

use service_contracts::{ClearOperationLogsResponse, OperationLogLevel, OperationLogListResponse};
use warp::{http::StatusCode, Filter, Rejection, Reply};

use crate::store::StoreError;
use crate::Runtime;

const DEFAULT_PAGE_SIZE: usize = 10;
const MAX_PAGE_SIZE: usize = 100;
const BATCH_ID_PREFIX_MAX_CHARS: usize = 36;
const STATUS_FILTER_MAX_CHARS: usize = 64;

pub fn routes<F>(
    runtime_filter: F,
) -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone
where
    F: Filter<Extract = (Runtime,), Error = Infallible> + Clone + Send + Sync + 'static,
{
    let list = warp::path!("api" / "v1" / "operation-logs")
        .and(warp::get())
        .and(warp::query::<ListQuery>())
        .and(runtime_filter.clone())
        .and_then(list_handler);

    let statistics = warp::path!("api" / "v1" / "operation-logs" / "statistics")
        .and(warp::get())
        .and(runtime_filter.clone())
        .and_then(statistics_handler);

    let storage_status = warp::path!("api" / "v1" / "operation-logs" / "storage-status")
        .and(warp::get())
        .and(runtime_filter.clone())
        .and_then(storage_status_handler);

    let clear = warp::path!("api" / "v1" / "operation-logs")
        .and(warp::delete())
        .and(runtime_filter)
        .and_then(clear_handler);

    list.or(statistics).or(storage_status).or(clear)
}

#[derive(Debug, serde::Deserialize)]
struct ListQuery {
    page: Option<usize>,
    page_size: Option<usize>,
    level: Option<String>,
    status: Option<String>,
    batch_id: Option<String>,
}

fn invalid_query(message: impl Into<String>) -> Rejection {
    crate::api_error(StatusCode::BAD_REQUEST, "INVALID_QUERY", message, false)
}

fn operation_log_storage_error() -> Rejection {
    crate::api_error(
        StatusCode::INTERNAL_SERVER_ERROR,
        "STORAGE_INTERNAL_ERROR",
        "Operation log storage operation failed",
        true,
    )
}

/// `batch_id` prefix charset is restricted to hex digits and hyphens (the
/// only characters a real UUID can contain) and bounded to the length of a
/// full UUID — this rules out `%`/`_` LIKE wildcards and unbounded scans
/// (安全约束 4) without needing a separate escape step.
fn validate_batch_id_prefix(value: &str) -> Result<(), Rejection> {
    if value.is_empty() || value.chars().count() > BATCH_ID_PREFIX_MAX_CHARS {
        return Err(invalid_query("batch_id 前缀长度不合法"));
    }
    if !value.chars().all(|c| c.is_ascii_hexdigit() || c == '-') {
        return Err(invalid_query("batch_id 前缀只能包含十六进制字符和连字符"));
    }
    Ok(())
}

async fn list_handler(query: ListQuery, runtime: Runtime) -> Result<impl Reply, Rejection> {
    let page = query.page.unwrap_or(1);
    if page == 0 {
        return Err(invalid_query("page 必须从 1 开始"));
    }
    let page_size = query.page_size.unwrap_or(DEFAULT_PAGE_SIZE);
    if page_size == 0 || page_size > MAX_PAGE_SIZE {
        return Err(invalid_query("page_size 必须在 1 到 100 之间"));
    }
    let level = match query.level.as_deref() {
        Some(raw) => Some(
            OperationLogLevel::parse(raw)
                .ok_or_else(|| invalid_query("level 取值不合法"))?
                .as_str(),
        ),
        None => None,
    };
    if let Some(status) = query.status.as_deref() {
        if status.is_empty() || status.chars().count() > STATUS_FILTER_MAX_CHARS {
            return Err(invalid_query("status 取值不合法"));
        }
    }
    if let Some(prefix) = query.batch_id.as_deref() {
        validate_batch_id_prefix(prefix)?;
    }

    let (entries, total_count) = runtime
        .store
        .list_operation_logs(
            page,
            page_size,
            level,
            query.status.as_deref(),
            query.batch_id.as_deref(),
        )
        .await
        .map_err(|_: StoreError| operation_log_storage_error())?;
    let total_pages = if total_count == 0 {
        0
    } else {
        total_count.div_ceil(page_size)
    };
    Ok(warp::reply::json(&OperationLogListResponse {
        entries,
        page,
        page_size,
        total_count,
        total_pages,
    }))
}

async fn statistics_handler(runtime: Runtime) -> Result<impl Reply, Rejection> {
    let statistics = runtime
        .store
        .operation_log_statistics()
        .await
        .map_err(|_: StoreError| operation_log_storage_error())?;
    Ok(warp::reply::json(&statistics))
}

async fn storage_status_handler(runtime: Runtime) -> Result<impl Reply, Rejection> {
    let status = runtime
        .store
        .operation_log_storage_status()
        .await
        .map_err(|_: StoreError| operation_log_storage_error())?;
    Ok(warp::reply::json(&status))
}

async fn clear_handler(runtime: Runtime) -> Result<impl Reply, Rejection> {
    let (deleted_job_events, deleted_restore_events) =
        runtime
            .store
            .clear_operation_logs()
            .await
            .map_err(|_: StoreError| operation_log_storage_error())?;
    Ok(warp::reply::json(&ClearOperationLogsResponse {
        deleted_job_events,
        deleted_restore_events,
    }))
}
