# OCR 按需下载功能指南

## 概述

为了减小安装包体积，OCR 功能采用按需下载的方式。用户首次遇到扫描版 PDF 时，应用会提示下载 OCR 依赖包。

## 架构设计

### 安装包结构

**精简版（默认）**
- 大小：~10MB
- 包含：核心应用功能
- 不包含：OCR 依赖

**OCR 包（按需下载）**
- 大小：~100MB
- 包含：Python 运行时 + EasyOCR + PyMuPDF
- 存储位置：用户应用数据目录

### 工作流程

```
用户处理 PDF
    ↓
尝试文本提取
    ↓
提取失败（扫描版）
    ↓
检查 OCR 是否已安装
    ↓
未安装 → 显示下载对话框
    ↓
用户点击下载
    ↓
从云端下载 OCR 包
    ↓
自动解压到应用数据目录
    ↓
重新处理 PDF（使用 OCR）
```

## 技术实现

### 后端（Rust）

#### 1. OCR 检测命令

```rust
#[tauri::command]
pub async fn check_ocr_installed(app: AppHandle) -> Result<bool, String>
```

检查 OCR 是否已安装在应用数据目录。

#### 2. OCR 下载命令

```rust
#[tauri::command]
pub async fn download_ocr_package(
    app: AppHandle,
    window: tauri::Window,
) -> Result<String, String>
```

功能：
- 从云端下载 OCR 包（ZIP 格式）
- 实时发送下载进度事件
- 自动解压到应用数据目录
- 清理临时文件

进度事件：
```rust
pub struct OcrDownloadProgress {
    pub downloaded: u64,
    pub total: u64,
    pub percentage: f64,
    pub status: String,
}
```

#### 3. OCR 路径查找

文件：`src-tauri/src/core/file_parser.rs`

查找顺序：
1. 应用数据目录（用户下载的）
2. 应用安装目录（预打包的）
3. 开发环境脚本
4. 系统 Python

```rust
// 方案 1: 用户下载的 OCR 包
let downloaded_python = app_data_dir.join("ocr-package").join("python").join("python.exe");
let downloaded_script = app_data_dir.join("ocr-package").join("pdf_ocr.py");

if downloaded_python.exists() && downloaded_script.exists() {
    return run_ocr_command(&downloaded_python, &[&downloaded_script, path], 300);
}
```

### 前端（React + TypeScript）

#### 1. OCR 下载对话框

文件：`src/components/file/OcrDownloadDialog.tsx`

功能：
- 显示下载说明
- 实时显示下载进度
- 处理下载错误
- 支持重试

使用：
```tsx
<OcrDownloadDialog
  open={showOcrDownload}
  onOpenChange={setShowOcrDownload}
  onComplete={() => {
    console.log('OCR installed successfully');
  }}
/>
```

#### 2. 错误检测

文件：`src/pages/FileProcess.tsx`

```typescript
const isOcrError = (error: unknown): boolean => {
  const errorStr = String(error).toLowerCase();
  return errorStr.includes('ocr') || 
         errorStr.includes('扫描版') || 
         errorStr.includes('python') ||
         errorStr.includes('easyocr');
};
```

当检测到 OCR 错误时，自动显示下载对话框。

#### 3. 进度监听

```typescript
useEffect(() => {
  const unlisten = await listen<OcrDownloadProgress>('ocr-download-progress', (event) => {
    setProgress(event.payload);
    if (event.payload.percentage >= 100) {
      setIsComplete(true);
    }
  });

  return () => unlisten();
}, [open]);
```

## 云端部署

### OCR 包准备

1. 创建 OCR 包目录结构：
```
ocr-package/
├── python/
│   ├── python.exe
│   ├── python311.dll
│   ├── Lib/
│   │   └── site-packages/
│   │       ├── easyocr/
│   │       ├── fitz/
│   │       └── ...
│   └── ...
└── pdf_ocr.py
```

2. 压缩为 ZIP：
```bash
# 使用 7-Zip 或其他工具
7z a -tzip ocr-package.zip ocr-package/
```

3. 上传到云存储：
- GitHub Releases
- 阿里云 OSS
- 腾讯云 COS
- AWS S3

### 配置下载 URL

#### 方法 1：环境变量（推荐）

```bash
# Windows
set OCR_DOWNLOAD_URL=https://github.com/your-org/ocr-package/releases/download/v1.0/ocr-package.zip

# Linux/Mac
export OCR_DOWNLOAD_URL=https://github.com/your-org/ocr-package/releases/download/v1.0/ocr-package.zip
```

#### 方法 2：修改代码

文件：`src-tauri/src/commands/ocr.rs`

