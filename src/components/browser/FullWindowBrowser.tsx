import { useEffect, useRef, useState } from "react";
import { ExternalLink, Loader2 } from "lucide-react";
import { tauriCommands } from "@/lib/tauri";
import { useNavigate } from "react-router-dom";
import { CLOUD_APP_URL } from "@/lib/cloud";

export default function FullWindowBrowser({ 
  initialUrl = CLOUD_APP_URL
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
        console.error("Failed to open webview:", err);
        setError(`打开窗口失败: ${err}`);
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
      <div className="text-center max-w-md">
        {error ? (
          <>
            <div className="text-red-600 mb-4">{error}</div>
            <p className="text-gray-600 mb-4">请刷新页面重试</p>
            <button
              onClick={() => navigate('/process')}
              className="px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700"
            >
              返回应用
            </button>
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
            <ExternalLink className="w-16 h-16 mx-auto mb-6 text-blue-600" />
            <p className="text-lg font-semibold text-gray-800 mb-2">正在打开 CheersAI 云端服务</p>
            <p className="text-sm text-gray-500 mb-4">窗口将最大化显示</p>
            <Loader2 className="w-8 h-8 animate-spin mx-auto text-blue-600" />
          </>
        )}
      </div>
    </div>
  );
}
