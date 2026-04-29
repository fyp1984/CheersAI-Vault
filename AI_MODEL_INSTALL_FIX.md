# AI 模型安装流程改进

## 问题描述

之前的实现中，点击"安装 AI 模型"按钮时，会先检查 Ollama 是否已安装。如果未安装，会显示错误提示，要求用户手动安装 Ollama。这给用户带来了额外的操作负担。

**旧的错误提示：**
```
请先安装 Ollama 服务！

国内用户推荐：
1. 访问 https://ollama.com/download（国外官网）
2. 或访问 https://gitee.com/mirrors/ollama（国内镜像）
3. 下载 Windows 版本并安装
4. 安装完成后重启本应用
5. 再次点击"安装 AI 模型"按钮

注意：Ollama 是 AI 模型运行的必需服务
```

## 解决方案

修改 `handleConfirmInstallAiModel()` 函数，直接使用 `installOllamaWithScript()` 自动安装 Ollama + AI 模型，无需用户手动操作。

### 改进后的流程

1. **用户点击"安装 AI 模型"**
2. **自动安装 Ollama + 模型**（使用 Python 脚本）
   - 下载 Ollama 安装程序（约 600MB）
   - 静默安装 Ollama
   - 启动 Ollama 服务
   - 下载 qwen2.5:1.5b 模型（约 1GB）
3. **显示实时进度**
   - 百分比进度条
   - 当前操作状态
4. **安装完成**

### 错误处理

如果自动安装失败（如缺少 Python），会显示友好的错误提示和手动安装指引：

```
安装失败: [错误信息]

提示：此功能需要 Python 3.7+ 才能使用自动安装。

您也可以手动安装 Ollama：
1. 访问 https://ollama.com/download（国外官网）
2. 或访问 https://gitee.com/mirrors/ollama（国内镜像）
3. 下载 Windows 版本并安装
4. 安装完成后，在命令行运行：ollama pull qwen2.5:1.5b
5. 重启本应用即可使用
```

## 代码修改

### 修改前（`src/pages/EnhancedServices.tsx`）

```typescript
const handleConfirmInstallAiModel = async () => {
  try {
    setShowPathDialog(null);
    setInstalling({ ...installing, aiModel: true });
    setMessage(null);
    setDownloadProgress({ ...downloadProgress, aiModel: 0 });

    // 先检查 Ollama 是否安装
    const ollamaInstalled = await tauriCommands.checkOllamaInstalled();
    if (!ollamaInstalled) {
      // 显示错误，要求手动安装
      setInstalling({ ...installing, aiModel: false });
      setMessage({ 
        type: 'error', 
        text: '请先安装 Ollama 服务！...'
      });
      return;
    }
    
    // 只安装模型
    const result = await tauriCommands.installAiModel();
    // ...
  }
};
```

### 修改后

```typescript
const handleConfirmInstallAiModel = async () => {
  try {
    setShowPathDialog(null);
    setInstalling({ ...installing, aiModel: true });
    setMessage(null);
    setDownloadProgress({ ...downloadProgress, aiModel: 0 });
    setProgressStatus({ ...progressStatus, aiModel: '' });

    // 使用脚本自动安装 Ollama + AI 模型
    setMessage({ 
      type: 'info', 
      text: '正在安装 Ollama 和 AI 脱敏模型（qwen2.5:1.5b）...\n' +
            '首次安装需要下载约 1.6GB 文件，请耐心等待。'
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
    setInstalling({ ...installing, aiModel: false });
    setDownloadProgress({ ...downloadProgress, aiModel: 0 });
    setProgressStatus({ ...progressStatus, aiModel: '' });
  }
};
```

## 优势

### 改进前
❌ 需要用户手动下载和安装 Ollama  
❌ 需要用户手动运行命令下载模型  
❌ 需要用户重启应用  
❌ 操作步骤多，容易出错  

### 改进后
✅ 一键自动安装 Ollama + 模型  
✅ 实时显示安装进度  
✅ 无需手动操作  
✅ 自动处理所有步骤  
✅ 失败时提供清晰的手动安装指引  

## 用户体验

### 正常流程
1. 用户点击"安装 AI 模型"
2. 看到提示："正在安装 Ollama 和 AI 脱敏模型..."
3. 看到实时进度：
   - "下载 Ollama 安装程序... 25.5%"
   - "安装 Ollama... 40.0%"
   - "启动 Ollama 服务... 50.0%"
   - "下载模型文件 18371c43: 75.5%"
   - "正在验证模型文件... 95.0%"
4. 看到成功提示："Ollama 和 AI 模型安装成功！"

### 失败流程（缺少 Python）
1. 用户点击"安装 AI 模型"
2. 看到错误提示：
   ```
   安装失败: Python 未安装
   
   提示：此功能需要 Python 3.7+ 才能使用自动安装。
   
   您也可以手动安装 Ollama：
   1. 访问 https://ollama.com/download
   2. ...
   ```

## 技术细节

### 安装脚本
使用 `scripts/install_ollama.py`，包含：
- 下载 Ollama 安装程序
- 静默安装（`/S` 参数）
- 启动服务
- 生成 SSH 密钥
- 下载模型
- 验证安装

### 进度显示
- 实时解析脚本输出
- 提取百分比和状态
- 更新前端进度条
- 显示友好的中文提示

### 错误处理
- 捕获所有异常
- 识别常见错误（如缺少 Python）
- 提供针对性的解决方案
- 保留手动安装选项

## 测试

### 测试场景 1：正常安装
1. 确保 Python 已安装
2. 确保 Ollama 未安装
3. 点击"安装 AI 模型"
4. 验证自动安装成功

### 测试场景 2：Python 未安装
1. 确保 Python 未安装
2. 点击"安装 AI 模型"
3. 验证显示友好的错误提示
4. 验证提供手动安装指引

### 测试场景 3：Ollama 已安装
1. 确保 Ollama 已安装
2. 点击"安装 AI 模型"
3. 验证跳过 Ollama 安装
4. 验证只下载模型

## 文件修改

- ✅ `src/pages/EnhancedServices.tsx` - 修改安装逻辑
- ✅ 编译成功
- ✅ 功能测试通过

## 总结

通过这次改进，我们将 AI 模型的安装流程从**5步手动操作**简化为**1键自动安装**，大大提升了用户体验。同时保留了手动安装选项，确保在自动安装失败时用户仍有解决方案。

**改进效果：**
- 🚀 安装步骤：5步 → 1步
- 📊 进度可见：无 → 实时百分比
- 🎯 用户体验：复杂 → 简单
- 🛡️ 错误处理：基础 → 完善
