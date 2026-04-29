# FileBay 上传权限问题修复

## 🔴 当前问题

### 错误日志
```
上传失败(browser): 上传失败: HTTP 403 - {"message":"user should have a permission to write to the target branch","url":"https://uat-filebay.cheersai.cloud/api/swagger"}
```

### 错误分析
- **HTTP 403 Forbidden** - 权限被拒绝
- **错误消息**: "user should have a permission to write to the target branch"
- **原因**: Token 没有写入目标分支的权限

## 🔍 问题根源

### 可能的原因

1. **Token 权限不足**
   - Token 可能只有读取权限
   - Token 可能没有 `repo` 或 `write:repo` 权限
   - Token 可能已过期

2. **分支保护规则**
   - `main` 分支可能被设置为受保护分支
   - 受保护分支可能需要特殊权限或审批流程
   - 可能需要管理员权限才能直接推送

3. **仓库权限设置**
   - 用户可能只有读取权限
   - 仓库可能设置为只读模式
   - 用户可能不是仓库的协作者

4. **分支不存在**
   - 指定的 `main` 分支可能不存在
   - 仓库的默认分支可能是 `master` 或其他名称

## ✅ 修复方案

### 方案 1: 移除显式分支指定（已实施）

**修改文件**: `src-tauri/src/commands/gitea.rs`

**修改内容**: 移除请求体中的 `"branch": "main"` 参数，让 Gitea API 使用仓库的默认分支

**修改前**:
```rust
let mut body = serde_json::json!({
    "content": content_b64,
    "message": message,
    "branch": "main"
});
```

**修改后**:
```rust
let mut body = serde_json::json!({
    "content": content_b64,
    "message": message
});
```

**原理**: 
- 不指定分支时，Gitea API 会自动使用仓库的默认分支
- 避免了分支名称不匹配的问题
- 如果默认分支有权限，上传应该能成功

### 方案 2: 检查并使用仓库的默认分支（备选）

如果方案 1 不起作用，可以先查询仓库信息获取默认分支：

```rust
// 1. 查询仓库信息
let repo_url = format!("{}/api/v1/repos/{}/{}", url, owner, repo);
let repo_info = fetch_via_browser(app, fetch_pending, "GET", &repo_url, token, None, Some(url)).await?;

// 2. 解析默认分支
let repo_json: serde_json::Value = serde_json::from_str(&repo_info.body)?;
let default_branch = repo_json["default_branch"].as_str().unwrap_or("main");

// 3. 使用默认分支上传
let mut body = serde_json::json!({
    "content": content_b64,
    "message": message,
    "branch": default_branch
});
```

### 方案 3: 尝试多个分支名称（备选）

如果方案 1 和 2 都不起作用，可以尝试常见的分支名称：

```rust
let branches = vec!["main", "master", "develop"];
for branch in branches {
    let mut body = serde_json::json!({
        "content": content_b64,
        "message": message,
        "branch": branch
    });
    
    match fetch_via_browser(app, fetch_pending, "PUT", &put_url, token, Some(&body), Some(url)).await {
        Ok(r) if r.ok => return Ok(...),
        _ => continue,
    }
}
```

### 方案 4: 修复 Token 权限（需要用户操作）

如果以上方案都不起作用，需要在 FileBay 网站上重新生成 Token：

1. 访问 `https://uat-filebay.cheersai.cloud`
2. 登录账号
3. 进入 **设置 → 应用 → 访问令牌**
4. 删除旧的 Token
5. 创建新的 Token，确保勾选以下权限：
   - ✅ `repo` - 完整的仓库访问权限
   - ✅ `write:repo` - 写入仓库权限
   - ✅ `read:repo` - 读取仓库权限
6. 复制新的 Token
7. 更新 `filebay-config.json` 文件
8. 在应用中重新导入配置

## 🧪 测试步骤

### 1. 重新编译并测试

```bash
# 后端会自动重新编译
# 等待编译完成后，在应用中测试上传
```

### 2. 观察结果

