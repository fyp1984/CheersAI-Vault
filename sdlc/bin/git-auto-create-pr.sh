#!/usr/bin/env bash
# git-auto-create-pr.sh — 自动创建 GitHub/Gitea Pull Request（严格 GitHub Flow + SoD 校验 + 审计链）
#
# 强制规则（违反 → exit 非 0）：
#   1) 不能在 main/master 上提 PR；必须在特性分支（feature/fix/...）且有 upstream；
#   2) PR title 必须遵循 Conventional Commit：feat/fix/refactor/chore/docs/test(scope): IMP-002 summary；
#   3) 提 PR 前运行 SoD 预检查（HE-4：CD 提代码 → QA 必须另外的人执行）；
#   4) PR 正文使用 .github/PULL_REQUEST_TEMPLATE.md，缺失则使用 git-feature-pr-flow Skill 模板。
#
# 链式哈希审计：
#   调用两次 audit-writer：BIZ-PR-PREPARE（创建前）+ BIZ-PR（创建成功后，含 PR URL & 编号）
set -euo pipefail

SDLC_ROOT="$(cd "$(dirname "$0")/.."; pwd)"

usage() {
  cat <<'EOF'
用法: git-auto-create-pr.sh --ticket <TICKET> --actor <xx@cheersai.ai>
                            --role <RA|TD|CD|QA|RO>
                            [--scope <module>]
                            [--type <feat|fix|refactor|chore|docs|test>]
                            [--summary <一句话摘要>]
                            [--body-file <path.md>]
                            [--draft]
                            [--reviewer <gh-user1>,<gh-user2>]
                            [--base <main|master>]
                            [--repo-root <path>]
                            [--fallback-print-only]
示例:
  # 自动推导 type/scope/summary（从最近 1 条 commit message）
  git-auto-create-pr.sh --ticket IMP-001 --actor sdlc-cd@cheersai.ai --role CD --reviewer cheersai-td,cheersai-qa --draft
说明:
  * 默认用 gh pr create 提 PR（需要 gh auth login / GH_TOKEN）。
  * 如果 gh 不可用：开启 --fallback-print-only 只打印 PR URL & 命令模板，配合 GitHub MCP 调用即可。
  * 创建成功 → audit 记录 BIZ-PR 事件（pr_url/base/head/ticket/reviewer）
EOF
}

TICKET="" ACTOR="" ROLE="CD" SCOPE="" TYPE_AUTO="" SUMMARY="" BODY_FILE="" DRAFT="" REVIEWERS="" BASE="main" REPO_ROOT="" FALLBACK=0
while [[ $# -gt 0 ]]; do
  case "$1" in
    --ticket)             TICKET="$2";          shift 2;;
    --actor)              ACTOR="$2";           shift 2;;
    --role)               ROLE="$2";            shift 2;;
    --scope)              SCOPE="$2";           shift 2;;
    --type)               TYPE_AUTO="$2";       shift 2;;
    --summary)            SUMMARY="$2";         shift 2;;
    --body-file)          BODY_FILE="$2";       shift 2;;
    --draft)              DRAFT="--draft";      shift;;
    --reviewer)           REVIEWERS="$2";       shift 2;;
    --base)               BASE="$2";            shift 2;;
    --repo-root)          REPO_ROOT="$2";       shift 2;;
    --fallback-print-only)FALLBACK=1;           shift;;
    -h|--help)            usage; exit 0;;
    *) echo "Unknown arg: $1"; usage; exit 2;;
  esac
done

# ===== 参数校验 =====
case "$ROLE" in RA|TD|CD|QA|RO) ;; *) echo "::error::--role 必须在 RA/TD/CD/QA/RO 中 (HE-4)"; exit 3;; esac
[[ -z "$TICKET" ]] && { echo "::error::--ticket 必填"; exit 3; }
[[ -z "$ACTOR"  ]] && { echo "::error::--actor 必填（企业邮箱，audit-writer 使用）"; exit 3; }

# ===== 切 repo =====
if [[ -n "$REPO_ROOT" ]]; then cd "$REPO_ROOT" >/dev/null; else cd "$SDLC_ROOT/.." >/dev/null; fi
if ! git rev-parse --is-inside-work-tree >/dev/null 2>&1; then echo "::error::不是 git 仓库"; exit 4; fi
REPO_ABS="$(git rev-parse --show-toplevel)"; cd "$REPO_ABS" >/dev/null
REPO_NAME="${REPO_ABS##*/}"

# ===== 必须在非 base 分支且已 upstream =====
HEAD="$(git symbolic-ref --short HEAD 2>/dev/null || echo DETACHED)"
if [[ "$HEAD" == "DETACHED" || "$HEAD" == "$BASE" ]]; then
  echo "::error::当前分支=$HEAD（或 detached），不能提 PR。必须在特性分支（feature/fix/...）上。"
  exit 5
