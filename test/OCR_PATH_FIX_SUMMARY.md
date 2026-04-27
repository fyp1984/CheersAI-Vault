# OCR 路径问题修复总结

## 问题描述

用户报告了两个问题：
1. 卸载 OCR 后，PDF 仍然可以解析（说明在使用系统 Python）
2. 安装 OCR 后，点击"开始处理"又弹出"OCR 安装完成"对话框

## 根本原因

### 问题 1: 路径不一致
- **下载路径** (`ocr.rs`): 使用 `app.path().app_data_dir()` → `%APPDATA%\com.cheersai.vault` (Roaming)
- **使用路径** (`file_parser.rs`): 使用 `%LOCALAPPDATA%\com.cheersai.vault` (Local)
- **结果**: 下载到 Roaming，但查找时在 Local，导致找不到下载的 OCR

### 问题 2: 仍在使用系统 Python
- `file_parser.rs` 中的 Method 4 在开发模式下会尝试使用系统 Python
- 即使下载了 OCR 包，如果路径不对，仍会回退到系统 Python

### 问题 3: 对话框重复显示
- 安装完成后，`OcrDownloadDialog` 没有检查 OCR 是否已安装
- 每次打开对话框都显示下载界面，即使已经安装

## 修复方案

### 1. 统一路径为 APPDATA (Roaming)

**修改文件**: `cheersai-desktop/src-tauri/src/core/file_parser.rs`

```rust
// 之前 (错误)
std::env::var("LOCALAPPDATA")  // → C:\Users\{user}\AppData\Local

// 现在 (正确)
std::env::var("APPDATA")       // → C:\Users\{user}\AppData\Roaming
```

**原因**: Tauri 的 `app.path().app_data_dir()` 在 Windows 上返回 `%APPDATA%` (Roaming)，而不是 `%LOCALAPPDATA%` (Local)。

### 2. 完全禁用系统 Python

**修改文件**: `cheersai-desktop/src-tauri/src/core/file_parser.rs`

```rust
// 之前: Method 4 在开发模式下使用系统 Python
if cfg!(debug_assertions) {
    // 尝试系统 Python...
}

// 现在: 完全禁用
println!("System Python is disabled - only downloaded OCR package will be used");
```

**原因**: 确保应用只使用下载的 OCR 包，不依赖系统环境。

### 3. 增强卸载功能

**修改文件**: `cheersai-desktop/src-tauri/src/commands/ocr.rs`

添加了：
- 详细日志，显示删除的文件
- 删除后验证目录是否真的被删除
- 如果删除失败，返回错误

### 4. 简化检查逻辑

**修改文件**: `cheersai-desktop/src-tauri/src/commands/ocr.rs`

```rust
// 之前: 检查下载的 OCR + 系统 Python
pub async fn check_ocr_installed(app: AppHandle) -> Result<bool, String> {
    // 1. 检查下载的 OCR
    // 2. 检查系统 Python
}

// 现在: 只检查下载的 OCR
pub async fn check_ocr_installed(app: AppHandle) -> Result<bool, String> {
    // 只检查下载的 OCR 包
}
```

### 5. 对话框智能检测

**修改文件**: `cheersai-desktop/src/components/file/OcrDownloadDialog.tsx`

添加了：
- 打开对话框时自动检查 OCR 是否已安装
- 如果已安装，直接显示"安装完成"状态
- 如果未安装，显示下载界面
- 关闭对话框时重置所有状态

## 修复后的行为

### 场景 1: 未安装 OCR
1. 用户尝试处理 PDF
2. 系统检测到需要 OCR
3. 弹出对话框，显示"下载 OCR 依赖"
4. 用户点击"开始下载"
5. 下载完成后显示"安装完成"

### 场景 2: 已安装 OCR
1. 用户尝试处理 PDF
2. 系统检测到 OCR 已安装
3. 如果弹出对话框，会立即显示"OCR 安装完成"状态
4. 用户点击"完成"关闭对话框
5. 继续处理 PDF

