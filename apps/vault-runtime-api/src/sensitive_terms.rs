//! Runtime HTTP CRUD + CSV import/export for the browser sensitive-term
//! library (`/api/v1/sensitive-terms*`). Storage, validation and the
//! transactional import live in `store.rs`; this module only owns HTTP
//! routing, request/response shaping and the `csv` crate usage for
//! RFC 4180-correct import/export (quotes, commas, embedded newlines, UTF-8
//! BOM). Never touches the shared masking algorithms.

use std::convert::Infallible;

use bytes::{BufMut, BytesMut};
use futures_util::TryStreamExt;
use service_contracts::{
    CreateSensitiveTermRequest, SensitiveTermCategoriesResponse, SensitiveTermsImportResponse,
    SensitiveTermsResponse, SensitiveTermsStats, UpdateSensitiveTermRequest,
};
use warp::{http::StatusCode, Filter, Rejection, Reply};

use crate::store::{StoreError, SENSITIVE_TERMS_IMPORT_MAX_BYTES, SENSITIVE_TERMS_IMPORT_MAX_ROWS};
use crate::Runtime;

const CSV_HEADER: [&str; 4] = ["分类", "敏感词", "描述", "状态"];

pub fn routes<F>(
    runtime_filter: F,
) -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone
where
    F: Filter<Extract = (Runtime,), Error = Infallible> + Clone + Send + Sync + 'static,
{
    let list = warp::path!("api" / "v1" / "sensitive-terms")
        .and(warp::get())
        .and(warp::query::<ListQuery>())
        .and(runtime_filter.clone())
        .and_then(list_handler);

    let create = warp::path!("api" / "v1" / "sensitive-terms")
        .and(warp::post())
        .and(warp::body::content_length_limit(64 * 1024))
        .and(warp::body::json())
        .and(runtime_filter.clone())
        .and_then(create_handler);

    let update = warp::path!("api" / "v1" / "sensitive-terms" / String)
        .and(warp::put())
        .and(warp::body::content_length_limit(64 * 1024))
        .and(warp::body::json())
        .and(runtime_filter.clone())
        .and_then(update_handler);

    let delete = warp::path!("api" / "v1" / "sensitive-terms" / String)
        .and(warp::delete())
        .and(runtime_filter.clone())
        .and_then(delete_handler);

    let categories = warp::path!("api" / "v1" / "sensitive-terms" / "categories")
        .and(warp::get())
        .and(runtime_filter.clone())
        .and_then(categories_handler);

    let stats = warp::path!("api" / "v1" / "sensitive-terms" / "stats")
        .and(warp::get())
        .and(runtime_filter.clone())
        .and_then(stats_handler);

    let import = warp::path!("api" / "v1" / "sensitive-terms" / "import")
        .and(warp::post())
        .and(
            warp::multipart::form().max_length(SENSITIVE_TERMS_IMPORT_MAX_BYTES as u64 + 64 * 1024),
        )
        .and(runtime_filter.clone())
        .and_then(import_handler);

    let export = warp::path!("api" / "v1" / "sensitive-terms" / "export")
        .and(warp::get())
        .and(runtime_filter)
        .and_then(export_handler);

    list.or(create)
        .or(update)
        .or(delete)
        .or(categories)
        .or(stats)
        .or(import)
        .or(export)
}

#[derive(Debug, serde::Deserialize)]
struct ListQuery {
    category: Option<String>,
    query: Option<String>,
    #[serde(default)]
    enabled_only: bool,
}

async fn list_handler(query: ListQuery, runtime: Runtime) -> Result<impl Reply, Rejection> {
    let terms = runtime
        .store
        .list_sensitive_terms(
            query.category.as_deref(),
            query.query.as_deref(),
            query.enabled_only,
        )
        .await
        .map_err(sensitive_term_rejection)?;
    Ok(warp::reply::json(&SensitiveTermsResponse { terms }))
}

async fn create_handler(
    request: CreateSensitiveTermRequest,
    runtime: Runtime,
) -> Result<impl Reply, Rejection> {
    let term = runtime
        .store
        .create_sensitive_term(
            &request.term,
            &request.category,
            request.description.as_deref(),
        )
        .await
        .map_err(sensitive_term_rejection)?;
    Ok(warp::reply::with_status(
        warp::reply::json(&term),
        StatusCode::CREATED,
    ))
}

