#!/usr/bin/env bash
# gate-code.sh — G3 门禁：按仓库主语言调用对应 Makefile / pnpm / mvn / cargo / ruff 目标
# 依赖: ../<repo>/Makefile 或等价命令（由各仓 AGENTS.md 约定）
# 兼容性：macOS 默认 bash v3（无 declare -A、无 <<<），用 POSIX sh 方式替代
set -uo pipefail   # 不 set -e：单步 check 失败要记 evidence，不会因 set -e 中断

SDLC_ROOT_HINT="$(cd "$(dirname "$0")/.."; pwd)"

usage() {
  echo "Usage: $0 --repo <path> [--actor email] [--ticket TICKET]"
  echo "  --repo  目标仓库路径 (必须包含 Makefile / package.json / go.mod / pom.xml / pyproject.toml / Cargo.toml)"
}

REPO="" ACTOR="sdlc-cd@cheersai.ai" TICKET="AUTO"
while [[ $# -gt 0 ]]; do
  case "$1" in
    --repo)   REPO="$2"; shift 2;;
    --actor)  ACTOR="$2"; shift 2;;
    --ticket) TICKET="$2"; shift 2;;
    -h|--help) usage; exit 0;;
    *) echo "Unknown arg: $1"; usage; exit 2;;
  esac
done

if [[ -z "$REPO" || ! -d "$REPO" ]]; then
  echo "[G3-CODE] ❌ --repo 必须是存在的目录" >&2
  exit 1
fi
cd "$REPO" >/dev/null
ABS_REPO="$(pwd)"
REPO_NAME="${ABS_REPO##*/}"
SDLC_ROOT="$SDLC_ROOT_HINT"

EVIDENCE_KEYS=""   # 空格分隔 key
EVIDENCE_VALUES="" # 同序的 value（换行分隔，避免与空格冲突）
push_ev() {
  local k="$1" v="$2"
  EVIDENCE_KEYS="${EVIDENCE_KEYS}${EVIDENCE_KEYS:+ }$k"
  EVIDENCE_VALUES="${EVIDENCE_VALUES}${EVIDENCE_VALUES:+
}$v"
}

echo "[G3-CODE] 仓库 = $ABS_REPO"

EXIT_CODE=0
run_target() {
  local name="$1"; shift
  echo "[G3-CODE]    ▶ 运行 $name"
  if "$@"; then
    push_ev "$name" "ok"
  else
    push_ev "$name" "fail"
    EXIT_CODE=5
  fi
}

# ---------- 检测语言 & 跑目标 ----------
if [[ -f "go.mod" ]]; then
  echo "[G3-CODE] 🔍 检测到 Go 主语言"
  if grep -q 'lint-go:' Makefile 2>/dev/null; then run_target go_lint make lint-go; fi
  if grep -q 'fmt:' Makefile 2>/dev/null;      then run_target go_fmt  make fmt; fi
  if [[ $(git status --porcelain go.mod go.sum 2>/dev/null | wc -l | tr -d ' ') -gt 0 ]] && grep -q 'tidy:' Makefile 2>/dev/null; then
    run_target go_tidy make tidy
  fi
  if grep -q 'test:' Makefile 2>/dev/null; then
    run_target go_test make test
  else
    if command -v go >/dev/null 2>&1; then run_target go_test go test ./...; fi
  fi
fi

if [[ -f "eslint.config.mjs" || -f "eslint.config.js" || -f ".eslintrc.cjs" || -f ".eslintrc.js" || -f ".eslintrc" ]]; then
  cp_eslint=eslint_9_plus
else
  cp_eslint=eslint_legacy
fi
# 初始阶段过渡策略：lint 未完全收敛时退化为 warn-only
SDLC_LINT_STRICT=${SDLC_LINT_STRICT:-warn-only}

