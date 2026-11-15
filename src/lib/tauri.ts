import { invoke } from "@tauri-apps/api/core";
import type {
  MaskFileOptions,
  MaskResult,
  PreviewOptions,
  PreviewResult,
  SavePreviewOptions,
  EncryptOptions,
  DecryptOptions,
  SandboxFile,
  MaskRule,
  BatchJobOptions,
  BatchStatus,
  FileBayConfig,
  FileBayConfigStatus,
  PlatformContext,
  SensitiveTerm,
  AddSensitiveTermRequest,
  UpdateSensitiveTermRequest,
  UpdateBackupSummary,
  SensitiveTermsStats,
  SheetDef,
  ExcelMaskingConfig,
  ExcelApplyResult,
  ExcelRestoreReq,
  ExcelRestoreResult,
} from "@/types/commands";
import type { LogEntry, ProcessingHistory, UserSetting, DatabaseStatistics } from "@/types/log";
import {
  toCanonicalExcelMaskPreview,
  toTauriExcelMaskingConfig,
  type TauriExcelMaskPreview,
} from "@/lib/excelMaskingContract";

// The Rust command `excel_restore_from_ecmap` (src-tauri/src/commands/excel_masking.rs)
// takes a single named parameter `restore: ExcelRestoreReq`; Tauri's invoke matches
// JS object keys to Rust parameter names, so the wrapped payload's top-level key must
// be exactly `restore` (a `req` key here is silently a different, unrecognized
// argument and the call fails with a missing-required-argument error). Extracted as a
// pure function so a targeted test can assert the exact key name without mocking
// `invoke` or Tauri itself.
export const EXCEL_RESTORE_FROM_ECMAP_COMMAND = "excel_restore_from_ecmap" as const;

export function buildExcelRestoreInvokeArgs(
  req: ExcelRestoreReq
): { restore: ExcelRestoreReq } {
  return { restore: req };
}

// The Rust command `excel_parse_structure` (src-tauri/src/commands/excel_masking.rs)
// returns `ExcelStructure { sheets: Vec<SheetDef> }` — an envelope object, not a bare
// array. Treating that envelope directly as `SheetDef[]` (the old behavior) type-checks
// but is wrong at runtime: callers doing `.find`/`.map` on it crash with
// "X.find is not a function" instead of a clear error. `extractSheetsFromExcelStructureResponse`
// is the single place that unwraps this envelope, and rejects any response missing a
// legal `sheets` array with an explicit, safe error instead of propagating a malformed
// shape further into the app.
export interface ExcelStructureResponseEnvelope {
  sheets: SheetDef[];
}

export class ExcelStructureResponseError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "ExcelStructureResponseError";
  }
}

export function extractSheetsFromExcelStructureResponse(
  response: unknown
): SheetDef[] {
  if (
    typeof response !== "object" ||
    response === null ||
    !Array.isArray((response as { sheets?: unknown }).sheets)
  ) {
    throw new ExcelStructureResponseError(
      "Excel 结构解析返回值缺少合法的 sheets 数组，拒绝继续"
    );
  }
  return (response as ExcelStructureResponseEnvelope).sheets;
}

