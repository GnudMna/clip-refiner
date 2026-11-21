#!/bin/bash
# ========================================================================
# Script Name : cargo-deny-check.sh
# Description : cargo-deny によるサプライチェーン監査
# Usage       : cargo-deny-check.sh
# ========================================================================

set -euo pipefail

if ! command -v cargo-deny >/dev/null 2>&1; then
    echo "エラー: cargo-deny が見つかりません" >&2
    echo "インストール: cargo install cargo-deny --locked" >&2
    exit 1
fi

cargo deny check
