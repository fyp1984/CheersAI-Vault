# FileBay Token 权限问题 - 最终解决方案

## 🔴 问题确认

### 错误日志
```
上传失败(browser): 上传失败: HTTP 403 - {"message":"user should have a permission to write to the target branch","url":"https://uat-filebay.cheersai.cloud/api/swagger"}
```

### 问题根源
**Token 没有写入权限**

已经尝试的修复：
1. ✅ 修复错误处理 - 完成
2. ✅ 修改 HTTP 方法 POST → PUT - 完成
3. ✅ 移除显式分支参数 - 完成
4. ❌ **仍然 403 错误** - Token 本身没有写入权限

## 🎯 最终解决方案

### 方案 A: 重新生成 Token（推荐）

这是最直接和最可靠的解决方案。

#### 步骤 1: 访问 FileBay 网站
```
https://uat-filebay.cheersai.cloud
```

#### 步骤 2: 登录账号
- 用户名: `admin_cheersai_cloud_de8df0`
- 使用现有密码登录

#### 步骤 3: 删除旧 Token
1. 点击右上角头像
2. 选择 **"设置"** (Settings)
3. 左侧菜单选择 **"应用"** (Applications)
4. 找到 **"访问令牌"** (Access Tokens) 标签
5. 找到旧的 Token（可能显示为 "CheersAI Vault" 或类似名称）
6. 点击 **"删除"** 按钮

#### 步骤 4: 创建新 Token
1. 在 **"访问令牌"** 页面
2. 点击 **"生成新令牌"** (Generate New Token)
3. 填写信息：
   - **令牌名称**: `CheersAI Vault Upload`
   - **选择权限**（非常重要）:
     - ✅ **repo** - 完整的仓库访问权限
     - ✅ **write:repo** - 写入仓库权限
     - ✅ **read:repo** - 读取仓库权限
     - ✅ **admin:repo** - 管理仓库权限（如果有的话）
4. 点击 **"生成令牌"**
5. **立即复制新 Token**（只显示一次！）

#### 步骤 5: 更新配置文件
编辑 `filebay-config.json`:
```json
{
  "url": "https://uat-filebay.cheersai.cloud",
  "username": "admin_cheersai_cloud_de8df0",
  "repoName": "workspace",
  "email": "admin@cheersai.cloud",
  "token": "新的Token粘贴在这里",
  "downloadedAt": "2026-04-29T12:00:00.000Z",
  "version": "1.0.0"
}
```

#### 步骤 6: 在应用中重新导入配置
1. 打开应用: http://localhost:1420/
2. 进入 **"FileBay 设置"**
3. 点击 **"导入配置"**
4. 选择更新后的 `filebay-config.json`
5. 确保 **"启用 FileBay 上传"** 开关是打开的

#### 步骤 7: 测试上传
1. 进入 **"文件管理"**
2. 选择一个文件
3. 点击 **"上传"** 按钮
4. 应该成功！✅

---

### 方案 B: 检查仓库协作者权限

如果方案 A 不起作用，可能是仓库权限问题。

#### 步骤 1: 检查仓库设置
1. 访问 https://uat-filebay.cheersai.cloud
2. 登录后进入 `workspace` 仓库
3. 点击 **"设置"** (Settings)

#### 步骤 2: 检查协作者
1. 左侧菜单选择 **"协作者"** (Collaborators)
2. 确认 `admin_cheersai_cloud_de8df0` 在列表中
3. 确认权限级别是 **"写入"** (Write) 或 **"管理员"** (Admin)

#### 步骤 3: 添加协作者（如果需要）
1. 如果用户不在列表中，点击 **"添加协作者"**
2. 搜索 `admin_cheersai_cloud_de8df0`
3. 选择权限级别: **"写入"** 或 **"管理员"**
4. 点击 **"添加"**

---

### 方案 C: 检查分支保护规则

如果仍然失败，可能是分支保护问题。

#### 步骤 1: 检查分支保护
1. 在 `workspace` 仓库设置中
2. 左侧菜单选择 **"分支"** (Branches)
3. 查看 **"分支保护规则"** (Branch Protection Rules)

#### 步骤 2: 修改保护规则
1. 找到 `main` 或 `master` 分支的保护规则
2. 如果有保护规则，点击 **"编辑"**
3. 选项：
   - **选项 A**: 移除保护规则（如果这是测试仓库）
   - **选项 B**: 添加 `admin_cheersai_cloud_de8df0` 到允许推送的用户列表
   - **选项 C**: 取消勾选 **"需要审批"** 等限制

---

### 方案 D: 使用 API 创建新分支（代码修改）

如果以上都不起作用，可以尝试创建一个新的无保护分支。

#### 修改代码
在 `src-tauri/src/commands/gitea.rs` 的 `browser_upload_file` 函数中：

