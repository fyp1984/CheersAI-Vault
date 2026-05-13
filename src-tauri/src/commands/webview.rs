use tauri::{AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, WebviewUrl, WebviewWindowBuilder, Window, webview::WebviewBuilder};
use tauri::WebviewWindow;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::collections::HashMap;
use tokio::sync::{Mutex as TokioMutex, oneshot};
use base64::Engine as _;

const CLOUD_HOST: &str = "uat-desktop.cheersai.cloud";

// 全局导航锁，防止重复触发
static NAVIGATION_IN_PROGRESS: AtomicBool = AtomicBool::new(false);
#[cfg(not(target_os = "macos"))]
static DESKTOP_CHILD_MONITOR_RUNNING: AtomicBool = AtomicBool::new(false);

// 编译时嵌入 64x64 缩略图
const SAFER_ICON_BYTES: &[u8] = include_bytes!("../../safer_small.png");
const DESKTOP_CHILD_LABEL: &str = "desktop_child";
const DESKTOP_URL: &str = "https://uat-desktop.cheersai.cloud/?source=vault-shell";
// Keep the embedded Desktop webview aligned with the Vault top header height.
const CONTENT_HEADER_HEIGHT: f64 = 120.0;

fn desktop_brand_hider_script() -> &'static str {
    r#"
        function hideDesktopBrand() {
            try {
                var anchors = Array.from(document.querySelectorAll('a[href="/apps/"], a[href="/apps"]'));
                anchors.forEach(function(el) {
                    var text = (el.textContent || '').replace(/\s+/g, ' ').trim();
                    var rect = typeof el.getBoundingClientRect === 'function'
                        ? el.getBoundingClientRect()
                        : { top: 999, left: 999, width: 0, height: 0 };
                    var hasBrandImage = !!el.querySelector('img[alt*="CheersAI"], img[src*="CheersAI"]');
                    var isTopLeftBrand = rect.top < 140 && rect.left < 320 && rect.width < 260 && hasBrandImage;
                    var hasBrandText =
                        text.indexOf('DESKTOP') !== -1 ||
                        text.indexOf('CHEERSAI') !== -1 ||
                        text.indexOf('智享AI，安全随行') !== -1;
                    if (isTopLeftBrand && hasBrandText) {
                        el.style.display = 'none';
                    }
                });
            } catch (_err) {}
        }

        hideDesktopBrand();
        if (!window.__cheersai_brand_observer) {
            try {
                var brandObserver = new MutationObserver(function() { hideDesktopBrand(); });
                brandObserver.observe(document.documentElement || document.body, { childList: true, subtree: true });
                window.__cheersai_brand_observer = brandObserver;
            } catch (_err) {}
        }
    "#
}

fn desktop_content_bounds(window: &Window) -> Result<(LogicalPosition<f64>, LogicalSize<f64>), String> {
    let scale = window.scale_factor().map_err(|e| format!("Failed to get scale factor: {}", e))?;
    let size = window.inner_size().map_err(|e| format!("Failed to get window size: {}", e))?;
    let logical_width = size.width as f64 / scale;
    let logical_height = size.height as f64 / scale;
    let sidebar_width = 256.0;
    let content_width = (logical_width - sidebar_width).max(320.0);

    Ok((
        LogicalPosition::new(sidebar_width, CONTENT_HEADER_HEIGHT),
        LogicalSize::new(content_width, (logical_height - CONTENT_HEADER_HEIGHT).max(320.0)),
    ))
}

