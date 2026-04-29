# 前端调用问题 - 需要修复

## 🔴 问题根源

**前端没有调用正确的命令！**

### 当前流程
```typescript
// src/pages/EnhancedServices.tsx - handleConfirmInstallAiModel()

1. 检查 Ollama 是否安装
2. 如果未安装:
   - 调用 tauriCommands.downloadOllama()  ← 这是旧命令！
   - 显示错误消息
   - return (退出函数)
3. 如果已安装:
   - 检查服务是否运行
   - 如果未运行: return (退出函数)
   - 如果运行: 调用 tauriCommands.installOllamaWithScript()
```

### 问题
- `downloadOllama` 是旧的命令，输出旧的错误消息
- 只有当 Ollama 已安装且服务运行时，才会调用 `installOllamaWithScript`
- 但我们的新脚本可以自动安装 Ollama，不需要预先检查

## ✅ 解决方案

### 需要修改的文件
`src/pages/EnhancedServices.tsx`

### 需要修改的函数
`handleConfirmInstallAiModel`

### 修改方案

**简化逻辑，直接调用 `installOllamaWithScript`**:

```typescript
const handleConfirmInstallAiModel = async () => {
  try {
    setShowPathDialog(null);
    setInstalling((prev) => ({ ...prev, aiModel: true }));
    setMessage(null);
    setDownloadProgress((prev) => ({ ...prev, aiModel: 0 }));
    setProgressStatus((prev) => ({ ...prev, aiModel: '' }));

    // 直接使用脚本自动安装 Ollama + AI 模型
    setMessage({ 
      type: 'info', 
      text: '正在安装 Ollama 和 AI 脱敏模型（qwen2.5:1.5b）...\n' +
            '首次安装需要下载约 1.6GB 文件，请耐心等待。\n' +
            '支持断点续传，可随时中断后继续。'
    });

    await tauriCommands.installOllamaWithScript();

    setMessage({ type: 'success', text: 'Ollama 和 AI 模型安装成功！' });
    await checkServicesStatus();
  } catch (error) {
    console.error('Failed to install AI model:', error);
    
    // 如果安装失败，提供手动安装指引
    const errorMsg = String(error);
    let helpText = `安装失败: ${errorMsg}\n\n`;
    
    if (errorMsg.includes('Python') || errorMsg.includes('python')) {
      helpText += '提示：此功能需要 Python 3.7+ 才能使用自动安装。\n\n';
    }
    
    helpText += '您也可以手动安装 Ollama：\n' +
                '1. 访问 https://ollama.com/download（国外官网）\n' +
                '2. 或访问 https://gitee.com/mirrors/ollama（国内镜像）\n' +
                '3. 下载 Windows 版本并安装\n' +
                '4. 安装完成后，在命令行运行：ollama pull qwen2.5:1.5b\n' +
                '5. 重启本应用即可使用';
    
    setMessage({ type: 'error', text: helpText });
  } finally {
    setInstalling((prev) => ({ ...prev, aiModel: false }));
    setDownloadProgress((prev) => ({ ...prev, aiModel: 0 }));
    setProgressStatus((prev) => ({ ...prev, aiModel: '' }));
  }
};
```

### 关键变化
1. ❌ 删除 `checkOllamaInstalled()` 检查
2. ❌ 删除 `downloadOllama()` 调用
3. ❌ 删除 `checkOllamaServiceRunning()` 检查
4. ✅ 直接调用 `installOllamaWithScript()`
5. ✅ 让 Python 脚本处理所有逻辑

### 为什么这样改？
- Python 脚本已经包含了所有检查逻辑
- 脚本会自动检查 Ollama 是否安装
- 脚本会自动下载和安装 Ollama
- 脚本会自动启动服务
- 脚本会自动下载模型
- 不需要前端重复检查

## 📝 具体修改步骤

### 步骤 1: 找到函数
在 `src/pages/EnhancedServices.tsx` 中找到 `handleConfirmInstallAiModel` 函数（约第 335 行）

### 步骤 2: 删除旧逻辑
删除从 `// 先检查 Ollama 是否安装` 到 `await tauriCommands.installOllamaWithScript();` 之前的所有代码

### 步骤 3: 添加新逻辑
```typescript
// 直接使用脚本自动安装 Ollama + AI 模型
setMessage({ 
  type: 'info', 
  text: '正在安装 Ollama 和 AI 脱敏模型（qwen2.5:1.5b）...\n' +
        '首次安装需要下载约 1.6GB 文件，请耐心等待。\n' +
        '支持断点续传，可随时中断后继续。'
});

await tauriCommands.installOllamaWithScript();
```

### 步骤 4: 保存文件
保存后前端会自动热重载

## 🎯 预期效果

### 修改前
```
点击"一键安装"
  ↓
检查 Ollama 是否安装
  ↓
未安装 → 调用 downloadOllama()
  ↓
显示: "Ollama 准备失败: 为避免路径和服务状态不一致..."
  ↓
退出（不继续）
```

### 修改后
```
点击"一键安装"
  ↓
直接调用 installOllamaWithScript()
  ↓
Python 脚本开始执行
  ↓
显示详细的安装日志
  ↓
自动下载和安装 Ollama
  ↓
自动下载模型
  ↓
完成
```

## 🧪 测试

修改后，点击"一键安装"应该看到：

```
后端日志:
=== Installing Ollama using script ===
Running installer script:
  Python: python
  Script: "C:\\Users\\...\\Temp\\cheersai_scripts\\install_ollama.py"
  Args: []
  ✓ Script created at: "..."
[2026-04-29 XX:XX:XX] [INFO] ============================================================
[2026-04-29 XX:XX:XX] [INFO] 开始安装 Ollama + AI 模型
[2026-04-29 XX:XX:XX] [INFO] ============================================================
[2026-04-29 XX:XX:XX] [INFO] ⚠ 重要提示：
[2026-04-29 XX:XX:XX] [INFO]   - 首次安装需要下载约 1.6GB 文件
...
```

## 总结

**问题**: 前端调用了错误的命令 (`downloadOllama` 而不是 `installOllamaWithScript`)  
**原因**: 旧的逻辑检查 Ollama 是否安装，未安装时调用旧命令  
**解决**: 删除检查逻辑，直接调用 `installOllamaWithScript`  
**状态**: 需要手动修改前端代码

---

**请按照上面的步骤修改 `src/pages/EnhancedServices.tsx` 文件！**
