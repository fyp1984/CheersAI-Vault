import type { BatchStatus, FileStatus } from "../types";

const labels: Record<BatchStatus | FileStatus, string> = {
  Running: "处理中",
  Completed: "已完成",
  CompletedWithErrors: "部分失败",
  Failed: "失败",
  Pending: "等待中",
  Processing: "处理中",
};

export function StatusBadge({ status }: { status: BatchStatus | FileStatus }) {
  const label = labels[status];
  if (!label) return <span className="badge badge-unknown">状态不兼容</span>;
  return <span className={`badge badge-${status.toLowerCase()}`}>{label}</span>;
}
