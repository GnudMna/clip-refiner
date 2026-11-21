# ========================================================================
# Script Name : test.ps1
# Description : Windows用のテスト実行スクリプト
# Usage       : ./test.ps1
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

    # テストを実行
    Write-Host 'テストを実行しています...'
    Write-Host ''

    Write-Host 'テストを実行中...'
    cargo test --all-targets

    if ($LASTEXITCODE -ne 0) {
        throw "エラー: cargo test が終了コード $LASTEXITCODE で失敗しました"
    }

    Write-Host ''
    Write-Host 'すべてのテストが完了しました'
}
