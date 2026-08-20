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
use excel_style_core::{CellKey, RewriteOutcome};
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
struct EcmapHeaderV1 {
    version: String,
    original_sha256: String,
    masked_sha256: String,
    source_encryption_key_source: String,
    passphrase_domain_hint8: String,
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

fn parse_excel_structure_detailed(path: &Path) -> Result<Vec<SheetDef>, ExcelBuildFailure> {
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
    let mut workbook = open_workbook_auto(input_path).map_err(|e| {
        build_failure(
            StatusCode::BAD_REQUEST,
            "INPUT_CORRUPTED",
            format!("Failed to open Excel file: {e}"),
            false,
        )
    })?;

    let mut preview_rows = Vec::new();

    for structure in &structures {
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
                    &compiled,
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

    Ok(ExcelMaskPreview {
        preview_rows,
        conflicts: compiled.conflicts,
    })
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
    let mut workbook = open_workbook_auto(input_path).map_err(|e| {
        build_failure(
            StatusCode::BAD_REQUEST,
            "INPUT_CORRUPTED",
            format!("Failed to open Excel file: {e}"),
            false,
        )
    })?;

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

    let mut replacements: HashMap<CellKey, String> = HashMap::new();
    let mut entries = Vec::new();

    for structure in &structures {
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
                    mask_for_position(&original, &compiled, &structure.name, row_idx, col_idx);
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
                    compiled_strategy_for_position(&compiled, &structure.name, row_idx, col_idx)
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
        excel_style_core::rewrite_clone_inject(input_path, &masked_path, &replacements)
            .unwrap_or_else(|e| {
                let headers = structures
                    .first()
                    .map(|s| s.headers.clone())
                    .unwrap_or_default();
                let mut fallback =
                    excel_style_core::fallback_xlsxwriter_full(&headers, &Vec::new(), &masked_path)
                        .unwrap_or_default();
                fallback.warnings.push(format!("克隆注入失败，已回退: {e}"));
                fallback
            });

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
        source_encryption_key_source: match config.key_mode {
            EncSourceKeyMode::SandboxReused => "SandboxReused".to_string(),
            EncSourceKeyMode::SecondaryPassphrase => "SecondaryPassphrase".to_string(),
            EncSourceKeyMode::DeviceKey => "DeviceKey".to_string(),
        },
        passphrase_domain_hint8: domain_hint8(&passphrase, b"ECMAP_V1\0"),
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
        "PHONE_MID4" => mask_middle(value, 3, 4),
        "IDCARD_MID10" => mask_middle(value, 4, 4),
        "BANKCARD_LAST4" => mask_middle(value, 0, 4),
        "EMAIL_USER_MASK" => mask_email(value),
        "DEFAULT_VALUE" => replacement.unwrap_or("[MASKED]").to_string(),
        "CLEAR_COL" => String::new(),
        _ => replacement
            .map(str::to_string)
            .unwrap_or_else(|| mask_middle(value, 0, 0)),
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
