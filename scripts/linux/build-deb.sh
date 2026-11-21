#!/bin/bash
# ========================================================================
# Script Name : build-deb.sh
# Description : Linux 向け .deb パッケージ作成スクリプト (cargo-deb)
# Usage       : ./build-deb.sh [--skip-build|-s]
# Requires    : cargo-deb, dpkg-deb (推奨: dpkg-dev)
#
# 前提:
#   - cargo install cargo-deb --locked
#   - ビルド用の開発パッケージ (DEVELOPMENT.md の「Linux の追加パッケージ」参照)
#   - packaging/linux/clip-refiner.desktop が存在すること
#
# 出力:
#   target/debian/clip-refiner_{version}-1_{arch}.deb
# ========================================================================

# シェルオプションを設定(エラー時に即終了)
set -euo pipefail

# プロジェクトルートへ移動
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
# shellcheck source=../common/cd-project-root.sh
source "$SCRIPT_DIR/../common/cd-project-root.sh"

# エラー時のメッセージを設定
trap 'echo "deb パッケージの作成に失敗しました" >&2' ERR

# cargo-deb がインストールされているか確認する
assert_cargo_deb_installed() {
    if ! cargo deb --version &>/dev/null; then
        cat >&2 <<'EOF'
エラー: cargo-deb が見つかりません。

以下でインストールしてください:
  cargo install cargo-deb --locked
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

DESKTOP_FILE="$PROJECT_ROOT/packaging/linux/clip-refiner.desktop"
if [[ ! -f "$DESKTOP_FILE" ]]; then
    echo "エラー: packaging/linux/clip-refiner.desktop が見つかりません" >&2
    exit 1
fi

assert_cargo_deb_installed

echo "deb パッケージの作成を開始します..."
echo

deb_args=(-p clip-refiner --release)
if [[ "$SKIP_BUILD" == true ]]; then
    deb_args+=(--no-build)
fi

cargo deb "${deb_args[@]}"

echo
echo "deb パッケージの作成が完了しました:"
echo "  $PROJECT_ROOT/target/debian"
