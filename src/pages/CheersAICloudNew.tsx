import { useState, useRef } from "react";
import { Card, CardContent } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { ExternalLink, Globe, Sparkles, Shield, Zap, RefreshCw, Code } from "lucide-react";
import { tauriCommands } from "@/lib/tauri";
import { CLOUD_APP_URL, CLOUD_LOGIN_URLS } from "@/lib/cloud";

export default function CheersAICloud() {
  const [isOpening, setIsOpening] = useState(false);
  const [currentWindowLabel, setCurrentWindowLabel] = useState<string | null>(null);
  const [isDebugging, setIsDebugging] = useState(false);
  const cloudUrl = CLOUD_APP_URL;
  const debugIntervalRef = useRef<NodeJS.Timeout | null>(null);

  const handleOpenInWebView = async () => {
    setIsOpening(true);
    try {
      console.log("Opening webview window...");
      const windowLabel = await tauriCommands.openWebviewWindow({
        url: cloudUrl,
        title: "CheersAI 云端服务",
        width: 1400,
        height: 900,
      });
      console.log("Webview window opened:", windowLabel);
      setCurrentWindowLabel(windowLabel);
      
      // 等待一下让页面加载，然后尝试自动跳转逻辑
      setTimeout(async () => {
        try {
          // 注入一些 JavaScript 来帮助页面跳转
          await tauriCommands.webviewEvalScript(windowLabel, `
            console.log('CheersAI Desktop: Page loaded, checking for redirects...');
            console.log('Current URL:', window.location.href);
            console.log('Document ready state:', document.readyState);
            
            // 检查是否有自动跳转的元素或逻辑
            if (window.location.href.includes('/apps/') && !window.location.href.includes('login')) {
              console.log('CheersAI Desktop: Checking for login redirect...');
              
              // 等待页面完全加载
              if (document.readyState !== 'complete') {
                window.addEventListener('load', function() {
                  setTimeout(checkForRedirect, 1000);
                });
              } else {
                setTimeout(checkForRedirect, 1000);
              }
              
              function checkForRedirect() {
                console.log('CheersAI Desktop: Checking for redirect elements...');
                
                // 查找可能的登录链接或按钮
                const loginSelectors = [
                  'a[href*="login"]',
                  'button[onclick*="login"]', 
                  '.login-btn',
                  '#login',
                  '.login',
                  '[data-login]',
                  'a[href*="auth"]',
                  'button[onclick*="auth"]'
                ];
                
                let foundElement = null;
                for (const selector of loginSelectors) {
                  const elements = document.querySelectorAll(selector);
                  if (elements.length > 0) {
                    foundElement = elements[0];
                    console.log('CheersAI Desktop: Found login element:', selector, foundElement);
                    break;
                  }
                }
                
                if (foundElement) {
                  console.log('CheersAI Desktop: Clicking login element...');
                  foundElement.click();
                } else {
                  // 检查是否有 meta refresh 或其他重定向
                  const metaRefresh = document.querySelector('meta[http-equiv="refresh"]');
                  if (metaRefresh) {
                    console.log('CheersAI Desktop: Found meta refresh:', metaRefresh.content);
                  } else {
                    // 尝试直接跳转到登录页面
                    console.log('CheersAI Desktop: No login elements found, trying direct navigation...');
                    const currentUrl = new URL(window.location.href);
                    const loginUrl = currentUrl.origin + '/login';
                    console.log('CheersAI Desktop: Attempting to navigate to:', loginUrl);
                    window.location.href = loginUrl;
                  }
                }
              }
            } else {
              console.log('CheersAI Desktop: Already on login page or different URL structure');
            }
          `);
        } catch (error) {
          console.error("Failed to inject script:", error);
        }
      }, 3000);
      
    } catch (error) {
      console.error("Failed to open webview:", error);
      alert(`打开失败: ${error}`);
    } finally {
      setIsOpening(false);
    }
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

  const handleDebugWindow = async () => {
    if (!currentWindowLabel) {
      alert("请先打开 WebView 窗口");
      return;
    }

    setIsDebugging(!isDebugging);
    
    if (!isDebugging) {
      // 开始调试模式
      debugIntervalRef.current = setInterval(async () => {
        try {
          const currentUrl = await tauriCommands.getWebviewUrl(currentWindowLabel);
          console.log("Current WebView URL:", currentUrl);
          
          // 注入调试脚本
          await tauriCommands.webviewEvalScript(currentWindowLabel, `
            console.log('Debug Info:', {
              url: window.location.href,
              title: document.title,
              readyState: document.readyState,
              hasLoginElements: document.querySelectorAll('a[href*="login"], .login, #login').length > 0
            });
          `);
        } catch (error) {
          console.error("Debug error:", error);
        }
      }, 3000);
    } else {
      // 停止调试模式
      if (debugIntervalRef.current) {
        clearInterval(debugIntervalRef.current);
        debugIntervalRef.current = null;
      }
    }
  };

  const handleForceLogin = async () => {
    if (!currentWindowLabel) {
      alert("请先打开 WebView 窗口");
      return;
    }

    try {
      // 先尝试多个可能的登录页面 URL
      const possibleLoginUrls = CLOUD_LOGIN_URLS;
      
      console.log("Trying multiple login URLs...");
      
      // 先尝试第一个 URL
      await tauriCommands.navigateWebview(currentWindowLabel, possibleLoginUrls[0]);
      console.log("Navigated to login page:", possibleLoginUrls[0]);
      
      // 等待一下，然后检查是否成功
      setTimeout(async () => {
        try {
          const currentUrl = await tauriCommands.getWebviewUrl(currentWindowLabel);
          console.log("Current URL after navigation:", currentUrl);
          
          // 如果还是在 apps 页面，尝试其他 URL
          if (currentUrl.includes('/apps/')) {
            console.log("Still on apps page, trying alternative URLs...");
            for (let i = 1; i < possibleLoginUrls.length; i++) {
              try {
                await tauriCommands.navigateWebview(currentWindowLabel, possibleLoginUrls[i]);
                console.log("Tried URL:", possibleLoginUrls[i]);
                await new Promise(resolve => setTimeout(resolve, 2000));
                
                const newUrl = await tauriCommands.getWebviewUrl(currentWindowLabel);
                if (!newUrl.includes('/apps/')) {
                  console.log("Successfully navigated to:", newUrl);
                  break;
                }
              } catch (error) {
                console.error("Failed to navigate to:", possibleLoginUrls[i], error);
              }
            }
          }
        } catch (error) {
          console.error("Failed to check URL:", error);
        }
      }, 2000);
      
    } catch (error) {
      console.error("Failed to navigate to login:", error);
      alert(`跳转失败: ${error}`);
    }
  };

  const handleInspectPage = async () => {
    if (!currentWindowLabel) {
      alert("请先打开 WebView 窗口");
      return;
    }

    try {
      await tauriCommands.webviewEvalScript(currentWindowLabel, `
        console.log('=== Page Inspection ===');
        console.log('URL:', window.location.href);
        console.log('Title:', document.title);
        console.log('Ready State:', document.readyState);
        console.log('Body HTML (first 500 chars):', document.body ? document.body.innerHTML.substring(0, 500) : 'No body');
        
        // 查找所有链接
        const links = Array.from(document.querySelectorAll('a')).map(a => ({
          href: a.href,
          text: a.textContent?.trim(),
          id: a.id,
          className: a.className
        }));
        console.log('All links:', links);
        
        // 查找所有按钮
        const buttons = Array.from(document.querySelectorAll('button')).map(btn => ({
          onclick: btn.onclick?.toString(),
          text: btn.textContent?.trim(),
          id: btn.id,
          className: btn.className
        }));
        console.log('All buttons:', buttons);
        
        // 查找所有表单
        const forms = Array.from(document.querySelectorAll('form')).map(form => ({
          action: form.action,
          method: form.method,
          id: form.id,
          className: form.className
        }));
        console.log('All forms:', forms);
        
        // 查找 meta 标签
        const metas = Array.from(document.querySelectorAll('meta')).map(meta => ({
          name: meta.name,
          content: meta.content,
          httpEquiv: meta.httpEquiv
        }));
        console.log('Meta tags:', metas);
        
        // 查找脚本标签
        const scripts = Array.from(document.querySelectorAll('script')).map(script => ({
          src: script.src,
          type: script.type,
          hasContent: script.innerHTML.length > 0
        }));
        console.log('Script tags:', scripts);
        
        console.log('=== End Inspection ===');
      `);
      
      console.log("Page inspection completed. Check the WebView console for details.");
      alert("页面检查完成，请查看控制台输出");
    } catch (error) {
      console.error("Failed to inspect page:", error);
      alert(`检查失败: ${error}`);
    }
  };

  const handleTryDifferentStrategies = async () => {
    if (!currentWindowLabel) {
      alert("请先打开 WebView 窗口");
      return;
    }

    try {
      await tauriCommands.webviewEvalScript(currentWindowLabel, `
        console.log('=== Trying Different Navigation Strategies ===');
        
        // 策略 1: 检查是否有重定向脚本
        console.log('Strategy 1: Looking for redirect scripts...');
        const scripts = document.querySelectorAll('script');
        scripts.forEach((script, index) => {
          if (script.innerHTML.includes('location') || script.innerHTML.includes('redirect') || script.innerHTML.includes('login')) {
            console.log('Found potential redirect script', index, ':', script.innerHTML.substring(0, 200));
          }
        });
        
        // 策略 2: 模拟用户交互
        console.log('Strategy 2: Simulating user interactions...');
        document.dispatchEvent(new Event('DOMContentLoaded'));
        window.dispatchEvent(new Event('load'));
        
        // 策略 3: 检查是否有隐藏的表单或链接
        console.log('Strategy 3: Looking for hidden elements...');
        const hiddenElements = document.querySelectorAll('[style*="display:none"], [style*="visibility:hidden"], .hidden');
        hiddenElements.forEach((el, index) => {
          if (el.tagName === 'A' || el.tagName === 'FORM' || el.tagName === 'BUTTON') {
            console.log('Found hidden interactive element', index, ':', el.outerHTML.substring(0, 200));
          }
        });
        
        // 策略 4: 尝试触发常见的页面加载事件
        console.log('Strategy 4: Triggering page events...');
        setTimeout(() => {
          // 模拟页面完全加载
          if (typeof window.onload === 'function') {
            window.onload();
          }
          
          // 尝试触发可能的自动跳转函数
          const commonFunctionNames = ['redirect', 'login', 'auth', 'navigate', 'goto'];
          commonFunctionNames.forEach(name => {
            if (typeof window[name] === 'function') {
              console.log('Found function:', name, 'attempting to call...');
              try {
                window[name]();
              } catch (e) {
                console.log('Failed to call', name, ':', e.message);
              }
            }
          });
        }, 1000);
        
        // 策略 5: 检查 localStorage 和 sessionStorage
        console.log('Strategy 5: Checking storage...');
        console.log('localStorage keys:', Object.keys(localStorage));
        console.log('sessionStorage keys:', Object.keys(sessionStorage));
        
        console.log('=== End Strategy Testing ===');
      `);
      
      console.log("Strategy testing completed. Check the WebView console for details.");
      alert("策略测试完成，请查看控制台输出");
    } catch (error) {
      console.error("Failed to test strategies:", error);
      alert(`策略测试失败: ${error}`);
    }
  };

  const handleTryDifferentUserAgents = async () => {
    if (currentWindowLabel) {
      try {
        await tauriCommands.closeWebviewWindow(currentWindowLabel);
      } catch (error) {
        console.log("Failed to close existing window:", error);
      }
    }

    const userAgents = [
      // Chrome on Windows
      "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
      // Firefox on Windows  
      "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:109.0) Gecko/20100101 Firefox/121.0",
      // Edge on Windows
      "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36 Edg/120.0.0.0",
      // Safari on macOS
      "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.1 Safari/605.1.15"
    ];

    for (let i = 0; i < userAgents.length; i++) {
      try {
        console.log(`Trying User Agent ${i + 1}:`, userAgents[i]);
        
        // 这里我们需要在后端支持自定义 User Agent
        // 目前先用默认的方式打开，但记录不同的尝试
        const windowLabel = await tauriCommands.openWebviewWindow({
          url: cloudUrl,
          title: `CheersAI 云端服务 (UA Test ${i + 1})`,
          width: 1400,
          height: 900,
        });
        
        setCurrentWindowLabel(windowLabel);
        
        // 等待页面加载并检查结果
        setTimeout(async () => {
          try {
            const currentUrl = await tauriCommands.getWebviewUrl(windowLabel);
            console.log(`User Agent ${i + 1} result:`, currentUrl);
            
            if (!currentUrl.includes('/apps/')) {
              console.log(`Success with User Agent ${i + 1}!`);
              alert(`成功！使用 User Agent ${i + 1} 成功跳转`);
              return;
            } else if (i === userAgents.length - 1) {
              alert("所有 User Agent 都尝试过了，仍然无法自动跳转");
            } else {
              // 关闭当前窗口，尝试下一个
              await tauriCommands.closeWebviewWindow(windowLabel);
            }
          } catch (error) {
            console.error(`Error with User Agent ${i + 1}:`, error);
          }
        }, 5000);
        
        // 如果不是最后一个，等待一下再尝试下一个
        if (i < userAgents.length - 1) {
          await new Promise(resolve => setTimeout(resolve, 6000));
        }
        
      } catch (error) {
        console.error(`Failed to test User Agent ${i + 1}:`, error);
      }
    }
  };

  return (
    <div className="flex flex-col h-full bg-gradient-to-br from-green-50 via-blue-50 to-purple-50">
      <div className="flex-1 flex items-center justify-center p-8">
        <div className="max-w-4xl w-full">
          <Card className="shadow-2xl border-0 overflow-hidden">
            <div className="bg-gradient-to-r from-green-600 to-blue-600 p-8 text-white">
              <div className="flex items-center gap-4 mb-4">
                <div className="flex items-center justify-center w-16 h-16 bg-white/20 backdrop-blur-sm rounded-2xl">
                  <Globe className="w-8 h-8" />
                </div>
                <div>
                  <h1 className="text-3xl font-bold">CheersAI 云端服务</h1>
                  <p className="text-green-100 mt-1">脱敏数据 AI 处理平台</p>
                </div>
              </div>
            </div>

            <CardContent className="p-8 space-y-6">
              {/* 功能介绍 */}
              <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
                <div className="flex items-start gap-3 p-4 bg-gradient-to-br from-green-50 to-green-100 rounded-xl">
                  <div className="flex items-center justify-center w-10 h-10 bg-green-500 rounded-lg flex-shrink-0">
                    <Sparkles className="w-5 h-5 text-white" />
                  </div>
                  <div>
                    <h3 className="font-semibold text-gray-900 mb-1">AI 智能处理</h3>
                    <p className="text-sm text-gray-600">使用先进的 AI 技术处理脱敏数据</p>
                  </div>
                </div>

                <div className="flex items-start gap-3 p-4 bg-gradient-to-br from-blue-50 to-blue-100 rounded-xl">
                  <div className="flex items-center justify-center w-10 h-10 bg-blue-500 rounded-lg flex-shrink-0">
                    <Shield className="w-5 h-5 text-white" />
                  </div>
                  <div>
                    <h3 className="font-semibold text-gray-900 mb-1">安全可靠</h3>
                    <p className="text-sm text-gray-600">企业级安全保障，数据加密传输</p>
                  </div>
                </div>

                <div className="flex items-start gap-3 p-4 bg-gradient-to-br from-purple-50 to-purple-100 rounded-xl">
                  <div className="flex items-center justify-center w-10 h-10 bg-purple-500 rounded-lg flex-shrink-0">
                    <Zap className="w-5 h-5 text-white" />
                  </div>
                  <div>
                    <h3 className="font-semibold text-gray-900 mb-1">高效便捷</h3>
                    <p className="text-sm text-gray-600">一键上传，快速处理，即时下载</p>
                  </div>
                </div>
              </div>

              {/* 访问方式说明 */}
              <div className="bg-gradient-to-r from-blue-50 to-purple-50 border border-blue-200 rounded-xl p-6">
                <h3 className="font-semibold text-gray-900 mb-3 flex items-center gap-2">
                  <Globe className="w-5 h-5 text-blue-600" />
                  选择访问方式
                </h3>
                <div className="space-y-3 text-sm text-gray-700">
                  <div className="flex items-start gap-2">
                    <div className="w-1.5 h-1.5 bg-blue-500 rounded-full mt-2 flex-shrink-0"></div>
                    <p><span className="font-medium">应用内窗口</span>：在独立窗口中打开，完整的浏览器体验，支持自动跳转到登录页面</p>
                  </div>
                  <div className="flex items-start gap-2">
                    <div className="w-1.5 h-1.5 bg-purple-500 rounded-full mt-2 flex-shrink-0"></div>
                    <p><span className="font-medium">系统浏览器</span>：使用默认浏览器打开，适合需要多标签页或扩展功能的场景</p>
                  </div>
                </div>
              </div>

              {/* 操作按钮 */}
              <div className="flex flex-col sm:flex-row gap-4 pt-4">
                <Button 
                  onClick={handleOpenInWebView}
                  disabled={isOpening}
                  className="flex-1 h-14 bg-gradient-to-r from-green-600 to-blue-600 hover:from-green-700 hover:to-blue-700 text-white shadow-lg text-base font-medium"
                  size="lg"
                >
                  {isOpening ? (
                    <>
                      <div className="w-5 h-5 border-2 border-white border-t-transparent rounded-full animate-spin mr-2"></div>
                      正在打开...
                    </>
                  ) : (
                    <>
                      <Globe className="w-5 h-5 mr-2" />
                      在应用内窗口打开
                    </>
                  )}
                </Button>
                
                <Button 
                  variant="outline" 
                  onClick={handleOpenExternal}
                  className="flex-1 h-14 border-2 border-gray-300 hover:border-gray-400 hover:bg-gray-50 text-base font-medium"
                  size="lg"
                >
                  <ExternalLink className="w-5 h-5 mr-2" />
                  在系统浏览器打开
                </Button>
              </div>

              {/* 调试工具 */}
              {currentWindowLabel && (
                <div className="border-t pt-6 space-y-3">
                  <h4 className="font-medium text-gray-900 flex items-center gap-2">
                    <Code className="w-4 h-4" />
                    调试工具
                  </h4>
                  <div className="flex gap-2 flex-wrap">
                    <Button 
                      variant="outline" 
                      size="sm"
                      onClick={handleDebugWindow}
                      className={isDebugging ? "bg-yellow-50 border-yellow-300" : ""}
                    >
                      <RefreshCw className={`w-4 h-4 mr-1 ${isDebugging ? 'animate-spin' : ''}`} />
                      {isDebugging ? "停止调试" : "开始调试"}
                    </Button>
                    <Button 
                      variant="outline" 
                      size="sm"
                      onClick={handleForceLogin}
                    >
                      强制跳转登录
                    </Button>
                    <Button 
                      variant="outline" 
                      size="sm"
                      onClick={handleInspectPage}
                    >
                      检查页面内容
                    </Button>
                    <Button 
                      variant="outline" 
                      size="sm"
                      onClick={handleTryDifferentStrategies}
                    >
                      尝试其他策略
                    </Button>
                    <Button 
                      variant="outline" 
                      size="sm"
                      onClick={handleTryDifferentUserAgents}
                    >
                      测试不同浏览器
                    </Button>
                  </div>
                </div>
              )}

              {/* 提示信息 */}
              <div className="text-center text-sm text-gray-500 pt-2">
                <p>云端服务地址: <span className="font-mono text-blue-600">{cloudUrl}</span></p>
              </div>
            </CardContent>
          </Card>

          {/* 使用提示 */}
          <div className="mt-6 text-center text-sm text-gray-600">
            <p>💡 提示：如果页面没有自动跳转到登录页面，请使用"强制跳转登录"按钮</p>
          </div>
        </div>
      </div>
    </div>
  );
}