fn desktop_fab_script() -> String {
    let brand_hider_script = desktop_brand_hider_script();
    let icon_b64 = base64::engine::general_purpose::STANDARD.encode(SAFER_ICON_BYTES);
    let icon_data_url = format!("data:image/png;base64,{}", icon_b64);

    format!(r#"
        (function() {{
            if (window.__cheersai_fab_bootstrapped) return;
            window.__cheersai_fab_bootstrapped = true;
            window.__cheersai_nav_lock = false;
            {brand_hider_script}

            function inject() {{
                var href = window.location.href;
                if (href.indexOf('uat-desktop.cheersai.cloud') === -1 &&
                    href.indexOf('uat-sso.cheersai.cloud') === -1) return;
                if (document.getElementById('__cheersai_fab')) return;

                var fab = document.createElement('div');
                fab.id = '__cheersai_fab';
                fab.style.cssText = 'position:fixed;top:40%;right:-24px;z-index:2147483647;' +
                    'display:flex;align-items:center;cursor:pointer;user-select:none;' +
                    'transition:right 0.25s cubic-bezier(0.4,0,0.2,1),opacity 0.25s ease,box-shadow 0.25s ease;' +
                    'opacity:0.7;';

                var circle = document.createElement('div');
                circle.style.cssText = 'width:56px;height:56px;min-width:56px;border-radius:50%;' +
                    'background:#fff;box-shadow:-2px 2px 12px rgba(0,0,0,0.15);' +
                    'display:flex;align-items:center;justify-content:center;overflow:hidden;' +
                    'transition:box-shadow 0.25s ease,transform 0.25s ease;';

                var img = document.createElement('img');
                img.src = '{icon_data_url}';
                img.style.cssText = 'width:40px;height:40px;border-radius:50%;pointer-events:none;';
                img.draggable = false;
                img.onerror = function() {{
                    img.style.display = 'none';
                    fallback.style.display = 'flex';
                }};
                circle.appendChild(img);

                var fallback = document.createElement('div');
                fallback.textContent = 'AI';
                fallback.style.cssText = 'display:none;width:40px;height:40px;border-radius:50%;' +
                    'align-items:center;justify-content:center;background:#2563eb;color:#fff;' +
                    'font-size:12px;font-weight:700;pointer-events:none;' +
                    'font-family:-apple-system,BlinkMacSystemFont,Segoe UI,Roboto,sans-serif;';
                circle.appendChild(fallback);
                fab.appendChild(circle);

                var label = document.createElement('div');
                label.textContent = '\u6570\u636E\u5B89\u5168\u5C4B';
                label.style.cssText = 'margin-right:8px;padding:6px 14px;border-radius:20px;' +
                    'background:#fff;color:#1e40af;font-size:13px;font-weight:600;white-space:nowrap;' +
                    'box-shadow:0 2px 10px rgba(0,0,0,0.12);pointer-events:none;' +
                    'opacity:0;transform:translateX(8px);' +
                    'transition:opacity 0.2s ease,transform 0.2s ease;' +
                    'font-family:-apple-system,BlinkMacSystemFont,Segoe UI,Roboto,sans-serif;';
                fab.insertBefore(label, circle);

                var hideTimer = null;
                var isDragging = false;

                function snapToEdge() {{
                    if (window.__cheersai_nav_lock || isDragging) return;
                    fab.style.right = '-24px';
                    fab.style.opacity = '0.7';
                    circle.style.boxShadow = '-2px 2px 12px rgba(0,0,0,0.15)';
                    circle.style.transform = 'scale(1)';
                    label.style.opacity = '0';
                    label.style.transform = 'translateX(8px)';
                }}
                function slideOut() {{
                    if (window.__cheersai_nav_lock || isDragging) return;
                    clearTimeout(hideTimer);
                    fab.style.right = '12px';
                    fab.style.opacity = '1';
                    circle.style.boxShadow = '0 6px 24px rgba(59,130,246,0.3)';
                    circle.style.transform = 'scale(1.08)';
                    label.style.opacity = '1';
                    label.style.transform = 'translateX(0)';
                }}
                function startHideTimer() {{
                    clearTimeout(hideTimer);
                    hideTimer = setTimeout(snapToEdge, 2000);
                }}

                fab.addEventListener('mouseenter', function() {{ slideOut(); }});
                fab.addEventListener('mouseleave', function() {{ startHideTimer(); }});

                var startY = 0, startTop = 0;
                fab.addEventListener('mousedown', function(e) {{
                    isDragging = false;
                    startY = e.clientY;
                    startTop = fab.getBoundingClientRect().top;
                    fab.style.transition = 'none';

                    function onMove(ev) {{
                        var dy = ev.clientY - startY;
                        if (Math.abs(dy) > 3) isDragging = true;
                        if (isDragging) {{
                            var newTop = Math.max(20, Math.min(window.innerHeight - 76, startTop + dy));
                            fab.style.top = newTop + 'px';
                        }}
                    }}
                    function onUp() {{
                        document.removeEventListener('mousemove', onMove);
                        document.removeEventListener('mouseup', onUp);
                        fab.style.transition = 'right 0.25s cubic-bezier(0.4,0,0.2,1),opacity 0.25s ease,box-shadow 0.25s ease';
                        if (isDragging) {{
                            startHideTimer();
                            setTimeout(function() {{ isDragging = false; }}, 50);
                        }}
                    }}
                    document.addEventListener('mousemove', onMove);
                    document.addEventListener('mouseup', onUp);
                }});

                setTimeout(function() {{
                    fab.style.right = '12px';
                    fab.style.opacity = '1';
                    label.style.opacity = '1';
                    label.style.transform = 'translateX(0)';
                }}, 300);
                setTimeout(snapToEdge, 2500);

                fab.addEventListener('click', function() {{
                    if (isDragging || window.__cheersai_nav_lock) return;
                    window.__cheersai_nav_lock = true;
                    fab.style.transform = 'scale(0)';
                    fab.style.opacity = '0';
                    try {{
                        var current = window.location.href.split('#')[0];
                        window.history.replaceState(null, '', current + '#__cheersai_return__');
                    }} catch (_err) {{
                        try {{
                            window.location.hash = '__cheersai_return__';
                        }} catch (_ignored) {{}}
                    }}
                }});

                document.body.appendChild(fab);
            }}

            inject();
            setInterval(inject, 1500);
        }})();
    "#, brand_hider_script = brand_hider_script)
}

#[cfg(not(target_os = "macos"))]
fn start_desktop_child_monitor(app: AppHandle) {
    if DESKTOP_CHILD_MONITOR_RUNNING
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return;
    }

    tokio::spawn(async move {
        loop {
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

            let Some(webview) = app.get_webview(DESKTOP_CHILD_LABEL) else {
                DESKTOP_CHILD_MONITOR_RUNNING.store(false, Ordering::SeqCst);
                break;
            };

            let url = match webview.url() {
                Ok(url) => url.to_string(),
                Err(_) => continue,
            };

            if url.contains("#__cheersai_return__") {
                let _ = webview.hide();
                let _ = webview.navigate(DESKTOP_URL.parse().unwrap());
                let _ = app.emit("navigate-to-process", ());
            }
        }
    });
}

