#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

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

PNPM_SHIM_DIR=""

ensure_pnpm_command() {
  if command -v pnpm >/dev/null 2>&1; then
    return
  fi

  if ! command -v corepack >/dev/null 2>&1; then
    echo "pnpm/corepack 均不可用，无法继续打包" >&2
    exit 1
  fi

  PNPM_SHIM_DIR="$(mktemp -d "${TMPDIR:-/tmp}/cheersai-vault-pnpm.XXXXXX")"
  cat > "$PNPM_SHIM_DIR/pnpm" <<'EOF'
#!/usr/bin/env bash
exec corepack pnpm "$@"
EOF
  chmod +x "$PNPM_SHIM_DIR/pnpm"
  export PATH="$PNPM_SHIM_DIR:$PATH"
}

PRODUCT_NAME="$(node -e "const fs=require('fs'); const config=JSON.parse(fs.readFileSync('./src-tauri/tauri.conf.json','utf8')); process.stdout.write(config.productName || 'CheersAI Vault')")"
DEFAULT_VERSION="$(node -p "require('./package.json').version")"
DEFAULT_OUTPUT_DIR="$ROOT_DIR/dist"
DEFAULT_SIGN="N"
DEFAULT_SIGN_FILE="null"

normalize_product_file_stem() {
  printf '%s' "$1" | tr ' ' '_' | tr -cd '[:alnum:]_.-\n'
}

normalize_arch_label() {
  local raw_arch="$1"
  case "$raw_arch" in
    x86_64|amd64)
      printf 'x86_64'
      ;;
    arm64|aarch64)
      printf 'arm64'
      ;;
    *)
      printf '%s' "$raw_arch"
      ;;
  esac
}

detect_current_arch_label() {
  local uname_arch
  uname_arch="$(uname -m)"
  normalize_arch_label "$uname_arch"
}

detect_current_rust_target() {
  local arch_label="$1"
  case "$arch_label" in
    x86_64)
      printf 'x86_64-apple-darwin'
      ;;
    arm64)
      printf 'aarch64-apple-darwin'
      ;;
    *)
      echo "不支持的 macOS 架构: $arch_label" >&2
      exit 1
      ;;
  esac
}

CURRENT_ARCH_LABEL="$(detect_current_arch_label)"
CURRENT_RUST_TARGET="$(detect_current_rust_target "$CURRENT_ARCH_LABEL")"
PRODUCT_FILE_STEM="$(normalize_product_file_stem "$PRODUCT_NAME")"

VERSION="$DEFAULT_VERSION"
OUTPUT_DIR="$DEFAULT_OUTPUT_DIR"
SIGN="$DEFAULT_SIGN"
SIGN_FILE="$DEFAULT_SIGN_FILE"
SOURCE_DMG=""
SOURCE_APP=""
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
  - 当前设备架构: $CURRENT_ARCH_LABEL
  - 当前 Rust target: $CURRENT_RUST_TARGET
  - 打包文件名必须强制包含芯片型号标识，当前要求文件名格式为: ${PRODUCT_FILE_STEM}_${DEFAULT_VERSION}_${CURRENT_ARCH_LABEL}_portable.dmg
  - 默认只生成与当前构建环境完全匹配的单架构安装包，不生成 universal 包
  - portable DMG 作为当前 macOS 默认交付形态，需承载最新的“/cloud 默认内嵌 + 统一回退页”逻辑
  - 当前统一验收入口为 src/pages/CheersAICloudBrowser.tsx，对应 /cloud 主窗口回退页
  - 验收至少覆盖：首次启动不闪退、/cloud 默认尝试内嵌、失败时停留在统一回退页并提供重试/独立窗口/系统浏览器入口

版本号修改建议:
  1. 自动递增:
     corepack pnpm version:patch
     corepack pnpm version:minor
     corepack pnpm version:major
  2. 指定版本:
     corepack pnpm version:set -- 0.1.21
  3. 单次打包直接指定:
     bash ./scripts/build-macos-portable-dmg.sh --version 0.1.21
