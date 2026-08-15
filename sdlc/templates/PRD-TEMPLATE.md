<!-- PRD-SHA256: <TBD by sha256sum before sign-off> -->
# PRD TEMPLATE - CheersAI-SDLC 标准需求文档
> 编号: PRD-<TICKET> | 版本: v1.0 | 负责人 (RA): <email> | 签署日期: <YYYY-MM-DD>

## 一、背景与目标（Business Context & Goals）
### 1.1 业务背景
### 1.2 目标（可量化 OKR）
- O: <Objective>
- KR1: <Key Result 可衡量>
- KR2:
### 1.3 非目标（Non-Goals, Out of Scope）
### 1.4 合规要求（来自 data-classification.yaml）
- 本 PRD 涉及最高数据分级: [C1 / C2 / C3 / C4]
- 涉及 PII 字段清单: <列出或写“无”>

## 二、用户故事与核心场景（User Stories & Scenarios）
### 2.1 用户画像
| 角色 | 描述 | 痛点 |
|---|---|---|
| | | |
### 2.2 用户故事 (As a … I want … so that …)
- US-01:
- US-02:
### 2.3 核心业务流程（Mermaid 流程图可选）
```
```

## 三、验收标准（Acceptance Criteria, AC 可量化可执行）
> 必须使用 Given-When-Then 格式，P0/Must 100% 通过才准进发布
### 3.1 功能验收
| ID | 优先级 | Scenario (Given/When/Then) |
|---|---|---|
| AC-01 | Must/P0 | |
| AC-02 | Should/P1 | |
### 3.2 性能验收
- P0 接口 QPS: ____  TP99 ≤ ____ ms
- 内存峰值 (浏览器/桌面): ≤ ____ MB
### 3.3 安全与合规验收
- SAST 0 Critical
- PII 字段全部按 C3/C4 要求：加密存储 + 脱敏展示 + 日志零明文
### 3.4 兼容性验收
- 浏览器矩阵: [Chrome 120+, Safari 17+, Edge 120+]
- 操作系统: [Windows 10+, macOS 13+, Ubuntu 22.04+]
- 分辨率 ≥ 1280×800

## 四、优先级（MoSCoW）
| 子功能 | 优先级 |
|---|---|
| F01  | Must  |
| F02  | Should|
| F03  | Could |
| F04  | Won't (本版本不做)|

## 五、依赖关系 & 上下游影响（Dependencies）
- 内部依赖：(其他模块/服务/数据)
- 外部依赖：(第三方 API/密钥/SSO)
- 风险 & 缓解：
  | 风险 | 概率 | 影响 | 缓解措施 |
  |---|---|---|---|

## 六、合规与数据分级（逐字段标注）
> 每条涉及的字段必须标注 C1-C4；C3+ 必须标注脱敏/加密策略
| 字段名 | 分类 | 加密存储 | 脱敏展示 | 日志是否允许明文 |
|---|---|---|---|---|
| | C1-C4 | Y/N | 策略 | Y/N |

## 七、验收通过证据清单（Gate 4 输入）
- [ ] 功能测试报告（含 P0 100%）
- [ ] 性能压测报告（达 §3.2）
- [ ] 安全扫描（SAST/SCA/DAST 零高危）
- [ ] 兼容性矩阵报告

## 附录 A：变更历史
| 版本 | 日期 | 修改人 | 修改说明 |
|---|---|---|---|
| v1.0 | | RA-<email> | 初稿 |
