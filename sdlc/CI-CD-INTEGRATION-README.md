# CheersAI-SDLC × GitHub / Gitea Actions 接入指南（CI/CD）

> **术语对齐**：本指南所有条款均严格对齐 `sdlc/README.md` 中的 **Harness Engineering 7 原则（HE-1 ~ HE-7）**，之前 v1.x 版本的「Hanessian」字样已统一更正为 **Harness Engineering**（参见 README §0 术语更正公告）。
>
> **不可篡改历史承诺**：`sdlc/audit/` 目录下 v1.x 的历史审计文件（含「Hanessian」字样）**一律保留不动**；所有新增 PR Gate 审计均从今日起使用新术语，链式哈希连续不中断。

---

## 0. 三条最高优先级红线（违规直接 CI 失败）

| 编号 | 红线 | 对齐的 Harness 原则 |
|---|---|---|
| 🔴 R1 | 严禁直接 push 到 `main/master`；所有改动必须走 **特性分支 → PR → 评审 → 合入**。 | HE-4 Zero-Trusted Pass-Thru + GitHub Flow 标准 |
| 🔴 R2 | 严禁在 CI 环境和本地使用**不同版本**的 check-sod.py / gate-*.py/sh；同一份性由 `verify-harness-integrity.py` SHA256 锁。 | HE-2 确定性可复现 + HE-5 Harness-as-Code |
| 🔴 R3 | 严禁单人兼任 4 种双岗组合：RA+TD / TD+CD / CD+QA / QA+RO，SoD 脚本在 CI 中严格阻断。 | HE-4 零信任 SoD 职责分离 |

---

## 1. 子仓接入两步走（5 个子仓通用）

适用于：**CheersAI-Vault / FileBay / Nexus / Desktop / SSO / Portal** 所有产品仓。

### 步骤 A：让 sdlc 脚本进入你的子仓（保证 CI / Local 同 SHA）

**方案 A（强烈推荐，符合 HE-5 Harness-as-Code 版本化）：git submodule**

```bash
# 在子仓根目录运行（一次即可）
cd ~/WorkSpace/CheersAI/CheersAI-Vault
git submodule add ../sdlc sdlc
git submodule update --init --recursive
git add .gitmodules sdlc && git commit -m "chore(sdlc): add central sdlc as submodule (HE-5)"
```

**方案 B（简单版，适合前期接入）：目录拷贝 + 锁 hash**

```bash
cd ~/WorkSpace/CheersAI/CheersAI-Vault
cp -R  ../sdlc ./sdlc
# 之后由 TD+RO 双签启用 strict 模式锁 hash
```

> 两种方案都必须：**保持 `sdlc/` 与中心 CheersAI/sdlc 一致**，否则 HE-2 `verify-harness-integrity --strict` 会 CI FAIL。

### 步骤 B：拷贝 GitHub / Gitea Actions 模板到子仓 `.github/workflows/`

```bash
# —— GitHub Actions ——
mkdir -p .github/workflows
cp sdlc/templates/github-actions.sdlc-pr-gate.yml .github/workflows/sdlc-pr-gate.yml

# —— Gitea Actions （私有部署 Gitea 时用）——
mkdir -p .gitea/workflows
cp sdlc/templates/gitea-actions.sdlc-pr-gate.yml .gitea/workflows/sdlc-pr-gate.yml
```

**推荐**：两个模板都存一份（GitHub 主仓 + Gitea 灾备镜像），二者 99% 语法兼容。

---

## 2. 启用 Strict 锁 hash（HE-2 / HE-5，TD + RO 双签流程）

初始接入时 workflow 默认 **`--warn-only` 过渡模式**（manifest 仍是 SEED 占位），正式启用严格锁请按以下流程走：

1. **在本地真实环境（非沙箱）** 生成真实 manifest hash：
   ```bash
   cd sdlc
   python3 bin/verify-harness-integrity.py --regen-hashes
   ```
2. **PR 申请变更 Harness-as-Code**：将更新后的 `sdlc/harnesses/_harness-integrity.manifest.json` 提交 PR，
   - Reviewers = **至少 1 位 TD + 至少 1 位 RO**（AGENTS.md §4.2 强制双签）
   - PR 描述中必须声明：「本次 Harness 变更不违反 HE-2 可复现，已通过所有回归脚本」