```rust
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
) -> Result<String, String> {
    let content = std::fs::read(file_path).map_err(|e| format!("读取文件失败: {}", e))?;
    let content_b64 = base64::engine::general_purpose::STANDARD.encode(&content);

    // 尝试使用 vault-uploads 分支
    let branch = "vault-uploads";
    
    let get_url = format!("{}/api/v1/repos/{}/{}/contents/{}?ref={}", url, owner, repo, remote_path, branch);
    let sha = match fetch_via_browser(app, fetch_pending, "GET", &get_url, token, None, Some(url)).await {
        Ok(r) if r.ok => {
            let json: serde_json::Value = serde_json::from_str(&r.body).unwrap_or_default();
            json["sha"].as_str().map(String::from)
        }
        _ => None,
    };

    let put_url = format!("{}/api/v1/repos/{}/{}/contents/{}", url, owner, repo, remote_path);
    let mut body = serde_json::json!({
        "content": content_b64,
        "message": message,
        "branch": branch  // 使用专门的上传分支
    });
    if let Some(sha_str) = sha {
        body["sha"] = serde_json::json!(sha_str);
    }

    match fetch_via_browser(app, fetch_pending, "PUT", &put_url, token, Some(&body), Some(url)).await {
        Ok(r) if r.ok => Ok(format!("{}/{}/{}/raw/branch/{}/{}", url, owner, repo, branch, remote_path)),
        Ok(r) => Err(format!("上传失败: HTTP {} - {}", r.status, r.body)),
        Err(e) => Err(e),
    }
}
```

这个方案会尝试使用一个新的分支 `vault-uploads`，该分支可能没有保护规则。

---

## 🧪 诊断工具

### 在应用中添加 Token 权限检查

可以添加一个诊断命令来检查 Token 权限：

```rust
#[tauri::command]
pub async fn diagnose_token_permissions(
    app: AppHandle,
    state: State<'_, GiteaState>,
    fetch_pending: State<'_, BrowserFetchPending>,
) -> Result<String, String> {
    let (url, token, owner, repo) = {
        let config = state.config.lock().await;
        (config.url.clone(), config.token.clone(), config.owner.clone(), config.repo.clone())
    };

    let mut report = String::new();
    
    // 1. 测试读取权限
    let repo_url = format!("{}/api/v1/repos/{}/{}", url, owner, repo);
    match fetch_via_browser(&app, &fetch_pending, "GET", &repo_url, &token, None, Some(&url)).await {
        Ok(r) if r.ok => report.push_str("✅ 读取权限: 正常\n"),
        Ok(r) => report.push_str(&format!("❌ 读取权限: 失败 (HTTP {})\n", r.status)),
        Err(e) => report.push_str(&format!("❌ 读取权限: 错误 - {}\n", e)),
    }
    
    // 2. 测试写入权限
    let test_content = base64::engine::general_purpose::STANDARD.encode("test");
    let test_url = format!("{}/api/v1/repos/{}/{}/contents/test_permission.txt", url, owner, repo);
    let test_body = serde_json::json!({
        "content": test_content,
        "message": "Permission test"
    });
    match fetch_via_browser(&app, &fetch_pending, "PUT", &test_url, &token, Some(&test_body), Some(&url)).await {
        Ok(r) if r.ok => report.push_str("✅ 写入权限: 正常\n"),
        Ok(r) => {
            report.push_str(&format!("❌ 写入权限: 失败 (HTTP {})\n", r.status));
            report.push_str(&format!("   错误消息: {}\n", r.body));
        }
        Err(e) => report.push_str(&format!("❌ 写入权限: 错误 - {}\n", e)),
    }
    
    Ok(report)
}
```

---

## 📊 预期结果

### 方案 A 成功后
```
✅ 上传成功
✅ 文件出现在 FileBay 仓库中
✅ 可以正常使用上传功能
```

### 如果所有方案都失败
可能需要联系 FileBay 管理员：
1. 检查账号权限
2. 检查仓库设置
3. 检查系统级别的权限限制

---

## 🎯 推荐行动

### 立即执行
1. **方案 A**: 重新生成 Token（最可能解决问题）
2. 确保勾选所有 `repo` 相关权限
3. 更新配置文件
4. 重新测试

### 如果方案 A 失败
1. 执行方案 B: 检查协作者权限
2. 执行方案 C: 检查分支保护
3. 考虑方案 D: 使用新分支

---

## 📝 总结

**问题**: Token 没有写入仓库的权限

**根本原因**: 
- Token 创建时没有勾选写入权限
- 或者 Token 已过期
- 或者仓库/分支有额外的保护规则

**最佳解决方案**: 
重新生成一个具有完整 `repo` 权限的新 Token

**预计时间**: 5-10 分钟

---

## 🚀 下一步

请执行方案 A（重新生成 Token），然后告诉我结果！

如果你无法访问 FileBay 网站或没有权限生成新 Token，请告诉我，我们可以尝试其他方案。
