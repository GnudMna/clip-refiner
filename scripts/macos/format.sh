#!/bin/bash
# ========================================================================
# Script Name : format.sh
# Description : macOS用のコード整形スクリプト
# Usage       : ./format.sh
# ========================================================================

# シェルオプションを設定(エラー時に即終了)
set -euo pipefail

# プロジェクトルートへ移動
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
# shellcheck source=../common/cd-project-root.sh
source "$SCRIPT_DIR/../common/cd-project-root.sh"

# エラー時のメッセージを設定
trap 'echo "コード整形に失敗しました" >&2' ERR

# rustfmt を実行
echo "rustfmt によるコード整形を実行しています..."
echo

echo "コード整形を実行中..."
cargo fmt
echo

# 完了メッセージを表示
echo "コード整形が完了しました"