3. **合入后，在子仓 Settings 开启 strict**：
   | 平台 | 变量路径 | 变量名 | 值 |
   |---|---|---|---|
   | GitHub | 仓库 Settings → Secrets and variables → Actions → **Variables** → New repository variable | `SDLC_HARNESS_STRICT_ENABLED` | `true` |
   | Gitea | 仓库 Settings → Actions → Variables | `SDLC_HARNESS_STRICT_ENABLED` | `true` |

✅ 自此任何**本地私自篡改 gate-*.py/sh**（比如改阈值放水）→ CI 中 `verify-harness-integrity --strict` 立即 exit 非 0 → PR 阻断（HE-4 Zero-Trusted）。

---

## 3. 开启分支保护（GitHub / Gitea，强制阻断 R1 红线）

### GitHub — 仓库 Settings → Branches → Branch protection rules → Add rule

```
Branch name pattern: main
✅ Require a pull request before merging
   ✅ Required approvals: 1 （企业级合规强烈建议 2）
✅ Dismiss stale pull request approvals when new commits are pushed
✅ Require status checks to pass before merging
   ✅ 搜索并勾选所有以 `[SDLC]` 开头的 6 个 Job：
     - [SDLC] 0. Meta + Harness Integrity
     - [SDLC] 1. G1 PRD + G2 DESIGN
     - [SDLC] 2. SoD 职责分离
     - [SDLC] 3. G3-CODE
     - [SDLC] 4. G4-TEST + G5-RELEASE
     - [SDLC] 5. 汇总 → PR 评论 → 上传审计
✅ Do not allow bypassing the above settings
✅ Restrict pushes that create matching branches（没人能直接 push main）
```

### Gitea — Settings → Branches → Protected Branches → Add

```
Branch: main
✅ Enable push whitelist（whitelist = <空>  → 没人能直接 push）
✅ Enable status check
   ✅ Require: sdlc-meta-and-integrity / sdlc-sod / sdlc-g3-code / sdlc-summary-and-upload
✅ Required Approvals: 1~2
✅ Block merge if all required checks are not passed
```

---

## 4. 日常工作流（一条命令开分支 + 一条命令提 PR，全量 SoD + 审计链）

### 4.1 开特性分支（替代手工 git checkout -b）

```bash
# 语法： sdlc/bin/git-create-feature-branch.sh --ticket IMP-XXX --type feature|fix|… --role XX --actor xx@ --topic xxx
cd CheersAI-Vault
../sdlc/bin/git-create-feature-branch.sh \
  --ticket IMP-001 \
  --type feature \
  --role CD \
  --actor sdlc-cd@cheersai.ai \
  --topic address-pii-masking-rule \
  --reviewer cheersai-td,cheersai-qa
```

✅ 自动：
1. 切回 main → pull 最新（防止分支基于老代码产生冲突）；
2. 创建分支 `feature/IMP-001-address-pii-masking-rule`（若远端有顶层 `feature` 分支 → 自动回退 `feature-IMP-001-…`）；
3. push -u origin 建立 upstream；
4. 写入链式哈希：`audit/YYYYMMDD/_*.jsonl` 的 **BIZ-BRANCH** 事件（base_sha / branch / actor / role）。

### 4.2 提交代码 & commit 审计链（可选）

把 `sdlc/templates/post-commit.sample` 软链到所有子仓的 `.git/hooks/post-commit`：

```bash
# 方式一：全局统一（所有仓库自动生效）
mkdir -p ~/.config/git/hooks
cp sdlc/templates/post-commit.sample ~/.config/git/hooks/post-commit
chmod +x ~/.config/git/hooks/post-commit
git config --global core.hooksPath ~/.config/git/hooks
```

✅ 好处：每一次 `git commit` → 自动写入 **BIZ-COMMIT** 事件（commit_sha、additions/deletions、ticket 分支自动提取、role 按邮箱前缀推断）→ 与 BIZ-BRANCH + BIZ-PR 形成**三元组全回溯**（需求落地 2.3 回溯审计能力要求）。

### 4.3 提 PR（替代手工 gh pr create 或网页点）

```bash
../sdlc/bin/git-auto-create-pr.sh \
  --ticket IMP-001 \
  --actor  sdlc-cd@cheersai.ai \
  --role   CD \
  --reviewer cheersai-td,cheersai-qa \
  --draft
```

✅ 自动：
1. SoD 预检查（如果当前邮箱之前已登记 QA → CD 角色冲突 → FAIL）；
2. Conventional Commit 标题推导：`feat(vault): IMP-001 新增地址脱敏规则`；
3. 写入 **BIZ-PR-PREPARE** + **BIZ-PR** 两条链式哈希；
4. 优先 gh pr create（需 `gh auth login`）→ 失败 fallback：输出 https://github.com/.../compare/... URL 或 GitHub MCP create_pull_request 参数包。

