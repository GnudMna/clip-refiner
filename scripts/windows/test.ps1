# ========================================================================
# Script Name : test.ps1
# Description : Windows 用のテスト実行
# Usage       : ./test.ps1
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

    Write-Host 'テストを実行しています...'
    Write-Host ''

    Assert-NodeInstalled
    $env:CLIP_REFINER_REQUIRE_NODE = '1'
    Write-Host ''

    Invoke-CargoTest -IncludeDocTests
    Write-Host ''

    Write-Host 'すべてのテストが完了しました'
}
