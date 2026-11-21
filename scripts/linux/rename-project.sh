#!/bin/bash
# ========================================================================
# Script Name : rename-project.sh
# Description : テンプレートのプロジェクト名を一括変更する
# Usage       : ./rename-project.sh <new-name> [copyright-holder]
#               new-name         : kebab-case(例: my-project)
#               copyright-holder : 省略可(例: "Your Name")
# ========================================================================

# シェルオプションを設定(エラー時に即終了)
set -euo pipefail

# 使い方を表示して終了
usage() {
    echo "Usage: $0 <new-name> [copyright-holder]" >&2
    echo "  new-name         : kebab-case(例: my-project)" >&2
    echo "  copyright-holder : 省略可 (LICENSE / build.rs を更新(例: \"Your Name\"))" >&2
    exit 1
}

# 引数の数を検証
[[ $# -lt 1 || $# -gt 2 ]] && usage

NEW_NAME="$1"
COPYRIGHT_HOLDER="${2:-}"

# プロジェクトルートへ移動
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
# shellcheck source=../common/cd-project-root.sh
source "$SCRIPT_DIR/../common/cd-project-root.sh"

# プロジェクト名の形式を検証(kebab-case)
if [[ ! "$NEW_NAME" =~ ^[a-z][a-z0-9]*(-[a-z0-9]+)*$ ]]; then
    echo "エラー: プロジェクト名は kebab-case で指定してください (例: my-project)" >&2
    exit 1
fi

# 現在のプロジェクト名と Rust 識別子を取得
CURRENT_NAME="$(grep -E '^name = ' Cargo.toml | head -1 | sed -E 's/^name = "(.*)"/\1/')"
NEW_IDENT="${NEW_NAME//-/_}"
OLD_IDENT="${CURRENT_NAME//-/_}"

# 変更が不要な場合は終了
if [[ "$CURRENT_NAME" == "$NEW_NAME" ]]; then
    echo "プロジェクト名は既に \"${NEW_NAME}\" です"
    exit 0
fi

echo "プロジェクト名を \"${CURRENT_NAME}\" から \"${NEW_NAME}\" に変更しています..."
echo

# Cargo.toml の name を更新
sed -i "s/^name = \"${CURRENT_NAME}\"/name = \"${NEW_NAME}\"/" Cargo.toml

# MSI 用 GUID を再生成(別アプリとの衝突を防ぐ)
if grep -qE '^upgrade-guid = ' Cargo.toml && grep -qE '^path-guid = ' Cargo.toml; then
    OLD_UPGRADE_GUID="$(grep -E '^upgrade-guid = ' Cargo.toml | sed -E 's/^upgrade-guid = "(.*)"/\1/')"
    OLD_PATH_GUID="$(grep -E '^path-guid = ' Cargo.toml | sed -E 's/^path-guid = "(.*)"/\1/')"
    NEW_UPGRADE_GUID="$(uuidgen | tr '[:lower:]' '[:upper:]')"
    NEW_PATH_GUID="$(uuidgen | tr '[:lower:]' '[:upper:]')"
    sed -i "s/^upgrade-guid = \".*\"/upgrade-guid = \"${NEW_UPGRADE_GUID}\"/" Cargo.toml
    sed -i "s/^path-guid = \".*\"/path-guid = \"${NEW_PATH_GUID}\"/" Cargo.toml
    if [[ -f wix/main.wxs ]]; then
        sed -i "s/${OLD_UPGRADE_GUID}/${NEW_UPGRADE_GUID}/g" wix/main.wxs
        sed -i "s/${OLD_PATH_GUID}/${NEW_PATH_GUID}/g" wix/main.wxs
    fi
fi

# use 文のクレート名を更新
for file in src/main.rs tests/integration_test.rs; do
    sed -i "s/use ${OLD_IDENT}::/use ${NEW_IDENT}::/g" "$file"
done

# lib.rs の doc コメントを更新
sed -i "s/\`${CURRENT_NAME}\`/\`${NEW_NAME}\`/g" src/lib.rs

# wix/main.wxs のプロジェクト名を更新
if [[ -f wix/main.wxs ]]; then
    sed -i "s/${CURRENT_NAME}/${NEW_NAME}/g" wix/main.wxs
fi

# 著作権表記を更新(指定時のみ)
if [[ -n "$COPYRIGHT_HOLDER" ]]; then
    COPYRIGHT_LINE="Copyright (c) 2026 ${COPYRIGHT_HOLDER}"
    sed -i "s/^Copyright (c) .*/${COPYRIGHT_LINE}/" LICENSE
    sed -i "s/const LEGAL_COPYRIGHT: &str = \".*\";/const LEGAL_COPYRIGHT: \&str = \"${COPYRIGHT_LINE}\";/" build.rs
fi

# Cargo.lock を更新
echo "Cargo.lock を更新しています..."
cargo check --quiet
echo

# 手動更新が必要な項目を案内
echo "変更が完了しました。必要に応じて以下を手動で更新してください:"
echo "  - Cargo.toml の description, authors, repository"
echo "  - README.md のタイトル"
echo "  - assets/icon.ico"
if [[ -z "$COPYRIGHT_HOLDER" ]]; then
    echo "  - LICENSE / build.rs の著作権表記(または copyright-holder を指定して再実行)"
fi
