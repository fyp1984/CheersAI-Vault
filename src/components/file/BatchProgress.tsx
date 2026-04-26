import { Progress } from "@/components/ui/progress";
import { CheckCircle2, AlertCircle, Loader2 } from "lucide-react";

interface BatchProgressProps {
  total: number;
  completed: number;
  failed: number;
  currentFile?: string;
  status: string;
}

export function BatchProgress({ total, completed, failed, currentFile, status }: BatchProgressProps) {
  const progress = total > 0 ? ((completed + failed) / total) * 100 : 0;
  const isRunning = status === "Running";
  const isCompleted = status === "Completed";
  const isFailed = status === "Failed";

  return (
    <div className="p-4 bg-blue-50 border border-blue-200 rounded-lg space-y-3">
      <div className="flex items-center gap-2">
        {isRunning && <Loader2 className="w-4 h-4 text-blue-500 animate-spin" />}
        {isCompleted && <CheckCircle2 className="w-4 h-4 text-green-500" />}
        {isFailed && <AlertCircle className="w-4 h-4 text-red-500" />}
        <span className="text-sm font-medium text-gray-700">
          批处理进度: {completed + failed} / {total}
        </span>
      </div>
      
      <Progress value={progress} className="w-full" />
      
      <div className="text-xs text-gray-600 space-y-1">
        <div>已完成: {completed} | 失败: {failed}</div>
        {currentFile && (
          <div className="truncate">当前处理: {currentFile}</div>
        )}
      </div>
    </div>
  );
}