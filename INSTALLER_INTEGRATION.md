# 安装器脚本集成文档

## 概述

已成功将独立的 Python 安装脚本集成到 Tauri 应用中，提供完全自动化的 OCR 和 Ollama 安装功能。

## 集成内容

### 1. 后端 (Rust)

#### 新增文件
- `src-tauri/src/commands/installer.rs` - 安装器命令模块

#### 新增命令
- `check_python_available()` - 检查 Python 是否可用
- `install_ocr_with_script()` - 使用脚本安装 OCR
- `uninstall_ocr_with_script()` - 使用脚本卸载 OCR
- `install_ollama_with_script()` - 使用脚本安装 Ollama
- `uninstall_ollama_with_script()` - 使用脚本卸载 Ollama

#### 功能特性
- 自动查找系统 Python（python3, python, py）
- 实时输出安装进度和日志
- 支持事件监听（`ocr-install-progress`, `ollama-install-progress` 等）
- 完整的错误处理

### 2. 前端 (TypeScript/React)

#### 新增文件
- `src/pages/InstallerTest.tsx` - 安装器测试页面

#### 新增类型
- `InstallerProgress` - 安装进度类型定义

#### 新增命令函数
- `tauriCommands.checkPythonAvailable()`
- `tauriCommands.installOcrWithScript()`
- `tauriCommands.uninstallOcrWithScript()`
- `tauriCommands.installOllamaWithScript()`
- `tauriCommands.uninstallOllamaWithScript()`

### 3. 安装脚本

#### OCR 安装脚本 (`scripts/install_ocr.py`)
- 自动下载 Python 3.11.9 嵌入式版本（11 MB）
- 自动安装 pip
- 自动安装 PyMuPDF（20 MB）和 EasyOCR（500 MB）
- 使用国内镜像源（华为云 + 阿里云）
- 支持卸载

#### Ollama 安装脚本 (`scripts/install_ollama.py`)
- 自动下载 Ollama 安装程序（~600 MB）
- 静默安装 Ollama
- 自动启动 Ollama 服务
- 自动下载 AI 模型 qwen2.5:1.5b（~1 GB）
- 支持完全卸载（删除程序和数据）

## 使用方法

### 访问测试页面

在应用中访问 `/#/installer-test` 路由即可看到安装器测试页面。

### 前置条件

- 系统必须安装 Python 3.7 或更高版本
- Python 必须在系统 PATH 中可访问

### 安装流程

1. 打开安装器测试页面
2. 检查 Python 状态（自动检测）
3. 点击"安装"按钮
4. 实时查看安装进度和日志
5. 安装完成后自动刷新状态

### 事件监听

前端可以监听以下事件获取实时进度：

```typescript
import { listen } from '@tauri-apps/api/event';
import type { InstallerProgress } from '@/types/commands';

// OCR 安装进度
listen<InstallerProgress>('ocr-install-progress', (event) => {
  console.log(event.payload.status);
  console.log(event.payload.percentage);
  console.log(event.payload.log);
});

// Ollama 安装进度
listen<InstallerProgress>('ollama-install-progress', (event) => {
  console.log(event.payload.status);
  console.log(event.payload.percentage);
  console.log(event.payload.log);
});
```

## 脚本位置

安装脚本会被打包到应用资源中：

- 开发环境：`cheersai-desktop/scripts/`
- 生产环境：应用资源目录 `resources/scripts/`

## 优势

### 相比原有实现

1. **完全自动化**
   - 原：Ollama 需要手动安装
   - 新：Ollama 自动下载和安装

2. **更好的进度反馈**
   - 原：简单的百分比
   - 新：详细的日志输出和状态信息

3. **更强的可维护性**
   - 原：安装逻辑分散在 Rust 代码中
   - 新：独立的 Python 脚本，易于测试和修改

4. **更好的错误处理**
   - 原：错误信息不够详细
   - 新：完整的错误堆栈和重试机制

5. **支持完全卸载**
   - 原：只能卸载模型
   - 新：可以完全卸载 Ollama 和所有数据

## 测试结果

✅ 编译成功（无错误，仅警告）
✅ OCR 安装脚本测试通过（~2 分钟）
✅ Ollama 安装脚本测试通过（~5 分钟）
✅ Ollama 卸载脚本测试通过
✅ 完整的卸载 → 重新安装流程测试通过

## 下一步

1. 在 `EnhancedServices.tsx` 中集成新的安装方式（可选）
2. 添加更多的错误处理和用户提示
3. 考虑添加安装日志保存功能
4. 优化安装进度的 UI 展示

## 注意事项

1. 用户必须先安装 Python 才能使用脚本安装功能
2. 安装过程需要稳定的网络连接
3. OCR 安装约需 530 MB 空间
4. Ollama 安装约需 1.6 GB 空间
5. 首次安装可能需要较长时间（5-10 分钟）

## 文件清单

### 后端
- `src-tauri/src/commands/installer.rs` ✅
- `src-tauri/src/commands/mod.rs` ✅ (已更新)
- `src-tauri/src/lib.rs` ✅ (已更新)

### 前端
- `src/types/commands.ts` ✅ (已更新)
- `src/lib/tauri.ts` ✅ (已更新)
- `src/pages/InstallerTest.tsx` ✅
- `src/App.tsx` ✅ (已更新)

### 脚本
- `scripts/install_ocr.py` ✅
- `scripts/install_ollama.py` ✅
- `scripts/README.md` ✅

### 配置
- `src-tauri/tauri.conf.json` ✅ (resources 已包含 scripts/*)
