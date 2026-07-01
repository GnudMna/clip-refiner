#!/bin/bash
# ========================================================================
# Script Name : build-dmg.sh
# Description : macOS 向け .app / .dmg インストーラー作成スクリプト (cargo-bundle)
# Usage       : ./build-dmg.sh [--skip-build|-s]
# Requires    : cargo-bundle, hdiutil (macOS 標準)
#
# 前提:
#   - cargo install cargo-bundle --locked
#   - Xcode Command Line Tools
#   - Cargo.toml の [package.metadata.bundle.bin.ClipRefiner] が設定済みであること
#
# 出力:
#   target/release/bundle/osx/ClipRefiner.app
#   target/bundle/clip-refiner-{version}-{arch}.dmg
# ========================================================================

# シェルオプションを設定(エラー時に即終了)
set -euo pipefail

# プロジェクトルートへ移動
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
# shellcheck source=../common/cd-project-root.sh
source "$SCRIPT_DIR/../common/cd-project-root.sh"

# エラー時のメッセージを設定
trap 'echo "DMG インストーラーの作成に失敗しました" >&2' ERR

# cargo-bundle がインストールされているか確認する
assert_cargo_bundle_installed() {
    if ! cargo bundle --version &>/dev/null; then
        cat >&2 <<'EOF'
エラー: cargo-bundle が見つかりません。

以下でインストールしてください:
  cargo install cargo-bundle --locked
EOF
        exit 1
    fi
}

# 引数を解析(--skip-build / -s)
SKIP_BUILD=false
for arg in "$@"; do
    case $arg in
        --skip-build|-s)
            SKIP_BUILD=true
            ;;
    esac
done

assert_cargo_bundle_installed

echo "macOS インストーラーの作成を開始します..."
echo

bundle_args=(-p clip-refiner --release --bin ClipRefiner --format osx)
if [[ "$SKIP_BUILD" == true ]]; then
    bundle_args+=(--no-build)
fi

cargo bundle "${bundle_args[@]}"

APP_PATH="$PROJECT_ROOT/target/release/bundle/osx/ClipRefiner.app"
if [[ ! -d "$APP_PATH" ]]; then
    echo "エラー: アプリバンドルが見つかりません: $APP_PATH" >&2
    exit 1
fi

VERSION="$(grep -E '^version = ' Cargo.toml | head -1 | sed -E 's/^version = "(.*)"/\1/')"
ARCH="$(uname -m)"
OUTPUT_DIR="$PROJECT_ROOT/target/bundle"
STAGING_DIR="$OUTPUT_DIR/dmg-staging"
DMG_NAME="clip-refiner-${VERSION}-${ARCH}.dmg"
DMG_PATH="$OUTPUT_DIR/$DMG_NAME"

rm -rf "$STAGING_DIR"
mkdir -p "$STAGING_DIR"
cp -R "$APP_PATH" "$STAGING_DIR/"
ln -s /Applications "$STAGING_DIR/Applications"

mkdir -p "$OUTPUT_DIR"
rm -f "$DMG_PATH"

hdiutil create \
    -volname "ClipRefiner" \
    -srcfolder "$STAGING_DIR" \
    -ov \
    -format UDZO \
    "$DMG_PATH"

rm -rf "$STAGING_DIR"

echo
echo "macOS インストーラーの作成が完了しました:"
echo "  $APP_PATH"
echo "  $DMG_PATH"