EOF
}

print_acceptance_checklist() {
  cat <<EOF
==> Portable DMG 验收提示
1. 首次启动主应用不闪退，主窗口可稳定进入。
2. 进入 /cloud 后默认先尝试内嵌工作区，而不是直接落到旧的独立页流程。
3. 若内嵌子 WebView 创建失败，主窗口仍保持可用，并停留在统一 Cloud 回退页。
4. 统一回退页至少可见“重新尝试嵌入式打开”“在独立窗口打开”“在系统浏览器打开”三个入口。
5. 验收记录需注明当前 DMG 是否基于最新“默认内嵌 + 统一回退页”逻辑构建。
EOF
}

prepare_version_if_needed() {
  if [[ "$VERSION_PREPARED" == "Y" ]]; then
    return
  fi

  echo "==> Preparing unified version metadata"
  run_pnpm version:prepare
  VERSION_PREPARED="Y"
}

locate_existing_app() {
  local target_app="$ROOT_DIR/src-tauri/target/$CURRENT_RUST_TARGET/release/bundle/macos/${PRODUCT_NAME}.app"
  if [[ -d "$target_app" ]]; then
    printf '%s' "$target_app"
    return
  fi

  find "$ROOT_DIR/src-tauri/target" \
    -path "*/bundle/macos/${PRODUCT_NAME}.app" \
    -print | head -n 1
}

app_executable_name() {
  local app_dir="$1"
  local info_plist="$app_dir/Contents/Info.plist"
  if [[ ! -f "$info_plist" ]]; then
    echo "缺少 Info.plist: $info_plist" >&2
    exit 1
  fi

  /usr/libexec/PlistBuddy -c 'Print :CFBundleExecutable' "$info_plist"
}

app_binary_path() {
  local app_dir="$1"
  local executable_name
  executable_name="$(app_executable_name "$app_dir")"
  printf '%s/Contents/MacOS/%s' "$app_dir" "$executable_name"
}

binary_architectures() {
  local binary_path="$1"
  lipo -archs "$binary_path" 2>/dev/null || true
}

verify_single_arch_binary() {
  local binary_path="$1"
  local archs
  archs="$(binary_architectures "$binary_path" | xargs)"
  if [[ -z "$archs" ]]; then
    echo "无法识别二进制架构: $binary_path" >&2
    exit 1
  fi

  if [[ "$archs" == *" "* ]]; then
    echo "检测到多架构二进制，当前流程仅允许单架构产物: $archs" >&2
    exit 1
  fi

  local normalized_arch
  normalized_arch="$(normalize_arch_label "$archs")"
  if [[ "$normalized_arch" != "$CURRENT_ARCH_LABEL" ]]; then
    echo "二进制架构与当前设备不匹配: binary=$normalized_arch current=$CURRENT_ARCH_LABEL" >&2
    exit 1
  fi
}

existing_app_matches_current_arch() {
  local app_dir="$1"
  local binary_path
  binary_path="$(app_binary_path "$app_dir")"
  if [[ ! -f "$binary_path" ]]; then
    return 1
  fi

  local archs
  archs="$(binary_architectures "$binary_path" | xargs)"
  if [[ -z "$archs" || "$archs" == *" "* ]]; then
    return 1
  fi

  [[ "$(normalize_arch_label "$archs")" == "$CURRENT_ARCH_LABEL" ]]
}

build_single_arch_app() {
  ensure_pnpm_command
  echo "==> Building single-arch app for $CURRENT_RUST_TARGET"
  if run_pnpm tauri build --bundles dmg --target "$CURRENT_RUST_TARGET"; then
    return
  fi

  local candidate_app
  candidate_app="$(locate_existing_app)"
  if [[ -n "$candidate_app" ]] && existing_app_matches_current_arch "$candidate_app"; then
    echo "==> Tauri 原始 DMG 步骤失败，但已产出匹配当前架构的 .app，继续执行 portable DMG 封装"
    SOURCE_APP="$candidate_app"
    return
  fi

  echo "Tauri 构建失败，且未找到可复用的当前架构 .app" >&2
  exit 1
}

