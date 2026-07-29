import { useCallback, useEffect, useState } from "react";
import { AlertTriangle, ChevronRight, RefreshCw } from "lucide-react";
import { Link, useParams } from "react-router-dom";
import { api, RuntimeApiError } from "../api/client";
import { EmptyState, ErrorState, LoadingState } from "../components/Feedback";
import type { BatchDetail } from "../types";

export function BatchErrorsPage() {
  const { batchId = "" } = useParams();
  const [detail, setDetail] = useState<BatchDetail | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [retrying, setRetrying] = useState<Set<string>>(new Set());

  const load = useCallback(async (clearError = true) => {
    if (clearError) setError(null);
    try { setDetail(await api.batch(batchId)); }
    catch (reason) { setError(reason instanceof Error ? reason.message : "错误列表加载失败。"); }
    finally { setLoading(false); }
  }, [batchId]);

  useEffect(() => { void load(); }, [load]);
  const failed = detail?.files.filter((file) => file.status === "Failed") ?? [];

  const retry = async (fileId: string) => {
    if (retrying.has(fileId)) return;
    setRetrying((current) => new Set(current).add(fileId)); setError(null);
    try { await api.retry(fileId); await load(); }
    catch (reason) {
      const message = reason instanceof RuntimeApiError ? `${reason.code}：${reason.message}` : reason instanceof Error ? reason.message : "重试失败。";
      setError(message); await load(false);
    } finally {
      setRetrying((current) => { const next = new Set(current); next.delete(fileId); return next; });
    }
  };

  if (loading) return <section className="page"><LoadingState /></section>;
  if (!detail && error) return <section className="page"><ErrorState message={error} onRetry={() => void load()} /></section>;

  return (
    <section className="page">
      <div className="breadcrumb"><Link to={`/batches/${batchId}`}>批次详情</Link><ChevronRight aria-hidden="true" /><span>错误与重试</span></div>
      <div className="page-heading compact"><div><h1>错误与重试</h1><p>这里只显示 Runtime 标记为 Failed 的文件及安全错误。</p></div></div>
      {error && <div className="inline-alert" role="alert">{error}</div>}
      {failed.length === 0 ? <EmptyState title="当前没有失败文件" detail="批次可能已恢复处理；返回详情查看最新真实状态。" /> : (
        <div className="error-list">{failed.map((file) => (
          <article className="error-card" key={file.file_id}>
            <div className="error-file"><span className="error-symbol"><AlertTriangle aria-hidden="true" /></span><div><h2>{file.display_name}</h2><span className="mono">{file.file_id}</span></div></div>
            <div className="error-detail"><span>{file.error_code ?? "PROCESSING_FAILED"}</span><p>{file.error_message ?? "文件处理失败，可重新提交处理。"}</p><small>第 {file.attempt} 次尝试</small></div>
            <button className="button danger" disabled={retrying.has(file.file_id)} onClick={() => void retry(file.file_id)}>{retrying.has(file.file_id) ? "正在请求…" : <><RefreshCw aria-hidden="true" />重新处理</>}</button>
          </article>
        ))}</div>
      )}
    </section>
  );
}
