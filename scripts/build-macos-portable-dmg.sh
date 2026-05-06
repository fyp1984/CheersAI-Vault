#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

PRODUCT_NAME="CheersAI Desktop"
DEFAULT_VERSION="$(node -p "require('./package.json').version")"
DEFAULT_OUTPUT_DIR="/Users/FYP/Documents/WorkSpace/CheersAI/subproducts/CheersAI-Vault/dist"
DEFAULT_SIGN="N"
DEFAULT_SIGN_FILE="null"

VERSION="$DEFAULT_VERSION"
OUTPUT_DIR="$DEFAULT_OUTPUT_DIR"
SIGN="$DEFAULT_SIGN"
SIGN_FILE="$DEFAULT_SIGN_FILE"
SOURCE_DMG=""
VERSION_PREPARED="N"

print_help() {
  cat <<EOF
Usage:
  bash ./scripts/build-macos-portable-dmg.sh [options]

Options:
  --version <version>         打包版本号，默认值: package.json 当前 version ($DEFAULT_VERSION)
  --output-dir <path>         DMG 输出目录，默认值: $DEFAULT_OUTPUT_DIR
  --sign <Y|N>                是否使用签名文件执行正式签名，默认值: $DEFAULT_SIGN
  --sign-file <path|null>     签名文件路径，默认值: $DEFAULT_SIGN_FILE
  --source-dmg <path>         使用现有源 DMG 二次封装，默认值: null
  -h, --help                  显示帮助

默认行为说明:
  - 未传 --version 时，使用 package.json 中的 version
  - 未传 --output-dir 时，输出到 $DEFAULT_OUTPUT_DIR
  - --sign=N 时，使用 ad-hoc 签名，适合内部验证包
  - --sign=Y 时，必须传入 --sign-file，且签名文件首行应为可用的 codesign identity

版本号修改建议:
  1. 自动递增:
     pnpm version:patch
     pnpm version:minor
     pnpm version:major
  2. 指定版本:
     pnpm version:set -- 0.1.21
  3. 单次打包直接指定:
     bash ./scripts/build-macos-portable-dmg.sh --version 0.1.21
EOF
}

prepare_version_if_needed() {
  if [[ "$VERSION_PREPARED" == "Y" ]]; then
    return
  fi

  echo "==> Preparing unified version metadata"
  pnpm version:prepare
  VERSION_PREPARED="Y"
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --version)
      VERSION="${2:-}"
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
    --source-dmg)
      SOURCE_DMG="${2:-}"
      shift 2
      ;;
    -h|--help)
      print_help
      exit 0
      ;;
    *)
      echo "Unknown argument: $1" >&2
      print_help >&2
      exit 1
      ;;
  esac
done

if [[ -z "$VERSION" ]]; then
  echo "--version 不能为空" >&2
  exit 1
fi

if [[ -z "$OUTPUT_DIR" ]]; then
  echo "--output-dir 不能为空" >&2
  exit 1
fi

if [[ "$SIGN" != "Y" && "$SIGN" != "N" ]]; then
  echo "--sign 仅支持 Y 或 N" >&2
  exit 1
fi

if [[ "$VERSION" != "$DEFAULT_VERSION" ]]; then
  echo "==> Setting package version to ${VERSION}"
  node ./scripts/version-manager.js set "$VERSION"
  VERSION_PREPARED="Y"
fi

if [[ "${1:-}" == "--source-dmg" ]]; then
  :
fi

if [[ -n "$SOURCE_DMG" ]]; then
  prepare_version_if_needed
else
  echo "==> Building source DMG with Tauri"
  pnpm tauri build --bundles dmg
fi

if [[ -z "$SOURCE_DMG" ]]; then
  SOURCE_DMG="$(find "$ROOT_DIR/src-tauri/target/release/bundle/dmg" -maxdepth 1 -name "${PRODUCT_NAME}_${VERSION}_*.dmg" | head -n 1)"
fi

if [[ -z "$SOURCE_DMG" || ! -f "$SOURCE_DMG" ]]; then
  echo "Source DMG not found for version ${VERSION}" >&2
  exit 1
fi

ARCH_SUFFIX="$(basename "$SOURCE_DMG" | sed -E "s/^${PRODUCT_NAME// /\\ }_${VERSION}_(.+)\.dmg$/\\1/")"
OUTPUT_DMG="$OUTPUT_DIR/${PRODUCT_NAME}_${VERSION}_${ARCH_SUFFIX}_portable.dmg"
TEMP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/cheersai-vault-dmg.XXXXXX")"
MOUNT_DIR="$TEMP_DIR/mount"
STAGING_DIR="$TEMP_DIR/staging"
APP_PATH="$STAGING_DIR/${PRODUCT_NAME}.app"

cleanup() {
  if mount | grep -q "$MOUNT_DIR"; then
    hdiutil detach "$MOUNT_DIR" >/dev/null 2>&1 || true
  fi
  rm -rf "$TEMP_DIR"
}
trap cleanup EXIT

mkdir -p "$MOUNT_DIR" "$STAGING_DIR" "$OUTPUT_DIR"

echo "==> Mounting source DMG"
hdiutil attach -nobrowse -readonly -mountpoint "$MOUNT_DIR" "$SOURCE_DMG" >/dev/null

echo "==> Copying app bundle"
cp -R "$MOUNT_DIR/${PRODUCT_NAME}.app" "$APP_PATH"
hdiutil detach "$MOUNT_DIR" >/dev/null

echo "==> Clearing extended attributes"
xattr -cr "$APP_PATH" || true

if [[ "$SIGN" == "Y" ]]; then
  if [[ -z "$SIGN_FILE" || "$SIGN_FILE" == "null" ]]; then
    echo "--sign=Y 时必须传入 --sign-file" >&2
    exit 1
  fi

  if [[ ! -f "$SIGN_FILE" ]]; then
    echo "Sign file not found: $SIGN_FILE" >&2
    exit 1
  fi

  SIGN_IDENTITY="$(head -n 1 "$SIGN_FILE" | tr -d '\r' | xargs)"
  if [[ -z "$SIGN_IDENTITY" ]]; then
    echo "签名文件首行不能为空，请填写可用的 codesign identity" >&2
    exit 1
  fi

  echo "==> Re-signing app bundle with custom identity from sign file"
  codesign --force --deep --sign "$SIGN_IDENTITY" "$APP_PATH"
else
  echo "==> Re-signing app bundle with ad-hoc signature (default sign=N)"
  codesign --force --deep --sign - "$APP_PATH"
fi

echo "==> Verifying app bundle signature"
codesign --verify --deep --strict --verbose=2 "$APP_PATH"

echo "==> Preparing DMG layout"
ln -s /Applications "$STAGING_DIR/Applications"

echo "==> Creating portable DMG"
hdiutil create \
  -volname "$PRODUCT_NAME" \
  -srcfolder "$STAGING_DIR" \
  -ov \
  -format UDZO \
  "$OUTPUT_DMG" >/dev/null

echo "==> Portable DMG created"
echo "PATH: $OUTPUT_DMG"
shasum -a 256 "$OUTPUT_DMG"
