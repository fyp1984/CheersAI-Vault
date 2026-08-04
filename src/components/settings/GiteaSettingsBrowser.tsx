import { useCallback, useEffect, useState } from "react";
import { RefreshCw } from "lucide-react";
import { PageHeader } from "@/components/layout/PageHeader";
import { Button, Message, Badge, Card, Loading } from "@/components/ui/cheersai-ui";
import {
  createRuntimeFileBayRepository,
  fetchRuntimeFileBayStatus,
  testRuntimeFileBayConnection,
} from "@/lib/runtime/client";
import type { RuntimeFetchResult } from "@/lib/runtime/client";
import type { RuntimeFileBayStatusResponse } from "@/types/runtime";

type StatusState =
  | { kind: "loading" }
  | { kind: "ready"; data: RuntimeFileBayStatusResponse }
  | { kind: "error"; message: string };

type ActionState =
  | { kind: "idle" }
  | { kind: "busy" }
  | { kind: "success"; message: string }
  | { kind: "error"; message: string };

/** 区分“连不上本机 Runtime” 与业务错误，从不把两者混为一谈。 */
function describeFailure(result: Extract<RuntimeFetchResult<unknown>, { ok: false }>): string {
  if (result.reason === "network") return "无法连接本机 Runtime，请确认服务已启动后重试。";
  if (result.reason === "http") return result.message ?? "请求失败，请稍后重试。";
  return "响应异常，请稍后重试。";
}

function statusBadge(status: RuntimeFileBayStatusResponse["status"]) {
  switch (status) {
    case "configured":
      return <Badge variant="success">已配置</Badge>;
    case "invalid":
      return <Badge variant="error">配置无效</Badge>;
    default:
      return <Badge variant="neutral">未配置</Badge>;
  }
}

/**
 * `/gitea` 路由的浏览器实现。FileBay 的 URL、Token、owner、repo 只由服务器
 * 管理员环境变量提供——本组件从不展示 Token 字段，也不提供可编辑的地址/
 * owner/repo 输入；只展示 Runtime 返回的安全状态，并可触发三个显式、
 * 用户主动发起的动作：刷新状态（不出站）、测试连接、创建私有仓库（需二次
 * 确认目标地址与仓库名）。
 */
