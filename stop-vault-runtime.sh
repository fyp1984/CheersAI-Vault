#!/usr/bin/env bash
# CheersAI Vault Enterprise Runtime — 本地一键停止脚本（对应 start-vault-runtime.sh）
set -euo pipefail
PIDFILE=/tmp/vault-runtime-api.pid
LOGFILE=/tmp/vault-runtime-api.log
PORT="${VAULT_RUNTIME_PORT:-8787}"

if [ -f "$PIDFILE" ]; then
  PID="$(cat "$PIDFILE")"
  echo "[stop] 找到 PID=$PID，发送 SIGTERM 优雅停机（Runtime 会响应 SIGTERM，见 apps/vault-runtime-api/src/main.rs）..."
  kill -TERM "$PID" 2>/dev/null || true
  GONE=0
  for i in $(seq 1 15); do
    sleep 1
    if ! kill -0 "$PID" 2>/dev/null; then
      GONE=1
      break
    fi
  done
  if [ "$GONE" -ne 1 ]; then
    echo "[stop] SIGTERM 未响应，强制 kill -9"
    kill -9 "$PID" 2>/dev/null || true
    sleep 2
  fi
  echo "[stop] 进程已终止；清理 PID 文件"
  rm -f "$PIDFILE"
else
  echo "[stop] 未找到 $PIDFILE；尝试按端口 $PORT 查进程..."
  PIDS="$(lsof -iTCP:"$PORT" -sTCP:LISTEN -Fp 2>/dev/null | sed 's/^p//' | tr '\n' ' ')"
  if [ -n "$PIDS" ]; then
    echo "[stop] 候选 PID=$PIDS，发送 SIGTERM"
    for p in $PIDS; do kill -TERM "$p" 2>/dev/null || true; done
    sleep 3
    PIDS2="$(lsof -iTCP:"$PORT" -sTCP:LISTEN -Fp 2>/dev/null | sed 's/^p//' | tr '\n' ' ')"
    if [ -n "$PIDS2" ]; then echo "[stop] 仍存活，强制 kill -9 $PIDS2"; for p in $PIDS2; do kill -9 "$p" 2>/dev/null || true; done; sleep 1; fi
  else
    echo "[stop] 没找到监听在 $PORT 的任何进程；无需停止"
  fi
fi

echo "[stop] === 当前端口占用 === "
lsof -iTCP:"$PORT" -sTCP:LISTEN -nP 2>/dev/null || echo "  PORT_${PORT}_FREE ✓"
echo "[stop] === 最近 5 行运行日志（保留原文件供排查）=== "
tail -n 5 "$LOGFILE" 2>/dev/null || echo "  （无日志文件）"