```rust
let download_url = std::env::var("OCR_DOWNLOAD_URL")
    .unwrap_or_else(|_| "https://your-actual-url.com/ocr-package.zip".to_string());
```

### GitHub Releases 示例

1. 创建 Release：
```bash
gh release create v1.0 \
  --title "OCR Package v1.0" \
  --notes "OCR dependencies for CheersAI Desktop"
```

2. 上传 OCR 包：
```bash
gh release upload v1.0 ocr-package.zip
```

3. 获取下载 URL：
```
https://github.com/your-org/cheersai-desktop/releases/download/v1.0/ocr-package.zip
```

## 用户体验

### 首次使用流程

1. 用户安装应用（~10MB）
2. 用户拖入扫描版 PDF
3. 应用尝试文本提取，失败
4. 弹出对话框：
   ```
   检测到扫描版 PDF，需要下载 OCR 依赖包才能识别文字。
   
   下载说明：
   • 大小约 100MB，首次下载需要 2-5 分钟
   • 下载后自动安装，无需重启应用
   • 仅需下载一次，后续可直接使用
   
   [取消] [开始下载]
   ```
5. 用户点击"开始下载"
6. 显示实时进度：
   ```
   正在下载... 45.2%
   已下载 45.2 MB / 100.0 MB
   ```
7. 下载完成，自动解压
8. 显示成功消息：
   ```
   OCR 功能已就绪，可以开始处理扫描版 PDF 文件了。
   ```
9. 用户点击"完成"，可以重新处理文件

### 后续使用

- OCR 已安装，直接使用
- 无需再次下载
- 处理速度：10-60 秒/文件（取决于页数）

## 故障排查

### 下载失败

**问题：** 网络连接失败

**解决：**
1. 检查网络连接
2. 尝试使用 VPN
3. 点击"重试"按钮
4. 手动下载并放置到应用数据目录

**手动安装：**
```
1. 下载 OCR 包：https://your-url.com/ocr-package.zip
2. 解压到：%APPDATA%\com.cheersai.vault\ocr-package\
3. 重启应用
```

### OCR 识别失败

**问题：** OCR 超时或识别错误

**解决：**
1. 检查 PDF 文件是否损坏
2. 尝试减小 PDF 文件大小
3. 使用在线 OCR 工具作为备选

### 磁盘空间不足

**问题：** 下载或解压失败

**解决：**
1. 清理磁盘空间（至少 200MB）
2. 重新下载

## 性能优化

### 下载优化

1. **使用 CDN**
   - 加速全球访问
   - 减少下载时间

2. **分片下载**
   - 支持断点续传
   - 提高下载成功率

3. **压缩优化**
   - 使用 7z 格式（更小）
   - 移除不需要的语言模型

### 存储优化

1. **按需下载模型**
   - 首次只下载中文和英文模型
   - 其他语言按需下载

2. **共享依赖**
   - 多个应用共享 OCR 包
   - 减少重复下载

## 未来改进

### 短期（v1.1）

- [ ] 支持断点续传
- [ ] 添加下载速度显示
- [ ] 支持镜像源切换
- [ ] 添加下载失败重试机制

### 中期（v1.2）

- [ ] 支持增量更新
- [ ] 添加 OCR 模型管理
- [ ] 支持自定义下载源
- [ ] 添加离线安装包

### 长期（v2.0）

- [ ] 云端 OCR 服务
- [ ] 本地 + 云端混合模式
- [ ] 支持更多 OCR 引擎
- [ ] AI 增强识别

## 相关文件

### 后端
- `src-tauri/src/commands/ocr.rs` - OCR 命令实现
- `src-tauri/src/core/file_parser.rs` - PDF 解析和 OCR 调用
- `src-tauri/Cargo.toml` - 依赖配置

### 前端
- `src/components/file/OcrDownloadDialog.tsx` - 下载对话框
- `src/pages/FileProcess.tsx` - 文件处理页面
- `src/lib/tauri.ts` - Tauri 命令封装
- `src/types/commands.ts` - 类型定义

### 文档
- `OCR_DOWNLOAD_GUIDE.md` - 本文档
- `BUILD_WITH_PYTHON_GUIDE.md` - 打包指南
- `OCR_PACKAGING.md` - OCR 打包方案

## 总结

OCR 按需下载方案的优势：

1. **小安装包**：从 600MB 减小到 10MB
2. **按需安装**：只有需要 OCR 的用户才下载
3. **易于更新**：OCR 包可以独立更新
4. **用户友好**：自动检测、一键下载、无需重启

这个方案在保持功能完整性的同时，大幅减小了安装包体积，提升了用户体验。
