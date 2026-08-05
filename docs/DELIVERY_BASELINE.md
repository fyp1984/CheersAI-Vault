# CheersAI脱敏沙箱交付基线与目录治理

## 结论

本文件用于定义 CheersAI脱敏沙箱仓库的最终交付边界、目录职责和持续治理规则。

从本次治理开始，仓库内文件按两类管理：

- 最终交付资产：应长期保留、可对外说明、可支撑研发与发布
- 非最终交付资产：开发调试、过程记录、临时产物、测试输出、构建生成物，默认不进入仓库主干

若后续出现冲突，以本文件为当前仓库的交付边界权威说明。

## 适用范围

适用根目录：

`/Users/FYP/Documents/WorkSpace/CheersAI/subproducts/CheersAI-Vault`

适用对象：

- 仓库维护者
- 产品文档编写者
- 开发、测试、发布人员

## 一、最终交付白名单

### 1. 根目录保留项

以下文件属于长期有效的基础交付资产：

| 路径 | 说明 |
|---|---|
| `README.md` | 仓库主入口与双语导航 |
| `LICENSE` | 开源许可证 |
| `NOTICE` | 版权与第三方依赖声明 |
| `SECURITY.md` | 漏洞上报与安全处理说明 |
| `DEPENDENCIES.md` | 依赖与许可证清单 |
| `CONTRIBUTING.md` | 贡献协作规范 |
| `CODE_OF_CONDUCT.md` | 社区行为准则 |
| `package.json` | 前端根工程元数据 |
| `pnpm-lock.yaml` | 前端锁定依赖 |
| `pnpm-workspace.yaml` | 多包工程工作区配置 |
| `docker-compose.yml` | 本地联调与运行入口 |
| `docker/` | Docker 运行相关配置 |
| `deploy/linux/` | Linux 部署资产与说明 |
| `compliance/generated/` | 合规扫描与许可证证据文件 |

### 2. 产品源码保留项

以下目录属于产品实现主体，应纳入正式交付范围：

| 路径 | 说明 |
|---|---|
| `src/` | Web 前端主源码 |
| `src-tauri/` | Tauri 桌面端主源码与配置 |
| `apps/vault-runtime-api/` | Runtime 服务源码 |
| `apps/vault-pro-web/` | 子应用源码 |
| `public/` | 前端静态资源 |

说明：

- `src-tauri/target/`、`src-tauri/gen/` 不属于源码保留项
- `apps/vault-pro-web/dist/` 不属于源码保留项
- `node_modules/` 不属于源码保留项

### 3. 文档保留项

当前仓库内的正式有效文档，保留以下主线：

| 路径 | 说明 |
|---|---|
| `docs/USER_GUIDE.md` | 中文版正式用户说明书 |
| `docs/USER_GUIDE_EN.md` | 英文版正式用户说明书 |
| `docs/enterprise/DEPLOYMENT.md` | 部署说明 |
| `docs/enterprise/OPERATION_GUIDE.md` | 操作手册 |
| `docs/enterprise/API_REFERENCE.md` | API 参考 |
| `docs/DELIVERY_BASELINE.md` | 交付边界与目录治理权威说明 |

### 4. 测试基线保留项

以下内容虽然不属于对外展示文档，但属于研发有效资产，应按需要长期保留：

| 路径 | 说明 |
|---|---|
| `apps/vault-runtime-api/tests/fixtures/` | 自动化测试样本与基线文件 |
| `test/` | 若后续沉淀为正式自动化测试，应作为源码测试资产保留 |

## 二、非最终交付范围

以下内容默认视为非最终交付资产：

- 调试记录
- 过程性总结
- 阶段性进度报告
- 手工验证脚本
- 浏览器测试页面
- 本地运行缓存
- 编译中间产物
- 性能压测输出
- 测试结果归档
- 一次性修复说明

对应常见形态包括：

- `*.log`
- `test-results/`
- `.local/`
- `dist/`
- `node_modules/`
- `src-tauri/target/`
- `src-tauri/gen/`
- `docs/*_SUMMARY*.md`
- `docs/*_PROGRESS*.md`
- `docs/*_FIX*.md`
- `docs/*_DEBUG*.md`
- `docs/*_VERIFICATION*.md`
- `docs/*_REPORT*.md`

## 三、目录职责

### 根目录

只保留以下四类内容：

- 仓库入口说明
- 合规与社区治理文件
- 顶层工程配置
- 顶层运行与部署入口

根目录不再新增：

- 单次功能总结文档
- 临时测试脚本
- 调试 HTML 页面
- 手工排障说明

### `docs/`

`docs/` 目录只保留正式、可复用、面向长期读者的有效文档。

推荐只保留三类文档：

- 用户说明书
- 部署与操作说明
- 仓库治理与交付边界说明

若新增文档只是解释某次修复过程、验证过程或阶段性进展，不应直接进入 `docs/` 主目录。

### `docs/enterprise/`

该目录只放企业部署与运行资料：

- 部署
- 运维
- API

不要把 UI 调整说明、测试报告、一次性接入记录混入该目录。

### `src/`、`src-tauri/`、`apps/`

只保留源码、资源、配置和正式测试基线。

不要在源码目录内长期保留：

- `TestPage`
- `InstallerTest`
- `debug.rs`
- 临时探针代码
- 手工验证入口

## 四、文档治理规则

### 1. 一题一文

同一主题只保留一份当前有效文档。

例如：

