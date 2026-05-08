import { ExternalLink, AlertCircle, Loader2, MonitorUp, RefreshCw, Shield, Wifi } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { CLOUD_APP_URL } from "@/lib/cloud";
import { tauriCommands } from "@/lib/tauri";

export type CloudFallbackState = "waiting" | "mounting" | "fallback";

interface CheersAICloudProps {
  cloudUrl?: string;
  mountState?: CloudFallbackState;
  mountError?: string | null;
  isMacOS?: boolean;
  onRetryEmbed?: () => void | Promise<void>;
  onOpenStandalone?: () => void | Promise<void>;
  onOpenExternal?: () => void | Promise<void>;
}

const getErrorMessage = (error: unknown) =>
  error instanceof Error ? error.message : String(error);

export default function CheersAICloud({
  cloudUrl = CLOUD_APP_URL,
  mountState = "fallback",
  mountError = null,
  isMacOS = true,
  onRetryEmbed,
  onOpenStandalone,
  onOpenExternal,
}: CheersAICloudProps) {
  const title =
    mountState === "fallback"
      ? "嵌入式云端页面暂时不可用"
      : isMacOS
        ? "正在准备嵌入式云端页面"
        : "正在打开嵌入式云端页面";

  const description =
    mountState === "fallback"
      ? "已自动回退到统一访问页，你可以改用应用内独立窗口或系统浏览器继续访问云端工作区。"
      : isMacOS
        ? "Vault 会先尝试在主窗口内挂载子 WebView；在 macOS 稳定挂载期间，你仍可直接切换到其他访问方式。"
        : "Vault 正在初始化内嵌子 WebView；如需立即继续，也可以直接打开独立窗口或系统浏览器。";

  const statusLabel =
    mountState === "fallback"
      ? "已回退"
      : mountState === "mounting"
        ? "正在挂载"
        : "等待挂载";

  const statusTone =
    mountState === "fallback"
      ? "border-amber-200 bg-amber-50 text-amber-700"
      : "border-blue-200 bg-blue-50 text-blue-700";

  const openStandalone = async () => {
    if (onOpenStandalone) {
      await onOpenStandalone();
      return;
    }

    await tauriCommands.openDesktopWindowWithButton(cloudUrl);
  };

  const openExternal = async () => {
    if (onOpenExternal) {
      await onOpenExternal();
      return;
    }

    try {
      const { open } = await import("@tauri-apps/plugin-shell");
      await open(cloudUrl);
    } catch (error) {
      console.error("Failed to open external browser:", error);
      window.open(cloudUrl, "_blank", "noopener,noreferrer");
    }
  };

  const handleOpenStandalone = async () => {
    try {
      await openStandalone();
    } catch (error) {
      console.error("Failed to open standalone cloud window:", error);
      window.alert(`打开应用内窗口失败：${getErrorMessage(error)}`);
    }
  };

  const handleOpenExternal = async () => {
    try {
      await openExternal();
    } catch (error) {
      console.error("Failed to open external cloud browser:", error);
      window.alert(`打开系统浏览器失败：${getErrorMessage(error)}`);
    }
  };

  const handleRetryEmbed = () => {
    if (!onRetryEmbed) {
      return;
    }

    void onRetryEmbed();
  };

  return (
    <div className="flex h-full flex-col bg-gray-50">
      <div className="border-b border-gray-200 bg-white">
        <div className="mx-auto flex w-full max-w-6xl flex-col gap-4 px-8 py-6">
          <div className="flex flex-col gap-4 lg:flex-row lg:items-start lg:justify-between">
            <div className="flex items-start gap-4">
              <div className="flex h-12 w-12 items-center justify-center rounded-xl bg-blue-600 text-white shadow-sm">
                {mountState === "fallback" ? (
                  <AlertCircle className="h-5 w-5" />
                ) : (
                  <MonitorUp className="h-5 w-5" />
                )}
              </div>
              <div className="space-y-2">
                <div className="space-y-1">
                  <h1 className="text-2xl font-bold text-gray-900">{title}</h1>
                  <p className="text-sm leading-6 text-gray-600">{description}</p>
                </div>
                <div className="flex flex-wrap gap-2 text-xs text-gray-600">
                  <span className="inline-flex items-center gap-1 rounded-full bg-blue-50 px-3 py-1 text-blue-700">
                    <Shield className="h-3.5 w-3.5" />
                    `/cloud` 默认优先尝试内嵌
                  </span>
                  <span className="inline-flex items-center gap-1 rounded-full bg-gray-100 px-3 py-1">
                    <Wifi className="h-3.5 w-3.5" />
                    兼容 macOS Intel / M1 / M2
                  </span>
                </div>
              </div>
            </div>

            <div className={`inline-flex items-center gap-2 rounded-full border px-3 py-1.5 text-xs font-medium ${statusTone}`}>
              {mountState === "fallback" ? (
                <AlertCircle className="h-3.5 w-3.5" />
              ) : (
                <Loader2 className="h-3.5 w-3.5 animate-spin" />
              )}
              {statusLabel}
            </div>
          </div>

          <div className="rounded-lg border border-blue-100 bg-blue-50 px-4 py-3 text-sm text-blue-800">
            兼容 macOS Intel、Apple M1、Apple M2 等机型；若内嵌 WebView 在当前环境下不可用，可直接切换到应用内独立窗口或系统浏览器继续访问。
          </div>
        </div>
      </div>

      <div className="flex-1 px-8 py-8">
        <div className="mx-auto flex h-full w-full max-w-6xl flex-col gap-6">
          <Card className="border-gray-200 shadow-sm">
            <CardContent className="space-y-4 p-6">
              <div className="flex flex-col gap-3 lg:flex-row lg:items-start lg:justify-between">
                <div className="space-y-2">
                  <div className="text-lg font-semibold text-gray-900">统一回退页</div>
                  <p className="text-sm leading-6 text-gray-600">
                    当 `/cloud` 的内嵌挂载失败或仍在等待时，当前页面统一承接状态提示与后续入口，避免白屏或多套页面分叉。
                  </p>
                </div>
                <div className="rounded-lg bg-gray-50 px-3 py-2 font-mono text-xs text-blue-700">
                  {cloudUrl}
                </div>
              </div>

              {mountError && (
                <div className="rounded-lg border border-amber-200 bg-amber-50 px-4 py-3 text-sm text-amber-800">
                  <div className="font-medium">挂载错误</div>
                  <div className="mt-1 break-words">{mountError}</div>
                </div>
              )}

              <div className="grid gap-4 md:grid-cols-3">
                <div className="rounded-lg border border-gray-200 bg-gray-50 p-4">
                  <div className="text-sm font-medium text-gray-900">默认行为</div>
                  <p className="mt-2 text-sm leading-6 text-gray-600">
                    先在主窗口尝试嵌入式子 WebView，保持导航、侧边栏和工作区上下文一致。
                  </p>
                </div>
                <div className="rounded-lg border border-gray-200 bg-gray-50 p-4">
                  <div className="text-sm font-medium text-gray-900">回退策略</div>
                  <p className="mt-2 text-sm leading-6 text-gray-600">
                    嵌入失败后保留当前页，统一提供“在应用内独立窗口打开”和“在系统浏览器打开”两个兜底入口。
                  </p>
                </div>
                <div className="rounded-lg border border-gray-200 bg-gray-50 p-4">
                  <div className="text-sm font-medium text-gray-900">平台兼容</div>
                  <p className="mt-2 text-sm leading-6 text-gray-600">
                    文案统一覆盖 macOS、Intel、Apple M1、Apple M2 场景，避免机型口径不一致。
                  </p>
                </div>
              </div>
            </CardContent>
          </Card>

          <Card className="border-gray-200 shadow-sm">
            <CardContent className="space-y-6 p-6">
              <div className="space-y-2">
                <div className="text-lg font-semibold text-gray-900">继续访问云端工作区</div>
                <p className="text-sm leading-6 text-gray-600">
                  {mountState === "fallback"
                    ? "当前已进入统一回退页。建议优先改用应用内独立窗口；若仍需最高兼容性，可直接在系统浏览器中打开。"
                    : "系统仍在尝试挂载内嵌页面。如你不想继续等待，也可以直接使用下方两种稳定入口。"}
                </p>
              </div>

              <div className="grid gap-3 lg:grid-cols-3">
                <Button
                  type="button"
                  onClick={handleOpenStandalone}
                  className="h-11 bg-blue-600 text-white hover:bg-blue-700"
                >
                  <MonitorUp className="mr-2 h-4 w-4" />
                  在应用内独立窗口打开
                </Button>

                <Button
                  type="button"
                  variant="outline"
                  onClick={handleOpenExternal}
                  className="h-11 border-gray-200 text-gray-700 hover:bg-gray-50"
                >
                  <ExternalLink className="mr-2 h-4 w-4" />
                  在系统浏览器打开
                </Button>

                <Button
                  type="button"
                  variant="outline"
                  onClick={handleRetryEmbed}
                  disabled={!onRetryEmbed || mountState === "mounting"}
                  className="h-11 border-gray-200 text-gray-700 hover:bg-gray-50"
                >
                  {mountState === "mounting" ? (
                    <>
                      <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                      正在尝试内嵌
                    </>
                  ) : (
                    <>
                      <RefreshCw className="mr-2 h-4 w-4" />
                      重新尝试内嵌
                    </>
                  )}
                </Button>
              </div>

              <div className="rounded-lg border border-gray-200 bg-gray-50 px-4 py-3 text-xs leading-6 text-gray-500">
                提示：若某些 macOS 环境下内嵌 WebView 未能稳定挂载，优先使用“在应用内独立窗口打开”；若需要最高兼容性，再使用“在系统浏览器打开”。
              </div>
            </CardContent>
          </Card>
        </div>
      </div>
    </div>
  );
}