verify_output_filename() {
  local output_path="$1"
  local output_name
  output_name="$(basename "$output_path")"
  local expected_name="${PRODUCT_FILE_STEM}_${VERSION}_${CURRENT_ARCH_LABEL}_portable.dmg"
  if [[ "$output_name" != "$expected_name" ]]; then
    echo "打包文件名不符合规范: actual=$output_name expected=$expected_name" >&2
    exit 1
  fi
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
  CANDIDATE_APP="$(locate_existing_app)"
  if [[ -n "$CANDIDATE_APP" ]] && existing_app_matches_current_arch "$CANDIDATE_APP"; then
    SOURCE_APP="$CANDIDATE_APP"
    echo "==> Reusing existing single-arch app bundle: $SOURCE_APP"
  else
    build_single_arch_app
  fi
fi

if [[ -z "$SOURCE_DMG" ]]; then
  if [[ -z "$SOURCE_APP" ]]; then
    SOURCE_APP="$(locate_existing_app)"
  fi
  if [[ -d "${SOURCE_APP:-}" ]]; then
    echo "==> Source DMG not found, fallback to app bundle: $SOURCE_APP"
  fi
fi

if [[ -n "$SOURCE_DMG" && ! -f "$SOURCE_DMG" ]]; then
  echo "Source DMG path is invalid: $SOURCE_DMG" >&2
  exit 1
fi

if [[ -z "$SOURCE_DMG" && ! -d "$SOURCE_APP" ]]; then
  echo "Source DMG and app bundle not found for version ${VERSION}" >&2
  exit 1
fi

ARCH_SUFFIX="$CURRENT_ARCH_LABEL"
OUTPUT_DMG="$OUTPUT_DIR/${PRODUCT_FILE_STEM}_${VERSION}_${ARCH_SUFFIX}_portable.dmg"
TEMP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/cheersai-vault-dmg.XXXXXX")"
MOUNT_DIR="$TEMP_DIR/mount"
STAGING_DIR="$TEMP_DIR/staging"
APP_PATH="$STAGING_DIR/${PRODUCT_NAME}.app"

cleanup() {
  if mount | grep -q "$MOUNT_DIR"; then
    hdiutil detach "$MOUNT_DIR" >/dev/null 2>&1 || true
  fi
  if [[ -n "$PNPM_SHIM_DIR" && -d "$PNPM_SHIM_DIR" ]]; then
    rm -rf "$PNPM_SHIM_DIR"
  fi
  rm -rf "$TEMP_DIR"
}
trap cleanup EXIT

mkdir -p "$MOUNT_DIR" "$STAGING_DIR" "$OUTPUT_DIR"

if [[ -n "$SOURCE_DMG" ]]; then
  echo "==> Mounting source DMG"
  hdiutil attach -nobrowse -readonly -mountpoint "$MOUNT_DIR" "$SOURCE_DMG" >/dev/null

  echo "==> Copying app bundle from source DMG"
  cp -R "$MOUNT_DIR/${PRODUCT_NAME}.app" "$APP_PATH"
  hdiutil detach "$MOUNT_DIR" >/dev/null
else
  echo "==> Copying app bundle from Tauri bundle output"
  cp -R "$SOURCE_APP" "$APP_PATH"
fi

verify_single_arch_binary "$(app_binary_path "$APP_PATH")"

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

verify_output_filename "$OUTPUT_DMG"

echo "==> Portable DMG created"
echo "PATH: $OUTPUT_DMG"
shasum -a 256 "$OUTPUT_DMG"
print_acceptance_checklist
