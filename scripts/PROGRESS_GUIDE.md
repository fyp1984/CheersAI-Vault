# 安装进度显示 - 快速指南

## 功能概述

安装 OCR 和 Ollama 时，用户可以看到：
- ✅ **百分比进度**: 精确到小数点后一位（如 50.5%）
- ✅ **状态文本**: 当前操作描述（如"下载模型文件"）
- ✅ **进度条**: 可视化进度条，平滑动画
- ✅ **实时更新**: 进度实时刷新，无需刷新页面

## 用户体验

### OCR 安装进度示例
```
下载 Python 3.11.9...                    25.5%
████████░░░░░░░░░░░░░░░░░░░░░░░░░░░░

安装 pip...                              40.0%
████████████░░░░░░░░░░░░░░░░░░░░░░░░

安装 PyMuPDF...                          60.0%
████████████████████░░░░░░░░░░░░░░░░

安装 EasyOCR...                          85.5%
████████████████████████████░░░░░░░░

✓ OCR 环境安装完成！                     100.0%
████████████████████████████████████
```

### Ollama 模型下载进度示例
```
正在获取模型清单...                       5.0%
██░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░

下载模型文件 18371c43: 50.5%            50.5%
████████████████████░░░░░░░░░░░░░░░░

正在验证模型文件...                      95.0%
██████████████████████████████████░░

模型下载完成                            100.0%
████████████████████████████████████
```

## 技术实现

### 进度数据流
```
Python 脚本输出
    ↓
Rust 解析器 (parse_installer_log)
    ↓
Tauri 事件 (InstallerProgress)
    ↓
React 组件 (listen)
    ↓
UI 更新 (进度条 + 文本)
```

### 支持的进度格式

1. **标准日志格式**:
   ```
   [2024-01-01 12:00:00] [INFO] 下载进度: 50.0% (5.60 MB / 11.20 MB)
   ```

2. **Ollama 输出格式**:
   ```
   pulling 183715c43589: 50% ▕████████▏ 500 MB
   ```

3. **简单格式**:
   ```
   downloading: 75.5%
   ```

### Ollama 状态识别

| 原始输出 | 显示文本 | 进度 |
|---------|---------|------|
| `pulling manifest` | 正在获取模型清单... | 5% |
| `pulling [hash]: X%` | 下载模型文件 [hash前8位]: X% | X% |
| `verifying` | 正在验证模型文件... | 95% |
| `writing manifest` | 正在写入模型清单... | 98% |
| `success` | 模型下载完成 | 100% |

## 代码示例

### Rust 进度解析
```rust
fn parse_installer_log(line: &str) -> InstallerProgress {
    let mut percentage = 0.0;
    let mut status = line.to_string();
    
    // 提取百分比
    if let Some(pos) = line.find('%') {
        let mut start = pos;
        while start > 0 {
            let ch = line.chars().nth(start - 1).unwrap_or(' ');
            if ch.is_numeric() || ch == '.' {
                start -= 1;
            } else {
                break;
            }
        }
        if let Ok(pct) = line[start..pos].trim().parse::<f64>() {
            percentage = pct;
        }
    }
    
    // 识别 Ollama 状态
    if status.contains("pulling manifest") {
        status = "正在获取模型清单...".to_string();
        percentage = 5.0;
    }
    
    InstallerProgress { percentage, status, log: line.to_string() }
}
```

### Python 实时输出
```python
def install_model(self):
    process = subprocess.Popen(
        ["ollama", "pull", self.model_name],
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
        bufsize=1,
        universal_newlines=True
    )
    
    import re
    last_percentage = 0.0
    
    for line in iter(process.stdout.readline, ''):
        print(line, flush=True)  # 输出原始行
        
        # 解析进度
        match = re.search(r'(\d+)%', line)
        if match:
            percentage = float(match.group(1))
            if percentage != last_percentage:
                self.log(f"下载进度: {percentage:.1f}%")
                last_percentage = percentage
```

### React 进度显示
```tsx
// 监听进度事件
useEffect(() => {
  listen<InstallerProgress>('ollama-install-progress', (event) => {
    setDownloadProgress(prev => ({ 
      ...prev, 
      aiModel: event.payload.percentage 
    }));
    setProgressStatus(prev => ({ 
      ...prev, 
      aiModel: event.payload.status 
    }));
  });
}, []);

// 显示进度条
{installing.aiModel && downloadProgress.aiModel > 0 && (
  <div className="mt-4">
    <div className="flex items-center justify-between text-sm text-gray-600 mb-1">
      <span className="truncate mr-2">
        {progressStatus.aiModel || '下载进度'}
      </span>
      <span className="font-medium">
        {downloadProgress.aiModel.toFixed(1)}%
      </span>
    </div>
    <div className="w-full bg-gray-200 rounded-full h-2">
      <div
        className="bg-purple-600 h-2 rounded-full transition-all duration-300"
        style={{ width: `${downloadProgress.aiModel}%` }}
      />
    </div>
  </div>
)}
```

## 测试步骤

### 1. 启动开发环境
```bash
cd cheersai-desktop
pnpm run tauri dev
```

### 2. 测试 OCR 安装
1. 打开应用
2. 导航到 "增强服务" 页面
3. 点击 "安装 OCR 服务"
4. 观察进度条和百分比更新

### 3. 测试 Ollama 安装
1. 点击 "安装 AI 脱敏模型"
2. 观察模型下载进度（约 1GB）
3. 验证不同阶段的状态提示

### 4. 查看详细日志
- 访问 `/#/installer-test` 页面
- 查看完整的安装日志
- 验证进度事件正确触发

## 故障排查

### 问题：进度不更新
**原因**: 事件监听器未设置
**解决**: 确保 `setupProgressListeners()` 在 `useEffect` 中调用

### 问题：百分比显示为 0
**原因**: 日志格式不匹配
**解决**: 检查 Python 脚本输出格式，确保包含 `%` 符号

### 问题：状态文本为空
**原因**: 状态解析失败
**解决**: 检查 Rust `parse_installer_log()` 函数的状态识别逻辑

### 问题：Ollama 下载失败
**原因**: 缺少 SSH 密钥
**解决**: 
1. 安装 Git for Windows
2. 脚本会自动生成 SSH 密钥
3. 或手动运行: `ssh-keygen -t ed25519 -f %USERPROFILE%\.ollama\id_ed25519 -N ""`

## 性能优化

1. **避免频繁更新**: 仅在百分比变化时更新
2. **行缓冲**: `bufsize=1` 实现实时输出
3. **平滑动画**: CSS `transition-all duration-300`
4. **文本截断**: `truncate` 避免溢出

## 文件位置

| 文件 | 路径 | 说明 |
|------|------|------|
| Rust 解析器 | `src-tauri/src/commands/installer.rs` | 进度解析逻辑 |
| Python 脚本 | `scripts/install_ollama.py` | 实时输出 |
| React 组件 | `src/pages/EnhancedServices.tsx` | 进度显示 |
| 测试页面 | `src/pages/InstallerTest.tsx` | 详细日志 |

## 相关文档

- `PROGRESS_ENHANCEMENT.md` - 详细技术文档
- `PROGRESS_SUMMARY.md` - 完成总结
- `README.md` - 项目主文档

## 总结

✅ 实时百分比进度显示
✅ 友好的状态文本提示
✅ 平滑的进度条动画
✅ 支持多种日志格式
✅ 完整的错误处理

用户现在可以清楚地看到安装进度，不再需要等待黑盒操作完成！
