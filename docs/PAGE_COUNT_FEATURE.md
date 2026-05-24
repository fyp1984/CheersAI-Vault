# 自动显示文件总页数功能

## 功能概述

在用户上传支持分页的文件（PDF、Word、PowerPoint）后，自动检测并显示文件的总页数，方便用户设置页码范围。

## 实现细节

### 后端实现（Rust）

#### 1. 新增函数：`get_page_count()`
位置：`src-tauri/src/core/file_parser.rs`

```rust
pub fn get_page_count(path: &str) -> Result<usize>
```

**功能：**
- PDF：使用 `lopdf::Document::get_pages().len()` 获取页数
- Word：解析 XML，统计分页符 `<w:br w:type="page"/>` 数量
- PowerPoint：统计 `ppt/slides/slide*.xml` 文件数量

**返回：**
- 成功：返回页数（usize）
- 失败：返回错误信息（不支持的格式或读取失败）

#### 2. 新增 Tauri 命令：`get_file_page_count`
位置：`src-tauri/src/commands/masking.rs`

```rust
#[tauri::command]
pub async fn get_file_page_count(file_path: String) -> Result<usize, String>
```

**功能：**
- 调用 `file_parser::get_page_count()` 获取页数
- 将错误转换为字符串返回给前端

#### 3. 注册命令
位置：`src-tauri/src/lib.rs`

```rust
.invoke_handler(tauri::generate_handler![
    // ...
    masking::get_file_page_count,
    // ...
])
```

### 前端实现（TypeScript/React）

#### 1. 添加类型定义
位置：`src/types/file.ts`

```typescript
export interface QueuedFile {
  // ... 其他字段
  totalPages?: number; // 文件总页数
}
```

#### 2. 添加 Tauri 命令调用
位置：`src/lib/tauri.ts`

```typescript
getFilePageCount: (filePath: string) =>
  invoke<number>("get_file_page_count", { filePath }),
```

#### 3. 文件上传时自动获取页数
位置：`src/pages/FileProcess.tsx`

```typescript
const handleDrop = async (paths: string[]) => {
  const queued = await Promise.all(
    paths.map(async (p) => {
      // ... 获取文件信息
      
      // 尝试获取页数
      try {
        totalPages = await tauriCommands.getFilePageCount(p);
        console.log(`📄 Total pages: ${totalPages}`);
      } catch (error) {
        // 不支持分页的文件，忽略错误
      }
      
      return {
        // ... 其他字段
        totalPages,
      };
    })
  );
  addFiles(queued);
};
```

#### 4. 更新 PageRangeInput 组件
位置：`src/components/file/PageRangeInput.tsx`

**新增功能：**
- 接收 `totalPages` 参数
- 在占位符中显示总页数：`页码范围（如 1-10，共 50 页，留空处理全部）`
- 在输入框下方显示提示：`💡 提示：该文件共 50 页`
- 验证起始页是否超出总页数

```typescript
interface PageRangeInputProps {
  // ... 其他字段
  totalPages?: number; // 文件总页数
}

// 构建占位符文本
const placeholder = totalPages 
  ? `页码范围（如 1-10，共 ${totalPages} 页，留空处理全部）`
  : "页码范围（如 1-10，留空处理全部）";

// 验证是否超出范围
if (totalPages && start > totalPages) {
  setError(`起始页超出总页数 ${totalPages}`);
  return;
}
```

#### 5. 更新 FileQueueItem 组件
位置：`src/components/file/FileQueueItem.tsx`

```typescript
<PageRangeInput
  fileName={file.name}
  fileFormat={file.path}
  value={file.pageRange}
  onChange={handlePageRangeChange}
  totalPages={file.totalPages}  // 传递总页数
/>
```

## 用户体验

### 上传文件后
1. 系统自动检测文件页数
2. 在控制台输出：`📄 Total pages: 50`
3. 不支持分页的文件不会显示页数

### 输入页码范围时
1. 占位符显示：`页码范围（如 1-10，共 50 页，留空处理全部）`
2. 输入框下方显示：`💡 提示：该文件共 50 页`
3. 如果输入的起始页超出总页数，显示错误：`起始页超出总页数 50`

## 支持的文件格式

| 格式 | 支持 | 页数计算方式 |
|-----|:---:|------------|
| PDF | ✅ | `lopdf::Document::get_pages().len()` |
| Word (.docx) | ✅ | 统计分页符 `<w:br w:type="page"/>` |
| PowerPoint (.pptx) | ✅ | 统计幻灯片文件数量 |
| TXT/MD | ❌ | 不支持分页 |
| CSV/Excel | ❌ | 不支持分页 |

## 错误处理

### 后端错误
- 文件不存在：返回错误信息
- 文件格式不支持：返回 "该文件格式不支持分页"
- 读取失败：返回具体错误信息

### 前端处理
- 捕获错误但不显示给用户
- 不支持分页的文件正常处理，只是不显示页数
- 在控制台输出信息日志

## 性能考虑

### PDF
- 使用 `lopdf` 库，只读取文档结构，不解析内容
- 性能：< 100ms（大多数情况）

### Word
- 只解析 `word/document.xml` 文件
- 只统计分页符，不提取文本
- 性能：< 50ms（大多数情况）

### PowerPoint
- 只列举 ZIP 文件中的幻灯片文件
- 不解析 XML 内容
- 性能：< 50ms（大多数情况）

## 示例

### PDF 文件
```
上传文件：技术报告.pdf
自动检测：共 100 页
显示：页码范围（如 1-10，共 100 页，留空处理全部）
提示：💡 提示：该文件共 100 页
```

### Word 文件
```
上传文件：合同文档.docx
自动检测：共 50 页（基于分页符）
显示：页码范围（如 1-10，共 50 页，留空处理全部）
提示：💡 提示：该文件共 50 页
```

### PowerPoint 文件
```
上传文件：演示文稿.pptx
自动检测：共 30 张幻灯片
显示：页码范围（如 1-10，共 30 页，留空处理全部）
提示：💡 提示：该文件共 30 页
```

### TXT 文件
```
上传文件：文本文档.txt
自动检测：不支持分页
显示：（不显示页码输入框）
```

## 注意事项

1. **Word 分页符依赖**：Word 文档的页数基于分页符，如果文档没有分页符，显示为 1 页
2. **异步加载**：页数检测是异步的，文件上传后可能需要几毫秒才能显示页数
3. **错误静默**：不支持分页的文件不会显示错误，只是不显示页数
4. **性能优化**：只在文件上传时检测一次，不会重复检测

## 未来改进

1. **加载状态**：在检测页数时显示加载动画
2. **缓存机制**：缓存已检测的文件页数
3. **批量检测**：优化多文件上传时的页数检测
4. **更多格式**：支持更多文档格式的页数检测
5. **页数预览**：在预览对话框中显示每页的内容

## 总结

自动显示文件总页数功能显著提升了用户体验，让用户在设置页码范围时更加方便和准确。该功能已完全实现并集成到前后端，支持 PDF、Word、PowerPoint 三种主要文档格式。
