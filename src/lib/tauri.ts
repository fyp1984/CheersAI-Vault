import { invoke } from "@tauri-apps/api/core";
import type {
  MaskFileOptions,
  MaskResult,
  PreviewOptions,
  PreviewResult,
  EncryptOptions,
  DecryptOptions,
  SandboxFile,
  MaskRule,
  BatchJobOptions,
  BatchStatus,
  FileBayConfig,
  FileBayConfigStatus,
  SensitiveTerm,
  AddSensitiveTermRequest,
  UpdateSensitiveTermRequest,
  SensitiveTermsStats,
} from "@/types/commands";
import type { LogEntry, ProcessingHistory, UserSetting, DatabaseStatistics } from "@/types/log";

export const tauriCommands = {
  // Masking
  maskFile: (options: MaskFileOptions) =>
    invoke<MaskResult>("mask_file", { options }),

  previewMasking: (options: PreviewOptions) =>
    invoke<PreviewResult>("preview_masking", { options }),

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

  ensureDesktopChildWebview: () =>
    invoke<void>("ensure_desktop_child_webview"),

  updateDesktopChildWebviewBounds: () =>
    invoke<void>("update_desktop_child_webview_bounds"),

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
};
