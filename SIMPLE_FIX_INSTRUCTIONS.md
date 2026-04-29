# 简单修复说明

## 🎯 问题

前端调用了错误的命令，导致看到旧的错误消息而不是新的安装脚本。

## ✅ 解决方案（3步）

### 步骤 1: 打开文件
打开文件: `src/pages/EnhancedServices.tsx`

### 步骤 2: 找到并删除代码
找到第 342 行附近的这段代码（从 `// 先检查 Ollama 是否安装` 开始）：

```typescript
// 先检查 Ollama 是否安装（包括系统安装和内置版本）
const ollamaInstalled = await tauriCommands.checkOllamaInstalled();
if (!ollamaInstalled) {
  setMessage({ 
    type: 'info', 
    text: isMac
      ? '未检测到 Ollama。请先安装并启动 Ollama.app，然后重新扫描服务状态。'
      : '未检测到 Ollama。请先完成安装后重新扫描服务状态。'
  });
  
  try {
    // 传递自定义路径（如果有）
    await tauriCommands.downloadOllama(customPaths.aiModel || undefined);
    setMessage({ 
      type: 'success', 
      text: isMac ? '请先完成 Ollama 安装或启动，再继续安装 AI 模型。' : 'Ollama 安装信息已准备完成，请先启动服务后再安装 AI 模型。'
    });
  } catch (error) {
    setMessage({ 
      type: 'info',
      text: `Ollama 准备失败: ${error}` 
    });
    return;
  }
} else {
  const ollamaRunning = await tauriCommands.checkOllamaServiceRunning();
  if (!ollamaRunning) {
    setMessage({
      type: 'info',
      text: isMac
        ? '检测到 Ollama 已安装但服务未启动。请先点击"启动 Ollama"或打开 Ollama.app，然后重新扫描。'
        : '检测到 Ollama 已安装但服务未启动。请先启动服务并重新扫描。'
    });
    return;
  }

  setMessage({ type: 'info', text: '检测到 Ollama 服务已运行，正在安装 AI 脱敏模型（qwen2.5:1.5b）...' });
}
setDownloadProgress({ ...downloadProgress, aiModel: 0 });
setProgressStatus({ ...progressStatus, aiModel: '' });

// 使用脚本自动安装 Ollama + AI 模型
```

**删除上面所有代码！**

### 步骤 3: 替换为新代码
在删除的位置，添加这段简单的代码：

```typescript
setProgressStatus((prev) => ({ ...prev, aiModel: '' }));

// 直接使用脚本自动安装 Ollama + AI 模型
```

### 完整的修改后代码
修改后，`handleConfirmInstallAiModel` 函数应该是这样的：

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

## 🎉 完成

保存文件后，前端会自动热重载。然后点击"一键安装"就应该能看到详细的安装日志了！

---

**关键点**:
- 删除所有检查 Ollama 的代码
- 删除调用 `downloadOllama` 的代码
- 直接调用 `installOllamaWithScript()`
- 让 Python 脚本处理所有逻辑
