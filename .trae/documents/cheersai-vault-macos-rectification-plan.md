# CheersAI Vault macOS 适配整改与优化方案

## Summary

本方案面向 [CheersAI-Vault](file:///Users/FYP/Documents/WorkSpace/CheersAI/subproducts/CheersAI-Vault) 当前“主要按 Windows 开发和测试”的现状，补齐 macOS 下的核心运行能力与用户体验。整改重点聚焦 3 个方向：

1. 文件路径与目录口径统一：前后端都改为通过真实平台上下文驱动，避免继续在 UI 和命令层硬编码 Windows 路径。
2. 本地服务状态检测与启动优化：补齐 macOS 下 OCR、Ollama、PIN/安全存储的实际可用链路，避免“检测已安装但不可用”或“只能 Windows 使用”的情况。
3. 用户体验优化：把当前明显的 Windows 专属提示、浏览器原生确认框、错误信息和路径说明改成平台感知型交互。

本次方案不直接改代码，只输出一版可执行的整改计划，确认后再进入实施。

## Current State Analysis

### 1. 路径与平台信息仍然由前端猜测，且文案明显偏 Windows

- [path.ts](file:///Users/FYP/Documents/WorkSpace/CheersAI/subproducts/CheersAI-Vault/src/lib/path.ts) 通过 `navigator.userAgent` 推断平台，只适合展示层，不适合作为 Tauri 桌面应用的真实平台事实来源。
- [SandboxManager.tsx](file:///Users/FYP/Documents/WorkSpace/CheersAI/subproducts/CheersAI-Vault/src/pages/SandboxManager.tsx) 和 [EnhancedServices.tsx](file:///Users/FYP/Documents/WorkSpace/CheersAI/subproducts/CheersAI-Vault/src/pages/EnhancedServices.tsx) 仍使用 `C:\...` 作为默认路径示例。
- [SandboxManager.tsx](file:///Users/FYP/Documents/WorkSpace/CheersAI/subproducts/CheersAI-Vault/src/pages/SandboxManager.tsx) 的 PIN 提示文案直接写死为 “Windows DPAPI 加密”。
- [FileBayConfigManager.tsx](file:///Users/FYP/Documents/WorkSpace/CheersAI/subproducts/CheersAI-Vault/src/components/filebay/FileBayConfigManager.tsx) 仍存在 `http://localhost:3000/filebay-download` 的硬编码入口，不适合作为正式桌面端跨平台体验基线。

### 2. PIN/安全存储在 macOS 下并不安全，且与产品口径不一致

- [dpapi.rs](file:///Users/FYP/Documents/WorkSpace/CheersAI/subproducts/CheersAI-Vault/src-tauri/src/core/dpapi.rs) 在非 Windows 平台只是 base64 fallback，本质上不是安全存储。
- [sandbox.rs](file:///Users/FYP/Documents/WorkSpace/CheersAI/subproducts/CheersAI-Vault/src-tauri/src/commands/sandbox.rs) 的命令说明仍围绕 Windows DPAPI 设计。
- PRD 中已经明确希望使用 “Keychain (macOS) / Credential Manager (Windows)” 的 OS 级保护，但代码未落地。

### 3. OCR 下载与执行链路本质上仍是 Windows 方案

- [ocr.rs](file:///Users/FYP/Documents/WorkSpace/CheersAI/subproducts/CheersAI-Vault/src-tauri/src/commands/ocr.rs) 硬编码下载 `python-3.11.9-embed-amd64.zip`，并固定依赖 `python.exe`。
- [file_parser.rs](file:///Users/FYP/Documents/WorkSpace/CheersAI/subproducts/CheersAI-Vault/src-tauri/src/core/file_parser.rs) 也固定优先查找 `ocr-package/python/python.exe` 与 `pdf_ocr.exe`。
- 这意味着当前 OCR 自动安装/自动识别能力对 macOS 实际不可用，只能靠“系统 Python 恰好可用”碰运气。

### 4. Ollama 的 macOS 检测有基础，但服务状态与启动体验仍不完整

- [ai_model.rs](file:///Users/FYP/Documents/WorkSpace/CheersAI/subproducts/CheersAI-Vault/src-tauri/src/commands/ai_model.rs) 已包含 macOS 路径候选和 `which ollama` 查找。
- 但 `check_ai_model_installed()` 通过 `ollama list` 直接判断，未把“二进制存在”和“服务已运行”拆开。
- `start_ollama_service()` 在非 Windows 平台统一直接执行 `ollama serve`，这在 macOS 下体验偏粗糙，且容易造成重复前台进程或状态不一致。
- `download_ollama()` 的失败提示仍是“下载 Windows 版本并安装到 C:\Ollama”。

### 5. 沙箱/服务页在 macOS 下存在体验不一致

- [EnhancedServices.tsx](file:///Users/FYP/Documents/WorkSpace/CheersAI/subproducts/CheersAI-Vault/src/pages/EnhancedServices.tsx) 仍使用浏览器原生 `confirm()`，桌面端体验不统一。
- OCR / AI 服务说明未区分 “Windows 自动安装” 与 “macOS 引导安装/自检/启动”。
- [OcrDownloadDialog.tsx](file:///Users/FYP/Documents/WorkSpace/CheersAI/subproducts/CheersAI-Vault/src/components/file/OcrDownloadDialog.tsx) 描述仍基于当前 Windows 式 OCR 下载流程，未为 macOS 提供单独话术和状态反馈。

### 6. 数据库与部分目录已具备跨平台基础，但口径不统一

- [database.rs](file:///Users/FYP/Documents/WorkSpace/CheersAI/subproducts/CheersAI-Vault/src-tauri/src/core/database.rs) 的 `get_database_path()` 当前落在 `temp_dir/cheersai-vault/cheersai-vault.db`，具备跨平台可运行性，但与同文件下 `get_cross_platform_app_data_dir()` 口径不一致。
- [sandbox.rs](file:///Users/FYP/Documents/WorkSpace/CheersAI/subproducts/CheersAI-Vault/src-tauri/src/commands/sandbox.rs) 的输出目录也基于 `temp_dir`，可运行，但展示层和安全存储层又在走其他目录逻辑，导致“界面展示路径”和“真实运行路径”容易脱节。

## Assumptions & Decisions

- 本次整改目标是“让 macOS 可稳定运行、可理解、可引导”，不是一次性做完整的 macOS 对外发布链路。
- 本次整改范围内会优先解决运行正确性与体验一致性，不包含 Apple 签名、公证、DMG 发版流程。
- Windows 现有可用链路保持兼容，不做破坏性重构；所有改动以“抽象平台差异”替代“推翻现有实现”为原则。
- OCR 在 macOS 的目标不是复制 Windows 的“嵌入式 Python ZIP 下载方案”，而是改为“macOS 原生更合理的 venv/bootstrap 方案 + 明确的失败指引”。
- PIN 安全存储在 macOS 上采用 Keychain 方案，优先使用系统 `security` 命令实现，避免引入不必要的新原生依赖。
- Ollama 检测与启动在 macOS 上采用“二进制检测”和“服务可达性检测”分层处理，避免继续把“安装了”与“能用”混为一谈。

## Proposed Changes

### A. 建立统一的平台上下文层

#### 目标

让前端不再靠 `navigator.userAgent` 猜测平台，而是从 Rust 后端获取真实平台能力和默认路径。

#### 涉及文件

- [path.ts](file:///Users/FYP/Documents/WorkSpace/CheersAI/subproducts/CheersAI-Vault/src/lib/path.ts)
- [tauri.ts](file:///Users/FYP/Documents/WorkSpace/CheersAI/subproducts/CheersAI-Vault/src/lib/tauri.ts)
- [commands/mod.rs](file:///Users/FYP/Documents/WorkSpace/CheersAI/subproducts/CheersAI-Vault/src-tauri/src/commands/mod.rs)
- 新增平台信息命令文件，建议放在 `src-tauri/src/commands/platform.rs`
- [types/commands.ts](file:///Users/FYP/Documents/WorkSpace/CheersAI/subproducts/CheersAI-Vault/src/types/commands.ts)

#### 修改策略

- 新增 `get_platform_context` Tauri 命令，返回：
  - `os`: `windows | macos | linux`
  - `pathSeparator`
  - `defaultDocumentsDir`
  - `appDataDir`
  - `cacheDir`
  - `tempDir`
  - `pinStorageMode`
  - `ocrStrategy`
  - `ollamaStrategy`
- 前端把 [path.ts](file:///Users/FYP/Documents/WorkSpace/CheersAI/subproducts/CheersAI-Vault/src/lib/path.ts) 降级为“纯格式化辅助库”，不再负责平台真值判断。
- 所有页面展示默认路径时，都改为读取平台上下文，而不是自己拼 Windows/macOS 示例字符串。

#### 为什么这样做

- 这是后续所有 macOS 适配的基础设施；不先统一平台上下文，后续每个页面都会继续各写各的条件分支。

### B. 把 PIN 安全存储从 “Windows-only 设计” 升级为 “Windows + macOS 真正可用”

#### 目标

让 PIN 在 macOS 下不再是 base64 fallback，而是使用 Keychain 存储，并同步更新界面文案。

#### 涉及文件

- [dpapi.rs](file:///Users/FYP/Documents/WorkSpace/CheersAI/subproducts/CheersAI-Vault/src-tauri/src/core/dpapi.rs)
- [sandbox.rs](file:///Users/FYP/Documents/WorkSpace/CheersAI/subproducts/CheersAI-Vault/src-tauri/src/commands/sandbox.rs)
- [SandboxManager.tsx](file:///Users/FYP/Documents/WorkSpace/CheersAI/subproducts/CheersAI-Vault/src/pages/SandboxManager.tsx)
- [tauri.ts](file:///Users/FYP/Documents/WorkSpace/CheersAI/subproducts/CheersAI-Vault/src/lib/tauri.ts)

#### 修改策略

- 保留 Windows DPAPI 逻辑。
- 在 `dpapi.rs` 中将模块角色重构为“跨平台 PIN 安全存储”，内部实现分支为：
  - Windows: 继续用 DPAPI。
  - macOS: 使用 `security add-generic-password` / `find-generic-password -w` / `delete-generic-password`，服务名固定为 `com.cheersai.vault.pin`。
  - Linux: 先保留当前 fallback，但在返回给前端的平台能力里明确标记为“弱保护/兼容模式”。
- 前端 [SandboxManager.tsx](file:///Users/FYP/Documents/WorkSpace/CheersAI/subproducts/CheersAI-Vault/src/pages/SandboxManager.tsx) 不再硬编码 “Windows DPAPI 加密”，而是根据 `pinStorageMode` 显示：
  - Windows: “PIN 将使用 Windows 凭据保护”
  - macOS: “PIN 将使用 macOS Keychain 安全存储”
  - Linux: “PIN 将以兼容模式存储”
- `set_pin()` 成功提示也改成平台感知型文案。

#### 为什么这样做

- 这是当前 macOS 风险最高的一处：功能表面可用，但安全性和产品口径完全不成立。

### C. 重做 OCR 的 macOS 路径与运行时方案

#### 目标

让 OCR 在 macOS 下至少具备“可检测、可安装、可调用、失败可解释”的完整链路。

#### 涉及文件

- [ocr.rs](file:///Users/FYP/Documents/WorkSpace/CheersAI/subproducts/CheersAI-Vault/src-tauri/src/commands/ocr.rs)
- [file_parser.rs](file:///Users/FYP/Documents/WorkSpace/CheersAI/subproducts/CheersAI-Vault/src-tauri/src/core/file_parser.rs)
- [EnhancedServices.tsx](file:///Users/FYP/Documents/WorkSpace/CheersAI/subproducts/CheersAI-Vault/src/pages/EnhancedServices.tsx)
- [OcrDownloadDialog.tsx](file:///Users/FYP/Documents/WorkSpace/CheersAI/subproducts/CheersAI-Vault/src/components/file/OcrDownloadDialog.tsx)
- [types/commands.ts](file:///Users/FYP/Documents/WorkSpace/CheersAI/subproducts/CheersAI-Vault/src/types/commands.ts)

#### 修改策略

- 抽出 OCR 运行时解析函数，统一返回：
  - 实际 OCR python 可执行路径
  - OCR 脚本路径
  - OCR 安装模式
- Windows 保持现有嵌入式 Python 方案。
- macOS 改为：
  - 优先检查 `app_data_dir/ocr-package/venv/bin/python3` 或 `bin/python`
  - 若不存在，则检查系统 `python3`
  - 自动安装时不再下载 `embed-amd64.zip`，而是：
    1. 使用系统 `python3 -m venv` 在 `app_data_dir/ocr-package/venv` 创建虚拟环境
    2. 用 venv 内的 python 安装 `PyMuPDF`
    3. 复制 `pdf_ocr.py`
  - 若系统没有 `python3`，返回明确错误并引导用户安装 Command Line Tools 或 Python
- [file_parser.rs](file:///Users/FYP/Documents/WorkSpace/CheersAI/subproducts/CheersAI-Vault/src-tauri/src/core/file_parser.rs) 不再硬编码 `python.exe` / `pdf_ocr.exe`，统一复用 OCR 运行时解析逻辑。
- 前端服务页与下载对话框改为区分平台说明：
  - Windows: 自动下载内置 Python 运行时
  - macOS: 创建本地 venv 并安装 PyMuPDF

#### 为什么这样做

- 这是当前 macOS 下最明确的“代码根本不可用”点，必须优先整改。

### D. 重构 Ollama 检测与服务状态模型

#### 目标

让 macOS 下 AI 模型能力的提示更准确：区分“是否安装 Ollama”“Ollama 服务是否已运行”“模型是否已拉取”。

#### 涉及文件

- [ai_model.rs](file:///Users/FYP/Documents/WorkSpace/CheersAI/subproducts/CheersAI-Vault/src-tauri/src/commands/ai_model.rs)
- [core/ner.rs](file:///Users/FYP/Documents/WorkSpace/CheersAI/subproducts/CheersAI-Vault/src-tauri/src/core/ner.rs)
- [EnhancedServices.tsx](file:///Users/FYP/Documents/WorkSpace/CheersAI/subproducts/CheersAI-Vault/src/pages/EnhancedServices.tsx)
- [tauri.ts](file:///Users/FYP/Documents/WorkSpace/CheersAI/subproducts/CheersAI-Vault/src/lib/tauri.ts)

#### 修改策略

- 抽出统一的 `resolve_ollama_path()`，避免 [ai_model.rs](file:///Users/FYP/Documents/WorkSpace/CheersAI/subproducts/CheersAI-Vault/src-tauri/src/commands/ai_model.rs) 与 [ner.rs](file:///Users/FYP/Documents/WorkSpace/CheersAI/subproducts/CheersAI-Vault/src-tauri/src/core/ner.rs) 重复维护路径搜索逻辑。
- 新增两个状态命令：
  - `check_ollama_binary_installed`
  - `check_ollama_service_running`
- 服务运行状态优先通过本地接口探测 `http://127.0.0.1:11434/api/tags`，超时短一些，避免页面卡顿。
- `check_ai_model_installed()` 依赖“服务可达 + `ollama list` 成功”这两个条件，不再用单一命令结果混合表达。
- macOS 启动策略调整为：
  - 若检测到 `Ollama.app`，优先 `open -a Ollama`
  - 否则 fallback 到 `ollama serve`
  - 返回文案区分“已触发启动，请稍后重扫”和“启动失败”
- `download_ollama()` 的提示语改为平台感知型，不再出现 “下载 Windows 版本”“安装到 C:\Ollama”。

#### 为什么这样做

- 当前用户看到的“已安装/未安装”并不准确，macOS 下最容易出现“装了命令行，但服务没起来”的误判。

### E. 统一默认目录与真实落点，减少路径认知混乱

#### 目标

让应用中“显示给用户的目录”和“代码真实使用的目录”尽可能一致。

#### 涉及文件

- [database.rs](file:///Users/FYP/Documents/WorkSpace/CheersAI/subproducts/CheersAI-Vault/src-tauri/src/core/database.rs)
- [sandbox.rs](file:///Users/FYP/Documents/WorkSpace/CheersAI/subproducts/CheersAI-Vault/src-tauri/src/commands/sandbox.rs)
- [path.ts](file:///Users/FYP/Documents/WorkSpace/CheersAI/subproducts/CheersAI-Vault/src/lib/path.ts)
- [SandboxManager.tsx](file:///Users/FYP/Documents/WorkSpace/CheersAI/subproducts/CheersAI-Vault/src/pages/SandboxManager.tsx)

#### 修改策略

- 统一定义以下目录口径：
  - 数据库: 继续使用 `temp_dir/cheersai-vault/cheersai-vault.db`，但在平台上下文中明确标记为“运行时数据库”
  - 沙箱输出: 继续使用 `temp_dir/cheersai-vault/output`
  - OCR/Ollama/配置: 使用 `app_data_dir`
- 前端展示时区分：
  - “默认输出目录”
  - “应用数据目录”
  - “运行时缓存/临时目录”
- [SandboxManager.tsx](file:///Users/FYP/Documents/WorkSpace/CheersAI/subproducts/CheersAI-Vault/src/pages/SandboxManager.tsx) 中的输出路径 placeholder 与帮助文本改为读取真实平台默认目录。

#### 为什么这样做

- 当前不是目录本身一定错，而是“展示目录”和“真实执行目录”口径不一致，容易让 macOS 用户误解文件实际落点。

### F. macOS 体验层优化

#### 目标

让服务页、沙箱页、配置页在 macOS 下不再像“把 Windows 提示硬搬过来”。

#### 涉及文件

- [EnhancedServices.tsx](file:///Users/FYP/Documents/WorkSpace/CheersAI/subproducts/CheersAI-Vault/src/pages/EnhancedServices.tsx)
- [SandboxManager.tsx](file:///Users/FYP/Documents/WorkSpace/CheersAI/subproducts/CheersAI-Vault/src/pages/SandboxManager.tsx)
- [FileBayConfigManager.tsx](file:///Users/FYP/Documents/WorkSpace/CheersAI/subproducts/CheersAI-Vault/src/components/filebay/FileBayConfigManager.tsx)
- [OcrDownloadDialog.tsx](file:///Users/FYP/Documents/WorkSpace/CheersAI/subproducts/CheersAI-Vault/src/components/file/OcrDownloadDialog.tsx)

#### 修改策略

- 把原生 `confirm()` 替换为统一的 `Dialog` 确认框。
- OCR / AI 服务页上的安装路径 placeholder 和说明全部改为平台感知型。
- `SandboxManager` 的安全说明改为平台感知型，不再输出 Windows 专属承诺。
- `FileBayConfigManager` 中的下载入口从固定 `localhost` 改为：
  - 优先打开当前配置中的 Desktop/Cloud URL
  - 若无配置，则展示说明而非打开假地址
- 对 macOS 下可能失败的动作，错误文案明确包含“下一步建议”，例如：
  - 未找到 `python3`
  - Ollama 二进制存在但服务未启动
  - Keychain 写入失败

#### 为什么这样做

- 这类问题不会直接导致编译失败，但会明显拉低 macOS 版本可用性与可信度。

## Implementation Order

### P0

- 建平台上下文命令与前端类型
- 修复 PIN 安全存储的 macOS 实现
- 修复 OCR 的 macOS 安装/检测/执行链路
- 修复 Ollama 的 macOS 状态检测与启动策略

### P1

- 全量替换前端路径 placeholder、说明文案、成功/失败提示
- 统一服务页确认框与错误反馈
- 收敛重复的路径与状态判断逻辑

### P2

- 优化 FileBay 配置页入口 URL 策略
- 进一步梳理数据库/临时目录/应用数据目录在 UI 中的表达

## Verification Steps

### 代码级验证

1. 前端类型检查通过。
2. Rust 编译与 `cargo check` 通过。
3. 相关页面无新增诊断错误。

### macOS 功能验证

1. 启动应用后，沙箱页能正确显示 macOS 平台名称与路径示例。
2. 设置 PIN 后，可在 macOS Keychain 中看到对应条目；重启应用后仍可验证 PIN。
3. OCR 服务在以下场景都能得到正确结果：
   - 已有 venv 与 PyMuPDF
   - 只有系统 `python3`
   - 没有 `python3`
4. Ollama 在以下场景提示正确：
   - 未安装二进制
   - 已安装但服务未运行
   - 服务已运行但模型未拉取
   - 服务已运行且模型已安装
5. 服务页所有路径 placeholder、安装说明、错误提示均不再出现 Windows 专属路径示例。

### 回归验证

1. Windows 现有 OCR 安装链路不回退。
2. Windows PIN 仍继续使用 DPAPI。
3. 现有 Vault 嵌入 Desktop 的主流程不受影响。

