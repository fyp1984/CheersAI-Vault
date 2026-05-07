import { useState, useRef, useEffect } from "react";
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
  CheckCircle,
  Bug,
  RefreshCw
} from "lucide-react";
import { tauriCommands } from "@/lib/tauri";
import { CLOUD_APP_URL } from "@/lib/cloud";

interface EmbeddedBrowserProps {
  initialUrl?: string;
  onUrlChange?: (url: string) => void;
}

export default function EmbeddedBrowser({ 
  initialUrl = CLOUD_APP_URL,
  onUrlChange 
}: EmbeddedBrowserProps) {
  const [currentUrl, setCurrentUrl] = useState(initialUrl);
  const [inputUrl, setInputUrl] = useState(initialUrl);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [pageContent, setPageContent] = useState<string>("");
  const [canGoBack, setCanGoBack] = useState(false);
  const [canGoForward, setCanGoForward] = useState(false);
  const [history, setHistory] = useState<string[]>([initialUrl]);
  const [historyIndex, setHistoryIndex] = useState(0);
  const [loadStatus, setLoadStatus] = useState<string>("");
  const [showDebugInfo, setShowDebugInfo] = useState(false);
  const [debugInfo, setDebugInfo] = useState<string>("");
  const [showRawContent, setShowRawContent] = useState(false);
  const iframeRef = useRef<HTMLIFrameElement>(null);

  const loadPage = async (url: string) => {
    setIsLoading(true);
    setError(null);
    setLoadStatus("正在连接服务器...");
    
    try {
      console.log("Loading page:", url);
      
      // 使用代理获取页面内容
      const response = await tauriCommands.fetchWebpage(url);
      setLoadStatus("正在处理页面内容...");
      
      if (response.status === 200) {
        console.log("Raw page content (first 1000 chars):", response.content.substring(0, 1000));
        console.log("Content type:", response.contentType);
        
        // 处理页面内容，注入基础样式和脚本
        const processedContent = processPageContent(response.content, url);
        setPageContent(processedContent);
        setCurrentUrl(url);
        setInputUrl(url);
        setLoadStatus("页面加载完成");
        
        // 更新历史记录
        const newHistory = history.slice(0, historyIndex + 1);
        if (newHistory[newHistory.length - 1] !== url) {
          newHistory.push(url);
          setHistory(newHistory);
          setHistoryIndex(newHistory.length - 1);
        }
        
        // 更新导航状态
        setCanGoBack(historyIndex > 0 || newHistory.length > 1);
        setCanGoForward(false);
        
        onUrlChange?.(url);
        
        // 分析页面内容
        const debugResult = analyzePageContent(response.content, url);
        setDebugInfo(debugResult);
        
        analyzePageContent(response.content, url);
        
      } else {
        setError(`加载失败: HTTP ${response.status}`);
        setLoadStatus(`加载失败: ${response.status}`);
      }
    } catch (err) {
      console.error("Failed to load page:", err);
      const errorMsg = `加载失败: ${err}`;
      setError(errorMsg);
      setLoadStatus(errorMsg);
    } finally {
      setIsLoading(false);
    }
  };

  const analyzePageContent = (content: string, url: string) => {
    console.log("=== Page Content Analysis ===");
    console.log("URL:", url);
    console.log("Content length:", content.length);
    
    let debugResult = `=== 页面分析结果 ===\n`;
    debugResult += `URL: ${url}\n`;
    debugResult += `内容长度: ${content.length} 字符\n\n`;
    
    // 查找所有链接
    const linkMatches = [...content.matchAll(/<a[^>]*href="([^"]*)"[^>]*>([^<]*)<\/a>/gi)];
    console.log("Found links:", linkMatches.map(m => ({ href: m[1], text: m[2] })));
    debugResult += `找到链接 ${linkMatches.length} 个:\n`;
    linkMatches.forEach((m, i) => {
      debugResult += `  ${i + 1}. ${m[2]} -> ${m[1]}\n`;
    });
    debugResult += '\n';
    
    // 查找所有按钮
    const buttonMatches = [...content.matchAll(/<button[^>]*>([^<]*)<\/button>/gi)];
    console.log("Found buttons:", buttonMatches.map(m => m[1]));
    debugResult += `找到按钮 ${buttonMatches.length} 个:\n`;
    buttonMatches.forEach((m, i) => {
      debugResult += `  ${i + 1}. ${m[1]}\n`;
    });
    debugResult += '\n';
    
    // 查找所有表单
    const formMatches = [...content.matchAll(/<form[^>]*action="([^"]*)"[^>]*>/gi)];
    console.log("Found forms:", formMatches.map(m => m[1]));
    debugResult += `找到表单 ${formMatches.length} 个:\n`;
    formMatches.forEach((m, i) => {
      debugResult += `  ${i + 1}. action="${m[1]}"\n`;
    });
    debugResult += '\n';
    
    // 查找脚本标签
    const scriptMatches = [...content.matchAll(/<script[^>]*>(.*?)<\/script>/gis)];
    console.log("Found scripts count:", scriptMatches.length);
    debugResult += `找到脚本 ${scriptMatches.length} 个\n\n`;
    
    // 查找可能的跳转逻辑
    const redirectPatterns = [
      /window\.location\s*=\s*["']([^"']+)["']/gi,
      /location\.href\s*=\s*["']([^"']+)["']/gi,
      /window\.location\.href\s*=\s*["']([^"']+)["']/gi,
      /document\.location\s*=\s*["']([^"']+)["']/gi
    ];
    
    debugResult += `跳转逻辑分析:\n`;
    redirectPatterns.forEach((pattern, index) => {
      const matches = [...content.matchAll(pattern)];
      if (matches.length > 0) {
        console.log(`Redirect pattern ${index + 1} matches:`, matches.map(m => m[1]));
        debugResult += `  模式 ${index + 1}: 找到 ${matches.length} 个匹配\n`;
        matches.forEach((m, i) => {
          debugResult += `    ${i + 1}. ${m[1]}\n`;
        });
      }
    });
    
    // 查找 meta refresh
    const metaRefreshMatch = content.match(/<meta[^>]*http-equiv="refresh"[^>]*content="[^"]*url=([^"]*)"[^>]*>/i);
    if (metaRefreshMatch) {
      console.log("Found meta refresh:", metaRefreshMatch[1]);
      debugResult += `\nMeta Refresh: ${metaRefreshMatch[1]}\n`;
    }
    
    // 查找页面标题
    const titleMatch = content.match(/<title>([^<]*)<\/title>/i);
    if (titleMatch) {
      debugResult += `\n页面标题: ${titleMatch[1]}\n`;
    }
    
    console.log("=== End Analysis ===");
    debugResult += `\n=== 分析完成 ===`;
    
    return debugResult;
  };

  const processPageContent = (content: string, baseUrl: string): string => {
    const baseUrlObj = new URL(baseUrl);
    const baseHref = `${baseUrlObj.protocol}//${baseUrlObj.host}`;
    
    console.log("Processing page content, original length:", content.length);
    console.log("Base URL:", baseHref);
    
    // 如果内容很短，可能是一个简单的页面，直接处理
    if (content.length < 5000) {
      console.log("Short content detected, applying minimal processing");
      
      // 简单处理短内容
      let processedContent = content
        // 确保有基本的 HTML 结构
        .replace(/<!DOCTYPE[^>]*>/i, '<!DOCTYPE html>')
        // 添加 viewport meta
        .replace('<head>', '<head><meta name="viewport" content="width=device-width, initial-scale=1">')
        // 添加 base 标签
        .replace('<head>', `<head><base href="${baseHref}">`)
        // 修复相对路径
        .replace(/src="\/([^"]*?)"/g, `src="${baseHref}/$1"`)
        .replace(/href="\/([^"]*?)"/g, `href="${baseHref}/$1"`)
        .replace(/url\(\/([^)]*?)\)/g, `url(${baseHref}/$1)`);
      
      // 如果没有 body 标签，添加一个
      if (!processedContent.includes('<body')) {
        processedContent = processedContent.replace('</head>', '</head><body>');
        processedContent += '</body></html>';
      }
      
      // 添加基本样式
      const basicStyles = `
        <style>
          body { 
            margin: 0; 
            padding: 20px; 
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            line-height: 1.6;
            background: #f9f9f9;
          }
          .container, .login-container, .auth-container, .form-container {
            max-width: 500px;
            margin: 50px auto;
            padding: 30px;
            border: 1px solid #e1e5e9;
            border-radius: 12px;
            box-shadow: 0 4px 20px rgba(0,0,0,0.1);
            background: white;
          }
          .btn, button, input[type="submit"] {
            padding: 12px 24px;
            border-radius: 6px;
            border: 1px solid #ddd;
            background: #007bff;
            color: white;
            cursor: pointer;
            transition: all 0.2s;
            font-size: 14px;
          }
          .btn:hover, button:hover, input[type="submit"]:hover {
            background: #0056b3;
            transform: translateY(-1px);
          }
          input[type="text"], input[type="email"], input[type="password"] {
            width: 100%;
            padding: 12px;
            border: 1px solid #ddd;
            border-radius: 6px;
            margin: 8px 0;
            box-sizing: border-box;
            font-size: 14px;
          }
          .error { color: #dc3545; }
          .success { color: #28a745; }
          h1, h2, h3 { color: #333; margin-bottom: 16px; }
          form { margin: 20px 0; }
          label { display: block; margin-bottom: 8px; font-weight: 500; }
        </style>
      `;
      
      processedContent = processedContent.replace('</head>', basicStyles + '</head>');
      
      console.log("Processed content length:", processedContent.length);
      return processedContent;
    }
    
    // 对于长内容，使用原来的复杂处理逻辑
    let injectedContent = content
      // 添加 base 标签
      .replace('<head>', `<head><base href="${baseHref}">`)
      // 修复相对路径
      .replace(/src="\/([^"]*?)"/g, `src="${baseHref}/$1"`)
      .replace(/href="\/([^"]*?)"/g, `href="${baseHref}/$1"`)
      .replace(/url\(\/([^)]*?)\)/g, `url(${baseHref}/$1)`)
      // 移除可能的框架限制
      .replace(/X-Frame-Options/gi, 'X-Frame-Options-Disabled')
      .replace(/frame-ancestors/gi, 'frame-ancestors-disabled');

    // 注入自动跳转和交互脚本（只对长内容）
    const injectedScript = `
      <script>
        console.log('CheersAI Browser: Page loaded at', window.location.href);
        console.log('Document title:', document.title);
        console.log('Document ready state:', document.readyState);
        
        // 等待页面完全加载
        function waitForPageLoad(callback) {
          if (document.readyState === 'complete') {
            callback();
          } else {
            window.addEventListener('load', callback);
          }
        }
        
        // 简化的自动跳转逻辑（因为我们已经在登录页面了）
        function attemptAutoRedirect() {
          console.log('CheersAI Browser: Analyzing login page...');
          
          // 查找登录表单
          const forms = document.querySelectorAll('form');
          console.log('Found forms:', forms.length);
          
          const inputs = document.querySelectorAll('input');
          console.log('Found inputs:', inputs.length);
          
          const buttons = document.querySelectorAll('button, input[type="submit"]');
          console.log('Found buttons:', buttons.length);
        }
        
        waitForPageLoad(() => {
          console.log('Login page fully loaded');
          setTimeout(attemptAutoRedirect, 1000);
        });
        
        // 拦截表单提交
        document.addEventListener('submit', function(e) {
          const form = e.target;
          if (form.action) {
            console.log('Form submitted to:', form.action);
            // 让表单正常提交，不拦截
          }
        });
      </script>
    `;
    
    // 注入样式
    const injectedStyle = `
      <style>
        body { 
          margin: 0; 
          padding: 20px; 
          font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
          line-height: 1.6;
        }
        .login-container, .auth-container, .form-container {
          max-width: 500px;
          margin: 50px auto;
          padding: 30px;
          border: 1px solid #e1e5e9;
          border-radius: 12px;
          box-shadow: 0 4px 20px rgba(0,0,0,0.1);
          background: white;
        }
        .btn, button, input[type="submit"] {
          padding: 10px 20px;
          border-radius: 6px;
          border: 1px solid #ddd;
          background: #007bff;
          color: white;
          cursor: pointer;
          transition: all 0.2s;
        }
        .btn:hover, button:hover, input[type="submit"]:hover {
          background: #0056b3;
          transform: translateY(-1px);
        }
        input[type="text"], input[type="email"], input[type="password"] {
          width: 100%;
          padding: 12px;
          border: 1px solid #ddd;
          border-radius: 6px;
          margin: 8px 0;
          box-sizing: border-box;
        }
        .error { color: #dc3545; }
        .success { color: #28a745; }
      </style>
    `;
    
    // 插入脚本和样式
    injectedContent = injectedContent.replace('</head>', injectedScript + injectedStyle + '</head>');
    
    return injectedContent;
  };

  const handleNavigate = (url: string) => {
    if (url && url.trim()) {
      const fullUrl = url.startsWith('http') ? url : `https://${url}`;
      loadPage(fullUrl);
    }
  };

  const handleGoBack = () => {
    if (canGoBack && historyIndex > 0) {
      const newIndex = historyIndex - 1;
      const url = history[newIndex];
      setHistoryIndex(newIndex);
      setCanGoBack(newIndex > 0);
      setCanGoForward(true);
      loadPage(url);
    }
  };

  const handleGoForward = () => {
    if (canGoForward && historyIndex < history.length - 1) {
      const newIndex = historyIndex + 1;
      const url = history[newIndex];
      setHistoryIndex(newIndex);
      setCanGoForward(newIndex < history.length - 1);
      setCanGoBack(true);
      loadPage(url);
    }
  };

  const handleRefresh = () => {
    loadPage(currentUrl);
  };

  const handleHome = () => {
    loadPage(initialUrl);
  };

  const handleOpenExternal = () => {
    window.open(currentUrl, '_blank');
  };

  const handleToggleDebug = () => {
    setShowDebugInfo(!showDebugInfo);
  };

  const handleForceRedirect = async () => {
    if (iframeRef.current) {
      try {
        handleNavigate('https://uat-desktop.cheersai.cloud/signin/');
      } catch (error) {
        console.error("Failed to force redirect:", error);
      }
    }
  };

  // 监听来自 iframe 的消息
  useEffect(() => {
    const handleMessage = (event: MessageEvent) => {
      console.log("Received message from iframe:", event.data);
      
      if (event.data.type === 'navigate') {
        handleNavigate(event.data.url);
      } else if (event.data.type === 'form_submit') {
        // 处理表单提交
        console.log("Form submit:", event.data);
        if (event.data.method === 'GET') {
          handleNavigate(event.data.url);
        } else {
          // POST 请求需要特殊处理
          handleNavigate(event.data.url);
        }
      } else if (event.data.type === 'url_changed') {
        setCurrentUrl(event.data.url);
        setInputUrl(event.data.url);
        onUrlChange?.(event.data.url);
      }
    };

    window.addEventListener('message', handleMessage);
    return () => window.removeEventListener('message', handleMessage);
  }, []);

  // 初始加载
  useEffect(() => {
    loadPage(initialUrl);
  }, []);

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
        
        <Button
          variant="outline"
          size="sm"
          onClick={handleToggleDebug}
          title="显示调试信息"
          className={showDebugInfo ? "bg-blue-50 border-blue-300" : ""}
        >
          <Bug className="w-4 h-4" />
        </Button>
        
        <Button
          variant="outline"
          size="sm"
          onClick={handleForceRedirect}
          title="回到 Desktop 登录页"
          className="text-orange-600 border-orange-300 hover:bg-orange-50"
        >
          <RefreshCw className="w-4 h-4" />
        </Button>
      </div>

      {/* 状态栏 */}
      {(isLoading || error || loadStatus) && (
        <div className={`flex items-center gap-2 p-2 text-sm border-b ${
          error ? 'bg-red-50 text-red-700' : 
          isLoading ? 'bg-blue-50 text-blue-700' : 
          'bg-green-50 text-green-700'
        }`}>
          {error ? (
            <AlertCircle className="w-4 h-4" />
          ) : isLoading ? (
            <Loader2 className="w-4 h-4 animate-spin" />
          ) : (
            <CheckCircle className="w-4 h-4" />
          )}
          <span>{error || loadStatus}</span>
          {error && (
            <Button
              variant="outline"
              size="sm"
              onClick={() => setError(null)}
              className="ml-auto h-6 px-2 text-xs"
            >
              关闭
            </Button>
          )}
        </div>
      )}

      {/* 调试信息面板 */}
      {showDebugInfo && debugInfo && (
        <div className="border-b bg-gray-50 p-3 max-h-40 overflow-y-auto">
          <div className="flex items-center justify-between mb-2">
            <h4 className="font-medium text-gray-900">页面调试信息</h4>
            <Button
              variant="outline"
              size="sm"
              onClick={() => setShowDebugInfo(false)}
              className="h-6 px-2 text-xs"
            >
              关闭
            </Button>
          </div>
          <pre className="text-xs text-gray-600 whitespace-pre-wrap font-mono">
            {debugInfo}
          </pre>
        </div>
      )}

      {/* 原始内容显示面板 */}
      {showRawContent && pageContent && (
        <div className="border-b bg-gray-50 p-3 max-h-60 overflow-y-auto">
          <div className="flex items-center justify-between mb-2">
            <h4 className="font-medium text-gray-900">页面原始内容</h4>
            <Button
              variant="outline"
              size="sm"
              onClick={() => setShowRawContent(false)}
              className="h-6 px-2 text-xs"
            >
              关闭
            </Button>
          </div>
          <pre className="text-xs text-gray-600 whitespace-pre-wrap font-mono bg-white p-2 rounded border">
            {pageContent.substring(0, 2000)}
            {pageContent.length > 2000 && "\n\n... (内容已截断，总长度: " + pageContent.length + " 字符)"}
          </pre>
        </div>
      )}

      {/* 页面内容 */}
      <div className="flex-1 relative">
        {pageContent ? (
          <iframe
            ref={iframeRef}
            srcDoc={pageContent}
            className="w-full h-full border-0"
            sandbox="allow-scripts allow-forms allow-same-origin allow-popups allow-popups-to-escape-sandbox allow-top-navigation allow-modals allow-downloads"
            title="Embedded Browser"
            onLoad={() => {
              console.log("Iframe loaded successfully");
              // 检查 iframe 内容
              try {
                const iframeDoc = iframeRef.current?.contentDocument;
                if (iframeDoc) {
                  console.log("Iframe document body:", iframeDoc.body?.innerHTML?.substring(0, 500));
                  console.log("Iframe document title:", iframeDoc.title);
                }
              } catch (e) {
                console.log("Cannot access iframe content:", e);
              }
            }}
            onError={(e) => {
              console.error("Iframe error:", e);
              setError("页面渲染失败");
            }}
          />
        ) : (
          <div className="flex items-center justify-center h-full text-gray-500">
            <div className="text-center">
              <div className="text-lg mb-2">准备加载页面</div>
              <div className="text-sm">请稍候...</div>
            </div>
          </div>
        )}
        
        {/* 如果页面内容为空或显示有问题，显示备用内容 */}
        {pageContent && currentUrl.includes('/login') && (
          <div className="absolute top-4 right-4 bg-blue-50 border border-blue-200 rounded-lg p-3 max-w-sm">
            <div className="text-sm text-blue-800">
              <div className="font-medium mb-1">登录页面已加载</div>
              <div className="text-xs text-blue-600">
                如果页面显示空白，请尝试：
                <br />• 点击刷新按钮
                <br />• 使用外部浏览器打开
              </div>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
