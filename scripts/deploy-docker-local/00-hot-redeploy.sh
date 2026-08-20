#!/usr/bin/env bash
# Copyright 2026 CheersAI Authors
# deploy-docker-local/00-hot-redeploy.sh — 本地 Docker 模式一键重建
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
cd "${REPO_ROOT}"

NODE_BIN="${HOME}/.trae/binaries/node/versions/24.19.0/bin"
if [ -d "${NODE_BIN}" ]; then
  export PATH="${NODE_BIN}:${PATH}"
fi

if command -v pnpm >/dev/null 2>&1 && pnpm -v >/dev/null 2>&1; then
  echo "[local-docker] (1/5) pnpm build (Vite prod bundle, force fresh)"
  pnpm build
elif [ -x "${REPO_ROOT}/node_modules/.bin/vite" ] && [ -x "${REPO_ROOT}/node_modules/.bin/tsc" ]; then
  echo "[local-docker] (1/5) manual build via node_modules/.bin (system pnpm unavailable; running version:prepare → tsc → vite build)"
  node scripts/version-manager.js prepare
  "${REPO_ROOT}/node_modules/.bin/tsc"
  "${REPO_ROOT}/node_modules/.bin/vite" build
else
  echo "[local-docker] (1/5) no local pnpm / vite binaries; falling back to \`npx pnpm build\` (cached via npx, no global writes)"
  npx --yes pnpm build
fi

echo "[local-docker] (2/5) ensure dist exists for web dockerfile COPY"
if [ ! -d dist ]; then
  echo "[error] dist/ missing after build; cannot bake vault-pro-web image." >&2
  exit 1
fi

echo "[local-docker] (3/5) compose down to release 8787/5173 port bindings"
if [ -f "docker-compose.yml" ] || [ -f "compose.yaml" ]; then
  docker compose down
else
  echo "[warn] No compose file found; skip docker rebuild."
  exit 0
fi

echo "[local-docker] (4/5) compose build --no-cache (drop old column_samples/data_hint mirror)"
docker compose build --no-cache

echo "[local-docker] (4.5/5) compose up -d --force-recreate"
docker compose up -d --force-recreate

echo "[local-docker] (5/5) post-deploy smoke: health + /api/v1/excel/parse-structure column_samples contract + jobs route reachable (HTTP 4xx, not 404)"
set +e
for i in 1 2 3 4 5 6; do
  H_CODE=$(curl -s -o /tmp/health_body.txt -w "%{http_code}" http://127.0.0.1:8787/api/v1/health || echo "000")
  echo "  health attempt $i => $H_CODE"
  [ "$H_CODE" = "200" ] && break
  sleep 5
done
set -e
[ "$H_CODE" = "200" ] || { echo "[smoke] runtime health never returned 200" >&2; exit 1; }

set +e
J_CODE=$(curl -s -o /tmp/jobs_body.txt -w "%{http_code}" -X POST \
  -H "Content-Type: multipart/form-data; boundary=SMOKE" \
  --data-binary "--SMOKE--" \
  http://127.0.0.1:8787/api/v1/excel/jobs || echo "000")
set -e
echo "  jobs route reachability => $J_CODE (expect 400/413/422, NOT 404)"
[ "$J_CODE" != "404" ] || {
  echo "[smoke] POST /api/v1/excel/jobs returned 404; runtime binary still carries pre-refactor routes (fix: docker compose build --no-cache was skipped)." >&2
  exit 1
}
[ "$J_CODE" = "000" ] && { echo "[smoke] POST /api/v1/excel/jobs failed to connect (runtime unreachable)" >&2; exit 1; }

set +e
TEST_XLSX=""
for f in \
  "apps/vault-runtime-api/tests/fixtures/sample.xlsx" \
  "apps/vault-runtime-api/tests/fixtures/excel_test.xlsx" \
  "apps/vault-runtime-api/tests/fixtures/*.xlsx"; do
  [ -f "$f" ] && { TEST_XLSX="$f"; break; }
done
set -e
if [ -n "${TEST_XLSX}" ] && [ -f "${TEST_XLSX}" ]; then
  echo "  using fixture ${TEST_XLSX} for parse-structure column_samples smoke"
  PARSE_BODY=$(curl -s -X POST \
    -F "file=@${TEST_XLSX};type=application/vnd.openxmlformats-officedocument.spreadsheetml.sheet" \
    http://127.0.0.1:8787/api/v1/excel/parse-structure || true)
  echo "${PARSE_BODY}" > /tmp/parse_body.json
  COL_COUNT=$(node -e 'try{const s=JSON.parse(require("fs").readFileSync("/tmp/parse_body.json","utf8"))[0];process.stdout.write(String((s.headers||[]).length))}catch(_){process.stdout.write("0")}')
  CS_LEN=$(node -e 'try{const s=JSON.parse(require("fs").readFileSync("/tmp/parse_body.json","utf8"))[0];process.stdout.write(String((s.column_samples||[]).length))}catch(_){process.stdout.write("0")}')
  CS_ARRAY=$(node -e 'try{const s=JSON.parse(require("fs").readFileSync("/tmp/parse_body.json","utf8"))[0];process.stdout.write(String(Array.isArray(s.column_samples)&&s.column_samples.every(Array.isArray)))}catch(_){process.stdout.write("false")}')
  echo "  parse-structure contract => headers.len=${COL_COUNT}, column_samples.len=${CS_LEN}, every-column-is-array=${CS_ARRAY}"
  [ "${COL_COUNT}" = "${CS_LEN}" ] || {
    echo "[smoke] parse-structure column_samples length mismatch vs headers (row-based data_hint still baked?)" >&2
    exit 1
  }
  [ "${CS_ARRAY}" = "true" ] || {
    echo "[smoke] parse-structure column_samples not Vec<Vec<String>> (data_hint rename propagated?)" >&2
    exit 1
  }
  echo "[smoke] parse-structure column_samples contract PASSED"
else
  echo "[warn] no test xlsx fixture found; skipping parse-structure column_samples assertion"
fi

echo "[local-docker] redeploy & smoke PASSED. 5173 (pro-web) / 8787 (runtime) online."
