import { useEffect, useRef, useState } from "react";
import { Loader2 } from "lucide-react";
import { tauriCommands } from "@/lib/tauri";
import { useNavigate } from "react-router-dom";

export default function FullWindowBrowser({ 
  initialUrl = "https://uat-desktop.cheersai.cloud/"
}: { initialUrl?: string }) {
  const [error, setError] = useState<string | null>(null);
  const [opened, setOpened] = useState(false);
  const startedRef = useRef(false);
  const retryCountRef = useRef(0);
  const navigate = useNavigate();
  
  useEffect(() => {
    if (startedRef.current) {
      return;
    }
    startedRef.current = true;

    let cancelled = false;
    let timeoutId: ReturnType<typeof setTimeout> | undefined;

    const navigateWithButton = async () => {
      try {
        setError(null);
        await tauriCommands.openDesktopWindowWithButton(initialUrl);
        setOpened(true);
        navigate("/process", { replace: true });

        timeoutId = setTimeout(() => {
          if (cancelled) {
            return;
          }
          if (retryCountRef.current >= 1) {
            setError("打开 Desktop 窗口超时，请检查网络或稍后重试");
            return;
          }
          retryCountRef.current += 1;
          void navigateWithButton();
        }, 8000);
      } catch (err) {
        console.error("Failed to navigate with button:", err);
        setError(`导航失败: ${err}`);
      }
    };

    void navigateWithButton();

    return () => {
      cancelled = true;
      if (timeoutId) {
        clearTimeout(timeoutId);
      }
    };
  }, [initialUrl, navigate]);

  return (
    <div className="fixed inset-0 z-50 bg-white flex items-center justify-center">
      <div className="text-center">
        {error ? (
          <>
            <div className="text-red-600 mb-4">{error}</div>
            <p className="text-gray-600">请刷新页面重试</p>
          </>
        ) : opened ? (
          <>
            <p className="text-lg font-semibold text-gray-800 mb-2">Desktop 已在独立窗口打开</p>
            <p className="text-sm text-gray-500 mb-6">点击右侧“数据安全屋”图标即可直接返回 Vault 工作区</p>
            <button
              type="button"
              className="px-4 py-2 rounded-md bg-blue-600 text-white text-sm hover:bg-blue-700 transition-colors"
              onClick={() => navigate("/process")}
            >
              返回 Vault 工作区
            </button>
          </>
        ) : (
          <>
            <Loader2 className="w-10 h-10 animate-spin mx-auto mb-6 text-blue-600" />
            <p className="text-lg font-semibold text-gray-800 mb-2">CheersAI</p>
            <p className="text-sm text-gray-500">让数据留在本地，让 AI 能力走在前沿</p>
          </>
        )}
      </div>
    </div>
  );
}
