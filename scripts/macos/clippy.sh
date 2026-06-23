#!/bin/bash
# ========================================================================
# Script Name : clippy.sh
# Description : macOS用の Clippy 実行スクリプト
# Usage       : ./clippy.sh
# ========================================================================

# シェルオプションを設定(エラー時に即終了)
set -euo pipefail

# プロジェクトルートへ移動
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
# shellcheck source=../common/cd-project-root.sh
source "$SCRIPT_DIR/../common/cd-project-root.sh"

# エラー時のメッセージを設定
trap 'echo "Clippy に失敗しました" >&2' ERR

# Clippy を実行
echo "Clippy による静的解析を実行しています..."
echo

echo "Clippy を実行中..."
cargo clippy --all-targets -- -D warnings
echo

# 完了メッセージを表示
echo "Clippy が完了しました"
