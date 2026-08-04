import { useEffect, useState } from "react";
import { useNavigate, useSearchParams } from "react-router-dom";
import { CheckCircle2, ChevronLeft, Unlock, WifiOff } from "lucide-react";
import { PageHeader } from "@/components/layout/PageHeader";
import { Badge, Button, Card, Loading, Message } from "@/components/ui/cheersai-ui";
import { fetchRuntimeBatch, fetchRuntimeBatches, restoreRuntimeArtifact } from "@/lib/runtime/client";
import { runtimeFormatLabel } from "@/lib/runtime/formatCatalog";
import type { RuntimeBatchDetail, RuntimeBatchFile, RuntimeBatchSummary } from "@/types/runtime";

/** 安全展示用的文件名/ID：只做控制字符清理，不解释为路径。 */
function safeDisplayName(name: string): string {
  // eslint-disable-next-line no-control-regex
  return name.replace(/[\x00-\x1f\x7f]/g, "");
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

function restorabilityReason(file: RuntimeBatchFile): string | null {
  if (file.status !== "Completed") {
    return "文件尚未处理完成，暂不可反脱敏。";
  }
  if (!file.artifact_id) {
    return "该文件没有可用的处理产物。";
  }
  if (!file.restore_available) {
    return "该文件暂无可用的服务器映射，无法反脱敏。";
  }
  return null;
}

/** 把恢复失败的服务端错误码映射为固定安全文案，不回显原始响应。 */
function restoreErrorMessage(reason: string, code?: string, message?: string): string {
  if (reason === "network") {
    return "无法连接本机 Runtime，请确认服务已启动后重试。";
  }
  if (reason === "invalid-count") {
    return "恢复结果异常，未生成任何文件，请重试或联系管理员。";
  }
  switch (code) {
    case "CMAP_MISMATCH":
      return "无法恢复：服务器映射数据无效或与该文件不匹配。";
    case "NOT_FOUND":
      return "该产物已不存在或不可恢复，请返回重新选择。";
    case "INPUT_CORRUPTED":
      return "服务器保存的处理结果已损坏，无法恢复。";
    default:
      return message ?? "恢复失败，请稍后重试。";
  }
}

type BatchListState =
  | { kind: "loading" }
  | { kind: "ready"; batches: RuntimeBatchSummary[] }
  | { kind: "error"; message: string };

type DetailState =
  | { kind: "loading" }
  | { kind: "ready"; detail: RuntimeBatchDetail }
  | { kind: "error"; message: string };

type RestoreState =
  | { kind: "idle" }
  | { kind: "restoring" }
  | { kind: "success"; count: number }
  | { kind: "error"; message: string };

export default function FileUnmaskBrowser() {
  const navigate = useNavigate();
  const [searchParams, setSearchParams] = useSearchParams();
  const batchId = searchParams.get("batch_id");
  const artifactId = searchParams.get("artifact_id");

  const [batchListState, setBatchListState] = useState<BatchListState>({ kind: "loading" });
  const [detailState, setDetailState] = useState<DetailState>({ kind: "loading" });
  const [restoreState, setRestoreState] = useState<RestoreState>({ kind: "idle" });

  useEffect(() => {
    if (batchId) return;
    let cancelled = false;
    setBatchListState({ kind: "loading" });
    void fetchRuntimeBatches().then((result) => {
      if (cancelled) return;
      if (!result.ok) {
        setBatchListState({
          kind: "error",
          message:
            result.reason === "http"
              ? result.message ?? "批次列表加载失败，请稍后重试。"
              : "无法连接本机 Runtime，请确认服务已启动后重试。",
        });
        return;
      }
      setBatchListState({ kind: "ready", batches: result.data.batches });
    });
    return () => {
      cancelled = true;
    };
  }, [batchId]);

  useEffect(() => {
    if (!batchId) return;
    let cancelled = false;
    setDetailState({ kind: "loading" });
    setRestoreState({ kind: "idle" });
    void fetchRuntimeBatch(batchId).then((result) => {
      if (cancelled) return;
      if (!result.ok) {
        setDetailState({
          kind: "error",
          message:
            result.reason === "http"
              ? result.status === 404
                ? "该批次不存在，请返回重新选择。"
                : result.message ?? "批次详情加载失败，请稍后重试。"
              : "无法连接本机 Runtime，请确认服务已启动后重试。",
        });
        return;
      }
      setDetailState({ kind: "ready", detail: result.data });
    });
    return () => {
      cancelled = true;
    };
    // batchId 变化即视为新一轮候选导航，需要重新验证，因此不依赖 artifactId。
  }, [batchId]);

  const selectBatch = (id: string) => {
    setSearchParams({ batch_id: id });
  };

  const selectFile = (id: string, fileArtifactId: string) => {
    setSearchParams({ batch_id: id, artifact_id: fileArtifactId });
  };

  const backToBatchList = () => {
    setSearchParams({});
  };

  const backToFileList = () => {
    if (batchId) setSearchParams({ batch_id: batchId });
  };

  const startRestore = async (file: RuntimeBatchFile) => {
    if (!file.artifact_id || restoreState.kind === "restoring") return;
    setRestoreState({ kind: "restoring" });
    const result = await restoreRuntimeArtifact(file.artifact_id, file.display_name);
    if (!result.ok) {
      setRestoreState({
        kind: "error",
        message: restoreErrorMessage(
          result.reason,
          "code" in result ? result.code : undefined,
          "message" in result ? result.message : undefined
        ),
      });
      return;
    }
    setRestoreState({ kind: "success", count: result.count });
  };

  // 步骤一：未指定 batch_id，展示批次选择列表（只做单次汇总请求，不逐批详情预取）。
  if (!batchId) {
    return (
      <div className="flex flex-col h-full">
        <PageHeader
          title="反脱敏"
          description="选择一个已完成的服务器批次，从中挑选可恢复的文件；服务器内部映射不会离开 Runtime。"
        />
        <div className="flex-1 overflow-auto p-6 space-y-4">
          {batchListState.kind === "loading" && <Loading text="正在加载批次列表…" />}
          {batchListState.kind === "error" && (
            <Message type="error">
              <div className="flex items-center justify-between gap-4">
                <span className="inline-flex items-center gap-2">
                  <WifiOff className="w-4 h-4" />
                  {batchListState.message}
                </span>
              </div>
            </Message>
          )}
          {batchListState.kind === "ready" && (
            <Card className="overflow-hidden">
              {batchListState.batches.length === 0 ? (
                <div className="p-10 text-center text-gray-500">还没有批次，请先在「文件脱敏」提交批次。</div>
              ) : (
                <table className="w-full text-sm">
                  <thead className="bg-gray-50 text-left text-gray-500">
                    <tr>
                      <th className="px-4 py-2">批次 ID</th>
                      <th className="px-4 py-2">状态</th>
                      <th className="px-4 py-2">完成/总数</th>
                      <th className="px-4 py-2" />
                    </tr>
                  </thead>
                  <tbody className="divide-y divide-gray-100">
                    {batchListState.batches.map((batch) => (
                      <tr key={batch.batch_id} className="hover:bg-gray-50">
                        <td className="px-4 py-3 font-mono text-xs text-gray-700" title={batch.batch_id}>
                          {batch.batch_id.slice(0, 8)}
                        </td>
                        <td className="px-4 py-3">
                          <Badge variant={batch.status === "Completed" ? "success" : "info"}>
                            {batchStatusLabel(batch.status)}
                          </Badge>
                        </td>
                        <td className="px-4 py-3">
                          {batch.completed_count}/{batch.file_count}
                        </td>
                        <td className="px-4 py-3">
                          <Button variant="secondary" size="sm" onClick={() => selectBatch(batch.batch_id)}>
                            选择
                          </Button>
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              )}
            </Card>
          )}
        </div>
      </div>
    );
  }

  if (detailState.kind === "loading") {
    return (
      <div className="flex flex-col h-full">
        <PageHeader title="反脱敏" description="正在验证所选批次与文件…" />
        <div className="flex-1 overflow-auto p-6">
          <Loading text="正在验证…" />
        </div>
      </div>
    );
  }

  if (detailState.kind === "error") {
    return (
      <div className="flex flex-col h-full">
        <PageHeader
          title="反脱敏"
          actions={
            <Button variant="secondary" size="sm" icon={ChevronLeft} onClick={backToBatchList}>
              返回批次列表
            </Button>
          }
        />
        <div className="flex-1 overflow-auto p-6">
          <Message type="error">{detailState.message}</Message>
        </div>
      </div>
    );
  }

  const { detail } = detailState;

  // 步骤二：已选批次但未选文件（或 query 中的 artifact_id 未通过验证）——
  // 展示该批次内的文件供选择，只允许挑选校验通过的可恢复文件。
  const selectedFile = artifactId
    ? detail.files.find((file) => file.artifact_id === artifactId)
    : undefined;
  const verified = Boolean(
    selectedFile &&
      selectedFile.artifact_id === artifactId &&
      selectedFile.status === "Completed" &&
      selectedFile.restore_available === true
  );

  if (!artifactId || !verified) {
    return (
      <div className="flex flex-col h-full">
        <PageHeader
          title="反脱敏"
          description={`批次 ${detail.batch.batch_id.slice(0, 8)}：选择一个可恢复的文件`}
          actions={
            <Button variant="secondary" size="sm" icon={ChevronLeft} onClick={backToBatchList}>
              返回批次列表
            </Button>
          }
        />
        <div className="flex-1 overflow-auto p-6 space-y-4">
          {artifactId && !verified && (
            <Message type="warning">
              指定的文件未通过校验，无法直接恢复：
              {selectedFile ? restorabilityReason(selectedFile) : "该文件不存在于此批次中，请重新选择。"}
            </Message>
          )}
          <Card className="overflow-hidden">
            <table className="w-full text-sm">
              <thead className="bg-gray-50 text-left text-gray-500">
                <tr>
                  <th className="px-4 py-2">文件</th>
                  <th className="px-4 py-2">格式</th>
                  <th className="px-4 py-2">状态</th>
                  <th className="px-4 py-2" />
                </tr>
              </thead>
              <tbody className="divide-y divide-gray-100">
                {detail.files.map((file) => {
                  const reason = restorabilityReason(file);
                  return (
                    <tr key={file.file_id}>
                      <td className="px-4 py-3">
                        <div className="font-medium text-gray-900 break-all">
                          {safeDisplayName(file.display_name)}
                        </div>
                        {reason && <div className="mt-1 text-xs text-gray-400">{reason}</div>}
                      </td>
                      <td className="px-4 py-3">{runtimeFormatLabel(file.input_format)}</td>
                      <td className="px-4 py-3">
                        <Badge variant={file.status === "Completed" ? "success" : "info"}>
                          {file.status === "Completed" ? "已完成" : file.status}
                        </Badge>
                      </td>
                      <td className="px-4 py-3">
                        <Button
                          variant="secondary"
                          size="sm"
                          disabled={reason !== null}
                          onClick={() => file.artifact_id && selectFile(detail.batch.batch_id, file.artifact_id)}
                        >
                          选择
                        </Button>
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </Card>
        </div>
      </div>
    );
  }

  // 步骤三：已校验通过的文件，等待用户明确点击后才发起恢复请求。
  const file = selectedFile as RuntimeBatchFile;

  return (
    <div className="flex flex-col h-full">
      <PageHeader
        title="反脱敏"
        description="服务器使用内部映射恢复原始内容；本页不显示、不上传、不下载映射或口令。"
        actions={
          <Button variant="secondary" size="sm" icon={ChevronLeft} onClick={backToFileList}>
            返回文件列表
          </Button>
        }
      />
      <div className="flex-1 overflow-auto p-6">
        <div className="max-w-2xl mx-auto space-y-6">
          <Card className="p-5">
            <div className="flex items-center gap-3 mb-2">
              <Unlock className="w-5 h-5 text-primary" />
              <h2 className="text-base font-semibold text-gray-900">待恢复文件</h2>
            </div>
            <p className="text-sm text-gray-700 break-all">{safeDisplayName(file.display_name)}</p>
            <p className="mt-1 text-xs text-gray-500">
              格式：{runtimeFormatLabel(file.input_format)} · 脱敏实体数：{file.masked_entity_count ?? "—"}
            </p>
          </Card>

          {restoreState.kind === "error" && <Message type="error">{restoreState.message}</Message>}

          {restoreState.kind === "success" ? (
            <Card className="p-5 border-green-200 bg-green-50">
              <div className="flex items-start gap-3">
                <CheckCircle2 className="w-5 h-5 text-green-600 flex-shrink-0 mt-0.5" />
                <div>
                  <p className="text-sm font-medium text-green-900">反脱敏成功</p>
                  <p className="text-sm text-green-700 mt-1">已恢复 {restoreState.count} 处，已开始下载。</p>
                </div>
              </div>
            </Card>
          ) : (
            <Button
              variant="primary"
              size="lg"
              className="w-full"
              icon={Unlock}
              loading={restoreState.kind === "restoring"}
              onClick={() => void startRestore(file)}
            >
              {restoreState.kind === "restoring" ? "正在恢复…" : "开始反脱敏"}
            </Button>
          )}

          <Button variant="secondary" size="sm" onClick={() => navigate("/files")}>
            返回文件管理
          </Button>
        </div>
      </div>
    </div>
  );
}