export default function GiteaSettingsBrowser() {
  const [statusState, setStatusState] = useState<StatusState>({ kind: "loading" });
  const [testState, setTestState] = useState<ActionState>({ kind: "idle" });
  const [createState, setCreateState] = useState<ActionState>({ kind: "idle" });
  const [confirmingCreate, setConfirmingCreate] = useState(false);

  const loadStatus = useCallback(async () => {
    setStatusState({ kind: "loading" });
    const result = await fetchRuntimeFileBayStatus();
    if (!result.ok) {
      setStatusState({ kind: "error", message: describeFailure(result) });
      return;
    }
    setStatusState({ kind: "ready", data: result.data });
  }, []);

  useEffect(() => {
    void loadStatus();
  }, [loadStatus]);

  const handleTest = async () => {
    setTestState({ kind: "busy" });
    const result = await testRuntimeFileBayConnection();
    if (!result.ok) {
      setTestState({ kind: "error", message: describeFailure(result) });
      return;
    }
    setTestState({
      kind: "success",
      message: result.data.repository_exists
        ? "连接成功：目标仓库已存在。"
        : "连接成功：目标仓库尚不存在，可点击“创建私有仓库”。",
    });
  };

  const handleCreate = async () => {
    setConfirmingCreate(false);
    setCreateState({ kind: "busy" });
    const result = await createRuntimeFileBayRepository();
    if (!result.ok) {
      setCreateState({ kind: "error", message: describeFailure(result) });
      return;
    }
    setCreateState({
      kind: "success",
      message: result.data.status === "created" ? "私有仓库已创建。" : "仓库已存在，无需重复创建。",
    });
    void loadStatus();
  };

  if (statusState.kind === "loading") {
    return (
      <div className="flex flex-col h-full">
        <PageHeader title="FileBay 上传设置" description="由服务器管理员配置，浏览器只显示安全状态。" />
        <div className="flex-1 flex items-center justify-center">
          <Loading text="正在加载状态…" />
        </div>
      </div>
    );
  }

  if (statusState.kind === "error") {
    return (
      <div className="flex flex-col h-full">
        <PageHeader title="FileBay 上传设置" description="由服务器管理员配置，浏览器只显示安全状态。" />
        <div className="flex-1 overflow-auto p-6">
          <Message type="error">
            <div className="flex items-center justify-between gap-4">
              <span>{statusState.message}</span>
              <Button variant="secondary" size="sm" icon={RefreshCw} onClick={() => void loadStatus()}>
                重试
              </Button>
            </div>
          </Message>
        </div>
      </div>
    );
  }

  const { data } = statusState;

  return (
    <div className="flex flex-col h-full">
      <PageHeader
        title="FileBay 上传设置"
        description="URL、Token、owner、repo 均由服务器管理员环境变量配置；浏览器仅可查看安全状态并触发测试连接、创建私有仓库。"
        actions={
          <Button variant="secondary" size="sm" icon={RefreshCw} onClick={() => void loadStatus()}>
            刷新
          </Button>
        }
      />
      <div className="flex-1 overflow-auto p-6 space-y-4">
        <Card className="p-5 space-y-4">
          <div>
            <div className="text-sm text-gray-500">配置状态</div>
            <div className="mt-1">{statusBadge(data.status)}</div>
          </div>

          {data.status === "unconfigured" && (
            <Message type="info">
              尚未配置 FileBay。请联系服务器管理员在 Runtime 环境变量中设置{" "}
              <code>VAULT_FILEBAY_URL</code>/<code>VAULT_FILEBAY_TOKEN</code>/
              <code>VAULT_FILEBAY_OWNER</code>/<code>VAULT_FILEBAY_REPO</code> 后重启服务。
            </Message>
          )}
          {data.status === "invalid" && (
            <Message type="error">
              当前 FileBay 配置无效（可能未同时设置全部四项，或格式不合法）。上传功能已禁用，请联系服务器管理员检查配置后重启服务。
            </Message>
          )}

          {data.status !== "unconfigured" && (
            <div className="grid grid-cols-2 gap-4 text-sm">
              <div>
                <div className="text-gray-500">目标地址</div>
                <div className="mt-0.5 font-mono text-gray-900 break-all">{data.target_origin ?? "—"}</div>
              </div>
              <div>
                <div className="text-gray-500">仓库</div>
                <div className="mt-0.5 font-mono text-gray-900 break-all">
                  {data.owner && data.repo ? `${data.owner}/${data.repo}` : "—"}
                </div>
              </div>
              <div>
                <div className="text-gray-500">访问令牌</div>
                <div className="mt-0.5 text-gray-900">{data.has_token ? "已配置" : "未配置"}</div>
              </div>
            </div>
          )}

          {data.status === "configured" && (
            <div className="flex flex-wrap items-center gap-3 pt-2 border-t border-gray-100">
              <Button
                variant="secondary"
                size="sm"
                loading={testState.kind === "busy"}
                onClick={() => void handleTest()}
              >
                测试连接
              </Button>
              <Button
                variant="secondary"
                size="sm"
                loading={createState.kind === "busy"}
                onClick={() => setConfirmingCreate(true)}
              >
                创建私有仓库
              </Button>
            </div>
          )}

          {testState.kind === "success" && (
            <Message type="success" onClose={() => setTestState({ kind: "idle" })}>
              {testState.message}
            </Message>
          )}
          {testState.kind === "error" && (
            <Message type="error" onClose={() => setTestState({ kind: "idle" })}>
              {testState.message}
            </Message>
          )}
          {createState.kind === "success" && (
            <Message type="success" onClose={() => setCreateState({ kind: "idle" })}>
              {createState.message}
            </Message>
          )}
          {createState.kind === "error" && (
            <Message type="error" onClose={() => setCreateState({ kind: "idle" })}>
              {createState.message}
            </Message>
          )}
        </Card>

        <Card className="p-5">
          <h3 className="text-sm font-semibold text-gray-900 mb-2">说明</h3>
          <ul className="text-sm text-gray-600 space-y-1 list-disc list-inside">
            <li>浏览器不显示、不接收 Token，也没有可编辑的地址/owner/repo 输入。</li>
            <li>上传时只提交已完成脱敏的 Markdown 制品 ID，远程路径由服务器生成，不上传原文件、映射文件或还原产物。</li>
            <li>“创建私有仓库”只会创建私有仓库，不会创建公开仓库。</li>
          </ul>
        </Card>
      </div>

      {confirmingCreate && data.status === "configured" && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40 p-4">
          <Card className="w-full max-w-md p-6 space-y-4">
            <h3 className="text-base font-semibold text-gray-900">确认创建私有仓库</h3>
            <p className="text-sm text-gray-600">
              将在 <span className="font-mono">{data.target_origin}</span> 上为{" "}
              <span className="font-mono">
                {data.owner}/{data.repo}
              </span>{" "}
              创建一个<strong>私有</strong>仓库（如已存在则不重复创建）。确认继续？
            </p>
            <div className="flex justify-end gap-3">
              <Button variant="secondary" size="sm" onClick={() => setConfirmingCreate(false)}>
                取消
              </Button>
              <Button size="sm" onClick={() => void handleCreate()}>
                确认创建
              </Button>
            </div>
          </Card>
        </div>
      )}
    </div>
  );
}
