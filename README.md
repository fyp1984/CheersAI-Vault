# CheersAI Vault

[![Release](https://img.shields.io/badge/release-0.1.42-blue?logo=semver)](https://github.com/fyp1984/CheersAI-Vault/releases)
[![License](https://img.shields.io/badge/license-Apache--2.0-green.svg)](./LICENSE)
[![Excel P0](https://img.shields.io/badge/Excel%20增强-P0-green?logo=microsoftexcel)](#excel-增强脱敏功能)
[![Platform](https://img.shields.io/badge/platform-macOS%20%7C%20Linux%20%7C%20Docker-lightgrey)](#快速开始)

Language:

- 中文说明： [`docs/USER_GUIDE.md`](./docs/USER_GUIDE.md)
- English manual: [`docs/USER_GUIDE_EN.md`](./docs/USER_GUIDE_EN.md)

CheersAI脱敏沙箱是一款面向企业与个人用户的本地敏感文件脱敏开源软件。它帮助用户在对外发送文件、共享资料，或把内容交给 AI 智能体与大模型之前，先把姓名、电话、证件号、地址、合同要点、客户信息等敏感内容处理干净。等文件修改完成、审阅结束或 AI 交互流程结束后，用户还可以依靠本地留存的规则，一键把需要恢复的内容自动还原回来。

CheersAI Desensitization Sandbox is an open source local file-masking application for enterprise teams and individual users. It helps remove sensitive data before files are shared externally or submitted to AI agents and large language models, and it also supports one-click restore based on locally retained mapping rules after editing, review, or AI-assisted processing is complete.

当前仓库同时包含三类能力：

- `CheersAI Vault`： 基于 Tauri 的桌面端
- `CheersAI Vault Pro`： 基于浏览器 + Runtime 的企业内网入口
- 共享核心： 脱敏规则、文件解析、映射编解码、OCR 组件封装、FileBay 集成等

本仓库的目标不是做一个普通文件工具，而是提供一套真正能落到日常工作里的“先脱敏、再分享、可恢复”的安全处理能力。

## Excel 增强脱敏功能（P0 客户交付版 · v0.1.42）

近期通过 PR #30 合并的 Excel 增强脱敏能力正式落地到 P0 客户交付，覆盖“结构化表格逐格解析 → 按列/按格配置规则 → 生成加密映射 → 输出报告与脱敏工作簿 → 双路径反脱敏恢复”的完整业务流程，并同时在桌面端（Tauri）、企业浏览器端（Runtime API）与 FileBay 集成三条链路复用同一套核心引擎。

### 功能能力矩阵

| 能力 | 说明 | 支持情况 | 关键模块 |
| --- | --- | --- | --- |
| 工作簿结构解析 | 读取 Sheet 列表、表头行、每列 sample（列优先）、数据量级提示 | ✅ | [excel_masking.rs:excel_parse_structure](file:///Users/sevensimle/Documents/WorkSpace/CheersAI/CheersAI-Vault/src-tauri/src/commands/excel_masking.rs#L412-L417) · [table_reader.rs](file:///Users/sevensimle/Documents/WorkSpace/CheersAI/CheersAI-Vault/src-tauri/crates/excel-style-core/src/table_reader.rs) |
| 按列配置掩码策略 | FULL_MASK / PHONE_MID4 / IDCARD_MID10 / BANKCARD_LAST4 / EMAIL_USER_MASK / DEFAULT_VALUE / CLEAR_COL 等 8 种策略，可独立配置 replacement | ✅ | [ColumnMaskRule](file:///Users/sevensimle/Documents/WorkSpace/CheersAI/CheersAI-Vault/src/types/commands.ts#L3-L100) · [ExcelMaskingDialog.tsx:STRATEGY_LABELS](file:///Users/sevensimle/Documents/WorkSpace/CheersAI/CheersAI-Vault/src/components/file/ExcelMaskingDialog.tsx#L91-L110) |
| 单元格级别覆盖规则 | 除列级规则外，可额外指定某几行某几格使用独立策略，用于处理“表头下方备注/行尾汇总”等异常情况 | ✅ | [CellOverrideRule](file:///Users/sevensimle/Documents/WorkSpace/CheersAI/CheersAI-Vault/src/types/commands.ts#L3-L100) · [ExcelMaskingDialog.tsx](file:///Users/sevensimle/Documents/WorkSpace/CheersAI/CheersAI-Vault/src/components/file/ExcelMaskingDialog.tsx#L267-L272) |
| 加密源留存双模式 | `SANDBOX_REUSED`（默认，沙箱内复用）与 `SECONDARY_PASSPHRASE`（ecmap 与加密源各用一套口令，适合跨用户安全分享） | ✅ | [EncSourceKeyMode](file:///Users/sevensimle/Documents/WorkSpace/CheersAI/CheersAI-Vault/src/types/commands.ts#L3-L100) · [crypto.rs encrypt_ecmap](file:///Users/sevensimle/Documents/WorkSpace/CheersAI/CheersAI-Vault/src-tauri/src/core/crypto.rs) |
| `.ecmap` 加密映射文件输出 | 以 AES-GCM + 口令派生密钥封装 EcmapDocumentV1（header + cell 映射），文件名后缀 `.ecmap`；可与脱敏工作簿单独外发 | ✅ | [excel_masking.rs:excel_apply_masking](file:///Users/sevensimle/Documents/WorkSpace/CheersAI/CheersAI-Vault/src-tauri/src/commands/excel_masking.rs#L605-L978) · `ecmap_header_declares_source_retained()` L184 |
| 双路径反脱敏恢复 | 路径 A：`.ecmap + 加密源文件 + 口令`（自动恢复）；路径 B：`.ecmap + 用户原始 xlsx + 口令`（SHA-256 必须与写入时 header 完全匹配，不支持凭空恢复） | ✅ | [excel_masking.rs:excel_restore_from_ecmap](file:///Users/sevensimle/Documents/WorkSpace/CheersAI/CheersAI-Vault/src-tauri/src/commands/excel_masking.rs#L1056-L1104) · `RETAIN_MESSAGES` 三路径安全文案 L72-L75 |
| 安全口令强度与 cell 范围上限 | 最小口令长度、校验口令不一致、Sheet/行/列数量上限与单元格覆盖数上限，避免超大型工作簿导致 DoS | ✅ | [commit 6f5efe4](https://github.com/fyp1984/CheersAI-Vault/commit/6f5efe4) · [ExcelMaskingDialog.test.tsx](file:///Users/sevensimle/Documents/WorkSpace/CheersAI/CheersAI-Vault/src/components/file/ExcelMaskingDialog.test.tsx) |
| 输出 / 恢复一致性闭环 | 报告 hits/conflicts/covered_cells 与实际输出 workbook、`.ecmap` entries 三者逐条对齐，杜绝历史出现的“报告统计为 0 但文件已被改”问题 | ✅ | [commit 622b8d7](https://github.com/fyp1984/CheersAI-Vault/commit/622b8d7) · [RewriteOutcome](file:///Users/sevensimle/Documents/WorkSpace/CheersAI/CheersAI-Vault/src-tauri/crates/excel-style-core/src/lib.rs#L47-L54) |
| 样式 / 公式保留 | 基于 zip + sharedStrings 的双后端实现（calamine 读 + rust_xlsxwriter 写 与 zip/xml 流解析），尽可能保留字体、填充、列宽、合并单元格与样式，仅替换被命中单元格的 value，公式不做二次改写 | ✅ | [excel-style-core/src/lib.rs](file:///Users/sevensimle/Documents/WorkSpace/CheersAI/CheersAI-Vault/src-tauri/crates/excel-style-core/src/lib.rs) · [excel.rs](file:///Users/sevensimle/Documents/WorkSpace/CheersAI/CheersAI-Vault/apps/vault-runtime-api/src/excel.rs)（Runtime 端） |
| Runtime 浏览器端 end-to-end | 企业浏览器端通过 Vault Pro Web 上传 Excel，走与桌面端相同的 engine-core + excel-style-core 核心路径，错误文案全部中文安全化，不得出现英文 HTTP 字面量 | ✅ | [excelClient.ts](file:///Users/sevensimle/Documents/WorkSpace/CheersAI/CheersAI-Vault/src/lib/runtime/excelClient.ts) · [errorClassification.ts](file:///Users/sevensimle/Documents/WorkSpace/CheersAI/CheersAI-Vault/src/lib/runtime/errorClassification.ts) |
| 桌面端 FileUnmask 双路径提示 | 上传 ecmap 后自动判定是否可走路径 A；若用户未勾选加密留存，UI 会醒目提示并引导使用“路径 B 用户原件”还原 | ✅ | [FileUnmaskDesktop.tsx](file:///Users/sevensimle/Documents/WorkSpace/CheersAI/CheersAI-Vault/src/pages/FileUnmaskDesktop.tsx) · [FileUnmaskBrowser.tsx](file:///Users/sevensimle/Documents/WorkSpace/CheersAI/CheersAI-Vault/src/pages/FileUnmaskBrowser.tsx) |

### 关键测试覆盖（共 11+ 份单测/合同测试）

- [excelMaskingContract.test.ts](file:///Users/sevensimle/Documents/WorkSpace/CheersAI/CheersAI-Vault/src/lib/excelMaskingContract.test.ts)：桌面端错误分类与合同断言（421+ 行）
- [ExcelMaskingDialog.test.tsx](file:///Users/sevensimle/Documents/WorkSpace/CheersAI/CheersAI-Vault/src/components/file/ExcelMaskingDialog.test.tsx)：UI 级交互与默认值
- [tauriExcelRestoreContract.test.ts](file:///Users/sevensimle/Documents/WorkSpace/CheersAI/CheersAI-Vault/src/lib/tauriExcelRestoreContract.test.ts)：双路径恢复合约
- [excelRestoreContract.test.ts](file:///Users/sevensimle/Documents/WorkSpace/CheersAI/CheersAI-Vault/src/lib/runtime/excelRestoreContract.test.ts)：Runtime 恢复路径合约
- [excelArtifactAvailability.test.ts](file:///Users/sevensimle/Documents/WorkSpace/CheersAI/CheersAI-Vault/src/lib/runtime/excelArtifactAvailability.test.ts)：生成物可用性检查
- [excel-style-core/tests/integration.rs](file:///Users/sevensimle/Documents/WorkSpace/CheersAI/CheersAI-Vault/src-tauri/crates/excel-style-core/tests/integration.rs)：Rust 端 rewrite engine 集成测试
- [excel-style-core/tests/table_reader.rs](file:///Users/sevensimle/Documents/WorkSpace/CheersAI/CheersAI-Vault/src-tauri/crates/excel-style-core/tests/table_reader.rs)：Rust 端表解析集成测试

### 与原有“通用 Excel 列映射”的主要区别

- **更细的控制粒度**：通用 Excel 流程只按列推断，增强版引入 `CellOverrideRule` 支持单元格级 override
- **可恢复性**：通用流程只保留预览，增强版使用 `.ecmap` + 双路径恢复，支持交付给客户后“先脱敏 → 客户改完 → 回来自动反脱敏”的完整链路
- **客户级安全性**：加密源留存可选 + 独立口令 + ecmap header source_retained 声明 3 层互锁，避免内部分享时误将加密源外带
- 合同化测试：单测与合同测试直接约束 UI 文案与 Rust 命令返回值，不允许英文 HTTP 字面量透出

---

## 桌面端 DMG 打包 Skill 调用方式（程序安装包版本管理沿用既有的 version-manager.js 机制）

自 v0.1.42 起，macOS 安装包统一固化为 **单架构 portable DMG**，工程显示名统一为 **CheersAI Desktop**（工程仓名、运行时用户数据目录名仍保留 `CheersAI-Vault`，避免老用户数据迁移，无需处理）。

### 0. 版本号管理（直接复用现有 version-manager / bump-version，不重新定义流程）

- **查询 5 处版本锚点是否一致**（package.json、Cargo.toml [package].version、tauri.conf.json、`releases/stable/version-info.json` latestVersion、`releases/stable/latest.json` version）：

```bash
corepack pnpm version:check
```

- **自动递增（patch / minor / major）并同步 5 处锚点**：

```bash
corepack pnpm version:patch    # 0.1.42 → 0.1.43
corepack pnpm version:minor    # 0.1.42 → 0.2.0
corepack pnpm version:major    # 0.1.42 → 1.0.0
```

- **指定版本号**：

```bash
corepack pnpm version:set -- 0.1.43
```

### 1. 标准打包 Skill（前端 + Rust 全量构建 → portable DMG）

- 主入口：`pnpm build:dmg:portable`（脚本：[scripts/build-macos-portable-dmg.sh](file:///Users/sevensimle/Documents/WorkSpace/CheersAI/CheersAI-Vault/scripts/build-macos-portable-dmg.sh)）
- 流程：自动跑 `version:check` 做锚点检查 → 执行 `tauri build --bundles dmg`（失败时 fallback 复用已产出的单架构 `.app`）→ ad-hoc 重签名 → `/Applications` 软链 → `hdiutil create -format UDZO` → 打印路径/SHA-256/验收清单/下一步命令
- 输出文件名强制：`CheersAI_Desktop_${VERSION}_${ARCH}_portable.dmg`（对应 `tauri.conf.json:productName` = `CheersAI Desktop`）
- **执行命令**：

```bash
corepack pnpm build:dmg:portable
# 等价：bash scripts/build-macos-portable-dmg.sh
```

### 2. 沙箱快速封装 Skill（不重新编译，只执行 .app → DMG 的最后一段）

- 场景：Trae / IDE sandbox 把前端 vite build + Rust cargo build 全跑完但因禁止访问 `/dev/rdisk*` 导致 hdiutil 失败，此时 `.app` 已存在于 `src-tauri/target/<target>/release/bundle/macos/CheersAI Desktop.app`。
- 入口：`pnpm build:dmg:quick`（脚本：[scripts/build-macos-portable-dmg-quick.sh](file:///Users/sevensimle/Documents/WorkSpace/CheersAI/CheersAI-Vault/scripts/build-macos-portable-dmg-quick.sh)）
- 流程：先 `pnpm version:check` → 定位已有的 `.app` → 校验单架构匹配 → 清 xattr → ad-hoc/custom sign + strict 校验 → `/Applications` 软链 → `hdiutil create UDZO` → 打印体积/SHA-256/CFBundleShortVersionString/架构 + 下一步 verify 命令
- **执行命令**（真终端执行 20~40s）：

```bash
corepack pnpm build:dmg:quick
# 等价：bash scripts/build-macos-portable-dmg-quick.sh
```

### 3. DMG 自动化校验 Skill（10 项自动 PASS/FAIL）

- 入口：`pnpm verify:dmg`（脚本：[scripts/verify-macos-portable-dmg.sh](file:///Users/sevensimle/Documents/WorkSpace/CheersAI/CheersAI-Vault/scripts/verify-macos-portable-dmg.sh)）
- 10 项校验：
  1. `dist` 中 DMG 存在
  2. DMG ≥ 10MiB（UDZO 正常压缩的合理下限，避免空壳/截断包）
  3. `shasum -a 256` 为 64 hex
  4. `hdiutil attach -readonly` 可挂载
  5. DMG 中含有 `CheersAI Desktop.app`
  6. `CFBundleShortVersionString == 指定版本`
  7. `CFBundleVersion == 指定版本`
  8. 二进制为单架构且等于 `--arch` / 当前机器 arch
  9. `codesign -dvv` 可读（存在签名）
  10. `codesign --verify --deep --strict` 通过
- **执行命令**：

```bash
corepack pnpm verify:dmg                                    # 默认 package.json version + 当前 arch
corepack pnpm verify:dmg --version 0.1.42                   # 指定版本
corepack pnpm verify:dmg --version 0.1.42 --arch x86_64     # 指定版本 + 架构
corepack pnpm verify:dmg --dmg ./dist/CheersAI_Desktop_0.1.42_x86_64_portable.dmg
```

---

## 产品定位与价值亮点

### 核心定位

- 面向企业与个人用户： 不论是法务、行政、财务、销售、运营，还是普通办公用户，都可以直接使用
- 面向真实工作场景： 适合在对外发文件、跨部门流转资料、提交审阅材料、接入 AI 助手前先做敏感信息处理
- 开源软件： 核心能力公开透明，便于下载试用、二次开发、内部审计和合规评估
- 本地处理优先： 重点是把敏感文件留在自己设备或企业内网中处理，减少“先上传、再担心”的不安全感

### 核心功能

- 单个文件脱敏： 临时要发一份合同、报告、表格时，可以快速处理一份文件
- 批量文件脱敏： 面对一批资料、多个附件、批量外发内容时，可以一次性集中处理
- 常用文件格式覆盖广： 支持 `CSV`、`Excel`、`JSON`、`TXT`、`Word`、`PPT`、`PDF`、`Markdown` 等常见办公文件
- 脱敏规则丰富： 可按姓名、电话、证件号、地址、机构名称、业务字段等常见敏感内容进行处理，也支持结合本地词库持续扩充
- 一键反脱敏： 文件在外部编辑完成，或 AI 返回结果后，可基于本地保留的规则自动恢复需要还原的信息
- 人更省心： 不需要反复手工查找、替换、记录对照表，普通用户也能把流程跑顺

### 安全能力

- 本地处理更安心： 重点能力在本机或企业内网执行，降低敏感原文外流风险
- 可设置脱敏口令： 重要脱敏任务可以增加口令保护，减少误用和误操作
- 规则文件加密： 用于恢复内容的规则文件会加密保存，避免被随意查看或直接拿走
- 脱敏与恢复分开控制： 先安全分享，后按需要恢复，敏感内容不会长期裸露在流转链路中
- 支持 AI 使用前预处理： 在把资料喂给大模型前，先把不该暴露的信息清理掉，更适合企业合规要求

### 核心价值

- 对个人用户： 不懂技术，也能把简历、病历、合同、申报材料、聊天记录等资料先处理再发送，降低隐私泄露压力
- 对企业团队： 在客户资料、报价单、制度文件、项目文档、内部报表等流转前先脱敏，更容易把数据安全要求落到日常工作里
- 对 AI 办公场景： 让“先脱敏，再提问”成为标准动作，减少把原始敏感信息直接交给外部模型的风险
- 对管理者： 能把个人信息保护、数据分类处理、对外共享控制这些要求，变成员工真正会用的简单流程
- 对非技术岗位： 把原本复杂的安全操作变成点选式流程，大幅降低上手门槛和培训成本

## 当前能力边界

当前仓库已经具备以下主线能力：

- 文本类文件脱敏预览
- 正式批次提交与状态轮询
- 脱敏 Markdown 产物下载
- 反脱敏恢复
- 敏感词库与内置规则协同
- 浏览器端企业内网部署
- 桌面端与 Runtime 共享核心能力
- FileBay 条件集成

当前仓库同时包含明确边界：

- 企业浏览器端目前按单系统用户模型运行，不包含账号体系、RBAC 或多租户能力
- OCR 为可选能力，不是默认随源码一起分发的运行时组件
- Linux 内网部署材料已提供，但应以实际环境验收结果为准

## 仓库结构

```text
.
├─ apps/
│  ├─ vault-pro-web/          企业浏览器端相关构建文件
│  └─ vault-runtime-api/      Rust Runtime API
├─ src/                       根前端入口与浏览器端页面
├─ src-tauri/                 桌面端与共享 Rust 核心
│  └─ crates/
│     ├─ engine-core/
│     ├─ component-runtime/
│     ├─ filebay-core/
│     ├─ sandbox-core/
│     └─ service-contracts/
├─ docs/enterprise/           部署、操作、API 文档
├─ deploy/linux/              Linux 内网部署模板
├─ docker-compose.yml         本地 Docker 验证入口
└─ compliance/generated/      依赖许可证与合规生成物
```

## 功能架构

### 1. 浏览器端路径

```text
Browser
  ↓
Nginx
  ├─ /      静态前端
  └─ /api/  Runtime API
            ↓
         engine-core
         component-runtime
         filebay-core
```

### 2. 桌面端路径

```text
Tauri Desktop
  ↓
src-tauri commands
  ↓
engine-core / sandbox-core / filebay-core
```

### 3. 条件组件

- OCR： Python + EasyOCR + 相关科学计算依赖
- 旧版 PPT 转换： LibreOffice
- FileBay： 管理员通过环境变量配置的固定目标仓库

## 快速开始

### 方式一：本地 Docker 验证

这条路径适合快速验证浏览器端与 Runtime 联调。

1. 安装基础依赖：
   - Docker
   - Docker Compose
2. 在仓库根目录执行：

```bash
docker compose up -d --build
```

3. 访问：
   - 浏览器端： `http://127.0.0.1:5173`
   - 健康检查： `http://127.0.0.1:5173/api/v1/health`

说明：

- 这条路径默认以本地验证为目标
- OCR 不应被视为默认随 Docker 一起完成合规分发的组件

### 方式二：源码构建

适合本地开发和定制化调试。

前置要求：

- Node.js 22+
- pnpm 11+
- Rust 1.85+
- Python 3.11+，仅 OCR 场景需要

安装依赖并构建前端：

```bash
pnpm install --frozen-lockfile
pnpm build
```

构建 Runtime：

```bash
cargo build --release --manifest-path apps/vault-runtime-api/Cargo.toml
```

运行桌面端开发模式：

```bash
pnpm tauri dev
```

### 方式三：Linux 内网部署

完整说明见：

- [`docs/enterprise/DEPLOYMENT.md`](./docs/enterprise/DEPLOYMENT.md)
- [`deploy/linux/README.md`](./deploy/linux/README.md)

## 文档导航

- 中文版用户说明书：
  [`docs/USER_GUIDE.md`](./docs/USER_GUIDE.md)
- English User Manual:
  [`docs/USER_GUIDE_EN.md`](./docs/USER_GUIDE_EN.md)
- 交付边界与目录治理：
  [`docs/DELIVERY_BASELINE.md`](./docs/DELIVERY_BASELINE.md)
- 部署与技术说明：
  [`docs/enterprise/DEPLOYMENT.md`](./docs/enterprise/DEPLOYMENT.md)
- 浏览器端操作手册：
  [`docs/enterprise/OPERATION_GUIDE.md`](./docs/enterprise/OPERATION_GUIDE.md)
- 客户测试 API 参考：
  [`docs/enterprise/API_REFERENCE.md`](./docs/enterprise/API_REFERENCE.md)
- 依赖与许可证清单：
  [`DEPENDENCIES.md`](./DEPENDENCIES.md)
- 安全政策：
  [`SECURITY.md`](./SECURITY.md)
- 贡献指南：
  [`CONTRIBUTING.md`](./CONTRIBUTING.md)
- 社区行为准则：
  [`CODE_OF_CONDUCT.md`](./CODE_OF_CONDUCT.md)

## 开源许可

本仓库主许可证为 `Apache-2.0`。

请特别注意：

- `LICENSE` 适用于本仓库自身代码与文档体系中明确按 Apache-2.0 发布的内容
- 第三方依赖的许可证义务不因主许可证而消失
- 默认源码仓库不会自动携带完整 `node_modules`、Cargo registry 或 OCR venv
- 任意二进制、安装包、Docker 镜像或客户交付物，都必须按实际 bundled 内容补齐对应的 NOTICE 与第三方声明

### OCR 合规特别说明

OCR 依赖链当前是本仓库最需要额外审查的区域：

- `PyMuPDF`： `AGPL-3.0` 或单独商业许可
- `python-bidi`： LGPL 家族义务需结合实际打包方式确认
- `SciPy` 等二进制 wheel 可能携带额外 bundled notices

因此：

- 默认开源源码发布可以保留 OCR 接入代码与安装说明
- 但在未完成额外许可证审查前，不要对外分发“已内置 OCR 环境”的安装包、镜像或客户交付物

细节见 [`DEPENDENCIES.md`](./DEPENDENCIES.md)。

## 参与贡献

欢迎提交 Issue、文档修正、测试改进和代码补丁。

开始前请先阅读：

- [`CONTRIBUTING.md`](./CONTRIBUTING.md)
- [`CODE_OF_CONDUCT.md`](./CODE_OF_CONDUCT.md)
- [`SECURITY.md`](./SECURITY.md)

对依赖、打包、Docker、OCR、FileBay、脱敏逻辑的改动，请同步检查：

- 是否引入新的第三方许可证义务
- 是否需要更新 `NOTICE`
- 是否需要更新 `DEPENDENCIES.md`
- 是否会改变当前安全边界与部署说明

## 合规状态摘要

基于当前仓库清点结果，可以先得出三个结论：

1. JavaScript 与 Rust 主体依赖以 MIT、Apache-2.0、BSD、ISC 等宽松许可证为主，与仓库主许可证兼容性整体良好
2. 本仓库本地 crate 原先缺少明确 SPDX 标识，现已补齐为 `Apache-2.0`
3. OCR 可选链路存在需要单独决策的合规风险，尤其是 `PyMuPDF`

本仓库保留了可追溯的合规生成物：

- [`compliance/generated/pnpm-licenses.json`](./compliance/generated/pnpm-licenses.json)
- [`compliance/generated/cargo-metadata-tauri.json`](./compliance/generated/cargo-metadata-tauri.json)
- [`compliance/generated/cargo-metadata-runtime.json`](./compliance/generated/cargo-metadata-runtime.json)
- [`compliance/generated/python-ocr-pip-show.txt`](./compliance/generated/python-ocr-pip-show.txt)

## 免责声明

本项目按 “AS IS” 提供，不提供任何明示或默示担保。对于处理真实敏感数据、客户文件、内网系统接入或二次商业分发，请先完成内部安全评估、开源合规评估和环境验收。
