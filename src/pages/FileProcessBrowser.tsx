import { useCallback, useEffect, useRef, useState, type ChangeEvent, type DragEvent } from "react";
import { flushSync } from "react-dom";
import { useNavigate, useSearchParams } from "react-router-dom";
import { useAppStore } from "@/store/appStore";
import {
  AlertTriangle,
  Download,
  Info,
  Lightbulb,
  RefreshCw,
  Trash2,
  UploadCloud,
  WifiOff,
} from "lucide-react";
import { PageHeader } from "@/components/layout/PageHeader";
import { Button, Message, Badge, Card, Loading } from "@/components/ui/cheersai-ui";
import { BrowserMaskingPreviewDialog } from "@/components/file/BrowserMaskingPreviewDialog";
import {
  cancelRuntimePreview,
  confirmRuntimePreview,
  createRuntimePreview,
  downloadRuntimeArtifact,
  fetchRuntimeBatch,
  fetchRuntimePreview,
  fetchRuntimeRules,
  fetchRuntimeSensitiveTermsStats,
  retryRuntimeFile,
} from "@/lib/runtime/client";
import {
  isRuntimeFormatSupported,
  runtimeAcceptAttribute,
  runtimeFormatLabel,
  runtimeInputFormatFromFilename,
} from "@/lib/runtime/formatCatalog";
import type {
  RuntimeBatchDetail,
  RuntimeBatchFile,
  RuntimePreviewDetail,
  RuntimeRuleMetadata,
} from "@/types/runtime";

const MAX_FILES = 100;
const MAX_FILE_BYTES = 500 * 1024 * 1024;
const MAX_BATCH_BYTES = 2 * 1024 * 1024 * 1024;
const POLL_INTERVAL_MS = 1500;

const UUID_RE =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

function isValidPreviewId(value: string | null | undefined): value is string {
  return typeof value === "string" && UUID_RE.test(value.trim());
}

interface QueuedFile {
  key: string;
  file: File;
}

function fileKey(file: File): string {
  return `${file.name}\u0000${file.size}\u0000${file.lastModified}`;
}

function readableSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
  return `${(bytes / 1024 / 1024 / 1024).toFixed(2)} GB`;
}

/** 安全展示用的文件名：只做长度与控制字符处理，不解释为路径。 */
function safeDisplayName(name: string): string {
  // eslint-disable-next-line no-control-regex
  return name.replace(/[\x00-\x1f\x7f]/g, "");
}

type RulesState =
  | { kind: "loading" }
  | { kind: "ready"; rules: RuntimeRuleMetadata[] }
  | { kind: "error"; message: string };

const SENSITIVE_TERMS_RULE_ID = "use_sensitive_terms";

type SensitiveTermsSummaryState =
  | { kind: "loading" }
  | { kind: "ready"; enabledCount: number }
  | { kind: "error" };

type ConnectionState = "ok" | "reconnecting";

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

