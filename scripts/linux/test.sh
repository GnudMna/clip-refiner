#!/bin/bash
# ========================================================================
# Script Name : test.sh
# Description : Linux用のテスト実行スクリプト
# Usage       : ./test.sh
# ========================================================================

# シェルオプションを設定(エラー時に即終了)
set -euo pipefail

# プロジェクトルートへ移動
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
# shellcheck source=../common/cd-project-root.sh
source "$SCRIPT_DIR/../common/cd-project-root.sh"

# エラー時のメッセージを設定
trap 'echo "テストに失敗しました" >&2' ERR

# テストを実行
echo "テストを実行しています..."
echo

echo "テストを実行中..."
cargo test --all-targets
echo

# 完了メッセージを表示
echo "すべてのテストが完了しました"
