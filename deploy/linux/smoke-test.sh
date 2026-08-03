#!/usr/bin/env bash
# CheersAI Vault Runtime — 黑盒客户测试 smoke 脚本
#
# 只使用脚本自己生成的虚构数据（假手机号/假邮箱），不连接任何真实业务
# 系统，不读取本机任何真实文件。通过公开的四项 API（health/batches 提交/
# batches 轮询/artifacts 下载）验证一次成功+失败混合批次的完整闭环。
#
# 用法：
#   ./smoke-test.sh [BASE_URL]
#   BASE_URL 默认 http://127.0.0.1:8787/api/v1（本机直连 Runtime）。
#   经 Nginx 测试请传入，例如：
#   ./smoke-test.sh http://<Nginx内网地址>/api/v1
#
# 任何一步断言失败都会以非零状态退出（`set -euo pipefail` + 显式 exit）。
# 依赖：curl、jq。

set -euo pipefail

BASE_URL="${1:-http://127.0.0.1:8787/api/v1}"
BASE_URL="${BASE_URL%/}"

for bin in curl jq; do
  if ! command -v "$bin" >/dev/null 2>&1; then
    echo "FAIL: 缺少依赖命令: $bin" >&2
    exit 1
  fi
done

WORKDIR="$(mktemp -d)"
cleanup() {
  rm -rf "$WORKDIR"
}
trap cleanup EXIT

fail() {
  echo "FAIL: $1" >&2
  exit 1
}

pass() {
  echo "PASS: $1"
}

# ---------------------------------------------------------------------------
# 0) 虚构测试数据 —— 全部为脚本生成的假数据，非任何真实客户/个人信息。
# ---------------------------------------------------------------------------
FAKE_PHONE="13800001234"
FAKE_EMAIL="smoke-test-fixture@example.invalid"

GOOD_FILE="$WORKDIR/good.txt"
cat > "$GOOD_FILE" <<EOF
虚构联系方式演示：手机号 ${FAKE_PHONE}，邮箱 ${FAKE_EMAIL}。
本文件由 smoke-test.sh 生成，仅用于黑盒验证，不含任何真实个人信息。
EOF

# 扩展名为受支持格式（.docx），但内容是随机字节，保证能通过提交时的
# 格式校验、但在实际处理阶段失败（触发文件级 Failed + 安全错误码），
# 从而使批次进入 CompletedWithErrors。
BAD_FILE="$WORKDIR/bad.docx"
head -c 256 /dev/urandom > "$BAD_FILE"

# ---------------------------------------------------------------------------
# 1) health ready
# ---------------------------------------------------------------------------
HEALTH_RESP="$(curl -sS -w '\n%{http_code}' "$BASE_URL/health")"
HEALTH_HTTP="$(echo "$HEALTH_RESP" | tail -n1)"
HEALTH_BODY="$(echo "$HEALTH_RESP" | sed '$d')"
[ "$HEALTH_HTTP" = "200" ] || fail "health 返回 HTTP $HEALTH_HTTP，期望 200（响应体：$HEALTH_BODY）"
HEALTH_STATUS="$(echo "$HEALTH_BODY" | jq -r '.status // empty')"
[ "$HEALTH_STATUS" = "ready" ] || fail "health.status 为 '$HEALTH_STATUS'，期望 'ready'"
pass "health ready"

# ---------------------------------------------------------------------------
# 2) 提交批次：一个虚构手机号/邮箱文本文件 + 一个损坏文件
# ---------------------------------------------------------------------------
SUBMIT_RESP="$(curl -sS -w '\n%{http_code}' -X POST "$BASE_URL/batches" \
  -F "files=@${GOOD_FILE};filename=good.txt" \
  -F "files=@${BAD_FILE};filename=bad.docx" \
  -F 'rule_ids=["phone","email"]')"
SUBMIT_HTTP="$(echo "$SUBMIT_RESP" | tail -n1)"
SUBMIT_BODY="$(echo "$SUBMIT_RESP" | sed '$d')"
[ "$SUBMIT_HTTP" = "202" ] || fail "批量提交返回 HTTP $SUBMIT_HTTP，期望 202（响应体：$SUBMIT_BODY）"
BATCH_ID="$(echo "$SUBMIT_BODY" | jq -r '.batch_id // empty')"
[ -n "$BATCH_ID" ] || fail "批量提交响应缺少 batch_id"
pass "批量提交成功，batch_id=$BATCH_ID"

# ---------------------------------------------------------------------------
# 3) 轮询到终态 CompletedWithErrors（限时，避免脚本挂死）
# ---------------------------------------------------------------------------
DETAIL_BODY=""
BATCH_STATUS=""
for _ in $(seq 1 60); do
  DETAIL_RESP="$(curl -sS -w '\n%{http_code}' "$BASE_URL/batches/${BATCH_ID}")"
  DETAIL_HTTP="$(echo "$DETAIL_RESP" | tail -n1)"
  DETAIL_BODY="$(echo "$DETAIL_RESP" | sed '$d')"
  [ "$DETAIL_HTTP" = "200" ] || fail "批次查询返回 HTTP $DETAIL_HTTP，期望 200（响应体：$DETAIL_BODY）"
  BATCH_STATUS="$(echo "$DETAIL_BODY" | jq -r '.batch.status // empty')"
  case "$BATCH_STATUS" in
    Completed|CompletedWithErrors|Failed) break ;;
    Running) sleep 1 ;;
    *) fail "batch.status 为未知值 '$BATCH_STATUS'" ;;
  esac
