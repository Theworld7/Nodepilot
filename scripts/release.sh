#!/usr/bin/env bash
#===============================================================================
# nodepilot macOS DMG Release Script
#
# 功能：
#   1. 从 Cargo.toml 读取当前版本，自动递增补丁号 (x.y.z → x.y.z+1)
#   2. 同步版本号到 Cargo.toml、tauri.conf.json、package.json
#   3. 检查/生成 Tauri updater 签名密钥，构建 DMG + 更新包
#   4. 生成 latest.json 更新清单（Tauri updater 格式）
#   5. 通过 gh CLI 创建 GitHub Draft Release 并上传所有产物
#
# 用法：
#   ./scripts/release.sh                          # 自动递增补丁号
#   ./scripts/release.sh 0.3.0                    # 指定版本号
#   ./scripts/release.sh --dry-run                # 仅检查，不实际构建
#   ./scripts/release.sh --target universal       # 构建 universal binary
#
# 依赖：
#   - Rust toolchain (cargo, aarch64-apple-darwin / universal target)
#   - pnpm + Node.js
#   - gh CLI (已登录)
#   - Tauri updater 密钥对（首次运行自动生成）
#===============================================================================

set -euo pipefail

#-------------------------------------------------------------------------------
# 配置区
#-------------------------------------------------------------------------------
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
TAURI_DIR="$PROJECT_DIR/src-tauri"
CARGO_TOML="$TAURI_DIR/Cargo.toml"
TAURI_CONF="$TAURI_DIR/tauri.conf.json"
PACKAGE_JSON="$PROJECT_DIR/package.json"
KEY_DIR="$HOME/.nodepilot-keys"
PRIVATE_KEY="$KEY_DIR/nodepilot.key"
PUBLIC_KEY="$KEY_DIR/nodepilot.key.pub"
REPO="Theworld7/Nodepilot"
RELEASE_NOTES_FILE="$PROJECT_DIR/RELEASE_NOTES.md"

# Tauri 构建目标
TARGET_TRIPLE="aarch64-apple-darwin"
TARGET_LABEL="aarch64"
DRY_RUN=false

#-------------------------------------------------------------------------------
# 辅助函数
#-------------------------------------------------------------------------------
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

log_info()  { echo -e "${GREEN}[INFO]${NC}  $*"; }
log_warn()  { echo -e "${YELLOW}[WARN]${NC}  $*"; }
log_error() { echo -e "${RED}[ERROR]${NC} $*"; }
log_step()  { echo -e "\n${CYAN}==>${NC} ${CYAN}$*${NC}"; }

die() {
  log_error "$@"
  exit 1
}

# 从 Cargo.toml 读取 version 字段
get_current_version() {
  grep '^version\s*=' "$CARGO_TOML" \
    | head -1 \
    | sed 's/.*"\([^"]*\)".*/\1/'
}

# 解析并递增补丁号: "0.2.2" → "0.2.3"
bump_patch() {
  local ver="$1"
  local major minor patch
  IFS='.' read -r major minor patch <<< "$ver"
  echo "${major}.${minor}.$((patch + 1))"
}