### 场景 3: 卸载 OCR
1. 用户点击"卸载"按钮
2. 系统删除 `%APPDATA%\com.cheersai.vault\ocr-package\` 目录
3. 日志显示删除的文件列表
4. 验证目录已被删除
5. 下次处理 PDF 时会提示需要下载 OCR

## 路径对照表

| 组件 | 之前的路径 | 现在的路径 | 状态 |
|------|-----------|-----------|------|
| ocr.rs (下载) | `%APPDATA%\Roaming` | `%APPDATA%\Roaming` | ✓ 正确 |
| file_parser.rs (使用) | `%LOCALAPPDATA%\Local` | `%APPDATA%\Roaming` | ✓ 已修复 |
| check_ocr_installed | `%APPDATA%\Roaming` | `%APPDATA%\Roaming` | ✓ 正确 |
| uninstall_ocr_package | `%APPDATA%\Roaming` | `%APPDATA%\Roaming` | ✓ 正确 |

## 测试步骤

### 测试 1: 验证路径一致性

1. 打开应用，点击"下载 OCR 依赖"
2. 下载完成后，打开文件资源管理器
3. 导航到: `C:\Users\{你的用户名}\AppData\Roaming\com.cheersai.vault\ocr-package\`
4. 应该看到 `python\` 文件夹和 `pdf_ocr.py` 文件
5. 尝试处理 PDF，应该成功
6. 查看日志，应该显示:
   ```
   App data directory: C:\Users\...\AppData\Roaming\com.cheersai.vault
   ✓ Using downloaded Python OCR
   ```

### 测试 2: 验证不使用系统 Python

1. 卸载 OCR 包
2. 确认 `AppData\Roaming\com.cheersai.vault\ocr-package\` 目录已删除
3. 尝试处理 PDF
4. 应该显示错误，提示需要下载 OCR
5. 日志应该显示:
   ```
   ✗ Downloaded OCR not found
   System Python is disabled - only downloaded OCR package will be used
   ```

### 测试 3: 验证对话框智能检测

1. 确保 OCR 已安装
2. 尝试处理 PDF（触发 OCR 检查）
3. 如果弹出对话框，应该立即显示"OCR 安装完成"
4. 不应该显示"下载 OCR 依赖"界面

## 关键日志标识

### 路径正确
```
App data directory: C:\Users\...\AppData\Roaming\com.cheersai.vault
Checking downloaded OCR:
  Python: C:\Users\...\AppData\Roaming\com.cheersai.vault\ocr-package\python\python.exe
  Script: C:\Users\...\AppData\Roaming\com.cheersai.vault\ocr-package\pdf_ocr.py
✓ Using downloaded Python OCR
```

### 路径错误（已修复）
```
App data directory: C:\Users\...\AppData\Local\com.cheersai.vault
Checking downloaded OCR:
  Python: C:\Users\...\AppData\Local\com.cheersai.vault\ocr-package\python\python.exe
✗ Downloaded OCR not found
```

### 系统 Python 已禁用
```
System Python is disabled - only downloaded OCR package will be used
```

### OCR 检查
```
Checking OCR installation:
  Python: C:\Users\...\AppData\Roaming\com.cheersai.vault\ocr-package\python\python.exe (exists: true)
  Script: C:\Users\...\AppData\Roaming\com.cheersai.vault\ocr-package\pdf_ocr.py (exists: true)
  Result: Installed
```

## 注意事项

1. **旧的 OCR 包**: 如果之前在 `AppData\Local` 下载过 OCR 包，需要手动删除，因为现在使用 `AppData\Roaming`

2. **开发模式**: 即使在开发模式下，也不会使用系统 Python，确保行为一致

3. **生产构建**: 打包后的应用行为与开发模式完全一致

4. **跨平台**: macOS 和 Linux 的路径逻辑保持不变，只修复了 Windows 的路径问题

## 相关文件

- `cheersai-desktop/src-tauri/src/commands/ocr.rs` - OCR 下载和检查
- `cheersai-desktop/src-tauri/src/core/file_parser.rs` - OCR 使用
- `cheersai-desktop/src/components/file/OcrDownloadDialog.tsx` - OCR 下载对话框
- `cheersai-desktop/src/pages/FileProcess.tsx` - 文件处理页面

## 修复完成时间

2024-XX-XX (根据实际时间填写)
