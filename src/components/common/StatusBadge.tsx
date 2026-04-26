import { Badge } from "@/components/ui/badge";
import { cn } from "@/lib/utils";
import type { FileStatus } from "@/types/file";

interface StatusBadgeProps {
  status: FileStatus;
}

const statusConfig: Record<FileStatus, { label: string; className: string }> = {
  pending: { label: "等待中", className: "bg-gray-100 text-gray-600" },
  processing: { label: "处理中", className: "bg-indigo-100 text-indigo-600" },
  completed: { label: "已完成", className: "bg-green-100 text-green-600" },
  failed: { label: "失败", className: "bg-red-100 text-red-600" },
};

export function StatusBadge({ status }: StatusBadgeProps) {
  const config = statusConfig[status];
  return (
    <Badge className={cn("font-normal text-xs", config.className)}>
      {config.label}
    </Badge>
  );
}
