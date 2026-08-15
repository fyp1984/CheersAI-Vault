#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

if [[ -d "$HOME/.cargo/bin" ]]; then
  export PATH="$HOME/.cargo/bin:$PATH"
fi

pass() {
  printf '[PASS] %s\n' "$1"
}

warn() {
  printf '[WARN] %s\n' "$1"
}

fail() {
  printf '[FAIL] %s\n' "$1" >&2
  exit 1
}

require_cmd() {
  local cmd="$1"
  local hint="$2"
  if command -v "$cmd" >/dev/null 2>&1; then
    pass "已找到命令: $cmd"
  else
    fail "缺少命令: $cmd；$hint"
  fi
}

detect_arch_label() {
  case "$(uname -m)" in
    x86_64|amd64)
      printf 'x86_64'
      ;;
    arm64|aarch64)
      printf 'arm64'
      ;;
    *)
      fail "不支持的 macOS 架构: $(uname -m)"
      ;;
  esac
}

detect_rust_target() {
  case "$1" in
    x86_64)
      printf 'x86_64-apple-darwin'
      ;;
    arm64)
      printf 'aarch64-apple-darwin'
      ;;
    *)
      fail "无法为架构 $1 推断 Rust target"
      ;;
  esac
}

ARCH_LABEL="$(detect_arch_label)"
RUST_TARGET="$(detect_rust_target "$ARCH_LABEL")"

echo "==> CheersAI Vault macOS 打包工具补齐"
echo "仓库: $ROOT_DIR"
echo "当前架构: $ARCH_LABEL"
echo "Rust target: $RUST_TARGET"
echo

require_cmd node "请先安装 Node.js 22+"
require_cmd corepack "请先安装支持 Corepack 的 Node.js"
require_cmd rustup "请先安装 Rustup"
require_cmd cargo "请先安装 Rust stable toolchain"
require_cmd codesign "请先安装 Xcode Command Line Tools"
require_cmd hdiutil "仅 macOS 支持 DMG 打包"
echo

echo "==> 启用 Corepack"
if corepack enable >/dev/null 2>&1; then
  pass "Corepack 已启用"
else
  warn "corepack enable 未能写入全局 pnpm shim；当前环境将回退为直接使用 corepack pnpm"
fi

echo "==> 准备 pnpm"
PNPM_VERSION="$(corepack pnpm -v)"
pass "pnpm 已可用: $PNPM_VERSION"

echo "==> 校验 Rust toolchain"
rustup show active-toolchain
rustup target add "$RUST_TARGET" >/dev/null
pass "Rust target 已就绪: $RUST_TARGET"
echo

echo "==> Xcode 环境"
if xcodebuild -version >/dev/null 2>&1; then
  pass "xcodebuild 可用"
else
  warn "当前仅检测到 CommandLineTools，未启用完整 Xcode"
  if [[ -d "/Applications/Xcode.app" ]]; then
    warn "已发现 /Applications/Xcode.app，可执行：sudo xcode-select -s /Applications/Xcode.app/Contents/Developer"
  else
    warn "未发现 /Applications/Xcode.app；如需打通 Tauri 原始 DMG 链路，请先安装完整 Xcode"
  fi
fi
echo

echo "==> 建议后续验证"
echo "1. bash ./scripts/check-macos-release-env.sh"
echo "2. bash ./scripts/build-macos-portable-dmg.sh --output-dir ./dist"
