# FileBay 删除文件 API 使用指南

## 概述

本文档说明如何使用 FileBay（Gitea）删除文件的 API 功能。

## API 接口

### 删除文件

**端点**: `DELETE /api/v1/repos/{owner}/{repo}/contents/{filepath}`

**请求参数**:
- `owner` (string, required) - 仓库所有者
- `repo` (string, required) - 仓库名称
- `filepath` (string, required) - 文件路径

**请求头**:
```
Authorization: token {your_access_token}
Content-Type: application/json
```

**请求体**:
```json
{
  "sha": "string",           // 必需：文件的 SHA 值（用于验证）
  "message": "string",       // 必需：提交信息
  "branch": "string",        // 可选：分支名称（默认为默认分支）
  "author": {                // 可选：作者信息
    "name": "string",
    "email": "string"
  },
  "committer": {             // 可选：提交者信息
    "name": "string",
    "email": "string"
  }
}
```

**响应状态码**:
- `200 OK` - 删除成功
- `404 Not Found` - 文件不存在
- `422 Unprocessable Entity` - SHA 不匹配或其他验证错误
- `401 Unauthorized` - 认证失败

## 实现说明

### 后端实现 (Rust)

#### 1. 核心功能 (`src-tauri/src/core/gitea.rs`)

```rust
/// 删除文件
pub async fn delete_file(
    &self,
    remote_path: &str,
    message: &str,
) -> Result<()> {
    // 先获取文件的 SHA
    let file_content = self.get_file(remote_path).await?;
    
    let sha = match file_content {
        Some(content) => content.sha,
        None => return Err(anyhow::anyhow!("文件不存在: {}", remote_path)),
    };

    let url = format!(
        "{}/api/v1/repos/{}/{}/contents/{}",
        self.config.url, self.config.owner, self.config.repo, remote_path
    );

    let body = serde_json::json!({
        "sha": sha,
        "message": message,
    });

    let response = self
        .client
        .request(reqwest::Method::DELETE, &url)
        .header("Authorization", format!("token {}", self.config.token))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await?;

    if !response.status().is_success() {
        // 错误处理...
    }

    Ok(())
}
```

#### 2. Tauri 命令 (`src-tauri/src/commands/gitea.rs`)

```rust
/// 删除 Gitea 文件
#[tauri::command]
pub async fn delete_from_gitea(
    app: AppHandle,
    state: State<'_, GiteaState>,
    fetch_pending: State<'_, BrowserFetchPending>,
    remote_path: String,
    message: Option<String>,
) -> Result<String, String> {
    // 实现逻辑...
}
```

### 前端实现 (TypeScript)

#### 服务函数 (`src/services/gitea.ts`)

```typescript
/**
 * 从 Gitea 删除文件
 */
export async function deleteFromGitea(
  remotePath: string,
  message?: string
): Promise<string> {
  return await invoke<string>('delete_from_gitea', {
    remotePath,
    message,
  });
}
```

## 使用示例

### 在 React 组件中使用

```typescript
import { deleteFromGitea } from '@/services/gitea';

// 删除单个文件
const handleDeleteFile = async (remotePath: string) => {
  try {
    const result = await deleteFromGitea(
      remotePath,
      `删除文件: ${remotePath}`
    );
    console.log(result); // "文件删除成功"
  } catch (error) {
    console.error('删除失败:', error);
  }
};

// 示例：删除脱敏文件
await deleteFromGitea(
  'masked/document.pdf',
  '删除已过期的脱敏文件'
);
```

## 工作流程

1. **获取文件 SHA**
   - 调用 `GET /api/v1/repos/{owner}/{repo}/contents/{filepath}`
   - 从响应中提取 `sha` 字段

2. **执行删除操作**
   - 调用 `DELETE /api/v1/repos/{owner}/{repo}/contents/{filepath}`
   - 在请求体中提供 `sha` 和 `message`

3. **处理响应**
   - 成功：返回删除确认信息
   - 失败：返回错误信息

## 注意事项

1. **SHA 验证**
   - 必须提供正确的文件 SHA 值
   - SHA 用于防止并发修改冲突
   - 如果文件已被修改，SHA 会不匹配，删除会失败

2. **权限要求**
   - 需要有效的 Access Token
   - Token 必须具有仓库写入权限

3. **Git 历史**
   - 删除操作会创建新的 commit
   - 文件内容仍保留在 Git 历史中
   - 可以通过 Git 历史恢复已删除的文件

4. **错误处理**
   - 404: 文件不存在
   - 401: 认证失败
   - 422: SHA 不匹配（文件已被修改）

## 相关功能

- ✅ 上传文件: `uploadToGitea()`
- ✅ 批量上传: `uploadBatchToGitea()`
- ✅ 删除文件: `deleteFromGitea()`
- 🔄 批量删除: 待实现

## 测试建议

1. 先在测试仓库中测试删除功能
2. 确保 Token 权限正确配置
3. 验证删除后文件确实从仓库中移除
4. 测试各种错误场景（文件不存在、权限不足等）

## 更新日志

- 2026-05-13: 添加删除文件功能
  - 后端实现 `delete_file()` 方法
  - 添加 Tauri 命令 `delete_from_gitea`
  - 前端服务函数 `deleteFromGitea()`
  - 支持 UAT 环境和标准 Gitea 环境
