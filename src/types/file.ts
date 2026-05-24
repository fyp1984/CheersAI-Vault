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
  pageRange?: [number, number]; // 页码范围 [起始页, 结束页]
  totalPages?: number; // 文件总页数
}
