// Custom rule (frontend-defined, passed to backend)
export interface CustomRule {
  id: string;
  name: string;
  pattern: string;
  replacement_template: string;
  use_counter?: boolean;
}

// Masking commands
export interface MaskFileOptions {
  file_path: string;
  output_path: string;
  rule_ids: string[];
  passphrase?: string;
  custom_rules?: CustomRule[];
}

export interface MaskResult {
  output_path: string;
  masked_count: number;
  mapping_path?: string;
}

export interface PreviewOptions {
  file_path: string;
  rule_ids: string[];
  max_rows?: number;
  custom_rules?: CustomRule[];
  use_ai_validation?: boolean;
}

export interface EntityMatch {
  text: string;
  entity_type: string;
  start: number;
  end: number;
}

export interface RowEntities {
  row_index: number;
  entities: EntityMatch[];
}

export interface PreviewResult {
  original_rows: string[][];
  masked_rows: string[][];
  headers: string[];
  detected_entities?: RowEntities[];
  mapping?: MappingEntry[];
}

export interface MappingEntry {
  original: string;
  masked: string;
}

export interface SavePreviewOptions {
  file_path: string;
  output_dir: string;
  masked_rows: string[][];
  headers?: string[];
  passphrase?: string;
  mapping?: MappingEntry[];
}

// Crypto commands
export interface EncryptOptions {
  mapping_json: string;
  passphrase: string;
  output_path: string;
}

export interface DecryptOptions {
  cmap_path: string;
  passphrase: string;
}

// Sandbox commands
export interface SandboxFile {
  name: string;
  path: string;
  size: number;
  modified: string;
}

export interface ExportSandboxOptions {
  file_name: string;
  dest_path: string;
  passphrase: string;
}

export interface ImportSandboxOptions {
  src_path: string;
  passphrase: string;
}

// Rules commands
export interface MaskRule {
  id: string;
  name: string;
  pattern: string;
  replacement: string;
  enabled: boolean;
  builtin: boolean;
}

// Batch commands
export interface BatchJobOptions {
  file_paths: string[];
  output_dir: string;
  rule_ids: string[];
  passphrase?: string;
  custom_rules?: CustomRule[];
  use_ai_validation?: boolean;
}

export interface BatchStatus {
  job_id: string;
  total: number;
  completed: number;
  failed: number;
  status: "Pending" | "Running" | "Completed" | "Failed" | "Cancelled";
  current_file?: string;
  error?: string;
}

// OCR commands
export interface OcrDownloadProgress {
  downloaded: number;
  total: number;
  percentage: number;
  status: string;
}

export interface PlatformContext {
  os: "windows" | "macos" | "linux" | "unknown";
  pathSeparator: string;
  defaultDocumentsDir: string;
  appDataDir: string;
  cacheDir: string;
  tempDir: string;
  pinStorageMode: "windows_dpapi" | "macos_keychain" | "fallback_file" | string;
  ocrStrategy: "embedded_python" | "system_python_venv" | "system_python" | string;
  ollamaStrategy: "binary_check_plus_background_serve" | "binary_check_plus_app_launch" | "binary_check_plus_cli_serve" | string;
}
// FileBay Config types
export interface FileBayConfig {
  url: string;
  username: string;
  repoName: string;
  email: string;
  token: string;
  downloadedAt: string;
  version: string;
}

export interface FileBayConfigStatus {
  exists: boolean;
  config?: FileBayConfig;
  filePath?: string;
  lastModified?: string;
}

// Sensitive Terms types
export interface SensitiveTerm {
  id: string;
  term: string;
  category: string;
  description?: string;
  enabled: boolean;
  created_at: string;
  updated_at: string;
}

export interface AddSensitiveTermRequest {
  term: string;
  category: string;
  description?: string;
}

export interface UpdateSensitiveTermRequest {
  id: string;
  term?: string;
  category?: string;
  description?: string;
  enabled?: boolean;
}

export interface SensitiveTermsStats {
  total: number;
  enabled: number;
  disabled: number;
  categories: number;
}

// Installer commands
export interface InstallerProgress {
  percentage: number;
  status: string;
  log: string;
}

// Sync Config types
export interface SyncConfigRequest {
  url: string;
  username: string;
  repo_name: string;
  email: string;
  token: string;
  user_id?: string;
}