---

## 5. PR 提完会发生什么？（Actions 工作流详细说明）

| Job 序号 | Job 名称 | 对应脚本 | 对齐 Harness |
|---|---|---|---|
| 0 | Meta + Integrity | `verify-harness-integrity.py` | **HE-2 可复现 + HE-5 Harness-as-Code**（CI 脚本 = 本地同 SHA，核心要求 1）|
| 1 | G1 PRD + G2 DESIGN | `gate-prd.py` / `gate-design.py` | HE-1 Fail-Fast + HE-2 BASE-PRD sha 锚定 |
| 2 | SoD 职责分离 | `check-sod.py` | HE-4 四禁组合 RA+TD / TD+CD / CD+QA / QA+RO |
| 3 | G3 CODE | `gate-code.sh` | HE-5 复用子仓 Makefile lint/test（**不另起炉灶**）|
| 4 | G4 TEST + G5 RELEASE | `gate-test.py` / `gate-release.py` | HE-3 观测 + HE-6 反馈驱动回归 |
| 5 | 汇总 → PR 评论 → 上传 artifact | `actions-comment-pull-request@v2` + upload-artifact | **HE-3 可回溯**：每条 9 字段 JSONL 链式哈希 |

### 你会在 PR 页面看到什么？

1. **Status checks**：6 个 Job 红 / 绿，红的一个不通过 → Require status checks 自动阻止 Merge；
2. **一条自动化评论（`sdlc-pr-gate-report`）**：含 Ticket、Run ID、各步骤对齐 Harness 原则、审计链 3 条 JSONL 节选。
3. **Artifacts**：点进 Actions 详情页下载 `sdlc-audit-PR-XXX.zip`，里面是完整的 `audit/YYYYMMDD/_*.jsonl`，可独立验链。

---

## 6. 验证类交付（需求 2.3 两类落地验证操作指南）

### ▶️ 类 1：本地 vs CI 脚本同 SHA 一致性验证

```bash
# —— 本地（你 laptop 上）——
cd CheersAI-Vault/sdlc
python3 bin/verify-harness-integrity.py --warn-only 2>&1 | tail -10
# 期望输出：[verify-harness-integrity] ✅ Harness integrity PASS (strict=False)

# —— 给 CI 生成一份快照，记录每个脚本 sha256 ——
python3 <<'PY' > ~/local-hashes.txt
import hashlib, pathlib
root = pathlib.Path('bin')
for f in sorted(root.glob('*.py')):
    h=hashlib.sha256(f.read_bytes()).hexdigest()
    print(f"{f.as_posix()}  {h}")
for f in sorted(root.glob('*.sh')):
    h=hashlib.sha256(f.read_bytes()).hexdigest()
    print(f"{f.as_posix()}  {h}")
PY

# —— CI：查看 workflow Job 0 的输出（Harness Integrity 段）——
# 如果 strict=true 没有 FAIL = 本地 & CI 已锁定同 SHA
#（manifest 是 TD+RO 双签的 hash 白名单，两者一致）
```

### ▶️ 类 2：全流程回溯审计能力验证（三元组 + 哈希链）

```bash
cd ~/WorkSpace/CheersAI/sdlc
TODAY=$(date +%Y%m%d)
FILE=$(ls -t audit/$TODAY/_*.jsonl 2>/dev/null | head -n 1)
echo "审计链文件: $FILE"
echo "=== 统计 3 类业务事件 ==="
python3 -c "
import json, sys
from collections import Counter
rows=[json.loads(l) for l in open(sys.argv[1]) if l.strip()]
cnt=Counter(r['gate'] for r in rows)
print('事件计数:', dict(cnt))
print('总记录数:', len(rows))
print('涉及 tickets:', sorted({r['ticket'] for r in rows}))
" "$FILE"

echo "=== 校验 prev_hash 链式连续性（任何断链 = 审计链被篡改 HE-3 FAIL）==="
python3 -c "
import json, hashlib, sys
rows=[json.loads(l) for l in open(sys.argv[1]) if l.strip()]
ok=True
for i,r in enumerate(rows):
    if i==0:
        if r.get('prev_hash') not in ('', None, 'genesis'): ok=False; break
        continue
    expected=hashlib.sha256(json.dumps(rows[i-1],ensure_ascii=False,sort_keys=True).encode('utf-8')).hexdigest()
    if r.get('prev_hash')!=expected:
        print(f'断链 @ row {i} [{r[\"gate\"]}] ticket={r[\"ticket\"]}')
        print(f'  expected: {expected}')
        print(f'  actual  : {r.get(\"prev_hash\")}')
        ok=False
print('审计链完整性:', 'PASS ✅' if ok else 'FAIL ❌ (疑似被篡改，请立即回滚 audit 到上一个 Git 版本)')
" "$FILE"
```

