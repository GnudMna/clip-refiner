# ========================================================================
# Script Name : check.ps1
# Description : Windows 用の品質チェック (fmt / clippy / deny / eslint / test)
# Usage       : ./check.ps1
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

    Write-Host '品質チェックを実行しています...'
    Write-Host ''

    Write-Host 'コード整形を検証中...'
    Invoke-CargoFmtCheck
    Write-Host ''

    Write-Host 'Clippy を実行中...'
    Invoke-CargoClippy
    Write-Host ''

    Write-Host 'サプライチェーン監査を実行中...'
    . (Join-Path $PSScriptRoot '..\common\cargo-deny-check.ps1')
    Write-Host ''

    Write-Host 'Node.js を検証中...'
    Assert-NodeInstalled
    $env:CLIP_REFINER_REQUIRE_NODE = '1'
    Write-Host ''

    Write-Host 'JavaScript リンターを実行中...'
    Invoke-JavaScriptLintCheck
    Write-Host ''

    Write-Host 'テストを実行中...'
    Invoke-CargoTest -IncludeDocTests
    Write-Host ''

    Write-Host 'すべてのチェックが完了しました'
}
