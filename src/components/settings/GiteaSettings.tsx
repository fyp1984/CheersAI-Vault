import { lazy, Suspense } from "react";
import { isTauriHost } from "@/lib/runtime/host";
import { Loading } from "@/components/ui/cheersai-ui";

/**
 * `/gitea` 路由的宿主分流入口。保持原具名导出 `GiteaSettings`不变，
 * 因为 `App.tsx`（只读范围）以
 * `import("@/components/settings/GiteaSettings").then((m) => ({ default: m.GiteaSettings }))`
 * 的方式静态引用它。
 *
 * 使用已通过 Review 的 `isTauriHost()` 作为单一判断依据：Tauri 宿主继续加载
 * 原桌面实现（`GiteaSettingsDesktop`，机械迁移自本文件的历史版本，Token
 * 输入、Vault 配置导入和全部 Tauri 命令行为不变），普通浏览器加载全新的
 * HTTP 适配实现（`GiteaSettingsBrowser`，只展示由服务器管理员配置的安全
 * 状态，不提供 Token 或目标地址输入）。两者均使用 `React.lazy` 按需加载，
 * 确保普通浏览器构建产物不会下载、也不会执行任何 Tauri/Keychain/DPAPI/
 * 系统对话框专属模块。
 */
const GiteaSettingsDesktop = lazy(() => import("./GiteaSettingsDesktop"));
const GiteaSettingsBrowser = lazy(() => import("./GiteaSettingsBrowser"));

export function GiteaSettings() {
  const isDesktop = isTauriHost();

  return (
    <Suspense
      fallback={
        <div className="flex h-full items-center justify-center">
          <Loading text="正在加载…" />
        </div>
      }
    >
      {isDesktop ? <GiteaSettingsDesktop /> : <GiteaSettingsBrowser />}
    </Suspense>
  );
}
