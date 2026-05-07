import { useEffect, useState } from "react";
import { tauriCommands } from "@/lib/tauri";
import { useAppStore } from "@/store/appStore";

export default function CheersAICloudBrowser() {
  const { sidebarCollapsed } = useAppStore();
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [retryCount, setRetryCount] = useState(0);

  useEffect(() => {
    console.log('🎯 CheersAICloudBrowser 页面已加载');
    
    let cancelled = false;
    let loadingTimeout: NodeJS.Timeout;

    const mountDesktopWorkspace = async () => {
      try {
        setLoading(true);
        setError(null);
        
        console.log('🚀 开始挂载 Desktop 工作区...');
        
        // 设置超时保护（15秒）
        loadingTimeout = setTimeout(() => {
          if (loading && !cancelled) {
            console.warn('⚠️ 加载超时，但继续尝试...');
            setLoading(false); // 移除加载指示器，让用户看到可能已经加载的内容
          }
        }, 15000);
        
        await tauriCommands.ensureDesktopChildWebview();
        console.log('✅ Desktop webview 已创建');
        
        await tauriCommands.updateDesktopChildWebviewBounds();
        console.log('✅ Desktop webview 边界已更新');
        
        // 等待一小段时间让 webview 开始加载
        await new Promise(resolve => setTimeout(resolve, 1000));
        
        // 标记加载完成
        if (!cancelled) {
          clearTimeout(loadingTimeout);
          setLoading(false);
        }
        
        // 等待页面加载完成后，注入自动同步脚本
        setTimeout(async () => {
          if (cancelled) return;
          
          try {
            console.log('⏰ 准备注入自动同步脚本...');
            const autoSyncScript = `
              (function() {
                // 清除所有旧版本标记
                delete window.__filebay_display_v2__;
                delete window.__filebay_display_v3__;
                delete window.__filebay_display_v4__;
                delete window.__filebay_autosync_v2__;
                
                // 使用新版本号
                if (window.__filebay_autosync_v3__) {
                  console.log('%c⚠️ 自动同步脚本 v3 已存在，跳过注入', 'color: #f59e0b;');
                  return;
                }
                window.__filebay_autosync_v3__ = true;
                
                console.log('%c=== 🎯 FileBay 自动同步 v3 已启动 ===', 'color: #2563eb; font-size: 16px; font-weight: bold;');
                
                // 监听登录成功事件
                let lastSyncedConfig = null;
                
                // 拦截 fetch 请求
                const originalFetch = window.fetch;
                window.fetch = function(...args) {
                  const requestUrl = args[0];
                  
                  // 执行原始请求
                  return originalFetch.apply(this, args).then(response => {
                    // 安全地获取 URL 字符串
                    let urlStr = '';
                    try {
                      if (typeof requestUrl === 'string') {
                        urlStr = requestUrl;
                      } else if (requestUrl && typeof requestUrl === 'object') {
                        if (requestUrl instanceof URL) {
                          urlStr = requestUrl.href;
                        } else if (requestUrl.url) {
                          urlStr = requestUrl.url;
                        } else {
                          urlStr = String(requestUrl);
                        }
                      }
                    } catch (e) {
                      return response;
                    }
                    
                    // 检测登录成功（登录 API 返回成功）
                    const isLoginSuccess = urlStr && (
                      urlStr.includes('/api/auth/login') || 
                      urlStr.includes('/api/auth/signin') ||
                      urlStr.includes('/console/api/auth/login')
                    ) && response.ok;
                    
                    // 检测 FileBay 配置 API
                    const isConfigApi = urlStr && (
                      urlStr.includes('/console/api/gitea/config') || 
                      urlStr.includes('/console/api/account/gitea-settings')
                    );
                    
                    // 检测下载配置文件的请求（拦截下载）
                    const isDownloadConfig = urlStr && (
                      urlStr.includes('/console/api/gitea/download-config') ||
                      (urlStr.includes('/download') && urlStr.includes('config'))
                    );
                    
                    if (isLoginSuccess) {
                      console.log('%c🎉 检测到登录成功，准备同步 FileBay 配置...', 'color: #10b981; font-weight: bold;');
                      
                      // 延迟 2 秒后获取配置（等待登录完成）
                      setTimeout(() => {
                        fetch('/console/api/gitea/config', {
                          method: 'GET',
                          headers: {
                            'Content-Type': 'application/json'
                          },
                          credentials: 'include'
                        }).then(r => r.json()).then(data => {
                          syncConfig(data);
                        }).catch(err => {
                          console.error('%c❌ 获取配置失败:', 'color: #dc2626;', err);
                        });
                      }, 2000);
                    }
                    
                    if (isConfigApi && response.ok) {
                      const clonedResponse = response.clone();
                      clonedResponse.json().then(data => {
                        console.log('%c🔍 [DEBUG] API 返回的原始数据:', 'color: #8b5cf6; font-weight: bold;', data);
                        syncConfig(data);
                      }).catch(err => {
                        console.error('%c⚠️ 解析配置失败:', 'color: #ef4444;', err);
                      });
                    }
                    
                    // 拦截下载配置文件的请求
                    if (isDownloadConfig && response.ok) {
                      console.log('%c🎯 检测到下载配置文件请求，正在解析...', 'color: #f59e0b; font-weight: bold;');
                      const clonedResponse = response.clone();
                      
                      clonedResponse.text().then(text => {
                        try {
                          // 尝试解析 JSON
                          const configData = JSON.parse(text);
                          console.log('%c✅ 成功解析配置文件内容', 'color: #10b981;');
                          console.log('%c🔍 [DEBUG] 下载配置文件的原始数据:', 'color: #8b5cf6; font-weight: bold;', configData);
                          
                          // 从配置文件中提取信息
                          const config = {
                            url: configData.url || configData.gitea_url || 'https://uat-filebay.cheersai.cloud',
                            username: configData.username || configData.gitea_owner || '',
                            repo_name: configData.repoName || configData.repo_name || configData.gitea_repo || 'workspace',
                            email: configData.email || '',
                            token: configData.token || configData.gitea_token || ''
                          };
                          
                          console.log('%c🔍 [DEBUG] 提取后的配置:', 'color: #8b5cf6; font-weight: bold;', config);
                          
                          // 直接同步，不检查是否加密
                          syncConfigWithPlainToken(config);
                        } catch (parseError) {
                          console.error('%c❌ 解析配置文件失败:', 'color: #dc2626;', parseError);
                        }
                      }).catch(err => {
                        console.error('%c⚠️ 读取配置文件内容失败:', 'color: #ef4444;', err);
                      });
                    }
                    
                    return response;
                  });
                };
                
                // 同步配置到脱敏程序（带明文 token）
                function syncConfigWithPlainToken(config) {
                  try {
                    console.log('%c━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━', 'color: #2563eb; font-weight: bold;');
                    console.log('%c🔄 正在自动同步 FileBay 配置...', 'color: #2563eb; font-size: 14px; font-weight: bold;');
                    console.log('%c👤 用户:', 'color: #059669;', config.username);
                    console.log('%c📁 仓库:', 'color: #059669;', config.repo_name);
                    console.log('%c🔑 Token 长度:', 'color: #059669;', config.token ? config.token.length + ' 字符' : '未获取');
                    console.log('%c🔑 完整 Token:', 'color: #059669; font-weight: bold;', config.token || '未获取');
                    
                    // 直接调用 Tauri 命令同步配置，不做任何检查
                    window.__TAURI_INTERNALS__.invoke('sync_filebay_config_from_desktop', {
                      url: config.url,
                      token: config.token,
                      owner: config.username,
                      repo: config.repo_name
                    }).then((result) => {
                      console.log('%c✅ FileBay 配置已自动同步到脱敏程序！', 'color: #10b981; font-weight: bold;');
                      console.log('%c📋 同步结果:', 'color: #3b82f6;', result);
                      console.log('%c💡 提示: 配置已成功同步', 'color: #3b82f6;');
                      console.log('%c━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━', 'color: #2563eb; font-weight: bold;');
                    }).catch(err => {
                      console.error('%c❌ 同步配置失败:', 'color: #dc2626;', err);
                      console.log('%c━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━', 'color: #2563eb; font-weight: bold;');
                    });
                    
                  } catch (error) {
                    console.error('%c❌ 处理配置失败:', 'color: #dc2626;', error);
                  }
                }
                
                // 同步配置到脱敏程序
                function syncConfig(data) {
                  try {
                    console.log('%c🔍 [DEBUG] syncConfig 接收到的数据:', 'color: #8b5cf6; font-weight: bold;', data);
                    
                    const config = {
                      url: data.gitea_url || 'https://uat-filebay.cheersai.cloud',
                      username: data.gitea_owner || '',
                      repo_name: data.gitea_repo || 'workspace',
                      email: data.email || '',
                      token: data.gitea_token || ''
                    };
                    
                    console.log('%c🔍 [DEBUG] 提取后的配置:', 'color: #8b5cf6; font-weight: bold;', config);
                    
                    // 直接同步配置，不管 Token 是否加密
                    syncConfigWithPlainToken(config);
                    
                  } catch (error) {
                    console.error('%c❌ 处理配置失败:', 'color: #dc2626;', error);
                  }
                }
                
                console.log('%c✅ 自动同步已激活！', 'color: #10b981; font-size: 14px; font-weight: bold;');
                console.log('%c💡 提示: 登录成功或点击"下载配置文件"时会自动同步', 'color: #3b82f6;');
              })();
            `;
            
            await tauriCommands.evalJsInDesktopWebview(autoSyncScript);
            console.log('✅ 自动同步脚本已注入到 Desktop 页面');
            console.log('💡 登录成功或下载配置文件时会自动同步 FileBay 配置');
          } catch (error) {
            console.error('❌ 注入脚本失败:', error);
            // 脚本注入失败不影响页面使用，只是自动同步功能不可用
          }
        }, 3000); // 等待3秒让页面完全加载
        
      } catch (error) {
        if (!cancelled) {
          console.error("❌ 挂载 Desktop 工作区失败:", error);
          clearTimeout(loadingTimeout);
          setError(`加载失败: ${error}`);
          setLoading(false);
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
      if (loadingTimeout) {
        clearTimeout(loadingTimeout);
      }
      window.removeEventListener("resize", handleResize);
      void tauriCommands.hideDesktopChildWebview();
    };
  }, [retryCount]); // 添加 retryCount 作为依赖，以便重试时重新执行

  useEffect(() => {
    void tauriCommands.updateDesktopChildWebviewBounds();
  }, [sidebarCollapsed]);

  const handleRetry = () => {
    setRetryCount(prev => prev + 1);
  };

  return (
    <div className="h-full bg-slate-50 relative">
      {/* 加载状态 */}
      {loading && (
        <div className="absolute inset-0 flex items-center justify-center bg-gradient-to-br from-blue-50 to-indigo-50 z-10">
          <div className="text-center">
            <div className="inline-flex items-center justify-center w-24 h-24 mb-6">
              <img 
                src="/safer.png" 
                alt="CheersAI" 
                className="w-full h-full object-contain animate-pulse"
                style={{ animationDuration: '1.5s' }}
              />
            </div>
            <h3 className="text-2xl font-bold text-gray-900 mb-3">正在加载 CheersAI Desktop</h3>
            <p className="text-gray-600 mb-2">连接到云端工作区...</p>
            <div className="flex items-center justify-center gap-2 mt-4">
              <div className="w-2 h-2 bg-blue-600 rounded-full animate-bounce" style={{ animationDelay: '0ms' }}></div>
              <div className="w-2 h-2 bg-blue-600 rounded-full animate-bounce" style={{ animationDelay: '150ms' }}></div>
              <div className="w-2 h-2 bg-blue-600 rounded-full animate-bounce" style={{ animationDelay: '300ms' }}></div>
            </div>
          </div>
        </div>
      )}
      
      {/* 错误状态 */}
      {error && (
        <div className="absolute inset-0 flex items-center justify-center bg-gradient-to-br from-red-50 to-orange-50 z-10">
          <div className="text-center max-w-md mx-4 bg-white rounded-2xl shadow-xl p-8">
            <div className="inline-flex items-center justify-center w-20 h-20 mb-4 bg-red-100 rounded-full">
              <svg className="w-10 h-10 text-red-600" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 8v4m0 4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
              </svg>
            </div>
            <h3 className="text-xl font-bold text-gray-900 mb-2">加载失败</h3>
            <p className="text-gray-600 mb-6 text-sm">{error}</p>
            <div className="flex gap-3 justify-center">
              <button
                onClick={handleRetry}
                className="px-6 py-2.5 bg-blue-600 text-white rounded-lg hover:bg-blue-700 transition-colors font-medium shadow-sm"
              >
                重试
              </button>
              <button
                onClick={() => window.history.back()}
                className="px-6 py-2.5 bg-gray-200 text-gray-700 rounded-lg hover:bg-gray-300 transition-colors font-medium"
              >
                返回
              </button>
            </div>
            {retryCount > 0 && (
              <p className="text-xs text-gray-500 mt-4">已重试 {retryCount} 次</p>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
