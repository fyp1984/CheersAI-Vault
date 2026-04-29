# FileBay 上传功能分析

## 测试结果

### 直接 HTTP 请求测试
使用 Python requests 和 curl 直接访问 FileBay API 时遇到 SSL 错误：

```
❌ SSL 错误: HTTPSConnectionPool(host='uat-filebay.cheersai.cloud', port=443): 
Max retries exceeded with url: /api/v1/user 
(Caused by SSLError(SSLEOFError(8, '[SSL: UNEXPECTED_EOF_WHILE_READING] 
EOF occurred in violation of protocol (_ssl.c:1028)')))
```

```bash
curl -k https://uat-filebay.cheersai.cloud/api/v1/user
# curl: (35) schannel: failed to receive handshake, SSL/TLS connection failed
```

### 原因分析

1. **服务器 SSL 配置问题**
   - UAT 环境的 SSL/TLS 配置可能不完整或使用了非标准配置
   - 标准的 HTTP 客户端（Python requests, curl）无法完成 SSL 握手

2. **应用使用浏览器引擎**
   - 应用通过 WebView2 的 JavaScript fetch API 发送请求
   - 利用浏览器的 BoringSSL TLS 栈，可以处理非标准的 SSL 配置
   - 这就是为什么应用内可能可以工作，但外部工具失败

## 代码实现分析

### 后端实现 (`src-tauri/src/commands/gitea.rs`)

```rust
// 通过浏览器 fetch 上传文件（UAT 环境）
async fn browser_upload_file(
    app: &AppHandle,
    fetch_pending: &BrowserFetchPending,
    url: &str,
    token: &str,
    owner: &str,
    repo: &str,
    file_path: &str,
    remote_path: &str,
    message: &str,
) -> Result<String, String>
```

关键点：
1. 使用 `fetch_via_browser` 函数通过 WebView2 发送请求
2. 文件内容 Base64 编码后发送
3. 先 GET 检查文件是否存在（获取 SHA）
4. 然后 POST 创建或更新文件

### 前端实现 (`src/components/file/FileManager.tsx`)

```typescript
const handleUploadToGitea = async (file: SandboxFile) => {
  const result = await uploadToGitea(
    file.path,
    remotePath,
    `Update: ${file.name}`
  );
  
  if (result.success) {
    setToast({ message: `${file.name} 上传成功`, type: 'success' });
  } else {
    setToast({ message: `${file.name} 上传失败`, type: 'error' });
  }
}
```

## 已修复的问题

### 1. 错误处理修复

**修复前**:
```rust
Err(e) => {
    println!("上传遇到错误(browser): {}", e);
    Ok(UploadResult { success: true, urls: vec![], message: "已更新".to_string() })
    // ❌ 返回成功，隐藏了真实错误
}
```

**修复后**:
```rust
Err(e) => {
    println!("上传失败(browser): {}", e);
    Err(format!("上传失败: {}", e))
    // ✅ 正确返回错误
}
```

### 2. 前端错误显示修复

**修复前**:
```typescript
catch (error) {
  // 即使出错也显示已更新
  setToast({ message: `${file.name} 已更新`, type: 'success' });
}
```

**修复后**:
```typescript
catch (error) {
  setToast({ message: `上传失败: ${error}`, type: 'error' });
}
```

## 可能的上传失败原因

### 1. FileBay 配置未启用
```rust
if !enabled {
    return Err("FileBay 功能未启用".to_string());
}
```

**解决方案**: 在 FileBay 设置中启用上传功能

### 2. Token 认证失败
- Token 过期
- Token 权限不足
- Token 格式错误

**解决方案**: 重新下载 FileBay 配置文件

### 3. 仓库不存在
- 仓库未创建
- 仓库名称错误
- 用户名错误

**解决方案**: 在 FileBay 设置中创建仓库

### 4. 文件路径问题
- 本地文件不存在
- 文件无法读取
- 文件过大

**解决方案**: 检查文件是否存在且可读

### 5. 网络问题
- 无法连接到 FileBay 服务器
- 请求超时
- SSL 握手失败（通过浏览器引擎应该可以避免）

**解决方案**: 检查网络连接

## 测试建议

由于直接 HTTP 请求无法工作，建议通过应用内测试：

### 1. 准备测试环境
1. 启动开发服务器（已启动）
2. 打开应用
3. 进入 FileBay 设置页面

### 2. 配置 FileBay
1. 导入 `filebay-config.json`
2. 测试连接
3. 创建仓库（如果不存在）
4. 启用上传功能

### 3. 测试上传
1. 进入文件管理页面
2. 选择一个文件
3. 点击"上传"按钮
4. 观察提示消息

### 4. 查看日志
在开发服务器终端中查看日志：
- 成功：应该看到 "文件上传成功"
- 失败：应该看到具体的错误信息

## 调试步骤

### 1. 检查 FileBay 配置
```bash
# 查看配置文件
cat filebay-config.json
```

### 2. 检查应用日志
在开发服务器终端中查看：
- `上传失败(browser): ...` - 后端错误
- `Upload error: ...` - 前端错误

### 3. 检查浏览器控制台
打开浏览器开发者工具（F12）：
- Network 标签：查看网络请求
- Console 标签：查看 JavaScript 错误

### 4. 检查 WebView2 窗口
应用会创建一个隐藏的 WebView2 窗口用于发送请求：
- 标签：`filebay_proxy_window`
- URL：`https://uat-filebay.cheersai.cloud`

## 下一步

1. **在应用内测试上传功能**
   - 使用真实的文件
   - 观察错误消息
   - 检查日志输出

2. **如果上传失败，收集信息**
   - 错误消息
   - 后端日志
   - 浏览器控制台日志
   - 网络请求详情

3. **根据错误类型修复**
   - 配置问题 → 更新配置
   - 认证问题 → 重新获取 Token
   - 网络问题 → 检查连接
   - 代码问题 → 修复代码

## 总结

- ✅ 错误处理已修复
- ✅ 错误消息现在会正确显示
- ⚠️ 无法通过外部工具测试（SSL 问题）
- 📝 需要在应用内测试上传功能
- 🔍 需要收集实际的错误日志来诊断问题

**建议**: 在应用中尝试上传一个文件，然后查看具体的错误消息。
