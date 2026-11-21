# ========================================================================
# Script Name : format.ps1
# Description : Windows用のコード整形スクリプト
# Usage       : ./format.ps1
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

    # rustfmt を実行
    Write-Host 'rustfmt によるコード整形を実行しています...'
    Write-Host ''

    Write-Host 'コード整形を実行中...'
    cargo fmt

    if ($LASTEXITCODE -ne 0) {
        throw "エラー: cargo fmt が終了コード $LASTEXITCODE で失敗しました"
    }

    Write-Host ''
    Write-Host 'コード整形が完了しました'
}
