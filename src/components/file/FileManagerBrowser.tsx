import { useCallback, useEffect, useMemo, useRef, useState, type ChangeEvent } from "react";
import { useNavigate, useSearchParams } from "react-router-dom";
import {
  Copy,
  Download,
  RefreshCw,
  Search,
  Unlock,
  UploadCloud,
  WifiOff,
  X,
} from "lucide-react";
import { PageHeader } from "@/components/layout/PageHeader";
import { Button, Message, Badge, Card, Loading, Input } from "@/components/ui/cheersai-ui";
import {
  confirmRuntimeFileBayUploads,
  downloadRuntimeArtifact,
  downloadRuntimeExcelArtifactMember as downloadRuntimeExcelArtifactMemberRequest,
  fetchRuntimeBatch,
  fetchRuntimeBatches,
  fetchRuntimeFileBayCandidates,
  fetchRuntimeFileBayStatus,
} from "@/lib/runtime/client";
import { runtimeFormatLabel } from "@/lib/runtime/formatCatalog";
import type {
  RuntimeBatchDetail,
  RuntimeBatchFile,
  RuntimeBatchStatus,
  RuntimeBatchSummary,
  RuntimeExcelArtifactMemberKind,
  RuntimeFileBayCandidate,
  RuntimeFileBayStatusResponse,
  RuntimeFileBayUploadItem,
} from "@/types/runtime";

const POLL_INTERVAL_MS = 1500;

/** 安全展示用的文件名/ID：只做控制字符清理，不解释为路径。 */
function safeDisplayName(name: string): string {
  // eslint-disable-next-line no-control-regex
  return name.replace(/[\x00-\x1f\x7f]/g, "");
}

const EXCEL_ARTIFACT_ACTIONS: ReadonlyArray<{
  kind: RuntimeExcelArtifactMemberKind;
  label: string;
}> = [
  { kind: "masked_workbook", label: "下载工作簿" },
  { kind: "report", label: "下载报告" },
  { kind: "ecmap", label: "下载 ECMAP" },
  { kind: "encrypted_source", label: "下载加密源" },
];

function formatTime(value: string): string {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return new Intl.DateTimeFormat("zh-CN", {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  }).format(date);
}

function batchStatusVariant(status: string): "success" | "warning" | "error" | "info" {
  switch (status) {
    case "Completed":
      return "success";
    case "CompletedWithErrors":
      return "warning";
    case "Failed":
      return "error";
    default:
      return "info";
  }
}

function batchStatusLabel(status: string): string {
  switch (status) {
    case "Running":
      return "处理中";
    case "Completed":
      return "已完成";
    case "CompletedWithErrors":
      return "部分失败";
    case "Failed":
      return "失败";
    default:
      return status;
  }
}

function fileStatusVariant(status: string): "success" | "warning" | "error" | "info" {
  switch (status) {
    case "Completed":
      return "success";
    case "Failed":
      return "error";
    case "Processing":
      return "info";
    default:
      return "warning";
  }
}

function fileStatusLabel(status: string): string {
  switch (status) {
    case "Pending":
      return "等待中";
    case "Processing":
      return "处理中";
    case "Completed":
      return "已完成";
    case "Failed":
      return "失败";
    default:
      return status;
  }
}

const STATUS_OPTIONS: Array<{ value: RuntimeBatchStatus | ""; label: string }> = [
  { value: "", label: "全部状态" },
  { value: "Running", label: "处理中" },
  { value: "Completed", label: "已完成" },
  { value: "CompletedWithErrors", label: "部分失败" },
  { value: "Failed", label: "失败" },
];

type ListState =
  | { kind: "loading" }
  | { kind: "ready"; batches: RuntimeBatchSummary[] }
  | { kind: "error"; message: string };

type ConnectionState = "ok" | "reconnecting";

/**
 * “上传到 FileBay” 弹窗的状态机。只提交 `artifact_ids`——从不在浏览器侧
 * 拼接远程路径、读取或展示 Token，也不提供可编辑的地址/owner/repo 输入；
 * 这些安全字段全部来自 Runtime 的 `/api/v1/filebay/status` 只读投影。
 */