fi
# 必须有 upstream
if ! git rev-parse --abbrev-ref --symbolic-full-name @{u} >/dev/null 2>&1; then
  echo "::error::当前分支没有 upstream。请先执行：git push -u origin $HEAD"
  exit 6
fi
HEAD_SHA="$(git rev-parse HEAD)"

# ===== 预检查：SoD =====
echo "▶ [HE-4] 预检查 SoD 四禁组合"
python3 "$SDLC_ROOT/bin/check-sod.py" --ticket "$TICKET" || {
  echo "::error::SoD 预检查失败 — 当前 actor=$ACTOR 被登记为 $ROLE，但历史 audit 显示其兼任了禁止组合。请更换角色。"
  exit 7
}

# ===== 推导 Conventional Commit 标题 =====
if [[ -z "$TYPE_AUTO" ]]; then
  case "$HEAD" in
    feature/*|feature-*) TYPE_AUTO="feat" ;;
    fix/*|fix-*)         TYPE_AUTO="fix"  ;;
    refactor/*|refactor-*) TYPE_AUTO="refactor" ;;
    chore/*|chore-*)       TYPE_AUTO="chore" ;;
    docs/*|docs-*)         TYPE_AUTO="docs" ;;
    test/*|test-*)         TYPE_AUTO="test" ;;
    *)                     TYPE_AUTO="feat" ;;
  esac
fi
if [[ -z "$SCOPE" ]]; then
  # scope = 仓库名（去掉 CheersAI- 前缀）
  SCOPE="$(printf '%s' "$REPO_NAME" | sed 's/^CheersAI-//' | tr '[:upper:]' '[:lower:]')"
fi
if [[ -z "$SUMMARY" ]]; then
  # 从最近 1 条 commit 提取 subject
  SUMMARY="$(git log -1 --pretty=%s | sed -E 's/^[a-z]+(\([^)]+\)):\s*//' | head -c 80)"
fi
[[ -z "$SUMMARY" ]] && SUMMARY="ticket ${TICKET} 开发交付"
PR_TITLE="$TYPE_AUTO($SCOPE): $TICKET $SUMMARY"

# ===== BIZ-PR-PREPARE 预注册 audit =====
PRE_EVIDENCE="$(python3 - <<PY
import json
print(json.dumps({
  "repo": "$REPO_NAME",
  "head_branch": "$HEAD",
  "head_sha": "$HEAD_SHA",
  "base": "$BASE",
  "pr_title": "$PR_TITLE",
  "draft": bool("$DRAFT"),
  "reviewers": [r for r in "$REVIEWERS".split(",") if r],
  "sod_precheck_ok": True,
  "actor": "$ACTOR",
  "role_claim": "$ROLE",
}, ensure_ascii=False))
PY
)"
python3 "$SDLC_ROOT/bin/audit-writer.py" \
  --ticket "$TICKET" --gate "BIZ-PR-PREPARE" \
  --actor_email "$ACTOR" --role "$ROLE" \
  --artifact "$REPO_ABS" --exit_code 0 \
  --evidence "$PRE_EVIDENCE"

# ===== 生成 PR body =====
TEMPLATE=""
for p in ".github/PULL_REQUEST_TEMPLATE.md" "$SDLC_ROOT/templates/PULL_REQUEST_TEMPLATE.md"; do
  [[ -f "$p" ]] && { TEMPLATE="$p"; break; }
done
if [[ -n "$BODY_FILE" && -f "$BODY_FILE" ]]; then
  BODY="$(cat "$BODY_FILE")"
elif [[ -n "$TEMPLATE" ]]; then
  BODY="$(cat "$TEMPLATE")"
else
  # 默认 PR 模板（与 git-feature-pr-flow Skill 完全一致）
  BODY="## Summary
- What changed
- Which modules or pages were affected

## Why
- Ticket: $TICKET
- 为什么需要这个变更（需求 / 缺陷）

## Changes
- 关键实现点
- 涉及文件或子系统

## Validation
- [ ] Lint passed（make lint-js / make lint-go / pnpm lint）
- [ ] Tests passed（pnpm test / mvn test / go test）
- [ ] Build passed
- [ ] 手动关键路径验证通过

