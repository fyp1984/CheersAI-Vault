# CheersAI · 企业级 SDLC 智能体工作流总章程（Harness Engineering × SDLC-Gatekeeper）

> 版本: v2.0.0 · 生效日期: 2026-08-14 · 适用范围: CheersAI 全家桶（Desktop/Portal/FileBay/Nexus/SSO/Vault）
> 术语更正说明: v1.x 中误写的「Hanessian Engineering」已统一修正为 **Harness Engineering**（测试工装工程 / 全生命周期 Harness 化）。本工作流所有原则均严格遵循业界标准 **Harness Engineering 7 原则**。

---

## 0. 核心设计哲学（Harness Engineering 7 原则融入全链路）

Harness Engineering（业界标准「测试工装工程 / 生命周期工装化工程」）的 **7 条核心原则** 内嵌为本工作流的 **硬约束**，不是最佳实践推荐：

| 编号 | 原则（业界标准术语） | 本工作流硬实现位置 |
|---|---|---|
| **HE-1** | **Explicit Harness Hierarchy**（**显式 Harness 分层**）：G1-G5 每一层独立 harness，禁止一套工装跑所有场景 | `sdlc/harnesses/G<X>-*.harness.yaml` × 5 份 + 每门脚本独立 exit code |
| **HE-2** | **Deterministic Reproducibility**（**确定性可复现**）：同一 harness 同一输入 100% 同一输出；禁止隐式环境依赖 | 所有 gate-* 脚本 `SHA256(inputs)` 写入审计链；`verify-harness-integrity.py` 对 harness 本身 sha 锁 |
| **HE-3** | **Observability First**（**观测优先**）：每个 harness 必须产出结构化 trace/metric/log，不能只有 stdout | `audit-writer.py` 链式 9 字段 JSONL；`collect-harness-coverage.py` 统计触发频次 / 捕获率 |
| **HE-4** | **Zero-Trusted Pass-Thru**（**零信任过站**）：未通过 harness 的交付物严禁进入下阶段；harness fail 立即丢弃产物 | G1-G5 `set -e` + SoD 双重阻断；FAIL 写入 audit 并由 `run-pipeline.sh` 立即 `set -e` 终止 |
| **HE-5** | **Harness-as-Code & Versioned**（**Harness 即代码 & 版本化**）：工装脚本/阈值配置 必须入版本库；改工装必须经 TD+RO 双签 | `sdlc/gates/*.spec.yaml` + `sdlc/harnesses/*.harness.yaml` + `sdlc/bin/*` 全入库；改 `gates/` 阈值 PR 必须 2 approval |
| **HE-6** | **Feedback-Driven Harness Evolution**（**反馈驱动 Harness 进化**）：每次 FP/FN 漏检/误检 必须反哺 harness 新增回归条目 | `retrospective-feedback.py` 自动统计；新增「regression harness registry」存历次 FAIL 条目 |
| **HE-7** | **Harness Coverage Not Only Code Coverage**（**Harness 覆盖率 ≥ 代码覆盖率**）：不仅代码要有覆盖率，每一门 harness 自己也要统计「被触发过多少次？捕获过多少真缺陷？」 | `collect-harness-coverage.py` 每次 release 后输出 harness coverage 报告；阈值要求 ≥ 90% harness 在 1 个 sprint 内至少被触发 1 次 |

---

## 1. 智能协作团队角色矩阵

| 角色代号 | 角色名称 | 核心交付物 | 上游输入 | 下游输出 | Harness 签收证据 (HE-3/HE-4) |
|---|---|---|---|---|---|
| **RA** | 需求分析 (Requirement Analyst) | `PRD-<TICKET>.md`（带验收标准/优先级/依赖/合规） | 业务诉求、Stakeholder 访谈记录 | Tech-Design 输入 (TD) | ✅ G1-PRD harness `gate-prd.py` exit-0 + audit 条目含 SHA |
| **TD** | 功能设计 (Tech Designer) | `DESIGN-<TICKET>.md`（架构选型/模块拆分/接口契约/资源/风险） | RA 签署的 PRD | 编码任务单 + CI 契约 | ✅ G2-DESIGN harness `gate-design.py` exit-0 + BASE-PRD 锚点校验通过 (HE-2) |
| **CD** | 程序开发 (Coder Developer) | 功能源码 + 单测 + Makefile 目标通过 | TD 签署的 DESIGN + 当前代码库技术栈 | PR/MR（含变更说明） | ✅ G3-CODE harness (HE-5 复用子仓 Makefile)：静态 lint + 覆盖率 ≥ 语言阈值 + Code Review ≥1 approval 三者 exit-0 |
| **QA** | 测试验收 (Quality Assurance) | `TEST-<TICKET>.md` + 用例执行 JSON + 缺陷闭环清单 | CD 签署的 PR/MR merge 后 commit | 发布准入单 | ✅ G4-TEST harness：核心场景 100% + P0/P1 缺陷 100% 修复 + SAST/SCA/DAST 零高危 (HE-6 反馈驱动) |
| **RO** | 发布运维 (Release Operator) | 灰度→全量发布记录 + 监控告警配置 + SLA 达标报告 | QA 签署的准入单 + 生产环境 | 版本发布 + 事后复盘 (Postmortem) | ✅ G5-RELEASE harness：三阶段灰度 + RED/USE 仪表盘 + 回滚演练通过 + SLA ≥ 99.9% |

