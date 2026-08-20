/**
 * 浏览器 HTTP 适配器使用的 Runtime 响应类型。
 *
 * 字段与 `apps/vault-runtime-api` 的 `HealthResponse` / `OcrStatusResponse`
 * 保持一致（见 `service-contracts`），
 * 仅用于承载服务端返回的已有安全状态字段，不新增任何服务端未提供的判断。
 */

export interface RuntimeHealthResponse {
  status: string;
  version: string;
}

export interface RuntimeOcrStatusResponse {
  status: "ready" | "invalid" | "unavailable" | string;
  model_ready: boolean;
  timeout_secs: number;
  max_pages: number;
}

/**
 * 浏览器多文件脱敏批处理链路使用的 Runtime 响应类型。
 *
 * 字段与 `service-contracts`（`FileStatus`/`BatchStatus`/`RulesResponse`/
 * `CreateBatchResponse`/`BatchSummary`/`BatchDetail`/`BatchFile`/
 * `RetryResponse`/`ErrorResponse`）
 * 逐字段保持一致，只做类型层面的浏览器侧投影，不新增服务端未提供的字段
 * 或状态枚举。
 */

export type RuntimeFileStatus = "Pending" | "Processing" | "Completed" | "Failed";

export type RuntimeBatchStatus = "Running" | "Completed" | "CompletedWithErrors" | "Failed";

export type RuntimeArtifactKind = "markdown" | "excel_bundle_manifest";

export type RuntimeExcelArtifactMemberKind =
  | "masked_workbook"
  | "ecmap"
  | "encrypted_source"
  | "report";

export interface RuntimeRuleMetadata {
  id: string;
  name: string;
  enabled_by_default: boolean;
}

export interface RuntimeRulesResponse {
  rules: RuntimeRuleMetadata[];
}

export interface RuntimeCreatedFile {
  file_id: string;
  display_name: string;
}

export interface RuntimeCreateBatchResponse {
  batch_id: string;
  files: RuntimeCreatedFile[];
}

export interface RuntimeBatchSummary {
  batch_id: string;
  status: RuntimeBatchStatus;
  file_count: number;
  completed_count: number;
  failed_count: number;
  masked_entity_count: number;
  created_at: string;
  updated_at: string;
}

export interface RuntimeBatchFile {
  file_id: string;
  display_name: string;
  input_format: string;
  status: RuntimeFileStatus;
  attempt: number;
  masked_entity_count: number | null;
  artifact_id: string | null;
  artifact_kind: RuntimeArtifactKind | null;
  error_code: string | null;
  error_message: string | null;
  restore_available: boolean;
}

export interface RuntimeBatchDetail {
  batch: RuntimeBatchSummary;
  files: RuntimeBatchFile[];
}

/** `GET /api/v1/batches` 列表响应，逐字段镜像 `service-contracts::BatchListResponse`。 */
export interface RuntimeBatchListResponse {
  batches: RuntimeBatchSummary[];
}

export interface RuntimeRetryResponse {
  file_id: string;
  status: RuntimeFileStatus;
  attempt: number;
}

export interface RuntimeExcelPersistedFile {
  kind: RuntimeExcelArtifactMemberKind;
  display_name: string;
  size_bytes: number;
}

export interface RuntimeExcelPersistArtifactsResponse {
  batch_id: string;
  file_id: string;
  artifact_id: string;
  persisted_files: RuntimeExcelPersistedFile[];
  saved_directory_hint: string;
}

export interface RuntimeExcelArtifactMembersResponse {
  artifact_id: string;
  batch_id: string;
  saved_directory_hint: string;
  persisted_files: RuntimeExcelPersistedFile[];
}

/** 服务端结构化错误体，仅承载 `code`/`message`/`retryable` 三个安全字段。 */
export interface RuntimeErrorBody {
  code: string;
  message: string;
  retryable: boolean;
}

/**
 * 浏览器两阶段预览/确认链路使用的 Runtime 响应类型。
 *
 * 字段与 `service-contracts`（`PreviewSessionStatus`/`PreviewFileStatus`/
 * `CreatePreviewResponse`/`PreviewDetail`/`PreviewFile`/
 * `ConfirmPreviewResponse`）逐字段保持一致，只做类型层面的浏览器侧投影。
 * 这些类型绝不携带 `original`/`mapping`/`.cmap`/路径等字段——服务端本身
 * 就不会在这些响应里返回它们。
 */

export type RuntimePreviewSessionStatus =
  | "Processing"
  | "Ready"
  | "ReadyWithErrors"
  | "Failed"
  | "Confirming"
  | "Confirmed";

export type RuntimePreviewFileStatus = "Pending" | "Processing" | "Ready" | "Failed";

export interface RuntimeCreatedPreviewFile {
  file_id: string;
  display_name: string;
}

export interface RuntimeCreatePreviewResponse {
  preview_id: string;
  files: RuntimeCreatedPreviewFile[];
  expires_at: string;
}

export interface RuntimePreviewFile {
  file_id: string;
  display_name: string;
  input_format: string;
  status: RuntimePreviewFileStatus;
  masked_entity_count: number | null;
  error_code: string | null;
  error_message: string | null;
  content_available: boolean;
}

export interface RuntimePreviewDetail {
  preview_id: string;
  status: RuntimePreviewSessionStatus;
  file_count: number;
  ready_count: number;
  failed_count: number;
  masked_entity_count: number;
  created_at: string;
  expires_at: string;
  files: RuntimePreviewFile[];
}

export interface RuntimeConfirmPreviewResponse {
  preview_id: string;
  batch_id: string;
}

