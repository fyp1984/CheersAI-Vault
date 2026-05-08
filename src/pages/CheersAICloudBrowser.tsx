import { useCallback, useEffect, useRef, useState } from "react";
import { CLOUD_APP_URL } from "@/lib/cloud";
import { getPlatform } from "@/lib/path";
import { tauriCommands } from "@/lib/tauri";
import CheersAICloud from "@/pages/CheersAICloud";
import { useAppStore } from "@/store/appStore";

type CloudMountState = "waiting" | "mounting" | "embedded" | "fallback";

const MACOS_EMBED_DELAY_MS = 450;

const getErrorMessage = (error: unknown) =>
  error instanceof Error ? error.message : String(error);

export default function CheersAICloudBrowser() {
  const { sidebarCollapsed } = useAppStore();
  const isMacOS = getPlatform() === "macos";
  const [mountState, setMountState] = useState<CloudMountState>(
    isMacOS ? "waiting" : "mounting"
  );
  const [mountError, setMountError] = useState<string | null>(null);
  const mountStateRef = useRef<CloudMountState>(isMacOS ? "waiting" : "mounting");
  const mountedRef = useRef(false);
  const mountAttemptRef = useRef(0);

  const setMountStateSafe = useCallback((nextState: CloudMountState) => {
    mountStateRef.current = nextState;
    setMountState(nextState);
  }, []);

  const mountDesktopWorkspace = useCallback(async () => {
    const attemptId = ++mountAttemptRef.current;

    setMountError(null);
    setMountStateSafe("mounting");

    try {
      await tauriCommands.ensureDesktopChildWebview();
      if (!mountedRef.current || attemptId !== mountAttemptRef.current) {
        return;
      }

      await tauriCommands.updateDesktopChildWebviewBounds();
      if (!mountedRef.current || attemptId !== mountAttemptRef.current) {
        return;
      }

      setMountStateSafe("embedded");
    } catch (error) {
      if (!mountedRef.current || attemptId !== mountAttemptRef.current) {
        return;
      }

      const message = getErrorMessage(error);
      console.error("Failed to mount desktop workspace:", error);
      setMountError(message);
      setMountStateSafe("fallback");
      void tauriCommands.hideDesktopChildWebview();
    }
  }, [setMountStateSafe]);

  const handleOpenStandalone = useCallback(async () => {
    try {
      await tauriCommands.openDesktopWindowWithButton(CLOUD_APP_URL);
    } catch (error) {
      const message = getErrorMessage(error);
      console.error("Failed to open desktop window:", error);
      setMountError(`打开独立窗口失败：${message}`);
    }
  }, []);

  const handleOpenExternal = useCallback(async () => {
    try {
      const { open } = await import("@tauri-apps/plugin-shell");
      await open(CLOUD_APP_URL);
    } catch (error) {
      console.error("Failed to open external browser:", error);
      window.open(CLOUD_APP_URL, "_blank", "noopener,noreferrer");
    }
  }, []);

  useEffect(() => {
    mountedRef.current = true;
    const delay = isMacOS ? MACOS_EMBED_DELAY_MS : 0;
    const timerId = window.setTimeout(() => {
      void mountDesktopWorkspace();
    }, delay);

    const handleResize = () => {
      if (mountStateRef.current === "embedded") {
        void tauriCommands.updateDesktopChildWebviewBounds();
      }
    };

    window.addEventListener("resize", handleResize);

    return () => {
      mountedRef.current = false;
      mountAttemptRef.current += 1;
      window.clearTimeout(timerId);
      window.removeEventListener("resize", handleResize);
      mountStateRef.current = "fallback";
      void tauriCommands.hideDesktopChildWebview();
    };
  }, [isMacOS, mountDesktopWorkspace]);

  useEffect(() => {
    if (mountState === "embedded") {
      void tauriCommands.updateDesktopChildWebviewBounds();
    }
  }, [mountState, sidebarCollapsed]);

  if (mountState === "embedded") {
    return <div className="h-full bg-slate-50" />;
  }

  return (
    <CheersAICloud
      cloudUrl={CLOUD_APP_URL}
      mountState={mountState}
      mountError={mountError}
      isMacOS={isMacOS}
      onRetryEmbed={mountDesktopWorkspace}
      onOpenStandalone={handleOpenStandalone}
      onOpenExternal={handleOpenExternal}
    />
  );
}