### 1.1 角色隔离硬约束（SoD，对应 **HE-4 零信任过站** + **HE-3 观测优先**）
- 同一 Git 提交者 (git user.email) 不得担任同一 TICKET 的 RA+TD、TD+CD、CD+QA、QA+RO 任意组合；
- 违反则 `check-sod.py --check-sod TICKET=xxx` **exit 非 0**，阻断流程 (HE-4)；
- 所有违规写入 audit 链，`collect-harness-coverage.py` 会统计「SoD 违规率」作为 harness 运营健康指标。

## 2. 全流程阶段 & 交接门禁（Gatekeeper G1-G5，对应 HE-1 分层）

```
 ┌──────────┐  G1: PRD Harness  ┌──────────┐  G2: DESIGN Harness ┌──────────┐  G3: CODE Harness  ┌──────────┐  G4: TEST Harness  ┌──────────┐  G5: RELEASE Harness
 │ RA · PRD │ ───────────────▶ │ TD · DSG │ ────────────────────▶ │ CD · SRC │ ──────────────────▶ │ QA · TST │ ──────────────────▶ │ RO · REL │
 └──────────┘                   └──────────┘                       └──────────┘                      └──────────┘                     └──────────┘
    ⚠️  G1 FAIL 丢弃 PRD           ⚠️  G2 FAIL 丢弃 DESIGN           ⚠️  G3 FAIL 拒收代码             ⚠️  G4 FAIL 禁止发布             ⚠️  G5 FAIL 停止灰度
```

每个门禁由 **三层 Harness-as-Code (HE-5)** 联合定义：
1. `sdlc/harnesses/G<X>-<NAME>.harness.yaml`：**完整工装驱动**（输入/输出/观测点/环境变量/退出码映射/回归条目 registry）
2. `sdlc/gates/G<X>-<NAME>.spec.yaml`：**可量化阈值**（覆盖率、错误率上限、SLA 下限）
3. `sdlc/bin/gate-<name>.{py,sh}`：**自动执行脚本**（exit 0 = PASS）；审计写入 `sdlc/audit/<DATE>/G<X>-<TICKET>*`

### 2.1 G1 · PRD 质量门禁（HE-2 可复现 + HE-3 观测）
**阈值（G1-PRD.spec.yaml）**：
- 合规性评分 = 100%（PII/C3/C4 字段必须逐字段标注脱敏/加密策略）
- 完整性评分 = 100%：验收标准 / 优先级(MoSCoW) / 依赖图 / NFR 四章节齐全
**命令**：`cd sdlc && ./bin/gate-prd.py --prd <PATH>`

### 2.2 G2 · DESIGN 质量门禁（HE-5 Harness-as-Code + HE-6 反馈驱动）
**阈值**：
- 架构评审 Checklist 18 项 100% 勾选；
- 技术栈一致性：`<REPO>-tech-stack.lock.json` 白名单比对；主版本不得降级 / 禁止新增大语言异类框架；
- HTTP 接口契约必须 OpenAPI 3.0 且 `spectral lint` exit 0；
- **BASE-PRD 锚点校验**（HE-2 可复现）：DESIGN 首行 `<!-- BASE-PRD: PRD-xxx.md@sha256 -->` 与真实 PRD sha 100% 相等。
**命令**：`cd sdlc && ./bin/gate-design.py --design <DSG> --repo ../CheersAI-Vault`

