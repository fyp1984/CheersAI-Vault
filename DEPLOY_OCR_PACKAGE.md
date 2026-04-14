# OCR 包部署快速指南

## 准备 OCR 包

### 1. 构建 Python 环境

```bash
# 运行设置脚本（首次需要 10-15 分钟）
setup_embedded_python.bat
```

这会：
- 下载 Python 3.11.9 嵌入式版本
- 安装 pip
- 安装 easyocr 和 PyMuPDF
- 总大小约 600MB

### 2. 创建 OCR 包

```bash
# 复制文件到 dist/ocr-package
build_ocr_with_python.bat
```

### 3. 压缩为 ZIP

```bash
# 使用 7-Zip（推荐）
cd dist
7z a -tzip ocr-package.zip ocr-package/

# 或使用 PowerShell
Compress-Archive -Path ocr-package -DestinationPath ocr-package.zip
```

最终文件：`dist/ocr-package.zip`（约 100MB）

## 上传到云存储

### 选项 1：GitHub Releases（推荐）

#### 优点
- 免费
- 全球 CDN
- 版本管理
- 易于更新

#### 步骤

1. **创建 Release**
   ```bash
   gh release create v1.0-ocr \
     --title "OCR Package v1.0" \
     --notes "OCR dependencies for CheersAI Desktop"
   ```

2. **上传文件**
   ```bash
   gh release upload v1.0-ocr dist/ocr-package.zip
   ```

3. **获取下载 URL**
   ```
   https://github.com/YOUR-ORG/cheersai-desktop/releases/download/v1.0-ocr/ocr-package.zip
   ```

### 选项 2：阿里云 OSS

#### 优点
- 国内速度快
- 稳定可靠
- 按量付费

#### 步骤

1. **安装 ossutil**
   ```bash
   # 下载：https://help.aliyun.com/document_detail/120075.html
   ```

2. **配置**
   ```bash
   ossutil config
   # 输入 AccessKey ID、AccessKey Secret、Endpoint
   ```

3. **上传**
   ```bash
   ossutil cp dist/ocr-package.zip oss://your-bucket/ocr-package.zip
   ```

4. **设置公共读**
   ```bash
   ossutil set-acl oss://your-bucket/ocr-package.zip public-read
   ```

5. **获取 URL**
   ```
   https://your-bucket.oss-cn-hangzhou.aliyuncs.com/ocr-package.zip
   ```

### 选项 3：腾讯云 COS

#### 步骤

1. **安装 COSCMD**
   ```bash
   pip install coscmd
   ```

2. **配置**
   ```bash
   coscmd config -a YOUR_SECRET_ID -s YOUR_SECRET_KEY -b your-bucket -r ap-guangzhou
   ```

3. **上传**
   ```bash
   coscmd upload dist/ocr-package.zip ocr-package.zip
   ```

4. **获取 URL**
   ```
   https://your-bucket-1234567890.cos.ap-guangzhou.myqcloud.com/ocr-package.zip
   ```

## 配置应用

### 方法 1：环境变量（推荐）

**开发环境**
```bash
# Windows CMD
set OCR_DOWNLOAD_URL=https://github.com/YOUR-ORG/cheersai-desktop/releases/download/v1.0-ocr/ocr-package.zip

# Windows PowerShell
$env:OCR_DOWNLOAD_URL="https://github.com/YOUR-ORG/cheersai-desktop/releases/download/v1.0-ocr/ocr-package.zip"

# Linux/Mac
export OCR_DOWNLOAD_URL="https://github.com/YOUR-ORG/cheersai-desktop/releases/download/v1.0-ocr/ocr-package.zip"
```

**生产环境**
- 在系统环境变量中设置
- 或在应用启动脚本中设置

### 方法 2：修改代码

编辑 `src-tauri/src/commands/ocr.rs`：

```rust
let download_url = std::env::var("OCR_DOWNLOAD_URL")
    .unwrap_or_else(|_| "https://github.com/YOUR-ORG/cheersai-desktop/releases/download/v1.0-ocr/ocr-package.zip".to_string());
```

将 URL 替换为实际地址。

## 构建应用

### 精简版（不含 OCR）

```bash
# 确保 tauri.conf.json 中 resources 只包含 scripts
# "resources": ["scripts/*"]

# 构建
pnpm tauri build
```

生成：
- `src-tauri/target/release/bundle/nsis/CheersAI Desktop_0.1.2_x64-setup.exe`
- 大小：~10MB

## 测试流程

### 1. 安装应用

```bash
# 运行安装程序
src-tauri\target\release\bundle\nsis\CheersAI Desktop_0.1.2_x64-setup.exe
```

