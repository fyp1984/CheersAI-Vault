import { useCallback, useEffect, useState } from "react";
import { ChevronRight, Download } from "lucide-react";
import { Link, useParams } from "react-router-dom";
import { api, downloadArtifact } from "../api/client";
import { ErrorState, LoadingState } from "../components/Feedback";
import { StatusBadge } from "../components/StatusBadge";
import { formatLabel } from "../formatCatalog";
import type { BatchDetail } from "../types";

export function BatchDetailPage() {
  const { batchId = "" } = useParams();
  const [detail, setDetail] = useState<BatchDetail | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [downloadError, setDownloadError] = useState<string | null>(null);
  const [downloading, setDownloading] = useState<string | null>(null);

  const load = useCallback(async (quiet = false) => {
    if (!quiet) setLoading(true);
    setError(null);
    try { setDetail(await api.batch(batchId)); }
    catch (reason) { setError(reason instanceof Error ? reason.message : "批次详情加载失败。"); }
    finally { setLoading(false); }
  }, [batchId]);

  useEffect(() => { void load(); }, [load]);
  useEffect(() => {
    if (detail?.batch.status !== "Running") return;
    const timer = window.setTimeout(() => void load(true), 1200);
    return () => window.clearTimeout(timer);
  }, [detail, load]);

  const download = async (artifactId: string, displayName: string) => {
    setDownloading(artifactId); setDownloadError(null);
    try { await downloadArtifact(artifactId, displayName); }
    catch (reason) { setDownloadError(reason instanceof Error ? reason.message : "下载失败。"); }
    finally { setDownloading(null); }
  };

  if (loading) return <section className="page"><LoadingState /></section>;
  if (error || !detail) return <section className="page"><ErrorState message={error ?? "批次不存在。"} onRetry={() => void load()} /></section>;

  return (
    <section className="page">
      <div className="breadcrumb"><Link to="/batches">作业列表</Link><ChevronRight aria-hidden="true" /><span className="mono">{batchId.slice(0, 8)}</span></div>
      <div className="page-heading compact detail-heading">
        <div><h1>批次详情</h1><p className="mono">{batchId}</p></div>
        <div className="heading-actions"><StatusBadge status={detail.batch.status} />{detail.batch.failed_count > 0 && <Link className="button danger" to={`/batches/${batchId}/errors`}>查看错误与重试</Link>}</div>
      </div>
      <div className="metric-grid">
        <div className="metric"><span>总文件</span><strong>{detail.batch.file_count}</strong></div>
        <div className="metric success"><span>已完成</span><strong>{detail.batch.completed_count}</strong></div>
        <div className="metric danger"><span>失败</span><strong>{detail.batch.failed_count}</strong></div>
        <div className="metric"><span>真实脱敏实体</span><strong>{detail.files.reduce((sum, file) => sum + (file.masked_entity_count ?? 0), 0)}</strong></div>
      </div>
      {downloadError && <div className="inline-alert" role="alert">{downloadError}</div>}
      <div className="panel table-panel">
        <table>
          <thead><tr><th>文件</th><th>格式</th><th>状态</th><th>尝试</th><th>脱敏实体</th><th>产物</th></tr></thead>
          <tbody>{detail.files.map((file) => (
            <tr key={file.file_id}>
              <td><strong>{file.display_name}</strong><small className="file-id mono">{file.file_id.slice(0, 8)}</small></td>
              <td>{formatLabel(file.input_format)}</td>
              <td><StatusBadge status={file.status} /></td><td>{file.attempt}</td>
              <td>{file.masked_entity_count ?? "未生成"}</td>
              <td>{file.artifact_id ? <button className="text-link button-link" disabled={downloading === file.artifact_id} onClick={() => void download(file.artifact_id!, file.display_name)}>{downloading === file.artifact_id ? "下载中…" : <><Download aria-hidden="true" />下载 Markdown</>}</button> : <span className="muted">尚不可用</span>}</td>
            </tr>
          ))}</tbody>
        </table>
      </div>
    </section>
  );
}
