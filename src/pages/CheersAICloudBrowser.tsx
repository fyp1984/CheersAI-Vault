import { lazy, Suspense } from "react";
import { isTauriHost } from "@/lib/runtime/host";
import { Loading } from "@/components/ui/cheersai-ui";

const CheersAICloudBrowserDesktop = lazy(() => import("./CheersAICloudBrowserDesktop"));
const CheersAICloudBrowserBrowser = lazy(() => import("./CheersAICloudBrowserBrowser"));

/**
 * `/cloud` 的宿主入口：单一判定，桌面挂载内嵌子 WebView 云端工作区
 * （机械迁移至 CheersAICloudBrowserDesktop.tsx，逻辑与行为不变），浏览器
 * 始终停在统一回退页（CheersAICloudBrowserBrowser.tsx），不下载/不执行
 * 任何 Tauri 命令。
 */
export default function CheersAICloudBrowser() {
  const isDesktop = isTauriHost();
  return (
    <Suspense fallback={<div className="flex h-full items-center justify-center"><Loading text="正在加载…" /></div>}>
      {isDesktop ? <CheersAICloudBrowserDesktop /> : <CheersAICloudBrowserBrowser />}
    </Suspense>
  );
}
