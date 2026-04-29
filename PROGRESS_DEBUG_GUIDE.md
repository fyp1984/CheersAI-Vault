# 进度显示调试指南

## 🔍 问题

前端没有显示 Ollama 安装的下载进度。

## ✅ 已完成的修复

### 1. 修复了事件监听器 (src/pages/EnhancedServices.tsx)

**问题：** `listen` 函数返回 Promise，但没有正确处理和清理

**修复：**
```typescript
useEffect(() => {
  const unlistenPromises: Promise<() => void>[] = [];
  
  // 监听 Ollama 安装进度
  unlistenPromises.push(
    listen<InstallerProgress>('ollama-install-progress', (event) => {
      console.log('Ollama install progress:', event.payload);
      setDownloadProgress(prev => ({ ...prev, aiModel: event.payload.percentage }));
      setProgressStatus(prev => ({ ...prev, aiModel: event.payload.status }));
    })
  );
  
  // ... 其他监听器
  
  // 清理函数
  return () => {
    Promise.all(unlistenPromises).then(unlisteners => {
      unlisteners.forEach(unlisten => unlisten());
    });
  };
}, []);
```

**改进：**
- ✅ 正确处理 `listen` 返回的 Promise
- ✅ 添加了清理函数防止内存泄漏
- ✅ 添加了 console.log 用于调试

## 🧪 测试步骤

### 1. 打开浏览器开发者工具

在应用中按 `F12` 或右键 → 检查，打开开发者工具的 Console 标签。

### 2. 点击"一键安装" AI 模型

观察以下内容：

#### A. 控制台日志
应该看到：
```
Ollama install progress: { percentage: 5.0, status: "正在获取模型清单...", log: "..." }
Ollama install progress: { percentage: 10.5, status: "下载进度: 10.5%...", log: "..." }
Ollama install progress: { percentage: 25.3, status: "下载进度: 25.3%...", log: "..." }
...
```

#### B. UI 进度条
应该看到：
- 进度条从 0% 开始增长
- 状态文本实时更新
- 百分比数字实时更新

### 3. 后端日志

在运行 `pnpm tauri dev` 的终端中，应该看到：
```
=== Installing Ollama using script ===
Running installer script:
  Python: python
  Script: "C:\Users\...\Temp\cheersai_scripts\install_ollama.py"
[2024-01-01 12:00:00] [INFO] 开始下载 Ollama 安装程序...
[2024-01-01 12:00:01] [INFO] 下载进度: 10.5% (...)
...
```

## 🐛 如果仍然没有进度

### 检查清单

1. **前端是否接收到事件？**
   - 打开浏览器控制台
   - 查找 `Ollama install progress:` 日志
   - 如果没有，说明事件没有发送或监听器没有注册

2. **后端是否发送事件？**
   - 查看终端日志
   - 确认 Python 脚本正在运行
   - 确认有日志输出

3. **Python 脚本是否输出进度？**
   - 检查是否有 `[INFO]` 日志
   - 检查是否有百分比 `%` 符号

### 常见问题

#### 问题 1: 控制台没有 "Ollama install progress" 日志

**原因：** 事件监听器没有注册或事件名称不匹配

**解决：**
- 确认前端使用 `ollama-install-progress`
- 确认后端发送 `ollama-install-progress`
- 重新加载页面（Ctrl+R）

#### 问题 2: 有日志但进度条不显示

**原因：** `downloadProgress.aiModel` 为 0 或 UI 条件不满足

**检查：**
```typescript
// 进度条显示条件
{installing.aiModel && downloadProgress.aiModel > 0 && (
  // 进度条 UI
)}
```

**解决：**
- 确认 `installing.aiModel` 为 `true`
- 确认 `downloadProgress.aiModel > 0`
- 在控制台运行：
  ```javascript
  // 查看状态
  console.log('installing:', window.__REACT_DEVTOOLS_GLOBAL_HOOK__.renderers);
  ```

#### 问题 3: Python 脚本没有输出

**原因：** 输出被缓冲

**已修复：** 后端使用 `python -u` 禁用缓冲

**验证：**
```rust
// 在 installer.rs 中
cmd.arg("-u");  // ✅ 已添加
cmd.arg(&script_path);
```

#### 问题 4: 进度解析失败

**原因：** 日志格式不匹配

**Python 脚本输出格式：**
```python
self.log(f"下载进度: {progress:.1f}% ({downloaded_size / 1024 / 1024:.2f} MB / {total_size / 1024 / 1024:.2f} MB)")
```

**Rust 解析逻辑：**
```rust
// 查找 '%' 符号
if let Some(pos) = line.find('%') {
    // 向前查找数字
    // 解析百分比
}
```

## 🔧 手动测试后端

如果需要单独测试后端，可以运行：

```bash
cd cheersai-desktop
python scripts/install_ollama.py
```

应该看到详细的进度输出。

## 📊 预期行为

### 完整的安装流程

1. **用户点击"一键安装"**
   ```
   前端: handleConfirmInstallAiModel() 被调用
   前端: setInstalling({ aiModel: true })
   前端: 调用 tauriCommands.installOllamaWithScript()
   ```

2. **后端开始安装**
   ```
   Rust: install_ollama_with_script() 被调用
   Rust: 启动 Python 脚本
   Rust: 读取 stdout 并解析
   ```

3. **Python 脚本输出进度**
   ```
   Python: [INFO] 下载进度: 10.5% ...
   Python: [INFO] 下载进度: 25.3% ...
   Python: [INFO] 下载进度: 50.0% ...
   ```

4. **Rust 解析并发送事件**
   ```
   Rust: parse_installer_log("下载进度: 10.5%")
   Rust: emit("ollama-install-progress", { percentage: 10.5, ... })
   ```

5. **前端接收并更新 UI**
   ```
   前端: listen 回调被触发
   前端: setDownloadProgress({ aiModel: 10.5 })
   前端: setProgressStatus({ aiModel: "下载进度: 10.5%..." })
   前端: UI 重新渲染，显示进度条
   ```

## 🎯 下一步

1. **重新加载应用** (Ctrl+R 或 F5)
2. **打开开发者工具** (F12)
3. **点击"一键安装"**
4. **观察控制台日志**
5. **报告结果**

如果仍然没有进度，请提供：
- 浏览器控制台的完整日志
- 终端（pnpm tauri dev）的完整日志
- 截图

---

**更新时间：** 2026-04-29
**状态：** 等待测试
