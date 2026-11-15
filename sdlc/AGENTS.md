# CheersAI-SDLC 智能体执行守则（面向 AI Agent 角色分工 · Harness Engineering 7 原则 + SoD）

> 本文件是所有 AI 智能体 / 人工工程师在 **任何 CheersAI 项目** 上执行研发任务前 **必须阅读并遵守** 的硬性规范。违反本文件将导致 G1-G5 Harness 脚本 `exit != 0` 阻断流程。
> 原则索引：业界标准 **Harness Engineering 7 原则 HE-1 ~ HE-7**，详见 [README.md §0](file:///Users/sevensimle/Documents/WorkSpace/CheersAI/sdlc/README.md#L8-L20)。

## 0. 前置动作：三读 + 注册

任何智能体打开一个 CheersAI 子仓后 **必须在 5 分钟内完成以下 3 读 + 1 注册**，**否则一律视为 HE-2（可复现性）违规**：

1. `cat ../sdlc/AGENTS.md` （本文件，读完全文）
2. `cat ../sdlc/README.md §0`（Harness Engineering 7 原则对齐）
3. `cat ./AGENTS.md`（子仓语言/工具链规范，例：FileBay/Go 的 `make lint-go`）
4. **角色注册**：写入 `sdlc/audit/YYYYMMDD/_actor_roles.jsonl`，格式：
   ```jsonl
   {"ts":1786721270,"actor_email":"<your_email>","role":"<RA|TD|CD|QA|RO>","allowed_repos":["CheersAI-Vault"]}
   ```
   同一会话 / 同一 TICKET 下注册后 **严禁切换角色**（违反 → HE-4 零信任 SoD 违规，FAIL）。

## 1. SoD 职责分离红线（对应 **HE-4 零信任过站** + HE-3 观测）

同一 TICKET 下 **以下 4 种组合属于 100% 违规**，[check-sod.py](file:///Users/sevensimle/Documents/WorkSpace/CheersAI/sdlc/bin/check-sod.py) 必 fail：

| 组合 | 违规类型 | 对应 SoD 原理 |
|---|---|---|
| RA + TD | 需求与设计合一 | 禁止既写 PRD 又自我批设计（自我交易风险）|
| TD + CD | 设计与编码合一 | 禁止既写设计又自我验证实现符合度 |
| CD + QA | 编码与测试合一 | 禁止「自己写的代码自己测」，杜绝 0 缺陷幻象 |
| QA + RO | 测试与发布合一 | 禁止既判是否准入又自行点击发布（合规内控红线）|

检测方式：
```bash
cd sdlc && ./bin/check-sod.py --ticket IMP-001   # 必须 exit 0 才能继续
```

## 2. 智能体分角色工作守则（逐条锚定 Harness 原则）

### RA (需求分析) 守则
- **HE-2 可复现基准**：唯一输出必须是 `sdlc/docs/PRD-<TICKET>.md`（[PRD-TEMPLATE.md](file:///Users/sevensimle/Documents/WorkSpace/CheersAI/sdlc/templates/PRD-TEMPLATE.md)），**禁止用自由文档替代模板**；
- **HE-3 观测优先**：必填章节 = ①背景目标 ②用户故事 ③验收标准（可量化） ④优先级(MoSCoW) ⑤依赖&风险 ⑥合规&C1-C4 逐字段；
- **HE-4 零信任自我签收**：产出后立即 `./bin/gate-prd.py --prd <PRD>` 必须 exit 0；缺任何章节或缺 PII 脱敏策略 → 立即 FAIL 不许交付给 TD；
- **HE-5 Harness-as-Code**：PRD 首行必须包含 `<!-- PRD-SHA256: <sha256sum> -->`，作为 DESIGN/TEST/RELEASE BASE-PRD 锚点（HE-2）；

### TD (功能设计) 守则
- **HE-2 可复现锚点**：首行 10 行内必须写入 `<!-- BASE-PRD: PRD-<TICKET>.md@<sha256-of-PRD> -->`，与 `sha256sum PRD` 完全相等；
- **HE-5 Harness-as-Code 技术栈锁**：架构选型必须读取 `sdlc/policies/<REPO>-tech-stack.lock.json` 白名单，**禁止引入 forbidden 语言/框架（例：在 Vault 中引入 Vue/Spring 直接 FAIL）**；
- **HE-1 分层**：接口契约 HTTP 必须 OpenAPI 3.0；gRPC/oRPC 必须带 proto/IDL；
- **HE-4 零信任自我签收**：`./bin/gate-design.py --design <DSG> --repo <repo>` exit 0；附录 A 评审 Checklist 必须 100% 勾选（不允许 - [ ] 留空）。

### CD (程序开发) 守则
- **Gitflow (HE-5)**：从 `develop` 切 `feature/<TICKET>-snake`；**禁止直 push main/master/develop**；
- **复用子仓 AGENTS.md 的 Makefile 目标（HE-5 禁止另起炉灶）**：
  - Go (FileBay)：`make fmt` → `make lint-go` → `make test` → `make tidy`（go.mod 变动时）
  - TS/React (Vault, Nexus FE)：`make lint-js` → `pnpm typecheck` → `pnpm test`
  - Java (Nexus BE)：`mvn checkstyle:check` → `mvn test`
  - Python (Desktop API)：`ruff check` → `pytest` → `coverage report`
  - Rust (Vault Runtime)：`cargo clippy -- -D warnings` → `cargo test`
- **HE-3 版权头 & 行尾空格**：新 `.go/.ts/.java/.py/.rs` 必须含当前年 copyright；所有源文件删除行尾 trailing whitespace；
- **HE-4 零信任自我签收**：`../sdlc/bin/gate-code.sh --repo .` exit 0 才能 PR/MR。

### QA (测试验收) 守则
- **HE-1 分层（四象限）**：功能/性能/安全/兼容 四象限 **必须全部覆盖**，输出 `sdlc/docs/TEST-<TICKET>.md` + 机器可读 `artifacts/test-report-<TICKET>.json`（[TEST-TEMPLATE.md](file:///Users/sevensimle/Documents/WorkSpace/CheersAI/sdlc/templates/TEST-TEMPLATE.md)）；
- **HE-6 反馈驱动进化**：P0 = 阻断发布 / P1 = 阻断发布 / P2 ≤ 3 遗留需签署延期；任何 FP(误判)/FN(漏判) 必须写入 `sdlc/audit/<date>/regression-harness-registry.jsonl` 反哺下轮 harness；
- **HE-4 零信任自我签收**：`./bin/gate-test.py --test-report artifacts/test-report.json --ticket xxx` exit 0；核心场景 P0 必须 100%；SAST/SCA 零 Critical / DAST 零 High。

### RO (发布运维) 守则
- **HE-1 分层灰度**：1% → 10% → 50% → 100% 四阶段；每阶段观察 ≥ 60min；错误率 < 0.1%；
- **HE-2 可复现回滚演练**：预演环境一键回滚脚本必须 **跑通 ≥1 次** 才允许进灰度；
- **HE-3 监控 + 告警 SLA**：RED/USE 双仪表盘齐全；P0 ≤ 15min 响应；P1 ≤ 60min；
- **HE-4 零信任自我签收**：`./bin/gate-release.py --release-log artifacts/release-vX.Y.Z.json` exit 0 才能宣布发布。

## 3. 审计日志格式规范（**HE-3 观测优先 + HE-2 不可篡改链**）

所有 G1-G5 成功/失败都必须由 [audit-writer.py](file:///Users/sevensimle/Documents/WorkSpace/CheersAI/sdlc/bin/audit-writer.py) 写入 `sdlc/audit/YYYYMMDD/_<session>.jsonl`，字段 **缺一则判定审计中断 → 下一 gate FAIL**：

```json
{
  "ticket": "IMP-001",
  "gate": "G1-PRD",
  "ts_unix": 1786721270,
  "actor_email": "sdlc-ra@cheersai.ai",
  "role": "RA",
  "artifact_sha256": "bd97ce9d8909a0b2ffbb6ed5777a6ba767b43495d7e5c8d971321da536e02245",
  "exit_code": 0,
  "evidence": {"prd_completeness_pct": 100.0, "prd_compliance_pct": 100.0},
  "prev_hash": "7f83b1657ff1fc53b92dc18148a1d65dfc2d4b1fa3d677284addd200126d9069"
}
```
- `prev_hash = sha256(上一条 JSON)`：保证链式不可篡改（任何一条被改 → 下一条 prev_hash 校验即破，整条链判定中断，HE-2）。

## 4. Fail-Fast 异常路径（**HE-4 零信任过站**）

任何 G1-G5 FAIL → **智能体必须**：
1. **立即停止下游产出**（绝不带病进入下一阶段）；
2. **生成** `sdlc/audit/<date>/FAIL-<TICKET>-<GATE>.md`：写明失败原因、证据、修复建议；
3. **从头重跑对应 G 步**：修复后必须从对应 G 步重新走，**禁止跳步**（例 G1 FAIL 修复后绝不能直接进入 G2）。

## 5. Harness-as-Code 变更流程（**HE-5 Harness 即代码**）

要改 `sdlc/gates/*.spec.yaml` 阈值 / 新增 `sdlc/harnesses/*.harness.yaml` / 改 `sdlc/bin/gate-*.py` **必须走以下流程，严禁智能体自行改动工装放水**：

1. 提交 PR 到 `sdlc/` 目录；
2. **双签审批**：至少 1 名 TD (技术设计岗) + 1 名 RO (发布运维岗) 双 Approve；
3. `./bin/verify-harness-integrity.py --regen-hashes` 把新 hash 写入 `sdlc/harnesses/_harness-integrity.manifest.json`；
4. `./bin/verify-harness-integrity.py` 在下次 `run-pipeline.sh` 启动时 **第一时间校验**，不匹配则立即 exit 非 0（防止本地改工装放水，HE-2/HE-5）。

## 6. Harness Coverage 阈值（**HE-7 Harness 覆盖率 ≥ 代码覆盖率**）

每个 Sprint (2 周) 内 `./bin/collect-harness-coverage.py` 必须报告：
- G1/G2/G3/G4/G5 5 个 Harness 的 **触发频次** & **首次捕获真缺陷数**；
- **触发率 ≥ 90%** 才算合格（即 5 门中至少 4~5 门在 Sprint 中被 ≥ 1 Ticket 触发）；
- 不合格必须在 Sprint 复盘会上说明原因 + 安排补齐 (HE-7)。

---
**最后红线**：违反本守则任一硬条款，智能体后续输出一律**视为违反 Harness Engineering 内控流程**，需 **TD + RO 双签豁免** 才能进入下一步。
