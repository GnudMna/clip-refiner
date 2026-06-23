#!/bin/bash
# ========================================================================
# Script Name : cd-project-root.sh
# Description : プロジェクトのルートディレクトリに移動するシェルスクリプト
# Usage       : cd-project-root.sh
# ========================================================================

# スクリプトのディレクトリを取得
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# プロジェクトのルートディレクトリを計算
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

# プロジェクトルートを環境変数にエクスポート
export PROJECT_ROOT

# プロジェクトルートに移動
cd "$PROJECT_ROOT"

# Cargo.toml の存在を確認
if [[ ! -f Cargo.toml ]]; then
    echo "エラー: プロジェクトルートが見つかりません (Cargo.toml がありません): ${PROJECT_ROOT}" >&2
    exit 1
fi
