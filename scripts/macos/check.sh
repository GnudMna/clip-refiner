#!/bin/bash
# ========================================================================
# Script Name : check.sh
# Description : macOS用の品質チェックスクリプト(fmt / clippy / test)
# Usage       : ./check.sh
# ========================================================================

# シェルオプションを設定(エラー時に即終了)
set -euo pipefail

# プロジェクトルートへ移動
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
# shellcheck source=../common/cd-project-root.sh
source "$SCRIPT_DIR/../common/cd-project-root.sh"

# エラー時のメッセージを設定
trap 'echo "チェックに失敗しました" >&2' ERR

# 品質チェックを開始
echo "品質チェックを実行しています..."
echo

# コード整形を検証
echo "コード整形を検証中..."
cargo fmt --check
echo

# Clippy を実行
echo "Clippy を実行中..."
cargo clippy --all-targets -- -D warnings
echo

# テストを実行
echo "テストを実行中..."
cargo test --all-targets
cargo test --doc
echo

# 完了メッセージを表示
echo "すべてのチェックが完了しました"