async fn update_handler(
    id: String,
    request: UpdateSensitiveTermRequest,
    runtime: Runtime,
) -> Result<impl Reply, Rejection> {
    let term = runtime
        .store
        .update_sensitive_term(
            &id,
            request.term.as_deref(),
            request.category.as_deref(),
            request.description.as_deref(),
            request.enabled,
        )
        .await
        .map_err(sensitive_term_rejection)?;
    Ok(warp::reply::json(&term))
}

async fn delete_handler(id: String, runtime: Runtime) -> Result<impl Reply, Rejection> {
    runtime
        .store
        .delete_sensitive_term(&id)
        .await
        .map_err(sensitive_term_rejection)?;
    Ok(warp::reply::with_status(
        warp::reply(),
        StatusCode::NO_CONTENT,
    ))
}

async fn categories_handler(runtime: Runtime) -> Result<impl Reply, Rejection> {
    let categories = runtime
        .store
        .sensitive_term_categories()
        .await
        .map_err(sensitive_term_rejection)?;
    Ok(warp::reply::json(&SensitiveTermCategoriesResponse {
        categories,
    }))
}

async fn stats_handler(runtime: Runtime) -> Result<impl Reply, Rejection> {
    let stats: SensitiveTermsStats = runtime
        .store
        .sensitive_terms_stats()
        .await
        .map_err(sensitive_term_rejection)?;
    Ok(warp::reply::json(&stats))
}

async fn import_handler(
    form: warp::multipart::FormData,
    runtime: Runtime,
) -> Result<impl Reply, Rejection> {
    let bytes = read_single_csv_field(form).await?;
    let rows = parse_sensitive_terms_csv(&bytes)?;
    let imported_count = runtime
        .store
        .import_sensitive_terms(rows)
        .await
        .map_err(sensitive_term_rejection)?;
    Ok(warp::reply::json(&SensitiveTermsImportResponse {
        imported_count,
    }))
}

async fn export_handler(runtime: Runtime) -> Result<impl Reply, Rejection> {
    let terms = runtime
        .store
        .list_sensitive_terms(None, None, false)
        .await
        .map_err(sensitive_term_rejection)?;

    fn export_failed() -> Rejection {
        crate::api_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "STORAGE_INTERNAL_ERROR",
            "Failed to build CSV export",
            true,
        )
    }

    let mut writer = csv::WriterBuilder::new().from_writer(Vec::new());
    writer
        .write_record(CSV_HEADER)
        .map_err(|_| export_failed())?;
    for term in &terms {
        let status = if term.enabled { "启用" } else { "禁用" };
        writer
            .write_record([
                term.category.as_str(),
                term.term.as_str(),
                term.description.as_deref().unwrap_or(""),
                status,
            ])
            .map_err(|_| export_failed())?;
    }
    let csv_bytes = writer.into_inner().map_err(|_| export_failed())?;

    Ok(warp::http::Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "text/csv; charset=utf-8")
        .header(
            "content-disposition",
            "attachment; filename=\"sensitive_terms.csv\"",
        )
        .body(csv_bytes)
        .expect("valid CSV export response"))
}

/// Read the single `file` multipart field expected by the import endpoint.
/// Rejects a missing field, more than one field, an unexpected field name,
/// or a field over the configured byte limit — mirrors `crate::parse_form`'s
/// multipart-reading pattern but for exactly one CSV upload.
async fn read_single_csv_field(mut form: warp::multipart::FormData) -> Result<Vec<u8>, Rejection> {
    let mut file_bytes: Option<Vec<u8>> = None;
    while let Some(part) = form.try_next().await.map_err(|_| {
        crate::api_error(
            StatusCode::BAD_REQUEST,
            "INVALID_MULTIPART",
            "Multipart body is invalid",
            false,
        )
    })? {
        let name = part.name().to_string();
        let data = part
            .stream()
            .try_fold(BytesMut::new(), |mut buffer, mut chunk| async move {
                buffer.put(&mut chunk);
                Ok::<_, warp::Error>(buffer)
            })
            .await
            .map_err(|_| {
                crate::api_error(
                    StatusCode::BAD_REQUEST,
                    "INVALID_MULTIPART",
                    "Multipart field could not be read",
                    false,
                )
            })?
            .to_vec();

        if name != "file" {
            return Err(crate::api_error(
                StatusCode::BAD_REQUEST,
                "UNEXPECTED_FIELD",
                "Multipart field is not supported",
                false,
            ));
        }
        if file_bytes.is_some() {
            return Err(crate::api_error(
                StatusCode::BAD_REQUEST,
                "SENSITIVE_TERMS_IMPORT_INVALID",
                "Only one CSV file is accepted",
                false,
            ));
        }
        if data.len() > SENSITIVE_TERMS_IMPORT_MAX_BYTES {
            return Err(crate::api_error(
                StatusCode::PAYLOAD_TOO_LARGE,
                "INPUT_LIMIT_EXCEEDED",
                "CSV file exceeds the size limit",
                false,
            ));
        }
        file_bytes = Some(data);
    }

    file_bytes.ok_or_else(|| {
        crate::api_error(
            StatusCode::BAD_REQUEST,
            "SENSITIVE_TERMS_IMPORT_INVALID",
            "A CSV file is required",
            false,
        )
    })
}

