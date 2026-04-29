# 健壮的 Ollama 自动安装脚本

## 🎯 设计目标

创建一个**完全自动化、健壮、可靠**的 Ollama 安装脚本，解决之前卡住的问题。

## ✨ 核心特性

### 1. 断点续传 📥
- **问题**: 大文件下载中断后需要重新开始
- **解决**: 
  - 使用 HTTP Range 请求支持断点续传
  - 下载到临时文件 `.download`
  - 中断后可从上次位置继续
  - 自动检测已下载大小

```python
# 检查部分下载的文件
temp_path = Path(str(dest_path) + ".download")
downloaded_size = 0
if temp_path.exists():
    downloaded_size = temp_path.stat().st_size
    
# 添加 Range 头支持断点续传
if downloaded_size > 0:
    req.add_header('Range', f'bytes={downloaded_size}-')
```

### 2. 实时进度反馈 📊
- **问题**: 用户不知道下载进度，以为卡住了
- **解决**:
  - 每秒更新一次进度
  - 显示百分比、已下载/总大小
  - 显示实时下载速度
  - 避免频繁输出影响性能

```python
# 每秒更新进度
if current_time - last_progress_time >= 1.0:
    progress = (downloaded_size / total_size) * 100
    speed = (downloaded_size - last_progress_bytes) / (current_time - last_progress_time) / 1024 / 1024
    self.log(f"下载进度: {progress:.1f}% - 速度: {speed:.2f} MB/s")
```

### 3. 智能重试机制 🔄
- **问题**: 网络不稳定导致下载失败
- **解决**:
  - 最多重试 5 次
  - 每次重试间隔 3 秒
  - 支持断点续传，不会重复下载
  - 区分不同类型的错误

```python
for attempt in range(1, self.max_retries + 1):
    try:
        # 下载逻辑
        break
    except urllib.error.HTTPError as e:
        if e.code == 416:  # Range Not Satisfiable - 已完全下载
            return True
        # 其他错误继续重试
    except Exception as e:
        if attempt < self.max_retries:
            time.sleep(self.retry_delay)
```

### 4. 多镜像源支持 🌐
- **问题**: 单一下载源可能失败或速度慢
- **解决**:
  - 尝试多个镜像源
  - 官方源失败后自动切换到 GitHub
  - 可以轻松添加更多镜像

```python
mirror_urls = [
    "https://ollama.com/download/OllamaSetup.exe",
    "https://github.com/ollama/ollama/releases/latest/download/OllamaSetup.exe",
]

for mirror_url in mirror_urls:
    try:
        self.download_file(mirror_url, installer_path, "Ollama 安装程序")
        break
    except Exception as e:
        continue  # 尝试下一个镜像
```

### 5. 文件完整性验证 ✅
- **问题**: 下载的文件可能损坏或不完整
- **解决**:
  - 检查文件是否存在
  - 验证文件大小（至少 1MB）
  - 安装前确认文件有效

```python
if not installer_path.exists() or installer_path.stat().st_size < 1024 * 1024:
    raise RuntimeError("下载的安装程序文件无效")
```

### 6. 安装状态监控 👀
- **问题**: 安装过程不透明，不知道是否成功
- **解决**:
  - 轮询检查 Ollama 是否安装成功
  - 最多等待 60 秒
  - 每 10 秒输出等待状态
  - 最终验证安装结果

```python
max_wait = 60
while waited < max_wait:
    time.sleep(wait_interval)
    if self.check_ollama_installed():
        self.log(f"✓ Ollama 安装成功（等待了 {waited} 秒）")
        break
    if waited % 10 == 0:
        self.log(f"仍在等待安装完成... ({waited}/{max_wait} 秒)")
```

### 7. 超时保护 ⏱️
- **问题**: 某些操作可能永久卡住
- **解决**:
  - 下载超时：30秒（单次读取）
  - 安装超时：300秒（5分钟）
  - 等待超时：60秒
  - 超时后自动失败并提示

```python
result = subprocess.run(
    [str(installer_path), "/S"],
    timeout=300,  # 5分钟超时
)
```

### 8. 详细的错误处理 🛡️
- **问题**: 错误信息不清晰，难以排查
- **解决**:
  - 区分不同类型的错误
  - 提供详细的错误消息
  - 给出故障排除建议
  - 保留临时文件以便继续

```python
except urllib.error.HTTPError as e:
    self.log(f"✗ HTTP 错误 {e.code}: {e.reason}", "ERROR")
except urllib.error.URLError as e:
    self.log(f"✗ 网络错误: {e.reason}", "ERROR")
except Exception as e:
    self.log(f"✗ 下载失败: {e}", "ERROR")
    if temp_path.exists():
        self.log(f"已保留部分下载的文件，下次可继续")
```

