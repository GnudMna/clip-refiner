#!/bin/bash
# ========================================================================
# Script Name : generate-third-party-notices.sh
# Description : THIRD_PARTY_NOTICES を cargo-about で生成する
# Usage       : generate-third-party-notices.sh
# ========================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
# shellcheck source=cd-project-root.sh
source "$SCRIPT_DIR/cd-project-root.sh"

OUTPUT_FILE="$PROJECT_ROOT/THIRD_PARTY_NOTICES"
TEMPLATE="$PROJECT_ROOT/scripts/templates/third-party-notices.hbs"
MANIFEST="$PROJECT_ROOT/crates/clip-refiner/Cargo.toml"

if ! command -v cargo-about >/dev/null 2>&1; then
    cat >&2 <<'EOF'
エラー: cargo-about が見つかりません。

以下でインストールしてください:
  cargo install cargo-about --locked --features cli
EOF
    exit 1
fi

if [[ ! -f "$TEMPLATE" ]]; then
    echo "エラー: テンプレートが見つかりません: $TEMPLATE" >&2
    exit 1
fi

echo "THIRD_PARTY_NOTICES を生成しています..."

cargo about generate "$TEMPLATE" \
    --config "$PROJECT_ROOT/about.toml" \
    --manifest-path "$MANIFEST" \
    --output-file "$OUTPUT_FILE" \
    --all-features \
    --locked

echo "生成しました: $OUTPUT_FILE"
