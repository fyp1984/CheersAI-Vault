# CheersAI Vault

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
- Runtime API 测试资料（功能 17 项与本机性能基线）：
  [`docs/testing/runtime-api/README.md`](./docs/testing/runtime-api/README.md)
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
