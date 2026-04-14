import { useState, useEffect } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { 
  RotateCcw, 
  Home, 
  ExternalLink,
  Loader2,
  AlertCircle,
  Globe,
  Monitor
} from "lucide-react";
import { tauriCommands } from "@/lib/tauri";
import { CLOUD_APP_URL, resolveCloudUrl } from "@/lib/cloud";

interface NativeBrowserProps {
  initialUrl?: string;
  onUrlChange?: (url: string) => void;
}

export default function NativeBrowser({ 
  initialUrl = CLOUD_APP_URL,
  onUrlChange 
}: NativeBrowserProps) {
  const [currentUrl, setCurrentUrl] = useState(initialUrl);
  const [inputUrl, setInputUrl] = useState(initialUrl);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [webviewLabel, setWebviewLabel] = useState<string | null>(null);
  const [isWebviewReady, setIsWebviewReady] = useState(false);

  // 初始化 WebView
  useEffect(() => {
    const initWebView = async () => {
      setIsLoading(true);
      setError(null);
      
      try {
        console.log("Creating WebView window...");
        
        // 创建一个新的 WebView 窗口
        const label = await tauriCommands.openWebviewWindow({
          url: initialUrl,
          title: "CheersAI 云端服务",
          width: 1200,
          height: 800,
        });
        
        console.log("WebView created with label:", label);
        setWebviewLabel(label);
        setIsWebviewReady(true);
        setCurrentUrl(initialUrl);
        onUrlChange?.(initialUrl);
        
      } catch (error) {
        console.error("Failed to create WebView:", error);
        setError(`浏览器创建失败: ${error}`);
      } finally {
        setIsLoading(false);
      }
    };

    initWebView();

    // 清理函数
    return () => {
      if (webviewLabel) {
        tauriCommands.closeWebviewWindow(webviewLabel).catch(console.error);
      }
    };
  }, []);

  const handleNavigate = async (url: string) => {
    if (!webviewLabel || !url || !url.trim()) return;
    
    const fullUrl = url.startsWith('http') ? url : `https://${url}`;
    setIsLoading(true);
    setError(null);
    
    try {
      console.log("Navigating to:", fullUrl);
      await tauriCommands.navigateWebview(webviewLabel, fullUrl);
      
      setCurrentUrl(fullUrl);
      setInputUrl(fullUrl);
      onUrlChange?.(fullUrl);
      
      console.log("Navigation successful");
    } catch (error) {
      console.error("Navigation failed:", error);
      setError(`导航失败: ${error}`);
    } finally {
      setIsLoading(false);
    }
  };

  const handleRefresh = async () => {
    if (!webviewLabel) return;
    
    setIsLoading(true);
    try {
      console.log("Refreshing WebView...");
      // 重新导航到当前 URL 来实现刷新
      await tauriCommands.navigateWebview(webviewLabel, currentUrl);
      console.log("Refresh successful");
    } catch (error) {
      console.error("Refresh failed:", error);
      setError(`刷新失败: ${error}`);
    } finally {
      setIsLoading(false);
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

  const handleGetCurrentUrl = async () => {
    if (!webviewLabel) return;
    
    try {
      const url = await tauriCommands.getWebviewUrl(webviewLabel);
      console.log("Current WebView URL:", url);
      setCurrentUrl(url);
      setInputUrl(url);
      onUrlChange?.(url);
    } catch (error) {
      console.error("Failed to get current URL:", error);
    }
  };

  const handleInjectScript = async () => {
    if (!webviewLabel) return;
    
    try {
      const script = `
        console.log('Script injected into WebView');
        console.log('Current URL:', window.location.href);
        console.log('Page title:', document.title);
        
        // 尝试自动跳转到登录页面
        if (window.location.href.includes('/apps/') && !window.location.href.includes('login')) {
          console.log('Attempting auto-redirect to login...');
          
          // 查找登录链接
          const loginLinks = document.querySelectorAll('a[href*="login"], .login, #login, [data-login]');
          if (loginLinks.length > 0) {
            console.log('Found login link, clicking...');
            loginLinks[0].click();
          } else {
            // 直接跳转到登录页面
            console.log('No login links found, direct navigation...');
            window.location.href = '${resolveCloudUrl("/login")}';
          }
        }
        
        // 返回页面信息
        JSON.stringify({
          url: window.location.href,
          title: document.title,
          readyState: document.readyState
        });
      `;
      
      await tauriCommands.webviewEvalScript(webviewLabel, script);
      console.log("Script injected successfully");
    } catch (error) {
      console.error("Failed to inject script:", error);
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
            onClick={handleRefresh}
            disabled={isLoading || !isWebviewReady}
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
            disabled={isLoading || !isWebviewReady}
          >
            <Home className="w-4 h-4" />
          </Button>
          <Button
            variant="outline"
            size="sm"
            onClick={handleGetCurrentUrl}
            disabled={isLoading || !isWebviewReady}
            title="获取当前 URL"
          >
            <Globe className="w-4 h-4" />
          </Button>
        </div>
        
        <div className="flex-1 flex items-center gap-2">
          <div className="flex items-center gap-2 flex-1">
            <Monitor className="w-4 h-4 text-gray-500" />
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
              disabled={isLoading || !isWebviewReady}
            />
          </div>
          <Button
            onClick={() => handleNavigate(inputUrl)}
            disabled={isLoading || !isWebviewReady}
            size="sm"
          >
            访问
          </Button>
        </div>
        
        <div className="flex items-center gap-1">
          <Button
            variant="outline"
            size="sm"
            onClick={handleInjectScript}
            disabled={isLoading || !isWebviewReady}
            title="注入自动跳转脚本"
            className="text-blue-600 border-blue-300 hover:bg-blue-50"
          >
            🚀
          </Button>
          <Button
            variant="outline"
            size="sm"
            onClick={handleOpenExternal}
            title="在外部浏览器打开"
          >
            <ExternalLink className="w-4 h-4" />
          </Button>
        </div>
      </div>

      {/* 状态显示 */}
      <div className="flex items-center justify-between px-3 py-2 bg-blue-50 border-b text-sm">
        <div className="flex items-center gap-2">
          <div className={`w-2 h-2 rounded-full ${isWebviewReady ? 'bg-green-500' : 'bg-gray-400'}`}></div>
          <span className="text-gray-700">
            {isWebviewReady ? '浏览器已就绪' : '正在初始化浏览器...'}
          </span>
        </div>
        {webviewLabel && (
          <div className="text-xs text-gray-500">
            WebView: {webviewLabel}
          </div>
        )}
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

      {/* 浏览器说明 */}
      <div className="flex-1 flex items-center justify-center bg-gradient-to-br from-blue-50 to-green-50">
        <div className="text-center max-w-md p-8">
          <Monitor className="w-16 h-16 mx-auto mb-4 text-blue-500" />
          <h3 className="text-xl font-semibold text-gray-900 mb-2">原生浏览器窗口</h3>
          <p className="text-gray-600 mb-4">
            浏览器已在独立窗口中打开，提供完整的网页浏览体验。
          </p>
          <div className="space-y-2 text-sm text-gray-500">
            <div className="flex items-center justify-center gap-2">
              <div className="w-1 h-1 bg-blue-500 rounded-full"></div>
              <span>支持完整的 JavaScript 执行</span>
            </div>
            <div className="flex items-center justify-center gap-2">
              <div className="w-1 h-1 bg-green-500 rounded-full"></div>
              <span>自动处理 Cookie 和会话</span>
            </div>
            <div className="flex items-center justify-center gap-2">
              <div className="w-1 h-1 bg-purple-500 rounded-full"></div>
              <span>原生性能，无代理延迟</span>
            </div>
          </div>
          
          {isWebviewReady && (
            <div className="mt-6 p-4 bg-white rounded-lg border border-blue-200">
              <div className="text-sm text-blue-800">
                <div className="font-medium mb-1">💡 使用提示</div>
                <div className="text-left space-y-1">
                  <div>• 点击 🚀 按钮注入自动跳转脚本</div>
                  <div>• 使用地址栏直接导航到其他页面</div>
                  <div>• 浏览器窗口支持完整的网页功能</div>
                </div>
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
