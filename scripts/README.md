# 安装脚本说明

这个目录包含独立的安装脚本，用于安装 OCR 和 Ollama 环境。

## 文件说明

- `install_ocr.py` - OCR 环境安装脚本（Python + PyMuPDF + EasyOCR）
- `install_ollama.py` - Ollama + AI 模型安装脚本
- `test_installers.py` - 测试脚本

## OCR 安装脚本 (install_ocr.py)

### 功能特点

1. **自动下载和安装**
   - Python 3.11.9 嵌入式版本
   - pip 包管理器
   - PyMuPDF (PDF 解析)
   - EasyOCR (OCR 识别)

2. **使用国内镜像**
   - Python: 华为云镜像
   - PyPI: 阿里云镜像

3. **健壮性**
   - 自动重试机制（大包 3 次，小包 1 次）
   - 详细的进度输出
   - 完整的错误处理
   - 安装验证

4. **安装位置**
   - 默认: `%APPDATA%\com.cheersai.vault\ocr-package`
   - 可自定义

### 使用方法

```bash
# 安装
python install_ocr.py

# 卸载
python install_ocr.py uninstall
```

### 输出文件

- `python/` - Python 嵌入式版本
- `pdf_ocr.py` - PDF OCR 处理脚本
- `install_info.json` - 安装信息

### 使用 OCR

```bash
# 使用安装的 Python 运行 OCR
%APPDATA%\com.cheersai.vault\ocr-package\python\python.exe ^
  %APPDATA%\com.cheersai.vault\ocr-package\pdf_ocr.py ^
  "path/to/file.pdf"
```

## Ollama 安装脚本 (install_ollama.py)

### 功能特点

1. **完全自动化安装**
   - 自动下载 Ollama 安装程序（~600 MB）
   - 静默安装 Ollama
   - 自动启动 Ollama 服务
   - 自动下载 AI 模型 (qwen2.5:1.5b, ~1 GB)

2. **健壮性**
   - 完整的错误处理和重试机制
   - 详细的进度显示
   - 服务状态检测
   - 模型安装验证
   - 自动测试模型

3. **卸载功能**
   - 停止所有 Ollama 进程
   - 删除程序文件
   - 删除用户数据和模型
   - 完全清理

### 使用方法

```bash
# 完整安装（自动下载并安装 Ollama + 模型）
python install_ollama.py

# 完全卸载（删除 Ollama 和所有数据）
python install_ollama.py uninstall
```

### 安装位置

- 程序: `%LOCALAPPDATA%\Programs\Ollama`
- 数据: `%USERPROFILE%\.ollama`

### 注意事项

- 首次安装需要下载约 1.6 GB 数据
- 安装过程约需 3-5 分钟
- 需要稳定的网络连接

## 集成到项目

### 1. 在 Rust 中调用

```rust
use std::process::Command;

// 安装 OCR
let output = Command::new("python")
    .arg("scripts/install_ocr.py")
    .output()
    .expect("Failed to run OCR installer");

// 解析输出
let info: serde_json::Value = serde_json::from_slice(&output.stdout)?;
```

### 2. 在 Tauri 命令中使用

```rust
#[tauri::command]
pub async fn install_ocr() -> Result<String, String> {
    let script_path = "scripts/install_ocr.py";
    
    let output = Command::new("python")
        .arg(script_path)
        .output()
        .map_err(|e| format!("Failed to run installer: {}", e))?;
    
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Installation failed: {}", stderr));
    }
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(stdout.to_string())
}
```

### 3. 进度监控

脚本输出格式：
```
[2024-01-01 12:00:00] [INFO] 下载 Python 3.11.9 (尝试 1/3)
[2024-01-01 12:00:01] [INFO] 下载进度: 50.0% (5.60 MB / 11.20 MB)
```

可以通过解析 stderr 来实时显示进度。

## 测试

```bash
# 测试 OCR 安装
python scripts/test_installers.py ocr

# 测试 Ollama 安装
python scripts/test_installers.py ollama

# 测试全部
python scripts/test_installers.py all
```

## 故障排除

### OCR 安装失败

1. **网络问题**
   - 检查网络连接
   - 尝试使用 VPN
   - 手动下载文件后放到指定位置

2. **权限问题**
   - 以管理员身份运行
   - 检查 %APPDATA% 目录权限

3. **空间不足**
   - OCR 环境约需 600MB
   - 检查磁盘空间

### Ollama 安装失败

1. **下载失败**
   - 检查网络连接
   - 脚本会自动重试 3 次
   - 如果持续失败，可手动下载安装程序

2. **安装超时**
   - 安装过程可能需要几分钟
   - 如果超时，可以重新运行脚本
   - 脚本会检测已安装的组件并跳过

3. **服务启动失败**
   - 检查端口 11434 是否被占用
   - 手动运行 `ollama serve`
   - 检查防火墙设置

4. **模型下载慢**
   - 模型约 1GB，需要耐心等待
   - 脚本会显示详细的下载进度
   - 可以使用国内镜像加速

5. **卸载失败**
   - 某些文件可能被锁定
   - 脚本会尝试强制删除
   - 如果失败，重启后再试或手动删除目录

## 性能优化

### OCR

- 首次运行会下载 EasyOCR 模型（约 100MB）
- 模型缓存在用户目录，后续运行更快
- 建议使用 GPU 加速（需要 CUDA）

### Ollama

- 首次运行模型会加载到内存（约 1.5GB）
- 后续调用响应更快
- 可以配置模型缓存策略

## 更新日志

### v1.0.0 (2024-01-01)

- 初始版本
- 支持 OCR 和 Ollama 安装
- 使用国内镜像源
- 完整的错误处理和重试机制
