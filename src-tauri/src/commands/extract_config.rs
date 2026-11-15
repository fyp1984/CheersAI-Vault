/// 从 Desktop 在线工作区的 WebView 中提取配置
///
/// 通过执行 JavaScript 代码从 Desktop 页面中提取当前用户的配置信息
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, WebviewWindow};

#[derive(Debug, Serialize, Deserialize)]
pub struct ExtractedConfig {
    pub username: String,
    pub email: String,
    pub repo_name: String,
    pub url: String,
    pub user_id: Option<String>,
}

/// 从 Desktop WebView 中提取配置
#[tauri::command]
pub async fn extract_config_from_desktop_webview(app: AppHandle) -> Result<String, String> {
    // 查找 Desktop 子 webview（使用 get_webview 而不是 get_webview_window）
    let webview = app
        .get_webview("desktop_child")
        .ok_or_else(|| "Desktop 窗口未打开，请先打开 CheersAI 页面".to_string())?;

    // 注入 JavaScript 代码，让它通过 Tauri 事件发送配置
    let js_code = r#"
        (async function() {
            try {
                console.log('[AutoSync] Starting config extraction...');
                
                // 尝试多种方式提取配置
                let config = null;
                
                // 方法 1: localStorage
                const filebayStr = localStorage.getItem('filebay_config');
                const accountStr = localStorage.getItem('account');
                const userStr = localStorage.getItem('user');
                
                if (filebayStr) {
                    const filebay = JSON.parse(filebayStr);
                    config = {
                        username: filebay.username,
                        email: filebay.email,
                        repo_name: filebay.repo_name || 'workspace',
                        url: filebay.url || 'https://uat-filebay.cheersai.cloud',
                        token: filebay.token || '',
                        user_id: filebay.user_id
                    };
                } else if (accountStr) {
                    const account = JSON.parse(accountStr);
                    config = {
                        username: account.name || account.username || account.email?.split('@')[0],
                        email: account.email,
                        repo_name: 'workspace',
                        url: 'https://uat-filebay.cheersai.cloud',
                        token: '',
                        user_id: account.id
                    };
                } else if (userStr) {
                    const user = JSON.parse(userStr);
                    config = {
                        username: user.name || user.username || user.email?.split('@')[0],
                        email: user.email,
                        repo_name: 'workspace',
                        url: 'https://uat-filebay.cheersai.cloud',
                        token: user.token || '',
                        user_id: user.id
                    };
                }
                
                if (config) {
                    console.log('[AutoSync] Config extracted:', {
                        username: config.username,
                        email: config.email,
                        repo_name: config.repo_name
                    });
                    
                    // 只发送非敏感元数据；Token 直接交给后端凭据迁移命令。
                    if (window.__TAURI__) {
                        await window.__TAURI__.event.emit('desktop-config-extracted', {
                            username: config.username,
                            email: config.email,
                            repo_name: config.repo_name,
                            url: config.url,
                            user_id: config.user_id
                        });
                    }

                    if (window.__TAURI_INTERNALS__?.invoke) {
                        await window.__TAURI_INTERNALS__.invoke('sync_filebay_config_from_desktop', {
                            url: config.url,
                            token: config.token,
                            owner: config.username,
                            repo: config.repo_name
                        });
                    }
                } else {
                    console.error('[AutoSync] No config found in localStorage');
                    throw new Error('未找到用户配置');
                }
                
            } catch (error) {
                console.error('[AutoSync] Error:', error);
                if (window.__TAURI__) {
                    await window.__TAURI__.event.emit('desktop-config-error', {
                        error: error.message || String(error)
                    });
                }
            }
        })();
    "#;

    // 执行 JavaScript
    webview
        .eval(js_code)
        .map_err(|e| format!("执行 JavaScript 失败: {}", e))?;

    Ok("配置提取脚本已注入，正在从 Desktop 页面提取配置...".to_string())
}

/// 从 Desktop WebView 中执行 JavaScript 并获取结果
#[tauri::command]
pub async fn eval_js_in_desktop_webview(app: AppHandle, js_code: String) -> Result<String, String> {
    // 查找 Desktop 子 webview（使用 get_webview 而不是 get_webview_window）
    let webview = app
        .get_webview("desktop_child")
        .ok_or_else(|| "Desktop 窗口未打开，请先打开 CheersAI 页面".to_string())?;

    // 执行 JavaScript
    webview
        .eval(&js_code)
        .map_err(|e| format!("执行 JavaScript 失败: {}", e))?;

    Ok("JavaScript 已执行，请在 Desktop 页面的控制台查看结果".to_string())
}
