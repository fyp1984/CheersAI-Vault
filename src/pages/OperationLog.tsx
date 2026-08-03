import { lazy, Suspense } from "react";
import { isTauriHost } from "@/lib/runtime/host";
import { Loading } from "@/components/ui/cheersai-ui";

/**
 * `/log` 路由的宿主分流入口。
 *
 * 使用已通过 Review 的 `isTauriHost()` 作为单一判断依据：Tauri 宿主继续加载
 * 原桌面实现（`OperationLogDesktop`，机械迁移自本文件的历史版本，`logStore`/
 * Tauri 命令/分页/筛选/统计/清空行为不变），普通浏览器加载全新的 HTTP 适配
 * 实现（`OperationLogBrowser`，直接投影 Runtime 现有 `job_events`/
 * `restore_events`/`batches`/`batch_files`，不建立平行日志表）。两者均使用
 * `React.lazy` 按需加载，确保普通浏览器构建产物不会下载、也不会执行任何
 * Tauri 专属模块或桌面 `logStore`。
 */
const OperationLogDesktop = lazy(() => import("./OperationLogDesktop"));
const OperationLogBrowser = lazy(() => import("./OperationLogBrowser"));

export default function OperationLog() {
  const isDesktop = isTauriHost();

  return (
    <Suspense
      fallback={
        <div className="flex h-full items-center justify-center">
          <Loading text="正在加载…" />
        </div>
      }
    >
      {isDesktop ? <OperationLogDesktop /> : <OperationLogBrowser />}
    </Suspense>
  );
}
