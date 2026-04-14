# OCR 功能完整设置指南

## 快速开始

### 用户：安装 OCR 功能

1. 运行安装脚本：
   ```batch
   install_ocr_windows.bat
   ```

2. 等待安装完成（约 2-5 分钟）

3. 重启应用，尝试处理扫描版 PDF

### 开发者：打包 OCR 到安装包

1. 构建 OCR 可执行文件：
   ```batch
   build_ocr.bat
   ```

2. 验证生成的文件：
   ```batch
   dist\pdf_ocr.exe test.pdf
   ```

3. 构建完整安装包：
   ```batch
   pnpm tauri build
   ```

## 详细说明

### 文件说明

- `src-tauri/scripts/pdf_ocr.py` - OCR Python 脚本
- `install_ocr_windows.bat` - 用户安装脚本
- `build_ocr.bat` - 开发者打包脚本
- `dist/pdf_ocr.exe` - 打包后的 OCR 可执行文件（约 500MB）

### 工作原理

1. **PDF 解析**：首先尝试直接提取 PDF 文本
2. **OCR 回退**：如果失败，自动启用 OCR
3. **超时保护**：90 秒超时，避免卡死
4. **友好提示**：失败时提供详细的解决方案

### 配置文件

`src-tauri/tauri.conf.json`:
```json
{
  "bundle": {
    "resources": [
      "scripts/*",
      "../dist/pdf_ocr.exe"
    ]
  }
}
```

### 代码逻辑

`src-tauri/src/core/file_parser.rs`:
```rust
// 1. 尝试直接解析 PDF
let result = extract_text(path);

// 2. 如果失败，尝试 OCR
if result.is_err() || text.is_empty() {
    parse_pdf_with_python_ocr(path)
}

// 3. OCR 优先使用打包的 exe
if bundled_exe.exists() {
    run_ocr_command(&bundled_exe, &[path], 90)
} else {
    // 回退到 Python 脚本
    run_ocr_command("python", &[script_path, path], 90)
}
```

## 常见问题

### Q: 为什么 OCR exe 这么大？

A: OCR 包含深度学习模型和依赖库，总大小约 500MB。这是正常的。

### Q: 可以减小体积吗？

A: 可以，但会影响功能：
- 使用 UPX 压缩（减小 30-40%）
- 只支持英文（减小 50%）
- 使用 Tesseract 替代 EasyOCR（减小到 50MB，但识别率下降）

### Q: 首次使用为什么慢？

A: 如果使用 Python 脚本模式，首次会下载 OCR 模型（约 100MB）。使用打包的 exe 则不需要。

### Q: 如何调试 OCR 问题？

A: 
1. 查看控制台输出
2. 手动运行：`dist\pdf_ocr.exe test.pdf`
3. 检查 Python 环境：`python --version`
4. 检查依赖：`pip list | findstr easyocr`

## 发布清单

### 精简版（不含 OCR）

- [ ] 移除 `tauri.conf.json` 中的 `../dist/pdf_ocr.exe`
- [ ] 构建：`pnpm tauri build`
- [ ] 测试安装包
- [ ] 包含 `install_ocr_windows.bat` 在发布包中
- [ ] 更新文档说明如何安装 OCR

### 完整版（含 OCR）

- [ ] 运行 `build_ocr.bat`
- [ ] 验证 `dist/pdf_ocr.exe` 存在
- [ ] 确认 `tauri.conf.json` 包含 OCR exe
- [ ] 构建：`pnpm tauri build`
- [ ] 测试 OCR 功能
- [ ] 验证安装包大小（约 500MB）

## 性能基准

- 小文件（<5 页）：10-30 秒
- 中等文件（5-20 页）：30-90 秒
- 大文件（>20 页）：可能超时，建议分批处理

## 支持的 PDF 类型

- ✅ 文本型 PDF（直接提取）
- ✅ 扫描版 PDF（OCR 识别）
- ✅ 混合型 PDF（自动选择最佳方式）
- ✅ 中英文混合
- ⚠️ 手写文字（识别率较低）
- ❌ 图片型 PDF（需要先转换）

## 更新日志

### v1.0 (当前版本)
- ✅ 实现 OCR 基础功能
- ✅ 添加超时保护（90 秒）
- ✅ 支持打包 exe 和 Python 脚本两种模式
- ✅ 友好的错误提示
- ✅ 实时进度输出

### 计划中
- [ ] GPU 加速支持
- [ ] 云端 OCR API 集成
- [ ] 批量 OCR 优化
- [ ] 进度条显示
