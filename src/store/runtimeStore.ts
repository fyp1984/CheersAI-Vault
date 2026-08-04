/**
 * 浏览器宿主 Runtime 健康状态的单一事实源。
 *
 * Sidebar 与 MainLayout 都从这里读取同一份状态，避免出现“全局已连接、内容区
 * 离线”的矛盾。普通浏览器下统一以 `/api/v1/health` 的 HTTP 2xx 判定 Runtime
 * 在线；网络失败、非 2xx 或响应解析失败一律归类为离线，不把任何服务端细节
 * 透出。桌面（Tauri）宿主不使用本状态，保持既有“运行正常”与本地工作区语义。
 *
 * 轮询在模块加载时启动（模块单例，全局只跑一个定时器）；标签页从后台回到
 * 前台时立即补一次健康检查，避免长时间停留在旧状态。页面只通过
 * `useRuntimeHealthStore` 读取 `status`，需要显式重试时调用 `refresh()`。
 */
import { create } from "zustand";
import { fetchRuntimeHealth } from "@/lib/runtime/client";
import { isTauriHost } from "@/lib/runtime/host";

export type RuntimeHealthStatus = "checking" | "online" | "offline";

interface RuntimeHealthStore {
  status: RuntimeHealthStatus;
  /** 立即发起一次健康检查；用户显式重试或标签页恢复可见时调用。 */
  refresh: () => Promise<void>;
}

const RUNTIME_HEALTH_POLL_MS = 5_000;

export const useRuntimeHealthStore = create<RuntimeHealthStore>((set) => ({
  status: "checking",
  refresh: async () => {
    const result = await fetchRuntimeHealth();
    set({ status: result.ok ? "online" : "offline" });
  },
}));

if (!isTauriHost() && typeof window !== "undefined") {
  void useRuntimeHealthStore.getState().refresh();
  window.setInterval(() => {
    void useRuntimeHealthStore.getState().refresh();
  }, RUNTIME_HEALTH_POLL_MS);
  document.addEventListener("visibilitychange", () => {
    if (document.visibilityState === "visible") {
      void useRuntimeHealthStore.getState().refresh();
    }
  });
}