/**
 * 浏览器敏感词库 CRUD/CSV 导入导出使用的 Runtime 响应类型。
 *
 * 字段与 `service-contracts`（`SensitiveTerm`/`CreateSensitiveTermRequest`/
 * `UpdateSensitiveTermRequest`/`SensitiveTermsResponse`/
 * `SensitiveTermCategoriesResponse`/`SensitiveTermsStats`/
 * `SensitiveTermsImportResponse`）逐字段保持一致，只做类型层面的浏览器侧投影。
 */

export interface RuntimeSensitiveTerm {
  id: string;
  term: string;
  category: string;
  description: string | null;
  enabled: boolean;
  created_at: string;
  updated_at: string;
}

export interface RuntimeCreateSensitiveTermRequest {
  term: string;
  category: string;
  description: string | null;
}

export interface RuntimeUpdateSensitiveTermRequest {
  term?: string;
  category?: string;
  description?: string;
  enabled?: boolean;
}

export interface RuntimeSensitiveTermsResponse {
  terms: RuntimeSensitiveTerm[];
}

export interface RuntimeSensitiveTermCategoriesResponse {
  categories: string[];
}

export interface RuntimeSensitiveTermsStats {
  total: number;
  enabled: number;
  disabled: number;
  categories: number;
}

export interface RuntimeSensitiveTermsImportResponse {
  imported_count: number;
}

/**
 * 浏览器操作日志使用的 Runtime 响应类型。
 *
 * 字段与 `service-contracts`（`OperationLogLevel`/`OperationLogEntry`/
 * `OperationLogListResponse`/`OperationLogStatistics`/
 * `OperationLogStorageStatus`/`ClearOperationLogsResponse`）逐字段保持一致，
 * 只做类型层面的浏览器侧投影。这些类型直接投影 Runtime 已有的
 * `job_events`/`restore_events`/`batches`/`batch_files`，不建立平行日志表；
 * 字段本身已经是服务端安全投影，绝不包含原文、mapping、敏感词内容、完整
 * 路径、口令或 Token。
 */

export type RuntimeOperationLogLevel = "info" | "success" | "warning" | "error";

export interface RuntimeOperationLogEntry {
  event_id: string;
  event_type: string;
  timestamp: string;
  level: RuntimeOperationLogLevel;
  batch_id: string | null;
  file_id: string | null;
  display_name: string | null;
  input_format: string | null;
  status: string;
  masked_entity_count: number | null;
  error_code: string | null;
  restored_entity_count: number | null;
}

export interface RuntimeOperationLogListResponse {
  entries: RuntimeOperationLogEntry[];
  page: number;
  page_size: number;
  total_count: number;
  total_pages: number;
}

export interface RuntimeOperationLogStatistics {
  total_files: number;
  successful_files: number;
  failed_files: number;
  total_masked_items: number;
  success_rate: number;
  recent_files_7days: number;
  average_processing_time_ms: number;
}

export interface RuntimeOperationLogStorageStatus {
  status: "ready" | "error" | string;
  event_count: number;
  runtime_version: string;
}

export interface RuntimeClearOperationLogsResponse {
  deleted_job_events: number;
  deleted_restore_events: number;
}

/**
 * 浏览器沙箱/PIN 使用的 Runtime 响应类型。
 *
 * 字段与 `service-contracts`（`SandboxStatusResponse`/`SetSandboxPinRequest`/
 * `UnlockSandboxRequest`/`ClearSandboxPinRequest`）逐字段保持一致，只做类型
 * 层面的浏览器侧投影。这是同一服务器系统用户共享的沙箱操作状态，不是账号
 * 登录、RBAC、多租户或 API 鉴权；这些类型绝不携带 PIN、哈希、盐或服务器
 * 路径字段——服务端本身就不会在这些响应里返回它们。
 */

export const RUNTIME_SANDBOX_STORAGE_MODE_SERVER_SYSTEM_USER = "server_system_user";

export interface RuntimeSandboxStatusResponse {
  pin_configured: boolean;
  locked: boolean;
  storage_mode: string;
  rate_limited: boolean;
  retry_after_seconds: number | null;
}

export interface RuntimeSetSandboxPinRequest {
  new_pin: string;
  current_pin?: string;
}

export interface RuntimeUnlockSandboxRequest {
  pin: string;
}

export interface RuntimeClearSandboxPinRequest {
  current_pin: string;
}

/**
 * FileBay 浏览器适配的 Runtime HTTP 契约（`/api/v1/filebay*`）。全部字段
 * 与 `service-contracts` 的 `FileBay*` 类型一一对应；从不包含 Token、
 * Authorization、完整服务器路径或远端响应体。
 */
export type RuntimeFileBayConfigStatus = "unconfigured" | "configured" | "invalid";

export interface RuntimeFileBayStatusResponse {
  status: RuntimeFileBayConfigStatus;
  configured: boolean;
  has_token: boolean;
  target_origin: string | null;
  owner: string | null;
  repo: string | null;
}

export interface RuntimeFileBayTestResponse {
  repository_exists: boolean;
}

export type RuntimeFileBayRepositoryStatus = "ready" | "created";

export interface RuntimeFileBayRepositoryResponse {
  status: RuntimeFileBayRepositoryStatus;
}

export interface RuntimeFileBayCandidate {
  artifact_id: string;
  display_name: string;
  remote_path: string;
}

export interface RuntimeFileBayCandidatesResponse {
  candidates: RuntimeFileBayCandidate[];
}

export interface RuntimeFileBayUploadRequest {
  artifact_ids: string[];
}

export interface RuntimeFileBayUploadItem {
  artifact_id: string;
  remote_path: string;
  success: boolean;
  url: string | null;
  error_code: string | null;
}

export interface RuntimeFileBayUploadResponse {
  items: RuntimeFileBayUploadItem[];
}
