#!/usr/bin/env bash
# =============================================================================
# CheersAI Vault — Portable DMG 快速封装（不做 pnpm/cargo 编译，只做最后一段）
# 固化进 scripts/ 作为 DMG skill 的第二入口，完全复用既有的 version-manager 规范。
#
# 场景：
#   - Trae / IDE sandbox 已经把前端 vite build + Rust cargo build 全跑完，
#     .app 存在于 src-tauri/target/<rust-target>/release/bundle/macos/，但
#     sandbox 禁止 hdiutil 访问 /dev/rdisk*，导致最后 DMG 包装失败；
#   - 用户只需在真终端执行本脚本（≈ 20~40s），即可拿到最终 DMG。
#
# 调用规范（不重新定义版本 bump 流程，全部沿用 version-manager.js）：
#   1. bash scripts/build-macos-portable-dmg-quick.sh
#        默认：package.json version + 本机 arch + ad-hoc 签名
#   2. bash scripts/build-macos-portable-dmg-quick.sh --version 0.1.42
#        指定版本（已 bump 过 5 个版本锚点后常用）
#   3. bash scripts/build-macos-portable-dmg-quick.sh --app /abs/path/to/Vault.app
#        指定已存在的 .app（不自动查 target 目录）
#   4. bash scripts/build-macos-portable-dmg-quick.sh \
#          --sign Y --sign-file /abs/path/to/identity.txt
#        使用自定义 codesign identity（首行是 identity 字符串，一行）
#
# 失败出口（显式提示去用主流程 build:dmg:portable）：
#   - 若找不到 .app，立即 exit 1，并提醒先执行：
#     corepack pnpm version:patch   # 或 minor/major
#     corepack pnpm build:dmg:portable
# =============================================================================
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

if [[ -d "$HOME/.cargo/bin" ]]; then
  export PATH="$HOME/.cargo/bin:$PATH"
fi

run_pnpm() {
  if command -v pnpm >/dev/null 2>&1; then
    pnpm "$@"
  else
    corepack pnpm "$@"
  fi
}

normalize_arch() {
  case "$1" in
    x86_64|amd64) printf 'x86_64' ;;
    arm64|aarch64) printf 'arm64' ;;
    *) printf '%s' "$1" ;;
  esac
}

PRODUCT_NAME="$(node -e "const fs=require('fs'); const cfg=JSON.parse(fs.readFileSync('./src-tauri/tauri.conf.json','utf8')); process.stdout.write(cfg.productName || 'CheersAI Vault')")"
FILE_STEM="$(printf '%s' "$PRODUCT_NAME" | tr ' ' '_' | tr -cd '[:alnum:]_.-\n')"
DEFAULT_VERSION="$(node -p "require('./package.json').version")"
CURRENT_ARCH="$(normalize_arch "$(uname -m)")"
case "$CURRENT_ARCH" in
  x86_64) RUST_TARGET="x86_64-apple-darwin" ;;
  arm64)  RUST_TARGET="aarch64-apple-darwin" ;;
  *) echo "不支持的 macOS 架构: $CURRENT_ARCH" >&2; exit 1 ;;
esac

VERSION="$DEFAULT_VERSION"
ARCH="$CURRENT_ARCH"
OUTPUT_DIR="$REPO_ROOT/dist"
SIGN="N"
SIGN_FILE="null"
APP=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --version)
      VERSION="${2:-}"
      shift 2
      ;;
    --arch)
      ARCH="$(normalize_arch "${2:-}")"
      shift 2
      ;;
    --output-dir)
      OUTPUT_DIR="${2:-}"
      shift 2
      ;;
    --sign)
      SIGN="${2:-}"
      shift 2
      ;;
    --sign-file)
      SIGN_FILE="${2:-}"
      shift 2
      ;;
    --app)
      APP="${2:-}"
      shift 2
      ;;
    -h|--help)
      sed -n '2,47p' "$0"
      exit 0
      ;;
    *)
      echo "Unknown argument: $1" >&2
      exit 2
      ;;
  esac
done

if [[ "$SIGN" != "Y" && "$SIGN" != "N" ]]; then
  echo "--sign 仅支持 Y 或 N" >&2
  exit 1
fi
if [[ "$SIGN" == "Y" && ( -z "$SIGN_FILE" || "$SIGN_FILE" == "null" || ! -f "$SIGN_FILE" ) ]]; then
  echo "--sign=Y 时 --sign-file 必须指向一个首行为 codesign identity 的文本文件" >&2
  exit 1
fi

echo "==> [版本检查] 调用既有 version-manager:check（沿用旧规范，不重造）"
run_pnpm version:check

if [[ -z "$APP" ]]; then
  CANDIDATE="$REPO_ROOT/src-tauri/target/$RUST_TARGET/release/bundle/macos/${PRODUCT_NAME}.app"
  if [[ -d "$CANDIDATE" ]]; then
    APP="$CANDIDATE"
  else
    APP="$(find "$REPO_ROOT/src-tauri/target" -path "*/bundle/macos/${PRODUCT_NAME}.app" -print 2>/dev/null | head -n 1 || true)"
  fi
