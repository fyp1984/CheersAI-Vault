# Contributing to CheersAI Vault

感谢你对 CheersAI Vault 的关注。

本仓库同时包含桌面端、企业浏览器端、Runtime、共享 Rust 核心、部署文档与合规文件。为了让贡献可审查、可回归、可发布，请在提交前遵守以下约定。

## 1. 开始之前

提交贡献前，请先阅读：

- [`README.md`](./README.md)
- [`SECURITY.md`](./SECURITY.md)
- [`CODE_OF_CONDUCT.md`](./CODE_OF_CONDUCT.md)
- [`DEPENDENCIES.md`](./DEPENDENCIES.md)

若改动涉及部署、OCR、FileBay、Docker、许可证、品牌素材或客户交付，请先开 Issue 或讨论帖确认范围。

## 2. 可接受的贡献类型

欢迎以下类型的贡献：

- Bug 修复
- 测试补充
- 文档完善
- 性能优化
- 可维护性重构
- 开源合规与安全治理改进

以下类型请先讨论再提交：

- 公开 API 改动
- 规则体系变更
- 新增第三方依赖
- 新增 OCR / 文件解析组件
- 新增对外可见的产品承诺
- 协议、品牌、商标、商业交付边界调整

## 3. 开发环境

常见本地依赖：

- Node.js 22+
- pnpm 11+
- Rust 1.85+
- Python 3.11+，仅 OCR 场景需要
- Docker，若要验证浏览器端部署链路

常用命令：

```bash
pnpm install --frozen-lockfile
pnpm build
cargo build --manifest-path src-tauri/Cargo.toml
cargo build --manifest-path apps/vault-runtime-api/Cargo.toml
docker compose up -d --build
```

## 4. 分支与提交建议

建议使用清晰的分支命名：

- `feature/<topic>`
- `fix/<topic>`
- `docs/<topic>`
- `chore/<topic>`

建议使用 Conventional Commits 风格：

```text
feat(runtime): add batch retry guard
fix(web): handle preview 413 error explicitly
docs(compliance): update dependency license inventory
chore(build): align runtime manifest metadata
```

提交信息要求：

- 主题行简洁明确
- 说明改动对象和原因
- 不要提交 “update” “misc” “fix bug” 这类无信息标题

## 5. Pull Request 流程

1. 先同步最新主分支
2. 在独立分支上提交改动
3. 自查构建、测试、文档、合规项
4. 发起 PR，并写清：
   - 变更背景
   - 变更范围
   - 风险点
   - 验证方式
   - 是否引入新依赖
5. 等待代码审查并根据意见修订

PR 描述至少应回答：

- 为什么改
- 改了什么
- 怎么验证
- 有什么已知限制

## 6. 代码审查标准

PR 会重点审查以下内容：

- 功能是否符合需求
- 是否引入行为回退
- 是否破坏部署链路
- 是否影响桌面端与浏览器端共享核心
- 是否改变脱敏、恢复、日志或沙箱边界
- 是否引入新的许可证义务
- 是否更新必要文档

任何涉及以下内容的 PR，都应被视为高敏感改动：

- 文件上传与下载
- OCR 组件
- Runtime HTTP 接口
- `.cmap` 映射处理
- FileBay 令牌与上传
- 日志、历史、路径暴露
- Docker 与客户交付脚本

## 7. 提交前检查

提交前至少完成以下检查：

### 通用检查

- `pnpm build`
- Rust 相关构建至少通过一条受影响路径
- 新增或改动文档已同步更新
- 不提交真实密钥、令牌、账号、客户文件

### 若改动浏览器端

- 页面可打开
- 关键交互可用
- Console 无新增明显错误

### 若改动 Runtime

- 健康检查正常
- 相关 API 行为已验证
- 批次、预览、产物或恢复链路未回退

### 若改动 OCR

- 明确说明是否改变了依赖栈
- 重新检查 `DEPENDENCIES.md`
- 明确说明是否影响二进制分发义务

### 若改动许可证或依赖

- 更新 `DEPENDENCIES.md`
- 视需要更新 `NOTICE`
- 将新依赖写入合规说明
- 说明其许可证与本仓库主许可证的兼容性

## 8. 文档要求

本仓库的用户、管理员、开发者和法务都会阅读文档。提交文档时请遵守：

- 不把未验收能力写成正式承诺
- 不把内部临时路径写成对外交付要求
- 不泄露真实服务器、令牌、客户信息
- README 负责总入口，详细步骤放到专题文档
- Markdown 必须通过兼容性检查

文档改动完成后，请运行：

```bash
python3 "/Users/FYP/Documents/WorkSpace/CheersAI/CheersAI - docs/.trae/skills/cheersai-markdown-compatibility/scripts/check_markdown_emphasis.py" <目标文件.md>
```

## 9. 依赖与许可证要求

新增依赖前请先回答：

1. 许可证是什么
2. 是否与 `Apache-2.0` 兼容
3. 是否会影响源码、二进制、Docker、安装包的分发义务
4. 是否包含模型、字体、词典、系统库或额外数据文件
5. 是否需要更新 `NOTICE` 或第三方声明

以下情况必须在 PR 中显式披露：

- GPL、LGPL、AGPL、MPL 等 copyleft 许可证
- 商业双许可
- 带专利、商标、数据集限制的组件
- 编译期和运行期许可证不同的组件

## 10. 安全问题

请不要在公开 Issue 中直接披露可利用漏洞细节。

安全问题请按 [`SECURITY.md`](./SECURITY.md) 中的流程私下报告。

## 11. 行为要求

所有贡献者都应遵守 [`CODE_OF_CONDUCT.md`](./CODE_OF_CONDUCT.md)。

简而言之：

- 尊重他人
- 讨论问题，不攻击个人
- 对事实负责
- 对用户数据与安全边界保持谨慎

## 12. 贡献许可

除非你明确声明其他条款，向本仓库提交的代码、文档和配置变更，将默认按本仓库当前主许可证处理。

如果你无权提交相关内容，请不要发起贡献。