type UploadPanelState =
  | { stage: "closed" }
  | { stage: "loading" }
  | { stage: "error"; message: string }
  | {
      stage: "select";
      filebay: RuntimeFileBayStatusResponse;
      candidates: RuntimeFileBayCandidate[];
      selected: Set<string>;
    }
  | {
      stage: "confirm";
      filebay: RuntimeFileBayStatusResponse;
      items: RuntimeFileBayCandidate[];
    }
  | { stage: "uploading"; items: RuntimeFileBayCandidate[] }
  | { stage: "result"; results: RuntimeFileBayUploadItem[]; namesByArtifactId: Record<string, string> };

export function FileManagerBrowser() {
  const navigate = useNavigate();
  const [searchParams, setSearchParams] = useSearchParams();
  const batchId = searchParams.get("batch");

  const [listState, setListState] = useState<ListState>({ kind: "loading" });
  const [searchQuery, setSearchQuery] = useState("");
  const [statusFilter, setStatusFilter] = useState<RuntimeBatchStatus | "">("");
  const [copiedId, setCopiedId] = useState<string | null>(null);

  const [detail, setDetail] = useState<RuntimeBatchDetail | null>(null);
  const [detailError, setDetailError] = useState<string | null>(null);
  const [detailRetryToken, setDetailRetryToken] = useState(0);
  const [connection, setConnection] = useState<ConnectionState>("ok");
  const [downloadingArtifactId, setDownloadingArtifactId] = useState<string | null>(null);
  const [downloadError, setDownloadError] = useState<string | null>(null);

  const loadBatches = useCallback(async () => {
    setListState({ kind: "loading" });
    const result = await fetchRuntimeBatches();
    if (!result.ok) {
      setListState({
        kind: "error",
        message:
          result.reason === "http"
            ? result.message ?? "批次列表加载失败，请稍后再试。"
            : "当前连不上本地服务，请确认服务已启动后再试。",
      });
      return;
    }
    setListState({ kind: "ready", batches: result.data.batches });
  }, []);

  useEffect(() => {
    void loadBatches();
  }, [loadBatches]);

  // 批次详情轮询：只在存在 batchId 时进行；不重叠请求；到达终态、切换批次或
  // 卸载时停止。网络临时失败显示"重连中"并继续自动重试（大概率会恢复）；
  // HTTP 错误（如批次不存在）与响应无法解析视为该批次的终态，停止自动轮询，
  // 只提供显式"重试"入口，避免对一个确定失败的资源无限重试。
  useEffect(() => {
    if (!batchId) {
      setDetail(null);
      setDetailError(null);
      setConnection("ok");
      return;
    }

    // 切换批次或手动重试时立即清空上一个批次的详情，避免在新批次加载中/
    // 出错期间仍显示另一个批次的过期数据。
    setDetail(null);
    setDetailError(null);
    setConnection("ok");

    let cancelled = false;
    let timer: number | undefined;

    const poll = async () => {
      if (cancelled) return;
      const result = await fetchRuntimeBatch(batchId);
      if (cancelled) return;

      if (!result.ok) {
        if (result.reason === "network") {
          setConnection("reconnecting");
          timer = window.setTimeout(() => void poll(), POLL_INTERVAL_MS);
          return;
        }
        setDetailError(
          result.reason === "http"
            ? result.status === 404
              ? "这个批次已不存在，请返回列表重新选择。"
              : result.message ?? "批次详情加载失败，请稍后再试。"
            : "批次详情返回异常，请稍后再试。"
        );
        return;
      }

      setConnection("ok");
      setDetailError(null);
      setDetail(result.data);

      const terminal =
        result.data.batch.status === "Completed" ||
        result.data.batch.status === "CompletedWithErrors" ||
        result.data.batch.status === "Failed";
      if (!terminal) {
        timer = window.setTimeout(() => void poll(), POLL_INTERVAL_MS);
      }
    };

    void poll();

    return () => {
      cancelled = true;
      if (timer !== undefined) window.clearTimeout(timer);
    };
  }, [batchId, detailRetryToken]);

  const filteredBatches = useMemo(() => {
    if (listState.kind !== "ready") return [];
    const query = searchQuery.trim().toLowerCase();
    return listState.batches.filter((batch) => {
      const matchesQuery = query === "" || batch.batch_id.toLowerCase().includes(query);
      const matchesStatus = statusFilter === "" || batch.status === statusFilter;
      return matchesQuery && matchesStatus;
    });
  }, [listState, searchQuery, statusFilter]);

  const hasFilters = searchQuery.trim() !== "" || statusFilter !== "";

  const clearFilters = () => {
    setSearchQuery("");
    setStatusFilter("");
  };

  const selectBatch = (id: string) => {
    setSearchParams({ batch: id });
  };

  const backToList = () => {
    setDetail(null);
    setDetailError(null);
    setSearchParams({});
  };

  const copyBatchId = async (id: string) => {
    try {
      await navigator.clipboard.writeText(id);
      setCopiedId(id);
      window.setTimeout(() => setCopiedId((current) => (current === id ? null : current)), 1500);
    } catch {
      // 剪贴板不可用时静默忽略，不影响其余功能；批次 ID 本身仍可见可选中复制。
    }
  };

  const download = async (artifactId: string, displayName: string) => {
    if (downloadingArtifactId) return;
    setDownloadingArtifactId(artifactId);
    setDownloadError(null);
    const result = await downloadRuntimeArtifact(artifactId, displayName);
    setDownloadingArtifactId(null);
    if (!result.ok) {
      setDownloadError(
        result.reason === "network"
          ? "当前连不上本地服务，请确认服务已启动后再试。"
          : result.message ?? "下载没有成功，请稍后再试。"
      );
    }
  };

  const downloadExcelArtifactMember = async (
    artifactId: string,
    memberKind: RuntimeExcelArtifactMemberKind
  ) => {
    if (downloadingArtifactId) return;
    setDownloadingArtifactId(artifactId);
    setDownloadError(null);
    const result = await downloadRuntimeExcelArtifactMemberRequest(artifactId, memberKind);
    setDownloadingArtifactId(null);
    if (!result.ok) {
      setDownloadError(
        result.reason === "network"
          ? "当前连不上本地服务，请确认服务已启动后再试。"
          : result.message ?? "Excel 产物下载没有成功，请稍后再试。"
      );
    }
  };

  const goToUnmask = (bId: string, artifactId: string) => {
    navigate(`/unmask?batch_id=${encodeURIComponent(bId)}&artifact_id=${encodeURIComponent(artifactId)}`);
  };

  const [uploadPanel, setUploadPanel] = useState<UploadPanelState>({ stage: "closed" });
  const uploadSubmittingRef = useRef(false);

  const openUploadPanel = async (bId: string) => {
    setUploadPanel({ stage: "loading" });
    const [statusResult, candidatesResult] = await Promise.all([
      fetchRuntimeFileBayStatus(),
      fetchRuntimeFileBayCandidates(bId),
    ]);
    if (!statusResult.ok) {
      setUploadPanel({
        stage: "error",
        message:
          statusResult.reason === "network"
            ? "当前连不上本地服务，请确认服务已启动后再试。"
            : statusResult.reason === "http"
              ? statusResult.message ?? "FileBay 状态加载失败，请稍后再试。"
              : "FileBay 状态返回异常，请稍后再试。",
      });
      return;
    }
    if (!candidatesResult.ok) {
      setUploadPanel({
        stage: "error",
        message:
          candidatesResult.reason === "network"
            ? "当前连不上本地服务，请确认服务已启动后再试。"
            : candidatesResult.reason === "http"
              ? candidatesResult.message ?? "可上传文件列表加载失败，请稍后再试。"
              : "可上传文件列表返回异常，请稍后再试。",
      });
      return;
    }
    setUploadPanel({
      stage: "select",
      filebay: statusResult.data,
      candidates: candidatesResult.data.candidates,
      selected: new Set(candidatesResult.data.candidates.map((c) => c.artifact_id)),
    });
  };

  const closeUploadPanel = () => {
    setUploadPanel({ stage: "closed" });
  };

  const toggleCandidate = (artifactId: string) => {
    setUploadPanel((current) => {
      if (current.stage !== "select") return current;
      const next = new Set(current.selected);
      if (next.has(artifactId)) next.delete(artifactId);
      else next.add(artifactId);
      return { ...current, selected: next };
    });
  };

  const toggleSelectAll = () => {
    setUploadPanel((current) => {
      if (current.stage !== "select") return current;
      const allSelected = current.selected.size === current.candidates.length;
      return {
        ...current,
        selected: allSelected ? new Set() : new Set(current.candidates.map((c) => c.artifact_id)),
      };
    });
  };

  const proceedToConfirm = () => {
    setUploadPanel((current) => {
      if (current.stage !== "select" || current.selected.size === 0) return current;
      return {
        stage: "confirm",
        filebay: current.filebay,
        items: current.candidates.filter((c) => current.selected.has(c.artifact_id)),
      };
    });
  };

  const backToSelect = () => {
    setUploadPanel((current) => {
      if (current.stage !== "confirm") return current;
      return {
        stage: "select",
        filebay: current.filebay,
        candidates: current.items,
        selected: new Set(current.items.map((c) => c.artifact_id)),
      };
    });
  };

  const submitUpload = async () => {
    if (uploadSubmittingRef.current) return;
    if (uploadPanel.stage !== "confirm") return;
    uploadSubmittingRef.current = true;
    const { items } = uploadPanel;
    setUploadPanel({ stage: "uploading", items });
    const result = await confirmRuntimeFileBayUploads({ artifact_ids: items.map((c) => c.artifact_id) });
    uploadSubmittingRef.current = false;
    if (!result.ok) {
      setUploadPanel({
        stage: "error",
        message:
          result.reason === "network"
            ? "当前连不上本地服务，请确认服务已启动后再试。"
            : result.reason === "http"
              ? result.message ?? "上传没有成功，请稍后再试。"
              : "上传返回异常，请稍后再试。",
      });
      return;
    }
    const namesByArtifactId: Record<string, string> = {};
    for (const item of items) namesByArtifactId[item.artifact_id] = item.display_name;
    setUploadPanel({ stage: "result", results: result.data.items, namesByArtifactId });
  };

  if (batchId) {
    return (
      <div className="flex flex-col h-full">
        <PageHeader
          title="批次详情"
          description={`批次 ID：${batchId}`}
          actions={
            <>
              {detail && detail.batch.completed_count > 0 && (
                <Button
                  variant="secondary"
                  size="sm"
                  icon={UploadCloud}
                  onClick={() => void openUploadPanel(batchId)}
                >
                  上传到 FileBay
                </Button>
              )}
              <Button variant="secondary" size="sm" onClick={backToList}>
                返回列表
              </Button>
            </>
          }
        />
        <div className="flex-1 overflow-auto p-6 space-y-4">
          {connection === "reconnecting" && (
            <Message type="warning">
              <span className="inline-flex items-center gap-2">
                <WifiOff className="w-4 h-4" />
                当前与本地服务的连接暂时中断，正在自动重连。页面先显示最近一次成功获取的状态。
              </span>
            </Message>
          )}
          {detailError && (
            <Message type="error">
              <div className="flex items-center justify-between gap-4">
                <span>{detailError}</span>
                <Button variant="secondary" size="sm" onClick={() => setDetailRetryToken((t) => t + 1)}>
                  重试
                </Button>
              </div>
            </Message>
          )}
          {downloadError && (
            <Message type="error" onClose={() => setDownloadError(null)}>
              {downloadError}
            </Message>
          )}

          {!detail ? (
            <Loading text="正在加载批次状态..." />
          ) : (
            <>
              <Card className="p-5">
                <div className="flex items-center justify-between flex-wrap gap-3">
                  <div>
                    <div className="text-sm text-gray-500">批次状态</div>
                    <div className="mt-1 flex items-center gap-2">
                      <Badge variant={batchStatusVariant(detail.batch.status)}>
                        {batchStatusLabel(detail.batch.status)}
                      </Badge>
                    </div>
                  </div>
                  <div className="grid grid-cols-4 gap-6 text-center">
                    <div>
                      <div className="text-2xl font-bold text-gray-900">{detail.batch.file_count}</div>
                      <div className="text-xs text-gray-500">总文件</div>
                    </div>
                    <div>
                      <div className="text-2xl font-bold text-green-600">{detail.batch.completed_count}</div>
                      <div className="text-xs text-gray-500">已完成</div>
                    </div>
                    <div>
                      <div className="text-2xl font-bold text-red-600">{detail.batch.failed_count}</div>
                      <div className="text-xs text-gray-500">失败</div>
                    </div>
                    <div>
                      <div className="text-2xl font-bold text-blue-600">{detail.batch.masked_entity_count}</div>
                      <div className="text-xs text-gray-500">脱敏实体数</div>
                    </div>
                  </div>
                </div>
              </Card>

              <Card className="overflow-hidden">
                <table className="w-full text-sm">
                  <thead className="bg-gray-50 text-left text-gray-500">
                    <tr>
                      <th className="px-4 py-2">文件</th>
                      <th className="px-4 py-2">格式</th>
                      <th className="px-4 py-2">状态</th>
                      <th className="px-4 py-2">尝试次数</th>
                      <th className="px-4 py-2">脱敏实体数</th>
                      <th className="px-4 py-2">操作</th>
                    </tr>
                  </thead>
                  <tbody className="divide-y divide-gray-100">
                    {detail.files.map((file: RuntimeBatchFile) => (
                      <tr key={file.file_id}>
                        <td className="px-4 py-3">
                          <div className="font-medium text-gray-900 break-all">
                            {safeDisplayName(file.display_name)}
                          </div>
                          {file.status === "Failed" && (
                            <div className="mt-1 text-xs text-red-600">
                              <span className="font-mono">{file.error_code ?? "PROCESSING_FAILED"}</span>
                              {file.error_message && <span>：{file.error_message}</span>}
                            </div>
                          )}
                        </td>
                        <td className="px-4 py-3">{runtimeFormatLabel(file.input_format)}</td>
                        <td className="px-4 py-3">
                          <Badge variant={fileStatusVariant(file.status)}>{fileStatusLabel(file.status)}</Badge>
                        </td>
                        <td className="px-4 py-3">{file.attempt}</td>
                        <td className="px-4 py-3">{file.masked_entity_count ?? "—"}</td>
                        <td className="px-4 py-3">
                          <div className="flex items-center gap-2">
                            {file.status === "Completed" &&
                            file.artifact_id &&
                            file.artifact_kind !== "excel_bundle_manifest" && (
                              <Button
                                variant="secondary"
                                size="sm"
                                icon={Download}
                                disabled={downloadingArtifactId === file.artifact_id}
                                onClick={() => void download(file.artifact_id as string, file.display_name)}
                              >
                                {downloadingArtifactId === file.artifact_id ? "下载中…" : "下载 Markdown"}
                              </Button>
                            )}
                            {file.status === "Completed" &&
                            file.artifact_id &&
                            file.artifact_kind === "excel_bundle_manifest" && (
                              <div className="flex flex-wrap items-center gap-2">
                                {EXCEL_ARTIFACT_ACTIONS.map((action) => (
                                  <Button
                                    key={action.kind}
                                    variant="secondary"
                                    size="sm"
                                    icon={Download}
                                    disabled={downloadingArtifactId === file.artifact_id}
                                    onClick={() =>
                                      void downloadExcelArtifactMember(
                                        file.artifact_id as string,
                                        action.kind
                                      )
                                    }
                                  >
                                    {downloadingArtifactId === file.artifact_id
                                      ? "下载中…"
                                      : action.label}
                                  </Button>
                                ))}
                              </div>
                            )}
                            {file.status === "Completed" && file.artifact_id && file.restore_available ? (
                              <Button
                                variant="secondary"
                                size="sm"
                                icon={Unlock}
                                onClick={() => goToUnmask(detail.batch.batch_id, file.artifact_id as string)}
                              >
                                反脱敏
                              </Button>
                            ) : file.status === "Completed" ? (
                              <span className="text-xs text-gray-400">暂无可用映射</span>
                            ) : null}
                          </div>
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </Card>
            </>
          )}
        </div>

        {uploadPanel.stage !== "closed" && (
          <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40 p-4">
            <Card className="w-full max-w-lg p-6 space-y-4 max-h-[85vh] overflow-auto">
              {uploadPanel.stage === "loading" && (
                <div className="py-6">
                  <Loading text="正在加载可上传文件…" />
                </div>
              )}

              {uploadPanel.stage === "error" && (
                <>
                  <h3 className="text-base font-semibold text-gray-900">上传到 FileBay</h3>
                  <Message type="error">{uploadPanel.message}</Message>
                  <div className="flex justify-end">
                    <Button variant="secondary" size="sm" onClick={closeUploadPanel}>
                      关闭
                    </Button>
                  </div>
                </>
              )}

              {uploadPanel.stage === "select" && (
                <>
                  <h3 className="text-base font-semibold text-gray-900">选择要上传到 FileBay 的文件</h3>
                  {uploadPanel.filebay.status !== "configured" ? (
                    <Message type="warning">
                      FileBay 还没有准备好，暂时无法上传。请先到“FileBay 设置”页面检查状态。
                    </Message>
                  ) : uploadPanel.candidates.length === 0 ? (
                    <Message type="info">这个批次里还没有可上传的脱敏 Markdown 文件。</Message>
                  ) : (
                    <>
                      <div className="text-sm text-gray-500">
                        目标：<span className="font-mono">{uploadPanel.filebay.target_origin}</span>{" "}
                        <span className="font-mono">
                          {uploadPanel.filebay.owner}/{uploadPanel.filebay.repo}
                        </span>
                      </div>
                      <div className="flex items-center justify-between">
                        <span className="text-sm text-gray-500">
                          已选择 {uploadPanel.selected.size}/{uploadPanel.candidates.length} 个文件
                        </span>
                        <Button variant="secondary" size="sm" onClick={toggleSelectAll}>
                          {uploadPanel.selected.size === uploadPanel.candidates.length ? "取消全选" : "全选"}
                        </Button>
                      </div>
                      <div className="border border-gray-200 rounded-lg divide-y divide-gray-100 max-h-64 overflow-auto">
                        {uploadPanel.candidates.map((candidate) => (
                          <label
                            key={candidate.artifact_id}
                            className="flex items-start gap-3 px-3 py-2 text-sm cursor-pointer hover:bg-gray-50"
                          >
                            <input
                              type="checkbox"
                              className="mt-1"
                              checked={uploadPanel.selected.has(candidate.artifact_id)}
                              onChange={() => toggleCandidate(candidate.artifact_id)}
                            />
                            <div className="min-w-0">
                              <div className="font-medium text-gray-900 break-all">
                                {safeDisplayName(candidate.display_name)}
                              </div>
                              <div className="text-xs text-gray-500 font-mono break-all">
                                {candidate.remote_path}
                              </div>
                            </div>
                          </label>
                        ))}
                      </div>
                    </>
                  )}
                  <div className="flex justify-end gap-3 pt-2">
                    <Button variant="secondary" size="sm" onClick={closeUploadPanel}>
                      取消
                    </Button>
                    {uploadPanel.filebay.status === "configured" && uploadPanel.candidates.length > 0 && (
                      <Button size="sm" disabled={uploadPanel.selected.size === 0} onClick={proceedToConfirm}>
                        下一步
                      </Button>
                    )}
                  </div>
                </>
              )}

              {uploadPanel.stage === "confirm" && (
                <>
                  <h3 className="text-base font-semibold text-gray-900">确认上传</h3>
                  <div className="text-sm text-gray-700 space-y-1">
                    <div>
                      目标地址：<span className="font-mono">{uploadPanel.filebay.target_origin}</span>
                    </div>
                    <div>
                      目标仓库：
                      <span className="font-mono">
                        {uploadPanel.filebay.owner}/{uploadPanel.filebay.repo}
                      </span>
                    </div>
                  </div>
                  <div className="border border-gray-200 rounded-lg divide-y divide-gray-100 max-h-56 overflow-auto">
                    {uploadPanel.items.map((item) => (
                      <div key={item.artifact_id} className="px-3 py-2 text-sm">
                        <div className="font-medium text-gray-900 break-all">
                          {safeDisplayName(item.display_name)}
                        </div>
                        <div className="text-xs text-gray-500 font-mono break-all">{item.remote_path}</div>
                      </div>
                    ))}
                  </div>
                  <Message type="info">
                    这里只会上传上面这些脱敏后的 Markdown 文件，不会上传原文件、映射文件（.cmap）或还原文件。
                  </Message>
                  <div className="flex justify-end gap-3 pt-2">
                    <Button variant="secondary" size="sm" onClick={backToSelect}>
                      上一步
                    </Button>
                    <Button size="sm" onClick={() => void submitUpload()}>
                      确认上传（{uploadPanel.items.length}）
                    </Button>
                  </div>
                </>
              )}

              {uploadPanel.stage === "uploading" && (
                <div className="py-6">
                  <Loading text={`正在上传 ${uploadPanel.items.length} 个文件...`} />
                </div>
              )}

              {uploadPanel.stage === "result" && (
                <>
                  <h3 className="text-base font-semibold text-gray-900">上传结果</h3>
                  <div className="border border-gray-200 rounded-lg divide-y divide-gray-100 max-h-64 overflow-auto">
                    {uploadPanel.results.map((item) => (
                      <div key={item.artifact_id} className="px-3 py-2 text-sm flex items-start justify-between gap-3">
                        <div className="min-w-0">
                          <div className="font-medium text-gray-900 break-all">
                            {safeDisplayName(uploadPanel.namesByArtifactId[item.artifact_id] ?? item.artifact_id)}
                          </div>
                          <div className="text-xs text-gray-500 font-mono break-all">{item.remote_path}</div>
                          {!item.success && item.error_code && (
                            <div className="text-xs text-red-600 mt-0.5">错误代码：{item.error_code}</div>
                          )}
                          {item.success && item.url && (
                            <a
                              href={item.url}
                              target="_blank"
                              rel="noreferrer"
                              className="text-xs text-blue-600 hover:underline break-all"
                            >
                              查看文件
                            </a>
                          )}
                        </div>
                        <Badge variant={item.success ? "success" : "error"}>
                          {item.success ? "成功" : "失败"}
                        </Badge>
                      </div>
                    ))}
                  </div>
                  <div className="flex justify-end pt-2">
                    <Button size="sm" onClick={closeUploadPanel}>
                      关闭
                    </Button>
                  </div>
                </>
              )}
            </Card>
          </div>
        )}
      </div>
    );
  }

  return (
    <div className="flex flex-col h-full">
      <PageHeader
        title="文件管理"
        description="查看已保存的处理结果。本页只展示本地服务保存的批次记录，不会浏览任意服务器目录。"
        actions={
          <Button variant="secondary" size="sm" icon={RefreshCw} onClick={() => void loadBatches()}>
            刷新
          </Button>
        }
      />
      <div className="flex-1 overflow-auto p-6 space-y-4">
        {listState.kind === "loading" && <Loading text="正在加载批次列表..." />}

        {listState.kind === "error" && (
          <Message type="error">
            <div className="flex items-center justify-between gap-4">
              <span className="inline-flex items-center gap-2">
                <WifiOff className="w-4 h-4" />
                {listState.message}
              </span>
              <Button variant="secondary" size="sm" onClick={() => void loadBatches()}>
                重试
              </Button>
            </div>
          </Message>
        )}

        {listState.kind === "ready" && (
          <>
            <Card className="p-4">
              <div className="flex flex-wrap items-center gap-3">
                <div className="relative flex-1 min-w-[220px]">
                  <Search className="w-4 h-4 text-gray-400 absolute left-3 top-1/2 -translate-y-1/2" />
                  <Input
                    className="pl-9"
                    placeholder="按批次 ID 搜索…"
                    value={searchQuery}
                    onChange={(event: ChangeEvent<HTMLInputElement>) => setSearchQuery(event.target.value)}
                    aria-label="按批次 ID 搜索"
                  />
                </div>
                <select
                  className="border border-gray-300 rounded-lg px-3 py-2.5 text-sm text-gray-700"
                  value={statusFilter}
                  onChange={(event) => setStatusFilter(event.target.value as RuntimeBatchStatus | "")}
                  aria-label="按批次状态筛选"
                >
                  {STATUS_OPTIONS.map((option) => (
                    <option key={option.value} value={option.value}>
                      {option.label}
                    </option>
                  ))}
                </select>
                {hasFilters && (
                  <Button variant="secondary" size="sm" icon={X} onClick={clearFilters}>
                    清空条件
                  </Button>
                )}
              </div>
            </Card>

            {listState.batches.length === 0 ? (
              <Card className="p-10 text-center text-gray-500">
                还没有处理记录。你在“文件脱敏”里完成第一批处理后，结果会显示在这里。
              </Card>
            ) : filteredBatches.length === 0 ? (
              <Card className="p-10 text-center text-gray-500">
                没有找到符合条件的批次，试试调整关键词或筛选条件。
                <div className="mt-3">
                  <Button variant="secondary" size="sm" onClick={clearFilters}>
                    清空筛选条件
                  </Button>
                </div>
              </Card>
            ) : (
              <Card className="overflow-hidden">
                <table className="w-full text-sm">
                  <thead className="bg-gray-50 text-left text-gray-500">
                    <tr>
                      <th className="px-4 py-2">批次 ID</th>
                      <th className="px-4 py-2">状态</th>
                      <th className="px-4 py-2">文件数</th>
                      <th className="px-4 py-2">完成</th>
                      <th className="px-4 py-2">失败</th>
                      <th className="px-4 py-2">脱敏实体数</th>
                      <th className="px-4 py-2">更新时间</th>
                      <th className="px-4 py-2" />
                    </tr>
                  </thead>
                  <tbody className="divide-y divide-gray-100">
                    {filteredBatches.map((batch) => (
                      <tr key={batch.batch_id} className="hover:bg-gray-50">
                        <td className="px-4 py-3">
                          <div className="flex items-center gap-2">
                            <span className="font-mono text-xs text-gray-700" title={batch.batch_id}>
                              {batch.batch_id.slice(0, 8)}
                            </span>
                            <button
                              type="button"
                              className="text-gray-400 hover:text-gray-600"
                              aria-label="复制完整批次 ID"
                              onClick={() => void copyBatchId(batch.batch_id)}
                            >
                              <Copy className="w-3.5 h-3.5" />
                            </button>
                            {copiedId === batch.batch_id && (
                              <span className="text-xs text-green-600">已复制</span>
                            )}
                          </div>
                        </td>
                        <td className="px-4 py-3">
                          <Badge variant={batchStatusVariant(batch.status)}>
                            {batchStatusLabel(batch.status)}
                          </Badge>
                        </td>
                        <td className="px-4 py-3">{batch.file_count}</td>
                        <td className="px-4 py-3">{batch.completed_count}</td>
                        <td className="px-4 py-3">{batch.failed_count}</td>
                        <td className="px-4 py-3">{batch.masked_entity_count}</td>
                        <td className="px-4 py-3 text-gray-500">{formatTime(batch.updated_at)}</td>
                        <td className="px-4 py-3">
                          <Button variant="secondary" size="sm" onClick={() => selectBatch(batch.batch_id)}>
                            查看详情
                          </Button>
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </Card>
            )}
          </>
        )}
      </div>
    </div>
  );
}
