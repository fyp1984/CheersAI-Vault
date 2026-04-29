# Ollama 手动安装指南

## 🔴 当前问题

安装脚本在下载 Ollama 时卡住了：
```
=== Starting Ollama download ===
✗ Ollama not found: Ollama 未安装
卡住了
```

**原因**: 
- 下载 Ollama 安装程序（约 600MB）时网络超时
- 或者下载速度太慢导致长时间无响应

## ✅ 推荐解决方案：手动安装

手动安装更可靠、更快速，而且可以看到安装进度。

### 步骤 1: 下载 Ollama

访问官方网站下载：
```
https://ollama.com/download
```

或者直接下载链接：
```
https://ollama.com/download/OllamaSetup.exe
```

**文件大小**: 约 600MB  
**下载时间**: 取决于网络速度（通常 5-15 分钟）

### 步骤 2: 安装 Ollama

1. 双击下载的 `OllamaSetup.exe`
2. 按照安装向导操作
3. 选择安装位置（默认即可）
4. 等待安装完成（约 1-2 分钟）
5. 安装完成后，Ollama 会自动启动

### 步骤 3: 验证安装

打开命令提示符（CMD）或 PowerShell，运行：
```bash
ollama --version
```

应该看到类似输出：
```
ollama version is 0.1.32
```

### 步骤 4: 下载 AI 模型

在命令提示符中运行：
```bash
ollama pull qwen2.5:1.5b
```

**模型大小**: 约 1GB  
**下载时间**: 取决于网络速度（通常 10-30 分钟）

你会看到下载进度：
```
pulling manifest
pulling 183715c43589: 100% ▕████████████████▏ 934 MB
pulling 4fa551d4f938: 100% ▕████████████████▏  11 KB
pulling 8ab4849b038c: 100% ▕████████████████▏  254 B
pulling 577073ffcc6c: 100% ▕████████████████▏  110 B
pulling ad70a5399d7f: 100% ▕████████████████▏  483 B
verifying sha256 digest
writing manifest
removing any unused layers
success
```

### 步骤 5: 测试模型

运行测试命令：
```bash
ollama run qwen2.5:1.5b "你好"
```

应该看到 AI 模型的回复。

### 步骤 6: 回到应用重新扫描

1. 打开 CheersAI Vault 应用
2. 进入 **"增强服务"** 页面
3. 点击 **"重新扫描"** 按钮
4. 应该看到 Ollama 状态变为 ✅ 已安装

---

## 🔧 如果手动安装也失败

### 问题 1: 下载速度太慢

**解决方案**: 使用国内镜像

#### 方法 A: 使用 Gitee 镜像（推荐）

1. 访问 Gitee 镜像：
   ```
   https://gitee.com/mirrors/ollama
   ```

2. 下载 Windows 版本：
   ```
   https://gitee.com/mirrors/ollama/releases
   ```

3. 找到最新版本的 `ollama-windows-amd64.zip`

4. 下载并解压到：
   ```
   C:\Users\你的用户名\AppData\Local\Programs\Ollama
   ```

5. 将该目录添加到系统 PATH 环境变量

#### 方法 B: 使用代理

如果你有代理服务器，可以设置环境变量：
```bash
set HTTP_PROXY=http://your-proxy:port
set HTTPS_PROXY=http://your-proxy:port
ollama pull qwen2.5:1.5b
```

### 问题 2: 安装后无法启动

**检查步骤**:

1. 检查 Ollama 服务是否运行：
   ```bash
   ollama list
   ```

2. 如果提示连接失败，手动启动服务：
   ```bash
   ollama serve
   ```

3. 在新的命令提示符窗口中再次尝试：
   ```bash
   ollama list
   ```

### 问题 3: 模型下载失败

**解决方案**:

1. 检查网络连接
2. 尝试使用较小的模型：
   ```bash
   ollama pull qwen2.5:0.5b
   ```

3. 或者使用其他模型：
   ```bash
   ollama pull llama2:7b
   ```

---

## 📊 安装时间估算

| 步骤 | 大小 | 时间（快速网络） | 时间（慢速网络） |
|------|------|------------------|------------------|
| 下载 Ollama | 600MB | 5-10 分钟 | 20-60 分钟 |
| 安装 Ollama | - | 1-2 分钟 | 1-2 分钟 |
| 下载模型 | 1GB | 10-20 分钟 | 30-120 分钟 |
| **总计** | **1.6GB** | **16-32 分钟** | **51-182 分钟** |

---

## 🎯 为什么推荐手动安装？

### 优点
1. ✅ **更可靠** - 可以看到下载进度，不会卡住
2. ✅ **更快速** - 浏览器下载通常比脚本下载更快
3. ✅ **更灵活** - 可以暂停、恢复下载
4. ✅ **更透明** - 可以看到每一步的进度
5. ✅ **更安全** - 从官方网站下载，更可信

### 缺点
1. ❌ 需要手动操作多个步骤
2. ❌ 需要打开命令提示符

---

## 🚀 快速命令参考

### 检查 Ollama 状态
```bash
ollama --version
ollama list
```

### 下载模型
```bash
ollama pull qwen2.5:1.5b
```

### 测试模型
```bash
ollama run qwen2.5:1.5b "你好"
```

### 启动服务
```bash
ollama serve
```

### 停止服务
```bash
# 按 Ctrl+C 停止 ollama serve
# 或者在任务管理器中结束 ollama.exe 进程
```

### 删除模型
```bash
ollama rm qwen2.5:1.5b
```

### 卸载 Ollama
```
控制面板 → 程序和功能 → 卸载 Ollama
```

---

## 📝 常见问题

### Q: 为什么自动安装会卡住？
A: 因为下载大文件（600MB + 1GB）时，如果网络不稳定或速度慢，脚本可能会超时或无响应。手动安装可以看到进度，更可靠。

### Q: 必须使用 qwen2.5:1.5b 模型吗？
A: 不是。你可以使用任何 Ollama 支持的模型。qwen2.5:1.5b 是推荐的轻量级中文模型，体积小（1GB），速度快。

### Q: 安装后应用还是检测不到 Ollama？
A: 尝试以下步骤：
1. 重启应用
2. 在应用中点击"重新扫描"
3. 检查 Ollama 是否在系统 PATH 中
4. 重启电脑

### Q: 可以使用其他 AI 模型吗？
A: 可以。安装 Ollama 后，你可以下载任何支持的模型：
```bash
ollama pull llama2:7b
ollama pull mistral:7b
ollama pull codellama:7b
```

---

## 🎉 安装完成后

安装完成后，你可以：

1. ✅ 在应用中使用 AI 辅助脱敏
2. ✅ 使用 AI 生成脱敏规则
3. ✅ 使用 AI 分析文档内容
4. ✅ 在命令行中直接使用 Ollama

---

## 📚 相关资源

- Ollama 官网: https://ollama.com
- Ollama GitHub: https://github.com/ollama/ollama
- Ollama 模型库: https://ollama.com/library
- Gitee 镜像: https://gitee.com/mirrors/ollama

---

## 总结

**推荐方案**: 手动安装 Ollama

**步骤**:
1. 访问 https://ollama.com/download
2. 下载并安装 Ollama
3. 运行 `ollama pull qwen2.5:1.5b`
4. 在应用中重新扫描

**预计时间**: 16-32 分钟（取决于网络速度）

**如果遇到问题**: 参考上面的故障排除部分

---

现在请按照上面的步骤手动安装 Ollama！安装完成后告诉我，我们可以继续其他工作。
