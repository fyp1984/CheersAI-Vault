use reqwest;
use serde::{Deserialize, Serialize};
use std::time::Duration;

const DEFAULT_CLOUD_ORIGIN: &str = "https://uat-desktop.cheersai.cloud";

#[derive(Debug, Serialize, Deserialize)]
pub struct ProxyResponse {
    pub content: String,
    pub status: u16,
    pub content_type: String,
}

#[tauri::command]
pub async fn fetch_webpage(url: String) -> Result<ProxyResponse, String> {
    let client = reqwest::Client::builder()
        .no_proxy()
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::limited(10))
        .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/122.0.0.0 Safari/537.36")
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let response = client
        .get(&url)
        .header("Accept", "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,*/*;q=0.8")
        .header("Accept-Language", "zh-CN,zh;q=0.9,en;q=0.8")
        .header("Cache-Control", "no-cache")
        .header("Pragma", "no-cache")
        .header("Upgrade-Insecure-Requests", "1")
        .send()
        .await
        .map_err(|e| format!("Failed to fetch webpage: {}", e))?;

    let status = response.status().as_u16();
    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|ct| ct.to_str().ok())
        .unwrap_or("text/html")
        .to_string();

    let mut content = response
        .text()
        .await
        .map_err(|e| format!("Failed to read response body: {}", e))?;

    // 修改内容以绕过一些限制
    content = modify_html_content(content, &url);

    Ok(ProxyResponse {
        content,
        status,
        content_type,
    })
}

fn modify_html_content(mut content: String, base_url: &str) -> String {
    // 移除 X-Frame-Options 相关的 meta 标签
    content = content.replace(r#"<meta http-equiv="X-Frame-Options" content="DENY">"#, "");
    content = content.replace(r#"<meta http-equiv="X-Frame-Options" content="SAMEORIGIN">"#, "");
    
    // 移除可能阻止嵌入的 JavaScript
    content = content.replace("top !== self", "false");
    content = content.replace("window.top !== window.self", "false");
    content = content.replace("parent !== window", "false");
    
    // 修复相对路径
    let base_domain = extract_base_domain(base_url);
    content = content.replace("src=\"/", &format!("src=\"{}/", base_domain));
    content = content.replace("href=\"/", &format!("href=\"{}/", base_domain));
    content = content.replace("url(/", &format!("url({}/", base_domain));
    
    // 添加 base 标签
    if !content.contains("<base") {
        content = content.replace(
            "<head>",
            &format!("<head>\n<base href=\"{}/\">", base_domain)
        );
    }
    
    content
}

fn extract_base_domain(url: &str) -> String {
    if let Ok(parsed) = url::Url::parse(url) {
        format!("{}://{}", parsed.scheme(), parsed.host_str().unwrap_or(""))
    } else {
        DEFAULT_CLOUD_ORIGIN.to_string()
    }
}
