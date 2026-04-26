import { useState, useEffect } from "react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { RefreshCw, ExternalLink, AlertCircle, Globe } from "lucide-react";
import { tauriCommands } from "@/lib/tauri";
import { CLOUD_APP_URL } from "@/lib/cloud";

export default function CheersAICloud() {
  const [isLoading, setIsLoading] = useState(true);
  const [hasError, setHasError] = useState(false);
  const [errorMessage, setErrorMessage] = useState("");
  const [htmlContent, setHtmlContent] = useState("");
  const cloudUrl = CLOUD_APP_URL;

  const loadContent = async () => {
    setIsLoading(true);
    setHasError(false);
    setErrorMessage("");

    try {
      console.log("Fetching webpage through proxy...");
      const response = await tauriCommands.fetchWebpage(cloudUrl);
      console.log("Proxy response:", response);
      
      if (response.status === 200) {
        setHtmlContent(response.content);
        setIsLoading(false);
      } else {
        throw new Error(`HTTP ${response.status}`);
      }
    } catch (error) {
      console.error("Failed to load content:", error);
      setIsLoading(false);
      setHasError(true);
      setErrorMessage(error instanceof Error ? error.message : "未知错误");
    }
  };

  useEffect(() => {
    loadContent();
  }, []);

  const handleRefresh = () => {
    loadContent();
  };

  const handleOpenExternal = async () => {
    try {
      const { open } = await import("@tauri-apps/plugin-shell");
      await open(cloudUrl);
    } catch (error) {
      console.error("Failed to open external link:", error);
      window.open(cloudUrl, '_blank');
    }
  };

  return (
    <div className="flex flex-col h-full bg-gray-50">
      {/* 头部工具栏 */}
      <div className="flex items-center justify-between p-4 bg-white border-b border-gray-200 shadow-sm">
        <div className="flex items-center gap-3">
          <div className="flex items-center justify-center w-8 h-8 bg-gradient-to-br from-green-500 to-green-600 rounded-lg">
            <Globe className="w-4 h-4 text-white" />
          </div>
          <div>
            <h1 className="text-lg font-semibold text-gray-900">CheersAI 云端服务</h1>
            <p className="text-sm text-gray-500">脱敏数据 AI 处理平台 (代理模式)</p>
          </div>
        </div>
        
        <div className="flex items-center gap-2">
          <Button
            variant="outline"
            size="sm"
            onClick={handleRefresh}
            disabled={isLoading}
            className="flex items-center gap-2"
          >
            <RefreshCw className={`w-4 h-4 ${isLoading ? 'animate-spin' : ''}`} />
            刷新
          </Button>
          <Button
            variant="outline"
            size="sm"
            onClick={handleOpenExternal}
            className="flex items-center gap-2"
          >
            <ExternalLink className="w-4 h-4" />
            在浏览器中打开
          </Button>
        </div>
      </div>

      {/* 主要内容区域 */}
      <div className="flex-1 relative">
        {isLoading && (
          <div className="absolute inset-0 flex items-center justify-center bg-white z-10">
            <div className="flex flex-col items-center gap-4">
              <div className="w-8 h-8 border-4 border-green-500 border-t-transparent rounded-full animate-spin"></div>
              <p className="text-gray-600">正在通过代理加载 CheersAI 云端服务...</p>
              <p className="text-sm text-gray-400">绕过嵌入限制，请稍候</p>
            </div>
          </div>
        )}

        {hasError && (
          <div className="absolute inset-0 flex items-center justify-center bg-gradient-to-br from-green-50 to-blue-50 z-10">
            <div className="max-w-2xl mx-4">
              <Card className="shadow-xl border-0">
                <CardHeader className="text-center pb-4">
                  <div className="flex justify-center mb-4">
                    <div className="flex items-center justify-center w-16 h-16 bg-gradient-to-br from-red-500 to-red-600 rounded-2xl shadow-lg">
                      <AlertCircle className="w-8 h-8 text-white" />
                    </div>
                  </div>
                  <CardTitle className="text-2xl text-gray-900">代理加载失败</CardTitle>
                  <p className="text-gray-600 mt-2">无法通过代理服务器获取内容</p>
                </CardHeader>
                <CardContent className="space-y-6">
                  <div className="bg-red-50 border border-red-200 rounded-xl p-4">
                    <div className="flex items-start gap-3">
                      <AlertCircle className="w-5 h-5 text-red-600 mt-0.5 flex-shrink-0" />
                      <div className="text-sm text-red-800">
                        <p className="font-medium mb-2">错误信息</p>
                        <p className="mb-3">{errorMessage}</p>
                        <div className="bg-red-100 rounded-lg p-3">
                          <p className="font-medium mb-1">可能的原因：</p>
                          <ul className="list-disc list-inside space-y-1 text-xs">
                            <li>网站服务器拒绝请求</li>
                            <li>网络连接问题</li>
                            <li>服务器返回了错误状态码</li>
                            <li>内容解析失败</li>
                          </ul>
                        </div>
                      </div>
                    </div>
                  </div>

                  <div className="flex flex-col sm:flex-row gap-3">
                    <Button 
                      onClick={handleOpenExternal} 
                      className="flex-1 bg-gradient-to-r from-green-600 to-green-500 hover:from-green-700 hover:to-green-600 text-white shadow-lg"
                      size="lg"
                    >
                      <ExternalLink className="w-4 h-4 mr-2" />
                      在浏览器中打开
                    </Button>
                    <Button 
                      variant="outline" 
                      onClick={handleRefresh} 
                      className="flex-1"
                      size="lg"
                    >
                      <RefreshCw className="w-4 h-4 mr-2" />
                      重新尝试
                    </Button>
                  </div>
                </CardContent>
              </Card>
            </div>
          </div>
        )}

        {!isLoading && !hasError && htmlContent && (
          <div className="w-full h-full">
            <iframe
              srcDoc={htmlContent}
              className="w-full h-full border-0"
              title="CheersAI 云端服务"
              sandbox="allow-same-origin allow-scripts allow-forms allow-popups allow-popups-to-escape-sandbox allow-top-navigation"
            />
          </div>
        )}
      </div>

      {/* 底部状态栏 */}
      <div className="px-4 py-2 bg-white border-t border-gray-200 text-xs text-gray-500">
        <div className="flex items-center justify-between">
          <span>云端服务地址: {cloudUrl} (代理模式)</span>
          <div className="flex items-center gap-2">
            <div className={`w-2 h-2 rounded-full ${hasError ? 'bg-red-400' : isLoading ? 'bg-yellow-400 animate-pulse' : 'bg-green-400 animate-pulse'}`}></div>
            <span>{hasError ? '连接失败' : isLoading ? '加载中...' : '已连接'}</span>
          </div>
        </div>
      </div>
    </div>
  );
}
