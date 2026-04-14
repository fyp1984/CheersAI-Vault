# OCR 功能打包方案

## 方案概述

将 Python OCR 功能打包成独立的可执行文件，随应用一起分发。

## 方案选择

### 方案 1：PyInstaller（推荐）

**优点：**
- 简单易用
- 支持打包所有依赖
- 生成单个 exe 文件

**缺点：**
- 文件较大（约 500MB，包含 OCR 模型）
- 首次启动较慢

**实施步骤：**

1. 安装 PyInstaller：
```bash
pip install pyinstaller
```

2. 创建打包脚本 `build_ocr.bat`：
```batch
@echo off
echo Building OCR executable...
pyinstaller --onefile --name pdf_ocr --add-data "models;models" src-tauri/scripts/pdf_ocr.py
echo Done! Executable is in dist/pdf_ocr.exe
```

3. 修改 Tauri 配置，将 OCR exe 包含在资源中：
```json
{
  "bundle": {
    "resources": [
      "scripts/*",
      "dist/pdf_ocr.exe"
    ]
  }
}
```

### 方案 2：嵌入式 Python（WinPython）

**优点：**
- 完整的 Python 环境
- 可以运行任何 Python 脚本

**缺点：**
- 体积非常大（约 1GB）
- 需要额外配置

### 方案 3：使用预编译的 OCR 库（Tesseract）

**优点：**
- 体积较小（约 50MB）
- 速度快

**缺点：**
- 需要用 Rust 重写 OCR 逻辑
- 中文识别效果可能不如 EasyOCR

## 推荐实施方案

使用 **PyInstaller** 打包，但采用分离式部署：

1. **核心应用**：不包含 OCR（体积小，约 10MB）
2. **OCR 插件**：可选下载安装（约 500MB）

### 实施步骤

1. 创建 OCR 检测逻辑：
   - 启动时检查 OCR 是否已安装
   - 如果未安装，提示用户下载
   - 提供一键下载安装功能

2. OCR 安装器：
   - 从服务器下载 `pdf_ocr.exe`
   - 解压到应用数据目录
   - 下载 OCR 模型文件

3. 修改代码逻辑：
   - 检查 OCR 是否可用
   - 如果不可用，显示安装提示
   - 安装后自动重试

## 快速实施（开发阶段）

暂时跳过 OCR 打包，在错误信息中提供：
1. Python 安装指南
2. 依赖安装命令
3. 在线 OCR 工具链接

等应用稳定后再实施完整的 OCR 打包方案。
