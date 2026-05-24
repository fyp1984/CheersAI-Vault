import { FileText, X, CheckCircle2, AlertCircle, Loader2 } from "lucide-react";
import { cn, formatBytes } from "@/lib/utils";
import type { QueuedFile } from "@/types/file";
import { PageRangeInput } from "./PageRangeInput";

interface FileQueueItemProps {
  file: QueuedFile;
  onRemove: (id: string) => void;
  onPageRangeChange?: (fileId: string, range?: [number, number]) => void;
}

const statusIcon = {
  pending: null,
  processing: <Loader2 className="w-4 h-4 text-indigo-500 animate-spin" />,
  completed: <CheckCircle2 className="w-4 h-4 text-blue-500" />,
  failed: <AlertCircle className="w-4 h-4 text-red-500" />,
};

export function FileQueueItem({ file, onRemove, onPageRangeChange }: FileQueueItemProps) {
  const handlePageRangeChange = (range?: [number, number]) => {
    if (onPageRangeChange) {
      onPageRangeChange(file.id, range);
    }
  };

  return (
    <div
      className={cn(
        "flex px-4 py-3 rounded-lg border bg-white transition-colors",
        file.status === "failed" && "border-red-200 bg-red-50/50",
        file.status === "completed" && "border-blue-200 bg-blue-50/50",
        file.status === "processing" && "border-indigo-200 bg-indigo-50/50",
        file.status === "pending" && "border-blue-100 hover:border-blue-200 hover:bg-blue-50/30"
      )}
    >
      <div className="flex w-full flex-wrap items-center gap-3">
        <FileText className="w-5 h-5 text-gray-400 shrink-0" />
        <div className="flex-1 min-w-0">
          <p className="text-sm font-medium text-gray-800 truncate">{file.name}</p>
          <p className="text-xs text-gray-400">
            {formatBytes(file.size)}
            {file.maskedCount != null && (
              <span className="ml-2 text-blue-600">脱敏 {file.maskedCount} 处</span>
            )}
            {file.pageRange && (
              <span className="ml-2 text-indigo-600">第 {file.pageRange[0]}-{file.pageRange[1]} 页</span>
            )}
            {file.error && (
              <span className="ml-2 text-red-500">{file.error}</span>
            )}
          </p>
        </div>
        {/* 页码范围输入（仅在 pending 状态显示） */}
        {file.status === "pending" && onPageRangeChange && (
          <PageRangeInput
            fileName={file.name}
            fileFormat={file.path}
            value={file.pageRange}
            onChange={handlePageRangeChange}
            totalPages={file.totalPages}
          />
        )}
        {statusIcon[file.status]}
        {file.status !== "processing" && (
          <button
            onClick={() => onRemove(file.id)}
            className="text-gray-300 hover:text-gray-500 transition-colors"
          >
            <X className="w-4 h-4" />
          </button>
        )}
      </div>
    </div>
  );
}