### 2.3 G3 · CODE 质量门禁（复用各仓技术栈，禁止另起炉灶）
**阈值（按语言动态，G3-CODE.spec.yaml）**：
- Go：`make lint-go` / `make fmt` / `make test` ≥ 70% 覆盖率 / `make tidy`（go.mod 变动时）
- TypeScript/React：`make lint-js` + `pnpm typecheck` + Vitest ≥ 70%
- Java (Nexus)：`mvn checkstyle:check` + `mvn test` ≥ 70%
- Python (Desktop)：`ruff check` / `pytest` ≥ 80%
- Rust (Vault)：`cargo clippy -- -D warnings` + `cargo test` ≥ 70%
- Code Review：≥ 1 位非提交者 Approve
**命令**（自动识别语言并调用子仓 Makefile，完全复用既有 AGENTS 技术栈）：
```bash
cd CheersAI-Vault && ../sdlc/bin/gate-code.sh --repo .
```

### 2.4 G4 · TEST 质量门禁（HE-6 反馈驱动 + HE-7 harness coverage）
**阈值**：
- 核心场景（P0）通过率 = 100%；
- 缺陷修复率：P0 = 100%、P1 = 100%、P2 遗留 ≤ 3 且签署延期；
- SAST / SCA 零 Critical；DAST 影子流量零 High；
- TP99 性能回归 ≤ baseline × 110%；
- **Harness Coverage 指标 (HE-7)**：本 release 涉及的 G4 用例 harness 触发率 ≥ 90%。
**命令**：`cd sdlc && ./bin/gate-test.py --test-report artifacts/test-report.json`

### 2.5 G5 · RELEASE 质量门禁（HE-1 分层灰度 + HE-3 观测）
**阈值**：
- 灰度 3 阶段（1% → 10% → 50%）每阶段 ≥ 60min，错误率 < 0.1%；
- RED/USE 仪表盘齐全；告警 P0 ≤ 15min 响应、P1 ≤ 60min；
- 一键回滚脚本 **预演环境执行成功 ≥ 1 次**（HE-2 可复现演练）；
- SLA 实际 ≥ 目标 ≥ 99.9%。
**命令**：`cd sdlc && ./bin/gate-release.py --release-log artifacts/release-vX.Y.Z.json`

## 3. 基于 Harness Engineering 7 原则的「不另起炉灶」强制一致性

对应 HE-2 (可复现) + HE-5 (Harness 即代码)：**任何增量开发禁止脱离当前代码库技术体系另起炉灶**，硬规则三条：

**硬规则 1 — PRD 基准锚定 (HE-2)**
- PRD 首行 `<!-- PRD-SHA256: <sha> -->`；
- DESIGN/TEST/RELEASE 首行 `<!-- BASE-PRD: PRD-xxx.md@<sha> -->`；
- 不匹配 → gate-design.py 立即 FAIL (HE-4 零信任)。

**硬规则 2 — 技术栈白名单锁 (HE-5)**
- `<REPO>-tech-stack.lock.json` 显式列 allowed + forbidden 语言/框架；
- gate-design.py 对 DESIGN 代码片段做 forbidden 正则扫描（例：Vault 不得出现 `import Vue` / `@SpringBootApplication`）；
- 新增大依赖必须提交 PR 到 `policies/*.lock.json` 且经 TD+RO 双签 (HE-5 Harness-as-Code 变更流程)。

**硬规则 3 — 反馈驱动去冗余 (HE-6 + HE-7)**
- `retrospective-feedback.py` 每次 Release 后统计 G1-G5：
  - 连续 3 次 100% PASS 且返工 < 1 → 候选「移入夜间巡检 harness registry」，缩短主链路；
  - 连续 2 次 FAIL → 候选「补充回归 harness 条目 (HE-6)」；
- `collect-harness-coverage.py` 统计 G1-G5 harness 触发率，≤ 90% 时发告警（HE-7）。

## 4. 目录结构（落地到 CheersAI 多仓库根）

