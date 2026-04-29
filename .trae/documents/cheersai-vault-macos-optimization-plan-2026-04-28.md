# CheersAI Vault macOS 整改与优化执行方案

## Summary

本方案面向 [CheersAI-Vault](file:///Users/FYP/Documents/WorkSpace/CheersAI/subproducts/CheersAI-Vault) 当前“以 Windows 开发与测试为主”的现状，目标是在 **不破坏现有 Windows 主流程** 的前提下，把 macOS 支持提升到“路径口径真实、服务状态可判断、关键能力可运行、页面提示可信”的水平。

本次整改聚焦 3 个方向：

1. 文件路径与平台上下文统一：前后端统一从 Tauri/Rust 获取真实平台信息，避免继续在前端拼接或展示 Windows 假路径。
2. 本地能力与服务状态检测完善：补齐 PIN 安全存储、OCR 运行时、Ollama 安装/运行/模型状态的 macOS 可用链路。
3. 用户体验优化：收敛 Windows 专属文案、错误提示、确认交互和 Cloud/FileBay 入口，让 macOS 用户看到的提示与真实系统行为一致。

本阶段只输出方案，不执行代码改动。确认后再按本方案实施。

## Current State Analysis

### 1. 平台上下文基础能力已经出现，但口径还没完全收口

- [platform.rs](file:///Users/FYP/Documents/WorkSpace/CheersAI/subproducts/CheersAI-Vault/src-tauri/src/commands/platform.rs) 已经提供 `get_platform_context`，能返回 `os`、默认目录、缓存目录、临时目录，以及 `pinStorageMode` / `ocrStrategy` / `ollamaStrategy`。
- [lib.rs](file:///Users/FYP/Documents/WorkSpace/CheersAI/subproducts/CheersAI-Vault/src-tauri/src/lib.rs) 和 [tauri.ts](file:///Users/FYP/Documents/WorkSpace/CheersAI/subproducts/CheersAI-Vault/src/lib/tauri.ts) 也已完成命令注册与前端调用封装。
- [App.tsx](file:///Users/FYP/Documents/WorkSpace/CheersAI/subproducts/CheersAI-Vault/src/App.tsx) 已在启动时加载平台上下文并调用 `setPlatformContext()`。
- 但 [path.ts](file:///Users/FYP/Documents/WorkSpace/CheersAI/subproducts/CheersAI-Vault/src/lib/path.ts) 仍保留 `navigator.userAgent` fallback、`%APPDATA%`、`~/Library/...` 这类展示占位口径，说明“平台真值”和“格式化展示”还没有彻底分层。

### 2. PIN 安全存储已有 macOS 雏形，但还需要统一产品口径

- [dpapi.rs](file:///Users/FYP/Documents/WorkSpace/CheersAI/subproducts/CheersAI-Vault/src-tauri/src/core/dpapi.rs) 已新增 macOS Keychain 方案，使用 `security add-generic-password` / `find-generic-password -w` / `delete-generic-password`。
- [SandboxManager.tsx](file:///Users/FYP/Documents/WorkSpace/CheersAI/subproducts/CheersAI-Vault/src/pages/SandboxManager.tsx) 也已经能根据 `pinStorageMode` 切换 `macOS Keychain` / `Windows DPAPI` / `兼容模式` 文案。
- 但该页仍同时依赖 [path.ts](file:///Users/FYP/Documents/WorkSpace/CheersAI/subproducts/CheersAI-Vault/src/lib/path.ts) 的平台判断和独立平台上下文；后续实现需要收敛来源，避免 UI 显示与后端真实行为脱节。

### 3. OCR 的跨平台链路已开始重构，但仍需验证和补齐边界

- [ocr.rs](file:///Users/FYP/Documents/WorkSpace/CheersAI/subproducts/CheersAI-Vault/src-tauri/src/commands/ocr.rs) 已区分 Windows 与非 Windows 路径，macOS 方向改成 `python3 -m venv` + PyMuPDF。
- [file_parser.rs](file:///Users/FYP/Documents/WorkSpace/CheersAI/subproducts/CheersAI-Vault/src-tauri/src/core/file_parser.rs) 计划应复用 OCR 运行时解析，但仍需在实施时确认所有调用点都已切到统一解析逻辑。
- [pdf_ocr.py](file:///Users/FYP/Documents/WorkSpace/CheersAI/subproducts/CheersAI-Vault/src-tauri/scripts/pdf_ocr.py) 当前定位已经更接近“PDF 文本提取轻量运行时”，不再是纯 Windows 嵌入式方案。
- 风险在于：当前能力更偏“PDF 文本提取”而非“完整图片 OCR”，因此页面说明、失败文案和安装提示必须同步调整，不能继续让用户误解功能边界。

### 4. Ollama 已有 macOS 路径搜索，但状态模型仍需做成闭环

- [ai_model.rs](file:///Users/FYP/Documents/WorkSpace/CheersAI/subproducts/CheersAI-Vault/src-tauri/src/commands/ai_model.rs) 已经加入 macOS 路径候选、`which ollama`、`/Applications/Ollama.app/...` 搜索，以及服务探测入口。
- [tauri.ts](file:///Users/FYP/Documents/WorkSpace/CheersAI/subproducts/CheersAI-Vault/src/lib/tauri.ts) 已暴露 `checkOllamaBinaryInstalled()` 与 `checkOllamaServiceRunning()`。
- [EnhancedServices.tsx](file:///Users/FYP/Documents/WorkSpace/CheersAI/subproducts/CheersAI-Vault/src/pages/EnhancedServices.tsx) 已拆出 `ollamaInstalled`、`ollamaRunning`、`aiModel` 三层状态。
- 但仍需在实施阶段重点确认：`download_ollama()`、`start_ollama_service()`、`check_ai_model_installed()` 这三条链路是否完全使用同一套路径解析和状态口径，避免“页面显示可用，命令实际失败”。

### 5. 体验层已经有跨平台方向，但还存在未完全收口的提示与入口

- [EnhancedServices.tsx](file:///Users/FYP/Documents/WorkSpace/CheersAI/subproducts/CheersAI-Vault/src/pages/EnhancedServices.tsx) 已经把原生 `confirm()` 改成 `Dialog` 状态模型，也初步接入平台感知说明。
- [OcrDownloadDialog.tsx](file:///Users/FYP/Documents/WorkSpace/CheersAI/subproducts/CheersAI-Vault/src/components/file/OcrDownloadDialog.tsx) 已改成“Windows 嵌入式 Python / macOS venv / 当前为轻量文本提取”的说明。
- [FileBayConfigManager.tsx](file:///Users/FYP/Documents/WorkSpace/CheersAI/subproducts/CheersAI-Vault/src/components/filebay/FileBayConfigManager.tsx) 已把 `localhost` 下载页替换为 [cloud.ts](file:///Users/FYP/Documents/WorkSpace/CheersAI/subproducts/CheersAI-Vault/src/lib/cloud.ts) 中的 `CLOUD_APP_URL`。
- 但仍需要在实施中统一：
  - 页面成功/失败提示是否都体现平台差异。
  - placeholder 是否都来自真实平台目录。
  - FileBay 入口文案是否足够说明“去 Desktop 在线工作区下载配置，而不是本地固定地址”。

### 6. 仍缺少一轮基于当前代码的完整验证闭环

- 仓库内当前已存在多处 macOS 适配代码，但是否能完整通过前端构建、Rust 编译、命令注册、页面类型检查，还没有在本方案里落成最终验收闭环。
- 因此执行阶段必须把“补实现”与“统一验证”绑定，不能只改页面文案而不做构建检查。

## Assumptions & Decisions

- 本次整改目标是“让 macOS 本地开发和测试可稳定运行、提示可信、核心链路可走通”，不是一次性完成 Apple 签名、公证或正式发布体系。
- Windows 现有可用流程保持兼容，不做推翻式重构；优先抽象平台差异，而不是重写整套功能。
- `get_platform_context` 作为平台真值来源继续保留并扩展；[path.ts](file:///Users/FYP/Documents/WorkSpace/CheersAI/subproducts/CheersAI-Vault/src/lib/path.ts) 在执行阶段降级为“展示与路径辅助工具”，不再承担系统事实来源职责。
- macOS PIN 存储采用 Keychain 方案，继续沿用 `security` CLI，不引入额外原生依赖。
- macOS OCR 采用“系统 Python + venv + PyMuPDF”的轻量运行时策略，目标是先保证 PDF 文本提取稳定可用；图片型 OCR 不在本轮承诺范围内，但必须清晰提示。
- Ollama 的产品口径拆成三层：
  - 是否安装二进制。
  - 服务是否在运行。
  - 模型是否已拉取。
- 执行阶段会先兼容并吸收当前仓库里的已有适配代码，再补齐缺口，不做“先回退后重写”的处理。

## Proposed Changes

### A. 收敛平台上下文与路径展示口径

#### 目标

让所有展示层路径、平台名称和能力说明都来自后端平台上下文，而不是前端猜测。

#### 涉及文件

- [platform.rs](file:///Users/FYP/Documents/WorkSpace/CheersAI/subproducts/CheersAI-Vault/src-tauri/src/commands/platform.rs)
- [tauri.ts](file:///Users/FYP/Documents/WorkSpace/CheersAI/subproducts/CheersAI-Vault/src/lib/tauri.ts)
- [types/commands.ts](file:///Users/FYP/Documents/WorkSpace/CheersAI/subproducts/CheersAI-Vault/src/types/commands.ts)
- [path.ts](file:///Users/FYP/Documents/WorkSpace/CheersAI/subproducts/CheersAI-Vault/src/lib/path.ts)
- [App.tsx](file:///Users/FYP/Documents/WorkSpace/CheersAI/subproducts/CheersAI-Vault/src/App.tsx)
- [SandboxManager.tsx](file:///Users/FYP/Documents/WorkSpace/CheersAI/subproducts/CheersAI-Vault/src/pages/SandboxManager.tsx)
- [EnhancedServices.tsx](file:///Users/FYP/Documents/WorkSpace/CheersAI/subproducts/CheersAI-Vault/src/pages/EnhancedServices.tsx)

#### 修改内容

- 明确 `PlatformContext` 为前端唯一平台事实来源。
- 收敛 [path.ts](file:///Users/FYP/Documents/WorkSpace/CheersAI/subproducts/CheersAI-Vault/src/lib/path.ts)：
  - 保留 `normalizePath()`、`joinPath()`、`validatePath()`、`getDisplayPath()` 等纯工具能力。
  - 弱化 `navigator.userAgent` 分支，只作为浏览器退路，不再作为桌面端业务判断依据。
- 页面中所有默认目录 placeholder、帮助说明、平台名称展示都优先使用 `platformContext`。

#### 价值

- 避免 macOS 页面继续出现 `C:\...`、`%APPDATA%` 或与实际运行目录不一致的文案。

### B. 完成 PIN 安全存储闭环与沙箱页口径统一

#### 目标

让 macOS PIN 真正依赖 Keychain，并让沙箱页面说明和真实实现完全对齐。

#### 涉及文件

- [dpapi.rs](file:///Users/FYP/Documents/WorkSpace/CheersAI/subproducts/CheersAI-Vault/src-tauri/src/core/dpapi.rs)
- [sandbox.rs](file:///Users/FYP/Documents/WorkSpace/CheersAI/subproducts/CheersAI-Vault/src-tauri/src/commands/sandbox.rs)
- [SandboxManager.tsx](file:///Users/FYP/Documents/WorkSpace/CheersAI/subproducts/CheersAI-Vault/src/pages/SandboxManager.tsx)

#### 修改内容

- 保持 Windows `DPAPI`、macOS `Keychain`、Linux `fallback` 三层实现不变，但补齐统一错误说明。
- 在沙箱页中统一使用 `platformContext.pinStorageMode` 生成：
  - 存储方式说明。
  - 设置成功提示。
  - 失败后的建议动作。
- 核对沙箱锁定/解锁与文件隐藏逻辑在 macOS 下是否使用正确系统命令，并让页面提示与其一致。

#### 价值

- 把“表面可用但安全口径不对”的问题修正为“实现与产品承诺一致”。

### C. 补齐 OCR 在 macOS 下的可检测、可安装、可解释链路

#### 目标

让 OCR 在 macOS 下从“部分重构”升级为“运行时有统一解析、安装失败有明确提示、页面说明不误导”。

#### 涉及文件

- [ocr.rs](file:///Users/FYP/Documents/WorkSpace/CheersAI/subproducts/CheersAI-Vault/src-tauri/src/commands/ocr.rs)
- [file_parser.rs](file:///Users/FYP/Documents/WorkSpace/CheersAI/subproducts/CheersAI-Vault/src-tauri/src/core/file_parser.rs)
- [pdf_ocr.py](file:///Users/FYP/Documents/WorkSpace/CheersAI/subproducts/CheersAI-Vault/src-tauri/scripts/pdf_ocr.py)
- [EnhancedServices.tsx](file:///Users/FYP/Documents/WorkSpace/CheersAI/subproducts/CheersAI-Vault/src/pages/EnhancedServices.tsx)
- [OcrDownloadDialog.tsx](file:///Users/FYP/Documents/WorkSpace/CheersAI/subproducts/CheersAI-Vault/src/components/file/OcrDownloadDialog.tsx)

#### 修改内容

- 统一 OCR 运行时解析接口，让安装检查、执行解析和页面提示使用同一套结果。
- macOS 路径遵循：
  - 优先 `app_data_dir/ocr-package/venv/bin/python3`。
  - 其次系统 `python3`。
  - 没有系统 Python 时返回明确指导。
- 保持当前轻量运行时能力边界，明确写成“PDF 文本提取优先，不承诺图片型 OCR”。
- 把下载/安装/检查失败文案改成可执行建议，例如：
  - 安装 Python。
  - 重新创建 venv。
  - 重新扫描服务。

#### 价值

- 这是 macOS 下最容易“代码编译过了但用户仍然不能用”的一块，必须从执行链路和提示链路一起收口。

### D. 完成 Ollama 二进制、服务、模型三层状态闭环

#### 目标

让 AI 服务页在 macOS 下准确区分“未安装”、“已安装但服务未启动”、“服务已运行但模型未安装”、“全部可用”。

#### 涉及文件

- [ai_model.rs](file:///Users/FYP/Documents/WorkSpace/CheersAI/subproducts/CheersAI-Vault/src-tauri/src/commands/ai_model.rs)
- [ner.rs](file:///Users/FYP/Documents/WorkSpace/CheersAI/subproducts/CheersAI-Vault/src-tauri/src/core/ner.rs)
- [tauri.ts](file:///Users/FYP/Documents/WorkSpace/CheersAI/subproducts/CheersAI-Vault/src/lib/tauri.ts)
- [EnhancedServices.tsx](file:///Users/FYP/Documents/WorkSpace/CheersAI/subproducts/CheersAI-Vault/src/pages/EnhancedServices.tsx)

#### 修改内容

- 统一 Ollama 路径解析函数，避免命令层和 NER 层各维护一份。
- 保证 `check_ollama_binary_installed()`、`check_ollama_service_running()`、`check_ai_model_installed()` 的判定层级清晰，不混用。
- macOS 启动优先 `open -a Ollama`，失败后才 fallback 到 CLI。
- 服务页根据三层状态给出不同操作建议和按钮反馈。
- 清理残留 Windows 指向型错误提示，避免继续出现“下载 Windows 版本”或 `C:\Ollama` 这类信息。

#### 价值

- 这是用户最容易感知的差异点，也是当前“看似能检测，实际上状态并不准确”的主要来源。

### E. 统一体验层交互与 Cloud/FileBay 入口口径

#### 目标

让页面交互不再像“Windows 逻辑平移到 macOS”，并把 FileBay 配置获取路径说清楚。

#### 涉及文件

- [EnhancedServices.tsx](file:///Users/FYP/Documents/WorkSpace/CheersAI/subproducts/CheersAI-Vault/src/pages/EnhancedServices.tsx)
- [SandboxManager.tsx](file:///Users/FYP/Documents/WorkSpace/CheersAI/subproducts/CheersAI-Vault/src/pages/SandboxManager.tsx)
- [OcrDownloadDialog.tsx](file:///Users/FYP/Documents/WorkSpace/CheersAI/subproducts/CheersAI-Vault/src/components/file/OcrDownloadDialog.tsx)
- [FileBayConfigManager.tsx](file:///Users/FYP/Documents/WorkSpace/CheersAI/subproducts/CheersAI-Vault/src/components/filebay/FileBayConfigManager.tsx)
- [cloud.ts](file:///Users/FYP/Documents/WorkSpace/CheersAI/subproducts/CheersAI-Vault/src/lib/cloud.ts)

#### 修改内容

- 全量替换仍残留的浏览器默认确认交互为项目统一 `Dialog`。
- 统一成功/错误/说明文案风格，所有平台相关提示都带“下一步建议”。
- FileBay 配置页明确引导：
  - 去 Desktop 在线工作区获取配置。
  - 本地导入已有 JSON。
  - 刷新检测当前落盘状态。
- 如存在未配置或打开失败，返回真实 Cloud 入口说明，而不是构造无效本地地址。

#### 价值

- 这些问题不一定阻塞编译，但决定了 macOS 版本是否“像一个可信的桌面产品”。

### F. 建立执行阶段的统一验证闭环

#### 目标

确保本轮整改不是“零散改动”，而是有清晰验收标准的完整优化。

#### 涉及文件

- [EnhancedServices.tsx](file:///Users/FYP/Documents/WorkSpace/CheersAI/subproducts/CheersAI-Vault/src/pages/EnhancedServices.tsx)
- [SandboxManager.tsx](file:///Users/FYP/Documents/WorkSpace/CheersAI/subproducts/CheersAI-Vault/src/pages/SandboxManager.tsx)
- [ai_model.rs](file:///Users/FYP/Documents/WorkSpace/CheersAI/subproducts/CheersAI-Vault/src-tauri/src/commands/ai_model.rs)
- [ocr.rs](file:///Users/FYP/Documents/WorkSpace/CheersAI/subproducts/CheersAI-Vault/src-tauri/src/commands/ocr.rs)
- [dpapi.rs](file:///Users/FYP/Documents/WorkSpace/CheersAI/subproducts/CheersAI-Vault/src-tauri/src/core/dpapi.rs)

#### 修改内容

- 执行阶段完成代码后，必须做一轮前端和 Rust 双侧检查。
- 对关键场景做最小但有效的功能回归：
  - PIN 存储。
  - OCR 检测与安装。
  - Ollama 状态探测与启动提示。
  - FileBay 入口打开与提示。

#### 价值

- 让本次整改从“看起来改了很多”变成“行为上真的可靠”。

## Implementation Order

### P0

1. 收敛平台上下文与路径工具职责。
2. 完成 PIN 存储与沙箱页口径统一。
3. 完成 OCR 运行时解析、页面说明和失败提示闭环。
4. 完成 Ollama 三层状态闭环。

### P1

1. 收口服务页、沙箱页、OCR 弹窗、FileBay 页的跨平台文案与确认交互。
2. 清理残留的 Windows 专属路径、说明和错误信息。

### P2

1. 做构建检查与关键链路回归。
2. 根据验证结果补齐小范围修正，不扩展到签名、公证或额外发布流程。

## Verification Steps

### 代码级验证

1. 前端类型与构建检查通过。
2. Rust `cargo check` 通过。
3. 新增或调整的命令都已注册，前端调用签名一致。
4. 最近修改文件无新增明显诊断错误。

### macOS 功能验证

1. 启动应用后，沙箱页显示的默认目录、平台名、PIN 存储方式与 macOS 一致。
2. 设置 PIN 后可成功验证，清除 PIN 后状态同步变化。
3. OCR 页面在以下场景下提示正确：
   - 已有 venv。
   - 仅有系统 `python3`。
   - 没有 `python3`。
4. AI 服务页在以下场景下提示正确：
   - 未安装 Ollama。
   - 已安装 Ollama 但服务未运行。
   - 服务运行但模型未安装。
   - 服务运行且模型已安装。
5. FileBay 配置页能正确打开在线工作区入口，并在未检测到配置时给出可信提示。

### Windows 回归验证

1. Windows PIN 仍继续使用 DPAPI。
2. Windows OCR 现有安装链路不被破坏。
3. 现有 Vault 主流程与嵌入 Desktop 入口不受影响。
