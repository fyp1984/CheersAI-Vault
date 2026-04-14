# OCR 功能安装指南

CheersAI Vault 支持扫描版 PDF 的 OCR（光学字符识别）功能。

## 方案 1：使用预打包的 OCR（推荐）

如果您下载的是完整版安装包，OCR 功能已经内置，无需额外配置。

## 方案 2：手动安装 Python OCR 环境

如果您使用的是精简版，或 OCR 功能不可用，请按以下步骤安装：

### Windows 系统

1. **安装 Python**
   - 下载 Python 3.7 或更高版本：https://www.python.org/downloads/
   - 安装时务必勾选 "Add Python to PATH"

2. **运行安装脚本**
   ```batch
   install_ocr_windows.bat
   ```
   
   或手动安装依赖：
   ```batch
   pip install easyocr PyMuPDF
   ```

3. **首次使用**
   - 首次处理扫描版 PDF 时，会自动下载 OCR 模型（约 100MB）
   - 请耐心等待，下载完成后会自动开始识别

### 验证安装

在命令行中运行：
```batch
python src-tauri/scripts/pdf_ocr.py test.pdf
```

如果能正常输出文本，说明安装成功。

## 方案 3：打包 OCR 可执行文件（开发者）

如果您是开发者，想要构建包含 OCR 的完整安装包：

1. **安装依赖**
   ```batch
   pip install pyinstaller easyocr PyMuPDF
   ```

2. **构建 OCR 可执行文件**
   ```batch
   build_ocr.bat
   ```
   
   这将在 `dist/pdf_ocr.exe` 生成约 500MB 的独立可执行文件。

3. **构建完整安装包**
   ```batch
   pnpm tauri build
   ```
   
   OCR 可执行文件会自动包含在安装包中。

## 常见问题

### Q: OCR 处理很慢或卡住？
A: 
- 首次使用需要下载模型，请耐心等待
- 大文件（>10MB）可能需要 1-2 分钟
- 如果超过 90 秒仍无响应，会自动超时并显示错误

### Q: 提示 "Python 未安装"？
A:
1. 确认已安装 Python 3.7+
2. 确认安装时勾选了 "Add Python to PATH"
3. 重启应用程序
4. 在命令行运行 `python --version` 验证

### Q: 提示缺少依赖包？
A:
```batch
pip install easyocr PyMuPDF
```

### Q: 不想安装 Python，有其他方案吗？
A:
- 使用在线 OCR 工具：
  - https://www.onlineocr.net/
  - https://ocr.space/
  - 百度 OCR、腾讯 OCR
- 使用桌面 OCR 软件：
  - Adobe Acrobat Pro
  - ABBYY FineReader
  - 福昕 PDF 编辑器

## 技术细节

### OCR 引擎
- 使用 EasyOCR（基于深度学习）
- 支持中文简体和英文
- 首次使用自动下载模型

### 处理流程
1. 尝试直接提取 PDF 文本
2. 如果失败或文本为空，自动启用 OCR
3. 将 PDF 每页转为图片
4. 使用 OCR 识别图片中的文字
5. 合并所有页面的识别结果

### 超时设置
- 默认超时：90 秒
- 可在代码中调整：`run_ocr_command(..., timeout_secs)`

## 支持

如有问题，请联系技术支持或查看项目文档。
