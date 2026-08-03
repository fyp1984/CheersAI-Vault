import { lazy, Suspense } from "react";
import { isTauriHost } from "@/lib/runtime/host";
import { Loading } from "@/components/ui/cheersai-ui";

/**
 * `/unmask` 路由的宿主分流入口。
 *
 * 使用已通过 Review 的 `isTauriHost()` 作为单一判断依据：Tauri 宿主继续加载
 * 原桌面实现（`FileUnmaskDesktop`，机械迁移自本文件的历史版本，业务逻辑不变），
 * 普通浏览器加载全新的服务器内部映射恢复实现（`FileUnmaskBrowser`）。两者均使用
 * `React.lazy` 按需加载，确保普通浏览器构建产物不会下载、也不会执行任何
 * Tauri 专属模块。
 */
const FileUnmaskDesktop = lazy(() => import("./FileUnmaskDesktop"));
const FileUnmaskBrowser = lazy(() => import("./FileUnmaskBrowser"));

export default function FileUnmask() {
  const isDesktop = isTauriHost();

  return (
    <Suspense
      fallback={
        <div className="flex h-full items-center justify-center">
          <Loading text="正在加载…" />
        </div>
      }
    >
      {isDesktop ? <FileUnmaskDesktop /> : <FileUnmaskBrowser />}
    </Suspense>
  );
}