#[tauri::command]
pub async fn navigate_to_local(app: AppHandle, return_url: String) -> Result<(), String> {

    if let Some(main_window) = app.get_webview_window("main") {
        let _ = app.emit("navigate-to-process", ());

        let local_url = return_url
            .parse()
            .map_err(|e| format!("Invalid URL: {}", e))?;
        main_window
            .navigate(local_url)
            .map_err(|e| format!("Failed to navigate: {}", e))?;

        Ok(())
    } else {
        Err("Main window not found".to_string())
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WebviewOptions {
    pub url: String,
    pub title: Option<String>,
    pub width: Option<f64>,
    pub height: Option<f64>,
}

#[tauri::command]
pub async fn open_webview_window(
    app: AppHandle,
    options: WebviewOptions,
) -> Result<String, String> {
    let window_label = format!("webview_{}", chrono::Utc::now().timestamp_millis());
    let title = options
        .title
        .unwrap_or_else(|| "CheersAI Desktop · 智享AI，安全随行".to_string());
    let width = options.width.unwrap_or(1200.0);
    let height = options.height.unwrap_or(800.0);

    // 创建新的 WebView 窗口，配置为全屏无边框模式
    let app_clone = app.clone();
    let webview_window = WebviewWindowBuilder::new(
        &app,
        &window_label,
        WebviewUrl::External(options.url.parse().map_err(|e| format!("Invalid URL: {}", e))?)
    )
    .title(&title)
    .inner_size(width, height)
    .center()
    .resizable(true)
    .fullscreen(false)
    .decorations(true)  // 保留标题栏以便关闭窗口
    .maximized(true)    // 启动时最大化
    .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
    .accept_first_mouse(true)
    .on_download(move |_window, download| {
        // 获取应用数据目录下的 downloads 文件夹
        match app_clone.path().app_data_dir() {
            Ok(app_data_dir) => {
                let downloads_dir = app_data_dir.join("downloads");
                
                // 确保 downloads 目录存在
                if let Err(e) = std::fs::create_dir_all(&downloads_dir) {

                    return false;
                }
                
                // 处理下载事件
                match download {
                    tauri::webview::DownloadEvent::Requested { url, destination, .. } => {
                        // 从 URL 提取文件名
                        let file_name = url.path_segments()
                            .and_then(|segments| segments.last())
                            .and_then(|name| if name.is_empty() { None } else { Some(name) })
                            .unwrap_or("download");
                        
                        let file_path = downloads_dir.join(file_name);
                        *destination = file_path.clone();

                        true
                    }
                    tauri::webview::DownloadEvent::Finished { url, path, success } => {
                        if success {

                        } else {

                        }
                        true
                    }
                    _ => true,
                }
            }
            Err(e) => {

                false
            }
        }
    })
    .build()
    .map_err(|e| format!("Failed to create webview window: {}", e))?;

    // 注入浮动返回按钮
    let icon_b64 = base64::engine::general_purpose::STANDARD.encode(SAFER_ICON_BYTES);
    let icon_data_url = format!("data:image/png;base64,{}", icon_b64);
    
    let inject_script = format!(r#"
        (function() {{
            window.__cheersai_nav_lock = window.__cheersai_nav_lock || false;

            if (document.readyState === 'loading') {{
                document.addEventListener('DOMContentLoaded', inject);
            }} else {{
                inject();
            }}

            function inject() {{
                var href = window.location.href;
                console.log('🔍 CheersAI FAB: Checking injection in new window...', href);

                // 只在外部页面注入
                if (href.indexOf('tauri://localhost') !== -1 ||
                    href.indexOf('localhost:1420') !== -1 ||
                    href.indexOf('127.0.0.1') !== -1) {{
                    console.log('❌ CheersAI FAB: Skipped - local page');
                    return;
                }}
                
                // 检查是否已经注入
                if (document.getElementById('__cheersai_fab')) {{
                    console.log('❌ CheersAI FAB: Already injected');
                    return;
                }}
                
                // 简化检查：只跳过明显的登录页面（URL 包含 login/signin）
                // 不再检查密码表单，因为很多页面都有登录表单但不是登录页
                var isLoginPage = href.indexOf('/login') !== -1 || href.indexOf('/signin') !== -1;
                if (isLoginPage) {{
                    console.log('❌ CheersAI FAB: Skipped - login page');
                    return;
                }}

                console.log('✅ CheersAI FAB: Injecting button in new window');

                var fab = document.createElement('div');
                fab.id = '__cheersai_fab';
                fab.style.cssText = 'position:fixed;top:40%;right:0px;z-index:2147483647;' +
                    'display:flex;align-items:center;cursor:pointer;user-select:none;' +
                    'transition:right 0.25s cubic-bezier(0.4,0,0.2,1),opacity 0.25s ease,box-shadow 0.25s ease;' +
                    'opacity:0.85;' +
                    'pointer-events:auto;';

                var circle = document.createElement('div');
                circle.style.cssText = 'width:56px;height:56px;min-width:56px;border-radius:50%;' +
                    'background:#fff;box-shadow:-2px 2px 12px rgba(0,0,0,0.15);' +
                    'display:flex;align-items:center;justify-content:center;overflow:hidden;' +
                    'transition:box-shadow 0.25s ease,transform 0.25s ease;';

                var img = document.createElement('img');
                img.src = '{icon_data_url}';
                img.style.cssText = 'width:40px;height:40px;border-radius:50%;pointer-events:none;';
                img.draggable = false;
                circle.appendChild(img);
                fab.appendChild(circle);

                var label = document.createElement('div');
                label.textContent = '\u6570\u636E\u5B89\u5168\u5C4B';
                label.style.cssText = 'margin-right:12px;padding:8px 16px;background:#fff;' +
                    'border-radius:24px;box-shadow:-2px 2px 12px rgba(0,0,0,0.1);' +
                    'font-size:14px;font-weight:500;color:#1f2937;white-space:nowrap;' +
                    'opacity:0;transform:translateX(8px);' +
                    'transition:opacity 0.2s ease,transform 0.2s ease;';
                fab.insertBefore(label, circle);

                var hideTimer = null;
                var isDragging = false;

                function snapToEdge() {{
                    if (window.__cheersai_nav_lock || isDragging) return;
                    fab.style.right = '0px';
                    fab.style.opacity = '0.85';
                    circle.style.boxShadow = '-2px 2px 12px rgba(0,0,0,0.15)';
                    circle.style.transform = 'scale(1)';
                    label.style.opacity = '0';
                    label.style.transform = 'translateX(8px)';
                }}

                function slideOut() {{
                    if (window.__cheersai_nav_lock || isDragging) return;
                    clearTimeout(hideTimer);
                    fab.style.right = '12px';
                    fab.style.opacity = '1';
                    circle.style.boxShadow = '0 6px 24px rgba(59,130,246,0.3)';
                    circle.style.transform = 'scale(1.08)';
                    label.style.opacity = '1';
                    label.style.transform = 'translateX(0)';
                }}

                function startHideTimer() {{
                    hideTimer = setTimeout(snapToEdge, 2000);
                }}

                fab.addEventListener('mouseenter', function() {{ slideOut(); }});
                fab.addEventListener('mouseleave', function() {{ startHideTimer(); }});

                fab.addEventListener('mousedown', function(e) {{
                    isDragging = false;
                    var startY = e.clientY;
                    var startTop = fab.getBoundingClientRect().top;
                    fab.style.transition = 'none';

                    function onMove(ev) {{
                        var dy = ev.clientY - startY;
                        if (Math.abs(dy) > 3) isDragging = true;
                        if (isDragging) {{
                            var newTop = Math.max(20, Math.min(window.innerHeight - 76, startTop + dy));
                            fab.style.top = newTop + 'px';
                        }}
                    }}
                    function onUp() {{
                        document.removeEventListener('mousemove', onMove);
                        document.removeEventListener('mouseup', onUp);
                        fab.style.transition = 'right 0.25s cubic-bezier(0.4,0,0.2,1),opacity 0.25s ease,box-shadow 0.25s ease';
                        if (isDragging) {{
                            startHideTimer();
                            setTimeout(function() {{ isDragging = false; }}, 50);
                        }}
                    }}
                    document.addEventListener('mousemove', onMove);
                    document.addEventListener('mouseup', onUp);
                }});

                setTimeout(function() {{
                    fab.style.right = '12px';
                    fab.style.opacity = '1';
                    label.style.opacity = '1';
                    label.style.transform = 'translateX(0)';
                }}, 300);
                setTimeout(snapToEdge, 2500);

                fab.addEventListener('click', function(e) {{
                    if (isDragging) return;
                    if (window.__cheersai_nav_lock) return;
                    window.__cheersai_nav_lock = true;
                    fab.style.transform = 'scale(0)';
                    fab.style.opacity = '0';
                    window.close();
                }});

                document.body.appendChild(fab);
                console.log('✅ CheersAI FAB: Button injected successfully in new window!');
                console.log('✅ CheersAI FAB: Button element:', fab);
                console.log('✅ CheersAI FAB: Button position:', fab.getBoundingClientRect());
            }}
        }})();
    "#, icon_data_url = icon_data_url);

    // 启动注入任务
    let window_clone = webview_window.clone();
    let script_clone = inject_script.clone();
    tokio::spawn(async move {
        let mut count = 0u32;
        loop {
            let delay = if count < 3 { 1500 } else { 3000 };
            tokio::time::sleep(tokio::time::Duration::from_millis(delay)).await;
            count += 1;

            let url_str = match window_clone.url() {
                Ok(u) => u.to_string(),
                Err(_) => break,
            };

            if count <= 5 {

            }

            // 只在外部页面注入
            if !url_str.contains("localhost") && !url_str.contains("tauri://") {
                if window_clone.eval(&script_clone).is_ok() {
                    if count <= 3 {

                    }
                } else if count <= 3 {

                }
            }

            if count > 200 {

                break;
            }
        }
    });

    Ok(window_label)
}

#[tauri::command]
pub async fn ensure_desktop_child_webview(
    app: AppHandle,
) -> Result<(), String> {
    let main_window = app.get_window("main").ok_or("Main window not found".to_string())?;
    let (mut position, mut size) = desktop_content_bounds(&main_window)?;
    let desktop_url = DESKTOP_URL
        .parse::<tauri::Url>()
        .map_err(|e| format!("Invalid desktop URL: {}", e))?;

    if let Some(webview) = app.get_webview(DESKTOP_CHILD_LABEL) {
        let _ = webview.set_position(position);
        let _ = webview.set_size(size);
        let _ = webview.show();
        let _ = webview.set_focus();
        #[cfg(target_os = "macos")]
        {
            let _ = webview.navigate(desktop_url.clone());
        }

        #[cfg(not(target_os = "macos"))]
        {
            if let Ok(current_url) = webview.url() {
                let current = current_url.to_string();
                if !current.contains("uat-desktop.cheersai.cloud") && !current.contains("uat-sso.cheersai.cloud") {
                    let _ = webview.navigate(desktop_url);
                }
            }
        }
    } else {
        #[cfg(target_os = "macos")]
        tokio::time::sleep(tokio::time::Duration::from_millis(350)).await;

        let create_child = |position: LogicalPosition<f64>, size: LogicalSize<f64>| {
            let builder = WebviewBuilder::new(
                DESKTOP_CHILD_LABEL,
                WebviewUrl::External(desktop_url.clone()),
            )
            .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/122.0.0.0 Safari/537.36")
            .auto_resize()
            .initialization_script(desktop_fab_script());

            main_window
                .add_child(builder, position, size)
                .map_err(|e| format!("Failed to create desktop child webview: {}", e))
        };

        let webview = match create_child(position, size) {
            Ok(webview) => webview,
            Err(first_error) => {
                #[cfg(target_os = "macos")]
                {
                    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                    let updated_bounds = desktop_content_bounds(&main_window)?;
                    position = updated_bounds.0;
                    size = updated_bounds.1;

                    create_child(position, size).map_err(|retry_error| {
                        format!(
                            "Failed to create desktop child webview after macOS retry: first attempt: {}; retry: {}",
                            first_error, retry_error
                        )
                    })?
                }

                #[cfg(not(target_os = "macos"))]
                {
                    return Err(first_error);
                }
            }
        };

        let _ = webview.show();
        let _ = webview.set_focus();

        #[cfg(not(target_os = "macos"))]
        start_desktop_child_monitor(app.clone());
    }

    Ok(())
}

#[tauri::command]
pub async fn update_desktop_child_webview_bounds(
    app: AppHandle,
) -> Result<(), String> {
    let main_window = app.get_window("main").ok_or("Main window not found".to_string())?;
    let (position, size) = desktop_content_bounds(&main_window)?;

    if let Some(webview) = app.get_webview(DESKTOP_CHILD_LABEL) {
        let _ = webview.set_position(position);
        let _ = webview.set_size(size);
    }

    Ok(())
}

#[tauri::command]
pub async fn hide_desktop_child_webview(
    app: AppHandle,
) -> Result<(), String> {
    if let Some(webview) = app.get_webview(DESKTOP_CHILD_LABEL) {
        let _ = webview.hide();
    }
    Ok(())
}

#[tauri::command]
pub async fn open_desktop_window_with_button(
    app: AppHandle,
    url: String,
) -> Result<(), String> {
    const DESKTOP_WINDOW_LABEL: &str = "desktop_cloud";

    let parsed_url = url.parse().map_err(|e| format!("Invalid URL: {}", e))?;

    let desktop_window = if let Some(existing) = app.get_webview_window(DESKTOP_WINDOW_LABEL) {
        existing
            .navigate(parsed_url)
            .map_err(|e| format!("Failed to navigate desktop window: {}", e))?;
        let _ = existing.show();
        let _ = existing.set_focus();
        existing
    } else {
        let app_clone = app.clone();
        WebviewWindowBuilder::new(
            &app,
            DESKTOP_WINDOW_LABEL,
            WebviewUrl::External(parsed_url),
        )
        .title("CheersAI Desktop · 智享AI，安全随行")
        .inner_size(1400.0, 900.0)
        .center()
        .resizable(true)
        .fullscreen(false)
        .accept_first_mouse(true)
        .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
        .on_download(move |_window, download| {
            // 获取应用数据目录下的 downloads 文件夹
            match app_clone.path().app_data_dir() {
                Ok(app_data_dir) => {
                    let downloads_dir = app_data_dir.join("downloads");
                    
                    // 确保 downloads 目录存在
                    if let Err(e) = std::fs::create_dir_all(&downloads_dir) {

                        return false;
                    }
                    
                    // 使用默认文件名或从 URL 提取
                    let filename = match download {
                        tauri::webview::DownloadEvent::Requested { url, destination, .. } => {
                            // 从 URL 提取文件名
                            let file_name = url.path_segments()
                                .and_then(|segments| segments.last())
                                .and_then(|name| if name.is_empty() { None } else { Some(name) })
                                .unwrap_or("download");
                            
                            let file_path = downloads_dir.join(file_name);
                            *destination = file_path.clone();

                            true
                        }
                        tauri::webview::DownloadEvent::Finished { url, path, success } => {
                            if success {

                            } else {

                            }
                            true
                        }
                        _ => true,
                    };
                    filename
                }
                Err(e) => {

                    false
                }
            }
        })
        .build()
        .map_err(|e| format!("Failed to create desktop window: {}", e))?
    };

    let icon_b64 = base64::engine::general_purpose::STANDARD.encode(SAFER_ICON_BYTES);
    let icon_data_url = format!("data:image/png;base64,{}", icon_b64);

    let brand_hider_script = desktop_brand_hider_script();
    let inject_script = format!(r#"
        (function() {{
            window.__cheersai_nav_lock = window.__cheersai_nav_lock || false;
            {brand_hider_script}

            if (document.readyState === 'loading') {{
                document.addEventListener('DOMContentLoaded', inject);
            }} else {{
                inject();
            }}

            function inject() {{
                var href = window.location.href;
                if (href.indexOf('uat-desktop.cheersai.cloud') === -1 &&
                    href.indexOf('uat-sso.cheersai.cloud') === -1) return;
                if (document.getElementById('__cheersai_fab')) return;

                var fab = document.createElement('div');
                fab.id = '__cheersai_fab';
                fab.style.cssText = 'position:fixed;top:40%;right:-24px;z-index:2147483647;' +
                    'display:flex;align-items:center;cursor:pointer;user-select:none;' +
                    'transition:right 0.25s cubic-bezier(0.4,0,0.2,1),opacity 0.25s ease,box-shadow 0.25s ease;' +
                    'opacity:0.7;';

                var circle = document.createElement('div');
                circle.style.cssText = 'width:56px;height:56px;min-width:56px;border-radius:50%;' +
                    'background:#fff;box-shadow:-2px 2px 12px rgba(0,0,0,0.15);' +
                    'display:flex;align-items:center;justify-content:center;overflow:hidden;' +
                    'transition:box-shadow 0.25s ease,transform 0.25s ease;';

                var img = document.createElement('img');
                img.src = '{icon_data_url}';
                img.style.cssText = 'width:40px;height:40px;border-radius:50%;pointer-events:none;';
                img.draggable = false;
                img.onerror = function() {{
                    img.style.display = 'none';
                    fallback.style.display = 'flex';
                }};
                circle.appendChild(img);

                var fallback = document.createElement('div');
                fallback.textContent = 'AI';
                fallback.style.cssText = 'display:none;width:40px;height:40px;border-radius:50%;' +
                    'align-items:center;justify-content:center;background:#2563eb;color:#fff;' +
                    'font-size:12px;font-weight:700;pointer-events:none;' +
                    'font-family:-apple-system,BlinkMacSystemFont,Segoe UI,Roboto,sans-serif;';
                circle.appendChild(fallback);
                fab.appendChild(circle);

                var label = document.createElement('div');
                label.textContent = '\u6570\u636E\u5B89\u5168\u5C4B';
                label.style.cssText = 'margin-right:8px;padding:6px 14px;border-radius:20px;' +
                    'background:#fff;color:#1e40af;font-size:13px;font-weight:600;white-space:nowrap;' +
                    'box-shadow:0 2px 10px rgba(0,0,0,0.12);pointer-events:none;' +
                    'opacity:0;transform:translateX(8px);' +
                    'transition:opacity 0.2s ease,transform 0.2s ease;' +
                    'font-family:-apple-system,BlinkMacSystemFont,Segoe UI,Roboto,sans-serif;';
                fab.insertBefore(label, circle);

                var hideTimer = null;
                var isDragging = false;

                function snapToEdge() {{
                    if (window.__cheersai_nav_lock || isDragging) return;
                    fab.style.right = '-24px';
                    fab.style.opacity = '0.7';
                    circle.style.boxShadow = '-2px 2px 12px rgba(0,0,0,0.15)';
                    circle.style.transform = 'scale(1)';
                    label.style.opacity = '0';
                    label.style.transform = 'translateX(8px)';
                }}
                function slideOut() {{
                    if (window.__cheersai_nav_lock || isDragging) return;
                    clearTimeout(hideTimer);
                    fab.style.right = '12px';
                    fab.style.opacity = '1';
                    circle.style.boxShadow = '0 6px 24px rgba(59,130,246,0.3)';
                    circle.style.transform = 'scale(1.08)';
                    label.style.opacity = '1';
                    label.style.transform = 'translateX(0)';
                }}
                function startHideTimer() {{
                    clearTimeout(hideTimer);
                    hideTimer = setTimeout(snapToEdge, 2000);
                }}

                fab.addEventListener('mouseenter', function() {{ slideOut(); }});
                fab.addEventListener('mouseleave', function() {{ startHideTimer(); }});

                var startY = 0, startTop = 0;
                fab.addEventListener('mousedown', function(e) {{
                    isDragging = false;
                    startY = e.clientY;
                    startTop = fab.getBoundingClientRect().top;
                    fab.style.transition = 'none';

                    function onMove(ev) {{
                        var dy = ev.clientY - startY;
                        if (Math.abs(dy) > 3) isDragging = true;
                        if (isDragging) {{
                            var newTop = Math.max(20, Math.min(window.innerHeight - 76, startTop + dy));
                            fab.style.top = newTop + 'px';
                        }}
                    }}
                    function onUp() {{
                        document.removeEventListener('mousemove', onMove);
                        document.removeEventListener('mouseup', onUp);
                        fab.style.transition = 'right 0.25s cubic-bezier(0.4,0,0.2,1),opacity 0.25s ease,box-shadow 0.25s ease';
                        if (isDragging) {{
                            startHideTimer();
                            setTimeout(function() {{ isDragging = false; }}, 50);
                        }}
                    }}
                    document.addEventListener('mousemove', onMove);
                    document.addEventListener('mouseup', onUp);
                }});

                setTimeout(function() {{
                    fab.style.right = '12px';
                    fab.style.opacity = '1';
                    label.style.opacity = '1';
                    label.style.transform = 'translateX(0)';
                }}, 300);
                setTimeout(snapToEdge, 2500);

                fab.addEventListener('click', function(e) {{
                    if (isDragging) return;
                    if (window.__cheersai_nav_lock) return;
                    window.__cheersai_nav_lock = true;
                    fab.style.transform = 'scale(0)';
                    fab.style.opacity = '0';

                    try {{
                        var current = window.location.href.split('#')[0];
                        window.history.replaceState(null, '', current + '#__cheersai_return__');
                    }} catch (_err) {{
                        try {{
                            window.location.hash = '__cheersai_return__';
                        }} catch (_ignored) {{}}
                    }}
                }});

                document.body.appendChild(fab);
                console.log('CheersAI: desktop FAB injected');
            }}
        }})();
    "#, icon_data_url = icon_data_url, brand_hider_script = brand_hider_script);

    let window_for_inject = desktop_window.clone();
    let app_for_monitor = app.clone();
    tokio::spawn(async move {
        let mut count = 0u32;
        loop {
            let delay = if count < 3 { 1500 } else { 3000 };
            tokio::time::sleep(tokio::time::Duration::from_millis(delay)).await;
            count += 1;

            let url_str = match window_for_inject.url() {
                Ok(u) => u.to_string(),
                Err(_) => {
                    if count > 10 {
                        break;
                    }
                    continue;
                }
            };

            if url_str.contains("#__cheersai_return__") {
                if let Some(main_window) = app_for_monitor.get_webview_window("main") {
                    let _ = main_window.show();
                    let _ = main_window.set_focus();
                }
                let _ = app_for_monitor.emit("navigate-to-process", ());
                let _ = window_for_inject.close();
                break;
            }

            if url_str.contains("uat-desktop.cheersai.cloud")
                || url_str.contains("uat-sso.cheersai.cloud") {
                let _ = window_for_inject.eval(&inject_script);
            }

            if count > 200 {
                break;
            }
        }
    });

    Ok(())
}

#[tauri::command]
pub async fn navigate_webview(
    app: AppHandle,
    label: String,
    url: String,
) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(&label) {
        let parsed_url = url.parse().map_err(|e| format!("Invalid URL: {}", e))?;
        window.navigate(parsed_url)
            .map_err(|e| format!("Failed to navigate: {}", e))?;
        Ok(())
    } else {
        Err(format!("Window not found: {}", label))
    }
}

#[tauri::command]
pub async fn webview_reload(
    app: AppHandle,
    label: String,
) -> Result<(), String> {
    if let Some(webview) = app.get_webview_window(&label) {
        // Reload by navigating to current URL
        if let Ok(current_url) = webview.url() {
            webview.navigate(current_url)
                .map_err(|e| format!("Failed to reload: {}", e))?;
        }
        Ok(())
    } else {
        Err(format!("Window not found: {}", label))
    }
}

#[tauri::command]
pub async fn close_webview_window(
    app: AppHandle,
    label: String,
) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(&label) {
        window.close().map_err(|e| format!("Failed to close window: {}", e))?;
        Ok(())
    } else {
        Err(format!("Window not found: {}", label))
    }
}

#[tauri::command]
pub async fn get_webview_url(
    app: AppHandle,
    label: String,
) -> Result<String, String> {
    if let Some(window) = app.get_webview_window(&label) {
        window.url()
            .map(|u| u.to_string())
            .map_err(|e| format!("Failed to get URL: {}", e))
    } else {
        Err(format!("Window not found: {}", label))
    }
}

#[tauri::command]
pub async fn webview_eval_script(
    app: AppHandle,
    label: String,
    script: String,
) -> Result<(), String> {
    if let Some(webview) = app.get_webview_window(&label) {
        webview.eval(&script)
            .map_err(|e| format!("Failed to evaluate script: {}", e))?;
        Ok(())
    } else {
        Err(format!("Window not found: {}", label))
    }
}

#[tauri::command]
pub async fn navigate_main_window_with_button(
    app: AppHandle,
    url: String,
    return_url: String,
) -> Result<(), String> {
    if NAVIGATION_IN_PROGRESS
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {

        return Ok(());
    }

    let main_window = app.get_webview_window("main")
        .ok_or("Main window not found".to_string())?;

    let return_url = if return_url.trim().is_empty() {
        if cfg!(debug_assertions) {
            "http://localhost:1420/?target=process".to_string()
        } else {
            "tauri://localhost/?target=process".to_string()
        }
    } else {
        return_url
    };

    tokio::spawn(async {
        tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
        NAVIGATION_IN_PROGRESS.store(false, Ordering::SeqCst);
    });

    // 将图标转为 base64 data URL
    let icon_b64 = base64::engine::general_purpose::STANDARD.encode(SAFER_ICON_BYTES);
    let icon_data_url = format!("data:image/png;base64,{}", icon_b64);

    // 导航到目标外部 URL

    let parsed_url = url.parse().map_err(|e| format!("Invalid URL: {}", e))?;
    main_window.navigate(parsed_url)
        .map_err(|e| format!("Failed to navigate: {}", e))?;

    // ===== 注入悬浮圆形按钮脚本 =====
    // 核心改变：按钮点击后由 JS 直接 window.location.href 导航，不再用 hash 信号
    let brand_hider_script = desktop_brand_hider_script();
    let inject_script = format!(r#"
        (function() {{
            window.__cheersai_nav_lock = window.__cheersai_nav_lock || false;
            {brand_hider_script}

            if (document.readyState === 'loading') {{
                document.addEventListener('DOMContentLoaded', inject);
            }} else {{
                inject();
            }}

            function inject() {{
                var href = window.location.href;
                console.log('🔍 CheersAI FAB: Checking injection conditions...', href);

                // 只在目标外部页面注入
                if (href.indexOf('tauri://localhost') !== -1 ||
                    href.indexOf('localhost:1420') !== -1 ||
                    href.indexOf('127.0.0.1') !== -1) return;
                    href.indexOf('127.0.0.1') !== -1) return;
                if (href.indexOf('{cloud_host}') === -1 &&
                    href.indexOf('uat-sso.cheersai.cloud') === -1) return;
                if (document.getElementById('__cheersai_fab')) return;

                // --- 悬浮圆形按钮（吸附右侧、可拖动、悬停显示文字） ---
                var fab = document.createElement('div');
                fab.id = '__cheersai_fab';
                fab.style.cssText = 'position:fixed;top:40%;right:0px;z-index:2147483647;' +
                    'display:flex;align-items:center;cursor:pointer;user-select:none;' +
                    'transition:right 0.25s cubic-bezier(0.4,0,0.2,1),opacity 0.25s ease,box-shadow 0.25s ease;' +
                    'opacity:0.85;' +
                    'pointer-events:auto;';

                // 圆形图标容器
                var circle = document.createElement('div');
                circle.style.cssText = 'width:56px;height:56px;min-width:56px;border-radius:50%;' +
                    'background:#fff;box-shadow:-2px 2px 12px rgba(0,0,0,0.15);' +
                    'display:flex;align-items:center;justify-content:center;overflow:hidden;' +
                    'transition:box-shadow 0.25s ease,transform 0.25s ease;';

                var img = document.createElement('img');
                img.src = '{icon_data_url}';
                img.style.cssText = 'width:40px;height:40px;border-radius:50%;pointer-events:none;';
                img.draggable = false;
                img.onerror = function() {{
                    img.style.display = 'none';
                    fallback.style.display = 'flex';
                }};
                circle.appendChild(img);

                var fallback = document.createElement('div');
                fallback.textContent = 'AI';
                fallback.style.cssText = 'display:none;width:40px;height:40px;border-radius:50%;' +
                    'align-items:center;justify-content:center;background:#2563eb;color:#fff;' +
                    'font-size:12px;font-weight:700;pointer-events:none;' +
                    'font-family:-apple-system,BlinkMacSystemFont,Segoe UI,Roboto,sans-serif;';
                circle.appendChild(fallback);
                fab.appendChild(circle);

                // 文字标签（悬停时显示）
                var label = document.createElement('div');
                label.textContent = '\u6570\u636E\u5B89\u5168\u5C4B';
                label.style.cssText = 'margin-right:8px;padding:6px 14px;border-radius:20px;' +
                    'background:#fff;color:#1e40af;font-size:13px;font-weight:600;white-space:nowrap;' +
                    'box-shadow:0 2px 10px rgba(0,0,0,0.12);pointer-events:none;' +
                    'opacity:0;transform:translateX(8px);' +
                    'transition:opacity 0.2s ease,transform 0.2s ease;' +
                    'font-family:-apple-system,BlinkMacSystemFont,Segoe UI,Roboto,sans-serif;';
                fab.insertBefore(label, circle);

                // --- 吸附 / 滑出 ---
                var hideTimer = null;
                var isDragging = false;

                function snapToEdge() {{
                    if (window.__cheersai_nav_lock || isDragging) return;
                    fab.style.right = '0px';
                    fab.style.opacity = '0.85';
                    circle.style.boxShadow = '-2px 2px 12px rgba(0,0,0,0.15)';
                    circle.style.transform = 'scale(1)';
                    label.style.opacity = '0';
                    label.style.transform = 'translateX(8px)';
                }}
                function slideOut() {{
                    if (window.__cheersai_nav_lock || isDragging) return;
                    clearTimeout(hideTimer);
                    fab.style.right = '12px';
                    fab.style.opacity = '1';
                    circle.style.boxShadow = '0 6px 24px rgba(59,130,246,0.3)';
                    circle.style.transform = 'scale(1.08)';
                    label.style.opacity = '1';
                    label.style.transform = 'translateX(0)';
                }}
                function startHideTimer() {{
                    clearTimeout(hideTimer);
                    hideTimer = setTimeout(snapToEdge, 3000);
                }}

                fab.addEventListener('mouseenter', function() {{ slideOut(); }});
                fab.addEventListener('mouseleave', function() {{ startHideTimer(); }});

                // --- 上下拖动（吸附右侧边缘） ---
                var startY = 0, startTop = 0;

                fab.addEventListener('mousedown', function(e) {{
                    isDragging = false;
                    startY = e.clientY;
                    startTop = fab.getBoundingClientRect().top;
                    fab.style.transition = 'none'; // 拖动时关闭动画

                    function onMove(ev) {{
                        var dy = ev.clientY - startY;
                        if (Math.abs(dy) > 3) isDragging = true;
                        if (isDragging) {{
                            var newTop = Math.max(20, Math.min(window.innerHeight - 76, startTop + dy));
                            fab.style.top = newTop + 'px';
                        }}
                    }}
                    function onUp() {{
                        document.removeEventListener('mousemove', onMove);
                        document.removeEventListener('mouseup', onUp);
                        fab.style.transition = 'right 0.25s cubic-bezier(0.4,0,0.2,1),opacity 0.25s ease,box-shadow 0.25s ease';
                        if (isDragging) {{
                            startHideTimer();
                            setTimeout(function() {{ isDragging = false; }}, 50);
                        }}
                    }}
                    document.addEventListener('mousemove', onMove);
                    document.addEventListener('mouseup', onUp);
                }});

                // 初始化：先完整显示 5 秒，再吸附
                setTimeout(function() {{
                    fab.style.right = '12px';
                    fab.style.opacity = '1';
                    label.style.opacity = '1';
                    label.style.transform = 'translateX(0)';
                }}, 300);
                setTimeout(snapToEdge, 5000);

                // --- 点击：直接导航回本地页面 ---
                fab.addEventListener('click', function(e) {{
                    if (isDragging) return; // 拖动结束不触发点击
                    if (window.__cheersai_nav_lock) return;
                    window.__cheersai_nav_lock = true;

                    fab.style.transform = 'scale(0)';
                    fab.style.opacity = '0';
                    window.name = '__cheersai_return_process__';

                    try {{
                        if (window.history.length > 1) {{
                            window.history.back();
                        }}
                    }} catch (_err) {{}}

                    setTimeout(function() {{
                        if (window.location.href.indexOf('tauri://localhost') !== -1 ||
                            window.location.href.indexOf('localhost:1420') !== -1) {{
                            return;
                        }}

                        try {{
                            var current = window.location.href.split('#')[0];
                            window.location.replace(current + '#__cheersai_return__');
                        }} catch (_err) {{}}
                    }}, 120);

                    setTimeout(function() {{
                        if (window.location.href.indexOf('tauri://localhost') !== -1 ||
                            window.location.href.indexOf('localhost:1420') !== -1) {{
                            return;
                        }}

                        window.location.replace('{return_url}');
                    }}, 450);
                }});

                document.body.appendChild(fab);
                console.log('✅ CheersAI FAB: Button injected successfully!');
                console.log('✅ CheersAI FAB: Button element:', fab);
                console.log('✅ CheersAI FAB: Button position:', fab.getBoundingClientRect());
                console.log('✅ CheersAI FAB: Button styles:', window.getComputedStyle(fab));
            }}
        }})();
    "#, icon_data_url = icon_data_url, return_url = return_url, cloud_host = CLOUD_HOST, brand_hider_script = brand_hider_script);

    // 注入任务：持续运行，只要还在外部页面就不断尝试注入
    // 这样退出登录再登录后，页面刷新也能重新注入按钮
    let window_for_inject = main_window.clone();
    let script_clone = inject_script.clone();
    tokio::spawn(async move {
        let mut count = 0u32;
        let mut found_target_domain = false;
        
        loop {
            // 前10次每秒检查一次，之后每2秒检查一次
            let delay = if count < 10 { 1000 } else { 2000 };
            tokio::time::sleep(tokio::time::Duration::from_millis(delay)).await;
            count += 1;

            // 检查当前 URL
            let url_str = match window_for_inject.url() {
                Ok(u) => u.to_string(),
                Err(_) => continue,
            };
            
            if count <= 10 {

            }
            // 检测是否在目标域名
            if url_str.contains(CLOUD_HOST) || url_str.contains("uat-sso.cheersai.cloud") {
                found_target_domain = true;
                if count <= 10 {

                }
                if window_for_inject.eval(&script_clone).is_ok() {
                    if count <= 5 {

                    }
                } else {

                }
            } else if url_str.contains("localhost:1420") || url_str.contains("tauri://localhost") {
                // 只有在之前找到过目标域名后，才在返回本地页面时停止
                if found_target_domain {

                    break;
                } else if count <= 10 {
                }
            } else {
                if count <= 10 {
                }
            }
            
            // 10 分钟超时
            if count > 300 {

                break;
            }
        }
    });

    // ===== 兜底任务：如果 JS 导航失败，Rust 强制导航 =====
    let window_clone = main_window.clone();
    let return_url_clone = return_url.clone();
    tokio::spawn(async move {
        let mut ticks = 0u32;

        loop {
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
            ticks += 1;

            let url_str = match window_clone.url() {
                Ok(u) => u.to_string(),
                Err(_) => continue,
            };

            // 外部页已收到“返回本地”点击信号，直接强制回本地
            if url_str.contains("#__cheersai_return__") {

                if let Ok(local_url) = return_url_clone.parse() {
                    let _ = window_clone.navigate(local_url);
                }
                break;
            }

            // 已经成功回到本地页面
            if url_str.contains("localhost:1420") || url_str.contains("tauri://localhost") {

                break;
            }

            // 还在外部页面，继续等待
            if url_str.starts_with("https://uat-desktop.cheersai.cloud")
                || url_str.starts_with("http://uat-desktop.cheersai.cloud")
                || url_str.starts_with("https://uat-sso.cheersai.cloud")
                || url_str.starts_with("http://uat-sso.cheersai.cloud") {
                // 正常等待用户点击按钮
                if ticks > 600 { // 5 分钟超时

                    break;
                }
                continue;
            }

            // URL 既不是外部也不是本地（可能 JS 导航卡在中间状态）
            // 等 2 秒看是否自动恢复

            tokio::time::sleep(tokio::time::Duration::from_millis(2000)).await;

            // 再检查一次
            let url_now = match window_clone.url() {
                Ok(u) => u.to_string(),
                Err(_) => continue,
            };

            if url_now.contains("localhost:1420") || url_now.contains("tauri://localhost") {

                break;
            }

            // 仍然卡住，Rust 强制导航

            if let Ok(local_url) = return_url_clone.parse() {
                let _ = window_clone.navigate(local_url);
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;

            if let Ok(final_url) = window_clone.url() {

            }
            break;
        }
    });
    Ok(())
}

// ===== Browser Fetch Infrastructure (uses WebView2/BoringSSL, bypasses reqwest TLS fingerprint) =====

pub struct BrowserFetchPending {
    pub channels: TokioMutex<HashMap<String, oneshot::Sender<BrowserFetchResult>>>,
}

impl Default for BrowserFetchPending {
    fn default() -> Self {
        Self {
            channels: TokioMutex::new(HashMap::new()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserFetchResult {
    pub ok: bool,
    pub status: u16,
    pub body: String,
}

#[tauri::command]
pub async fn on_browser_fetch_result(
    state: tauri::State<'_, BrowserFetchPending>,
    id: String,
    ok: bool,
    status: u16,
    body: String,
) -> Result<(), String> {
    let mut channels = state.channels.lock().await;
    if let Some(tx) = channels.remove(&id) {
        let _ = tx.send(BrowserFetchResult { ok, status, body });
    }
    Ok(())
}

/// 获取或创建隐藏的 FileBay API 代理窗口（同源 fetch，绕过 CORS）
async fn ensure_filebay_proxy_window(app: &AppHandle, base_url: &str) -> Result<WebviewWindow, String> {
    let label = "filebay-api-proxy";
    if let Some(win) = app.get_webview_window(label) {
        return Ok(win);
    }
    let parsed_url = base_url
        .parse::<tauri::Url>()
        .map_err(|e| format!("Invalid FileBay URL: {}", e))?;
    let win = WebviewWindowBuilder::new(app, label, WebviewUrl::External(parsed_url))
        .title("FileBay Proxy")
        .visible(false)
        .inner_size(1.0, 1.0)
        .skip_taskbar(true)
        .build()
        .map_err(|e| format!("Failed to create FileBay proxy window: {}", e))?;
    // 等待 WebView2 初始化
    tokio::time::sleep(tokio::time::Duration::from_millis(2000)).await;
    Ok(win)
}

/// 通过 WebView2 的 JavaScript fetch 发 HTTP 请求（使用浏览器的 BoringSSL TLS 栈）
/// proxy_base_url: 若提供，则在该域下的隐藏窗口中发请求（同源，避免 CORS）
pub async fn fetch_via_browser(
    app: &AppHandle,
    pending: &BrowserFetchPending,
    method: &str,
    url: &str,
    auth_token: &str,
    body: Option<&serde_json::Value>,
    proxy_base_url: Option<&str>,
) -> Result<BrowserFetchResult, String> {
    let id = uuid::Uuid::new_v4().to_string();
    let (tx, rx) = oneshot::channel();

    {
        let mut channels = pending.channels.lock().await;
        channels.insert(id.clone(), tx);
    }

    let body_js = match body {
        Some(b) => {
            let body_str = b.to_string();
            let body_b64 = base64::engine::general_purpose::STANDARD.encode(body_str.as_bytes());
            format!(", body: atob('{}')" , body_b64)
        }
        None => String::new(),
    };

    let script = format!(
        r#"(async function() {{
    try {{
        const resp = await fetch('{}', {{
            method: '{}',
            headers: {{
                'Authorization': 'token {}',
                'Content-Type': 'application/json'
            }}{}
        }});
        const text = await resp.text();
        window.__TAURI_INTERNALS__.invoke('on_browser_fetch_result', {{
            id: '{}', ok: resp.ok, status: resp.status, body: text
        }});
    }} catch(err) {{
        window.__TAURI_INTERNALS__.invoke('on_browser_fetch_result', {{
            id: '{}', ok: false, status: 0, body: JSON.stringify({{error: err.message}})
        }});
    }}
}})();"#,
        url, method, auth_token, body_js, id, id
    );

    let window = if let Some(base_url) = proxy_base_url {
        ensure_filebay_proxy_window(app, base_url).await?
    } else {
        app.get_webview_window("main")
            .ok_or_else(|| "Main window not found".to_string())?
    };

    // 带重试的 eval（代理窗口刚创建时可能未就绪）
    let mut last_err = String::new();
    for attempt in 0..3u32 {
        match window.eval(&script) {
            Ok(_) => { last_err = String::new(); break; }
            Err(e) => {
                last_err = format!("Eval failed: {}", e);
                if attempt < 2 {
                    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                }
            }
        }
    }
    if !last_err.is_empty() {
        return Err(last_err);
    }

    tokio::time::timeout(tokio::time::Duration::from_secs(30), rx)
        .await
        .map_err(|_| "Browser fetch timeout after 30s".to_string())?
        .map_err(|_| "Channel was closed".to_string())
}
