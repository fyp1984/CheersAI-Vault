#!/usr/bin/env bash
# git-create-feature-branch.sh — 创建特性分支（严格 GitHub Flow + SoD 审计入链）
#
# 强制规则（违反 → exit 非 0，符合 git-feature-pr-flow Skill）：
#   1) 必须在 git 仓库；
#   2) 当前分支如果是 main/master → 必须先 stash/提交未暂存改动；
#   3) 分支名前缀必须是 feature/ fix/ refactor/ chore/ docs/ test/（现有 top-level 冲突时 fall back 连字符）；
#   4) 分支必须包含 ticket 编号（例如 IMP-001 / FIX-042），可通过 --ticket 显式提供，也可从分支名前缀推导。
#
# 链式哈希审计（HE-3 Observability First）：
#   每次调用 → 向 sdlc/audit/<date>/_*.jsonl 写入 BIZ-BRANCH 事件。
set -euo pipefail

# 定位 sdlc 根（脚本位于 sdlc/bin/ 下）
SDLC_ROOT="$(cd "$(dirname "$0")/.."; pwd)"

usage() {
  cat <<EOF
用法: $0 --ticket <TICKET> --role <RA|TD|CD|QA|RO> --actor <xx@cheersai.ai>
                --type <feature|fix|refactor|chore|docs|test>
                --topic <short-description>
                [--base <main|master>] [--repo-root <path>] [--no-push]
示例:
  $0 --ticket IMP-001 --role CD --actor sdlc-cd@cheersai.ai \
     --type feature --topic address-pii-masking-rule
说明:
  * 默认 base=main；不允许在 base 分支直接开发（HE-4 Zero-Trusted：防止 TD+CD 双岗）
  * 创建分支后自动写入 audit BIZ-BRANCH 事件（base_sha + head_branch + actor + role）
  * 若远端已存在同名 top-level 分支，自动回退: feature/xxx → feature-xxx
EOF
}

TICKET="" ROLE="" ACTOR="" TYPE="feature" TOPIC="" BASE="main" REPO_ROOT="" NO_PUSH=0
while [[ $# -gt 0 ]]; do
  case "$1" in
    --ticket)    TICKET="$2";  shift 2;;
    --role)      ROLE="$2";    shift 2;;
    --actor)     ACTOR="$2";   shift 2;;
    --type)      TYPE="$2";    shift 2;;
    --topic)     TOPIC="$2";   shift 2;;
    --base)      BASE="$2";    shift 2;;
    --repo-root) REPO_ROOT="$2"; shift 2;;
    --no-push)   NO_PUSH=1;    shift;;
    -h|--help)   usage; exit 0;;
    *) echo "Unknown arg: $1"; usage; exit 2;;
  esac
done

# ===== 参数校验 =====
case "$ROLE" in RA|TD|CD|QA|RO) ;; *) echo "::error::--role 必须是 RA|TD|CD|QA|RO (HE-4 SoD)"; exit 3;; esac
case "$TYPE" in feature|fix|refactor|chore|docs|test) ;; *) echo "::error::--type 必须是 feature/fix/refactor/chore/docs/test"; exit 3;; esac
[[ -z "$TICKET" ]] && { echo "::error::--ticket 必填（例如 IMP-001）"; exit 3; }
[[ -z "$TOPIC"  ]] && { echo "::error::--topic  必填（短横线风格，例如 address-masking）"; exit 3; }
[[ -z "$ACTOR"  ]] && { echo "::error::--actor  必填（企业邮箱）"; exit 3; }

# ===== 切到仓库根 =====
if [[ -n "$REPO_ROOT" ]]; then
  cd "$REPO_ROOT" >/dev/null
else
  # 默认为 sdlc 的父目录（通常 CheersAI 仓库集合的 workspace root）
  cd "$SDLC_ROOT/.." >/dev/null
fi
if ! git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
  echo "::error::当前目录不是 git 仓库 ($(pwd))。请使用 --repo-root 传入仓库绝对路径。"
  exit 4
fi
REPO_ABS="$(git rev-parse --show-toplevel)"; cd "$REPO_ABS" >/dev/null
REPO_NAME="${REPO_ABS##*/}"

# ===== 主分支检查 =====
CURRENT="$(git symbolic-ref --short HEAD 2>/dev/null || echo DETACHED)"
if [[ "$CURRENT" == "$BASE" ]]; then
  UNSTAGED="$(git status --porcelain | wc -l | tr -d ' ')"
  if [[ "$UNSTAGED" != "0" ]]; then
    echo "::error::当前在 base=$BASE 分支且有 $UNSTAGED 个未提交更改。请先 git stash，再运行本脚本。"
    echo "        → 规则: 严禁直接在 main/master 上开发 (GitHub Flow)。"
    exit 5
  fi
