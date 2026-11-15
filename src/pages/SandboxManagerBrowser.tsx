import { useCallback, useEffect, useState } from "react";
import { PageHeader } from "@/components/layout/PageHeader";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Separator } from "@/components/ui/separator";
import { Message } from "@/components/ui/cheersai-ui";
import Toast from "@/components/common/Toast";
import {
  Lock,
  Unlock,
  Shield,
  Eye,
  EyeOff,
  Save,
  RefreshCw,
  Info,
  CheckCircle2,
  AlertTriangle,
  XCircle,
  Users,
} from "lucide-react";
import {
  clearRuntimeSandboxPin,
  fetchRuntimeSandboxStatus,
  lockRuntimeSandbox,
  setRuntimeSandboxPin,
  unlockRuntimeSandbox,
  type RuntimeFetchResult,
} from "@/lib/runtime/client";
import type { RuntimeSandboxStatusResponse } from "@/types/runtime";
import SettingsPrivacySection from "@/components/settings/SettingsPrivacySection";

type LoadState = "loading" | "ready" | "error";
type ToastMessage = { message: string; type: "success" | "error" | "warning" };
const CONNECTION_ERROR_TEXT = "当前连不上本地服务，请确认服务已启动后再试。";
const LOAD_ERROR_TEXT = "暂时无法读取沙箱状态，请稍后再试。";

function describeSandboxFailure(
  result: Extract<RuntimeFetchResult<unknown>, { ok: false }>,
  fallback: string
): string {
  if (result.reason === "network") {
    return CONNECTION_ERROR_TEXT;
  }
  if (result.reason === "http") {
    if (result.code === "SANDBOX_PIN_RATE_LIMITED") {
      return "验证过于频繁，沙箱已临时全局锁定，请稍后再试。";
    }
    if (result.code === "SANDBOX_PIN_INVALID") {
      return "PIN 不正确，请重试。";
    }
    if (result.code === "SANDBOX_PIN_NOT_CONFIGURED") {
      return "尚未设置 PIN。";
    }
    return result.message ?? fallback;
  }
  return fallback;
}

function describeSandboxLoadFailure(
  result: Extract<RuntimeFetchResult<unknown>, { ok: false }>
): string {
  return result.reason === "network" ? CONNECTION_ERROR_TEXT : LOAD_ERROR_TEXT;
}

/**
 * `/sandbox` 的普通浏览器实现：操作的是服务器系统用户共享的一份 PIN/
 * `locked` 状态，所有浏览器会话看到同一结果——这不是账号登录、RBAC、
 * 多租户或 API 鉴权，也不提供服务器目录文本框、系统目录选择器、
 * “打开服务器目录”、默认 mapping 加密口令或“记住口令”（这些仍是桌面
 * `SandboxManagerDesktop` 专属，服务器数据目录由部署管理员通过既有
 * Runtime 配置管理）。PIN 只存在于表单的瞬时内存中，本组件不把它写入
 * console、URL 或任何持久化存储。
 */
