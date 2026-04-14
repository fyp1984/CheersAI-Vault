# 打包 Python 环境完整指南

## 概述

本指南说明如何将 Python 运行时和 OCR 依赖打包到应用中，让用户无需安装 Python 即可使用 OCR 功能。

## 方案对比

| 方案 | 体积 | 用户体验 | 构建复杂度 |
|------|------|----------|-----------|
| 不含 OCR | ~10MB | 需要手动安装 Python | 简单 |
| PyInstaller 打包 | ~500MB | 开箱即用 | 中等 |
| **嵌入式 Python（推荐）** | **~600MB** | **开箱即用** | **中等** |

## 使用嵌入式 Python（推荐）

### 优点
- ✅ 用户无需安装 Python
- ✅ 开箱即用
- ✅ 体积可控（约 600MB）
- ✅ 易于维护和更新
- ✅ 支持所有 Python 包

### 构建步骤

#### 1. 设置嵌入式 Python 环境

```batch
setup_embedded_python.bat
```

这个脚本会：
- 下载 Python 3.11.9 嵌入式版本（25MB）
- 启用 pip
- 安装 OCR 依赖（easyocr, PyMuPDF）
- 总大小约 600MB

**首次运行需要 10-15 分钟**，主要时间用于下载依赖。

#### 2. 创建 OCR 包

```batch
build_ocr_with_python.bat
```

这个脚本会：
- 复制 Python 运行时到 `dist/ocr-package/python/`
- 复制 OCR 脚本到 `dist/ocr-package/`
- 创建启动脚本 `pdf_ocr.bat`
- 测试 OCR 功能

#### 3. 构建完整应用

```batch
build_full_with_python.bat
```

这个脚本会：
- 更新 Tauri 配置，包含 OCR 包
- 构建应用
- 恢复配置

#### 4. 测试安装包

```batch
# 安装生成的 exe
src-tauri\target\release\bundle\nsis\CheersAI Desktop_0.1.2_x64-setup.exe

# 测试 OCR 功能
# 在应用中处理一个扫描版 PDF
```

## 目录结构

构建后的应用结构：

```
CheersAI Desktop/
├── CheersAI Desktop.exe          # 主程序
├── ocr-package/                  # OCR 包
│   ├── python/                   # Python 运行时
│   │   ├── python.exe
│   │   ├── python311.dll
│   │   ├── Lib/
│   │   │   └── site-packages/   # OCR 依赖
│   │   │       ├── easyocr/
│   │   │       ├── fitz/        # PyMuPDF
│   │   │       └── ...
│   │   └── ...
│   ├── pdf_ocr.py               # OCR 脚本
│   └── pdf_ocr.bat              # 启动脚本
└── scripts/                      # 其他脚本
```

## 代码逻辑

应用会按以下顺序查找 OCR 环境：

1. **打包的 Python**（推荐）
   ```
   ocr-package/python/python.exe
   ocr-package/pdf_ocr.py
   ```

2. **打包的 exe**（PyInstaller）
   ```
   pdf_ocr.exe
   ```

3. **系统 Python**（开发环境）
   ```
   python / python3 / py
   scripts/pdf_ocr.py
   ```

## 体积优化

### 当前体积分布

- Python 运行时：~50MB
- OCR 依赖（easyocr）：~400MB
- OCR 依赖（PyMuPDF）：~20MB
- 其他依赖：~130MB
- **总计：~600MB**

### 优化建议

1. **移除不需要的语言模型**
   ```batch
   # 只保留中文和英文模型
   # 删除 python-embed/Lib/site-packages/easyocr/model/
   # 中的其他语言模型
   ```
   可减小约 100MB

2. **使用轻量级 OCR 引擎**
   - 替换为 PaddleOCR（更小更快）
   - 或使用 Tesseract（约 50MB）

3. **按需下载模型**
   - 首次使用时下载模型
   - 减小初始安装包体积

## 常见问题

### Q: 为什么不用 PyInstaller？

A: PyInstaller 的问题：
- 打包后的 exe 约 500MB
- 启动较慢（需要解压）
- 难以调试和更新
- 某些杀毒软件可能误报

嵌入式 Python 的优势：
- 目录结构清晰
- 易于调试和更新
- 可以单独更新 Python 或依赖
- 不会被杀毒软件误报

### Q: 可以减小体积吗？

A: 可以，但会影响功能：

