#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

PRODUCT_NAME="CheersAI Desktop"
VERSION="$(node -p "require('./package.json').version")"
SOURCE_DMG="${2:-}"

if [[ "${1:-}" == "--source-dmg" ]]; then
  if [[ -z "$SOURCE_DMG" ]]; then
    echo "Usage: $0 [--source-dmg /absolute/path/to/source.dmg]" >&2
    exit 1
  fi
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
OUTPUT_DMG="$ROOT_DIR/dist/${PRODUCT_NAME}_${VERSION}_${ARCH_SUFFIX}_portable.dmg"
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

mkdir -p "$MOUNT_DIR" "$STAGING_DIR" "$ROOT_DIR/dist"

echo "==> Mounting source DMG"
hdiutil attach -nobrowse -readonly -mountpoint "$MOUNT_DIR" "$SOURCE_DMG" >/dev/null

echo "==> Copying app bundle"
cp -R "$MOUNT_DIR/${PRODUCT_NAME}.app" "$APP_PATH"
hdiutil detach "$MOUNT_DIR" >/dev/null

echo "==> Clearing extended attributes"
xattr -cr "$APP_PATH" || true

echo "==> Re-signing app bundle with ad-hoc signature"
codesign --force --deep --sign - "$APP_PATH"

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
