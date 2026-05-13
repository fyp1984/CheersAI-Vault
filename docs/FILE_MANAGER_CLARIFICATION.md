# 文件管理器实现说明

## 当前实现

### FileManager 组件 (`/files` 路由)

**✅ 正确实现 - 实时读取文件系统**

```typescript
const loadFiles = async () => {
  if (!outputDir) {
    setFiles([]);
    setLoading(false);
    return;
  }

  try {
    setLoading(true);
    // 实时读取文件系统中的文件
    const result = await invoke<SandboxFile[]>('list_files_in_directory', { 
      directory: outputDir 
    });
    // 过滤掉 .cmap 对照文件
    const filteredFiles = result.filter(file => !file.name.endsWith('.cmap'));
    
    // 按修改时间降序排序（最新的文件在最上面）
    filteredFiles.sort((a, b) => {
      const dateA = new Date(a.modified).getTime();
      const dateB = new Date(b.modified).getTime();
      return dateB - dateA;
    });
    
    setFiles(filteredFiles);
  } catch (error) {
    console.error('Failed to load files:', error);
    setFiles([]);
  } finally {
    setLoading(false);
  }
};
```

### 后端实现 (`list_files_in_directory` 命令)

**✅ 正确实现 - 直接读取目录**

```rust
#[tauri::command]
pub async fn list_files_in_directory(directory: String) -> Result<Vec<SandboxFile>, String> {
    let dir_path = Path::new(&directory);
    
    if !dir_path.exists() {
        return Ok(vec![]);
    }
    
    if !dir_path.is_dir() {
        return Err("Path is not a directory".to_string());
    }
    
    let mut files = Vec::new();
    
    // 使用 std::fs::read_dir 实时读取目录内容
    match std::fs::read_dir(dir_path) {
        Ok(entries) => {
            for entry in entries {
                // 处理每个文件...
            }
        },
        Err(e) => return Err(format!("Failed to read directory: {}", e)),
    }
    
    Ok(files)
}
```

## 数据流

```
用户打开 /files 页面
    ↓
loadFiles() 被调用
    ↓
invoke('list_files_in_directory', { directory: outputDir })
    ↓
Rust 后端读取文件系统 (std::fs::read_dir)
    ↓
返回实际文件列表
    ↓
前端显示文件列表
```

## 关键特性

### ✅ 实时性
- 每次打开页面都会重新读取文件系统
- 点击"刷新"按钮会重新读取
- 不依赖数据库缓存

### ✅ 准确性
- 显示的是实际文件系统中的文件
- 包含文件的实时大小和修改时间
- 自动过滤 `.cmap` 对照文件

### ✅ 功能完整
- 文件列表显示
- 单个文件上传到 FileBay
- 批量上传到 FileBay
- 单个文件删除
- 批量删除
- 清空目录

## 与数据库的关系

### 数据库用途
数据库 (`managed_files` 表) 主要用于：
- 记录文件处理历史
- 跟踪脱敏操作记录
- 存储文件元数据（脱敏规则、上传状态等）

### FileManager 不使用数据库
FileManager 组件**不读取数据库**，而是：
- 直接读取文件系统
- 显示当前输出目录中的实际文件
- 确保用户看到的是最新的文件状态

## 验证方法

### 测试步骤
1. 打开文件管理页面 (`/files`)
2. 手动在输出目录中添加一个新文件
3. 点击"刷新"按钮
4. **预期结果**：新文件立即出现在列表中

### 对比测试
1. 删除输出目录中的某个文件
2. 刷新文件管理页面
3. **预期结果**：该文件从列表中消失

## 常见问题

### Q: 为什么我看不到某些文件？
A: 可能的原因：
1. 文件在其他目录中（检查输出目录设置）
2. 文件是 `.cmap` 对照文件（会被自动过滤）
3. 文件不在当前选择的输出目录中

### Q: 文件列表不更新怎么办？
A: 解决方案：
1. 点击"刷新"按钮
2. 检查输出目录路径是否正确
3. 确认文件确实存在于输出目录中

### Q: 数据库中的记录会影响文件列表吗？
A: **不会**。文件列表完全基于文件系统，与数据库记录无关。

## 总结

✅ **FileManager 组件已正确实现**
- 实时读取文件系统
- 不依赖数据库记录
- 显示当前输出目录中的实际文件
- 支持完整的文件管理功能

如果用户反馈文件列表不准确，请检查：
1. 输出目录设置是否正确
2. 文件是否确实在该目录中
3. 是否需要点击刷新按钮