**三个事件必须全部存在于今天的 audit**：

```
BIZ-BRANCH   → 特性分支何时、被谁、claim 什么角色创建
BIZ-COMMIT   → 每次 commit sha / 文件变更统计
BIZ-PR       → PR 编号、URL、reviewer、base/head 分支
BIZ-PR-PREPARE → PR 创建前 SoD 预检查结果
```

缺少任何一类 → 在 `sdlc/bin/audit-writer.py` 中打开 debug 或检查 post-commit hook 是否被正确软链。

---

## 7. 失败排查速查表

| 现象 | 可能原因 | 修复 |
|---|---|---|
| Integrity FAIL `mismatch: bin/check-sod.py` | 本地改了脚本未锁 hash / CI 用的 manifest 与本地不同 | 回退你的手工改；按 §2 走 TD+RO 双签 PR regen |
| SoD FAIL `double role: TD+CD` | 你既是 TD 审设计又作为 CD 提代码 | 换一个同事做 TD review / 你自己只担任单一角色 |
| G3 CODE 找不到 `make lint-js` | 子仓没有 Makefile，但 `package.json` 有 lint 脚本 | 在子仓 AGENTS.md 补 make lint-js 或直接 PR 给中心 gate-code.sh 增强 |
| G4 TEST FAIL SAST Critical=3 | 代码中被 Semgrep 扫描出 3 个高危问题 | 修复后再 push，或在 SAST 中加合规的 suppress（要有理由）|
| PR 评论看不到 SDLC 报告 | 检查 workflow Job 5 summary 步骤；`thollander/actions-comment-pull-request@v2` 需要 pull-requests:write 权限 | 在 workflow 顶部 permissions 段补 `pull-requests: write` |
| 审计链完整性 FAIL ❌ 断链 | 有人手改了 audit JSONL | Git 恢复 `sdlc/audit/` 到上一次 commit（审计链不可篡改，HE-3）|

---

## 8. 升级中心 sdlc 时怎么办？（HE-5 Harness-as-Code 变更 SOP）

> 本 SOP 与 `sdlc/AGENTS.md §4.2 Harness-as-Code 变更流程**完全一致**。

1. **TD 起草 PR**（只改 sdlc/bin / gates / harnesses / templates，不动 audit/）；
2. 本地执行：
   ```bash
   python3 sdlc/bin/verify-harness-integrity.py --regen-hashes   # 更新 manifest hash
   ```
3. PR Reviewers = **TD（同行审）+ RO（发布审）** 双签 + 再附加一位 QA 抽测脚本；
4. 所有子仓如果是 submodule：合入后各仓执行：
   ```bash
   cd CheersAI-Vault && git submodule update --remote sdlc && git add sdlc && git commit -m "chore(sdlc): bump sdlc submodule to latest"
   ```
   如果是拷贝方案：重跑 cp 覆盖 + 再 `--regen-hashes`；
5. **合入后次日 Sprint 复盘**：核对 HE-7 Harness Coverage ≥ 90%（触发率与首次捕获缺陷数）。

---

## 9. 交付物清单（落地后即可见）

| 类型 | 位置 | 说明 |
|---|---|---|
| 模板 | `sdlc/templates/github-actions.sdlc-pr-gate.yml` | GitHub Actions 模板 |
| 模板 | `sdlc/templates/gitea-actions.sdlc-pr-gate.yml` | Gitea Actions 模板 |
| 脚本 | `sdlc/bin/git-create-feature-branch.sh` | 开分支（GitHub Flow + 审计） |
| 脚本 | `sdlc/bin/git-auto-create-pr.sh` | 提 PR（Conventional + SoD + 审计）|
| Hook 模板 | `sdlc/templates/post-commit.sample` | commit 钩子（BIZ-COMMIT 审计）|
| 本文 | `sdlc/CI-CD-INTEGRATION-README.md` | 本接入指南 |
