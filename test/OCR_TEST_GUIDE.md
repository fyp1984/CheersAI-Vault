# OCR 功能测试指南

## 修复内容

### 1. 禁用系统 Python
- **之前**: 开发模式和生产模式都可能使用系统 Python
- **现在**: 完全禁用系统 Python，只使用下载的 OCR 包

### 2. 增强卸载功能
- 添加详细日志，显示删除的文件
- 验证删除是否成功
- 确保目录完全清理

### 3. 简化检查逻辑
- `check_ocr_installed` 只检查下载的 OCR 包
- 不再检查系统 Python

## 测试步骤

### 测试 1: 验证不使用系统 Python

1. **确保没有下载 OCR 包**
   - 打开文件资源管理器
   - 导航到: `C:\Users\{你的用户名}\AppData\Local\com.cheersai.vault\`
   - 如果存在 `ocr-package` 文件夹，删除它

2. **尝试处理 PDF**
   - 在应用中选择一个 PDF 文件
   - 点击"开始处理"
   - **预期结果**: 应该显示错误，提示需要下载 OCR 包
   - **不应该**: 使用系统 Python 成功处理

3. **检查日志**
   - 在终端中查看日志输出
   - 应该看到:
     ```
     Checking downloaded OCR:
       Python: C:\Users\...\AppData\Local\com.cheersai.vault\ocr-package\python\python.exe
       Script: C:\Users\...\AppData\Local\com.cheersai.vault\ocr-package\pdf_ocr.py
     ✗ Downloaded OCR not found
     Checking bundled OCR:
       ...
     ✗ Bundled OCR not found
     System Python is disabled - only downloaded OCR package will be used
     ```

### 测试 2: 下载并使用 OCR 包

1. **下载 OCR 包**
   - 点击"下载 OCR 依赖"按钮
   - 等待下载完成（约 100MB）

2. **验证安装**
   - 打开文件资源管理器
   - 导航到: `C:\Users\{你的用户名}\AppData\Local\com.cheersai.vault\ocr-package\`
   - 应该看到:
     ```
     ocr-package/
       ├── python/
       │   ├── python.exe
       │   ├── python311.dll
       │   └── ... (其他 Python 文件)
       └── pdf_ocr.py
     ```

3. **处理 PDF**
   - 选择一个 PDF 文件
   - 点击"开始处理"
   - **预期结果**: 成功处理 PDF
   - 日志应该显示: `✓ Using downloaded Python OCR`

### 测试 3: 卸载功能

1. **卸载 OCR 包**
   - 点击"卸载 OCR"按钮
   - 查看终端日志

2. **验证日志输出**
   应该看到类似:
   ```
   Uninstalling OCR package from: "C:\Users\...\AppData\Local\com.cheersai.vault\ocr-package"
   OCR directory exists, removing...
     - Removing: "C:\Users\...\ocr-package\python"
     - Removing: "C:\Users\...\ocr-package\pdf_ocr.py"
   ✓ OCR package uninstalled successfully
   ```

3. **验证文件系统**
   - 打开文件资源管理器
   - 导航到: `C:\Users\{你的用户名}\AppData\Local\com.cheersai.vault\`
   - **预期结果**: `ocr-package` 文件夹应该不存在

4. **再次尝试处理 PDF**
   - 选择一个 PDF 文件
   - 点击"开始处理"
   - **预期结果**: 应该显示错误，提示需要下载 OCR 包
   - **不应该**: 使用系统 Python 成功处理

### 测试 4: 检查 OCR 状态

1. **未安装状态**
   - 确保 OCR 包已卸载
   - 打开应用，查看 OCR 状态
   - **预期结果**: 显示"未安装"

2. **已安装状态**
   - 下载 OCR 包
   - 刷新或重启应用
   - **预期结果**: 显示"已安装"

## 关键日志标识

### 成功使用下载的 OCR
```
✓ Using downloaded Python OCR
OCR completed: XXX chars extracted
```

### 未找到 OCR 包
```
✗ Downloaded OCR not found
✗ Bundled OCR not found
System Python is disabled - only downloaded OCR package will be used
```

### 卸载成功
```
✓ OCR package uninstalled successfully
```

## 常见问题

### Q: 卸载后仍然可以处理 PDF？
**A**: 这说明还在使用系统 Python。检查代码是否正确禁用了 Method 4。

### Q: 下载后找不到 OCR 包？
**A**: 检查路径是否正确：
- 下载路径: `app.path().app_data_dir()` → `%LOCALAPPDATA%\com.cheersai.vault\ocr-package`
- 使用路径: 应该与下载路径一致

### Q: 日志显示路径不一致？
**A**: 检查 `file_parser.rs` 中的 `app_data_dir` 计算逻辑，确保与 `ocr.rs` 一致。

## 预期行为总结

| 场景 | 预期行为 |
|------|---------|
| 未安装 OCR + 处理 PDF | 显示错误，提示下载 OCR |
| 已安装 OCR + 处理 PDF | 成功处理，使用下载的 OCR |
| 卸载 OCR | 完全删除 ocr-package 文件夹 |
| 卸载后 + 处理 PDF | 显示错误，提示下载 OCR |
| 检查状态（未安装） | 返回 false |
| 检查状态（已安装） | 返回 true |

**重要**: 在任何情况下都不应该使用系统 Python！
