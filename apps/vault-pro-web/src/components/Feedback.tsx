import { AlertCircle, Inbox } from "lucide-react";

export function LoadingState({ label = "正在读取真实 Runtime 状态…" }: { label?: string }) {
  return (
    <div className="state-panel" role="status">
      <span className="spinner" aria-hidden="true" />
      <p>{label}</p>
    </div>
  );
}

export function ErrorState({ message, onRetry }: { message: string; onRetry?: () => void }) {
  return (
    <div className="state-panel state-error" role="alert">
      <AlertCircle className="state-icon" aria-hidden="true" />
      <div>
        <strong>暂时无法完成请求</strong>
        <p>{message}</p>
      </div>
      {onRetry && (
        <button className="button secondary" type="button" onClick={onRetry}>
          重新连接
        </button>
      )}
    </div>
  );
}

export function EmptyState({ title, detail }: { title: string; detail: string }) {
  return (
    <div className="state-panel empty-state">
      <Inbox className="empty-mark" aria-hidden="true" />
      <strong>{title}</strong>
      <p>{detail}</p>
    </div>
  );
}
