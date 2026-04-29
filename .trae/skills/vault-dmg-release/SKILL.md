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
5. 进入 `CheersAI` 菜单后，Desktop 工作区不白屏

### 功能验收

1. Desktop SSO 登录可进入工作区
2. `/apps`、`/datasets`、`/chat`、`/audit-logs` 可访问
3. 审计日志页面可显示“日志列表”
4. 高优先级动作至少可记录：
   - 登录
   - 访问应用列表/详情
   - 访问知识库列表/详情
   - 聊天调用

## 产物记录模板

- 分支：
- commit：
- 工作区状态：
- 构建命令：
- DMG 路径：
- 文件大小：
- SHA-256：
- 挂载结果：
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

## 输出要求

执行结束后必须汇总：

- 是否删除旧 DMG
- 是否构建成功
- 新 DMG 的完整路径
- 文件大小
- SHA-256
- 当前是否为 unsigned 包
- 是否建议继续人工安装验证
