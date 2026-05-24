# 快速测试指南

## 问题修复

✅ **已修复**：开发环境中 OCR 脚本路径错误
- 之前：`scripts/pdf_ocr.py`
- 现在：`src-tauri/scripts/pdf_ocr.py`

## 环境检查

✅ **Python**: 3.13.5 已安装
✅ **PyMuPDF**: 已安装
✅ **OCR 脚本**: `src-tauri/scripts/pdf_ocr.py` 存在
✅ **应用**: 已重新编译并运行

## 测试步骤

### 1. 上传中文 PDF + 页码范围

1. 打开应用：http://localhost:5173/
2. 上传包含中文字体编码问题的 PDF
3. 填写页码范围：`1-2`
4. 点击"开始脱敏"

### 2. 预期日志输出

```
🔍 parse_pdf_with_range called with page_range: Some((1, 2))
✅ Using lopdf for page range: 1-2
⚠️ 第 1 页提取失败或包含不支持的编码
⚠️ 第 2 页提取失败或包含不支持的编码
⚠️ lopdf 无法提取有效内容（可能是中文编码问题）
⚠️ lopdf 失败，尝试使用 OCR 提取页码范围 1-2
🔍 OCR with page range: 1-2
Opening PDF: E:\path\to\file.pdf
PDF has X pages
Extracting pages 1-2 (indices 0-1)
Processing page 1/X...
Page 1 extracted XXX characters
Processing page 2/X...
Page 2 extracted XXX characters
Total extracted: XXX characters
```

### 3. 预期结果

- ✅ 成功提取第 1-2 页的文本
- ✅ 文件名：`文件名_脱敏_p1-2.txt`
- ✅ 文件内容包含提取的文本
- ✅ 生成映射文件：`文件名_脱敏_p1-2.txt.cmap`

## 如果仍然失败

### 检查 1：查看完整错误信息

在控制台中查找错误信息，特别是：
- `ERROR: ...`
- `Failed to ...`
- Python 异常堆栈

### 检查 2：手动测试 OCR 脚本

```bash
# 创建一个测试 PDF 文件路径
python src-tauri/scripts/pdf_ocr.py "path/to/your/test.pdf" 1 2
```

如果手动测试成功，说明脚本本身没问题，可能是 Rust 调用的问题。

### 检查 3：查看 Rust 错误处理

OCR 调用失败时，错误可能被吞掉了。检查 `run_ocr_command` 函数的错误处理。

## 常见问题

### Q: 日志停在 "🔍 OCR with page range: 1-2"

**可能原因**：
1. OCR 脚本路径错误（已修复）
2. Python 命令找不到
3. OCR 脚本执行失败但错误被吞掉

**解决方案**：
- 检查 `run_ocr_command` 的错误输出
- 在 `file_parser.rs` 中添加更多调试日志

### Q: 提示 "OCR 功能未安装"

**解决方案**：
1. 点击"下载 OCR 依赖"按钮
2. 或手动安装：`pip install PyMuPDF`

### Q: OCR 处理整个文档而不是指定页面

**检查**：
- 确认日志中有 `🔍 OCR with page range: X-Y`
- 确认日志中有 `Extracting pages X-Y (indices ...)`

## 调试技巧

### 添加更多日志

在 `src-tauri/src/core/file_parser.rs` 的 `run_ocr_command` 函数中添加：

```rust
eprintln!("🔍 Running OCR command: {} {:?}", command, args);
```

在 Python 脚本中添加：

```python
print(f"DEBUG: Received args: {sys.argv}", file=sys.stderr)
```

### 查看 Python 输出

OCR 脚本的 stderr 输出会显示在控制台中，包括：
- 处理进度
- 错误信息
- 调试信息

## 成功标志

✅ 看到完整的 OCR 处理日志
✅ 生成了脱敏文件
✅ 文件名包含页码范围
✅ 文件内容正确
✅ 没有错误提示

## 下一步

如果测试成功：
1. 测试不同的页码范围
2. 测试更大的 PDF 文件
3. 测试批量处理

如果测试失败：
1. 提供完整的控制台日志
2. 提供错误信息
3. 说明 PDF 文件的特征（页数、大小、来源）