export default function SandboxManagerBrowser() {
  const [loadState, setLoadState] = useState<LoadState>("loading");
  const [loadErrorMessage, setLoadErrorMessage] = useState<string | null>(null);
  const [status, setStatus] = useState<RuntimeSandboxStatusResponse | null>(null);
  const [toast, setToast] = useState<ToastMessage | null>(null);

  const [unlockPin, setUnlockPin] = useState("");
  const [showUnlockPin, setShowUnlockPin] = useState(false);
  const [unlocking, setUnlocking] = useState(false);

  const [currentPin, setCurrentPin] = useState("");
  const [newPin, setNewPin] = useState("");
  const [confirmPin, setConfirmPin] = useState("");
  const [showSetPins, setShowSetPins] = useState(false);
  const [savingPin, setSavingPin] = useState(false);

  const [clearingPin, setClearingPin] = useState(false);
  const [locking, setLocking] = useState(false);

  const loadStatus = useCallback(async () => {
    setLoadState((current) => (current === "ready" ? current : "loading"));
    const result = await fetchRuntimeSandboxStatus();
    if (!result.ok) {
      setLoadErrorMessage(describeSandboxLoadFailure(result));
      setLoadState("error");
      return;
    }
    setStatus(result.data);
    setLoadErrorMessage(null);
    setLoadState("ready");
  }, []);

  useEffect(() => {
    void loadStatus();
  }, [loadStatus]);
  const handleUnlock = async () => {
    if (!unlockPin) return;
    setUnlocking(true);
    try {
      const result = await unlockRuntimeSandbox({ pin: unlockPin });
      setUnlockPin("");
      if (!result.ok) {
        setToast({ message: describeSandboxFailure(result, "解锁失败"), type: "error" });
        await loadStatus();
        return;
      }
      setStatus(result.data);
      setToast({ message: "沙箱已解锁，现在可以继续操作。", type: "success" });
    } finally {
      setUnlocking(false);
    }
  };

  const handleLock = async () => {
    setLocking(true);
    try {
      const result = await lockRuntimeSandbox();
      if (!result.ok) {
        setToast({ message: describeSandboxFailure(result, "锁定失败"), type: "error" });
        return;
      }
      setStatus(result.data);
      setToast({ message: "沙箱已锁定。", type: "success" });
    } finally {
      setLocking(false);
    }
  };

  const handleSetPin = async () => {
    if (!newPin || newPin !== confirmPin) {
      setToast({ message: "两次输入的 PIN 不一致，请重新输入。", type: "warning" });
      return;
    }
    if (status?.pin_configured && !currentPin) {
      setToast({ message: "请先输入当前 PIN，再设置新的 PIN。", type: "warning" });
      return;
    }

    setSavingPin(true);
    try {
      const result = await setRuntimeSandboxPin({
        new_pin: newPin,
        current_pin: status?.pin_configured ? currentPin : undefined,
      });
      if (!result.ok) {
        setToast({ message: describeSandboxFailure(result, "PIN 设置失败"), type: "error" });
        await loadStatus();
        return;
      }
      setStatus(result.data);
      setCurrentPin("");
      setNewPin("");
      setConfirmPin("");
      setShowSetPins(false);
      setToast({ message: "PIN 已设置完成，沙箱已自动锁定。", type: "success" });
    } finally {
      setSavingPin(false);
    }
  };

  const handleClearPin = async () => {
    if (!currentPin) {
      setToast({ message: "请先输入当前 PIN，再清除 PIN。", type: "warning" });
      return;
    }
    setClearingPin(true);
    try {
      const result = await clearRuntimeSandboxPin({ current_pin: currentPin });
      setCurrentPin("");
      if (!result.ok) {
        setToast({ message: describeSandboxFailure(result, "清除 PIN 失败"), type: "error" });
        await loadStatus();
        return;
      }
      setStatus(result.data);
      setToast({ message: "PIN 已清除。", type: "success" });
    } finally {
      setClearingPin(false);
    }
  };

  if (loadState === "error") {
    return (
      <div className="flex flex-col h-full">
        <PageHeader title="沙箱管理" description="服务器共享沙箱操作状态" />
        <div className="flex-1 p-6">
          <div className="mx-auto max-w-3xl">
            <Message
              type="error"
              title="暂时无法读取沙箱状态"
              className="mx-auto"
            >
              <div className="flex items-center justify-between gap-4">
                <span>{loadErrorMessage ?? LOAD_ERROR_TEXT}</span>
                <Button size="sm" onClick={() => void loadStatus()}>
                  <RefreshCw className="mr-1 h-4 w-4" />
                  重新加载
                </Button>
              </div>
            </Message>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="flex flex-col h-full">
      <PageHeader
        title="沙箱管理"
        description="服务器共享沙箱操作状态"
        actions={
          <div className="flex gap-2">
            {loadState === "ready" && status && !status.locked && status.pin_configured && (
              <Button size="sm" variant="outline" onClick={() => void handleLock()} disabled={locking}>
                <Lock className="w-4 h-4 mr-1" />
                锁定沙箱
              </Button>
            )}
          </div>
        }
      />

      <div className="flex-1 overflow-auto p-6">
        <div className="w-full max-w-6xl mx-auto space-y-6">
          {loadState === "loading" || !status ? (
            <p className="text-sm text-gray-400 py-8 text-center">正在加载…</p>
          ) : (
            <>
              {/* 单系统用户共享状态说明 */}
              <Message type="info" title="先了解这页会影响什么">
                <div className="flex items-start gap-2">
                  <Users className="mt-0.5 h-4 w-4 shrink-0" />
                  <span>
                    这里管理的是服务器上同一套沙箱状态。也就是说，不同浏览器打开这页时，看到并修改的是同一个
                    PIN 和锁定状态。本页不会提供服务器目录浏览、文件下载或账号权限管理。
                  </span>
                </div>
              </Message>

              {status.rate_limited && (
                <Message type="error" title="沙箱已临时锁定">
                  尝试次数过多，
                  {status.retry_after_seconds != null ? `大约 ${status.retry_after_seconds} 秒后再试。` : "请稍后再试。"}
                  这是整个沙箱的统一限制，换一个浏览器也不会绕过。
                </Message>
              )}

              {/* 沙箱状态卡片 */}
              <Card>
                <CardHeader>
                  <CardTitle className="flex items-center gap-2">
                    {status.locked ? (
                      <Lock className="w-5 h-5 text-red-500" />
                    ) : (
                      <Unlock className="w-5 h-5 text-blue-500" />
                    )}
                    沙箱状态
                    <span className="ml-auto px-2 py-0.5 text-xs bg-blue-100 text-blue-700 rounded">
                      服务器共享沙箱
                    </span>
                  </CardTitle>
                </CardHeader>
                <CardContent>
                  {!status.pin_configured ? (
                    <div className="space-y-3">
                      <Message type="warning" title="沙箱还没有 PIN">
                        当前沙箱没有密码保护。建议先在下方“安全设置”里设置 PIN。
                      </Message>
                    </div>
                  ) : status.locked ? (
                    <div className="space-y-4">
                      <p className="text-sm text-gray-600">沙箱已锁定。输入 PIN 后才能继续操作。</p>
                      <div className="flex gap-2 max-w-md">
                        <div className="relative flex-1">
                          <Input
                            type={showUnlockPin ? "text" : "password"}
                            placeholder="请输入 PIN"
                            value={unlockPin}
                            onChange={(e) => setUnlockPin(e.target.value)}
                            onKeyDown={(e) => e.key === "Enter" && void handleUnlock()}
                            disabled={status.rate_limited}
                          />
                          <button
                            type="button"
                            onClick={() => setShowUnlockPin(!showUnlockPin)}
                            className="absolute right-3 top-1/2 -translate-y-1/2 text-gray-400 hover:text-gray-600"
                          >
                            {showUnlockPin ? <EyeOff className="w-4 h-4" /> : <Eye className="w-4 h-4" />}
                          </button>
                        </div>
                        <Button
                          onClick={() => void handleUnlock()}
                          disabled={!unlockPin || unlocking || status.rate_limited}
                        >
                          <Unlock className="w-4 h-4 mr-1" />
                          立即解锁
                        </Button>
                      </div>
                    </div>
                  ) : (
                    <div className="space-y-4">
                      <p className="text-sm text-blue-600 flex items-center gap-1.5">
                        <CheckCircle2 className="w-4 h-4" />
                        沙箱已解锁，可以继续操作。
                      </p>
                    </div>
                  )}
                </CardContent>
              </Card>

              {/* 安全设置 */}
              <Card>
                <CardHeader>
                  <CardTitle className="flex items-center gap-2">
                    <Shield className="w-5 h-5 text-blue-500" />
                    安全设置
                  </CardTitle>
                </CardHeader>
                <CardContent className="space-y-6">
                  {status.pin_configured && (
                    <div className="space-y-2">
                      <Label className="text-sm font-medium">当前 PIN</Label>
                      <p className="text-xs text-gray-500">修改或清除 PIN 前，需要先验证当前 PIN。</p>
                      <Input
                        type="password"
                        placeholder="请输入当前 PIN"
                        value={currentPin}
                        onChange={(e) => setCurrentPin(e.target.value)}
                        className="max-w-md"
                        disabled={status.rate_limited}
                      />
                    </div>
                  )}

                  <div className="space-y-4">
                    <Label className="text-sm font-medium">{status.pin_configured ? "重新设置 PIN" : "设置新 PIN"}</Label>
                    <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                      <div className="relative">
                        <Input
                          type={showSetPins ? "text" : "password"}
                        placeholder="请输入新 PIN（至少 4 位）"
                          value={newPin}
                          onChange={(e) => setNewPin(e.target.value)}
                        />
                        <button
                          type="button"
                          onClick={() => setShowSetPins(!showSetPins)}
                          className="absolute right-3 top-1/2 -translate-y-1/2 text-gray-400 hover:text-gray-600"
                        >
                          {showSetPins ? <EyeOff className="w-4 h-4" /> : <Eye className="w-4 h-4" />}
                        </button>
                      </div>
                      <Input
                        type="password"
                        placeholder="请再次输入新 PIN"
                        value={confirmPin}
                        onChange={(e) => setConfirmPin(e.target.value)}
                      />
                    </div>
                    <div className="flex gap-2">
                      <Button
                        onClick={() => void handleSetPin()}
                        disabled={!newPin || !confirmPin || savingPin || status.rate_limited}
                        size="sm"
                      >
                        <Save className="w-4 h-4 mr-1" />
                        {status.pin_configured ? "保存新 PIN" : "设置 PIN"}
                      </Button>
                      {status.pin_configured && (
                        <Button
                          onClick={() => void handleClearPin()}
                          disabled={clearingPin || status.rate_limited}
                          size="sm"
                          variant="outline"
                          className="text-red-600 hover:text-red-700 border-red-200 hover:border-red-300"
                        >
                          删除 PIN
                        </Button>
                      )}
                    </div>
                  </div>

                  <Separator />

                  <Message type="info" title="这页不包含的能力">
                    浏览器版不支持查看服务器目录、选择目录或记住口令。这些能力只在桌面端提供，服务器目录仍由管理员统一维护。
                  </Message>
                </CardContent>
              </Card>

              <Separator className="my-6" />
              <div>
                <Label className="text-sm font-semibold">隐私与 Excel 自动脱敏设置</Label>
                <div className="mt-2"><SettingsPrivacySection /></div>
              </div>
            </>
          )}
        </div>
      </div>

      {toast && <Toast message={toast.message} type={toast.type} onClose={() => setToast(null)} />}
    </div>
  );
}