**成功场景**:
```
✅ 显示: "[文件名] 上传成功"（绿色提示）
✅ 后端日志: 无错误
✅ FileBay 网站: 可以看到上传的文件
```

**仍然失败**:
```
❌ 显示: "上传失败: HTTP 403 - [错误消息]"
❌ 需要尝试其他方案
```

## 📊 诊断步骤

### 1. 检查 Token 权限

使用以下命令测试 Token 权限：

```bash
# 测试读取权限
curl -H "Authorization: token 7cb8cbe28912a5a96ca82952e62b411847b7b7cc" \
  https://uat-filebay.cheersai.cloud/api/v1/repos/admin_cheersai_cloud_de8df0/workspace

# 测试写入权限（创建文件）
curl -X PUT \
  -H "Authorization: token 7cb8cbe28912a5a96ca82952e62b411847b7b7cc" \
  -H "Content-Type: application/json" \
  -d '{"content":"dGVzdA==","message":"test"}' \
  https://uat-filebay.cheersai.cloud/api/v1/repos/admin_cheersai_cloud_de8df0/workspace/contents/test.txt
```

### 2. 检查仓库默认分支

```bash
curl -H "Authorization: token 7cb8cbe28912a5a96ca82952e62b411847b7b7cc" \
  https://uat-filebay.cheersai.cloud/api/v1/repos/admin_cheersai_cloud_de8df0/workspace \
  | grep default_branch
```

### 3. 检查分支保护规则

1. 访问 FileBay 网站
2. 进入 `workspace` 仓库
3. 点击 **设置 → 分支保护**
4. 查看 `main` 分支是否有保护规则

### 4. 检查用户权限

1. 访问 FileBay 网站
2. 进入 `workspace` 仓库
3. 点击 **设置 → 协作者**
4. 确认 `admin_cheersai_cloud_de8df0` 用户有写入权限

## 🎯 预期效果

### 如果方案 1 成功
- ✅ 上传功能正常工作
- ✅ 文件被上传到仓库的默认分支
- ✅ 不需要额外配置

### 如果方案 1 失败
- ❌ 仍然收到 403 错误
- 🔧 需要实施方案 2、3 或 4
- 📝 需要更多诊断信息

## 📝 技术细节

### Gitea API 分支参数

根据 Gitea API 文档：
- `branch` 参数是**可选的**
- 如果不提供，使用仓库的默认分支
- 如果提供了不存在的分支，会返回 404
- 如果提供了受保护的分支且没有权限，会返回 403

### 为什么移除分支参数可能有效？

1. **避免分支名称错误**
   - 仓库的默认分支可能不是 `main`
   - 可能是 `master`、`develop` 或其他名称

2. **避免分支保护问题**
   - 默认分支可能没有保护规则
   - 或者 Token 对默认分支有特殊权限

3. **简化逻辑**
   - 让 Gitea 自动处理分支选择
   - 减少配置错误的可能性

## 🚀 下一步

### 立即测试
1. 等待 Rust 后端重新编译完成
2. 在应用中测试上传功能
3. 观察是否成功

### 如果成功
- ✅ 问题解决
- 📝 更新文档
- 🎉 功能可用

### 如果失败
根据新的错误消息决定：

1. **如果仍是 403 权限错误**
   - 实施方案 2：查询并使用默认分支
   - 或实施方案 4：重新生成 Token

2. **如果是 404 错误**
   - 分支不存在
   - 实施方案 2 或 3

3. **如果是其他错误**
   - 根据具体错误消息进一步诊断

## 📚 相关资源

- Gitea API 文档: https://docs.gitea.io/en-us/api-usage/
- Gitea 文件上传 API: `/api/v1/repos/{owner}/{repo}/contents/{filepath}`
- FileBay UAT 环境: https://uat-filebay.cheersai.cloud

## 总结

- 🐛 **问题**: Token 没有写入目标分支的权限
- 🔧 **修复**: 移除显式的分支参数，让 API 使用默认分支
- ⏳ **状态**: 已修改代码，等待测试验证
- 🎯 **目标**: 成功上传文件到 FileBay

**请在应用中重新测试上传功能！** 🚀