export default function FileProcessBrowser() {
  const navigate = useNavigate();
  const [searchParams, setSearchParams] = useSearchParams();
  const batchId = searchParams.get("batch");
  const urlPreviewId = searchParams.get("preview");
  const { activePreviewId, setActivePreviewId } = useAppStore();


  // URL 与全局 store/sessionStorage 的同步：
  // - 首次加载时，若 URL 带合法 preview_id 且 store 为空，则从 URL 恢复（支持
  //   直接刷新 `#/process?preview=<id>`）。
  // - 此后以 store 为唯一来源：store 变化时同步到 URL；store 被显式清空时
  //   URL 也清空，避免 cancel/confirm 后被 URL sync 自动恢复回来。
  const restoredFromUrlRef = useRef(false);
  const pendingBatchIdRef = useRef<string | null>(null);
  useEffect(() => {
    // `setSearchParams` 的导航更新与 Zustand 的 preview 清空可能跨一个
    // render；确认成功后，在 batch 参数真正落入 URL 前，不能让旧 preview
    // 清理分支覆盖这次导航。
    if (pendingBatchIdRef.current) {
      if (batchId === pendingBatchIdRef.current) {
        pendingBatchIdRef.current = null;
      } else {
        return;
      }
    }
    // 正式批次详情使用独立的 `batch` 查询参数。确认 preview 时会同时清空
    // 活动 preview 并切换到 batch；此处必须让批次导航成为终态，不能被
    // preview 清理分支竞争性覆盖为无参数的 /process。
    if (batchId) return;
    if (!restoredFromUrlRef.current) {
      restoredFromUrlRef.current = true;
      if (isValidPreviewId(urlPreviewId) && !isValidPreviewId(activePreviewId)) {
        setActivePreviewId(urlPreviewId);
        return;
      }
    }
    if (isValidPreviewId(activePreviewId)) {
      if (urlPreviewId !== activePreviewId) {
        setSearchParams({ preview: activePreviewId }, { replace: true });
      }
    } else if (urlPreviewId) {
      setSearchParams({}, { replace: true });
    }
  }, [batchId, urlPreviewId, activePreviewId, setActivePreviewId, setSearchParams]);

  // URL 参数优先；若 URL 没有但全局 store/sessionStorage 有活动 preview，则恢复。
  const previewId = isValidPreviewId(urlPreviewId)
    ? urlPreviewId
    : isValidPreviewId(activePreviewId)
      ? activePreviewId
      : null;

  const [queue, setQueue] = useState<QueuedFile[]>([]);
  const [addWarning, setAddWarning] = useState<string | null>(null);
  const [rulesState, setRulesState] = useState<RulesState>({ kind: "loading" });
  const [selectedRuleIds, setSelectedRuleIds] = useState<Set<string>>(new Set());
  const [sensitiveTermsSummary, setSensitiveTermsSummary] = useState<SensitiveTermsSummaryState>({ kind: "loading" });
  const [submitting, setSubmitting] = useState(false);
  const [submitPhase, setSubmitPhase] = useState<"idle" | "uploading" | "submitted" | "error">("idle");
  const [submitError, setSubmitError] = useState<string | null>(null);
  const [submitStartTime, setSubmitStartTime] = useState<number | null>(null);
  const [elapsedMs, setElapsedMs] = useState(0);
  const [lastUpdatedAt, setLastUpdatedAt] = useState<number | null>(null);
  // multipart 上传期间阻止浏览器刷新/关闭标签页，避免在请求返回前丢失可能
  // 已创建的 preview。in-app 路由离开则通过不 abort 请求 + 全局 store/sessionStorage
  // 持久化 preview_id 来保证返回后可恢复。
  useEffect(() => {
    if (submitPhase !== "uploading") return;
    const handler = (event: BeforeUnloadEvent) => {
      event.preventDefault();
      event.returnValue = "";
    };
    window.addEventListener("beforeunload", handler);
    return () => window.removeEventListener("beforeunload", handler);
  }, [submitPhase]);

  const [detail, setDetail] = useState<RuntimeBatchDetail | null>(null);
  const [detailError, setDetailError] = useState<string | null>(null);
  const [connection, setConnection] = useState<ConnectionState>("ok");
  const [retryingFileId, setRetryingFileId] = useState<string | null>(null);
  const [downloadingArtifactId, setDownloadingArtifactId] = useState<string | null>(null);
  const [downloadError, setDownloadError] = useState<string | null>(null);
  // 成功 retry 后自增，强制轮询 effect 重新挂载并从当前状态继续查询直到终态；
  // 轮询 effect 本身到达终态后会停止 timer，不靠这个值区分"是否终态"。
  const [pollResumeToken, setPollResumeToken] = useState(0);

  const [previewDetail, setPreviewDetail] = useState<RuntimePreviewDetail | null>(null);
  const [previewDetailError, setPreviewDetailError] = useState<string | null>(null);

  // 上传 / 处理阶段的活动计时器：只在真正等待时走动，到达终态或错误后停止。
  useEffect(() => {
    const isWaiting =
      submitPhase === "uploading" ||
      submitPhase === "submitted" ||
      (!!previewId && previewDetail?.status === "Processing");
    if (!isWaiting) {
      return;
    }
    const start = Date.now();
    const tick = () => {
      setElapsedMs(Date.now() - start);
    };
    tick();
    const timer = window.setInterval(tick, 1000);
    return () => window.clearInterval(timer);
  }, [submitPhase, previewId, previewDetail?.status]);

  const [previewExpired, setPreviewExpired] = useState(false);
  const [previewConnection, setPreviewConnection] = useState<ConnectionState>("ok");
  const [confirming, setConfirming] = useState(false);
  const [confirmError, setConfirmError] = useState<string | null>(null);
  const [cancelling, setCancelling] = useState(false);
  const [cancelError, setCancelError] = useState<string | null>(null);

  const loadRules = useCallback(async () => {
    setRulesState({ kind: "loading" });
    const result = await fetchRuntimeRules();
    if (!result.ok) {
      setRulesState({
        kind: "error",
        message:
          result.reason === "http"
            ? result.message ?? "规则加载失败，请稍后重试。"
            : "无法连接本机 Runtime，请确认服务已启动后重试。",
      });
      return;
    }
    setRulesState({ kind: "ready", rules: result.data.rules });
    setSelectedRuleIds(new Set(result.data.rules.filter((rule) => rule.enabled_by_default).map((rule) => rule.id)));
  }, []);

  useEffect(() => {
    void loadRules();
  }, [loadRules]);

  const loadSensitiveTermsSummary = useCallback(async () => {
    setSensitiveTermsSummary({ kind: "loading" });
    const result = await fetchRuntimeSensitiveTermsStats();
    if (!result.ok) {
      setSensitiveTermsSummary({ kind: "error" });
      return;
    }
    setSensitiveTermsSummary({ kind: "ready", enabledCount: result.data.enabled });
  }, []);

  useEffect(() => {
    void loadSensitiveTermsSummary();
  }, [loadSensitiveTermsSummary]);

  // 批次轮询：只在存在 batchId 时进行；不重叠请求；到达终态或卸载时停止；
  // 网络临时失败显示"重连中"而不是把批次伪造为 Failed。
  useEffect(() => {
    if (!batchId) {
      setDetail(null);
      setDetailError(null);
      setConnection("ok");
      return;
    }

    let cancelled = false;
    let timer: number | undefined;

    const poll = async () => {
      if (cancelled) return;
      const result = await fetchRuntimeBatch(batchId);
      if (cancelled) return;

      if (!result.ok) {
        if (result.reason === "http") {
          setDetailError(result.message ?? "批次查询失败，请稍后重试。");
        } else {
          setConnection("reconnecting");
        }
        timer = window.setTimeout(() => void poll(), POLL_INTERVAL_MS);
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
  }, [batchId, pollResumeToken]);

  // 预览会话轮询：只在存在 previewId 时进行；到达非 Processing 的终态即停止；
  // 404/410（过期或 Runtime 重启后不存在）单独标记为 previewExpired，不再继续
  // 轮询，也不把它伪装成某种"处理失败"状态。
  useEffect(() => {
    if (!previewId) {
      setPreviewDetail(null);
      setPreviewDetailError(null);
      setPreviewExpired(false);
      setPreviewConnection("ok");
      return;
    }

    let cancelled = false;
    let timer: number | undefined;
    // 新的 previewId 一律从干净状态重新开始：上一个预览会话残留的
    // confirm/cancel 错误提示不得挂在一个全新的预览会话上。
    setConfirmError(null);
    setCancelError(null);
    // 新的 previewId 一律从"未过期"状态重新开始轮询：上一个预览会话的
    // previewExpired=true 不得残留到这次导航，否则一个刚创建的合法预览会
    // 被错误地展示为"已过期"。
    setPreviewExpired(false);

    const poll = async () => {
      if (cancelled) return;
      const result = await fetchRuntimePreview(previewId);
      if (cancelled) return;

      if (!result.ok) {
        if (result.reason === "http") {
          if (result.status === 404 || result.status === 410) {
            setPreviewExpired(true);
            setPreviewDetail(null);
            setActivePreviewId(null);
            return;
          }
          setPreviewDetailError(result.message ?? "预览查询失败，请稍后重试。");
        } else {
          setPreviewConnection("reconnecting");
        }
        timer = window.setTimeout(() => void poll(), POLL_INTERVAL_MS);
        return;
      }

      setPreviewConnection("ok");
      setPreviewDetailError(null);
      setPreviewExpired(false);
      setPreviewDetail(result.data);
      setLastUpdatedAt(Date.now());

      if (result.data.status === "Processing") {
        timer = window.setTimeout(() => void poll(), POLL_INTERVAL_MS);
      }
    };

    void poll();

    return () => {
      cancelled = true;
      if (timer !== undefined) window.clearTimeout(timer);
    };
  }, [previewId]);

  const addFiles = (incoming: File[]) => {
    if (incoming.length === 0) return;

    const existingKeys = new Set(queue.map((item) => item.key));
    const nextQueue = [...queue];
    let runningBytes = queue.reduce((sum, item) => sum + item.file.size, 0);

    const rejectedUnsupported: string[] = [];
    const rejectedOversize: string[] = [];
    const rejectedBatchLimit: string[] = [];
    const rejectedCountLimit: string[] = [];
    let duplicateCount = 0;

    for (const file of incoming) {
      const key = fileKey(file);
      if (existingKeys.has(key)) {
        duplicateCount += 1;
        continue;
      }
      if (!isRuntimeFormatSupported(file.name)) {
        rejectedUnsupported.push(safeDisplayName(file.name));
        continue;
      }
      if (nextQueue.length >= MAX_FILES) {
        rejectedCountLimit.push(safeDisplayName(file.name));
        continue;
      }
      if (file.size > MAX_FILE_BYTES) {
        rejectedOversize.push(safeDisplayName(file.name));
        continue;
      }
      if (runningBytes + file.size > MAX_BATCH_BYTES) {
        rejectedBatchLimit.push(safeDisplayName(file.name));
        continue;
      }
      existingKeys.add(key);
      nextQueue.push({ key, file });
      runningBytes += file.size;
    }

    setQueue(nextQueue);

    const notes: string[] = [];
    if (rejectedUnsupported.length > 0) {
      notes.push(`不支持的格式，已跳过：${rejectedUnsupported.join("、")}`);
    }
    if (rejectedOversize.length > 0) {
      notes.push(`单文件超过 500 MB，已跳过：${rejectedOversize.join("、")}`);
    }
    if (rejectedBatchLimit.length > 0) {
      notes.push(`加入后批次将超过 2 GB，已跳过：${rejectedBatchLimit.join("、")}`);
    }
    if (rejectedCountLimit.length > 0) {
      notes.push(`单批最多 ${MAX_FILES} 个文件，已跳过：${rejectedCountLimit.join("、")}`);
    }
    if (duplicateCount > 0) {
      notes.push(`已跳过 ${duplicateCount} 个重复文件（文件名、大小与修改时间均相同）`);
    }
    setAddWarning(notes.length > 0 ? notes.join("；") : null);
  };

  const handleFileInput = (event: ChangeEvent<HTMLInputElement>) => {
    const chosen = Array.from(event.target.files ?? []);
    addFiles(chosen);
    event.target.value = "";
  };

  const handleDrop = (event: DragEvent<HTMLLabelElement>) => {
    event.preventDefault();
    if (submitting) return;
    const dropped = Array.from(event.dataTransfer.files ?? []);
    addFiles(dropped);
  };

  const handleDragOver = (event: DragEvent<HTMLLabelElement>) => {
    event.preventDefault();
  };

  const removeFile = (key: string) => {
    if (submitting) return;
    setQueue((current) => current.filter((item) => item.key !== key));
  };

  const toggleRule = (ruleId: string) => {
    setSelectedRuleIds((current) => {
      const next = new Set(current);
      if (next.has(ruleId)) next.delete(ruleId);
      else next.add(ruleId);
      return next;
    });
  };

  const totalBytes = queue.reduce((sum, item) => sum + item.file.size, 0);
  const canSubmit =
    queue.length > 0 && selectedRuleIds.size > 0 && !submitting && rulesState.kind === "ready";

  const abortControllerRef = useRef<AbortController | null>(null);

  const generatePreview = async () => {
    if (!canSubmit) return;
    setSubmitting(true);
    setSubmitPhase("uploading");
    setSubmitError(null);
    setSubmitStartTime(Date.now());
    setElapsedMs(0);
    const controller = new AbortController();
    abortControllerRef.current = controller;
    try {
      const files = queue.map((item) => item.file);
      const result = await createRuntimePreview(files, Array.from(selectedRuleIds), controller.signal);
      if (!result.ok) {
        setSubmitPhase("error");
        setSubmitError(
          result.reason === "http"
            ? result.message ?? "生成预览失败，请稍后重试。"
            : "无法连接本机 Runtime，请确认服务已启动后重试。"
        );
        return;
      }
      flushSync(() => setSubmitPhase("submitted"));
      // 让“已提交”状态先完成一次真实渲染，再切换到 preview 轮询页面；
      // 不引入固定延迟，只等待浏览器下一帧，避免状态在同一 React 批次内被
      // preview 导航覆盖而从未对用户可见。
      await new Promise<void>((resolve) => window.requestAnimationFrame(() => resolve()));
      // 队列与规则选择保留在内存中：预览取消后用户可以直接调整并重新生成，
      // 不在这里清空（只有确认成功后才清空，见 confirmPreview）。
      // 同时把活动 previewId 写入会话级 store 与 sessionStorage，支持切页/刷新恢复。
      setActivePreviewId(result.data.preview_id);
      setSearchParams({ preview: result.data.preview_id });
    } finally {
      setSubmitting(false);
      if (abortControllerRef.current === controller) {
        abortControllerRef.current = null;
      }
    }
  };


  const startNewBatch = () => {
    setDetail(null);
    setDetailError(null);
    setSearchParams({});
  };

  const confirmPreview = async () => {
    if (!previewId || confirming) return;
    setConfirming(true);
    setConfirmError(null);
    try {
      const result = await confirmRuntimePreview(previewId);
      if (!result.ok) {
        setConfirmError(
          result.reason === "http"
            ? result.message ?? "确认失败，请稍后重试。"
            : "无法连接本机 Runtime，请确认服务已启动后重试。"
        );
        return;
      }
      pendingBatchIdRef.current = result.data.batch_id;
      // 先提交批次 URL，再清空活动 preview。这样 URL 同步 effect 在同一
      // 次状态转移中先看到 batch 终态，不会把确认后的正式批次导航清回 /process。
      setSearchParams({ batch: result.data.batch_id });
      setQueue([]);
      setAddWarning(null);
      setPreviewDetail(null);
      setActivePreviewId(null);
      setElapsedMs(0);
      setLastUpdatedAt(null);
      setSubmitPhase("idle");
    } finally {
      setConfirming(false);
    }
  };

  const cancelPreview = async () => {
    if (!previewId || cancelling) return;
    setCancelling(true);
    setCancelError(null);
    try {
      const result = await cancelRuntimePreview(previewId);
      if (!result.ok) {
        setCancelError(
          result.reason === "http"
            ? result.message ?? "取消失败，请稍后重试。"
            : "无法连接本机 Runtime，请确认服务已启动后重试。"
        );
        return;
      }
      // 保留 queue/selectedRuleIds：用户可以直接调整后重新生成预览。
      setPreviewDetail(null);
      setActivePreviewId(null);
      setElapsedMs(0);
      setLastUpdatedAt(null);
      setSubmitPhase("idle");
      setSearchParams({});
    } finally {
      setCancelling(false);
    }
  };

  /** 纯前端状态重置，不调用取消 API——用于预览已过期/Runtime 已重启的场景
   * （此时服务端会话本就不存在），以及页面顶部"重新选择文件"导航入口。 */
  const startOverFromPreview = () => {
    setPreviewDetail(null);
    setPreviewDetailError(null);
    setPreviewExpired(false);
    setConfirmError(null);
    setCancelError(null);
    setActivePreviewId(null);
    setElapsedMs(0);
    setLastUpdatedAt(null);
    setSubmitPhase("idle");
    setSearchParams({});
  };

  const retry = async (fileId: string) => {
    if (retryingFileId) return;
    setRetryingFileId(fileId);
    const result = await retryRuntimeFile(fileId);
    setRetryingFileId(null);
    if (!result.ok) {
      setDetailError(
        result.reason === "http"
          ? result.message ?? "重试请求失败，请稍后再试。"
          : "无法连接本机 Runtime，请确认服务已启动后重试。"
      );
      return;
    }
    // 重新挂载轮询 effect，从 Runtime 当前状态继续查询直到下一次终态；
    // 不再靠这里的一次性请求收尾，避免非终态结果后没有 timer 继续查询。
    setPollResumeToken((token) => token + 1);
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
          ? "无法连接本机 Runtime，请确认服务已启动后重试。"
          : result.message ?? "下载失败，请稍后重试。"
      );
    }
  };

  if (batchId) {
    return (
      <div className="flex flex-col h-full">
        <PageHeader
          title="批次详情"
          description={`批次 ID：${batchId}`}
          actions={
            <Button variant="secondary" size="sm" onClick={startNewBatch}>
              新建批次
            </Button>
          }
        />
        <div className="flex-1 overflow-auto p-6 space-y-4">
          {connection === "reconnecting" && (
            <Message type="warning">
              <span className="inline-flex items-center gap-2">
                <WifiOff className="w-4 h-4" />
                与 Runtime 的连接暂时中断，正在自动重连……当前展示的是最近一次成功获取的状态。
              </span>
            </Message>
          )}
          {detailError && <Message type="error">{detailError}</Message>}
          {downloadError && <Message type="error" onClose={() => setDownloadError(null)}>{downloadError}</Message>}

          {!detail ? (
            <Loading text="正在加载批次状态…" />
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
                          <div className="font-medium text-gray-900">{safeDisplayName(file.display_name)}</div>
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
                            {file.status === "Failed" && (
                              <Button
                                variant="secondary"
                                size="sm"
                                icon={RefreshCw}
                                disabled={retryingFileId === file.file_id}
                                onClick={() => void retry(file.file_id)}
                              >
                                {retryingFileId === file.file_id ? "重试中…" : "重新处理"}
                              </Button>
                            )}
                            {file.artifact_id && (
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
      </div>
    );
  }

  if (previewId) {
    return (
      <div className="flex flex-col h-full">
        <PageHeader
          title="脱敏预览"
          description={`预览 ID：${previewId}`}
          actions={
            <Button variant="secondary" size="sm" onClick={startOverFromPreview}>
              重新选择文件
            </Button>
          }
        />
        <div className="flex-1 overflow-auto p-6 space-y-4">
          {previewConnection === "reconnecting" && (
            <Message type="warning">
              <span className="inline-flex items-center gap-2">
                <WifiOff className="w-4 h-4" />
                与 Runtime 的连接暂时中断，正在自动重连……当前展示的是最近一次成功获取的状态。
              </span>
            </Message>
          )}
          {previewDetailError && <Message type="error">{previewDetailError}</Message>}

          {previewExpired ? (
            <div className="flex flex-col items-center gap-4 py-16">
              <Message type="error">预览已过期或服务已重启，请重新选择文件。</Message>
              <Button onClick={startOverFromPreview}>重新选择文件</Button>
            </div>
          ) : !previewDetail ? (
            <Loading text="正在加载预览状态…" />
          ) : (
            <BrowserMaskingPreviewDialog
              detail={previewDetail}
              previewId={previewId}
              onConfirm={() => void confirmPreview()}
              onCancel={() => void cancelPreview()}
              confirming={confirming}
              cancelling={cancelling}
              confirmError={confirmError}
              cancelError={cancelError}
              elapsedMs={elapsedMs}
              lastUpdatedAt={lastUpdatedAt}
            />
          )}
        </div>
      </div>
    );
  }

  return (
    <div className="flex flex-col h-full">
      <PageHeader
        title="文件处理"
        description="通过浏览器提交多文件批量脱敏（由本机企业 Runtime 处理）"
        actions={
          <Button
            onClick={() => void generatePreview()}
            disabled={!canSubmit}
            icon={UploadCloud}
          >
            {submitting
              ? submitPhase === "uploading"
                ? "正在提交到本机 Runtime…"
                : "已提交，正在等待响应…"
              : `生成脱敏预览${queue.length > 0 ? `（${queue.length}）` : ""}`}
          </Button>
        }
      />
      <div className="flex-1 overflow-auto p-6">
        <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
          <div className="lg:col-span-2 space-y-4">
            {(submitting || submitPhase === "error") && (
              <div className="text-sm text-gray-600">
                {submitPhase === "uploading" && (
                  <span className="inline-flex items-center gap-2">
                    <Loading size="sm" />
                    正在把文件上传到本机 Runtime，已等待 {Math.floor(elapsedMs / 1000)} 秒…
                  </span>
                )}
                {submitPhase === "submitted" && (
                  <span className="inline-flex items-center gap-2 text-blue-700">
                    <Info className="w-4 h-4" />
                    已提交，Runtime 正在生成预览…
                  </span>
                )}
                {submitPhase === "error" && submitError && (
                  <Message type="error" onClose={() => { setSubmitError(null); setSubmitPhase("idle"); }}>
                    {submitError}
                  </Message>
                )}
              </div>
            )}
            {submitError && submitPhase !== "error" && <Message type="error" onClose={() => setSubmitError(null)}>{submitError}</Message>}
            {addWarning && <Message type="warning" onClose={() => setAddWarning(null)}>{addWarning}</Message>}

            <label
              className="flex flex-col items-center justify-center gap-2 border-2 border-dashed border-gray-300 rounded-xl p-10 text-center cursor-pointer hover:border-primary/60 transition-colors"
              onDrop={handleDrop}
              onDragOver={handleDragOver}
            >
              <input
                type="file"
                multiple
                accept={runtimeAcceptAttribute}
                className="sr-only"
                onChange={handleFileInput}
                disabled={submitting}
                aria-label="选择需要脱敏的文件"
              />
              <UploadCloud className="w-8 h-8 text-gray-400" />
              <div className="font-medium text-gray-700">
                点击选择或拖放 TXT / Markdown / CSV / Excel / DOCX / PPT / PPTX / PDF
              </div>
              <div className="text-xs text-gray-500">文件仅发送到本机 Runtime；单批最多 100 个文件</div>
            </label>

            {queue.length > 0 && (
              <div className="space-y-2">
                <p className="text-sm font-medium text-gray-700">
                  文件队列（{queue.length}，合计 {readableSize(totalBytes)}）
                </p>
                {queue.map((item) => (
                  <div
                    key={item.key}
                    className="flex items-center justify-between gap-3 border border-gray-200 rounded-lg px-4 py-2"
                  >
                    <div className="min-w-0 flex-1">
                      <div className="truncate text-sm font-medium text-gray-900">
                        {safeDisplayName(item.file.name)}
                      </div>
                      <div className="text-xs text-gray-500">
                        {runtimeFormatLabel(runtimeInputFormatFromFilename(item.file.name) ?? "")} ·{" "}
                        {readableSize(item.file.size)}
                      </div>
                    </div>
                    <Button
                      variant="icon"
                      size="sm"
                      disabled={submitting}
                      onClick={() => removeFile(item.key)}
                      aria-label={`移除 ${safeDisplayName(item.file.name)}`}
                    >
                      <Trash2 className="w-4 h-4" />
                    </Button>
                  </div>
                ))}
              </div>
            )}
          </div>

          <div className="space-y-4">
            <Card className="p-4">
              <div className="flex items-center justify-between mb-3">
                <h3 className="text-sm font-semibold text-gray-900">脱敏规则</h3>
                <span className="text-xs text-gray-500">来自 Runtime</span>
              </div>
              {rulesState.kind === "loading" && <Loading size="sm" text="正在加载规则…" />}
              {rulesState.kind === "error" && (
                <div className="space-y-2">
                  <div className="flex items-start gap-2 text-sm text-red-600">
                    <AlertTriangle className="w-4 h-4 mt-0.5 flex-shrink-0" />
                    <span>{rulesState.message}</span>
                  </div>
                  <Button variant="secondary" size="sm" onClick={() => void loadRules()}>
                    重试
                  </Button>
                </div>
              )}
              {rulesState.kind === "ready" && (
                <div className="space-y-2">
                  {rulesState.rules
                    .filter((rule) => rule.id !== SENSITIVE_TERMS_RULE_ID)
                    .map((rule) => (
                      <label key={rule.id} className="flex items-center gap-2 text-sm">
                        <input
                          type="checkbox"
                          checked={selectedRuleIds.has(rule.id)}
                          onChange={() => toggleRule(rule.id)}
                        />
                        <span className="text-gray-900">{rule.name}</span>
                        <span className="text-gray-400 text-xs">{rule.id}</span>
                      </label>
                    ))}
                  {rulesState.rules.every((rule) => rule.id === SENSITIVE_TERMS_RULE_ID) && (
                    <p className="text-sm text-gray-500">Runtime 未返回任何可用规则。</p>
                  )}
                  {selectedRuleIds.size === 0 && rulesState.rules.length > 0 && (
                    <p className="text-xs text-amber-600">请至少选择一项规则后再提交。</p>
                  )}

                  {/* 敏感词库：与内置规则不同，不显示勾选开关——沿用桌面
                      RuleSelector 的"仅启用词条自动生效"产品语义（7.2）。 */}
                  <div className="border-t border-gray-100 pt-2 mt-2">
                    {sensitiveTermsSummary.kind === "loading" && (
                      <div className="flex items-center justify-between mb-1 bg-gray-50 px-2 py-1.5 rounded">
                        <span className="text-sm text-gray-600">敏感词库（加载中…）</span>
                      </div>
                    )}
                    {sensitiveTermsSummary.kind === "error" && (
                      <div className="space-y-1.5 bg-red-50 border border-red-100 px-3 py-2.5 rounded-lg">
                        <div className="flex items-start gap-2 text-sm text-red-600">
                          <AlertTriangle className="w-4 h-4 mt-0.5 flex-shrink-0" />
                          <span>敏感词库状态加载失败，请稍后重试。</span>
                        </div>
                        <Button variant="secondary" size="sm" onClick={() => void loadSensitiveTermsSummary()}>
                          重试
                        </Button>
                      </div>
                    )}
                    {sensitiveTermsSummary.kind === "ready" && sensitiveTermsSummary.enabledCount === 0 && (
                      <div className="bg-blue-50 border border-blue-200 px-3 py-2.5 rounded-lg">
                        <div className="flex items-start gap-2 mb-2">
                          <Lightbulb className="w-4 h-4 text-blue-600 flex-shrink-0 mt-0.5" />
                          <p className="text-sm font-medium text-blue-900">当前未配置已启用敏感词</p>
                        </div>
                        <button
                          type="button"
                          onClick={() => navigate("/sensitive-terms")}
                          className="w-full text-xs bg-blue-600 hover:bg-blue-700 text-white px-3 py-1.5 rounded transition-colors"
                        >
                          前往配置敏感词库 →
                        </button>
                      </div>
                    )}
                    {sensitiveTermsSummary.kind === "ready" && sensitiveTermsSummary.enabledCount > 0 && (
                      <div className="flex items-center justify-between mb-1 bg-blue-50 px-2 py-1.5 rounded">
                        <div className="flex items-center gap-1.5">
                          <span className="text-sm text-blue-900">敏感词库</span>
                          <span className="text-xs px-1 py-0 bg-blue-100 rounded text-blue-700">自动启用</span>
                          <span className="text-xs px-1 py-0 bg-blue-100 rounded text-blue-700">
                            {sensitiveTermsSummary.enabledCount} 个词条
                          </span>
                        </div>
                        <button
                          type="button"
                          onClick={() => navigate("/sensitive-terms")}
                          className="text-xs text-blue-600 hover:text-blue-800 hover:underline"
                        >
                          管理
                        </button>
                      </div>
                    )}
                  </div>
                </div>
              )}
            </Card>

            <div className="p-4 bg-blue-50/60 border border-blue-100 rounded-xl text-xs text-blue-700 space-y-1">
              <div className="flex items-center gap-2 mb-1">
                <Info className="w-4 h-4 text-blue-900" />
                <h3 className="text-sm font-bold text-blue-900">说明</h3>
              </div>
              <p>处理完成后可在批次详情页下载脱敏后的 Markdown；映射数据只保存在服务器内部，不提供下载。</p>
              <p>扫描件 PDF 的文字识别由服务器管理员统一配置的 OCR 组件提供。</p>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
