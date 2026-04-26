export type FileStatus =
  | "pending"
  | "processing"
  | "completed"
  | "failed";

export interface QueuedFile {
  id: string;
  name: string;
  path: string;
  size: number;
  status: FileStatus;
  outputPath?: string;
  mappingPath?: string;
  maskedCount?: number;
  error?: string;
  addedAt: number;
}