done
[ "$BATCH_STATUS" = "CompletedWithErrors" ] || fail "批次终态为 '$BATCH_STATUS'，期望 'CompletedWithErrors'（一好一坏文件应产生部分失败）"
pass "批次到达终态 CompletedWithErrors"

# ---------------------------------------------------------------------------
# 4) 成功文件：有 artifact_id，无 error_code；失败文件：只有安全错误信息
# ---------------------------------------------------------------------------
GOOD_ENTRY="$(echo "$DETAIL_BODY" | jq -c '.files[] | select(.display_name=="good.txt")')"
BAD_ENTRY="$(echo "$DETAIL_BODY" | jq -c '.files[] | select(.display_name=="bad.docx")')"
[ -n "$GOOD_ENTRY" ] || fail "响应中找不到 good.txt 的文件记录"
[ -n "$BAD_ENTRY" ] || fail "响应中找不到 bad.docx 的文件记录"

GOOD_STATUS="$(echo "$GOOD_ENTRY" | jq -r '.status')"
GOOD_ARTIFACT="$(echo "$GOOD_ENTRY" | jq -r '.artifact_id // empty')"
[ "$GOOD_STATUS" = "Completed" ] || fail "good.txt 状态为 '$GOOD_STATUS'，期望 'Completed'"
[ -n "$GOOD_ARTIFACT" ] || fail "good.txt 已 Completed 但缺少 artifact_id"
pass "成功文件具有 artifact_id：$GOOD_ARTIFACT"

BAD_STATUS="$(echo "$BAD_ENTRY" | jq -r '.status')"
BAD_ARTIFACT="$(echo "$BAD_ENTRY" | jq -r '.artifact_id // empty')"
BAD_ERROR_CODE="$(echo "$BAD_ENTRY" | jq -r '.error_code // empty')"
BAD_ERROR_MSG="$(echo "$BAD_ENTRY" | jq -r '.error_message // empty')"
[ "$BAD_STATUS" = "Failed" ] || fail "bad.docx 状态为 '$BAD_STATUS'，期望 'Failed'"
[ -z "$BAD_ARTIFACT" ] || fail "bad.docx 应为失败文件但携带了 artifact_id"
[ -n "$BAD_ERROR_CODE" ] || fail "bad.docx 缺少 error_code"
[ -n "$BAD_ERROR_MSG" ] || fail "bad.docx 缺少 error_message"
# 错误信息不得包含明显的服务器本地路径特征（消毒检查，启发式）。
case "$BAD_ERROR_MSG" in
  *"$WORKDIR"*|*"/Users/"*|*"/home/"*|*"/var/"*) fail "error_message 疑似泄露服务器本地路径: $BAD_ERROR_MSG" ;;
esac
pass "失败文件只携带安全错误码/信息：$BAD_ERROR_CODE"

# ---------------------------------------------------------------------------
# 5) 下载成功文件，验证虚构原文消失、占位符存在
# ---------------------------------------------------------------------------
DOWNLOAD_FILE="$WORKDIR/downloaded.md"
DOWNLOAD_HTTP="$(curl -sS -w '%{http_code}' -o "$DOWNLOAD_FILE" "$BASE_URL/artifacts/${GOOD_ARTIFACT}")"
[ "$DOWNLOAD_HTTP" = "200" ] || fail "下载 artifact 返回 HTTP $DOWNLOAD_HTTP，期望 200"

if grep -qF -- "$FAKE_PHONE" "$DOWNLOAD_FILE"; then
  fail "下载内容仍包含虚构原始手机号，脱敏未生效"
fi
if grep -qF -- "$FAKE_EMAIL" "$DOWNLOAD_FILE"; then
  fail "下载内容仍包含虚构原始邮箱，脱敏未生效"
fi
if ! grep -q -- '\*\*\*PHONE\*\*\*' "$DOWNLOAD_FILE"; then
  fail "下载内容未找到手机号占位符（***PHONE***...）"
fi
if ! grep -q -- '\*\*\*EMAIL\*\*\*' "$DOWNLOAD_FILE"; then
  fail "下载内容未找到邮箱占位符（***EMAIL***...）"
fi
pass "下载内容已脱敏：虚构原文消失，占位符存在"

# ---------------------------------------------------------------------------
# 6) 不存在 .cmap 下载入口
# ---------------------------------------------------------------------------
CT_HEADER="$(curl -sS -D - -o /dev/null "$BASE_URL/artifacts/${GOOD_ARTIFACT}" | tr -d '\r' | grep -i '^content-type:' || true)"
case "$CT_HEADER" in
  *cmap*) fail "artifact 下载响应的 Content-Type 疑似暴露 .cmap: $CT_HEADER" ;;
esac
CMAP_GUESS_HTTP="$(curl -sS -o /dev/null -w '%{http_code}' "$BASE_URL/artifacts/${GOOD_ARTIFACT}.cmap")"
[ "$CMAP_GUESS_HTTP" = "404" ] || fail "猜测的 .cmap 下载路径未返回 404（返回 $CMAP_GUESS_HTTP），可能存在非预期的映射泄露入口"
pass "确认不存在 .cmap 下载入口"

echo "ALL SMOKE CHECKS PASSED"
exit 0