else
  echo "ℹ️ 当前在分支 $CURRENT，将先切回 $BASE 拉最新代码后再创建特性分支。"
fi

# 切回 base + pull 最新
if ! git show-ref --verify --quiet "refs/heads/$BASE"; then
  echo "::error::base 分支 $BASE 不存在，请使用 --base 指定。"
  exit 6
fi
echo "▶ git checkout $BASE && git pull origin $BASE"
git checkout "$BASE" >/dev/null
git pull origin "$BASE" --ff-only >/dev/null 2>&1 || git pull origin "$BASE" >/dev/null

BASE_SHA="$(git rev-parse HEAD)"

# ===== 构造分支名（按 skill 约定） =====
BRANCH_SLASH="$TYPE/$TICKET-$TOPIC"
BRANCH_FLAT="$TYPE-$TICKET-$TOPIC"
# 先试 feature/IMP-001-xxx；若远端已有同名顶层分支（存在 top-level feature refs）则 fall back flat
if git ls-remote --exit-code --heads origin "refs/heads/$TYPE" >/dev/null 2>&1; then
  BRANCH="$BRANCH_FLAT"
  echo "ℹ️  检测到远端存在顶层分支 refs/heads/$TYPE → 自动回退 flat 命名 $BRANCH（避免与 slash 冲突）"
else
  BRANCH="$BRANCH_SLASH"
fi

# 分支名小写化 + 下划线转横杠（Git 最佳实践）
BRANCH="$(printf '%s' "$BRANCH" | tr '[:upper:]' '[:lower:]' | tr '_' '-')"

# ===== 创建分支 =====
if git show-ref --verify --quiet "refs/heads/$BRANCH"; then
  echo "ℹ️  本地分支 $BRANCH 已存在，直接 checkout（如需新建请换 --topic）。"
  git checkout "$BRANCH" >/dev/null
else
  echo "▶ 创建特性分支: $BRANCH"
  git checkout -b "$BRANCH" "$BASE_SHA" >/dev/null
fi

# ===== 首次 push（建立 upstream） =====
if [[ "$NO_PUSH" == "0" ]]; then
  echo "▶ push -u origin $BRANCH （GitHub PR 前置）"
  if ! git push -u origin "$BRANCH"; then
    # 若 slash 推不上去，说明远端存在顶层分支；fallback flat
    echo "⚠️ slash 形式 push 失败 → 自动回退 flat 命名并重试"
    BRANCH="$BRANCH_FLAT"
    BRANCH="$(printf '%s' "$BRANCH" | tr '[:upper:]' '[:lower:]' | tr '_' '-')"
    if git show-ref --verify --quiet "refs/heads/$BRANCH"; then git checkout "$BRANCH" >/dev/null; else git checkout -b "$BRANCH" "$BASE_SHA" >/dev/null; fi
    git push -u origin "$BRANCH"
  fi
fi

HEAD_SHA="$(git rev-parse HEAD)"

# ===== HE-3 链式哈希审计：BIZ-BRANCH 事件 =====
EVIDENCE="$(python3 - <<PY
import json
print(json.dumps({
  "repo": "$REPO_NAME",
  "base": "$BASE",
  "base_sha": "$BASE_SHA",
  "branch": "$BRANCH",
  "head_sha": "$HEAD_SHA",
  "branch_type": "$TYPE",
  "role_claim": "$ROLE",
  "sod_promise": "actor=$ACTOR claims role=$ROLE only for this ticket=$TICKET (no cross-role)",
}, ensure_ascii=False))
PY
)"
python3 "$SDLC_ROOT/bin/audit-writer.py" \
  --ticket "$TICKET" --gate "BIZ-BRANCH" \
  --actor_email "$ACTOR" --role "$ROLE" \
  --artifact "$REPO_ABS" --exit_code 0 \
  --evidence "$EVIDENCE"

echo
echo "✅ 特性分支创建成功 → 符合 GitHub Flow / Harness Engineering 规范："
echo "   · Repo       : $REPO_NAME"
echo "   · Base branch: $BASE @ ${BASE_SHA:0:12}…"
echo "   · New branch : $BRANCH"
echo "   · Role / Actor: $ROLE / $ACTOR"
echo "   · Ticket    : $TICKET"
echo
echo "下一步：开发完成后 → 提交（Conventional Commit）→ 运行: sdlc/bin/git-auto-create-pr.sh --ticket $TICKET"
