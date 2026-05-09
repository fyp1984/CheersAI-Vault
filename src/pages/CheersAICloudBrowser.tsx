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

      // 等待3秒让webview完全加载，然后注入自动同步脚本
      setTimeout(async () => {
        if (!mountedRef.current || attemptId !== mountAttemptRef.current) {
          return;
        }

        try {
          console.log('[FileBay Auto-Sync] Preparing to inject auto-sync script...');
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
          console.log('[FileBay Auto-Sync] Script injected successfully');
        } catch (error) {
          console.warn('[FileBay Auto-Sync] Failed to inject script:', error);
          // 脚本注入失败不影响页面使用，只是自动同步功能不可用
        }
      }, 3000); // 等待3秒让页面完全加载

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
