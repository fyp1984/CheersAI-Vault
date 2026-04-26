import { FileText, X, CheckCircle2, AlertCircle, Loader2 } from "lucide-react";
import { cn, formatBytes } from "@/lib/utils";
import type { QueuedFile } from "@/types/file";

interface FileQueueItemProps {
  file: QueuedFile;
  onRemove: (id: string) => void;
}

const statusIcon = {
  pending: null,
  processing: <Loader2 className="w-4 h-4 text-indigo-500 animate-spin" />,
  completed: <CheckCircle2 className="w-4 h-4 text-green-500" />,
  failed: <AlertCircle className="w-4 h-4 text-red-500" />,
};

export function FileQueueItem({ file, onRemove }: FileQueueItemProps) {
  return (
    <div
      className={cn(
        "flex items-center gap-3 px-4 py-3 rounded-lg border bg-white",
        file.status === "failed" && "border-red-200 bg-red-50/50",
        file.status === "completed" && "border-green-200 bg-green-50/50",
        file.status === "processing" && "border-indigo-200 bg-indigo-50/50",
        file.status === "pending" && "border-gray-200"
      )}
    >
      <FileText className="w-5 h-5 text-gray-400 shrink-0" />
      <div className="flex-1 min-w-0">
        <p className="text-sm font-medium text-gray-800 truncate">{file.name}</p>
        <p className="text-xs text-gray-400">
          {formatBytes(file.size)}
          {file.maskedCount != null && (
            <span className="ml-2 text-green-600">脱敏 {file.maskedCount} 处</span>
          )}
          {file.error && (
            <span className="ml-2 text-red-500">{file.error}</span>
          )}
        </p>
      </div>
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
  );
}