校验命令：
\`\`\`bash
# 请填入仓库对应命令（参考各仓 AGENTS.md 提交前 5 条）
\`\`\`

## Risk
- 已知副作用 / 兼容性 / 部署影响

## Rollback
- 方法 1：在 GitHub/Gitea 上 Revert PR → 走 CI 回滚
- 方法 2：若有发布，回退到上一个稳定版本
"
fi

# ===== 提 PR =====
REVIEW_OPT=""
if [[ -n "$REVIEWERS" ]]; then REVIEW_OPT="--reviewer $REVIEWERS"; fi

PR_CREATE_RC=0
PR_URL=""
PR_NUMBER=""

if [[ "$FALLBACK" == "0" ]] && command -v gh >/dev/null 2>&1; then
  echo "▶ gh 可用 — 调用 gh pr create 创建 PR"
  set +e
  if [[ -n "$DRAFT" ]]; then
    OUT="$(gh pr create --title "$PR_TITLE" --body "$BODY" --base "$BASE" --head "$HEAD" $DRAFT $REVIEW_OPT 2>&1)"
  else
    OUT="$(gh pr create --title "$PR_TITLE" --body "$BODY" --base "$BASE" --head "$HEAD" $REVIEW_OPT 2>&1)"
  fi
  PR_CREATE_RC=$?
  set -e
  if [[ "$PR_CREATE_RC" == "0" ]]; then
    PR_URL="$(printf '%s' "$OUT" | tail -n 1)"
    PR_NUMBER="$(printf '%s' "$PR_URL" | grep -oE '/pull/[0-9]+' | head -n 1 | tr -d '/pull/')"
  else
    echo "⚠️ gh pr create 失败："
    printf '%s\n' "$OUT"
    FALLBACK=1
  fi
fi

# Fallback：打印模板，用户可用 MCP create_pull_request 或手动提
if [[ -z "$PR_URL" ]]; then
  REMOTE="$(git remote get-url origin)"
  # origin 格式：git@github.com:CheersAI/CheersAI-Vault.git 或 https://github.com/CheersAI/CheersAI-Vault
  ORG_REPO="$(printf '%s' "$REMOTE" | sed -E 's#^git@[^:]+:##' | sed -E 's#^https?://[^/]+/##' | sed 's/\.git$//')"
  HOST="github.com"
  if printf '%s' "$REMOTE" | grep -qE 'gitea|git\.cheersai'; then HOST="自定义 Gitea"; fi
  PR_URL="https://${HOST}/${ORG_REPO}/compare/${BASE}...${HEAD}?expand=1&title=$(python3 -c "import urllib.parse,sys;print(urllib.parse.quote(sys.argv[1]))" "$PR_TITLE")"
  echo "ℹ️ Fallback 模式 —— 请打开以下链接（或使用 GitHub MCP create_pull_request）创建 PR："
  echo "   Compare: $PR_URL"
  echo "   Title : $PR_TITLE"
  echo "   Base  : $BASE"
  echo "   Head  : $HEAD"
  echo "   Draft : ${DRAFT:-no}"
  echo "   Reviewers: ${REVIEWERS:-<按需分配 TD/QA/RO>}"
fi

# ===== BIZ-PR 最终事件 audit =====
POST_EVIDENCE="$(python3 - <<PY
import json
print(json.dumps({
  "repo": "$REPO_NAME",
  "org_repo": "${ORG_REPO:-}",
  "host": "${HOST:-github.com}",
  "base": "$BASE",
  "head_branch": "$HEAD",
  "head_sha": "$HEAD_SHA",
  "pr_title": "$PR_TITLE",
  "pr_number": "${PR_NUMBER:-}",
  "pr_url": "$PR_URL",
  "draft": bool("$DRAFT"),
  "reviewers": [r for r in "$REVIEWERS".split(",") if r],
  "actor": "$ACTOR",
  "role_claim": "$ROLE",
  "gh_create_rc": "$PR_CREATE_RC",
  "fallback": bool([[]] if not "$FALLBACK" else bool("$FALLBACK")),
  "template_used": "${TEMPLATE:-builtin}",
}, ensure_ascii=False))
PY
)"
python3 "$SDLC_ROOT/bin/audit-writer.py" \
  --ticket "$TICKET" --gate "BIZ-PR" \
  --actor_email "$ACTOR" --role "$ROLE" \
  --artifact "$REPO_ABS" --exit_code 0 \
  --evidence "$POST_EVIDENCE"

echo
echo "✅ PR 创建流程执行完成（Harness Engineering × GitHub Flow）："
echo "   · PR Title : $PR_TITLE"
echo "   · PR URL   : $PR_URL"
[[ -n "$PR_NUMBER" ]] && echo "   · PR #     : $PR_NUMBER"
echo "   · Repo     : $REPO_NAME ($HEAD → $BASE)"
echo "   · Reviewers: ${REVIEWERS:-<请添加 TD/QA/RO review>}"
echo
echo "下一步：等待 TD 代码评审 + QA 测试 + RO 发布审批（SoD，HE-4）；所有评论/审批完成后合入。"
