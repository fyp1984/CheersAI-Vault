# 卸载功能更新文档

## 更新内容

已将 `EnhancedServices.tsx` 页面中的卸载功能更新为使用 Python 脚本进行完全卸载。

## 更改详情

### OCR 服务卸载

**之前：**
- 使用 `tauriCommands.uninstallOcrPackage()`
- 只删除应用数据目录中的 OCR 包

**现在：**
- 使用 `tauriCommands.uninstallOcrWithScript()`
- 调用 `scripts/install_ocr.py uninstall`
- 完全删除整个 OCR 环境目录
- 包括 Python、pip、PyMuPDF、EasyOCR 等所有文件

**确认对话框：**
```
确定要完全卸载 OCR 服务吗？这将删除所有 OCR 相关文件。卸载后将无法处理扫描版 PDF 文件。
```

### Ollama + AI 模型卸载

**之前：**
- 使用 `tauriCommands.uninstallAiModel()`
- 只删除 AI 模型（使用 `ollama rm` 命令）
- Ollama 程序本身保留

**现在：**
- 使用 `tauriCommands.uninstallOllamaWithScript()`
- 调用 `scripts/install_ollama.py uninstall`
- 完全卸载 Ollama：
  - 停止所有 Ollama 进程（ollama.exe, ollama app.exe）
  - 删除程序文件（`%LOCALAPPDATA%\Programs\Ollama`）
  - 删除用户数据（`%USERPROFILE%\.ollama`）
  - 删除所有模型

**确认对话框：**
```
确定要完全卸载 Ollama 和 AI 模型吗？这将删除 Ollama 程序、所有模型和用户数据。卸载后将无法使用智能脱敏功能。
```

### UI 更新

**按钮文本：**
- OCR: "卸载服务" → "完全卸载"
- AI 模型: "卸载模型" → "完全卸载"

**卸载过程提示：**
- 显示 "正在卸载..." 的信息提示
- 卸载完成后显示成功消息
- 自动刷新服务状态

## 文件修改

### 前端
- `src/pages/EnhancedServices.tsx`
  - `handleUninstallOcr()` - 更新为使用脚本卸载
  - `handleUninstallAiModel()` - 更新为使用脚本卸载
  - 按钮文本更新为"完全卸载"
  - 确认对话框文本更新

## 卸载流程

### OCR 卸载流程
1. 用户点击"完全卸载"按钮
2. 显示确认对话框
3. 调用 `uninstallOcrWithScript()`
4. 后端运行 `python install_ocr.py uninstall`
5. 脚本删除整个 OCR 目录
6. 返回成功消息
7. 刷新服务状态

### Ollama 卸载流程
1. 用户点击"完全卸载"按钮
2. 显示确认对话框
3. 调用 `uninstallOllamaWithScript()`
4. 后端运行 `python install_ollama.py uninstall`
5. 脚本执行：
   - 停止 ollama.exe 和 ollama app.exe 进程
   - 删除 `%LOCALAPPDATA%\Programs\Ollama`
   - 删除 `%USERPROFILE%\.ollama`
6. 返回成功消息
7. 刷新服务状态

## 优势

### 相比之前的实现

1. **更彻底的清理**
   - 之前：只删除部分文件
   - 现在：完全删除所有相关文件和目录

2. **一致的卸载体验**
   - 之前：OCR 和 Ollama 卸载方式不同
   - 现在：都使用脚本进行完全卸载

3. **更好的用户提示**
   - 明确告知用户将删除哪些内容
   - 卸载过程中显示实时状态

4. **更可靠的卸载**
   - 脚本处理进程停止、权限问题等
   - 完整的错误处理和日志输出

## 测试建议

1. **OCR 卸载测试**
   ```
   1. 安装 OCR 服务
   2. 验证 OCR 功能正常
   3. 点击"完全卸载"
   4. 确认所有文件已删除
   5. 验证无法使用 OCR 功能
   ```

2. **Ollama 卸载测试**
   ```
   1. 安装 Ollama + AI 模型
   2. 验证 AI 功能正常
   3. 点击"完全卸载"
   4. 确认 Ollama 进程已停止
   5. 确认程序文件已删除
   6. 确认用户数据已删除
   7. 验证无法使用 AI 功能
   ```

3. **重新安装测试**
   ```
   1. 完全卸载服务
   2. 重新安装服务
   3. 验证功能正常
   ```

## 注意事项

1. **数据丢失警告**
   - 卸载会删除所有相关数据
   - 用户需要明确确认

2. **进程停止**
   - Ollama 卸载会强制停止所有相关进程
   - 可能影响正在进行的操作

3. **权限问题**
   - 某些文件可能被锁定
   - 脚本会尝试处理权限问题
   - 如果失败，会提示用户重启后再试

4. **Python 依赖**
   - 卸载功能需要 Python 环境
   - 如果 Python 不可用，卸载会失败

## 兼容性

- ✅ Windows 10/11
- ✅ 需要 Python 3.7+
- ✅ 向后兼容（旧的卸载命令仍然可用）

## 相关文件

- `src/pages/EnhancedServices.tsx` - 前端页面
- `src-tauri/src/commands/installer.rs` - 后端命令
- `scripts/install_ocr.py` - OCR 安装/卸载脚本
- `scripts/install_ollama.py` - Ollama 安装/卸载脚本
