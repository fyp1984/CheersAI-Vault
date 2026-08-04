import { useCallback, useEffect, useState } from "react";
import { PageHeader } from "@/components/layout/PageHeader";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Separator } from "@/components/ui/separator";
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

type LoadState = "loading" | "ready" | "disconnected";
type ToastMessage = { message: string; type: "success" | "error" | "warning" };

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

  useEffect(() => {
    if (!toast) return;
    const timer = setTimeout(() => setToast(null), 3000);
    return () => clearTimeout(timer);
  }, [toast]);

  const loadStatus = useCallback(async () => {
    setLoadState((current) => (current === "ready" ? current : "loading"));
    const result = await fetchRuntimeSandboxStatus();
    if (!result.ok) {
      setLoadState("disconnected");
      return;
    }
    setStatus(result.data);
    setLoadState("ready");
  }, []);

  useEffect(() => {
    void loadStatus();
  }, [loadStatus]);

  /**
   * 把三类失败原因分开呈现（C5）：网络层失败（Runtime 完全联系不上）与
   * HTTP 业务错误（含限速）绝不合并成同一句“网络断开”文案。
   */
  const describeFailure = (
    result: Extract<RuntimeFetchResult<unknown>, { ok: false }>,
    fallback: string
  ): string => {
    if (result.reason === "network") {
      return "无法连接本机 Runtime，请确认服务已启动后重试。";
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
  };

  const handleUnlock = async () => {
    if (!unlockPin) return;
    setUnlocking(true);
    try {
      const result = await unlockRuntimeSandbox({ pin: unlockPin });
      setUnlockPin("");
      if (!result.ok) {
        setToast({ message: describeFailure(result, "解锁失败"), type: "error" });
        await loadStatus();
        return;
      }
      setStatus(result.data);
      setToast({ message: "沙箱已解锁", type: "success" });
    } finally {
      setUnlocking(false);
    }
  };

  const handleLock = async () => {
    setLocking(true);
    try {
      const result = await lockRuntimeSandbox();
      if (!result.ok) {
        setToast({ message: describeFailure(result, "锁定失败"), type: "error" });
        return;
      }
      setStatus(result.data);
      setToast({ message: "沙箱已锁定", type: "success" });
    } finally {
      setLocking(false);
    }
  };

  const handleSetPin = async () => {
    if (!newPin || newPin !== confirmPin) {
      setToast({ message: "PIN 不匹配，请重新输入", type: "warning" });
      return;
    }
    if (status?.pin_configured && !currentPin) {
      setToast({ message: "请先输入当前 PIN 才能重新设置", type: "warning" });
      return;
    }

    setSavingPin(true);
    try {
      const result = await setRuntimeSandboxPin({
        new_pin: newPin,
        current_pin: status?.pin_configured ? currentPin : undefined,
      });
      if (!result.ok) {
        setToast({ message: describeFailure(result, "PIN 设置失败"), type: "error" });
        await loadStatus();
        return;
      }
      setStatus(result.data);
      setCurrentPin("");
      setNewPin("");
      setConfirmPin("");
      setShowSetPins(false);
      setToast({ message: "PIN 设置成功，沙箱已锁定", type: "success" });
    } finally {
      setSavingPin(false);
    }
  };

  const handleClearPin = async () => {
    if (!currentPin) {
      setToast({ message: "请先输入当前 PIN 才能清除", type: "warning" });
      return;
    }
    setClearingPin(true);
    try {
      const result = await clearRuntimeSandboxPin({ current_pin: currentPin });
      setCurrentPin("");
      if (!result.ok) {
        setToast({ message: describeFailure(result, "清除 PIN 失败"), type: "error" });
        await loadStatus();
        return;
      }
      setStatus(result.data);
      setToast({ message: "PIN 已清除", type: "success" });
    } finally {
      setClearingPin(false);
    }
  };

  if (loadState === "disconnected") {
    return (
      <div className="flex flex-col h-full">
        <PageHeader title="沙箱管理" description="服务器共享沙箱操作状态" />
        <div className="flex-1 flex flex-col items-center justify-center gap-4 p-6">
          <div className="flex items-center gap-2 text-red-600 text-sm">
            <AlertTriangle className="w-5 h-5" />
            <span>无法连接本机 Runtime，请确认服务已启动后重试。</span>
          </div>
          <Button size="sm" onClick={() => void loadStatus()}>
            <RefreshCw className="w-4 h-4 mr-1" />
            重试
          </Button>
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
              <div className="p-3 bg-blue-50 border border-blue-200 rounded-lg flex items-start gap-2">
                <Users className="w-4 h-4 text-blue-600 flex-shrink-0 mt-0.5" />
                <p className="text-xs text-blue-800">
                  这是同一服务器系统用户共享的沙箱操作状态：所有浏览器会话看到并影响同一个 PIN 和锁定状态。
                  这不是账号登录、RBAC、多租户或 API 鉴权，服务器脱敏映射文件（.cmap）仍只保存在服务器内部，不会
                  因为本页面而对外提供服务器目录浏览或下载。
                </p>
              </div>

              {status.rate_limited && (
                <div className="p-3 bg-red-50 border border-red-200 rounded-lg flex items-start gap-2">
                  <AlertTriangle className="w-4 h-4 text-red-600 flex-shrink-0 mt-0.5" />
                  <p className="text-sm text-red-800">
                    验证过于频繁，沙箱已临时全局锁定
                    {status.retry_after_seconds != null ? `，请约 ${status.retry_after_seconds} 秒后重试` : "，请稍后重试"}
                    。这是 Runtime 单一沙箱状态的全局限制，不按浏览器或客户端分别计数，更换浏览器无法绕过。
                  </p>
                </div>
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
                      <div className="p-3 bg-yellow-50 border border-yellow-200 rounded-lg flex items-start gap-2">
                        <AlertTriangle className="w-4 h-4 text-yellow-600 flex-shrink-0 mt-0.5" />
                        <p className="text-sm text-yellow-800">
                          尚未设置 PIN，沙箱当前无密码保护。请在下方「安全设置」中设置 PIN。
                        </p>
                      </div>
                    </div>
                  ) : status.locked ? (
                    <div className="space-y-4">
                      <p className="text-sm text-gray-600">沙箱已锁定，请输入 PIN 解锁</p>
                      <div className="flex gap-2 max-w-md">
                        <div className="relative flex-1">
                          <Input
                            type={showUnlockPin ? "text" : "password"}
                            placeholder="输入 PIN"
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
                          解锁
                        </Button>
                      </div>
                    </div>
                  ) : (
                    <div className="space-y-4">
                      <p className="text-sm text-blue-600 flex items-center gap-1.5">
                        <CheckCircle2 className="w-4 h-4" />
                        沙箱已解锁
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
                      <p className="text-xs text-gray-500">重新设置或清除 PIN 都需要先验证当前 PIN。</p>
                      <Input
                        type="password"
                        placeholder="输入当前 PIN"
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
                          placeholder="新 PIN (至少 4 位)"
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
                        placeholder="确认 PIN"
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
                        {status.pin_configured ? "重新设置 PIN" : "设置 PIN"}
                      </Button>
                      {status.pin_configured && (
                        <Button
                          onClick={() => void handleClearPin()}
                          disabled={clearingPin || status.rate_limited}
                          size="sm"
                          variant="outline"
                          className="text-red-600 hover:text-red-700 border-red-200 hover:border-red-300"
                        >
                          清除 PIN
                        </Button>
                      )}
                    </div>
                  </div>

                  <Separator />

                  <div className="p-3 bg-gray-50 border border-gray-200 rounded-lg flex items-start gap-2">
                    <Info className="w-4 h-4 text-gray-400 flex-shrink-0 mt-0.5" />
                    <p className="text-xs text-gray-500">
                      普通浏览器不提供服务器目录浏览、目录选择或“打开服务器目录”，也没有默认 mapping 加密口令或
                      “记住口令”选项——这些仍是桌面客户端的能力。服务器数据目录由部署管理员通过 Runtime 配置管理。
                    </p>
                  </div>
                </CardContent>
              </Card>
            </>
          )}
        </div>
      </div>

      {/* Toast 通知 */}
      {toast && (
        <div className="fixed bottom-6 right-6 z-50 animate-in slide-in-from-bottom-4 fade-in duration-300">
          <div
            className={`flex items-center gap-3 px-4 py-3 rounded-lg shadow-lg border ${
              toast.type === "success"
                ? "bg-blue-50 border-blue-200 text-blue-800"
                : toast.type === "error"
                  ? "bg-red-50 border-red-200 text-red-800"
                  : "bg-yellow-50 border-yellow-200 text-yellow-800"
            }`}
          >
            {toast.type === "success" && <CheckCircle2 className="w-5 h-5 text-blue-500 shrink-0" />}
            {toast.type === "error" && <XCircle className="w-5 h-5 text-red-500 shrink-0" />}
            {toast.type === "warning" && <AlertTriangle className="w-5 h-5 text-yellow-500 shrink-0" />}
            <span className="text-sm font-medium">{toast.message}</span>
          </div>
        </div>
      )}
    </div>
  );
}