```
CheersAI/
├─ sdlc/                                   ← 本流程中心（跨所有仓库复用，HE-5 Harness-as-Code 全部入库）
│  ├─ AGENTS.md                            ← AI 智能体执行守则（角色注册 / SoD 红线 / Harness Engineering 7 原则锚点）
│  ├─ README.md                            ← 你正在读的文件
│  ├─ harnesses/                           ← ★ Harness Engineering 核心 ★：5 份完整工装驱动定义
│  │  ├─ G1-PRD.harness.yaml
│  │  ├─ G2-DESIGN.harness.yaml
│  │  ├─ G3-CODE.harness.yaml
│  │  ├─ G4-TEST.harness.yaml
│  │  └─ G5-RELEASE.harness.yaml
│  ├─ gates/                               ← 5 份可量化阈值 spec
│  ├─ bin/                                 ← 12 份可执行脚本（全部 +x，HE-5 版本化）
│  │  ├─ gate-prd.py / gate-design.py / gate-code.sh
│  │  ├─ gate-test.py / gate-release.py
│  │  ├─ audit-writer.py                   ← HE-3 观测优先：链式 JSONL trace
│  │  ├─ check-sod.py                      ← HE-4 零信任：SoD 4 种组合阻断
│  │  ├─ verify-harness-integrity.py       ← HE-2/HE-5：harness 自身 sha 锁（防改工装放水）
│  │  ├─ collect-harness-coverage.py       ← HE-7：harness 覆盖率统计 & 告警
│  │  ├─ retrospective-feedback.py         ← HE-6：反馈驱动 harness 进化
│  │  └─ demo-bootstrap.sh / run-pipeline.sh
│  ├─ policies/                            ← 4 子仓技术栈 lock + 数据分级
│  ├─ templates/                           ← 5 份强制交付物模板
│  ├─ docs/                                ← 实际 PRD/DESIGN/TEST/RELEASE 文档
│  ├─ artifacts/                           ← test-report.json / release-log.json 等机器可读证据 (HE-2)
│  └─ audit/                               ← YYYYMMDD/_<session>.jsonl (HE-3 不可篡改 trace)
├─ CheersAI-Vault/AGENTS.md                ← 每个子仓 AGENTS 末尾追加：引用中心 SDLC 章程 + Harness Engineering SoD
├─ CheersAI-FileBay/AGENTS.md
├─ CheersAI-Nexus/AGENTS.md
├─ CheersAI-Desktop/{web,api}/AGENTS.md
```

## 5. 与各单仓 AGENTS.md 的衔接

每个子仓 `AGENTS.md` 末尾追加（已在 v2.0 中统一更正术语为 Harness Engineering）：
```
🔗 企业级全流程协作（Harness Engineering × SDLC-Gatekeeper）：
   执行任何需求/设计/编码/测试/发布前，必须先阅读并严格遵守
   [CheersAI-SDLC /sdlc/AGENTS.md](file:///Users/sevensimle/Documents/WorkSpace/CheersAI/sdlc/AGENTS.md)
   与 [sdlc/README.md](file:///Users/sevensimle/Documents/WorkSpace/CheersAI/sdlc/README.md)；
   所有交接必须经 G1-G5 Harness 脚本并通过 verify-harness-integrity (HE-2) + 不可篡改链式审计 (HE-3)；
   严禁脱离 Harness Engineering SoD（RA/TD/CD/QA/RO 4 种双岗组合）一人兼任。
```

例：
- [CheersAI-FileBay/AGENTS.md](file:///Users/sevensimle/Documents/WorkSpace/CheersAI/CheersAI-FileBay/AGENTS.md)
- [CheersAI-Nexus/AGENTS.md](file:///Users/sevensimle/Documents/WorkSpace/CheersAI/CheersAI-Nexus/AGENTS.md)
- [CheersAI-Vault/AGENTS.md](file:///Users/sevensimle/Documents/WorkSpace/CheersAI/CheersAI-Vault/AGENTS.md)

## 6. 一键跑通「端到端 Harness 演示」

```bash
cd CheersAI/sdlc
# 0) ★ 先校验 Harness 本身没被篡改 (HE-2 + HE-5)
./bin/verify-harness-integrity.py          # 所有 bin/*/gates/*/harnesses/* sha 对得上才放行

# 1) 生成示例 IMP-001 PRD + DESIGN（带 BASE-PRD 锚点）
./bin/demo-bootstrap.sh

# 2) 从 G1 → SoD → G2 → SoD → G3 → … → G5 依次跑，每步写 trace (HE-3) + 累计 coverage (HE-7)
./bin/run-pipeline.sh --ticket IMP-001 --repo ../CheersAI-Vault

# 3) 查看三个核心产物：
cat audit/$(date +%Y%m%d)/pipeline-IMP-001.summary.md        # 门禁汇总 + harness coverage
cat audit/$(date +%Y%m%d)/process-redundancy-report.md       # HE-6 反馈驱动建议
cat audit/$(date +%Y%m%d)/harness-coverage.md                # HE-7 harness 覆盖率
```

---

本章程作为 CheersAI 企业级 Harness Engineering × SDLC-Gatekeeper 工作流的 **最高法**：子仓定制化门禁只能 **更严格而不可更宽松**；调整阈值 / 新增 harness 必须提交 PR 到 `sdlc/harnesses/` + `sdlc/gates/` 并经 TD+RO 双签后生效，`verify-harness-integrity.py` 会自动校验新 hash 已入允许清单 (HE-5)。