### 2. 测试 OCR 下载

1. 启动应用
2. 拖入扫描版 PDF（测试文件）
3. 应用提示下载 OCR
4. 点击"开始下载"
5. 观察下载进度
6. 等待安装完成
7. 重新处理 PDF

### 3. 验证安装

检查文件是否存在：
```
%APPDATA%\com.cheersai.vault\ocr-package\
├── python\
│   ├── python.exe
│   └── ...
└── pdf_ocr.py
```

### 4. 测试 OCR 功能

1. 处理扫描版 PDF
2. 检查识别结果
3. 验证脱敏功能

## 更新 OCR 包

### 1. 构建新版本

```bash
# 更新 Python 依赖
cd python-embed
.\python.exe -m pip install --upgrade easyocr PyMuPDF

# 重新构建 OCR 包
cd ..
build_ocr_with_python.bat

# 压缩
cd dist
7z a -tzip ocr-package-v1.1.zip ocr-package/
```

### 2. 上传新版本

```bash
# GitHub
gh release create v1.1-ocr
gh release upload v1.1-ocr dist/ocr-package-v1.1.zip

# 阿里云 OSS
ossutil cp dist/ocr-package-v1.1.zip oss://your-bucket/ocr-package-v1.1.zip
```

### 3. 更新应用配置

更新下载 URL 指向新版本。

## 监控和维护

### 下载统计

**GitHub Releases**
- 在 Release 页面查看下载次数
- 使用 GitHub API 获取详细统计

**阿里云 OSS**
- 在控制台查看访问日志
- 配置日志分析

### 成本估算

**GitHub Releases**
- 免费（有流量限制）
- 适合中小规模

**阿里云 OSS**
- 存储费用：0.12 元/GB/月
- 流量费用：0.5 元/GB（国内）
- 100MB 文件，1000 次下载/月 ≈ 50 元/月

**腾讯云 COS**
- 类似阿里云
- 价格略有差异

## 故障排查

### 下载失败

**症状**：用户报告下载失败

**检查**：
1. URL 是否正确
2. 文件是否存在
3. 权限是否设置为公共读
4. 网络是否可达

**解决**：
```bash
# 测试 URL
curl -I https://your-url.com/ocr-package.zip

# 应该返回 200 OK
```

### 解压失败

**症状**：下载完成但解压失败

**检查**：
1. ZIP 文件是否完整
2. 磁盘空间是否足够
3. 权限是否正确

**解决**：
- 重新上传 ZIP 文件
- 验证 ZIP 文件完整性

### OCR 不工作

**症状**：下载成功但 OCR 不工作

**检查**：
1. 文件结构是否正确
2. python.exe 是否存在
3. 依赖是否完整

**解决**：
```bash
# 手动测试
cd %APPDATA%\com.cheersai.vault\ocr-package
python\python.exe pdf_ocr.py test.pdf
```

## 最佳实践

### 1. 版本管理

- 使用语义化版本号（v1.0, v1.1, v2.0）
- 保留旧版本以支持旧客户端
- 在 Release Notes 中说明变更

### 2. 镜像源

- 提供多个下载源（GitHub + 国内 CDN）
- 在应用中支持切换下载源
- 自动选择最快的源

### 3. 完整性验证

- 提供 SHA256 校验和
- 在下载后验证文件完整性
- 防止文件损坏或篡改

### 4. 增量更新

- 只更新变化的文件
- 减少下载大小
- 提高更新速度

## 自动化部署

### GitHub Actions

创建 `.github/workflows/release-ocr.yml`：

```yaml
name: Release OCR Package

on:
  push:
    tags:
      - 'ocr-v*'

jobs:
  build-and-release:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v3
      
      - name: Setup Python Environment
        run: .\setup_embedded_python.bat
      
      - name: Build OCR Package
        run: .\build_ocr_with_python.bat
      
      - name: Compress Package
        run: |
          cd dist
          7z a -tzip ocr-package.zip ocr-package/
      
      - name: Create Release
        uses: softprops/action-gh-release@v1
        with:
          files: dist/ocr-package.zip
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
```

使用：
```bash
git tag ocr-v1.0
git push origin ocr-v1.0
```

## 总结

部署 OCR 包的关键步骤：

1. ✅ 构建 Python 环境
2. ✅ 创建 OCR 包
3. ✅ 压缩为 ZIP
4. ✅ 上传到云存储
5. ✅ 配置下载 URL
6. ✅ 构建精简版应用
7. ✅ 测试完整流程

完成这些步骤后，用户就可以享受快速安装 + 按需下载的体验了！
