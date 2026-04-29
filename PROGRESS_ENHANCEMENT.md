# 安装进度显示增强

## 更新内容

### 1. Rust 后端 - 增强进度解析 (`src-tauri/src/commands/installer.rs`)

**改进的 `parse_installer_log()` 函数：**

- **支持多种进度格式：**
  - 标准日志格式：`[2024-01-01 12:00:00] [INFO] 下载进度: 50.0% (5.60 MB / 11.20 MB)`
  - Ollama 输出格式：`pulling 183715c43589: 50% ▕████████▏ 500 MB`
  - 简单格式：`downloading: 75.5%`

- **智能百分比提取：**
  - 向前查找数字开始位置，支持小数点
  - 正确解析各种格式的百分比

- **Ollama 特定状态识别：**
  - `pulling manifest` → "正在获取模型清单..." (5%)
  - `pulling [hash]: X%` → "下载模型文件 [hash前8位]: X%"
  - `verifying` → "正在验证模型文件..." (95%)
  - `writing manifest` → "正在写入模型清单..." (98%)
  - `success` → "模型下载完成" (100%)

- **ANSI 转义序列清理：**
  - 移除 `\x1b[?2026h`, `\x1b[?25l`, `\x1b[?25h` 等终端控制字符
  - 清理 `[K`, `[1G` 等光标控制序列

### 2. Python 脚本 - 实时进度输出 (`scripts/install_ollama.py`)

**改进的 `install_model()` 函数：**

- **使用 `subprocess.Popen` 实时读取输出：**
  ```python
  process = subprocess.Popen(
      ["ollama", "pull", self.model_name],
      stdout=subprocess.PIPE,
      stderr=subprocess.STDOUT,
      text=True,
      bufsize=1,
      universal_newlines=True
  )
  ```

- **正则表达式解析进度：**
  ```python
  match = re.search(r'(\d+)%', line)
  if match:
      percentage = float(match.group(1))
      self.log(f"下载进度: {percentage:.1f}%")
  ```

- **识别关键状态：**
  - `pulling manifest` → 输出 "正在获取模型清单..."
  - `verifying` → 输出 "正在验证模型文件..."
  - `writing manifest` → 输出 "正在写入模型清单..."
  - `success` → 输出 "模型下载完成"

### 3. 前端 - 进度显示增强

#### EnhancedServices.tsx

**新增状态：**
```typescript
const [progressStatus, setProgressStatus] = useState({
  ocr: '',
  aiModel: '',
});
```

**事件监听器：**
```typescript
const setupProgressListeners = () => {
  listen<InstallerProgress>('ocr-install-progress', (event) => {
    setDownloadProgress(prev => ({ ...prev, ocr: event.payload.percentage }));
    setProgressStatus(prev => ({ ...prev, ocr: event.payload.status }));
  });
  
  listen<InstallerProgress>('ollama-install-progress', (event) => {
    setDownloadProgress(prev => ({ ...prev, aiModel: event.payload.percentage }));
    setProgressStatus(prev => ({ ...prev, aiModel: event.payload.status }));
  });
  // ... 卸载进度监听器
};
```

**进度条显示：**
```tsx
{installing.ocr && downloadProgress.ocr > 0 && (
  <div className="mt-4">
    <div className="flex items-center justify-between text-sm text-gray-600 mb-1">
      <span className="truncate mr-2">{progressStatus.ocr || '下载进度'}</span>
      <span className="font-medium">{downloadProgress.ocr.toFixed(1)}%</span>
    </div>
    <div className="w-full bg-gray-200 rounded-full h-2">
      <div
        className="bg-blue-600 h-2 rounded-full transition-all duration-300"
        style={{ width: `${downloadProgress.ocr}%` }}
      />
    </div>
  </div>
)}
```

## 效果

### OCR 安装进度示例：
```
正在下载 Python 3.11.9...
下载进度: 25.5% (2.85 MB / 11.20 MB)
下载进度: 50.0% (5.60 MB / 11.20 MB)
下载进度: 75.5% (8.45 MB / 11.20 MB)
下载进度: 100.0% (11.20 MB / 11.20 MB)
✓ Python 3.11.9 下载完成
```

### Ollama 模型下载进度示例：
```
正在获取模型清单... (5%)
下载模型文件 18371c43: 10.0%
下载模型文件 18371c43: 25.5%
下载模型文件 18371c43: 50.0%
下载模型文件 18371c43: 75.5%
下载模型文件 18371c43: 100.0%
正在验证模型文件... (95%)
正在写入模型清单... (98%)
模型下载完成 (100%)
```

## 测试

1. **启动开发环境：**
   ```bash
   pnpm run tauri dev
   ```

2. **测试 OCR 安装：**
   - 访问 "增强服务" 页面
   - 点击 "安装 OCR 服务"
   - 观察进度条和状态文本实时更新

3. **测试 Ollama 安装：**
   - 点击 "安装 AI 脱敏模型"
   - 观察模型下载进度（约 1GB）
   - 查看不同阶段的状态提示

4. **测试页面：**
   - 访问 `/#/installer-test`
   - 查看详细的日志输出和进度显示

## 技术细节

### 进度计算逻辑

1. **OCR 安装：**
   - Python 下载：0-30%
   - pip 安装：30-40%
   - PyMuPDF 安装：40-60%
   - EasyOCR 安装：60-100%

2. **Ollama 安装：**
   - Ollama 下载：0-20%
   - Ollama 安装：20-30%
   - 服务启动：30-35%
   - 模型下载：35-100%（根据实际下载进度）

### 性能优化

- 使用 `bufsize=1` 和 `universal_newlines=True` 实现行缓冲
- 避免频繁更新进度（仅在百分比变化时更新）
- 前端使用 `transition-all duration-300` 平滑动画
- 状态文本使用 `truncate` 避免溢出

## 文件修改清单

- ✅ `src-tauri/src/commands/installer.rs` - 增强进度解析
- ✅ `scripts/install_ollama.py` - 实时进度输出
- ✅ `src/pages/EnhancedServices.tsx` - 进度显示增强
- ✅ `src/pages/InstallerTest.tsx` - 已有完整进度显示

## 模型存储位置

Ollama 模型下载到：
- **Windows**: `%USERPROFILE%\.ollama\models\`
- **结构**:
  ```
  .ollama/
  ├── models/
  │   ├── manifests/
  │   │   └── registry.ollama.ai/library/qwen2.5/1.5b
  │   └── blobs/
  │       ├── sha256-183715c43589...
  │       ├── sha256-a3e4b2c1d5f6...
  │       └── ...
  └── id_ed25519 (SSH 密钥)
  ```
- **大小**: qwen2.5:1.5b 约 986 MB

## 已知问题

1. **SSH 密钥要求：**
   - Ollama 需要 SSH 密钥才能下载模型
   - 脚本会自动生成（需要 Git for Windows）
   - 如果生成失败，会显示警告但继续执行

2. **进度更新频率：**
   - Ollama 的进度输出可能不是线性的
   - 某些阶段可能停留较长时间（如验证）

3. **ANSI 转义序列：**
   - Windows 终端可能输出额外的控制字符
   - 已在 Rust 端清理，但日志中仍可见原始输出
