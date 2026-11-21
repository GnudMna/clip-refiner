# ========================================================================
# Script Name : assert.ps1
# Description : 外部コマンドの終了コード検証 (ドットソース用)
# Usage       : . (Join-Path $PSScriptRoot 'assert.ps1')
# Requires    : PowerShell 7.0+
# ========================================================================

#requires -Version 7.0

Set-StrictMode -Version Latest

# 直前の外部コマンドが成功したことを検証する
function Assert-LastExitCode {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)]
        [string]$CommandLabel
    )

    if ((Test-Path variable:LASTEXITCODE) -and $LASTEXITCODE -ne 0) {
        throw "エラー: $CommandLabel が終了コード $LASTEXITCODE で失敗しました"
    }
}
