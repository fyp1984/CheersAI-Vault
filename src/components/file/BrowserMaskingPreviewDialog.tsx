import { useEffect, useState } from "react";
import { CheckCircle2, Eye, RotateCcw } from "lucide-react";
import { Badge, Button, Card, Loading, Message } from "@/components/ui/cheersai-ui";
import { fetchRuntimePreviewFileContent } from "@/lib/runtime/client";
import { runtimeFormatLabel } from "@/lib/runtime/formatCatalog";
import type { RuntimePreviewDetail, RuntimePreviewFileStatus } from "@/types/runtime";

/**
 * 浏览器两阶段预览的展示组件——只承载 `RuntimePreviewDetail` 中已经安全的
 * 字段与按需拉取的脱敏 Markdown 正文，不引入桌面端 `MaskingPreviewDialog`
 * 的 `original_rows`/映射/手动查找替换。内容仅在用户主动选中某个 Ready
 * 文件时才请求，不做预取；切换文件使用 `cancelled` 标记防止旧请求覆盖新
 * 选择（F4）。
 */

function fileStatusVariant(status: RuntimePreviewFileStatus): "success" | "warning" | "error" | "info" {
  switch (status) {
    case "Ready":
      return "success";
    case "Failed":
      return "error";
    case "Processing":
      return "info";
    default:
      return "warning";
  }
}

function fileStatusLabel(status: RuntimePreviewFileStatus): string {
  switch (status) {
    case "Pending":
      return "等待中";
    case "Processing":
      return "处理中";
    case "Ready":
      return "已就绪";
    case "Failed":
      return "失败";
    default:
      return status;
  }
}

function sessionStatusLabel(status: RuntimePreviewDetail["status"]): string {
  switch (status) {
    case "Processing":
      return "正在生成预览…";
    case "Ready":
      return "预览已就绪";
    case "ReadyWithErrors":
      return "预览已就绪（部分文件失败）";
    case "Failed":
      return "预览生成失败";
    case "Confirming":
      return "正在确认…";
    case "Confirmed":
      return "已确认";
    default:
      return status;
  }
}

function sessionStatusVariant(status: RuntimePreviewDetail["status"]): "success" | "warning" | "error" | "info" {
  switch (status) {
    case "Ready":
    case "Confirmed":
      return "success";
    case "ReadyWithErrors":
      return "warning";
    case "Failed":
      return "error";
    default:
      return "info";
  }
}

/** 展示用文件名清洗：只做控制字符处理，不解释为路径。 */
function safeDisplayName(name: string): string {
  // eslint-disable-next-line no-control-regex
  return name.replace(/[\x00-\x1f\x7f]/g, "");
}

type ContentState =
  | { kind: "idle" }
  | { kind: "loading" }
  | { kind: "ready"; text: string }
  | { kind: "error"; message: string };

interface BrowserMaskingPreviewDialogProps {
  detail: RuntimePreviewDetail;
  previewId: string;
  onConfirm: () => void;
  onCancel: () => void;
  confirming: boolean;
  cancelling: boolean;
  confirmError: string | null;
  cancelError: string | null;
  /** 当前 preview 已持续等待的毫秒数（由父组件计时器提供）。 */
  elapsedMs?: number;
  /** 最近一次成功从 Runtime 获取状态的时间戳（ms）。 */
  lastUpdatedAt?: number | null;
}

