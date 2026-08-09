#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

if [[ -d "$HOME/.cargo/bin" ]]; then
  export PATH="$HOME/.cargo/bin:$PATH"
fi

ERRORS=0
WARNINGS=0

pass() {
  printf '[PASS] %s\n' "$1"
}

warn() {
  printf '[WARN] %s\n' "$1"
  WARNINGS=$((WARNINGS + 1))
}

fail() {
  printf '[FAIL] %s\n' "$1"
  ERRORS=$((ERRORS + 1))
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

echo "==> CheersAI Vault macOS 打包环境预检"
echo "仓库: $ROOT_DIR"
echo "时间: $(date '+%Y-%m-%d %H:%M:%S')"
echo

echo "==> 系统信息"
sw_vers || true
RAW_ARCH="$(uname -m || true)"
echo "$RAW_ARCH"
case "$RAW_ARCH" in
  x86_64|amd64)
    ARCH_LABEL="x86_64"
    RUST_TARGET="x86_64-apple-darwin"
    ;;
  arm64|aarch64)
    ARCH_LABEL="arm64"
    RUST_TARGET="aarch64-apple-darwin"
    ;;
  *)
    ARCH_LABEL="$RAW_ARCH"
    RUST_TARGET="unknown"
    warn "未识别的 macOS 架构: $RAW_ARCH"
    ;;
esac
echo "规范化架构: $ARCH_LABEL"
echo "Rust target: $RUST_TARGET"
echo

echo "==> 必需命令"
require_cmd node "请先安装 Node.js 22+"
require_cmd corepack "请先安装支持 Corepack 的 Node.js"
require_cmd rustup "请先安装 Rustup"
require_cmd rustc "请先安装 Rust stable toolchain"
require_cmd cargo "请先安装 Rust stable toolchain"
require_cmd hdiutil "仅 macOS 支持 DMG 打包"
require_cmd codesign "请先安装 Xcode Command Line Tools"
require_cmd xattr "请先安装 Xcode Command Line Tools"
require_cmd shasum "系统缺少 shasum"
require_cmd lipo "请先安装 Xcode Command Line Tools"
echo

echo "==> 版本信息"
node -v || true
corepack pnpm -v || true
rustup show active-toolchain || true
rustc -V || true
cargo -V || true
echo

echo "==> Xcode 环境"
if xcode-select -p >/dev/null 2>&1; then
  pass "xcode-select 已配置: $(xcode-select -p)"
else
  fail "xcode-select 未配置"
fi

if xcodebuild -version >/dev/null 2>&1; then
  pass "xcodebuild 可用"
else
  warn "xcodebuild 不可用；当前可继续尝试 unsigned/ad-hoc 候选包，但正式签名链路存在风险"
fi
echo

echo "==> 仓库版本一致性"
PACKAGE_VERSION="$(node -p "require('./package.json').version")"
TAURI_VERSION="$(node -e "const fs=require('fs'); const p=JSON.parse(fs.readFileSync('./src-tauri/tauri.conf.json','utf8')); process.stdout.write(p.version)")"
if [[ "$PACKAGE_VERSION" == "$TAURI_VERSION" ]]; then
  pass "package.json 与 tauri.conf.json 版本一致: $PACKAGE_VERSION"
else
  fail "版本不一致: package.json=$PACKAGE_VERSION, tauri.conf.json=$TAURI_VERSION"
fi

TAURI_PRODUCT_NAME="$(node -e "const fs=require('fs'); const p=JSON.parse(fs.readFileSync('./src-tauri/tauri.conf.json','utf8')); process.stdout.write(p.productName)")"
echo "产品名: $TAURI_PRODUCT_NAME"
echo "版本号: $PACKAGE_VERSION"
echo

echo "==> 预检总结"
echo "错误数: $ERRORS"
echo "警告数: $WARNINGS"

if [[ "$ERRORS" -gt 0 ]]; then
  exit 1
fi
