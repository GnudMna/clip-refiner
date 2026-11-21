# ========================================================================
# Script Name : clippy.ps1
# Description : Windows用の Clippy 実行スクリプト
# Usage       : ./clippy.ps1
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

    # Clippy を実行
    Write-Host 'Clippy による静的解析を実行しています...'
    Write-Host ''

    Write-Host 'Clippy を実行中...'
    cargo clippy --all-targets -- -D warnings

    if ($LASTEXITCODE -ne 0) {
        throw "エラー: cargo clippy が終了コード $LASTEXITCODE で失敗しました"
    }

    Write-Host ''
    Write-Host 'Clippy が完了しました'
}
