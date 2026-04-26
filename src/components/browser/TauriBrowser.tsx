import { useState, useEffect, useRef } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { 
  ArrowLeft, 
  ArrowRight, 
  RotateCcw, 
  Home, 
  ExternalLink,
  Loader2,
  AlertCircle,
  Globe
} from "lucide-react";
import { CLOUD_APP_URL } from "@/lib/cloud";

interface TauriBrowserProps {
  initialUrl?: string;
  onUrlChange?: (url: string) => void;
}

export default function TauriBrowser({ 
  initialUrl = CLOUD_APP_URL,
  onUrlChange 
}: TauriBrowserProps) {
  const [currentUrl, setCurrentUrl] = useState(initialUrl);
  const [inputUrl, setInputUrl] = useState(initialUrl);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [canGoBack, setCanGoBack] = useState(false);
  const [canGoForward, setCanGoForward] = useState(false);
  const webviewRef = useRef<HTMLDivElement>(null);

  // 使用 Tauri 的 WebView API
  useEffect(() => {
    const initWebView = async () => {
      try {
        // 动态导入 Tauri WebView
        const { WebviewWindow } = await import("@tauri-apps/api/webviewWindow");
        
        // 创建嵌入式 WebView
        const webview = new WebviewWindow("embedded-browser", {
          url: initialUrl,
          width: 800,
          height: 600,
          resizable: true,
          decorations: false,
          transparent: false,
          alwaysOnTop: false,
          skipTaskbar: true,
          parent: "main"
        });

        // 监听 WebView 事件
        webview.once("tauri://created", () => {
          console.log("WebView created successfully");
          setIsLoading(false);
        });

        webview.once("tauri://error", (e) => {
          console.error("WebView creation error:", e);
          setError("浏览器创建失败");
          setIsLoading(false);
        });

      } catch (error) {
        console.error("Failed to initialize WebView:", error);
        setError("浏览器初始化失败");
        setIsLoading(false);
      }
    };

    setIsLoading(true);
    initWebView();
  }, [initialUrl]);

  const handleNavigate = async (url: string) => {
    if (!url || !url.trim()) return;
    
    const fullUrl = url.startsWith('http') ? url : `https://${url}`;
    setIsLoading(true);
    setError(null);
    
    try {
      // 使用 Tauri 的导航 API
      const { invoke } = await import("@tauri-apps/api/core");
      await invoke("navigate_webview", { url: fullUrl });
      
      setCurrentUrl(fullUrl);
      setInputUrl(fullUrl);
      onUrlChange?.(fullUrl);
    } catch (error) {
      console.error("Navigation failed:", error);
      setError(`导航失败: ${error}`);
    } finally {
      setIsLoading(false);
    }
  };

  const handleGoBack = async () => {
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      await invoke("webview_go_back");
      setCanGoBack(false);
      setCanGoForward(true);
    } catch (error) {
      console.error("Go back failed:", error);
    }
  };

  const handleGoForward = async () => {
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      await invoke("webview_go_forward");
      setCanGoForward(false);
      setCanGoBack(true);
    } catch (error) {
      console.error("Go forward failed:", error);
    }
  };

  const handleRefresh = async () => {
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      await invoke("webview_reload");
    } catch (error) {
      console.error("Refresh failed:", error);
    }
  };

  const handleHome = () => {
    handleNavigate(initialUrl);
  };

  const handleOpenExternal = async () => {
    try {
      const { open } = await import("@tauri-apps/plugin-shell");
      await open(currentUrl);
    } catch (error) {
      console.error("Failed to open external:", error);
      window.open(currentUrl, '_blank');
    }
  };

  return (
    <div className="flex flex-col h-full bg-white">
      {/* 浏览器工具栏 */}
      <div className="flex items-center gap-2 p-3 border-b bg-gray-50">
        <div className="flex items-center gap-1">
          <Button
            variant="outline"
            size="sm"
            onClick={handleGoBack}
            disabled={!canGoBack || isLoading}
          >
            <ArrowLeft className="w-4 h-4" />
          </Button>
          <Button
            variant="outline"
            size="sm"
            onClick={handleGoForward}
            disabled={!canGoForward || isLoading}
          >
            <ArrowRight className="w-4 h-4" />
          </Button>
          <Button
            variant="outline"
            size="sm"
            onClick={handleRefresh}
            disabled={isLoading}
          >
            {isLoading ? (
              <Loader2 className="w-4 h-4 animate-spin" />
            ) : (
              <RotateCcw className="w-4 h-4" />
            )}
          </Button>
          <Button
            variant="outline"
            size="sm"
            onClick={handleHome}
            disabled={isLoading}
          >
            <Home className="w-4 h-4" />
          </Button>
        </div>
        
        <div className="flex-1 flex items-center gap-2">
          <div className="flex items-center gap-2 flex-1">
            <Globe className="w-4 h-4 text-gray-500" />
            <Input
              value={inputUrl}
              onChange={(e) => setInputUrl(e.target.value)}
              onKeyPress={(e) => {
                if (e.key === 'Enter') {
                  handleNavigate(inputUrl);
                }
              }}
              placeholder="输入网址..."
              className="flex-1"
              disabled={isLoading}
            />
          </div>
          <Button
            onClick={() => handleNavigate(inputUrl)}
            disabled={isLoading}
            size="sm"
          >
            访问
          </Button>
        </div>
        
        <Button
          variant="outline"
          size="sm"
          onClick={handleOpenExternal}
          title="在外部浏览器打开"
        >
          <ExternalLink className="w-4 h-4" />
        </Button>
      </div>

      {/* 错误提示 */}
      {error && (
        <div className="flex items-center gap-2 p-3 bg-red-50 border-b border-red-200 text-red-700">
          <AlertCircle className="w-4 h-4" />
          <span>{error}</span>
          <Button
            variant="outline"
            size="sm"
            onClick={() => setError(null)}
            className="ml-auto h-6 px-2 text-xs"
          >
            关闭
          </Button>
        </div>
      )}

      {/* 加载状态 */}
      {isLoading && (
        <div className="flex items-center justify-center p-4 bg-blue-50 border-b">
          <Loader2 className="w-5 h-5 animate-spin mr-2" />
          <span>正在加载页面...</span>
        </div>
      )}

      {/* WebView 容器 */}
      <div 
        ref={webviewRef}
        className="flex-1 relative bg-white"
        id="webview-container"
      >
        {/* WebView 将被嵌入到这里 */}
        <div className="w-full h-full flex items-center justify-center text-gray-500">
          <div className="text-center">
            <Globe className="w-12 h-12 mx-auto mb-4 text-gray-300" />
            <div className="text-lg mb-2">浏览器引擎</div>
            <div className="text-sm">正在初始化...</div>
          </div>
        </div>
      </div>
    </div>
  );
}
