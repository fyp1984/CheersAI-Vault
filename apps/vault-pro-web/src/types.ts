export type FileStatus = "Pending" | "Processing" | "Completed" | "Failed";
export type BatchStatus = "Running" | "Completed" | "CompletedWithErrors" | "Failed";

export interface RuleMetadata {
  id: string;
  name: string;
  enabled_by_default: boolean;
}

export interface RulesResponse {
  rules: RuleMetadata[];
}

export interface CreatedFile {
  file_id: string;
  display_name: string;
}

export interface CreateBatchResponse {
  batch_id: string;
  files: CreatedFile[];
}

export interface BatchSummary {
  batch_id: string;
  status: BatchStatus;
  file_count: number;
  completed_count: number;
  failed_count: number;
  masked_entity_count: number;
  created_at: string;
  updated_at: string;
}

export interface BatchListResponse {
  batches: BatchSummary[];
}

export interface BatchFile {
  file_id: string;
  display_name: string;
  input_format: string;
  status: FileStatus;
  attempt: number;
  masked_entity_count: number | null;
  artifact_id: string | null;
  error_code: string | null;
  error_message: string | null;
  restore_available: boolean;
}

export interface BatchDetail {
  batch: BatchSummary;
  files: BatchFile[];
}

export interface RetryResponse {
  file_id: string;
  status: FileStatus;
  attempt: number;
}

export interface ErrorResponse {
  code: string;
  message: string;
  retryable: boolean;
}

export interface OcrStatusResponse {
  status: string;
  model_ready: boolean;
  timeout_secs: number;
  max_pages: number;
}