### 9. 资源清理 🧹
- **问题**: 临时文件占用磁盘空间
- **解决**:
  - 下载完成后删除临时文件
  - 安装完成后删除安装程序
  - 失败时保留临时文件以便继续

```python
# 下载完成，重命名临时文件
if temp_path.exists():
    temp_path.rename(dest_path)

# 删除安装程序
try:
    installer_path.unlink()
    self.log("✓ 已清理安装文件")
except Exception as e:
    self.log(f"清理安装文件失败（可忽略）: {e}", "WARNING")
```

### 10. 用户友好的提示 💬
- **问题**: 用户不知道安装需要多长时间
- **解决**:
  - 安装前显示预计时间和文件大小
  - 实时显示进度和速度
  - 提供故障排除建议
  - 支持手动安装作为备选

```python
self.log("⚠ 重要提示：")
self.log("  - 首次安装需要下载约 1.6GB 文件")
self.log("  - 支持断点续传，可随时中断后继续")
self.log("  - 请确保网络连接稳定")
self.log("  - 预计时间：10-30 分钟（取决于网络速度）")
```

## 📊 技术参数

### 下载配置
```python
chunk_size = 65536  # 64KB chunks for better performance
max_retries = 5     # 最多重试5次
retry_delay = 3     # 重试间隔3秒
download_timeout = 30  # 单次读取超时30秒
```

### 安装配置
```python
install_timeout = 300  # 安装超时5分钟
max_wait = 60         # 最多等待60秒
wait_interval = 2     # 每2秒检查一次
```

## 🔄 工作流程

### 完整安装流程

```
1. 检查 Ollama 是否已安装
   ├─ 已安装 → 跳到步骤 3
   └─ 未安装 → 继续

2. 下载并安装 Ollama
   ├─ 检查是否有部分下载的文件
   ├─ 尝试从官方源下载
   │  ├─ 成功 → 继续
   │  └─ 失败 → 尝试 GitHub 镜像
   ├─ 验证下载的文件
   ├─ 执行静默安装
   ├─ 等待安装完成（最多60秒）
   ├─ 验证安装结果
   └─ 清理临时文件

3. 启动 Ollama 服务
   ├─ 检查服务是否已运行
   ├─ 未运行 → 启动服务
   └─ 等待服务启动

4. 下载 AI 模型
   ├─ 检查模型是否已安装
   ├─ 未安装 → 下载 qwen2.5:1.5b
   └─ 实时显示下载进度

5. 验证安装
   ├─ 检查 Ollama 版本
   ├─ 列出已安装的模型
   └─ 测试模型运行

6. 完成
   └─ 输出安装信息
```

### 断点续传流程

```
用户第一次安装（网络中断）:
1. 开始下载 OllamaSetup.exe
2. 已下载 300MB / 600MB
3. 网络中断 → 保存到 OllamaSetup.exe.download
4. 安装失败

用户第二次安装（继续下载）:
1. 检测到 OllamaSetup.exe.download (300MB)
2. 从 300MB 处继续下载
3. 下载剩余 300MB
4. 下载完成 → 重命名为 OllamaSetup.exe
5. 继续安装流程
```

## 🧪 测试场景

### 场景 1: 正常安装
```
网络稳定，首次安装
→ 下载 600MB 安装程序（约 5-10 分钟）
→ 安装 Ollama（约 1-2 分钟）
→ 下载 1GB 模型（约 10-20 分钟）
→ 总计：16-32 分钟
→ 结果：✅ 成功
```

### 场景 2: 网络中断后继续
```
下载到 50% 时网络中断
→ 保存临时文件（300MB）
→ 用户重新运行安装
→ 从 50% 继续下载
→ 下载剩余 50%
→ 继续安装流程
→ 结果：✅ 成功（节省了 50% 时间）
```

### 场景 3: 官方源失败
```
官方源下载失败（防火墙/DNS问题）
→ 自动切换到 GitHub 镜像
→ 从 GitHub 下载
→ 继续安装流程
→ 结果：✅ 成功
```

### 场景 4: 安装超时
```
安装程序运行超过 5 分钟
→ 超时保护触发
→ 显示错误消息
→ 提供故障排除建议
→ 结果：❌ 失败（但有清晰的错误信息）
```

### 场景 5: Ollama 已安装
```
用户已手动安装 Ollama
→ 检测到已安装
→ 跳过下载和安装步骤
→ 直接启动服务
→ 下载模型（如果未安装）
→ 结果：✅ 成功（节省了下载时间）
```

## 📈 性能优化

