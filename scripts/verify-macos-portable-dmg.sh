#!/usr/bin/env bash
# =============================================================================
# CheersAI Desktop - Portable DMG verification script (official skill entry)
# Version management: STRICTLY reuse the existing version-manager.js /
# bump-version.js mechanism. DO NOT redefine any version bump flow here.
#
# Usage:
#   1. bash scripts/verify-macos-portable-dmg.sh
#        (auto-detect version from package.json + native arch)
#   2. bash scripts/verify-macos-portable-dmg.sh --version 0.1.42
#   3. bash scripts/verify-macos-portable-dmg.sh --version 0.1.42 --arch x86_64
#   4. bash scripts/verify-macos-portable-dmg.sh --dmg ./dist/xxx.dmg
#
# Verifies 10 contracts:
#   1. DMG file exists
#   2. File size >= 10 MiB (sanity UDZO-compression lower bound, not a cap)
#   3. SHA-256 is 64 hex chars
#   4. DMG mounts cleanly via hdiutil attach
#   5. Mounted volume contains a .app bundle
#   6. CFBundleShortVersionString == specified version
#   7. CFBundleVersion == specified version
#   8. Mach-O is single arch, matches --arch / native arch
#   9. codesign -dvv is readable (bundle is signed, at least ad-hoc)
#  10. codesign --verify --deep --strict passes
# =============================================================================
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${REPO_ROOT}"

normalize_arch_label() {
  case "$1" in
    x86_64|amd64) printf 'x86_64' ;;
    arm64|aarch64) printf 'arm64' ;;
    *) printf '%s' "$1" ;;
  esac
}

CURRENT_ARCH_LABEL="$(normalize_arch_label "$(uname -m)")"
DEFAULT_VERSION="$(node -p "require('./package.json').version")"
PRODUCT_NAME="$(node -e "const fs=require('fs'); const cfg=JSON.parse(fs.readFileSync('./src-tauri/tauri.conf.json','utf8')); process.stdout.write(cfg.productName || 'CheersAI Desktop')")"
PRODUCT_FILE_STEM="$(printf '%s' "${PRODUCT_NAME}" | tr ' ' '_' | tr -cd '[:alnum:]_.-\n')"

VERSION="${DEFAULT_VERSION}"
ARCH_LABEL="${CURRENT_ARCH_LABEL}"
DMG_PATH=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --version)
      VERSION="${2:-}"
      shift 2
      ;;
    --arch)
      ARCH_LABEL="$(normalize_arch_label "${2:-}")"
      shift 2
      ;;
    --dmg)
      DMG_PATH="${2:-}"
      shift 2
      ;;
    -h|--help)
      sed -n '2,36p' "$0"
      exit 0
      ;;
    *)
      echo "Unknown argument: $1" >&2
      exit 2
      ;;
  esac
done

if [[ -z "${DMG_PATH}" ]]; then
  DMG_PATH="${REPO_ROOT}/dist/${PRODUCT_FILE_STEM}_${VERSION}_${ARCH_LABEL}_portable.dmg"
fi

EXPECTED_SHORT_VERSION="${VERSION}"
EXPECTED_BUNDLE_VERSION="${VERSION}"
MIN_BYTES=$((10 * 1024 * 1024))

echo "=============================================="
echo " CheersAI Desktop Portable DMG Verifier"
echo " version : ${VERSION}"
echo " arch    : ${ARCH_LABEL}"
echo " DMG     : ${DMG_PATH}"
echo "=============================================="
echo ""

PASS=0
FAIL=0
check() {
  local name="$1"
  local rc="${2:-1}"
  if [[ "${rc}" -eq 0 ]]; then
    echo "[PASS] ${name}"
    PASS=$((PASS + 1))
  else
    echo "[FAIL] ${name}"
    FAIL=$((FAIL + 1))
  fi
}

[[ -f "${DMG_PATH}" ]]
check "1. DMG file exists" $?

SIZE_BYTES="$(stat -f%z "${DMG_PATH}" 2>/dev/null || echo 0)"
echo "   size bytes = ${SIZE_BYTES}"
[[ "${SIZE_BYTES}" -gt "${MIN_BYTES}" ]]
check "2. DMG size >= 10 MiB (UDZO sanity bound)" $?

SHA256="$(shasum -a 256 "${DMG_PATH}" | awk '{print $1}')"
echo "   SHA256 = ${SHA256}"
[[ -n "${SHA256}" && ${#SHA256} -eq 64 ]]
check "3. SHA-256 is 64-hex valid" $?

MOUNT_POINT="$(mktemp -d "/private/tmp/vault-dmg-verify.XXXXXX")"
cleanup() {
  hdiutil detach "${MOUNT_POINT}" -force >/dev/null 2>&1 || true
  rmdir "${MOUNT_POINT}" >/dev/null 2>&1 || true
}
trap cleanup EXIT

hdiutil attach -nobrowse -readonly -mountpoint "${MOUNT_POINT}" "${DMG_PATH}" >/dev/null 2>&1
MOUNT_RC=$?
check "4. DMG mounts (hdiutil attach)" ${MOUNT_RC}
if [[ ${MOUNT_RC} -ne 0 ]]; then
  echo "   mount failed, skipping further checks"
  echo ""
  echo "TOTAL: PASS=${PASS} FAIL=${FAIL}"
  [[ ${FAIL} -eq 0 ]]
  exit
fi

APP_PATH="$(find "${MOUNT_POINT}" -maxdepth 2 -name "*.app" -type d | head -n 1)"
[[ -n "${APP_PATH}" && -d "${APP_PATH}" ]]
check "5. DMG contains a .app bundle" $?

SHORT_VER="$(/usr/libexec/PlistBuddy -c "Print :CFBundleShortVersionString" "${APP_PATH}/Contents/Info.plist" 2>/dev/null || echo "")"
echo "   CFBundleShortVersionString = ${SHORT_VER} / expect=${EXPECTED_SHORT_VERSION}"
[[ "${SHORT_VER}" == "${EXPECTED_SHORT_VERSION}" ]]
check "6. CFBundleShortVersionString matches version" $?

BUNDLE_VER="$(/usr/libexec/PlistBuddy -c "Print :CFBundleVersion" "${APP_PATH}/Contents/Info.plist" 2>/dev/null || echo "")"
echo "   CFBundleVersion = ${BUNDLE_VER} / expect=${EXPECTED_BUNDLE_VERSION}"
[[ "${BUNDLE_VER}" == "${EXPECTED_BUNDLE_VERSION}" ]]
check "7. CFBundleVersion matches version" $?

EXE="$(/usr/libexec/PlistBuddy -c "Print :CFBundleExecutable" "${APP_PATH}/Contents/Info.plist" 2>/dev/null || echo "")"
ARCHS="$(lipo -archs "${APP_PATH}/Contents/MacOS/${EXE}" 2>/dev/null | xargs || echo "")"
echo "   binary archs = ${ARCHS}"
[[ "${ARCHS}" == "${ARCH_LABEL}" ]]
check "8. Single arch equals ${ARCH_LABEL} (not universal)" $?

codesign -dvv "${APP_PATH}" >/dev/null 2>&1
check "9. codesign -dvv readable (bundle signed)" $?
codesign --verify --deep --strict "${APP_PATH}" >/dev/null 2>&1
check "10. codesign strict verify passes" $?

echo ""
echo "TOTAL: PASS=${PASS} FAIL=${FAIL}"
echo ""
if [[ ${FAIL} -eq 0 ]]; then
  echo "OK: All DMG verifications passed"
  exit 0
else
  echo "NG: ${FAIL} verification(s) FAILED"
  exit 1
fi
