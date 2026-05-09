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
    pub token: String,
    pub user_id: Option<String>,
}

/// 从 Desktop WebView 中提取配置
#[tauri::command]
pub async fn extract_config_from_desktop_webview(app: AppHandle) -> Result<String, String> {
    println!("=== Extracting config from Desktop WebView ===");
    
    // 查找 Desktop 子 webview（使用 get_webview 而不是 get_webview_window）
    let webview = app.get_webview("desktop_child")
        .ok_or_else(|| "Desktop 窗口未打开，请先打开 CheersAI 页面".to_string())?;
    
    println!("✓ Found desktop_child webview");
    
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
                    
                    // 通过 Tauri 事件发送配置
                    if (window.__TAURI__) {
                        await window.__TAURI__.event.emit('desktop-config-extracted', config);
                        console.log('[AutoSync] Config sent to Tauri');
                    }
                    
                    // 同时通过 Vault API 直接同步
                    try {
                        const response = await fetch('http://localhost:7788/api/v1/filebay/config', {
                            method: 'POST',
                            headers: {'Content-Type': 'application/json'},
                            body: JSON.stringify(config)
                        });
                        const result = await response.json();
                        console.log('[AutoSync] Synced to Vault API:', result);
                        
                        // 显示成功消息
                        const msg = `配置已自动同步！\n\n用户: ${config.username}\n邮箱: ${config.email}\n仓库: ${config.repo_name}`;
                        console.log('[AutoSync] ' + msg);
                        
                        // 尝试显示通知（可能会被阻止）
                        try {
                            if (window.Notification && Notification.permission === 'granted') {
                                new Notification('配置同步成功', { body: msg });
                            }
                        } catch(e) {}
                        
                    } catch (apiError) {
                        console.error('[AutoSync] Failed to sync to Vault API:', apiError);
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
    
    println!("Injecting JavaScript to extract and sync config...");
    
    // 执行 JavaScript
    webview.eval(js_code)
        .map_err(|e| format!("执行 JavaScript 失败: {}", e))?;
    
    Ok("配置提取脚本已注入，正在从 Desktop 页面提取配置...".to_string())
}

/// 从 Desktop WebView 中执行 JavaScript 并获取结果
#[tauri::command]
pub async fn eval_js_in_desktop_webview(
    app: AppHandle,
    js_code: String,
) -> Result<String, String> {
    println!("=== Evaluating JavaScript in Desktop WebView ===");
    
    // 查找 Desktop 子 webview（使用 get_webview 而不是 get_webview_window）
    let webview = app.get_webview("desktop_child")
        .ok_or_else(|| "Desktop 窗口未打开，请先打开 CheersAI 页面".to_string())?;
    
    println!("✓ Found desktop_child webview");
    println!("Executing JavaScript:\n{}", js_code);
    
    // 执行 JavaScript
    webview.eval(&js_code)
        .map_err(|e| format!("执行 JavaScript 失败: {}", e))?;
    
    Ok("JavaScript 已执行，请在 Desktop 页面的控制台查看结果".to_string())
}