fi
if [[ -z "$APP" || ! -d "$APP" ]]; then
  echo "ERROR: 未找到已构建好的 ${PRODUCT_NAME}.app（本脚本不做 pnpm build / cargo build）。" >&2
  echo "请先在真终端执行主流程构建（沿用既有的 release skill）：" >&2
  echo "  cd $REPO_ROOT" >&2
  echo "  corepack pnpm version:patch   # 或 version:minor / version:major" >&2
  echo "  corepack pnpm build:dmg:portable" >&2
  exit 1
fi

mkdir -p "$OUTPUT_DIR"
OUTPUT_DMG="$OUTPUT_DIR/${FILE_STEM}_${VERSION}_${ARCH}_portable.dmg"

STAGING="$(mktemp -d "${TMPDIR:-/tmp}/cheersai-vault-quickdmg.XXXXXX")"
cleanup() {
  rm -rf "$STAGING" >/dev/null 2>&1 || true
}
trap cleanup EXIT

echo ""
echo "============================================================"
echo " CheersAI Vault — Portable DMG Quick Builder（不重新编译）"
echo " repo      : $REPO_ROOT"
echo " version   : $VERSION"
echo " arch      : $ARCH"
echo " .app      : $APP"
echo " output    : $OUTPUT_DMG"
echo " sign mode : ad-hoc=$([[ "$SIGN" == "N" ]] && echo YES || echo NO), custom=$([[ "$SIGN" == "Y" ]] && echo YES || echo NO)"
echo "============================================================"
echo ""

echo "[1/7] 复制 .app 到 staging 目录..."
cp -R "$APP" "$STAGING/${PRODUCT_NAME}.app"

echo "[2/7] 校验二进制架构（要求单架构 $ARCH）..."
EXE="$(/usr/libexec/PlistBuddy -c "Print :CFBundleExecutable" "$STAGING/${PRODUCT_NAME}.app/Contents/Info.plist")"
ARCHS="$(lipo -archs "$STAGING/${PRODUCT_NAME}.app/Contents/MacOS/$EXE" 2>/dev/null | xargs || echo "")"
if [[ "$ARCHS" != "$ARCH" ]]; then
  echo "ERROR: 二进制不匹配期望的单架构: actual='$ARCHS' expect='$ARCH'" >&2
  exit 1
fi
echo "   arch OK: $ARCHS"

echo "[3/7] 清除扩展属性..."
xattr -cr "$STAGING/${PRODUCT_NAME}.app" || true

echo "[4/7] 执行签名（ad-hoc / 自定义 identity）并 strict 校验..."
if [[ "$SIGN" == "Y" ]]; then
  SIGN_IDENTITY="$(head -n 1 "$SIGN_FILE" | tr -d '\r' | xargs)"
  if [[ -z "$SIGN_IDENTITY" ]]; then
    echo "ERROR: --sign-file 内容为空（首行需为可用的 codesign identity）" >&2
    exit 1
  fi
  codesign --force --deep --sign "$SIGN_IDENTITY" "$STAGING/${PRODUCT_NAME}.app"
else
  codesign --force --deep --sign - "$STAGING/${PRODUCT_NAME}.app"
fi
codesign --verify --deep --strict "$STAGING/${PRODUCT_NAME}.app" >/dev/null

echo "[5/7] 添加 /Applications 软链（安装时直接拖）..."
ln -s /Applications "$STAGING/Applications"

echo "[6/7] 使用 hdiutil 创建 UDZO 只读 DMG..."
hdiutil create \
  -volname "$PRODUCT_NAME" \
  -srcfolder "$STAGING" \
  -ov \
  -format UDZO \
  "$OUTPUT_DMG" >/dev/null

echo "[7/7] 校验产物..."
SIZE_BYTES="$(stat -f%z "$OUTPUT_DMG" 2>/dev/null || echo 0)"
SHA256="$(shasum -a 256 "$OUTPUT_DMG" | awk '{print $1}')"
SHORT_VER="$(/usr/libexec/PlistBuddy -c "Print :CFBundleShortVersionString" "$STAGING/${PRODUCT_NAME}.app/Contents/Info.plist" 2>/dev/null || echo "")"
BUNDLE_VER="$(/usr/libexec/PlistBuddy -c "Print :CFBundleVersion" "$STAGING/${PRODUCT_NAME}.app/Contents/Info.plist" 2>/dev/null || echo "")"

NUMFMT_BIN="$(command -v numfmt || true)"
if [[ -n "$NUMFMT_BIN" ]]; then
  SIZE_HUMAN="$("$NUMFMT_BIN" --to=iec --suffix=B "$SIZE_BYTES" 2>/dev/null || echo "${SIZE_BYTES}B")"
else
  SIZE_HUMAN="${SIZE_BYTES}B"
fi

echo ""
echo "============================================================"
echo "🎉 DMG 生成成功！"
echo "   PATH              : $OUTPUT_DMG"
echo "   SIZE              :  $SIZE_HUMAN"
echo "   SHA256            : $SHA256"
echo "   CFBundleShortVersion : $SHORT_VER"
echo "   CFBundleVersion      : $BUNDLE_VER"
echo "   Architectures     : $ARCHS (single arch ✅)"
echo "============================================================"
echo ""
echo "下一步（沿用既有的 verify-macos-portable-dmg.sh 脚本，10 项自动校验）："
echo "  cd $REPO_ROOT"
echo "  bash scripts/verify-macos-portable-dmg.sh --version $VERSION --arch $ARCH"
echo ""
