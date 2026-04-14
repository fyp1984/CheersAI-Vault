# CheersAI Vault

<div align="center">
  <img src="public/logo.jpg" alt="CheersAI Vault Logo" width="120" height="120">
  
  **安全的数据脱敏桌面应用程序**
  
  [![Version](https://img.shields.io/badge/version-0.1.0-blue.svg)](https://github.com/your-org/cheersai-vault)
  [![License](https://img.shields.io/badge/license-MIT-green.svg)](LICENSE)
  [![Platform](https://img.shields.io/badge/platform-Windows%20%7C%20macOS%20%7C%20Linux-lightgrey.svg)](#支持平台)
</div>

## 📖 项目简介

CheersAI Vault 是一款基于 Tauri + React + TypeScript 开发的跨平台桌面应用程序，专注于数据脱敏和隐私保护。应用提供了直观的用户界面，支持批量文件处理、自定义脱敏规则、加密存储等功能。

### 🎯 核心功能

- **📁 批量文件处理** - 支持拖拽上传，批量处理多种文件格式
- **🔒 数据脱敏** - 内置多种脱敏规则，支持自定义规则配置
- **🛡️ 安全存储** - 加密映射文件，PIN 码保护沙箱环境
- **📊 操作日志** - 完整的操作记录，支持分页查看和统计
- **⚙️ 沙箱管理** - 统一的安全设置和文件输出管理
- **🔍 OCR 支持** - 自动识别扫描版 PDF，支持中英文混合识别
- **🌍 跨平台** - 支持 Windows、macOS、Linux 三大平台

## 🏗️ 技术架构

### 前端技术栈
- **框架**: React 19 + TypeScript
- **构建工具**: Vite 7
- **UI 组件**: Radix UI + Tailwind CSS
- **状态管理**: Zustand
- **路由**: React Router v7
- **图标**: Lucide React

### 后端技术栈
- **框架**: Tauri 2.0 (Rust)
- **数据库**: SQLite (本地存储)
- **加密**: AES-GCM + PBKDF2
- **文件处理**: 支持 CSV, Excel, Word, PowerPoint, Markdown 等格式

### 项目结构

```
cheersai-vault/
├── src/                          # React 前端源码
│   ├── components/               # UI 组件
│   │   ├── common/              # 通用组件
│   │   ├── file/                # 文件处理相关组件
│   │   ├── layout/              # 布局组件
│   │   └── ui/                  # 基础 UI 组件
│   ├── pages/                   # 页面组件
│   ├── store/                   # Zustand 状态管理
│   ├── types/                   # TypeScript 类型定义
│   └── lib/                     # 工具库
├── src-tauri/                   # Tauri 后端源码
│   ├── src/
│   │   ├── commands/            # Tauri 命令处理
│   │   └── core/                # 核心业务逻辑
│   ├── icons/                   # 应用图标
│   └── tauri.conf.json         # Tauri 配置
├── public/                      # 静态资源
└── dist/                        # 构建输出
```

## 🚀 快速开始

### 环境要求

- **Node.js**: >= 18.0.0
- **pnpm**: >= 8.0.0 (推荐) 或 npm/yarn
- **Rust**: >= 1.70.0
- **系统要求**: Windows 10+, macOS 10.15+, 或 Linux (Ubuntu 18.04+)

### 安装依赖

```bash
# 克隆项目
git clone https://github.com/your-org/cheersai-vault.git
cd cheersai-vault

# 安装前端依赖
pnpm install

# 安装 Tauri CLI (如果未安装)
pnpm add -D @tauri-apps/cli
```

### 开发模式

```bash
# 启动开发服务器 (前端 + 后端)
pnpm tauri dev

# 或者分别启动
pnpm dev          # 仅前端开发服务器
pnpm tauri dev    # Tauri 开发模式
```

### 构建应用

```bash
# 构建生产版本（精简版，不含 OCR）
pnpm tauri build

# 构建完整版（包含 OCR 功能，约 500MB）
.\build_full.bat

# 使用自定义构建脚本 (包含图标处理)
.\final-build-solution.ps1
```

### OCR 功能设置

应用支持扫描版 PDF 的 OCR（光学字符识别）功能。

**方式 1：使用完整版（推荐）**

完整版包含嵌入式 Python 环境，用户无需安装任何依赖。

```batch
# 1. 设置嵌入式 Python 环境（首次需要 10-15 分钟）
setup_embedded_python.bat

# 2. 创建 OCR 包
build_ocr_with_python.bat

# 3. 构建完整应用（约 600MB）
build_full_with_python.bat
```

**方式 2：用户手动安装 Python**

精简版不含 Python，用户需要自行安装：

```batch
# 用户运行安装脚本
install_ocr_windows.bat

# 或手动安装
pip install easyocr PyMuPDF
```

**方式 3：PyInstaller 打包**

将 Python 脚本打包为单个 exe（约 500MB）：

```batch
# 构建 OCR exe
build_ocr.bat

# 构建包含 OCR 的完整安装包
build_full.bat
```

详细说明请参考：
- [Python 打包完整指南](BUILD_WITH_PYTHON_GUIDE.md) - **推荐阅读**
- [OCR 安装指南](INSTALL_OCR.md) - 用户安装说明
- [OCR 打包方案](OCR_PACKAGING.md) - 开发者打包指南
- [OCR 实现总结](OCR_IMPLEMENTATION_SUMMARY.md) - 技术实现细节

## 🛠️ 开发指南

### 代码规范

- **TypeScript**: 严格模式，完整类型定义
- **React**: 函数组件 + Hooks，遵循 React 最佳实践
- **Rust**: 遵循 Rust 官方代码规范
- **提交信息**: 使用语义化提交信息

### 主要开发任务

1. **添加新的脱敏规则**
   - 在 `src-tauri/src/core/masking_engine.rs` 中实现规则逻辑
   - 在 `src/components/file/RuleSelector.tsx` 中添加 UI 选项

2. **支持新的文件格式**
   - 在 `src-tauri/src/core/file_parser.rs` 中添加解析器
   - 更新 `src/components/file/DropZone.tsx` 支持的文件类型

3. **添加新页面**
   - 在 `src/pages/` 中创建页面组件
   - 在 `src/App.tsx` 中添加路由配置
   - 在 `src/components/layout/Sidebar.tsx` 中添加导航

### 调试技巧

```bash
# 查看 Tauri 日志
pnpm tauri dev --verbose

# 前端调试
# 在浏览器中打开 http://localhost:1420
# 使用 React DevTools

# 后端调试
# 在 Rust 代码中使用 println! 或 log 宏
# 日志会显示在终端中
```

## 📦 构建和部署

### 本地构建

```bash
# 构建所有平台 (需要对应平台环境)
pnpm tauri build

# 构建特定平台
pnpm tauri build --target x86_64-pc-windows-msvc  # Windows
pnpm tauri build --target x86_64-apple-darwin     # macOS Intel
pnpm tauri build --target aarch64-apple-darwin    # macOS Apple Silicon
pnpm tauri build --target x86_64-unknown-linux-gnu # Linux
```

### 生成安装包

项目包含自动化构建脚本：

- `final-build-solution.ps1` - 完整构建解决方案 (Windows)
- `build-with-icons.ps1` - 带图标的构建脚本
- `generate-icons.ps1` - 图标生成脚本

构建产物：
- **Windows**: `.exe` 可执行文件 + `.msi` 安装包 + NSIS 安装程序
- **macOS**: `.app` 应用包 + `.dmg` 磁盘镜像
- **Linux**: `.AppImage` 或 `.deb` 包

## 🔧 配置说明

### 环境变量

```bash
# 开发环境
TAURI_DEV=true

# 数据库路径 (可选，默认使用应用数据目录)
DATABASE_PATH=/path/to/database.db

# 日志级别
RUST_LOG=debug
```

### 应用配置

主要配置文件：
- `src-tauri/tauri.conf.json` - Tauri 应用配置
- `tailwind.config.js` - Tailwind CSS 配置
- `vite.config.ts` - Vite 构建配置
- `tsconfig.json` - TypeScript 配置

## 🧪 测试

```bash
# 运行前端测试
pnpm test

# 运行 Rust 测试
cd src-tauri
cargo test

# 端到端测试
pnpm test:e2e
```

## 📋 支持平台

| 平台 | 状态 | 最低版本 | 备注 |
|------|------|----------|------|
| Windows | ✅ | Windows 10 | 完全支持 |
| macOS | ✅ | macOS 10.15 | Intel + Apple Silicon |
| Linux | ✅ | Ubuntu 18.04+ | 其他发行版需测试 |

## 🤝 贡献指南

### 参与开发

1. **Fork 项目** 并克隆到本地
2. **创建功能分支**: `git checkout -b feature/amazing-feature`
3. **提交更改**: `git commit -m 'Add amazing feature'`
4. **推送分支**: `git push origin feature/amazing-feature`
5. **创建 Pull Request**

### 报告问题

- 使用 [GitHub Issues](https://github.com/your-org/cheersai-vault/issues) 报告 Bug
- 提供详细的复现步骤和环境信息
- 包含相关的日志和截图

### 开发规范

- 遵循现有代码风格
- 添加适当的注释和文档
- 确保所有测试通过
- 更新相关文档

## 📄 许可证

本项目采用 [MIT 许可证](LICENSE)。

## 🙏 致谢

- [Tauri](https://tauri.app/) - 跨平台桌面应用框架
- [React](https://reactjs.org/) - 用户界面库
- [Radix UI](https://www.radix-ui.com/) - 无障碍 UI 组件
- [Tailwind CSS](https://tailwindcss.com/) - CSS 框架
- [Lucide](https://lucide.dev/) - 图标库

## 📞 联系我们

- **项目维护者**: CheersAI Team
- **邮箱**: team@cheersai.com
- **问题反馈**: [GitHub Issues](https://github.com/your-org/cheersai-vault/issues)

---

<div align="center">
  Made with ❤️ by CheersAI Team
</div>