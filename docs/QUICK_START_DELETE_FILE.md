# FileBay 删除文件功能 - 快速入门

## 🚀 快速开始

### 前提条件

1. ✅ FileBay 已配置并启用
2. ✅ 仓库已创建
3. ✅ Access Token 有写入权限

### 使用步骤

#### 方法 1: 通过 UI 界面

1. **打开 FileBay 配置页面**
   - 点击左侧导航栏的 "FileBay 配置"

2. **滚动到文件管理部分**
   - 页面底部会显示 "FileBay 文件管理" 区域
   - 仅在 FileBay 配置完成且仓库存在时显示

3. **输入文件信息**
   - **文件路径**: 输入要删除的文件路径（例如：`masked/test.pdf`）
   - **提交信息**: 可选，输入自定义的 Git commit 消息

4. **执行删除**
   - 点击 "删除文件" 按钮
   - 在确认对话框中确认删除
   - 等待操作完成

5. **查看结果**
   - 成功：显示 "文件删除成功"
   - 失败：显示具体的错误信息

#### 方法 2: 在代码中使用

```typescript
import { deleteFromGitea } from '@/services/gitea';

// 基本用法
const result = await deleteFromGitea('masked/document.pdf');

// 带自定义提交信息
const result = await deleteFromGitea(
  'masked/document.pdf',
  '删除已过期的脱敏文件'
);

// 完整示例（带错误处理）
try {
  const result = await deleteFromGitea(
    'reports/2024/annual-report.pdf',
    '删除旧版本报告'
  );
  console.log(result); // "文件删除成功"
  // 执行后续操作...
} catch (error) {
  console.error('删除失败:', error);
  // 处理错误...
}
```

## 📝 常见使用场景

### 场景 1: 删除单个脱敏文件

```typescript
// 删除根目录下的文件
await deleteFromGitea('test.pdf', '删除测试文件');

// 删除子目录中的文件
await deleteFromGitea('masked/document.pdf', '删除已处理的文件');
```

### 场景 2: 清理过期文件

```typescript
// 删除过期的报告
await deleteFromGitea(
  'reports/2023/old-report.pdf',
  '清理过期报告'
);

// 删除临时文件
await deleteFromGitea(
  'temp/cache.json',
  '清理临时缓存'
);
```

### 场景 3: 在文件管理器中集成

```typescript
const handleDeleteFile = async (filePath: string) => {
  const confirmed = window.confirm(`确定要删除 ${filePath} 吗？`);
  if (!confirmed) return;

  try {
    await deleteFromGitea(filePath, `删除文件: ${filePath}`);
    // 刷新文件列表
    await refreshFileList();
    showSuccessMessage('文件已删除');
  } catch (error) {
    showErrorMessage(`删除失败: ${error}`);
  }
};
```

## ⚠️ 注意事项

### 文件路径格式

✅ **正确的路径格式**:
- `document.pdf` - 根目录文件
- `masked/document.pdf` - 子目录文件
- `reports/2024/annual.pdf` - 多级目录

❌ **错误的路径格式**:
- `/document.pdf` - 不要以 `/` 开头
- `./masked/document.pdf` - 不要使用 `./`
- `masked\document.pdf` - 使用 `/` 而不是 `\`

### 权限要求

- ✅ Token 必须有 `repo` 权限
- ✅ 用户必须是仓库的所有者或协作者
- ✅ 仓库不能是只读状态

### 删除行为

- 📝 删除操作会创建新的 Git commit
- 🔄 文件内容仍保留在 Git 历史中
- ♻️ 可以通过 Git 历史恢复文件
- ⚡ 删除是即时生效的

## 🔍 故障排查

### 问题 1: "文件不存在"

**原因**: 文件路径错误或文件已被删除

**解决方案**:
1. 检查文件路径是否正确
2. 确认文件确实存在于仓库中
3. 检查路径中的大小写是否正确

### 问题 2: "认证失败"

**原因**: Token 无效或权限不足

**解决方案**:
1. 重新生成 Access Token
2. 确保 Token 有 `repo` 权限
3. 在 FileBay 设置中更新 Token

### 问题 3: "文件版本冲突"

**原因**: 文件在删除前被其他人修改

**解决方案**:
1. 刷新文件信息
2. 重新尝试删除
3. 如果持续失败，检查是否有并发操作

### 问题 4: "FileBay 功能未启用"

**原因**: FileBay 未配置或未启用

**解决方案**:
1. 进入 FileBay 配置页面
2. 完成配置（URL、Token、Owner、Repo）
3. 启用 FileBay 功能
4. 确保仓库已创建

## 📊 API 响应

### 成功响应

```
"文件删除成功"
```

### 错误响应

| 错误消息 | 含义 | 解决方案 |
|---------|------|----------|
| `文件不存在` | 文件路径错误 | 检查路径 |
| `认证失败，请检查 Token 配置` | Token 无效 | 更新 Token |
| `删除失败：文件版本冲突` | 文件已被修改 | 刷新后重试 |
| `FileBay 功能未启用` | 未启用 FileBay | 启用功能 |

## 🎯 最佳实践

### 1. 始终提供有意义的提交信息

```typescript
// ❌ 不好
await deleteFromGitea('file.pdf');

// ✅ 好
await deleteFromGitea('file.pdf', '删除已过期的客户报告');
```

### 2. 删除前确认

```typescript
// ✅ 使用确认对话框
const confirmed = await showConfirmDialog(
  '确定要删除这个文件吗？',
  '此操作不可撤销'
);
if (confirmed) {
  await deleteFromGitea(filePath);
}
```

### 3. 处理错误

```typescript
// ✅ 完整的错误处理
try {
  await deleteFromGitea(filePath, message);
  showSuccess('删除成功');
} catch (error) {
  if (error.includes('不存在')) {
    showError('文件不存在，可能已被删除');
  } else if (error.includes('认证失败')) {
    showError('权限不足，请检查配置');
  } else {
    showError(`删除失败: ${error}`);
  }
}
```

### 4. 记录删除操作

```typescript
// ✅ 记录删除日志
await deleteFromGitea(filePath, message);
await logOperation({
  action: 'delete',
  file: filePath,
  timestamp: new Date(),
  user: currentUser,
});
```

## 🔗 相关功能

- [上传文件](./GITEA_DELETE_API.md#上传文件)
- [批量上传](./GITEA_DELETE_API.md#批量上传)
- [文件管理](./GITEA_DELETE_API.md#文件管理)

## 📚 更多资源

- [完整 API 文档](./GITEA_DELETE_API.md)
- [实现细节](./GITEA_DELETE_IMPLEMENTATION.md)
- [Gitea API 官方文档](https://docs.gitea.io/en-us/api-usage/)

## 💡 提示

- 删除操作是安全的，文件内容保留在 Git 历史中
- 可以通过 Git 命令恢复已删除的文件
- 建议在删除重要文件前先备份
- 使用有意义的提交信息便于追踪历史

## 🎉 完成！

现在你已经掌握了如何使用 FileBay 删除文件功能。如有问题，请查看[故障排查](#-故障排查)部分或联系技术支持。
