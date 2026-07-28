import { ChangeEvent, FormEvent, useEffect, useMemo, useState } from "react";
import { ArrowRight, UploadCloud } from "lucide-react";
import { useNavigate } from "react-router-dom";
import { api, RuntimeApiError } from "../api/client";
import { ErrorState, LoadingState } from "../components/Feedback";
import { accept, isSupported } from "../formatCatalog";
import type { OcrStatusResponse, RuleMetadata } from "../types";

const maxFiles = 100;

function readableSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}

export function SubmitPage() {
  const navigate = useNavigate();
  const [rules, setRules] = useState<RuleMetadata[]>([]);
  const [selectedRules, setSelectedRules] = useState<Set<string>>(new Set());
  const [files, setFiles] = useState<File[]>([]);
  const [loadingRules, setLoadingRules] = useState(true);
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [ocrStatus, setOcrStatus] = useState<OcrStatusResponse | null>(null);

  const loadRules = async () => {
    setLoadingRules(true);
    setError(null);
    try {
      const [rulesResponse, ocrResponse] = await Promise.all([
        api.rules(),
        api.ocrStatus().catch(() => null),
      ]);
      setRules(rulesResponse.rules);
      setOcrStatus(ocrResponse);
      setSelectedRules(
        new Set(rulesResponse.rules.filter((rule) => rule.enabled_by_default).map((rule) => rule.id)),
      );
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : "规则加载失败。");
    } finally {
      setLoadingRules(false);
    }
  };

  useEffect(() => {
    void loadRules();
  }, []);

  const totalBytes = useMemo(() => files.reduce((total, file) => total + file.size, 0), [files]);
  const canSubmit = files.length > 0 && selectedRules.size > 0 && !submitting;
  const ocrReady = ocrStatus?.status === "ready";
  const ocrLabel = ocrReady ? "PDF OCR（可用）" : "PDF OCR（不可用）";

  const chooseFiles = (event: ChangeEvent<HTMLInputElement>) => {
    const chosen = Array.from(event.target.files ?? []);
    setError(null);
    if (chosen.length > maxFiles) {
      setFiles([]);
      setError(`单批最多选择 ${maxFiles} 个文件。`);
      return;
    }
    const unsupported = chosen.find((file) => !isSupported(file));
    if (unsupported) {
      setFiles([]);
      setError("仅支持 TXT、Markdown、CSV、Excel、DOCX、PPT、PPTX 与 PDF。" + (ocrReady ? "" : " 扫描 PDF 需配置 OCR 组件。"));
      return;
    }
    setFiles(chosen);
  };

  const toggleRule = (ruleId: string) => {
    setSelectedRules((current) => {
      const next = new Set(current);
      if (next.has(ruleId)) next.delete(ruleId);
      else next.add(ruleId);
      return next;
    });
  };

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    if (!canSubmit) {
      setError(files.length === 0 ? "请先选择至少一个文件。" : "请至少选择一项脱敏规则。");
      return;
    }
    setSubmitting(true);
    setError(null);
    try {
      const created = await api.createBatch(files, Array.from(selectedRules));
      setFiles([]);
      navigate(`/batches/${created.batch_id}`);
    } catch (reason) {
      if (reason instanceof RuntimeApiError) {
        setError(`${reason.code}：${reason.message}`);
      } else {
        setError(reason instanceof Error ? reason.message : "提交失败。");
      }
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <section className="page">
      <div className="page-heading">
        <div>
          <h1>批量脱敏提交</h1>
          <p>文件仅发送到本机 Runtime。支持 TXT、Markdown、CSV、Excel、DOCX、PPT、PPTX 与 PDF。{ocrReady ? "" : " 扫描 PDF 需配置 OCR 组件。"}</p>
        </div>
      </div>

      {loadingRules ? (
        <LoadingState label="正在从共享核心加载规则…" />
      ) : error && rules.length === 0 ? (
        <ErrorState message={error} onRetry={() => void loadRules()} />
      ) : (
        <form onSubmit={submit} className="submit-grid">
          <div className="panel upload-panel">
            <div className="panel-heading">
              <div><h2>选择文件</h2></div>
              <span className="hint">最多 100 个</span>
            </div>
            <label className="dropzone">
              <input
                aria-label="选择需要脱敏的文件"
                type="file"
                multiple
                accept={accept}
                onChange={chooseFiles}
              />
              <span className="upload-symbol"><UploadCloud aria-hidden="true" /></span>
              <strong>点击选择 TXT / Markdown / CSV / Excel / DOCX / PPT / PPTX / PDF</strong>
              <span>浏览器只在本次提交内保留文件对象</span>
            </label>
            {files.length > 0 && (
              <div className="file-selection" aria-live="polite">
                <div className="selection-summary">
                  <strong>{files.length} 个文件</strong>
                  <span>合计 {readableSize(totalBytes)}</span>
                </div>
                <ul>
                  {files.map((file) => (
                    <li key={`${file.name}-${file.size}-${file.lastModified}`}>
                      <span className="file-kind">{file.name.split(".").pop()?.toUpperCase()}</span>
                      <span className="file-name">{file.name}</span>
                      <span>{readableSize(file.size)}</span>
                    </li>
                  ))}
                </ul>
              </div>
            )}
          </div>

          <div className="panel rules-panel">
            <div className="panel-heading">
              <div><h2>脱敏规则</h2></div>
              <span className="hint">来自 Runtime</span>
            </div>
            <div className="rules-list">
              {rules.map((rule) => (
                <label className="rule-row" key={rule.id}>
                  <input
                    type="checkbox"
                    checked={selectedRules.has(rule.id)}
                    onChange={() => toggleRule(rule.id)}
                  />
                  <span className="checkbox-mark" />
                  <span><strong>{rule.name}</strong><small>{rule.id}</small></span>
                  {rule.enabled_by_default && <em>默认</em>}
                </label>
              ))}
            </div>
            <div className="submit-footer">
              <div>
                <span>已选规则</span>
                <strong>{selectedRules.size}</strong>
              </div>
              <button className="button primary wide" disabled={!canSubmit} type="submit">
                {submitting ? "正在创建真实批次…" : <><span>创建脱敏批次</span><ArrowRight aria-hidden="true" /></>}
              </button>
            </div>
          </div>

          {error && rules.length > 0 && <div className="inline-alert" role="alert">{error}</div>}
        </form>
      )}
    </section>
  );
}
