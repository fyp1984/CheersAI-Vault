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
