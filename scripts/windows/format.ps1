# ========================================================================
# Script Name : format.ps1
# Description : Windows 用のコード整形 (rustfmt / Prettier)
# Usage       : ./format.ps1
# Requires    : PowerShell 7.0+, Node.js (npm)
# ========================================================================

#requires -Version 7.0

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

. (Join-Path $PSScriptRoot '..\common\wait-if-double-clicked.ps1')

Invoke-ScriptMain {
    . (Join-Path $PSScriptRoot '..\common\cd-project-root.ps1')
    . (Join-Path $PSScriptRoot '..\common\cargo.ps1')
    . (Join-Path $PSScriptRoot '..\common\npm.ps1')

    Write-Host 'rustfmt によるコード整形を実行しています...'
    Write-Host ''
    Invoke-CargoFmt
    Write-Host ''

    Write-Host 'Prettier による JavaScript 整形を実行しています...'
    Write-Host ''
    Invoke-JavaScriptFormat
    Write-Host ''

    Write-Host 'コード整形が完了しました'
}
