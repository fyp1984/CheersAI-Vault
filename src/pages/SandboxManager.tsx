import { lazy, Suspense } from "react";
import { isTauriHost } from "@/lib/runtime/host";
import { Loading } from "@/components/ui/cheersai-ui";

/**
 * `/sandbox` 路由的宿主分流入口。
 *
 * 使用已通过 Review 的 `isTauriHost()` 作为单一判断依据：Tauri 宿主继续加载
 * 原桌面实现（`SandboxManagerDesktop`，机械迁移自本文件的历史版本，
 * Keychain/DPAPI 文案、目录选择、默认加密口令/记住口令、调用参数均不变），
 * 普通浏览器加载全新的 HTTP 适配实现（`SandboxManagerBrowser`，操作服务器
 * 系统用户共享的 PIN/`locked` 状态，不提供服务器目录选择或默认加密口令）。
 * 两者均使用 `React.lazy` 按需加载，确保普通浏览器构建产物不会下载、也不会
 * 执行任何 Tauri/Keychain/DPAPI/系统对话框专属模块。
 */
const SandboxManagerDesktop = lazy(() => import("./SandboxManagerDesktop"));
const SandboxManagerBrowser = lazy(() => import("./SandboxManagerBrowser"));

export default function SandboxManager() {
  const isDesktop = isTauriHost();

  return (
    <Suspense
      fallback={
        <div className="flex h-full items-center justify-center">
          <Loading text="正在加载…" />
        </div>
      }
    >
      {isDesktop ? <SandboxManagerDesktop /> : <SandboxManagerBrowser />}
    </Suspense>
  );
}
