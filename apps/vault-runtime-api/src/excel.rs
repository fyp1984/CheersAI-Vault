use std::collections::{HashMap, HashSet};
use std::convert::Infallible;
use std::fs;
use std::path::{Path, PathBuf};

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use bytes::{BufMut, BytesMut};
use calamine::{open_workbook_auto, Data, Reader};
use excel_style_core::{table_reader, CellKey, RewriteOutcome};
use futures_util::TryStreamExt;
use hmac::Hmac;
use pbkdf2::pbkdf2;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use service_contracts::{
    ExcelArtifactMemberKind, ExcelArtifactMembersResponse, ExcelPersistArtifactsResponse,
};
use sha2::{Digest, Sha256};
use tempfile::tempdir;
use warp::{http::StatusCode, multipart::FormData, Filter, Rejection, Reply};

use crate::store::{ExcelArtifactMemberPayload, StoreError};

const ECMAP_MAGIC: &[u8] = b"ECMAP\x02";
const ENCSRC_MAGIC: &[u8] = b"VAULT_ENCSRC\x01";
const SALT_LEN: usize = 32;
const NONCE_LEN: usize = 12;
const DEFAULT_SUFFIX: &str = "_masked";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SheetDef {
    pub name: String,
    pub headers: Vec<String>,
    pub column_samples: Vec<Vec<String>>,
    #[serde(rename = "data_hint", skip_serializing_if = "Option::is_none")]
    pub deprecated_data_hint: Option<Vec<String>>,
    pub max_row: u32,
    pub max_col: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EncSourceKeyMode {
    SandboxReused,
    SecondaryPassphrase,
    DeviceKey,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnMaskRule {
    pub sheet: String,
    #[serde(rename = "colIndex")]
    pub col_index: usize,
    #[serde(rename = "headerText")]
    pub header_text: String,
    pub strategy: String,
    pub replacement: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CellOverrideRule {
    pub sheet: String,
    pub row: usize,
    pub col: usize,
    pub strategy: String,
    pub replacement: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SheetPolicy {
    pub sheet: String,
    pub column_rules: Vec<ColumnMaskRule>,
    pub cell_overrides: Vec<CellOverrideRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExcelMaskingConfig {
    pub file_path: String,
    pub sheet_policies: Vec<SheetPolicy>,
    pub retain_encrypted_source: bool,
    pub key_mode: EncSourceKeyMode,
    pub secondary_passphrase: Option<String>,
    pub processing_time_ms: Option<u64>,
    pub excel_config_sha256: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreviewRow {
    pub original_preview: Vec<Option<String>>,
    pub masked: Vec<String>,
    pub row_index: u32,
    pub sheet: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExcelMaskPreview {
    pub preview_rows: Vec<PreviewRow>,
    pub conflicts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct EcmapEntryV1 {
    cell_ref: String,
    original_sha256: String,
    original_preview: String,
    masked: String,
    strategy_id: String,
    col_index: u32,
    row_index: u32,
    sheet: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct EcmapHeaderV1 {
    pub(crate) version: String,
    #[serde(rename = "originalSha256", alias = "original_sha256")]
    pub(crate) original_sha256: String,
    #[serde(rename = "maskedSha256", alias = "masked_sha256")]
    pub(crate) masked_sha256: String,
    #[serde(rename = "sourceEncryptionKeySource", alias = "source_encryption_key_source")]
    pub(crate) source_encryption_key_source: String,
    #[serde(rename = "passphraseDomainHint8", alias = "passphrase_domain_hint8")]
    pub(crate) passphrase_domain_hint8: String,
    #[serde(rename = "sourceRetained", alias = "source_retained", default)]
    pub(crate) source_retained: bool,
}

pub(crate) const KEY_SOURCE_SANDBOX: &str = "SANDBOX_PASSPHRASE_REUSED";
pub(crate) const KEY_SOURCE_SEPARATE: &str = "SEPARATE_PASSPHRASE";
pub(crate) const KEY_SOURCE_DEVICE: &str = "DEVICE_KEY";

pub(crate) fn canonical_key_source(mode: &EncSourceKeyMode) -> &'static str {
    match mode {
        EncSourceKeyMode::SandboxReused => KEY_SOURCE_SANDBOX,
        EncSourceKeyMode::SecondaryPassphrase => KEY_SOURCE_SEPARATE,
        EncSourceKeyMode::DeviceKey => KEY_SOURCE_DEVICE,
    }
}

/// Accepts both the current canonical enum values and the pre-fix values
/// written by older Runtime/Tauri builds, so existing `.ecmap` artifacts
/// stay readable. Rejects anything else instead of guessing.
///
/// Not yet wired into a live Runtime restore endpoint (Runtime does not
/// expose one), but shared with the Tauri host's equivalent validation and
/// exercised directly by tests.
#[allow(dead_code)]
pub(crate) fn normalize_key_source(raw: &str) -> Result<&'static str, String> {
    match raw {
        "SANDBOX_PASSPHRASE_REUSED" | "SandboxReused" => Ok(KEY_SOURCE_SANDBOX),
        "SEPARATE_PASSPHRASE" | "SecondaryPassphrase" | "SecondaryPhrase" => {
            Ok(KEY_SOURCE_SEPARATE)
        }
        "DEVICE_KEY" | "DeviceKey" => Ok(KEY_SOURCE_DEVICE),
        other => Err(format!("未知的密钥来源枚举值: {other}")),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct EcmapDocumentV1 {
    header: EcmapHeaderV1,
    entries: Vec<EcmapEntryV1>,
}

struct ExcelUploadPayload {
    display_name: String,
    file_bytes: Vec<u8>,
    config: Option<ExcelMaskingConfig>,
    max_rows: usize,
    rule_ids: Vec<String>,
}

#[derive(Debug, Clone)]
struct CompiledColumnBinding {
    strategy: String,
    replacement: Option<String>,
}

#[derive(Debug, Clone)]
struct CompiledSheetPolicy {
    column_bindings: HashMap<usize, CompiledColumnBinding>,
    cell_overrides: HashMap<(usize, usize), (String, Option<String>)>,
}

#[derive(Debug, Clone)]
struct CompiledWorkbookPolicy {
    sheets: HashMap<String, CompiledSheetPolicy>,
    conflicts: Vec<String>,
}

#[derive(Debug)]
struct BuiltExcelArtifacts {
    ascii_stem: String,
    input_display_name_hash8: String,
    masked_entity_count: usize,
    members: Vec<ExcelArtifactMemberPayload>,
}

#[derive(Debug, Clone)]
struct ExcelBuildFailure {
    status: StatusCode,
    code: &'static str,
    message: String,
    retryable: bool,
}

impl ExcelBuildFailure {
    fn into_rejection(self) -> Rejection {
        crate::api_error(self.status, self.code, self.message, self.retryable)
    }
}

pub fn routes<F>(
    runtime_filter: F,
) -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone
where
    F: Filter<Extract = (crate::Runtime,), Error = Infallible> + Clone + Send + Sync + 'static,
{
    let parse_structure = warp::path!("api" / "v1" / "excel" / "parse-structure")
        .and(warp::post())
        .and(warp::multipart::form().max_length(512 * 1024 * 1024))
        .and(runtime_filter.clone())
        .and_then(parse_structure_handler);

    let preview = warp::path!("api" / "v1" / "excel" / "preview")
        .and(warp::post())
        .and(warp::multipart::form().max_length(512 * 1024 * 1024))
        .and(runtime_filter.clone())
        .and_then(preview_handler);

    let jobs = warp::path!("api" / "v1" / "excel" / "jobs")
        .and(warp::post())
        .and(warp::multipart::form().max_length(512 * 1024 * 1024))
        .and(runtime_filter.clone())
        .and_then(persist_handler);

    let members = warp::path!("api" / "v1" / "artifacts" / String / "members")
        .and(warp::get())
        .and(runtime_filter.clone())
        .and_then(list_artifact_members_handler);

    let member_download = warp::path!("api" / "v1" / "artifacts" / String / "members" / String)
        .and(warp::get())
        .and(runtime_filter)
        .and_then(download_artifact_member_handler);

    parse_structure
        .or(preview)
        .or(jobs)
        .or(members)
        .or(member_download)
}

async fn parse_structure_handler(
    form: FormData,
    _runtime: crate::Runtime,
) -> Result<impl Reply, Rejection> {
    let payload = parse_excel_form(form, false).await?;
    let temp = write_upload_to_temp(&payload.display_name, &payload.file_bytes).map_err(|msg| {
        crate::api_error(StatusCode::BAD_REQUEST, "INPUT_READ_FAILED", msg, false)
    })?;
    let sheets =
        parse_excel_structure_detailed(&temp.path).map_err(ExcelBuildFailure::into_rejection)?;
    Ok(warp::reply::json(&sheets))
}

async fn preview_handler(
    form: FormData,
    _runtime: crate::Runtime,
) -> Result<impl Reply, Rejection> {
    let payload = parse_excel_form(form, true).await?;
    let config = payload.config.ok_or_else(|| {
        crate::api_error(
            StatusCode::BAD_REQUEST,
            "EXCEL_CONFIG_REQUIRED",
            "Excel config payload is required",
            false,
        )
    })?;
    let temp = write_upload_to_temp(&payload.display_name, &payload.file_bytes).map_err(|msg| {
        crate::api_error(StatusCode::BAD_REQUEST, "INPUT_READ_FAILED", msg, false)
    })?;
    let preview = build_preview(&temp.path, &config, payload.max_rows)
        .map_err(ExcelBuildFailure::into_rejection)?;
    Ok(warp::reply::json(&preview))
}

async fn persist_handler(form: FormData, runtime: crate::Runtime) -> Result<impl Reply, Rejection> {
    let payload = parse_excel_form(form, true).await?;
    let config = payload.config.ok_or_else(|| {
        crate::api_error(
            StatusCode::BAD_REQUEST,
            "EXCEL_CONFIG_REQUIRED",
            "Excel config payload is required",
            false,
        )
    })?;
    let job = runtime
        .store
        .create_excel_job(
            crate::store::NewUpload {
                display_name: payload.display_name.clone(),
                input_format: "excel".to_string(),
                bytes: payload.file_bytes.clone(),
            },
            payload.rule_ids.clone(),
        )
        .await
        .map_err(store_rejection)?;
    let temp = write_upload_to_temp(&payload.display_name, &payload.file_bytes).map_err(|msg| {
        crate::api_error(StatusCode::BAD_REQUEST, "INPUT_READ_FAILED", msg, false)
    })?;
    let artifact_id = uuid::Uuid::new_v4().to_string();
    let built = match build_excel_artifacts(
        &temp.path,
        &payload.display_name,
        &payload.file_bytes,
        &config,
        &artifact_id,
    ) {
        Ok(built) => built,
        Err(error) => {
            let _ = runtime
                .store
                .mark_failed(&job, error.code, &error.message)
                .await;
            return Err(error.into_rejection());
        }
    };
    let response: ExcelPersistArtifactsResponse = runtime
        .store
        .write_excel_completed(
            &job,
            &artifact_id,
            &built.ascii_stem,
            &built.input_display_name_hash8,
            built.masked_entity_count,
            built.members,
        )
        .await
        .map_err(store_rejection)?;
    Ok(warp::reply::json(&response))
}

async fn list_artifact_members_handler(
    artifact_id: String,
    runtime: crate::Runtime,
) -> Result<impl Reply, Rejection> {
    let response: ExcelArtifactMembersResponse = runtime
        .store
        .excel_artifact_members(&artifact_id)
        .await
        .map_err(store_rejection)?;
    Ok(warp::reply::json(&response))
}

async fn download_artifact_member_handler(
    artifact_id: String,
    member_kind: String,
    runtime: crate::Runtime,
) -> Result<impl Reply, Rejection> {
    let member_kind = ExcelArtifactMemberKind::parse(&member_kind).ok_or_else(|| {
        crate::api_error(
            StatusCode::BAD_REQUEST,
            "EXCEL_MEMBER_KIND_INVALID",
            "The requested Excel artifact member is invalid",
            false,
        )
    })?;
    let (member, bytes) = runtime
        .store
        .excel_artifact_member(&artifact_id, member_kind)
        .await
        .map_err(store_rejection)?;
    Ok(warp::http::Response::builder()
        .status(StatusCode::OK)
        .header("content-type", member_content_type(member.kind))
        .header(
            "content-disposition",
            format!("attachment; filename=\"{}\"", member.display_name),
        )
        .body(bytes)
        .expect("valid excel member response"))
}

async fn parse_excel_form(
    form: FormData,
    require_config: bool,
) -> Result<ExcelUploadPayload, Rejection> {
    let mut file_bytes: Option<Vec<u8>> = None;
    let mut display_name: Option<String> = None;
    let mut config: Option<ExcelMaskingConfig> = None;
    let mut max_rows = 20usize;
    let mut rule_ids: Vec<String> = Vec::new();
    let mut form = form;

    while let Some(part) = form.try_next().await.map_err(|_| {
        crate::api_error(
            StatusCode::BAD_REQUEST,
            "INVALID_MULTIPART",
            "The multipart request could not be parsed",
            false,
        )
    })? {
        let name = part.name().to_string();
        let filename = part.filename().map(str::to_string);
        let bytes = part
            .stream()
            .try_fold(BytesMut::new(), |mut data, chunk| async move {
                data.put(chunk);
                Ok::<_, warp::Error>(data)
            })
            .await
            .map_err(|_| {
                crate::api_error(
                    StatusCode::BAD_REQUEST,
                    "INVALID_MULTIPART",
                    "The multipart request could not be parsed",
                    false,
                )
            })?
            .to_vec();

        match name.as_str() {
            "file" => {
                display_name = filename.or_else(|| Some("input.xlsx".to_string()));
                file_bytes = Some(bytes);
            }
            "config" => {
                config = Some(
                    serde_json::from_slice::<ExcelMaskingConfig>(&bytes).map_err(|_| {
                        crate::api_error(
                            StatusCode::BAD_REQUEST,
                            "EXCEL_CONFIG_INVALID",
                            "Excel config payload is invalid",
                            false,
                        )
                    })?,
                );
            }
            "max_rows" => {
                let text = String::from_utf8(bytes).map_err(|_| {
                    crate::api_error(
                        StatusCode::BAD_REQUEST,
                        "INVALID_REQUEST",
                        "max_rows must be utf-8 text",
                        false,
                    )
                })?;
                max_rows = text.trim().parse::<usize>().map_err(|_| {
                    crate::api_error(
                        StatusCode::BAD_REQUEST,
                        "INVALID_REQUEST",
                        "max_rows must be a positive integer",
                        false,
                    )
                })?;
            }
            "rule_ids" => {
                rule_ids = serde_json::from_slice::<Vec<String>>(&bytes).map_err(|_| {
                    crate::api_error(
                        StatusCode::BAD_REQUEST,
                        "INVALID_REQUEST",
                        "rule_ids must be a JSON string array",
                        false,
                    )
                })?;
            }
            _ => {}
        }
    }

    let file_bytes = file_bytes.ok_or_else(|| {
        crate::api_error(
            StatusCode::BAD_REQUEST,
            "INPUT_REQUIRED",
            "A single Excel file is required",
            false,
        )
    })?;
    let display_name = display_name.unwrap_or_else(|| "input.xlsx".to_string());
    if require_config && config.is_none() {
        return Err(crate::api_error(
            StatusCode::BAD_REQUEST,
            "EXCEL_CONFIG_REQUIRED",
            "Excel config payload is required",
            false,
        ));
    }

    Ok(ExcelUploadPayload {
        display_name,
        file_bytes,
        config,
        max_rows: max_rows.clamp(1, 100),
        rule_ids,
    })
}

struct TempUpload {
    _dir: tempfile::TempDir,
    path: PathBuf,
}

fn write_upload_to_temp(display_name: &str, bytes: &[u8]) -> Result<TempUpload, String> {
    let dir = tempdir().map_err(|e| format!("Failed to create temp dir: {e}"))?;
    let ext = Path::new(display_name)
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("xlsx");
    let path = dir.path().join(format!("input.{ext}"));
    fs::write(&path, bytes).map_err(|e| format!("Failed to write temp upload: {e}"))?;
    Ok(TempUpload { _dir: dir, path })
}

fn build_failure(
    status: StatusCode,
    code: &'static str,
    message: impl Into<String>,
    retryable: bool,
) -> ExcelBuildFailure {
    ExcelBuildFailure {
        status,
        code,
        message: message.into(),
        retryable,
    }
}

fn is_csv_path(path: &Path) -> bool {
    path.extension()
        .and_then(|s| s.to_str())
        .map(|s| s.eq_ignore_ascii_case("csv"))
        .unwrap_or(false)
}

/// True for a legacy OLE `.xls` path (case-insensitive). `.xls` is a
/// completely different binary container (OLE/CFB, not a ZIP archive) from
/// `.xlsx`, so it must never be routed to `table_reader`'s ZIP-based reader
/// (see R1: that misrouting made every valid `.xls` fail as "not a valid
/// ZIP/XLSX archive"). It keeps using calamine's `open_workbook_auto`, which
/// still validates the actual bytes rather than trusting the extension —
/// routing by extension only decides *which* reader to try, not whether the
/// content is accepted.
fn is_xls_path(path: &Path) -> bool {
    path.extension()
        .and_then(|s| s.to_str())
        .map(|s| s.eq_ignore_ascii_case("xls"))
        .unwrap_or(false)
}

const TABLE_READ_SAMPLE_ROWS: usize = 5;

fn parse_excel_structure_detailed(path: &Path) -> Result<Vec<SheetDef>, ExcelBuildFailure> {
    if is_csv_path(path) {
        let structure = table_reader::read_csv_structure(path, TABLE_READ_SAMPLE_ROWS).map_err(
            |e| {
                build_failure(
                    StatusCode::BAD_REQUEST,
                    "INPUT_CORRUPTED",
                    format!("Failed to read CSV file: {e}"),
                    false,
                )
            },
        )?;
        return Ok(vec![SheetDef {
            name: structure.name,
            headers: structure.headers,
            column_samples: structure.column_samples,
            deprecated_data_hint: None,
            max_row: structure.max_row,
            max_col: structure.max_col,
        }]);
    }

    if is_xls_path(path) {
        return parse_xls_structure_legacy(path);
    }

    let structures = table_reader::read_xlsx_all_sheets_structure(path, TABLE_READ_SAMPLE_ROWS)
        .map_err(|e| {
            build_failure(
                StatusCode::BAD_REQUEST,
                "INPUT_CORRUPTED",
                format!("Failed to open Excel file: {e}"),
                false,
            )
        })?;

    Ok(structures
        .into_iter()
        .map(|s| SheetDef {
            name: s.name,
            headers: s.headers,
            column_samples: s.column_samples,
            deprecated_data_hint: None,
            max_row: s.max_row,
            max_col: s.max_col,
        })
        .collect())
}

/// The pre-`table_reader` calamine-based structure reader, restored
/// verbatim for `.xls` only (R1). `.xls` is not affected by the `.xlsx`
/// performance work (calamine already reads the whole legacy binary format
/// regardless), so this keeps its original, already-correct behavior.
fn parse_xls_structure_legacy(path: &Path) -> Result<Vec<SheetDef>, ExcelBuildFailure> {
    let mut workbook = open_workbook_auto(path).map_err(|e| {
        build_failure(
            StatusCode::BAD_REQUEST,
            "INPUT_CORRUPTED",
            format!("Failed to open Excel file: {e}"),
            false,
        )
    })?;

    let sheet_names = workbook.sheet_names().to_vec();
    let mut result = Vec::with_capacity(sheet_names.len());

    for sheet_name in sheet_names {
        let range = workbook.worksheet_range(&sheet_name).map_err(|e| {
            build_failure(
                StatusCode::BAD_REQUEST,
                "INPUT_CORRUPTED",
                format!("Worksheet '{sheet_name}' error: {e}"),
                false,
            )
        })?;

        let (height_usize, width_usize) = range.get_size();
        let headers: Vec<String> = if height_usize > 0 {
            (0..width_usize)
                .map(|c| {
                    range
                        .get((0usize, c))
                        .map(cell_to_string)
                        .unwrap_or_default()
                })
                .collect()
        } else {
            Vec::new()
        };

        let sample_rows = std::cmp::min(5usize, height_usize.saturating_sub(1));
        let mut column_samples: Vec<Vec<String>> =
            (0..width_usize).map(|_| Vec::with_capacity(sample_rows)).collect();
        for r in 1..=sample_rows {
            for (c, samples) in column_samples.iter_mut().enumerate().take(width_usize) {
                let value = range.get((r, c)).map(cell_to_string).unwrap_or_default();
                samples.push(value);
            }
        }

        result.push(SheetDef {
            name: sheet_name,
            headers,
            column_samples,
            deprecated_data_hint: None,
            max_row: height_usize as u32,
            max_col: width_usize as u32,
        });
    }

    Ok(result)
}

fn build_preview(
    input_path: &Path,
    config: &ExcelMaskingConfig,
    max_rows: usize,
) -> Result<ExcelMaskPreview, ExcelBuildFailure> {
    let structures = parse_excel_structure_detailed(input_path)?;
    let compiled = compile_workbook_policy(&structures, config);

    if is_xls_path(input_path) {
        let preview_rows =
            build_xls_preview_rows_legacy(input_path, &structures, &compiled, max_rows)?;
        return Ok(ExcelMaskPreview {
            preview_rows,
            conflicts: compiled.conflicts,
        });
    }

    let is_csv = is_csv_path(input_path);

    let mut preview_rows = Vec::new();

    for structure in &structures {
        let table_preview = if is_csv {
            table_reader::read_csv_preview(input_path, max_rows)
        } else {
            table_reader::read_xlsx_preview(input_path, &structure.name, max_rows)
        }
        .map_err(|e| {
            build_failure(
                StatusCode::BAD_REQUEST,
                "INPUT_CORRUPTED",
                format!("Worksheet '{}' error: {e}", structure.name),
                false,
            )
        })?;

        let width = structure.max_col as usize;
        for row in table_preview.rows {
            // `row_number` is the 1-based file row (row 1 = header), matching
            // the calamine-range convention this replaces where row_idx=1
            // was the first data row within a 0-based, header-inclusive range.
            let row_idx = (row.row_number - 1) as usize;
            let mut original_preview = Vec::with_capacity(width);
            let mut masked = Vec::with_capacity(width);
            for (col_idx, original) in row.values.iter().enumerate().take(width) {
                original_preview.push(if original.is_empty() {
                    None
                } else {
                    Some(original.clone())
                });
                masked.push(mask_for_position(
                    original,
                    &compiled,
                    &structure.name,
                    row_idx,
                    col_idx,
                ));
            }
            preview_rows.push(PreviewRow {
                original_preview,
                masked,
                row_index: row.row_number,
                sheet: structure.name.clone(),
            });
        }
    }

    Ok(ExcelMaskPreview {
        preview_rows,
        conflicts: compiled.conflicts,
    })
}

/// The pre-`table_reader` calamine-based preview reader, restored verbatim
/// for `.xls` only (R1), opening the workbook once and reusing it across
/// sheets exactly as the original code did.
fn build_xls_preview_rows_legacy(
    input_path: &Path,
    structures: &[SheetDef],
    compiled: &CompiledWorkbookPolicy,
    max_rows: usize,
) -> Result<Vec<PreviewRow>, ExcelBuildFailure> {
    let mut workbook = open_workbook_auto(input_path).map_err(|e| {
        build_failure(
            StatusCode::BAD_REQUEST,
            "INPUT_CORRUPTED",
            format!("Failed to open Excel file: {e}"),
            false,
        )
    })?;

    let mut preview_rows = Vec::new();

    for structure in structures {
        let range = workbook.worksheet_range(&structure.name).map_err(|e| {
            build_failure(
                StatusCode::BAD_REQUEST,
                "INPUT_CORRUPTED",
                format!("Worksheet '{}' error: {e}", structure.name),
                false,
            )
        })?;
        let (height, width) = range.get_size();
        let row_end = height.min(max_rows.saturating_add(1));
        for row_idx in 1..row_end {
            let mut original_preview = Vec::with_capacity(width);
            let mut masked = Vec::with_capacity(width);
            for col_idx in 0..width {
                let original = range
                    .get((row_idx, col_idx))
                    .map(cell_to_string)
                    .unwrap_or_default();
                original_preview.push(if original.is_empty() {
                    None
                } else {
                    Some(original.clone())
                });
                masked.push(mask_for_position(
                    &original,
                    compiled,
                    &structure.name,
                    row_idx,
                    col_idx,
                ));
            }
            preview_rows.push(PreviewRow {
                original_preview,
                masked,
                row_index: (row_idx + 1) as u32,
                sheet: structure.name.clone(),
            });
        }
    }

    Ok(preview_rows)
}

fn build_excel_artifacts(
    input_path: &Path,
    display_name: &str,
    original_bytes: &[u8],
    config: &ExcelMaskingConfig,
    artifact_id: &str,
) -> Result<BuiltExcelArtifacts, ExcelBuildFailure> {
    let structures = parse_excel_structure_detailed(input_path)?;
    let compiled = compile_workbook_policy(&structures, config);
    if !compiled.conflicts.is_empty() {
        return Err(build_failure(
            StatusCode::BAD_REQUEST,
            "EXCEL_BINDING_AMBIGUOUS",
            "Excel column bindings are ambiguous; review preview conflicts and retry",
            false,
        ));
    }
    let temp = tempdir().map_err(|e| {
        build_failure(
            StatusCode::INTERNAL_SERVER_ERROR,
            "RUNTIME_STORAGE_FAILED",
            format!("Failed to create temp dir: {e}"),
            true,
        )
    })?;

    let (ascii_stem, input_display_name_hash8) =
        ascii_artifact_stem(display_name, original_bytes, artifact_id);
    let masked_filename = format!("{ascii_stem}{DEFAULT_SUFFIX}.xlsx");
    let masked_path = temp.path().join(&masked_filename);

    let (outcome, entries) = if is_csv_path(input_path) {
        build_csv_masked_artifact(input_path, &structures, &compiled, &masked_path)?
    } else if is_xls_path(input_path) {
        build_xls_masked_artifact(input_path, &compiled, &masked_path)?
    } else {
        build_xlsx_masked_artifact(input_path, &structures, &compiled, &masked_path)?
    };

    let masked_bytes = fs::read(&masked_path).map_err(|e| {
        build_failure(
            StatusCode::INTERNAL_SERVER_ERROR,
            "RUNTIME_STORAGE_FAILED",
            format!("Failed to read masked workbook: {e}"),
            true,
        )
    })?;
    let report_md = excel_style_core::build_report_md(&outcome);

    let passphrase = effective_passphrase(config).map_err(|msg| {
        build_failure(StatusCode::BAD_REQUEST, "EXCEL_CONFIG_INVALID", msg, false)
    })?;

    let ecmap_header = EcmapHeaderV1 {
        version: "1.2".to_string(),
        original_sha256: sha256_hex(original_bytes),
        masked_sha256: sha256_hex(&masked_bytes),
        source_encryption_key_source: canonical_key_source(&config.key_mode).to_string(),
        passphrase_domain_hint8: domain_hint8(&passphrase, b"ECMAP_V1\0"),
        source_retained: config.retain_encrypted_source,
    };
    let masked_entity_count = entries.len();
    let ecmap_doc = EcmapDocumentV1 {
        header: ecmap_header,
        entries,
    };
    let ecmap_bytes = encrypt_scoped(
        ECMAP_MAGIC,
        &serde_json::to_vec(&ecmap_doc).map_err(|e| {
            build_failure(
                StatusCode::INTERNAL_SERVER_ERROR,
                "RUNTIME_STORAGE_FAILED",
                format!("Failed to serialize ecmap: {e}"),
                true,
            )
        })?,
        &passphrase,
        b"ECMAP_V1\0",
    )
    .map_err(|msg| {
        build_failure(
            StatusCode::INTERNAL_SERVER_ERROR,
            "RUNTIME_STORAGE_FAILED",
            msg,
            true,
        )
    })?;

    let encrypted_source_bytes = if config.retain_encrypted_source {
        Some(
            encrypt_scoped(ENCSRC_MAGIC, original_bytes, &passphrase, b"ENCSRC_V1\0").map_err(
                |msg| {
                    build_failure(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "RUNTIME_STORAGE_FAILED",
                        msg,
                        true,
                    )
                },
            )?,
        )
    } else {
        None
    };
    let masked_sha256 = sha256_hex(&masked_bytes);
    let ecmap_sha256 = sha256_hex(&ecmap_bytes);

    let mut members = vec![
        ExcelArtifactMemberPayload {
            kind: ExcelArtifactMemberKind::MaskedWorkbook,
            display_name: masked_filename,
            bytes: masked_bytes,
            sha256: masked_sha256,
        },
        ExcelArtifactMemberPayload {
            kind: ExcelArtifactMemberKind::Ecmap,
            display_name: format!("{ascii_stem}{DEFAULT_SUFFIX}.ecmap"),
            bytes: ecmap_bytes,
            sha256: ecmap_sha256,
        },
        ExcelArtifactMemberPayload {
            kind: ExcelArtifactMemberKind::Report,
            display_name: format!("{ascii_stem}{DEFAULT_SUFFIX}_report.md"),
            bytes: report_md.into_bytes(),
            sha256: String::new(),
        },
    ];
    if let Some(encsrc) = encrypted_source_bytes {
        members.push(ExcelArtifactMemberPayload {
            kind: ExcelArtifactMemberKind::EncryptedSource,
            display_name: format!("{ascii_stem}{DEFAULT_SUFFIX}.encrypted_src"),
            bytes: encsrc,
            sha256: String::new(),
        });
    }
    for member in &mut members {
        if member.sha256.is_empty() {
            member.sha256 = sha256_hex(&member.bytes);
        }
    }
    Ok(BuiltExcelArtifacts {
        ascii_stem,
        input_display_name_hash8,
        masked_entity_count,
        members,
    })
}

/// R3 (TASK-EXCEL-P0-DYNAMIC-FAILURES-CLOSEOUT-001): `.xls` is legacy OLE,
/// not an OOXML ZIP, so it can never go through `rewrite_clone_inject`
/// (that path is `.xlsx`-only and silently produced a headers-only,
/// single-sheet fallback for `.xls` before this fix). This instead uses the
/// shared `excel_style_core::rewrite_legacy_xls_with_mask`, which reads
/// every sheet in full, masks every data cell via the same
/// `mask_for_position`/`compiled_strategy_for_position` used by `.xlsx`,
/// and returns the exact list of changed cells the output workbook was
/// built from — `.ecmap` entries below are built from that same list, not
/// a second independent pass over the file.
fn build_xls_masked_artifact(
    input_path: &Path,
    compiled: &CompiledWorkbookPolicy,
    masked_path: &Path,
) -> Result<(RewriteOutcome, Vec<EcmapEntryV1>), ExcelBuildFailure> {
    let (outcome, changes) = excel_style_core::rewrite_legacy_xls_with_mask(
        input_path,
        masked_path,
        |sheet_name, row_idx, col_idx, original| {
            mask_for_position(original, compiled, sheet_name, row_idx, col_idx)
        },
    )
    .map_err(|e| {
        build_failure(
            StatusCode::BAD_REQUEST,
            "INPUT_CORRUPTED",
            format!("Failed to process legacy xls file: {e}"),
            false,
        )
    })?;

    let entries = changes
        .into_iter()
        .map(|change| {
            let row_number = (change.row_idx + 1) as u32;
            let col_number = (change.col_idx + 1) as u32;
            let strategy = compiled_strategy_for_position(
                compiled,
                &change.sheet,
                change.row_idx,
                change.col_idx,
            )
            .map(|(strategy, _)| strategy)
            .unwrap_or_else(|| "FULL_MASK".to_string());
            EcmapEntryV1 {
                cell_ref: cell_ref_a1(row_number, col_number),
                original_sha256: sha256_hex(change.original.as_bytes()),
                original_preview: preview_preview(&change.original, 8),
                masked: change.masked,
                strategy_id: strategy,
                col_index: col_number,
                row_index: row_number,
                sheet: change.sheet,
            }
        })
        .collect();

    Ok((outcome, entries))
}

fn build_xlsx_masked_artifact(
    input_path: &Path,
    structures: &[SheetDef],
    compiled: &CompiledWorkbookPolicy,
    masked_path: &Path,
) -> Result<(RewriteOutcome, Vec<EcmapEntryV1>), ExcelBuildFailure> {
    let mut workbook = open_workbook_auto(input_path).map_err(|e| {
        build_failure(
            StatusCode::BAD_REQUEST,
            "INPUT_CORRUPTED",
            format!("Failed to open Excel file: {e}"),
            false,
        )
    })?;

    let mut replacements: HashMap<CellKey, String> = HashMap::new();
    let mut entries = Vec::new();

    for structure in structures {
        let range = workbook.worksheet_range(&structure.name).map_err(|e| {
            build_failure(
                StatusCode::BAD_REQUEST,
                "INPUT_CORRUPTED",
                format!("Worksheet '{}' error: {e}", structure.name),
                false,
            )
        })?;
        let (height, width) = range.get_size();
        for row_idx in 1..height {
            for col_idx in 0..width {
                let original = range
                    .get((row_idx, col_idx))
                    .map(cell_to_string)
                    .unwrap_or_default();
                if original.is_empty() {
                    continue;
                }
                let masked =
                    mask_for_position(&original, compiled, &structure.name, row_idx, col_idx);
                if masked == original {
                    continue;
                }
                let row_number = (row_idx + 1) as u32;
                let col_number = (col_idx + 1) as u32;
                replacements.insert(
                    CellKey {
                        sheet: structure.name.clone(),
                        row: row_number,
                        col: col_number,
                    },
                    masked.clone(),
                );
                let strategy =
                    compiled_strategy_for_position(compiled, &structure.name, row_idx, col_idx)
                        .map(|(strategy, _)| strategy)
                        .unwrap_or_else(|| "FULL_MASK".to_string());
                entries.push(EcmapEntryV1 {
                    cell_ref: cell_ref_a1(row_number, col_number),
                    original_sha256: sha256_hex(original.as_bytes()),
                    original_preview: preview_preview(&original, 8),
                    masked,
                    strategy_id: strategy,
                    col_index: col_number,
                    row_index: row_number,
                    sheet: structure.name.clone(),
                });
            }
        }
    }

    let outcome: RewriteOutcome =
        excel_style_core::rewrite_clone_inject(input_path, masked_path, &replacements)
            .unwrap_or_else(|e| {
                let headers = structures
                    .first()
                    .map(|s| s.headers.clone())
                    .unwrap_or_default();
                let mut fallback =
                    excel_style_core::fallback_xlsxwriter_full(&headers, &Vec::new(), masked_path)
                        .unwrap_or_default();
                fallback.warnings.push(format!("克隆注入失败，已回退: {e}"));
                fallback
            });

    Ok((outcome, entries))
}

/// CSV has no OOXML to clone-inject into, so the masked output is always
/// built via `fallback_xlsxwriter_full` (never `rewrite_clone_inject`),
/// reusing the same column-rule/cell-override masking logic as `.xlsx`.
fn build_csv_masked_artifact(
    input_path: &Path,
    structures: &[SheetDef],
    compiled: &CompiledWorkbookPolicy,
    masked_path: &Path,
) -> Result<(RewriteOutcome, Vec<EcmapEntryV1>), ExcelBuildFailure> {
    let sheet_name = structures
        .first()
        .map(|s| s.name.as_str())
        .unwrap_or("Sheet1");

    let (header, rows) = table_reader::read_csv_all_rows(input_path).map_err(|e| {
        build_failure(
            StatusCode::BAD_REQUEST,
            "INPUT_CORRUPTED",
            format!("Failed to read CSV file: {e}"),
            false,
        )
    })?;

    let mut entries = Vec::new();
    let mut masked_rows: Vec<Vec<String>> = Vec::with_capacity(rows.len());
    for (i, row) in rows.iter().enumerate() {
        // row_idx=1 for the first data row mirrors the calamine-range
        // convention (row_idx=0 is the header) used by the xlsx path above.
        let row_idx = i + 1;
        let row_number = (row_idx + 1) as u32;
        let mut masked_row = Vec::with_capacity(row.len());
        for (col_idx, original) in row.iter().enumerate() {
            let masked = mask_for_position(original, compiled, sheet_name, row_idx, col_idx);
            if !original.is_empty() && masked != *original {
                let col_number = (col_idx + 1) as u32;
                let strategy = compiled_strategy_for_position(compiled, sheet_name, row_idx, col_idx)
                    .map(|(strategy, _)| strategy)
                    .unwrap_or_else(|| "FULL_MASK".to_string());
                entries.push(EcmapEntryV1 {
                    cell_ref: cell_ref_a1(row_number, col_number),
                    original_sha256: sha256_hex(original.as_bytes()),
                    original_preview: preview_preview(original, 8),
                    masked: masked.clone(),
                    strategy_id: strategy,
                    col_index: col_number,
                    row_index: row_number,
                    sheet: sheet_name.to_string(),
                });
            }
            masked_row.push(masked);
        }
        masked_rows.push(masked_row);
    }

    let outcome = excel_style_core::fallback_xlsxwriter_full(&header, &masked_rows, masked_path)
        .map_err(|e| {
            build_failure(
                StatusCode::INTERNAL_SERVER_ERROR,
                "RUNTIME_STORAGE_FAILED",
                format!("Failed to write masked CSV output: {e}"),
                true,
            )
        })?;

    Ok((outcome, entries))
}

fn compile_workbook_policy(
    structures: &[SheetDef],
    config: &ExcelMaskingConfig,
) -> CompiledWorkbookPolicy {
    let mut sheets = HashMap::new();
    let mut conflicts = Vec::new();
    let structure_by_name: HashMap<&str, &SheetDef> = structures
        .iter()
        .map(|structure| (structure.name.as_str(), structure))
        .collect();

    for policy in &config.sheet_policies {
        let Some(structure) = structure_by_name.get(policy.sheet.as_str()) else {
            conflicts.push(format!(
                "工作表 '{}' 不存在，无法绑定列策略。",
                policy.sheet
            ));
            continue;
        };
        let mut header_map: HashMap<String, Vec<usize>> = HashMap::new();
        for (idx, header) in structure.headers.iter().enumerate() {
            header_map
                .entry(normalize_header(header))
                .or_default()
                .push(idx);
        }

        let mut seen_columns = HashSet::new();
        let mut column_bindings = HashMap::new();
        for rule in &policy.column_rules {
            let requested_header = normalize_header(&rule.header_text);
            let explicit_header_matches = structure
                .headers
                .get(rule.col_index)
                .map(|header| normalize_header(header) == requested_header)
                .unwrap_or(false);
            let requested_matches = header_map
                .get(&requested_header)
                .cloned()
                .unwrap_or_default();
            let resolved_col =
                if structure.headers.get(rule.col_index).is_some() && explicit_header_matches {
                    Some(rule.col_index)
                } else if !requested_header.is_empty() && requested_matches.len() == 1 {
                    if structure.headers.get(rule.col_index).is_some()
                        && requested_matches[0] != rule.col_index
                    {
                        conflicts.push(format!(
                            "工作表 '{}' 的列规则 '{}' 同时指向列 {} 与表头 '{}', 绑定冲突。",
                            policy.sheet,
                            rule.strategy,
                            rule.col_index + 1,
                            rule.header_text
                        ));
                        None
                    } else {
                        Some(requested_matches[0])
                    }
                } else if requested_matches.len() > 1 {
                    conflicts.push(format!(
                        "工作表 '{}' 的表头 '{}' 匹配到多个列，无法确定绑定。",
                        policy.sheet, rule.header_text
                    ));
                    None
                } else if structure.headers.get(rule.col_index).is_some() {
                    Some(rule.col_index)
                } else {
                    conflicts.push(format!(
                        "工作表 '{}' 的列规则 '{}' 既没有命中列索引，也没有命中表头 '{}'",
                        policy.sheet, rule.strategy, rule.header_text
                    ));
                    None
                };
            if let Some(resolved_col) = resolved_col {
                if !seen_columns.insert(resolved_col) {
                    conflicts.push(format!(
                        "工作表 '{}' 的多条列规则命中了同一列 {}。",
                        policy.sheet,
                        resolved_col + 1
                    ));
                    continue;
                }
                column_bindings.insert(
                    resolved_col,
                    CompiledColumnBinding {
                        strategy: rule.strategy.clone(),
                        replacement: rule.replacement.clone(),
                    },
                );
            }
        }

        let mut cell_overrides = HashMap::new();
        for rule in &policy.cell_overrides {
            if cell_overrides
                .insert(
                    (rule.row, rule.col),
                    (rule.strategy.clone(), rule.replacement.clone()),
                )
                .is_some()
            {
                conflicts.push(format!(
                    "{}!{} 存在重复单元格覆盖配置。",
                    policy.sheet,
                    cell_ref_a1((rule.row + 1) as u32, (rule.col + 1) as u32)
                ));
            }
            if column_bindings.contains_key(&rule.col) {
                conflicts.push(format!(
                    "{}!{} 同时命中列策略与单元格覆盖，单元格覆盖优先。",
                    policy.sheet,
                    cell_ref_a1((rule.row + 1) as u32, (rule.col + 1) as u32)
                ));
            }
        }

        sheets.insert(
            policy.sheet.clone(),
            CompiledSheetPolicy {
                column_bindings,
                cell_overrides,
            },
        );
    }

    CompiledWorkbookPolicy { sheets, conflicts }
}

fn compiled_strategy_for_position(
    compiled: &CompiledWorkbookPolicy,
    sheet_name: &str,
    row_idx: usize,
    col_idx: usize,
) -> Option<(String, Option<String>)> {
    let policy = compiled.sheets.get(sheet_name)?;
    if let Some((strategy, replacement)) = policy.cell_overrides.get(&(row_idx, col_idx)) {
        return Some((strategy.clone(), replacement.clone()));
    }
    policy
        .column_bindings
        .get(&col_idx)
        .map(|binding| (binding.strategy.clone(), binding.replacement.clone()))
}

fn mask_for_position(
    value: &str,
    compiled: &CompiledWorkbookPolicy,
    sheet_name: &str,
    row_idx: usize,
    col_idx: usize,
) -> String {
    if value.is_empty() {
        return String::new();
    }
    let Some((strategy, replacement)) =
        compiled_strategy_for_position(compiled, sheet_name, row_idx, col_idx)
    else {
        return value.to_string();
    };
    apply_strategy(value, &strategy, replacement.as_deref())
}

fn apply_strategy(value: &str, strategy: &str, replacement: Option<&str>) -> String {
    match strategy {
        "FULL_MASK" | "BANK_CARD" | "EMAIL" | "ADDRESS" | "COMPLIANCE_ID" => replacement
            .map(str::to_string)
            .unwrap_or_else(|| mask_middle(value, 0, 0)),
        "PHONE_MID4" => mask_phone(value),
        "IDCARD_MID10" => mask_idcard(value),
        "BANKCARD_LAST4" => mask_middle(value, 0, 4),
        "EMAIL_USER_MASK" => mask_email(value),
        "DEFAULT_VALUE" => replacement.unwrap_or("[MASKED]").to_string(),
        "CLEAR_COL" => String::new(),
        _ => replacement
            .map(str::to_string)
            .unwrap_or_else(|| mask_middle(value, 0, 0)),
    }
}

/// 仅当值恰好是 11 位 ASCII 数字时才脱敏（保留前 3、后 4）；其他任何情况
/// （空值、长度不符、含字母/符号等）一律保持原值不变。
fn mask_phone(value: &str) -> String {
    let is_11_ascii_digits =
        value.len() == 11 && value.chars().all(|c| c.is_ascii_digit());
    if is_11_ascii_digits {
        mask_middle(value, 3, 4)
    } else {
        value.to_string()
    }
}

/// 18 位保留前 6、后 4；15 位保留前 3、后 4；其他长度（含空值）保持原值不变。
pub(crate) fn mask_idcard(value: &str) -> String {
    match value.chars().count() {
        18 => mask_middle(value, 6, 4),
        15 => mask_middle(value, 3, 4),
        _ => value.to_string(),
    }
}

fn mask_middle(value: &str, keep_prefix: usize, keep_suffix: usize) -> String {
    let chars: Vec<char> = value.chars().collect();
    if chars.is_empty() {
        return String::new();
    }
    if keep_prefix + keep_suffix >= chars.len() {
        return "*".repeat(chars.len().max(1));
    }
    let mut output = String::new();
    for ch in chars.iter().take(keep_prefix) {
        output.push(*ch);
    }
    output.push_str(&"*".repeat(chars.len() - keep_prefix - keep_suffix));
    for ch in chars.iter().skip(chars.len().saturating_sub(keep_suffix)) {
        output.push(*ch);
    }
    output
}

fn mask_email(value: &str) -> String {
    let Some((user, domain)) = value.split_once('@') else {
        return mask_middle(value, 1, 1);
    };
    let masked_user = if user.chars().count() <= 2 {
        "*".repeat(user.chars().count().max(1))
    } else {
        mask_middle(user, 1, 1)
    };
    format!("{masked_user}@{domain}")
}

fn normalize_header(value: &str) -> String {
    value
        .trim()
        .to_lowercase()
        .chars()
        .filter(|ch| ch.is_alphanumeric())
        .collect()
}

fn sanitize_ascii_stem(value: &str) -> String {
    let mut output = String::new();
    let mut last_was_sep = false;
    for ch in value.chars() {
        let normalized = ch.to_ascii_lowercase();
        if normalized.is_ascii_alphanumeric() {
            output.push(normalized);
            last_was_sep = false;
        } else if !last_was_sep {
            output.push('-');
            last_was_sep = true;
        }
    }
    output.trim_matches('-').to_string()
}

fn ascii_artifact_stem(
    display_name: &str,
    original_bytes: &[u8],
    artifact_id: &str,
) -> (String, String) {
    let input_display_name_hash8 = sha256_hex(display_name.as_bytes())[0..8].to_string();
    let input_sha8 = sha256_hex(original_bytes)[0..8].to_string();
    let stem = Path::new(display_name)
        .file_stem()
        .and_then(|segment| segment.to_str())
        .map(sanitize_ascii_stem)
        .filter(|segment| !segment.is_empty())
        .unwrap_or_else(|| "excel-file".to_string());
    let job8 = artifact_id.chars().take(8).collect::<String>();
    (
        format!("{stem}-{input_sha8}-{job8}"),
        input_display_name_hash8,
    )
}

fn member_content_type(kind: ExcelArtifactMemberKind) -> &'static str {
    match kind {
        ExcelArtifactMemberKind::MaskedWorkbook => {
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
        }
        ExcelArtifactMemberKind::Report => "text/markdown; charset=utf-8",
        ExcelArtifactMemberKind::Ecmap | ExcelArtifactMemberKind::EncryptedSource => {
            "application/octet-stream"
        }
    }
}

fn store_rejection(error: StoreError) -> Rejection {
    match error {
        StoreError::NotFound => crate::api_error(
            StatusCode::NOT_FOUND,
            "NOT_FOUND",
            "The requested resource was not found",
            false,
        ),
        StoreError::RetryConflict => crate::api_error(
            StatusCode::CONFLICT,
            "RETRY_NOT_ALLOWED",
            "Only failed files can be retried",
            false,
        ),
        StoreError::Storage
        | StoreError::InvalidState
        | StoreError::PreviewAlreadyConfirmed
        | StoreError::SensitiveTermInvalid(_)
        | StoreError::SensitiveTermDuplicate
        | StoreError::SensitiveTermNotFound
        | StoreError::SensitiveTermsImportInvalid(_)
        | StoreError::InputLimitExceeded => crate::api_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "RUNTIME_STORAGE_FAILED",
            "Runtime storage operation failed",
            true,
        ),
    }
}

fn effective_passphrase(config: &ExcelMaskingConfig) -> Result<String, String> {
    match config.key_mode {
        EncSourceKeyMode::SandboxReused => read_or_create_secret("excel-browser-sandbox.secret"),
        EncSourceKeyMode::SecondaryPassphrase => config
            .secondary_passphrase
            .clone()
            .filter(|s| !s.trim().is_empty())
            .ok_or_else(|| "独立二级口令不能为空".to_string()),
        EncSourceKeyMode::DeviceKey => read_or_create_secret("excel-browser-device.secret"),
    }
}

fn read_or_create_secret(filename: &str) -> Result<String, String> {
    let data_root = std::env::var_os("VAULT_RUNTIME_DATA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("enterprise-data"));
    fs::create_dir_all(&data_root)
        .map_err(|e| format!("Failed to create runtime data dir: {e}"))?;
    let path = data_root.join(filename);
    if path.exists() {
        let value = fs::read_to_string(&path).map_err(|e| format!("Failed to read secret: {e}"))?;
        let trimmed = value.trim().to_string();
        if !trimmed.is_empty() {
            return Ok(trimmed);
        }
    }
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    let secret: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    fs::write(&path, &secret).map_err(|e| format!("Failed to persist secret: {e}"))?;
    Ok(secret)
}

fn encrypt_scoped(
    magic: &[u8],
    plaintext: &[u8],
    passphrase: &str,
    domain: &[u8],
) -> Result<Vec<u8>, String> {
    let mut salt = [0u8; SALT_LEN];
    rand::thread_rng().fill_bytes(&mut salt);
    let mut nonce_bytes = [0u8; NONCE_LEN];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let key = derive_key_scoped(passphrase, &salt, domain)?;
    let cipher = Aes256Gcm::new_from_slice(&key).map_err(|e| format!("Cipher init error: {e}"))?;
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher
        .encrypt(nonce, plaintext)
        .map_err(|e| format!("Encrypt error: {e}"))?;
    let mut output = Vec::with_capacity(magic.len() + SALT_LEN + NONCE_LEN + ciphertext.len());
    output.extend_from_slice(magic);
    output.extend_from_slice(&salt);
    output.extend_from_slice(&nonce_bytes);
    output.extend_from_slice(&ciphertext);
    Ok(output)
}

fn derive_key_scoped(passphrase: &str, salt: &[u8], domain: &[u8]) -> Result<[u8; 32], String> {
    let scoped_input = [domain, passphrase.as_bytes()].concat();
    let mut key = [0u8; 32];
    pbkdf2::<Hmac<Sha256>>(&scoped_input, salt, 200_000, &mut key)
        .map_err(|e| format!("PBKDF2 error: {e}"))?;
    Ok(key)
}

fn domain_hint8(passphrase: &str, domain: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(passphrase.as_bytes());
    hasher.update(b"HINT");
    hasher.update(domain);
    let digest = hasher.finalize();
    digest[..8]
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .join("")
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .join("")
}

fn preview_preview(value: &str, limit: usize) -> String {
    let chars: Vec<char> = value.chars().collect();
    if chars.len() <= limit {
        value.to_string()
    } else {
        chars.into_iter().take(limit).collect()
    }
}

fn cell_ref_a1(row: u32, col: u32) -> String {
    format!("{}{}", col_letters(col), row)
}

fn col_letters(index_1: u32) -> String {
    let mut n = index_1;
    let mut letters = String::new();
    while n > 0 {
        let rem = (n - 1) % 26;
        letters.insert(0, (b'A' + rem as u8) as char);
        n = (n - 1) / 26;
    }
    letters
}

fn cell_to_string(cell: &Data) -> String {
    match cell {
        Data::Int(i) => i.to_string(),
        Data::Float(f) => f.to_string(),
        Data::String(s) => s.clone(),
        Data::Bool(b) => b.to_string(),
        Data::DateTime(dt) => dt.to_string(),
        Data::DateTimeIso(s) => s.clone(),
        Data::DurationIso(s) => s.clone(),
        Data::Error(e) => format!("Error: {e:?}"),
        Data::Empty => String::new(),
    }
}

#[cfg(test)]
fn decrypt_scoped_for_test(
    data: &[u8],
    magic: &[u8],
    passphrase: &str,
    domain: &[u8],
) -> Result<Vec<u8>, String> {
    let min_len = magic.len() + SALT_LEN + NONCE_LEN + 16;
    if data.len() < min_len || !data.starts_with(magic) {
        return Err("scoped data too short or magic mismatch".to_string());
    }
    let offset = magic.len();
    let salt = &data[offset..offset + SALT_LEN];
    let nonce_bytes = &data[offset + SALT_LEN..offset + SALT_LEN + NONCE_LEN];
    let ciphertext = &data[offset + SALT_LEN + NONCE_LEN..];
    let key = derive_key_scoped(passphrase, salt, domain)?;
    let cipher = Aes256Gcm::new_from_slice(&key).map_err(|e| format!("Cipher init error: {e}"))?;
    let nonce = Nonce::from_slice(nonce_bytes);
    cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| "scoped decrypt failed".to_string())
}

#[cfg(test)]
pub(crate) fn decrypt_ecmap_header_for_test(
    ecmap_bytes: &[u8],
    passphrase: &str,
) -> Result<EcmapHeaderV1, String> {
    let plain = decrypt_scoped_for_test(ecmap_bytes, ECMAP_MAGIC, passphrase, b"ECMAP_V1\0")?;
    let doc: EcmapDocumentV1 = serde_json::from_slice(&plain).map_err(|e| e.to_string())?;
    Ok(doc.header)
}

/// (sheet, row_index, col_index, masked) for every `.ecmap` entry — used by
/// R3 tests to cross-check that every mapped coordinate/masked value is
/// actually present in the downloaded masked workbook, from the same
/// decrypted document, not a second independent assumption about its shape.
#[cfg(test)]
pub(crate) fn decrypt_ecmap_entries_for_test(
    ecmap_bytes: &[u8],
    passphrase: &str,
) -> Result<Vec<(String, u32, u32, String)>, String> {
    let plain = decrypt_scoped_for_test(ecmap_bytes, ECMAP_MAGIC, passphrase, b"ECMAP_V1\0")?;
    let doc: EcmapDocumentV1 = serde_json::from_slice(&plain).map_err(|e| e.to_string())?;
    Ok(doc
        .entries
        .into_iter()
        .map(|e| (e.sheet, e.row_index, e.col_index, e.masked))
        .collect())
}
