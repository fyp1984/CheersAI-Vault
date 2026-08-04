import { useEffect } from "react";
import { useNavigate } from "react-router-dom";
import { listen } from "@tauri-apps/api/event";
import { useLogStore } from "@/store/logStore";
import { tauriCommands } from "@/lib/tauri";
import { setPlatformContext } from "@/lib/path";
import "@/lib/sync-config";

/**
 * 桌面（Tauri）宿主专属的应用启动编排：平台上下文、旧数据库迁移与初始化、
 * `navigate-to-process` 事件监听、配置同步调试工具挂载。
 * 仅在 `isTauriHost()` 为真时由 App.tsx 懒加载挂载，普通浏览器不下载/不执行本模块。
 */
export function DesktopBootstrap() {
  const { initializeDatabase } = useLogStore();
  const navigate = useNavigate();

  useEffect(() => {
    const bootstrapPlatformContext = async () => {
      try {
        const context = await tauriCommands.getPlatformContext();
        setPlatformContext(context);
      } catch (error) {
        console.error("Failed to load platform context:", error);
      }
    };
    bootstrapPlatformContext();
  }, []);

  useEffect(() => {
    const init = async () => {
      try {
        try {
          const migrationResult = await tauriCommands.migrateOldDatabase();
          console.log("Migration result:", migrationResult);
        } catch (migrationError) {
          console.log("No migration needed or migration failed:", migrationError);
        }
        await initializeDatabase();
        console.log("Database initialized successfully");
      } catch (error) {
        console.error("Failed to initialize database:", error);
      }
    };
    init();
  }, [initializeDatabase]);

  useEffect(() => {
    // 卸载可能发生在 listen() 尚未 resolve 之前；用 cancelled 标记确保晚到的
    // unlisten 句柄也会被立即调用一次，不留下泄漏的监听器，也不会重复注册。
    let cancelled = false;
    let unlisten: (() => void) | undefined;
    const setupListener = async () => {
      try {
        const stopListening = await listen("navigate-to-process", () => {
          console.log("Received navigate-to-process event");
          navigate("/process");
        });
        if (cancelled) {
          stopListening();
        } else {
          unlisten = stopListening;
        }
      } catch (error) {
        console.error("Failed to setup event listener:", error);
      }
    };
    setupListener();
    return () => {
      cancelled = true;
      if (unlisten) unlisten();
    };
  }, [navigate]);

  return null;
}

export default DesktopBootstrap;
