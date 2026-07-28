import { useEffect, useState } from "react";
import { api, RuntimeApiError } from "../api/client";
import { ErrorState, LoadingState } from "../components/Feedback";
import type { BatchDetail, BatchFile, BatchSummary } from "../types";

export function RestorePage() {
  const [files, setFiles] = useState<Array<{ batchId: string; file: BatchFile }>>([]);
  const [loading, setLoading] = useState(true);
  const [selected, setSelected] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);
  const [result, setResult] = useState<{ count: number } | null>(null);
  const [error, setError] = useState<string | null>(null);

  const load = async () => {
    setLoading(true); setError(null);
    try {
      const list = await api.batches();
      const all: Array<{ batchId: string; file: BatchFile }> = [];
      for (const b of list.batches) {
        try {
          const detail = await api.batch(b.batch_id);
          for (const f of detail.files) {
            if (f.status === "Completed" && f.restore_available && f.artifact_id) {
              all.push({ batchId: b.batch_id, file: f });
            }
          }
        } catch { /* skip batches that fail */ }
      }
      setFiles(all);
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : "加载失败。");
    } finally { setLoading(false); }
  };

  useEffect(() => { void load(); }, []);

  const handleRestore = async () => {
    if (!selected) return;
    setSubmitting(true); setError(null); setResult(null);
    try {
      const res = await api.restoreArtifact(selected);
      setResult(res);
    } catch (reason) {
      if (reason instanceof RuntimeApiError) {
        const msg = reason.code === "CMAP_MISMATCH" ? "无法恢复：映射数据无效。" :
                    reason.code === "NOT_FOUND" ? "该产物已不存在或不可恢复。" :
                    reason.code === "RUNTIME_UNAVAILABLE" ? "无法连接本机 Runtime，请确认服务已启动。" :
                    `反脱敏失败（${reason.code}）。`;
        setError(msg);
      } else {
        setError("反脱敏失败。");
      }
    } finally { setSubmitting(false); }
  };

  if (loading) return <section className="page"><LoadingState /></section>;

  return (
    <section className="page">
      <div className="page-heading">
        <div>
          <h1>反脱敏（映射恢复）</h1>
          <p>选择已完成的服务器产物，恢复为原文。（本机 MVP，非发布版本。）</p>
        </div>
      </div>

      {error && <div className="inline-alert" role="alert"><ErrorState message={error} onRetry={() => setError(null)} /></div>}
      {result && <div className="inline-success" role="status"><p>已恢复 <strong>{result.count}</strong> 处。</p></div>}

      {files.length === 0 ? (
        <div className="panel"><p className="muted" style={{ padding: "2rem", textAlign: "center" }}>没有可恢复的产物。先提交一个脱敏批次并等待完成。</p></div>
      ) : (
        <div className="panel table-panel">
          <table>
            <thead><tr><th /><th>文件</th><th>批次</th><th>脱敏实体</th></tr></thead>
            <tbody>{files.map(({ batchId, file }) => (
              <tr key={file.file_id}>
                <td><input type="radio" name="restore-file" checked={selected === file.artifact_id} onChange={() => setSelected(file.artifact_id ?? null)} /></td>
                <td><strong>{file.display_name}</strong></td>
                <td className="mono">{batchId.slice(0, 8)}</td>
                <td>{file.masked_entity_count ?? "—"}</td>
              </tr>
            ))}</tbody>
          </table>
          <div className="submit-footer">
            <button className="button primary wide" disabled={!selected || submitting} onClick={handleRestore}>
              {submitting ? "正在恢复…" : "开始反脱敏"}
            </button>
          </div>
        </div>
      )}
    </section>
  );
}
