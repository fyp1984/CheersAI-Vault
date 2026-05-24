# 中文 PDF 字体编码问题修复

## 问题描述

用户上传包含中文字体的 PDF 文件时，使用 `lopdf` 库提取文本会出现 `?Identity-H Unimplemented?` 错误。这是因为 `lopdf` 不支持某些中文字体编码（特别是 Identity-H 编码）。

## 根本原因

- `lopdf` 库对中文 PDF 的字体编码支持有限
- Identity-H 是一种常见的中文字体编码方式，但 `lopdf` 无法解析
- 当 PDF 使用这种编码时，`lopdf` 会返回 `?Identity-H Unimplemented?` 占位符

## 解决方案

### 1. 检测并过滤无效内容（已实现）

在 `src-tauri/src/core/file_parser.rs` 的 `parse_pdf_pages_with_lopdf()` 函数中：

```rust
// 检查是否包含 "Unimplemented" 错误
if !trimmed.is_empty() && !trimmed.contains("Unimplemented") {
    content.push_str(&format!("--- 第 {} 页 ---\n", page_num));
    content.push_str(&text);
    content.push('\n');
    has_content = true;
} else {
    eprintln!("⚠️ 第 {} 页提取失败或包含不支持的编码", page_num);
}
```

如果所有页面都包含 "Unimplemented" 错误，函数会返回错误：

```rust
if !has_content {
    eprintln!("⚠️ lopdf 无法提取有效内容（可能是中文编码问题）");
    return Err(anyhow::anyhow!("lopdf 无法提取有效内容，可能包含不支持的字体编码"));
}
```

### 2. OCR 回退机制（已实现）

在 `src-tauri/src/commands/masking.rs` 中，当 `lopdf` 提取失败时，自动尝试使用 OCR：

```rust
file_parser::FileFormat::Pdf => {
    // 尝试解析 PDF，如果失败则尝试 OCR
    let content = match file_parser::parse_pdf_with_range(&options.file_path, options.page_range) {
        Ok(text) => text,
        Err(e) => {
            eprintln!("⚠️ 使用 lopdf 提取的内容为空，尝试 OCR");
            // 如果指定了页码范围，目前 OCR 不支持分页，返回错误提示
            if options.page_range.is_some() {
                return Err(format!(
                    "PDF 文本提取失败（可能包含不支持的字体编码）。\n\n\
                    当前 OCR 功能暂不支持页码范围提取。\n\n\
                    建议：\n\
                    1. 取消页码范围限制，处理整个文档\n\
                    2. 或使用其他 PDF 工具先提取指定页面，再上传处理\n\n\
                    原始错误: {}", e
                ));
            }
            // 没有页码范围，尝试 OCR 全文提取
            file_parser::parse_pdf_with_range(&options.file_path, None)
                .map_err(|ocr_err| {
                    format!("PDF 文本提取失败: {}\n\nOCR 尝试也失败: {}", e, ocr_err)
                })?
        }
    };
```

## 当前限制

### 页码范围 + 中文 PDF 的限制

如果用户：
1. 上传了包含中文字体编码的 PDF
2. 指定了页码范围（如 1-10 页）
3. `lopdf` 提取失败

系统会提示用户：
- OCR 功能暂不支持页码范围提取
- 建议取消页码范围限制，处理整个文档
- 或使用其他工具先提取指定页面

### 为什么 OCR 不支持页码范围？

当前的 OCR 实现（`parse_pdf_with_python_ocr`）使用 PyMuPDF 或其他 Python 脚本，这些脚本：
1. 接收整个 PDF 文件路径
2. 提取全部内容
3. 没有页码范围参数

## 测试步骤

### 测试场景 1：中文 PDF + 无页码范围

1. 上传包含中文字体的 PDF 文件
2. 不指定页码范围
3. 点击"开始脱敏"
4. **预期结果**：
   - 控制台显示：`⚠️ 使用 lopdf 提取的内容为空，尝试 OCR`
   - 如果 OCR 已安装，应该成功提取文本
   - 如果 OCR 未安装，显示安装提示

### 测试场景 2：中文 PDF + 指定页码范围

1. 上传包含中文字体的 PDF 文件
2. 指定页码范围（如 1-3）
3. 点击"开始脱敏"
4. **预期结果**：
   - 显示错误提示：
     ```
     PDF 文本提取失败（可能包含不支持的字体编码）。
     
     当前 OCR 功能暂不支持页码范围提取。
     
     建议：
     1. 取消页码范围限制，处理整个文档
     2. 或使用其他 PDF 工具先提取指定页面，再上传处理
     ```

### 测试场景 3：普通 PDF + 页码范围

1. 上传不包含中文字体编码问题的 PDF
2. 指定页码范围（如 1-3）
3. 点击"开始脱敏"
4. **预期结果**：
   - 成功提取指定页码范围的内容
   - 文件名包含页码标识：`filename_脱敏_p1-3.txt`

## 未来改进方向

### 短期改进

1. **为 OCR 添加页码范围支持**
   - 修改 Python OCR 脚本，接受页码范围参数
   - 在 `parse_pdf_with_python_ocr` 中传递页码范围
   - 这样中文 PDF 也能支持页码范围提取

### 长期改进

1. **使用更好的 PDF 解析库**
   - 考虑使用 `pdf-extract` 作为主要方法
   - 或集成 `pdfium` 等更强大的 PDF 引擎
   - 这些库对中文字体编码的支持更好

2. **智能检测 PDF 类型**
   - 在上传时检测 PDF 是否包含不支持的字体编码
   - 提前提示用户可能需要使用 OCR
   - 自动选择最佳提取方法

## 相关文件

- `src-tauri/src/core/file_parser.rs` - PDF 解析逻辑
- `src-tauri/src/commands/masking.rs` - 脱敏命令，包含 OCR 回退逻辑
- `src-tauri/src/commands/ocr.rs` - OCR 相关功能

## 日志输出

修复后，在处理中文 PDF 时，控制台会显示：

```
🔍 parse_pdf_with_range called with page_range: Some((1, 3))
✅ Using lopdf for page range: 1-3
⚠️ 第 1 页提取失败或包含不支持的编码
⚠️ 第 2 页提取失败或包含不支持的编码
⚠️ 第 3 页提取失败或包含不支持的编码
⚠️ lopdf 无法提取有效内容（可能是中文编码问题）
⚠️ 使用 lopdf 提取的内容为空，尝试 OCR
```

如果指定了页码范围，会显示错误提示。
如果没有指定页码范围，会尝试 OCR 全文提取。
