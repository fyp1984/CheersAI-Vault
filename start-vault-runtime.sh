#!/usr/bin/env bash
# CheersAI Vault Enterprise Runtime — 本地一键启动脚本（方案 B，不依赖 Docker Desktop）
# 对应 release-ops-flow § 本地开发快速入口；Runtime 仅监听 loopback 127.0.0.1:8787
# 前置：已执行 cargo build --manifest-path apps/vault-runtime-api/Cargo.toml（见 repo README）
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
export PATH="$HOME/.cargo/bin:$PATH"

export VAULT_RUNTIME_PORT="${VAULT_RUNTIME_PORT:-8787}"
export VAULT_RUNTIME_BIND_HOST="${VAULT_RUNTIME_BIND_HOST:-127.0.0.1}"
export VAULT_RUNTIME_DATA_DIR="${VAULT_RUNTIME_DATA_DIR:-$ROOT_DIR/apps/vault-runtime-api/enterprise-data}"
export VAULT_RUNTIME_CORS_ORIGINS="${VAULT_RUNTIME_CORS_ORIGINS:-http://127.0.0.1:3000,http://localhost:3000,http://127.0.0.1:5173,http://localhost:5173}"

mkdir -p "$VAULT_RUNTIME_DATA_DIR"
BINARY="$ROOT_DIR/apps/vault-runtime-api/target/debug/vault-runtime-api"
if [ ! -x "$BINARY" ]; then
  echo "[start] 未找到 debug 二进制，开始 cargo build（首次 3-10 min）..."
  cd "$ROOT_DIR"
  cargo build --manifest-path apps/vault-runtime-api/Cargo.toml
fi

PIDFILE=/tmp/vault-runtime-api.pid
LOGFILE=/tmp/vault-runtime-api.log
if [ -f "$PIDFILE" ] && kill -0 "$(cat "$PIDFILE")" 2>/dev/null; then
  echo "[start] Runtime 已在运行，PID=$(cat "$PIDFILE")。先执行 ./stop-vault-runtime.sh 再重启。"
  exit 9
fi

rm -f "$PIDFILE"
nohup "$BINARY" > "$LOGFILE" 2>&1 &
RUNTIME_PID=$!
echo "$RUNTIME_PID" > "$PIDFILE"
disown "$RUNTIME_PID" 2>/dev/null || true
echo "[start] vault-runtime-api PID=$RUNTIME_PID，日志 $LOGFILE，等待 127.0.0.1:$VAULT_RUNTIME_PORT 监听..."

READY=0
for i in $(seq 1 40); do
  sleep 2
  if ! kill -0 "$RUNTIME_PID" 2>/dev/null; then
    echo "[start] 进程早退出。最近 40 行日志："
    tail -n 40 "$LOGFILE" || true
    exit 3
  fi
  if lsof -iTCP:"$VAULT_RUNTIME_PORT" -sTCP:LISTEN -nP >/dev/null 2>&1; then
    READY=1
    echo "[start] OK，监听就绪耗时 $((i*2))s"
    break
  fi
done
if [ "$READY" -ne 1 ]; then
  echo "[start] TIMEOUT。最近 40 行日志："
  tail -n 40 "$LOGFILE" || true
  exit 4
fi

echo "[start] === 健康检查（直连 + Vite 3000 同源代理）==="
set +e
curl -sS -w "  DIRECT HTTP_CODE=%{http_code}  %{time_total}s\n" -m 6 "http://${VAULT_RUNTIME_BIND_HOST}:${VAULT_RUNTIME_PORT}/api/v1/health"
echo
curl -sS -w "  VITE_3000_PROXY HTTP_CODE=%{http_code}  %{time_total}s\n" -m 6 "http://127.0.0.1:3000/api/v1/health"
echo
echo "[start] 完成。浏览器访问：http://127.0.0.1:3000/  —— 桌面端 Tauri 请运行：pnpm tauri dev"
exit 0
