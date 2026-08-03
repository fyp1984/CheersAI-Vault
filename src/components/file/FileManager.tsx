import { lazy, Suspense } from "react";
import { isTauriHost } from "@/lib/runtime/host";
import { Loading } from "@/components/ui/cheersai-ui";

/**
 * `/files` 路由的宿主分流入口。
 *
 * 使用已通过 Review 的 `isTauriHost()` 作为单一判断依据：Tauri 宿主继续加载
 * 原桌面实现（`FileManagerDesktop`，机械迁移自本文件的历史版本，业务逻辑不变），
 * 普通浏览器加载全新的服务器处理结果查看实现（`FileManagerBrowser`）。两者均使用
 * `React.lazy` 按需加载，确保普通浏览器构建产物不会下载、也不会执行任何
 * Tauri 专属模块。保留原有具名导出 `FileManager`，`App.tsx` 的导入方式不变。
 */
const FileManagerDesktop = lazy(() =>
  import("./FileManagerDesktop").then((module) => ({ default: module.FileManagerDesktop }))
);
const FileManagerBrowser = lazy(() =>
  import("./FileManagerBrowser").then((module) => ({ default: module.FileManagerBrowser }))
);

export function FileManager() {
  const isDesktop = isTauriHost();

  return (
    <Suspense
      fallback={
        <div className="flex h-full items-center justify-center">
          <Loading text="正在加载…" />
        </div>
      }
    >
      {isDesktop ? <FileManagerDesktop /> : <FileManagerBrowser />}
    </Suspense>
  );
}