fn strip_utf8_bom(bytes: &[u8]) -> &[u8] {
    bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes)
}

fn import_invalid(message: impl Into<String>) -> Rejection {
    crate::api_error(
        StatusCode::BAD_REQUEST,
        "SENSITIVE_TERMS_IMPORT_INVALID",
        message,
        false,
    )
}

/// Parse and fully validate a CSV upload into `(category, term, description,
/// enabled)` rows, ready for `Store::import_sensitive_terms`. Every check
/// here fails the request outright — there is no partial-parse fallback
/// (8.5): non-UTF-8 bytes, a missing/incorrect header, wrong column count,
/// or an illegal status value all reject the whole file before any row
/// reaches storage.
fn parse_sensitive_terms_csv(
    bytes: &[u8],
) -> Result<Vec<(String, String, Option<String>, bool)>, Rejection> {
    let bytes = strip_utf8_bom(bytes);
    let text = std::str::from_utf8(bytes)
        .map_err(|_| import_invalid("CSV 文件必须是合法的 UTF-8 编码"))?;

    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(false)
        .from_reader(text.as_bytes());

    {
        let headers = reader
            .headers()
            .map_err(|_| import_invalid("CSV 表头缺失或格式不正确"))?;
        let actual: Vec<&str> = headers.iter().collect();
        if actual != CSV_HEADER.to_vec() {
            return Err(import_invalid("CSV 表头必须精确为：分类,敏感词,描述,状态"));
        }
    }

    let mut rows = Vec::new();
    for (index, record) in reader.records().enumerate() {
        if index >= SENSITIVE_TERMS_IMPORT_MAX_ROWS {
            return Err(crate::api_error(
                StatusCode::PAYLOAD_TOO_LARGE,
                "INPUT_LIMIT_EXCEEDED",
                "CSV 数据行数超过上限",
                false,
            ));
        }
        let record = record.map_err(|_| import_invalid("CSV 数据行格式不正确"))?;
        if record.len() != CSV_HEADER.len() {
            return Err(import_invalid("CSV 数据行列数必须为 4 列"));
        }

        let category = record.get(0).unwrap_or("").to_string();
        let term = record.get(1).unwrap_or("").to_string();
        let description_raw = record.get(2).unwrap_or("").trim();
        let description = if description_raw.is_empty() {
            None
        } else {
            Some(description_raw.to_string())
        };
        let status = record.get(3).unwrap_or("").trim();
        let enabled = match status {
            "启用" => true,
            "禁用" => false,
            other => {
                return Err(import_invalid(format!(
                    "状态列必须是「启用」或「禁用」，实际为「{other}」"
                )));
            }
        };

        rows.push((category, term, description, enabled));
    }

    Ok(rows)
}

fn sensitive_term_rejection(error: StoreError) -> Rejection {
    match error {
        StoreError::SensitiveTermInvalid(message) => crate::api_error(
            StatusCode::BAD_REQUEST,
            "SENSITIVE_TERM_INVALID",
            message,
            false,
        ),
        StoreError::SensitiveTermDuplicate => crate::api_error(
            StatusCode::CONFLICT,
            "SENSITIVE_TERM_DUPLICATE",
            "This sensitive term already exists",
            false,
        ),
        StoreError::SensitiveTermNotFound => crate::api_error(
            StatusCode::NOT_FOUND,
            "SENSITIVE_TERM_NOT_FOUND",
            "The requested sensitive term was not found",
            false,
        ),
        StoreError::SensitiveTermsImportInvalid(message) => crate::api_error(
            StatusCode::BAD_REQUEST,
            "SENSITIVE_TERMS_IMPORT_INVALID",
            message,
            false,
        ),
        StoreError::InputLimitExceeded => crate::api_error(
            StatusCode::PAYLOAD_TOO_LARGE,
            "INPUT_LIMIT_EXCEEDED",
            "Input exceeds the configured limit",
            false,
        ),
        _ => crate::api_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "STORAGE_INTERNAL_ERROR",
            "Sensitive term storage operation failed",
            true,
        ),
    }
}
