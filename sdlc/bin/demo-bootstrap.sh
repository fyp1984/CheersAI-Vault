#!/usr/bin/env bash
# demo-bootstrap.sh — 生成本次端到端演示的最小 PRD/DESIGN 文档（address 内置 PII 规则）
set -euo pipefail

SDLC_ROOT="$(cd "$(dirname "$0")/.."; pwd)"
DOCS_DIR="$SDLC_ROOT/docs"
ART_DIR="$SDLC_ROOT/artifacts"
mkdir -p "$DOCS_DIR" "$ART_DIR"

# 命令行参数解析（bash 3.2 兼容）
while [[ $# -gt 0 ]]; do
  case "$1" in
    --ticket)    TICKET="$2";      shift 2 ;;
    --repo-name) REPO_NAME="$2";   shift 2 ;;
    --docs)      DOCS_DIR="$2";    shift 2 ;;
    --artifacts) ART_DIR="$2";     shift 2 ;;
    *)           shift ;;
  esac
done

TICKET="${TICKET:-IMP-001}"
PRD_NAME="PRD-${TICKET}-address-masking-rule.md"
DSG_NAME="DESIGN-${TICKET}-address-masking-rule.md"

cd "$SDLC_ROOT"

# ---------- 1) PRD 写入（用 python 避免 bash heredoc 中特殊字符解析）----------
python3 -u - "$DOCS_DIR/$PRD_NAME" "$TICKET" <<'PYEOF'
import sys, pathlib
dst = pathlib.Path(sys.argv[1])
ticket = sys.argv[2]
content = f'''<!-- PRD-SHA256: PLACEHOLDER -->
# PRD {ticket} · CheersAI-Vault 「住址类结构性 PII」内置脱敏规则落地
编号: PRD-{ticket} | 版本: v1.0 | 负责人 (RA): sdlc-ra@cheersai.ai | 签署日期: 2026-08-14

## 一、背景与目标
### 1.1 业务背景
CheersAI-Vault 个人版 Runtime 内置 7 条规则（bank_card/email/id_card/ipv4/passport/phone/use_sensitive_terms）
唯独缺少「住址 / address」结构性 PII 识别规则，用户上传 MD/TXT 时，默认勾选所有规则也会把完整住址明文输出，
与产品 PRD 约定的"6 类个人敏感信息全部开箱即用"不一致，已于 2026-08-09 回归测试中以 DEFECT-ADDR-001 / IMP-01 形式首次报出。
### 1.2 目标（可量化 OKR）
- O: 在 CheersAI-Vault Runtime 中补齐「中国住址结构化识别」内置规则。
- KR1: 标准 GFM / 内嵌 HTML / 超长 MD（≥260KB）三类文档中，默认规则选中 address 后，
       「省/市/区/路/号/楼/室」完整住址识别准确率 ≥ 99%（人工验收）；
- KR2: 对手机号码 / 身份证号 / 邮箱等 5 类已通过 PII 不产生任何新增误识别（误识别率 = 0）；
- KR3: 反脱敏 round-trip 对 address 标记支持，内容一致性 100%（字节级）；
- KR4: 性能：300KB 文件全规则脱敏耗时 ≤ 1.5s baseline，TP99 不超过 baseline 的 110%。
### 1.3 非目标
- 本 PRD 不包含境外地址（US/EU 等）识别；下个版本按需加；
- 不引入 NER 模型；本版本使用「31 省市区划关键字 + 路/号/楼/室 + 正则」工程化方案，避免模型依赖膨胀；
- 不涉及 Nexus / FileBay / Desktop 的改动，仅限 Vault Runtime + 前端 rules 清单展示。
### 1.4 合规要求
- 本 PRD 涉及最高数据分级：C3_SENSITIVE（家庭住址属于个人敏感 PII）
- 涉及 PII 字段清单：家庭住址、办公地址、身份证住址字段；按 C3：加密存储 + 默认脱敏展示 + 日志零明文。

## 二、用户故事与核心场景
### 2.1 用户画像
| 角色 | 描述 | 痛点 |
|---|---|---|
| 个人版普通用户 | 使用 Vault 脱敏 MD 后分享给协作方 | 默认勾选全部规则仍然把住址明文泄出，合规风险 |
| 合规官（企业版未来用户） | 审计 PII 识别覆盖率 | 目前只能用敏感词库手工录入地址，效率极低 |
### 2.2 用户故事
- US-01: As a 个人版用户，I want 勾选「全部规则」时 住址也自动被脱敏，so that 我无需额外维护敏感词库即可分享文档；
- US-02: As a 合规官，I want 脱敏结果中仅保留 ADDRESS seq 标记，so that 完全不可逆；
- US-03: As a 文档作者，I want 反脱敏时 address 标记 100% 还原原文，so that 不丢失内容与格式；
### 2.3 核心业务流程
(code block flow)
用户上传 MD → 勾选默认规则（新增 address） → create_preview → Runtime mask address → Ready
        ↓ confirm → 下载 .masked.md → restore → 原文 100% 一致
(code block end)

## 三、验收标准（AC，可量化）
### 3.1 功能验收
| ID | 优先级 | Scenario (Given/When/Then) |
|---|---|---|
| AC-01 | Must/P0 | Given 规则列表 /api/v1/rules 被请求；When 响应；Then 其中必须包含 id=address, class=pii_c3, enabled_by_default=true 条目 |
| AC-02 | Must/P0 | Given 10 条中国标准格式住址样例（含 省/市/区/路/号/楼/室 不同组合）；When 预览完成；Then 10/10 全部被识别为 address 且 mask 成标记 |
| AC-03 | Must/P0 | Given 回归测试 5 类 PII 标准 MD（手机/身份证/邮箱/银行卡/护照）；When 启用 address 规则并预览；Then 原 5 类 PII 的数量与 mask 标记与 baseline 完全一致（0 新增误识别）|
| AC-04 | Must/P0 | Given 已脱敏的 address 标记文件；When 执行 restore；Then 字节级与原文相等（与 phone/id_card 同类规则一致）|
| AC-05 | Should/P1 | Given 300KB 超长 MD 含 800+ address 样例；When 预览；Then 总耗时 ≤ baseline * 1.1 且 OOM = 0 |
| AC-06 | Could/P2 | Given 不完整住址（仅"北京市海淀区"缺路/号）；When 预览；Then 至少识别到区级别（允许部分匹配，但不得误识别其他内容）|
### 3.2 性能验收
- 300KB 全规则脱敏 QPS ≥ baseline（baseline 以 2026-08-09 回归 IMP-001 前为准）
- TP99 ≤ baseline * 110%
### 3.3 安全与合规验收
- SAST 0 Critical；SCA 0 Critical；DAST 影子流量 0 High；
- 日志中不输出任何 address 明文或标记对应的 mapping 原文；
- mapping 仅保存在服务端 runtime 的 artifact restore 映射中。
### 3.4 兼容性验收
- 浏览器矩阵: Chrome 120+, Safari 17+, Edge 120+ → 规则勾选 正常 显示「地址」中文文案；
- 操作系统: Windows 10+, macOS 13+, Ubuntu 22.04+ → Tauri 与 Browser 双宿主正常；

## 四、优先级（MoSCoW）
| 子功能 | 优先级 |
|---|---|
| F01 Runtime 规则 /rules 列表新增 address 条目 | Must |
| F02 Runtime mask 引擎：中国地址正则 + 区划关键字识别 | Must |
| F03 Runtime restore：address mapping 写入 cmap，restore 100% | Must |
| F04 前端 rules 清单 address 中文展示为「地址 / 住址」 | Should |
| F05 不完整住址（区级别）兜底识别 | Could |
| F06 多语言地址 / 海外地址识别 | Wont（本版）|

## 五、依赖关系 & 上下游影响
- 内部依赖：
  - Vault Runtime rules.rs / masker.rs / restore.rs 已有的规则扩展点
  - 前端规则展示 [rules.ts 类型文件](file:///Users/sevensimle/Documents/WorkSpace/CheersAI/CheersAI-Vault/src/types/rules.ts)
- 外部依赖：无
- 风险 & 缓解：
  | 风险 | 概率 | 影响 | 缓解 |
  |---|---|---|---|
  | address 正则过宽 → 误识别普通句子为地址 | 中 | PII mask 误删 → 影响可读性 | 区划关键字必须命中 + 必须伴随「路/号/楼/室/街道」后缀；误识别黑名单见 DESIGN §5 |

## 六、合规与数据分级（逐字段）
| 字段名 | 分类 | 加密存储 | 脱敏展示 | 日志明文？|
|---|---|---|---|---|
| 省份 | C3_SENSITIVE | 是 | 是 | 否 |
| 城市 | C3_SENSITIVE | 是 | 是 | 否 |
| 区县 | C3_SENSITIVE | 是 | 是 | 否 |
| 路/号/楼/室 | C3_SENSITIVE | 是 | 是 | 否 |
| address mapping | C3_SENSITIVE | 是（随 artifact 加密）| 反脱敏接口需要权限校验 | 否 |

## 七、附录
### 7.1 术语
- C3_SENSITIVE：CheersAI 数据分级中「个人敏感 PII」，要求加密存储 + 默认脱敏展示 + 日志零明文。
- Round-trip：脱敏 → 反脱敏字节级一致的端到端能力。
### 7.2 修订记录
| 版本 | 日期 | 修改人 | 说明 |
|---|---|---|---|
| v1.0 | 2026-08-14 | sdlc-ra@cheersai.ai | 初稿，与 2026-08-09 回归测试 IMP-01 对齐 |
'''
dst.parent.mkdir(parents=True, exist_ok=True)
dst.write_text(content, encoding="utf-8")
PYEOF

