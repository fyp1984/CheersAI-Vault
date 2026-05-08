---
name: "vault-dmg-release"
description: "Packages CheersAI Vault into a macOS DMG. Invoke when refreshing the Vault Desktop installer, validating DMG output, or preparing a release candidate build."
---

# Vault DMG Release

## 用途

用于在 [CheersAI-Vault](file:///Users/FYP/Documents/WorkSpace/CheersAI/subproducts/CheersAI-Vault) 仓库内执行标准化的 macOS DMG 打包流程，适用于内部验证版、候选发布版和最终测试版。

## 何时调用

- 用户要求重新打包 Vault 客户端 DMG
- 用户要求基于当前本地工作树刷新一版安装包
- 用户要求验证图标、主标题区、Desktop 嵌入工作区布局
- 用户要求记录产物 SHA-256、路径、大小和验收结果

## 当前默认基线

- 仓库路径：`/Users/FYP/Documents/WorkSpace/CheersAI/subproducts/CheersAI-Vault`
- 默认分支：`feature`
- 默认产品名：`CheersAI Desktop`
- Desktop 菜单地址：`https://uat-desktop.cheersai.cloud/?source=vault-shell`
- 当前 UI 规范：
  - Vault 顶部主头部高度：`120px`
  - Desktop 子 WebView 顶部偏移：`120px`
  - Desktop 左上品牌区默认隐藏
  - 顶部右侧显示 `NETWORK 在线/离线`

## 正式包口径

- 若未提供 Apple 签名证书、公证配置、entitlements，则“正式 DMG”按当前仓库能力解释为：
  - 完整功能的最终候选包
  - 可安装、可启动、可用于内部测试
  - 默认仍为 unsigned DMG
- 当前 portable DMG 除了解决 Gatekeeper/签名兼容问题，还必须承载最新 macOS `/cloud` 安全策略：
  - 默认进入嵌入式工作区
  - 若内嵌子 WebView 创建失败，则主窗口不崩溃，并停留在统一 Cloud 回退页
  - 统一 Cloud 回退页以 `src/pages/CheersAICloudBrowser.tsx` 为唯一验收基线
  - 统一回退页需提供“重新尝试嵌入式打开”“在独立窗口打开”“在系统浏览器打开”三个入口
- 若用户明确要求对外分发正式版，则必须额外具备：
  - Apple Developer 证书
  - 签名配置
  - 公证链路

## 打包前检查

1. 确认仓库存在且是 Git 工作副本。
2. 记录当前分支、最新 commit、工作区脏改动。
3. 确认当前本机具备：
   - `node`
   - `pnpm`
   - `rustc`
   - `cargo`
4. 检查 `xcode-select -p`。
5. 若 `xcodebuild -version` 不可用，仍允许继续构建 unsigned DMG，但需在结果里注明风险。
6. 确认以下关键文件未被错误回退：
   - `src/App.tsx`
   - `src/components/layout/MainLayout.tsx`
   - `src/pages/CheersAICloudBrowser.tsx`
   - `src-tauri/src/commands/webview.rs`

## 标准执行步骤

### 1. 清理旧产物

```bash
rm -f src-tauri/target/release/bundle/dmg/CheersAI\ Desktop_0.1.0_aarch64.dmg
```

### 2. 前端构建

```bash
pnpm build
```

### 3. Rust 校验

```bash
cd src-tauri
cargo check
cd ..
```

### 4. 生成可分发 DMG

```bash
pnpm build:dmg:portable
```

说明：

- 该脚本会先调用 `pnpm tauri build --bundles dmg` 生成原始 DMG
- 然后重新挂载原始 DMG，复制其中的 `.app`
- 清理扩展属性并执行 `codesign --force --deep --sign -`
- 最后重新封装为 `*_portable.dmg`
- 这样可以修复 Tauri 默认产物在部分设备上出现的 “已损坏，无法打开” 问题
- 同时要求测试口径覆盖 `/cloud` 的“默认内嵌 + 统一回退页”验收，而不能只看 DMG 是否可打开

## 产物位置

- 可分发 DMG：
  - `dist/CheersAI Desktop_0.1.0_aarch64_portable.dmg`
- 原始 DMG：
  - `src-tauri/target/release/bundle/dmg/CheersAI Desktop_0.1.0_aarch64.dmg`
- 中间产物：
  - `src-tauri/target/release/bundle/macos/`

## 验收清单

### 结构验收

1. DMG 文件成功生成
2. 文件大小正常
3. SHA-256 已记录
4. 挂载后卷内包含：
   - `CheersAI Desktop.app`
   - `Applications`

### UI 验收

1. Vault 顶部左侧文案完整显示：
   - `Desktop 在线工作区`
   - `把敏感数据留在本地，让AI能力触手可及。`
2. 顶部右侧 `NETWORK 在线/离线` 完整显示
3. Desktop 嵌入区不压住头部，不留下异常缝隙
4. Desktop 左上 logo/slogan 默认隐藏
5. 进入 `CheersAI` 菜单后，`/cloud` 默认先尝试进入内嵌工作区，而不是直接进入旧的独立页流程
6. 若内嵌创建失败，主窗口不闪退、不白屏，并停留在 `CheersAICloudBrowser` 的统一回退页
7. 统一回退页中可见“重新尝试嵌入式打开”“在独立窗口打开”“在系统浏览器打开”入口

### 功能验收

1. 首次启动主应用不闪退，主窗口可稳定进入
2. Desktop SSO 登录可进入工作区
3. `/apps`、`/datasets`、`/chat`、`/audit-logs` 可访问
4. 审计日志页面可显示“日志列表”
5. 若 `/cloud` 内嵌失败，仍可通过统一回退页入口继续访问工作区
6. 高优先级动作至少可记录：
   - 登录
   - 访问应用列表/详情
   - 访问知识库列表/详情
   - 聊天调用

### 测试说明补充

1. 若本次仅修改打包脚本或文档，至少执行 `bash ./scripts/build-macos-portable-dmg.sh --help`，确认帮助文案已覆盖“默认内嵌 + 统一回退页”口径。
2. 若同时存在 `/cloud` 或 `webview` 相关代码改动，建议实际构建一版 portable DMG，并在另一台 Mac 或干净用户环境完成以下人工验证：
   - 首次启动是否闪退
   - `/cloud` 是否默认尝试内嵌
   - 内嵌失败时是否停留在统一回退页
   - 统一回退页是否同时提供重试、独立窗口、系统浏览器三个入口
3. 若未做实际构建，需要在结果中明确说明“本次仅完成脚本/文档口径调整，未产出新的 DMG 实物验收结果”。

## 产物记录模板

- 分支：
- commit：
- 工作区状态：
- 构建命令：
- DMG 路径：
- 文件大小：
- SHA-256：
- 挂载结果：
- 首次启动是否闪退：
- `/cloud` 是否默认尝试内嵌：
- 内嵌失败时是否进入统一回退页：
- 回退页入口是否完整：
- UI 验收结果：
- 风险说明：

## 常见问题

### 1. Desktop 登录后白屏

- 先检查 `src-tauri/src/commands/webview.rs` 中品牌隐藏脚本是否误伤页面结构
- 再检查 Desktop URL 是否仍包含 `source=vault-shell`

### 2. 顶部文案或 NETWORK 状态被裁切

- 先确认 `MainLayout.tsx` 的头部高度
- 再确认 `CONTENT_HEADER_HEIGHT` 与之完全一致

### 3. DMG 构建失败

- 先检查 `pnpm build`
- 再检查 `cargo check`
- 再检查 `xcode-select -p` 与 `xcodebuild -version`

### 4. 其他设备提示“已损坏，移到废纸篓”

- 优先检查原始 `.app` 是否通过 `codesign --verify --deep --strict`
- 若原始 Tauri DMG 内的 `.app` 为无效 ad-hoc 签名，必须改走 `pnpm build:dmg:portable`
- 没有 Apple Developer 证书时，`*_portable.dmg` 仍属于 unsigned 包，但通常会从“已损坏”收敛为可安装或提示“无法验证开发者”

### 5. DMG 能打开但 `/cloud` 没有默认内嵌或没有进入统一回退页

- 先确认当前构建是否包含最新 `src/App.tsx`、`src/pages/CheersAICloudBrowser.tsx` 与 `src-tauri/src/commands/webview.rs`
- 再核对验收记录是否真的覆盖了“默认内嵌 + 统一回退页”，而不是只验证了 DMG 挂载与安装
- 若主窗口直接白屏、闪退或只能停留在不可操作页面，应判定本次 DMG 验收不通过

## 输出要求

执行结束后必须汇总：

- 是否删除旧 DMG
- 是否构建成功
- 新 DMG 的完整路径
- 文件大小
- SHA-256
- 当前是否为 unsigned 包
- 是否覆盖“默认内嵌 + 统一回退页”验收
- 是否建议继续人工安装验证
