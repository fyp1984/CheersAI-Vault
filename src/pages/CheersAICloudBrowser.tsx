import { useEffect } from "react";
import { tauriCommands } from "@/lib/tauri";
import { useAppStore } from "@/store/appStore";

export default function CheersAICloudBrowser() {
  const { sidebarCollapsed } = useAppStore();

  useEffect(() => {
    let cancelled = false;

    const mountDesktopWorkspace = async () => {
      try {
        await tauriCommands.ensureDesktopChildWebview();
        await tauriCommands.updateDesktopChildWebviewBounds();
      } catch (error) {
        if (!cancelled) {
          console.error("Failed to mount desktop workspace:", error);
        }
      }
    };

    const handleResize = () => {
      void tauriCommands.updateDesktopChildWebviewBounds();
    };

    void mountDesktopWorkspace();
    window.addEventListener("resize", handleResize);

    return () => {
      cancelled = true;
      window.removeEventListener("resize", handleResize);
      void tauriCommands.hideDesktopChildWebview();
    };
  }, []);

  useEffect(() => {
    void tauriCommands.updateDesktopChildWebviewBounds();
  }, [sidebarCollapsed]);

  return (
    <div className="h-full bg-slate-50" />
  );
}
