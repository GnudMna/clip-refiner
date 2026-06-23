# ========================================================================
# Script Name : check.ps1
# Description : Windows用の品質チェックスクリプト(fmt / clippy / test)
# Usage       : ./check.ps1
# Requires    : PowerShell 7.0+
# ========================================================================

#requires -Version 7.0

# シェルオプションを設定(エラー時に即終了)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

# ダブルクリック実行時の Enter 待ちヘルパーを読み込み
. (Join-Path $PSScriptRoot '..\common\wait-if-double-clicked.ps1')

Invoke-ScriptMain {
    # プロジェクトルートへ移動
    . (Join-Path $PSScriptRoot '..\common\cd-project-root.ps1')

    # 品質チェックを開始
    Write-Host '品質チェックを実行しています...'
    Write-Host ''

    # コード整形を検証
    Write-Host 'コード整形を検証中...'
    cargo fmt --check
    if ($LASTEXITCODE -ne 0) {
        throw "エラー: cargo fmt --check が終了コード $LASTEXITCODE で失敗しました"
    }
    Write-Host ''

    # Clippy を実行
    Write-Host 'Clippy を実行中...'
    cargo clippy --all-targets -- -D warnings
    if ($LASTEXITCODE -ne 0) {
        throw "エラー: cargo clippy が終了コード $LASTEXITCODE で失敗しました"
    }
    Write-Host ''

    # テストを実行
    Write-Host 'テストを実行中...'
    cargo test --all-targets
    if ($LASTEXITCODE -ne 0) {
        throw "エラー: cargo test が終了コード $LASTEXITCODE で失敗しました"
    }

    cargo test --doc
    if ($LASTEXITCODE -ne 0) {
        throw "エラー: cargo test --doc が終了コード $LASTEXITCODE で失敗しました"
    }
    Write-Host ''

    # 完了メッセージを表示
    Write-Host 'すべてのチェックが完了しました'
}