# 把 SHA256 占位替换掉
SHA=$(python3 -u - "$DOCS_DIR/$PRD_NAME" <<'PY'
import hashlib, pathlib, re, sys
p = pathlib.Path(sys.argv[1])
b = p.read_bytes()
sha = hashlib.sha256(b).hexdigest()
text = b.decode("utf-8")
new = re.sub(r"<!-- PRD-SHA256: PLACEHOLDER -->", f"<!-- PRD-SHA256: {sha} -->", text, count=1)
try:
    if new.encode() != b:
        p.write_text(new, encoding="utf-8")
        sha = hashlib.sha256(p.read_bytes()).hexdigest()
except PermissionError:
    pass
print(sha)
PY
)

echo "[demo-bootstrap] PRD_SHA=$SHA → $DOCS_DIR/$PRD_NAME"

# ---------- 2) DESIGN 写入（必须 BASE-PRD 锚点引用上面 SHA）----------
python3 -u - "$DOCS_DIR/$DSG_NAME" "$PRD_NAME" "$SHA" "$TICKET" <<'PYEOF'
import sys, pathlib
dst = pathlib.Path(sys.argv[1])
prd_name = sys.argv[2]
prd_sha  = sys.argv[3]
ticket   = sys.argv[4]
content = f'''<!-- BASE-PRD: {prd_name}@{prd_sha} -->
# DESIGN {ticket} · 住址结构性 PII 规则落地 技术设计
编号: DESIGN-{ticket} | 版本: v1.0 | 负责人 (TD): sdlc-td@cheersai.ai | 签署: 2026-08-14

## 0. 基准 PRD 锚点
- PRD 文件名: {prd_name}
- PRD SHA256: {prd_sha}

## 一、架构选型
### 1.1 架构决策（ADR）
(code block ADR)
ADR-IMP001-01: Address 识别方案 = 省市区划关键字 + 后缀 路/号/楼/室 正则级联，而非 NER 模型
Context: 用户要求「个人版开箱即用」，不能引入 100MB+ 模型权重（否则 Tauri 包体膨胀）
Decision: 用纯 Rust 正则 + 编译期 include_str 嵌入 ~13KB 的 31 省市区划表
Consequences: Pro: 包体零膨胀，1ms 内完成编译；Con: 对不规范地址识别覆盖率略低于 NER，已在 Could 中标记缓解
(code block end)
### 1.2 技术栈一致性（白名单内）
| 依赖 | 版本 | 用途 | 在白名单? |
|---|---|---|---|
| Rust regex crate (已有) | 1.10+ | address 正则级联 | 是 Runtime 已用 |
| include_str / phf (已有) | - | 编译期区划表嵌入 | 是 |
| 无新增前端依赖 | - | 仅增加一条文案翻译 | 是 |
### 1.3 整体架构图
(code block diagram)
VaultBrowserUI --(勾选 rules 含 address)--> 4173 Proxy --> RustRuntime:8787
                                         [masker.rs + rules::address]
                                              |
                                     PreviewReady --> confirm --> artifact+cmap(含 address mapping)
(code block end)

## 二、模块拆分 & 职责边界
| 模块 | 职责 | 依赖 |
|---|---|---|
| (新增) runtime/src/rules/address.rs | 编译期区划常量 + REGEX_ADDRESS + is_address_candidate(text) 判定；黑名单排除 | regex / phf |
| (改)  runtime/src/masker.rs | 把 AddressRule 注册进 ALL_RULES；扫描时调用 | rules::address |
| (改)  runtime/src/restore.rs | address 类 mapping 写入 cmap 时同 phone 类（key 同格式） | cmap / artifact |
| (改)  runtime/src/api/rules.rs | /rules 接口新增 id=address, class=pii_c3, enabled_by_default=true | rules 列表 |
| (改)  CheersAI-Vault/src/types/rules.ts | RuleId union 追加 address；i18n 文案「地址 / 住址个人敏感」 | 无新依赖 |

## 三、接口契约
### 3.1 HTTP API 增量契约
GET /api/v1/rules 响应新增（已存在的 7 条保留）：
(yaml block)
- id: address
  class: pii_c3
  enabled_by_default: true
  display_name:
    zh-CN: 地址 / 住址
    en-US: Residential / Office Address
  description: 识别中国标准格式地址（省市区+路号楼层室）
(yaml block end)
其它 HTTP 接口均 不改变 request/response schema（保持向后兼容）。

## 四、数据库 & 存储
- 无新增表 / 无变更 migration；address mapping 复用现有的 cmap 结构（String → String），无需升级数据库协议。

## 五、资源评估
| 资源 | 当前基线（不含 address 规则） | 本改动预期 | 原因 |
|---|---|---|---|
| Runtime 二进制大小 | ~13MB (release) | +40KB 量级 | 31 省市区划表 ~13KB + regex dfa ~20KB |
| 首次加载时间（浏览器 fetch Runtime） | ~220ms | +<5ms | include_str 编译期嵌入 |
| 300KB MD 扫描 CPU | ~480ms baseline | +10~40ms | 多出一次 regex 扫描 pass（与其它规则在同 pass 中短路，不做串行多 pass）|
| 内存峰值 | ~25MB | +<1MB | 无 |

## 六、风险预案
| 风险 | 等级 | 触发条件 | 预案动作 |
|---|---|---|---|
| 误识别「2025 年 5 月 1 日」为地址（含数字 +「号」） | 高 | 正则过宽 | 必须同时命中 省/市/区/县/自治区/特别行政区 任一 前缀关键字；否则不认为是地址 |
| 「XX 路」与「XX 路公交站」误识别 | 中 | 普通句末带路 | 黑名单词：公交站/地铁站/路口/路口南/路线 出现在后缀 → 排除 |
| 超长地址（> 200 字符）导致 regex 回溯 | 低 | 极端用户构造 | regex 限定 max(200 bytes)；使用非回溯组 "(?:…)" |

## 七、可行性 PoC 验证结果
- PoC 分支: poc-imp-001-address-regex
- 实验样例：10 条标准住址 + 30 条普通新闻句子；
- 结果：10/10 标准地址识别，30 条普通句子 0 误识别（exit_code=0，见 audit 日期目录下的 poc-imp001.log）
- 结论：满足 DESIGN 可落地前置要求。

## 附录 A：评审 Checklist（已勾选）
- [x] 未引入禁用语言/框架
- [x] 接口契约语法正确（OpenAPI 片段）
- [x] C3 住址字段：加密存储 + 脱敏 + 日志零明文 策略均明确
- [x] 所有依赖已列白 & 版本锁定
- [x] 上游 Runtime rules/ 下游前端 rules.ts 接口 owner 已对齐
'''
dst.parent.mkdir(parents=True, exist_ok=True)
dst.write_text(content, encoding="utf-8")
PYEOF

echo "[demo-bootstrap] OK：PRD=$DOCS_DIR/$PRD_NAME  DESIGN=$DOCS_DIR/$DSG_NAME"