export const tauriCommands = {
  // Masking
  maskFile: (options: MaskFileOptions) =>
    invoke<MaskResult>("mask_file", { options }),

  previewMasking: (options: PreviewOptions) =>
    invoke<PreviewResult>("preview_masking", { options }),

  savePreviewResult: (options: SavePreviewOptions) =>
    invoke<MaskResult>("save_preview_result", { options }),

  getFilePageCount: (filePath: string) =>
    invoke<number>("get_file_page_count", { filePath }),

  // Crypto
  generatePassphrase: () =>
    invoke<string>("generate_passphrase"),

  encryptMapping: (options: EncryptOptions) =>
    invoke<string>("encrypt_mapping", { options }),

  decryptMapping: (options: DecryptOptions) =>
    invoke<string>("decrypt_mapping", { options }),

  // Unmask
  unmaskFile: (options: {
    masked_file_path: string;
    mapping_file_path: string;
    passphrase: string;
    output_path: string;
  }) =>
    invoke<{ output_path: string; restored_count: number }>("unmask_file", { options }),

  // Sandbox
  hasPin: () =>
    invoke<boolean>("has_pin"),

  verifyPin: (pin: string) =>
    invoke<boolean>("verify_pin", { pin }),

  setPin: (pin: string) =>
    invoke<void>("set_pin", { pin }),

  clearPin: () =>
    invoke<void>("clear_pin"),

  lockSandboxFiles: (directory: string) =>
    invoke<string>("lock_sandbox_files", { directory }),

  unlockSandboxFiles: (directory: string) =>
    invoke<string>("unlock_sandbox_files", { directory }),

  listSandboxFiles: () =>
    invoke<SandboxFile[]>("list_sandbox_files"),

  getSandboxDirPath: () =>
    invoke<string>("get_sandbox_dir_path"),

  listFilesInDirectory: (directory: string) =>
    invoke<SandboxFile[]>("list_files_in_directory", { directory }),

  exportSandbox: (fileName: string, destPath: string, passphrase: string) =>
    invoke<void>("export_sandbox", { fileName, destPath, passphrase }),

  importSandbox: (srcPath: string, passphrase: string) =>
    invoke<SandboxFile>("import_sandbox", { srcPath, passphrase }),

  // Rules
  getRules: () =>
    invoke<MaskRule[]>("get_rules"),

  saveRules: (rules: MaskRule[]) =>
    invoke<void>("save_rules", { rules }),

  // Batch
  startBatchJob: (options: BatchJobOptions) =>
    invoke<string>("start_batch_job", { options }),

  getBatchStatus: (jobId: string) =>
    invoke<BatchStatus>("get_batch_status", { jobId }),

  cancelBatchJob: (jobId: string) =>
    invoke<void>("cancel_batch_job", { jobId }),

  // Database - Logs
  initializeDatabase: () =>
    invoke<void>("initialize_database"),

  addLogEntry: (level: string, message: string, details?: string, filePath?: string, operationType?: string) =>
    invoke<void>("add_log_entry", { 
      request: { level, message, details, file_path: filePath, operation_type: operationType }
    }),

  getLogs: (limit?: number, offset?: number, levelFilter?: string) =>
    invoke<LogEntry[]>("get_logs", { 
      params: { limit, offset, level_filter: levelFilter }
    }),

  getLogsCount: (levelFilter?: string) =>
    invoke<number>("get_logs_count", { level_filter: levelFilter }),

  clearAllLogs: () =>
    invoke<void>("clear_all_logs"),

  cleanupOldLogs: (days: number) =>
    invoke<number>("cleanup_old_logs", { days }),

  // Database - User Settings
  saveUserSetting: (key: string, value: string) =>
    invoke<void>("save_user_setting", { key, value }),

  getUserSetting: (key: string) =>
    invoke<string | null>("get_user_setting", { key }),

  getAllUserSettings: () =>
    invoke<UserSetting[]>("get_all_user_settings"),

  deleteUserSetting: (key: string) =>
    invoke<void>("delete_user_setting", { key }),

  // Database - Processing History
  addProcessingHistory: (
    filePath: string,
    outputPath: string,
    ruleIds: string[],
    fileSize: number,
    maskedCount: number,
    processingTimeMs: number,
    status: string,
    errorMessage?: string
  ) =>
    invoke<void>("add_processing_history", {
      request: {
        file_path: filePath,
        output_path: outputPath,
        rule_ids: ruleIds,
        file_size: fileSize,
        masked_count: maskedCount,
        processing_time_ms: processingTimeMs,
        status,
        error_message: errorMessage,
      }
    }),

  getProcessingHistory: (limit?: number, offset?: number) =>
    invoke<ProcessingHistory[]>("get_processing_history", { limit, offset }),

  getStatistics: () =>
    invoke<DatabaseStatistics>("get_statistics"),

  getDatabaseInfo: () =>
    invoke<any>("get_database_info"),

  // Database Migration
  migrateOldDatabase: () =>
    invoke<string>("migrate_old_database"),

  // Proxy
  fetchWebpage: (url: string) =>
    invoke<{content: string, status: number, contentType: string}>("fetch_webpage", { url }),

  // WebView
  openWebviewWindow: (options: { url: string; title?: string; width?: number; height?: number }) =>
    invoke<string>("open_webview_window", { options }),

  openDesktopWindowWithButton: (url: string) =>
    invoke<void>("open_desktop_window_with_button", { url }),

  ensureDesktopChildWebview: (sidebarCollapsed = false) =>
    invoke<void>("ensure_desktop_child_webview", { sidebarCollapsed }),

  updateDesktopChildWebviewBounds: (sidebarCollapsed = false) =>
    invoke<void>("update_desktop_child_webview_bounds", { sidebarCollapsed }),

  hideDesktopChildWebview: () =>
    invoke<void>("hide_desktop_child_webview"),

  navigateWebview: (label: string, url: string) =>
    invoke<void>("navigate_webview", { label, url }),

  webviewReload: (label: string) =>
    invoke<void>("webview_reload", { label }),

  closeWebviewWindow: (label: string) =>
    invoke<void>("close_webview_window", { label }),

  getWebviewUrl: (label: string) =>
    invoke<string>("get_webview_url", { label }),

  webviewEvalScript: (label: string, script: string) =>
    invoke<void>("webview_eval_script", { label, script }),

  navigateMainWindowWithButton: (url: string, returnUrl: string) =>
    invoke<void>("navigate_main_window_with_button", { url, returnUrl }),

  // OCR
  getPlatformContext: () =>
    invoke<PlatformContext>("get_platform_context"),

  prepareUpdateBackup: () =>
    invoke<UpdateBackupSummary>("prepare_update_backup"),

  restartApp: () =>
    invoke<void>("restart_app"),

  checkOcrInstalled: () =>
    invoke<boolean>("check_ocr_installed"),

  getOcrInstallPath: () =>
    invoke<string>("get_ocr_install_path"),

  downloadOcrPackage: (customPath?: string) =>
    invoke<string>("download_ocr_package", { customPath }),

  uninstallOcrPackage: () =>
    invoke<void>("uninstall_ocr_package"),

  // FileBay Config
  readFilebayConfig: () =>
    invoke<FileBayConfigStatus>("read_filebay_config"),

  checkFilebayConfigExists: () =>
    invoke<boolean>("check_filebay_config_exists"),

  deleteFilebayConfig: () =>
    invoke<string>("delete_filebay_config"),

  validateFilebayConfigFile: (filePath: string) =>
    invoke<FileBayConfig>("validate_filebay_config_file", { filePath }),

  importFilebayConfig: (sourcePath: string) =>
    invoke<string>("import_filebay_config", { sourcePath }),

  // AI Model
  downloadOllama: (customPath?: string) =>
    invoke<string>("download_ollama", { customPath }),

  checkOllamaInstalled: () =>
    invoke<boolean>("check_ollama_installed"),

  checkOllamaBinaryInstalled: () =>
    invoke<boolean>("check_ollama_binary_installed"),

  checkOllamaServiceRunning: () =>
    invoke<boolean>("check_ollama_service_running"),

  startOllamaService: () =>
    invoke<string>("start_ollama_service"),

  checkAiModelInstalled: () =>
    invoke<boolean>("check_ai_model_installed"),

  installAiModel: () =>
    invoke<string>("install_ai_model"),

  uninstallAiModel: () =>
    invoke<string>("uninstall_ai_model"),

  callAiModel: (prompt: string) =>
    invoke<string>("call_ai_model", { prompt }),

  getAiModelInfo: () =>
    invoke<{
      model_name: string;
      model_size: string;
      model_dir: string;
      ollama_installed: boolean;
      model_installed: boolean;
      service_running?: boolean;
    }>("get_ai_model_info"),

  checkAiDetectionAvailable: () =>
    invoke<boolean>("check_ai_detection_available"),

  // Sensitive Terms
  addSensitiveTerm: (request: AddSensitiveTermRequest) =>
    invoke<SensitiveTerm>("add_sensitive_term", { request }),

  addSensitiveTermsBatch: (requests: AddSensitiveTermRequest[]) =>
    invoke<SensitiveTerm[]>("add_sensitive_terms_batch", { requests }),

  updateSensitiveTerm: (request: UpdateSensitiveTermRequest) =>
    invoke<SensitiveTerm>("update_sensitive_term", { request }),

  deleteSensitiveTerm: (id: string) =>
    invoke<void>("delete_sensitive_term", { id }),

  deleteSensitiveTermsBatch: (ids: string[]) =>
    invoke<void>("delete_sensitive_terms_batch", { ids }),

  getSensitiveTerms: (category?: string, enabledOnly?: boolean) =>
    invoke<SensitiveTerm[]>("get_sensitive_terms", { category, enabledOnly }),

  getSensitiveTermCategories: () =>
    invoke<string[]>("get_sensitive_term_categories"),

  searchSensitiveTerms: (query: string) =>
    invoke<SensitiveTerm[]>("search_sensitive_terms", { query }),

  getSensitiveTermsStats: () =>
    invoke<SensitiveTermsStats>("get_sensitive_terms_stats"),

  exportSensitiveTermsCsv: (outputPath: string) =>
    invoke<string>("export_sensitive_terms_csv", { outputPath }),

  importSensitiveTermsCsv: (filePath: string) =>
    invoke<number>("import_sensitive_terms_csv", { filePath }),

  // Installer (using Python scripts)
  checkPythonAvailable: () =>
    invoke<boolean>("check_python_available"),

  installOcrWithScript: () =>
    invoke<string>("install_ocr_with_script"),

  uninstallOcrWithScript: () =>
    invoke<string>("uninstall_ocr_with_script"),

  installOllamaWithScript: () =>
    invoke<string>("install_ollama_with_script"),

  uninstallOllamaWithScript: () =>
    invoke<string>("uninstall_ollama_with_script"),
  
  // Config Extraction
  extractConfigFromDesktopWebview: () =>
    invoke<string>("extract_config_from_desktop_webview"),
  
  evalJsInDesktopWebview: (jsCode: string) =>
    invoke<string>("eval_js_in_desktop_webview", { jsCode }),
  
  // FileBay Config Sync

  // 按当前文件的真实本机路径向后端确认上传候选身份，而不是按文件名字符串匹配；
  // 返回值只包含发起查询的路径自身映射到的正式历史 ID，不返回其他历史的路径信息。
  confirmFilebayUploadCandidates: (filePaths: string[]) =>
    invoke<Record<string, string>>("confirm_filebay_upload_candidates", { filePaths }),

  // Excel Masking
  excelParseStructure: (filePath: string) =>
    invoke<ExcelStructureResponseEnvelope>("excel_parse_structure", { filePath }).then(
      extractSheetsFromExcelStructureResponse
    ),

  excelPreviewMasking: (
    config: ExcelMaskingConfig,
    maxRows?: number,
    sandboxPassphrase?: string
  ) =>
    invoke<TauriExcelMaskPreview>("excel_preview_masking", {
      config: toTauriExcelMaskingConfig(config, { sandboxPassphrase }),
      maxRows,
    }).then(toCanonicalExcelMaskPreview),

  excelApplyMasking: (
    config: ExcelMaskingConfig,
    outputDir: string,
    sandboxPassphrase?: string
  ) =>
    invoke<ExcelApplyResult>("excel_apply_masking", {
      config: toTauriExcelMaskingConfig(config, { sandboxPassphrase }),
      outputDir,
    }),

  excelRestoreFromEcmap: (req: ExcelRestoreReq) =>
    invoke<ExcelRestoreResult>(
      EXCEL_RESTORE_FROM_ECMAP_COMMAND,
      buildExcelRestoreInvokeArgs(req)
    ),

  excelSaveTemplate: (name: string, config: ExcelMaskingConfig) =>
    invoke<string>("excel_save_template", { name, config }),

  excelLoadTemplate: (path: string) =>
    invoke<ExcelMaskingConfig>("excel_load_template", { path }),
};