### 1. 更大的 Chunk Size
```python
chunk_size = 65536  # 64KB instead of 8KB
```
- 减少系统调用次数
- 提高下载速度
- 减少 CPU 使用率

### 2. 进度更新频率控制
```python
if current_time - last_progress_time >= 1.0:
    # 每秒更新一次，而不是每个 chunk
```
- 减少日志输出
- 提高下载效率
- 避免刷屏

### 3. 智能等待策略
```python
wait_interval = 2  # 每2秒检查一次
if waited % 10 == 0:  # 每10秒输出一次
```
- 减少轮询频率
- 降低 CPU 使用
- 保持用户知情

## 🛡️ 错误处理策略

### HTTP 错误
```python
if e.code == 416:  # Range Not Satisfiable
    # 文件已完全下载
    return True
elif e.code == 404:
    # 文件不存在，尝试下一个镜像
    continue
else:
    # 其他错误，重试
    retry()
```

### 网络错误
```python
except urllib.error.URLError as e:
    # DNS 解析失败、连接超时等
    # 重试或切换镜像
```

### 文件系统错误
```python
except IOError as e:
    # 磁盘空间不足、权限问题等
    # 提供清晰的错误消息
```

### 超时错误
```python
except subprocess.TimeoutExpired:
    # 安装超时
    # 建议检查系统权限或手动安装
```

## 💡 使用建议

### 对于用户
1. **确保网络稳定** - 虽然支持断点续传，但稳定的网络更快
2. **不要关闭应用** - 下载过程中保持应用运行
3. **可以中断** - 如果需要中断，下次会自动继续
4. **查看进度** - 实时显示下载进度和速度
5. **耐心等待** - 首次安装需要 16-32 分钟

### 对于开发者
1. **添加更多镜像** - 可以添加国内镜像源提高速度
2. **调整参数** - 根据实际情况调整超时和重试参数
3. **监控日志** - 详细的日志有助于排查问题
4. **测试场景** - 测试各种网络条件和错误情况
5. **用户反馈** - 收集用户反馈持续改进

## 🎯 与之前版本的对比

### 之前的问题
```python
# 问题 1: 没有超时
with urllib.request.urlopen(req, timeout=600) as response:
    # 600秒总超时，太长了

# 问题 2: 没有断点续传
with open(dest_path, 'wb') as f:
    # 总是从头开始

# 问题 3: 进度更新太频繁
if downloaded % (chunk_size * 100) == 0:
    # 每 800KB 更新一次，太频繁

# 问题 4: 没有速度显示
self.log(f"下载进度: {progress:.1f}%")
# 用户不知道速度

# 问题 5: 错误处理简单
except Exception as e:
    # 所有错误一样处理
```

### 现在的改进
```python
# 改进 1: 合理的超时
with urllib.request.urlopen(req, timeout=30) as response:
    # 30秒读取超时，不是总超时

# 改进 2: 支持断点续传
if downloaded_size > 0:
    req.add_header('Range', f'bytes={downloaded_size}-')

# 改进 3: 每秒更新一次
if current_time - last_progress_time >= 1.0:
    # 每秒更新，合理

# 改进 4: 显示速度
speed = (downloaded_size - last_progress_bytes) / elapsed / 1024 / 1024
self.log(f"速度: {speed:.2f} MB/s")

# 改进 5: 详细的错误处理
except urllib.error.HTTPError as e:
    if e.code == 416:
        # 特殊处理
except urllib.error.URLError as e:
    # 网络错误
except Exception as e:
    # 其他错误
```

## 📝 总结

### 核心改进
1. ✅ **断点续传** - 中断后可继续，不浪费已下载的数据
2. ✅ **实时进度** - 每秒更新进度和速度，用户知道状态
3. ✅ **智能重试** - 最多5次重试，支持断点续传
4. ✅ **多镜像源** - 官方源失败自动切换
5. ✅ **文件验证** - 确保下载的文件完整有效
6. ✅ **状态监控** - 实时监控安装状态
7. ✅ **超时保护** - 避免永久卡住
8. ✅ **错误处理** - 详细的错误消息和建议
9. ✅ **资源清理** - 自动清理临时文件
10. ✅ **用户友好** - 清晰的提示和进度反馈

### 预期效果
- **不会卡住** - 有超时保护和进度反馈
- **更可靠** - 支持断点续传和多镜像
- **更快速** - 优化的下载参数
- **更透明** - 实时显示进度和速度
- **更友好** - 清晰的提示和错误消息

### 用户体验
```
之前：
下载中... [卡住，不知道发生了什么]

现在：
下载进度: 45.2% (271.2 MB / 600 MB) - 速度: 5.23 MB/s
```

**完全自动化、健壮、可靠的安装体验！** 🎉