export function BrowserMaskingPreviewDialog({
  detail,
  previewId,
  onConfirm,
  onCancel,
  confirming,
  cancelling,
  confirmError,
  cancelError,
  elapsedMs = 0,
  lastUpdatedAt,
}: BrowserMaskingPreviewDialogProps) {
  const [selectedFileId, setSelectedFileId] = useState<string | null>(null);
  const [content, setContent] = useState<ContentState>({ kind: "idle" });

  useEffect(() => {
    if (!selectedFileId) {
      setContent({ kind: "idle" });
      return;
    }
    const file = detail.files.find((item) => item.file_id === selectedFileId);
    if (!file || file.status !== "Ready") {
      setContent({ kind: "idle" });
      return;
    }

    let cancelled = false;
    setContent({ kind: "loading" });
    void (async () => {
      const result = await fetchRuntimePreviewFileContent(previewId, selectedFileId);
      if (cancelled) return;
      if (!result.ok) {
        setContent({
          kind: "error",
          message:
            result.reason === "http"
              ? result.message ?? "内容加载失败，请稍后重试。"
              : "无法连接本机 Runtime，请确认服务已启动后重试。",
        });
        return;
      }
      setContent({ kind: "ready", text: result.text });
    })();

    return () => {
      cancelled = true;
    };
  }, [previewId, selectedFileId, detail.files]);

  const readyFiles = detail.files.filter((file) => file.status === "Ready");
  const canConfirm =
    detail.ready_count > 0 &&
    !confirming &&
    !cancelling &&
    (detail.status === "Ready" || detail.status === "ReadyWithErrors");

  return (
    <div className="space-y-4">
      <Card className="p-5">
        <div className="flex items-center justify-between flex-wrap gap-3">
          <div>
            <div className="text-sm text-gray-500">预览状态</div>
            <div className="mt-1">
              <Badge variant={sessionStatusVariant(detail.status)}>{sessionStatusLabel(detail.status)}</Badge>
            </div>
          </div>
          <div className="grid grid-cols-4 gap-6 text-center">
            <div>
              <div className="text-2xl font-bold text-gray-900">{detail.file_count}</div>
              <div className="text-xs text-gray-500">总文件</div>
            </div>
            <div>
              <div className="text-2xl font-bold text-green-600">{detail.ready_count}</div>
              <div className="text-xs text-gray-500">已就绪</div>
            </div>
            <div>
              <div className="text-2xl font-bold text-red-600">{detail.failed_count}</div>
              <div className="text-xs text-gray-500">失败</div>
            </div>
            <div>
              <div className="text-2xl font-bold text-blue-600">{detail.masked_entity_count}</div>
              <div className="text-xs text-gray-500">脱敏实体数</div>
            </div>
          </div>
        </div>
      </Card>

      {confirmError && <Message type="error">{confirmError}</Message>}
      {cancelError && <Message type="error">{cancelError}</Message>}
      {detail.status === "Failed" && (
        <Message type="error">全部文件处理失败，无法确认为正式批次；请取消预览并调整文件或规则后重试。</Message>
      )}
      {detail.status === "Processing" && (
        <Message type="info">
          <span className="inline-flex items-center gap-2">
            <span className="relative flex h-2 w-2">
              <span className="animate-ping absolute inline-flex h-full w-full rounded-full bg-blue-400 opacity-75" />
              <span className="relative inline-flex rounded-full h-2 w-2 bg-blue-500" />
            </span>
            Runtime 正在处理，已等待 {Math.floor(elapsedMs / 1000)} 秒
            {lastUpdatedAt ? `，最近更新 ${Math.floor((Date.now() - lastUpdatedAt) / 1000)} 秒前` : ""}。
            扫描件 PDF 的文字识别通常需要几十秒至数分钟，请保持页面打开。
          </span>
        </Message>
      )}

      <div className="grid grid-cols-1 lg:grid-cols-3 gap-4">
        <Card className="lg:col-span-1 p-3 space-y-1 max-h-[480px] overflow-auto">
          {detail.files.map((file) => (
            <button
              key={file.file_id}
              type="button"
              onClick={() => file.status === "Ready" && setSelectedFileId(file.file_id)}
              disabled={file.status !== "Ready"}
              className={`w-full text-left px-3 py-2 rounded-lg text-sm transition-colors border ${
                selectedFileId === file.file_id
                  ? "bg-primary/10 border-primary/40"
                  : "border-transparent hover:bg-gray-50"
              } ${file.status !== "Ready" ? "cursor-not-allowed opacity-70" : "cursor-pointer"}`}
            >
              <div className="flex items-center justify-between gap-2">
                <span className="truncate font-medium text-gray-900">{safeDisplayName(file.display_name)}</span>
                <Badge variant={fileStatusVariant(file.status)}>{fileStatusLabel(file.status)}</Badge>
              </div>
              <div className="mt-1 text-xs text-gray-500">
                {runtimeFormatLabel(file.input_format)}
                {file.status === "Ready" && ` · 脱敏 ${file.masked_entity_count ?? 0} 处`}
              </div>
              {file.status === "Failed" && (
                <div className="mt-1 text-xs text-red-600">
                  <span className="font-mono">{file.error_code ?? "PROCESSING_FAILED"}</span>
                  {file.error_message && <span>：{file.error_message}</span>}
                </div>
              )}
            </button>
          ))}
        </Card>

        <Card className="lg:col-span-2 p-0 overflow-hidden">
          <div className="border-b border-gray-100 px-4 py-2 text-sm font-medium text-gray-700 flex items-center gap-2">
            <Eye className="w-4 h-4 text-gray-400" />
            脱敏后内容预览
          </div>
          <div className="p-4 h-[420px] overflow-auto">
            {!selectedFileId && (
              <p className="text-sm text-gray-500">
                {readyFiles.length > 0 ? "请选择左侧已就绪的文件查看脱敏后的内容。" : "暂无已就绪的文件可供预览。"}
              </p>
            )}
            {selectedFileId && content.kind === "loading" && <Loading size="sm" text="正在加载内容…" />}
            {selectedFileId && content.kind === "error" && <p className="text-sm text-red-600">{content.message}</p>}
            {selectedFileId && content.kind === "ready" && (
              <pre className="whitespace-pre-wrap break-words font-mono text-sm text-gray-800">{content.text}</pre>
            )}
          </div>
        </Card>
      </div>

      <div className="flex items-center justify-end gap-3">
        <Button variant="secondary" icon={RotateCcw} disabled={cancelling || confirming} onClick={onCancel}>
          {cancelling ? "正在取消…" : "取消预览并重新开始"}
        </Button>
        <Button icon={CheckCircle2} disabled={!canConfirm} onClick={onConfirm}>
          {confirming ? "正在确认…" : "确认并生成正式批次"}
        </Button>
      </div>
    </div>
  );
}
