import { lazy, Suspense } from "react";
import { isTauriHost } from "@/lib/runtime/host";
import { Loading } from "@/components/ui/cheersai-ui";

/**
 * `/sensitive-terms` 路由的宿主分流入口。
 *
 * 使用已通过 Review 的 `isTauriHost()` 作为单一判断依据：Tauri 宿主继续加载
 * 原桌面实现（`SensitiveTermsDesktop`，机械迁移自本文件的历史版本，业务逻辑
 * 不变），普通浏览器加载全新的 HTTP 适配实现（`SensitiveTermsBrowser`）。两者
 * 均使用 `React.lazy` 按需加载，确保普通浏览器构建产物不会下载、也不会执行
 * 任何 Tauri 专属模块。
 */
const SensitiveTermsDesktop = lazy(() => import("./SensitiveTermsDesktop"));
const SensitiveTermsBrowser = lazy(() => import("./SensitiveTermsBrowser"));

export default function SensitiveTerms() {
  const isDesktop = isTauriHost();

  return (
    <Suspense
      fallback={
        <div className="flex h-full items-center justify-center">
          <Loading text="正在加载…" />
        </div>
      }
    >
      {isDesktop ? <SensitiveTermsDesktop /> : <SensitiveTermsBrowser />}
    </Suspense>
  );
}