if [[ -f "package.json" ]]; then
  echo "[G3-CODE] 🔍 检测到 TypeScript/JS 前端 (lint-strict=$SDLC_LINT_STRICT)"
  LINT_RAN=0
  if grep -q 'lint-js:' Makefile 2>/dev/null; then
    if [[ "$SDLC_LINT_STRICT" == "strict" ]]; then run_target ts_lint make lint-js; else echo '[G3-CODE]   ⚠️  (warn-only)  lint-js 忽略 lint 问题'; push_ev ts_lint "warn-only"; fi
    LINT_RAN=1
  fi
  if [[ $LINT_RAN -eq 0 ]] && (grep -q '"lint"' package.json); then
    if [[ "$SDLC_LINT_STRICT" == "strict" ]]; then
      if pnpm lint >/dev/null 2>&1; then
        push_ev ts_lint ok
      elif npm run lint >/dev/null 2>&1; then
        push_ev ts_lint ok
      else
        push_ev ts_lint fail; EXIT_CODE=5
      fi
    else
      (pnpm lint >/dev/null 2>&1 || npm run lint >/dev/null 2>&1 || true)
      push_ev ts_lint "warn-only"
    fi
  fi
  if grep -q '"typecheck"' package.json || grep -q 'typecheck:' Makefile 2>/dev/null; then
    if pnpm typecheck >/dev/null 2>&1; then
      push_ev ts_typecheck ok
    elif npm run typecheck >/dev/null 2>&1; then
      push_ev ts_typecheck ok
    else
      push_ev ts_typecheck fail; EXIT_CODE=5
    fi
  fi
  TEST_RAN=0
  if grep -q 'test:' Makefile 2>/dev/null; then run_target ts_test make test; TEST_RAN=1; fi
  if [[ $TEST_RAN -eq 0 ]] && (grep -q '"test"' package.json); then
    if pnpm test >/dev/null 2>&1; then
      push_ev ts_test ok
    elif npm test >/dev/null 2>&1; then
      push_ev ts_test ok
    else
      push_ev ts_test fail; EXIT_CODE=5
    fi
  fi
  # 供 evidence 记录
  push_ev ts_eslint "$cp_eslint"
  push_ev ts_lint_mode "$SDLC_LINT_STRICT"
fi

if [[ -f "Cargo.toml" ]]; then
  echo "[G3-CODE] 🔍 检测到 Rust"
  if command -v cargo >/dev/null 2>&1; then
    run_target rust_clippy cargo clippy -- -D warnings
    run_target rust_test   cargo test
  fi
fi

if ls pom.xml nexus-backend/pom.xml 2>/dev/null | head -1 >/dev/null; then
  echo "[G3-CODE] 🔍 检测到 Java/Maven (Nexus 类)"
  POM="pom.xml"; [[ -f "nexus-backend/pom.xml" ]] && POM="nexus-backend/pom.xml"
  if command -v mvn >/dev/null 2>&1; then
    run_target java_checkstyle mvn -q -f "$POM" checkstyle:check
    run_target java_test       mvn -q -f "$POM" test
  fi
fi

if ls pyproject.toml api/pyproject.toml 2>/dev/null | head -1 >/dev/null; then
  echo "[G3-CODE] 🔍 检测到 Python (Desktop/Django 类)"
  PY_ROOT="."; [[ -f "api/pyproject.toml" ]] && PY_ROOT="api"
  if command -v ruff >/dev/null 2>&1; then run_target py_ruff  bash -lc "cd '$PY_ROOT' && ruff check ."; fi
  if command -v pytest >/dev/null 2>&1; then run_target py_pytest bash -lc "cd '$PY_ROOT' && pytest -q"; fi
fi

# 无任何语言检测
if [[ -z "$EVIDENCE_KEYS" ]]; then
  echo "[G3-CODE] ⚠️  未识别任何主流语言构建文件，跳过具体检查（仍写入审计）"
  push_ev fallback no-targets
fi

# 组装 evidence JSON（POSIX sh 兼容：awk 遍历 keys/values）
EVIDENCE_JSON=$(python3 - "$EVIDENCE_KEYS" "$EVIDENCE_VALUES" <<'PY'
import sys
keys   = sys.argv[1].split() if sys.argv[1].strip() else []
values = [v for v in sys.argv[2].split("\n")] if sys.argv[2] else []
if len(values) < len(keys): values += [""]*(len(keys)-len(values))
obj = dict(zip(keys, values))
import json
print(json.dumps(obj, ensure_ascii=False))
PY
)
# 上面用 here-doc 仅是纯文本，无 bash v3 <<< 运算符问题

echo "[G3-CODE] 汇总: $EVIDENCE_JSON -> exit=$EXIT_CODE"

python3 "$SDLC_ROOT/bin/audit-writer.py" \
  --ticket "$TICKET" --gate "G3-CODE" \
  --actor_email "$ACTOR" --role "CD" \
  --artifact "$ABS_REPO" \
  --exit_code "$EXIT_CODE" \
  --evidence "$EVIDENCE_JSON" || true

exit $EXIT_CODE