# 验证版本号格式
validate_version() {
  [[ "$1" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || die "无效版本号: $1 (需要 x.y.z 格式)"
}

# 检查前置依赖
check_prerequisites() {
  log_step "检查前置依赖"

  local missing=()

  command -v cargo  &>/dev/null || missing+=("cargo (Rust)")
  command -v pnpm   &>/dev/null || missing+=("pnpm")
  command -v node   &>/dev/null || missing+=("node")
  command -v gh     &>/dev/null || missing+=("gh (GitHub CLI)")

  if [[ ${#missing[@]} -gt 0 ]]; then
    die "缺少依赖: ${missing[*]}"
  fi

  # 检查 gh 登录状态
  if ! gh auth status &>/dev/null 2>&1; then
    die "gh 未登录，请先执行: gh auth login"
  fi

  # 检查是否在项目根目录
  [[ -f "$CARGO_TOML" ]] || die "未找到 $CARGO_TOML，请在 nodepilot 仓库内运行"

  log_info "依赖检查通过"
}

#-------------------------------------------------------------------------------
# 步骤 1: 版本号处理
#-------------------------------------------------------------------------------
handle_version() {
  log_step "版本号处理"

  local current_ver
  current_ver=$(get_current_version)
  log_info "当前版本: $current_ver"

  local new_ver
  if [[ -n "${1:-}" ]] && [[ "$1" != "--dry-run" ]] && [[ "$1" != "--target" ]]; then
    # 用户指定了版本号
    new_ver="$1"
    validate_version "$new_ver"
    log_info "指定版本: $new_ver"
  else
    # 自动递增补丁号
    new_ver=$(bump_patch "$current_ver")
    validate_version "$new_ver"
    log_info "自动递增: $current_ver → $new_ver"
  fi

  echo "$new_ver"
}

# 更新所有版本文件
update_version_files() {
  local old_ver="$1"
  local new_ver="$2"

  log_step "更新版本号: $old_ver → $new_ver"

  if [[ "$DRY_RUN" == true ]]; then
    log_info "[DRY-RUN] 跳过文件修改"
    return
  fi

  # 1. Cargo.toml
  log_info "更新 $CARGO_TOML"
  sed -i '' "s/^version = \"$old_ver\"/version = \"$new_ver\"/" "$CARGO_TOML"

  # 2. tauri.conf.json
  log_info "更新 $TAURI_CONF"
  # 使用 sed 处理 JSON 中的 version 字段（精确匹配 "version": "x.y.z" 模式）
  sed -i '' "s/\"version\": \"$old_ver\"/\"version\": \"$new_ver\"/" "$TAURI_CONF"

  # 3. package.json（仅当需要保持一致的场合）
  # 由于 package.json 当前是 "0.0.0"，这里跳过；若将来需要同步，取消注释：
  # log_info "更新 $PACKAGE_JSON"
  # sed -i '' "s/\"version\": \"$old_ver\"/\"version\": \"$new_ver\"/" "$PACKAGE_JSON"

  log_info "版本文件更新完成"
}

#-------------------------------------------------------------------------------
# 步骤 2: Updater 签名密钥
#-------------------------------------------------------------------------------
setup_updater_keys() {
  log_step "Tauri Updater 签名密钥"

  if [[ -f "$PRIVATE_KEY" ]] && [[ -f "$PUBLIC_KEY" ]]; then
    log_info "密钥对已存在: $KEY_DIR/"
    local pubkey
    pubkey=$(cat "$PUBLIC_KEY")
    log_info "公钥: ${pubkey:0:16}..."

    # 检查 tauri.conf.json 中的 pubkey 是否匹配
    local configured_pubkey
    configured_pubkey=$(grep '"pubkey"' "$TAURI_CONF" | sed 's/.*"pubkey": "\([^"]*\)".*/\1/')
    if [[ -z "$configured_pubkey" ]]; then
      log_warn "tauri.conf.json 中 pubkey 为空！正在填入公钥..."
      if [[ "$DRY_RUN" == false ]]; then
        sed -i '' "s/\"pubkey\": \"\"/\"pubkey\": \"$pubkey\"/" "$TAURI_CONF"
        log_info "pubkey 已写入 tauri.conf.json"
      fi
    elif [[ "$configured_pubkey" != "$pubkey" ]]; then
      log_warn "tauri.conf.json 中的 pubkey 与本地密钥不匹配，正在更新..."
      if [[ "$DRY_RUN" == false ]]; then
        sed -i '' "s/\"pubkey\": \"[^\"]*\"/\"pubkey\": \"$pubkey\"/" "$TAURI_CONF"
        log_info "pubkey 已更新"
      fi
    else
      log_info "pubkey 与 tauri.conf.json 一致"
    fi
    return
  fi

  log_info "密钥对不存在，正在生成..."
  if [[ "$DRY_RUN" == true ]]; then
    log_info "[DRY-RUN] 跳过密钥生成"
    return
  fi

  mkdir -p "$KEY_DIR"
  pnpm tauri signer generate \
    --write-keys "$PRIVATE_KEY" \
    --ci \
    || die "密钥生成失败"

  local pubkey
  pubkey=$(cat "$PUBLIC_KEY" | tr -d '\n\r' | xargs)
  sed -i '' "s|\"pubkey\": \"[^\"]*\"|\"pubkey\": \"$pubkey\"|" "$TAURI_CONF"
  log_info "密钥对已生成并写入 tauri.conf.json"
  log_warn "请妥善保管私钥: $PRIVATE_KEY（切勿提交到 Git）"
}

#-------------------------------------------------------------------------------
# 步骤 3: 构建
#-------------------------------------------------------------------------------
build_app() {
  local version="$1"
  log_step "构建 nodepilot v$version"

  if [[ "$DRY_RUN" == true ]]; then
    log_info "[DRY-RUN] 跳过构建"
    return
  fi

  cd "$PROJECT_DIR"

  # 设置签名私钥环境变量
  if [[ -f "$PRIVATE_KEY" ]]; then
    export TAURI_SIGNING_PRIVATE_KEY="$(cat "$PRIVATE_KEY")"
    log_info "已加载签名私钥"
  else
    log_warn "未找到私钥，更新包将不被签名"
  fi

  # 执行 Tauri 构建
  # --target 可选: aarch64-apple-darwin (Apple Silicon) 或 universal-apple-darwin
  local tauri_args=()
  if [[ "$TARGET_TRIPLE" == "universal-apple-darwin" ]]; then
    tauri_args+=(--target universal-apple-darwin)
  fi

  log_info "运行: pnpm tauri build ${tauri_args[*]+"${tauri_args[*]}"}"
  pnpm tauri build ${tauri_args[@]+"${tauri_args[@]}"} || die "Tauri 构建失败"

  # 验证产物
  local dmg_path="$TAURI_DIR/target/release/bundle/dmg/nodepilot_${version}_${TARGET_LABEL}.dmg"
  if [[ -f "$dmg_path" ]]; then
    local dmg_size
    dmg_size=$(du -h "$dmg_path" | cut -f1)
    log_info "DMG 构建成功: $dmg_path ($dmg_size)"
  else
    log_warn "未找到 DMG: $dmg_path"
    log_info "可用产物列表:"
    ls -lh "$TAURI_DIR/target/release/bundle/dmg/" 2>/dev/null || true
  fi

  # 查找 updater 产物（.app.tar.gz 和 .app.tar.gz.sig）
  local tar_gz=""
  local found_tar
  found_tar=$(find "$TAURI_DIR/target/release/bundle/macos" -maxdepth 1 -name "nodepilot.app.tar.gz" -type f 2>/dev/null | head -1)
  if [[ -n "$found_tar" ]] && [[ -f "$found_tar" ]]; then
    tar_gz="$found_tar"
    log_info "Updater 包: $tar_gz"
  else
    log_warn "未找到 .app.tar.gz（updater 包），请确认 tauri.conf.json 中已配置 updater plugin"
  fi

  local sig_file=""
  local found_sig
  found_sig=$(find "$TAURI_DIR/target/release/bundle/macos" -maxdepth 1 -name "nodepilot.app.tar.gz.sig" -type f 2>/dev/null | head -1)
  if [[ -n "$found_sig" ]] && [[ -f "$found_sig" ]]; then
    sig_file="$found_sig"
    log_info "签名文件: $sig_file"
  else
    log_warn "未找到 .sig 签名文件"
  fi
}

#-------------------------------------------------------------------------------
# 步骤 4: 生成 latest.json 更新清单
#-------------------------------------------------------------------------------
generate_latest_json() {
  local version="$1"
  log_step "生成 latest.json 更新清单"

  local json_path="$TAURI_DIR/target/release/bundle/latest.json"
  local tar_gz
  tar_gz=$(find "$TAURI_DIR/target/release/bundle/macos" -maxdepth 1 -name "nodepilot.app.tar.gz" -type f 2>/dev/null | head -1)
  local sig_file
  sig_file=$(find "$TAURI_DIR/target/release/bundle/macos" -maxdepth 1 -name "nodepilot.app.tar.gz.sig" -type f 2>/dev/null | head -1)

  if [[ "$DRY_RUN" == true ]]; then
    log_info "[DRY-RUN] 跳过 latest.json 生成"
    return
  fi

  local signature=""
  if [[ -n "$sig_file" ]] && [[ -f "$sig_file" ]]; then
    signature=$(cat "$sig_file")
    log_info "已读取签名"
  else
    log_warn "缺少签名文件，latest.json 中 signature 为空"
  fi

  local pub_date
  pub_date=$(date -u +"%Y-%m-%dT%H:%M:%SZ")

  local download_url="https://github.com/$REPO/releases/download/v${version}/nodepilot_${TARGET_LABEL}.app.tar.gz"

  # 读取 release notes（纯 bash 实现 JSON 转义，避免依赖 python3）
  local notes="Release v${version}"
  if [[ -f "$RELEASE_NOTES_FILE" ]]; then
    local raw_notes
    raw_notes=$(cat "$RELEASE_NOTES_FILE" | sed "s/{version}/$version/g")
    # 转义 JSON 特殊字符：反斜杠、双引号、换行符转 \n
    notes=$(echo "$raw_notes" | sed 's/\\/\\\\/g; s/"/\\"/g' | awk '{printf "%s\\n", $0}' | sed 's/\\n$//')
    notes="\"$notes\""
  fi

  mkdir -p "$(dirname "$json_path")"
  cat > "$json_path" << EOF
{
  "version": "${version}",
  "notes": ${notes},
  "pub_date": "${pub_date}",
  "platforms": {
    "darwin-${TARGET_LABEL}": {
      "signature": "${signature}",
      "url": "${download_url}"
    }
  }
}
EOF

  log_info "latest.json 已生成: $json_path"
  log_info "更新端点: https://github.com/$REPO/releases/latest/download/latest.json"
}

#-------------------------------------------------------------------------------
# 步骤 5: 验证产物
#-------------------------------------------------------------------------------
verify_artifacts() {
  local version="$1"
  log_step "验证产物"

  local dmg="$TAURI_DIR/target/release/bundle/dmg/nodepilot_${version}_${TARGET_LABEL}.dmg"

  if [[ -f "$dmg" ]]; then
    # 挂载 DMG 验证版本号和文件完整性
    log_info "挂载 DMG 验证..."

    local mount_point="/Volumes/nodepilot_verify_$$"
    hdiutil attach "$dmg" -mountpoint "$mount_point" -nobrowse -quiet 2>/dev/null || {
      log_warn "无法挂载 DMG，跳过内容验证"
      return
    }

    # 检查 .app bundle 是否存在
    if [[ -d "$mount_point/nodepilot.app" ]]; then
      log_info "DMG 内容: nodepilot.app 存在"

      # 读取 Info.plist 中的版本号
      local plist="$mount_point/nodepilot.app/Contents/Info.plist"
      if [[ -f "$plist" ]]; then
        local dmg_version
        dmg_version=$(/usr/libexec/PlistBuddy -c "Print CFBundleShortVersionString" "$plist" 2>/dev/null || echo "无法读取")
        log_info "Info.plist 版本号: $dmg_version"
        if [[ "$dmg_version" == "$version" ]]; then
          log_info "版本号验证通过 ✓"
        else
          log_error "版本号不匹配！期望: $version, 实际: $dmg_version"
        fi
      fi
    fi

    hdiutil detach "$mount_point" -quiet
  else
    log_warn "DMG 文件不存在，跳过验证"
  fi
}

#-------------------------------------------------------------------------------
# 步骤 6: 创建 GitHub Draft Release + 上传
#-------------------------------------------------------------------------------
create_github_release() {
  local version="$1"
  log_step "创建 GitHub Draft Release"

  local tag="v${version}"
  local dmg="$TAURI_DIR/target/release/bundle/dmg/nodepilot_${version}_${TARGET_LABEL}.dmg"
  local tar_gz
  tar_gz=$(find "$TAURI_DIR/target/release/bundle/macos" -maxdepth 1 -name "nodepilot.app.tar.gz" -type f 2>/dev/null | head -1)
  local sig_file
  sig_file=$(find "$TAURI_DIR/target/release/bundle/macos" -maxdepth 1 -name "nodepilot.app.tar.gz.sig" -type f 2>/dev/null | head -1)
  local json_path="$TAURI_DIR/target/release/bundle/latest.json"

  if [[ "$DRY_RUN" == true ]]; then
    log_info "[DRY-RUN] 将创建 Release tag=$tag"
    log_info "[DRY-RUN] 上传文件:"
    [[ -f "$dmg" ]]      && log_info "  - $dmg"
    [[ -f "$tar_gz" ]]   && log_info "  - $tar_gz"
    [[ -f "$sig_file" ]] && log_info "  - $sig_file"
    [[ -f "$json_path" ]] && log_info "  - $json_path"
    return
  fi

  # 检查 tag 是否已存在
  if git rev-parse "$tag" >/dev/null 2>&1; then
    log_warn "tag $tag 已存在，跳过创建（将使用已有的 release 上传）"
  fi

  # 准备 release notes
  local notes_arg="Release v${version}"
  if [[ -f "$RELEASE_NOTES_FILE" ]]; then
    notes_arg="$(cat "$RELEASE_NOTES_FILE")"
  fi

  # 创建 Draft Release
  log_info "创建 Draft Release: $tag"
  gh release create "$tag" \
    --repo "$REPO" \
    --draft \
    --title "v${version}" \
    --notes "$notes_arg" \
    || die "创建 Release 失败"

  # 上传 DMG
  if [[ -f "$dmg" ]]; then
    log_info "上传 DMG..."
    gh release upload "$tag" "$dmg" --repo "$REPO" --clobber
  fi

  # 上传 updater .app.tar.gz
  if [[ -n "$tar_gz" ]] && [[ -f "$tar_gz" ]]; then
    log_info "上传 updater 包..."
    gh release upload "$tag" "$tar_gz" --repo "$REPO" --clobber
  fi

  # 上传 .sig 签名文件
  if [[ -n "$sig_file" ]] && [[ -f "$sig_file" ]]; then
    log_info "上传签名文件..."
    gh release upload "$tag" "$sig_file" --repo "$REPO" --clobber
  fi

  # 上传 latest.json（覆盖式上传，确保 latest/download/latest.json 始终指向最新）
  if [[ -f "$json_path" ]]; then
    log_info "上传 latest.json 更新清单..."
    gh release upload "$tag" "$json_path" --repo "$REPO" --clobber
  fi

  log_info "GitHub Draft Release 创建完成:"
  log_info "  https://github.com/$REPO/releases/tag/$tag"
}

#-------------------------------------------------------------------------------
# 主流程
#-------------------------------------------------------------------------------
main() {
  local specified_version=""

  # 解析参数
  while [[ $# -gt 0 ]]; do
    case "$1" in
      --dry-run)
        DRY_RUN=true
        shift
        ;;
      --target)
        shift
        TARGET_TRIPLE="$1"
        TARGET_LABEL="${1#*-}"  # "aarch64-apple-darwin" → "aarch64-apple-darwin"... hmm
        # 简化标签
        case "$TARGET_TRIPLE" in
          aarch64-*)   TARGET_LABEL="aarch64" ;;
          x86_64-*)    TARGET_LABEL="x64" ;;
          universal-*) TARGET_LABEL="universal" ;;
        esac
        shift
        ;;
      --help|-h)
        echo "用法: $0 [OPTIONS] [VERSION]"
        echo ""
        echo "OPTIONS:"
        echo "  --dry-run        仅检查，不构建和发布"
        echo "  --target TRIPLE  Rust 目标 (默认: aarch64-apple-darwin)"
        echo "                   可选: universal-apple-darwin, x86_64-apple-darwin"
        echo "  --help           显示帮助"
        echo ""
        echo "VERSION:"
        echo "  不传则自动递增补丁号 (0.2.2 → 0.2.3)"
        echo "  传入 x.y.z 则使用指定版本号"
        exit 0
        ;;
      *)
        specified_version="$1"
        shift
        ;;
    esac
  done

  echo ""
  echo "╔══════════════════════════════════════════╗"
  echo "║   nodepilot macOS DMG Release Script    ║"
  echo "╚══════════════════════════════════════════╝"
  [[ "$DRY_RUN" == true ]] && echo -e "  ${YELLOW}[DRY-RUN MODE]${NC}"

  check_prerequisites

  # 确保在项目根目录运行
  cd "$PROJECT_DIR"

  local current_ver
  current_ver=$(get_current_version)

  # 步骤 1: 确定新版本号
  local new_ver
  if [[ -n "$specified_version" ]]; then
    new_ver="$specified_version"
    validate_version "$new_ver"
  else
    new_ver=$(bump_patch "$current_ver")
    validate_version "$new_ver"
  fi
  log_info "版本号: $current_ver → $new_ver"
  log_info "构建目标: $TARGET_TRIPLE ($TARGET_LABEL)"

  # 步骤 2: 更新版本文件
  update_version_files "$current_ver" "$new_ver"

  # 步骤 3: 设置签名密钥
  setup_updater_keys

  # 步骤 4: 构建应用
  build_app "$new_ver"

  # 步骤 5: 生成 latest.json
  generate_latest_json "$new_ver"

  # 步骤 6: 验证 DMG
  verify_artifacts "$new_ver"

  # 步骤 7: 创建 GitHub Release
  create_github_release "$new_ver"

  # 完成
  echo ""
  log_info "════════════════════════════════════════"
  log_info "发布流程完成！"
  log_info "版本: v${new_ver}"
  log_info "Draft Release: https://github.com/$REPO/releases/tag/v${new_ver}"
  log_info ""
  if [[ "$DRY_RUN" == false ]]; then
    log_info "下一步操作:"
    log_info "  1. 检查 Draft Release 内容"
    log_info "  2. 测试 DMG 安装"
    log_info "  3. 确认无误后 Publish Release"
    log_info "  4. 提交版本号变更: git commit && git push"
  fi
  echo ""
}

main "$@"
