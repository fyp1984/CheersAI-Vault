export type LogLevel = "info" | "warning" | "error" | "success";

export interface LogEntry {
  id: string;
  timestamp: number | string; // 支持时间戳或 ISO 字符串
  level: LogLevel;
  message: string;
  details?: string;
  filePath?: string;
  operationType?: string;
  userId?: string;
}

export interface ProcessingHistory {
  id: string;
  filePath: string;
  outputPath: string;
  ruleIds: string[];
  fileSize: number;
  maskedCount: number;
  processingTimeMs: number;
  status: "success" | "failed" | "cancelled";
  errorMessage?: string;
  createdAt: string;
}

export interface UserSetting {
  key: string;
  value: string;
  updatedAt: string;
}

export interface DatabaseStatistics {
  totalFiles: number;
  successfulFiles: number;
  failedFiles: number;
  totalMaskedItems: number;
  averageProcessingTimeMs: number; // 改为平均处理时间
  recentFiles7days: number;
  successRate: number;
}