- 用户使用说明只保留 `docs/USER_GUIDE.md` 和 `docs/USER_GUIDE_EN.md`
- 交付边界说明只保留 `docs/DELIVERY_BASELINE.md`

### 2. 正式文档优先

当某类信息已进入正式文档后，相关的：

- 快速说明
- 修复说明
- 过程总结
- 验证记录

应及时移出主干，避免并行口径。

### 3. 过程文档不常驻

如果必须保留阶段性材料，应放到仓库外部知识库、项目管理系统或单独归档位置，不作为主仓库当前事实的一部分。

## 五、Git 忽略基线

以下目录和产物应长期由 `.gitignore` 控制：

- `node_modules/`
- `dist/`
- `.local/`
- `src-tauri/target/`
- `src-tauri/gen/`
- `test-results/`
- `.cleanup-archive/`
- 常见日志和临时文件

原则是：

- 过程产物默认不进仓库
- 如确需保留测试资产，应保留源测试代码和测试基线，不保留一次性运行结果

## 六、提交前检查

每次提交前至少检查以下事项：

| 检查项 | 要求 |
|---|---|
| 文档新增 | 是否属于正式长期文档 |
| 根目录新增文件 | 是否真的需要长期保留 |
| 构建产物 | 不应进入提交 |
| 测试结果 | 不应进入提交 |
| 调试文件 | 不应进入提交 |
| README 导航 | 若新增正式文档，需同步更新入口 |
| 合规文件 | 不得被过程文档覆盖或分叉 |

## 七、后续治理优先级

当前仓库已完成一轮明显过程文件清理，但仍建议继续做两项收敛：

1. 继续复核 `docs/` 下仍保留的功能说明型文档，区分：
   - 必须长期保留的产品能力说明
   - 可并入正式说明书后下线的历史文档
2. 将 `.gitignore` 进一步标准化，建立固定的提交前检查动作

## 八、执行口径

后续如果再出现类似问题，默认按以下顺序处理：

1. 先识别是否属于最终交付资产
2. 若不是，先归档
3. 经确认后移出仓库
4. 更新 `.gitignore`
5. 更新本文件或 README 导航

## 九、`docs/` 目录当前分类结论

本轮清理后，`docs/` 目录仍保留三类内容，但它们的治理优先级不同。

### 1. A 类：当前正式保留

这些文件应继续作为当前仓库内的正式有效文档保留：

| 路径 | 角色 | 结论 |
|---|---|---|
| `docs/USER_GUIDE.md` | 中文正式用户说明书 | 保留 |
| `docs/USER_GUIDE_EN.md` | 英文正式用户说明书 | 保留 |
| `docs/enterprise/DEPLOYMENT.md` | 部署说明 | 保留 |
| `docs/enterprise/OPERATION_GUIDE.md` | 操作手册 | 保留 |
| `docs/enterprise/API_REFERENCE.md` | API 参考 | 保留 |
| `docs/DELIVERY_BASELINE.md` | 交付边界与治理基线 | 保留 |

### 2. B 类：建议并入正式文档后下线

本轮治理后，原 `B 类` 文档已完成并入或下线，当前不再保留独立 `B 类` 文件。

| 路径 | 当前性质 | 建议归并目标 |
|---|---|---|
| 无 | 无 | 后续若再出现功能说明型文档，应优先并入正式说明书或操作手册 |

### 3. C 类：已完成分流处理

本批 `C 类` 文档已按“历史设计参考”与“低价值历史清单”两类做分流处理。

| 路径 | 当前性质 | 建议动作 |
|---|---|---|
| `CheersAI - docs/11-技术/99-archive/CheersAI-Vault-历史设计参考_20260805/SYSTEM_ARCHITECTURE.md` | 架构示意与历史流程说明 | 已外移为历史设计参考 |
| `CheersAI - docs/11-技术/99-archive/CheersAI-Vault-历史设计参考_20260805/FILEBAY_DATABASE_SOLUTION.md` | 方案设计记录 | 已外移为历史设计参考 |
| `CheersAI - docs/11-技术/99-archive/CheersAI-Vault-历史设计参考_20260805/CheersAI产品UI规范.md` | UI 规范 | 已外移为历史设计参考 |
| `CheersAI - docs/11-技术/99-archive/CheersAI-Vault-历史设计参考_20260805/CHEERSAI_UI_COMPONENTS.md` | UI 组件说明 | 已外移为历史设计参考 |
| `CheersAI - docs/11-技术/99-archive/CheersAI-Vault-历史设计参考_20260805/UI_IMPLEMENTATION_GUIDE.md` | UI 实施指南 | 已外移为历史设计参考 |
| `CheersAI - docs/11-技术/recycle-bin/20260805-CheersAI-Vault-回收站清理记录.md` | 回收区最终清理记录 | 已保留治理留痕 |

### 4. 当前治理顺序

后续处理 `docs/` 时，按以下顺序推进：

1. 先保住 A 类文档不分叉
2. 将 B 类内容继续并入 A 类正式文档
3. 对回收区中的剩余 C 类逐份复核，决定永久下线时间

### 5. 默认判断规则

今后新增 `docs/` 文件时，默认先问自己四个问题：

1. 这是不是长期有效的正式说明？
2. 这份内容能不能直接并入现有正式文档？
3. 这是不是一次性修复、测试或功能迭代记录？
4. 这份内容是否更适合放到外部技术设计库而不是当前仓库？

只要第 2、3、4 项里任一答案为“是”，就不应直接把它当作 `docs/` 主目录的长期文档。