**方案 1：精简版 + 可选下载**
- 初始安装包不含 OCR（10MB）
- 首次使用时提示下载 OCR 包（600MB）
- 最佳用户体验

**方案 2：使用轻量级 OCR**
- 替换为 Tesseract（50MB）
- 识别率可能下降
- 不支持某些复杂场景

**方案 3：云端 OCR**
- 使用云端 API
- 需要网络连接
- 可能有隐私问题

### Q: 首次使用为什么慢？

A: 如果使用嵌入式 Python：
- 首次使用需要下载 OCR 模型（约 100MB）
- 下载完成后会缓存，后续使用很快

如果使用 PyInstaller：
- 不需要下载模型（已打包）
- 但首次启动需要解压（约 10 秒）

### Q: 如何更新 OCR 依赖？

A: 
```batch
# 进入嵌入式 Python 目录
cd python-embed

# 更新依赖
.\python.exe -m pip install --upgrade easyocr PyMuPDF

# 重新构建 OCR 包
cd ..
build_ocr_with_python.bat
```

### Q: 支持其他平台吗？

A: 当前脚本仅支持 Windows。

**macOS/Linux 适配：**
- 下载对应平台的 Python 嵌入式版本
- 修改脚本中的路径和命令
- 原理相同，只需调整细节

## 发布清单

### 精简版（不含 OCR）

- [ ] 构建：`pnpm tauri build`
- [ ] 测试基本功能
- [ ] 测试 OCR 错误提示
- [ ] 包含安装说明文档
- [ ] 体积：~10MB

### 完整版（含 Python OCR）

- [ ] 运行：`setup_embedded_python.bat`
- [ ] 运行：`build_ocr_with_python.bat`
- [ ] 测试 OCR 包：`dist\ocr-package\pdf_ocr.bat test.pdf`
- [ ] 运行：`build_full_with_python.bat`
- [ ] 测试安装包
- [ ] 测试 OCR 功能
- [ ] 验证体积（约 600MB）

## 自动化构建

可以创建 CI/CD 流程：

```yaml
# .github/workflows/build.yml
name: Build with Python OCR

on:
  push:
    tags:
      - 'v*'

jobs:
  build:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v3
      
      - name: Setup Node.js
        uses: actions/setup-node@v3
        with:
          node-version: '18'
      
      - name: Install dependencies
        run: pnpm install
      
      - name: Setup Embedded Python
        run: .\setup_embedded_python.bat
      
      - name: Build OCR Package
        run: .\build_ocr_with_python.bat
      
      - name: Build Application
        run: .\build_full_with_python.bat
      
      - name: Upload Artifacts
        uses: actions/upload-artifact@v3
        with:
          name: installer
          path: src-tauri/target/release/bundle/nsis/*.exe
```

## 性能基准

| 操作 | 时间 | 备注 |
|------|------|------|
| 首次构建 Python 环境 | 10-15 分钟 | 下载依赖 |
| 后续构建 OCR 包 | 1-2 分钟 | 复制文件 |
| 构建应用 | 5-10 分钟 | Tauri 构建 |
| 安装应用 | 2-3 分钟 | 解压文件 |
| 首次 OCR（下载模型） | 2-5 分钟 | 仅首次 |
| 后续 OCR | 10-60 秒 | 取决于文件大小 |

## 技术细节

### Python 嵌入式版本

- 版本：3.11.9
- 架构：x64
- 大小：25MB（压缩）
- 来源：https://www.python.org/downloads/

### 启用 pip

修改 `python311._pth`：
```
python311.zip
.
Lib\site-packages
import site
```

### 依赖安装

```batch
python.exe -m pip install --no-warn-script-location easyocr PyMuPDF
```

### 路径解析

```rust
// 优先使用打包的 Python
let bundled_python = exe_dir.join("ocr-package").join("python").join("python.exe");
let bundled_script = exe_dir.join("ocr-package").join("pdf_ocr.py");

if bundled_python.exists() && bundled_script.exists() {
    run_ocr_command(&bundled_python, &[&bundled_script, path], 90)
}
```

## 参考资料

- [Python Embeddable Package](https://docs.python.org/3/using/windows.html#embedded-distribution)
- [EasyOCR Documentation](https://github.com/JaidedAI/EasyOCR)
- [Tauri Resource Bundling](https://tauri.app/v1/guides/building/resources)

## 更新日志

### v1.0
- ✅ 实现嵌入式 Python 打包
- ✅ 自动化构建脚本
- ✅ 完整的文档和指南
- ✅ 多种回退方案
