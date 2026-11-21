# ========================================================================
# Script Name : coverage.ps1
# Description : Windows 用の Rust カバレッジ計測 (cargo-llvm-cov)
# Usage       : ./coverage.ps1 [-Html] [-Open]
# Requires    : PowerShell 7.0+, Rust / Cargo, cargo-llvm-cov, llvm-tools-preview
# ========================================================================

#requires -Version 7.0

param(
    [switch]$Html,
    [switch]$Open
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

. (Join-Path $PSScriptRoot '..\common\wait-if-double-clicked.ps1')

Invoke-ScriptMain {
    . (Join-Path $PSScriptRoot '..\common\cd-project-root.ps1')
    . (Join-Path $PSScriptRoot '..\common\cargo.ps1')
    . (Join-Path $PSScriptRoot '..\common\npm.ps1')

    Write-Host 'カバレッジ計測付きテストを実行しています...'
    Write-Host ''

    Assert-NodeInstalled
    $env:CLIP_REFINER_REQUIRE_NODE = '1'
    Write-Host ''

    if ($Open) {
        Invoke-CargoLlvmCov -Open
    }
    elseif ($Html) {
        Invoke-CargoLlvmCov -Html
    }
    else {
        Invoke-CargoLlvmCov
    }
    Write-Host ''

    Write-Host 'doctests を実行中 (カバレッジ対象外)...'
    Invoke-CargoDocTest
    Write-Host ''

    Write-Host 'カバレッジ計測が完了しました'
    if (-not ($Html -or $Open)) {
        Write-Host 'HTML レポートが必要な場合: ./scripts/windows/coverage.ps1 -Html'
        Write-Host 'ブラウザで開く場合: ./scripts/windows/coverage.ps1 -Open'
    }
}
