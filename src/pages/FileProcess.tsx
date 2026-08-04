import { lazy, Suspense } from "react";
import { isTauriHost } from "@/lib/runtime/host";
import { Loading } from "@/components/ui/cheersai-ui";

/**
 * `/process` 路由的宿主分流入口。
 *
 * 使用已通过 Review 的 `isTauriHost()` 作为单一判断依据：Tauri 宿主继续加载
 * 原桌面实现（`FileProcessDesktop`，机械迁移自本文件的历史版本，业务逻辑不变），
 * 普通浏览器加载全新的 HTTP 适配实现（`FileProcessBrowser`）。两者均使用
 * `React.lazy` 按需加载，确保普通浏览器构建产物不会下载、也不会执行任何
 * Tauri 专属模块。
 */
const FileProcessDesktop = lazy(() => import("./FileProcessDesktop"));
const FileProcessBrowser = lazy(() => import("./FileProcessBrowser"));

export default function FileProcess() {
  const isDesktop = isTauriHost();

  return (
    <Suspense
      fallback={
        <div className="flex h-full items-center justify-center">
          <Loading text="正在加载…" />
        </div>
      }
    >
      {isDesktop ? <FileProcessDesktop /> : <FileProcessBrowser />}
    </Suspense>
  );
}
