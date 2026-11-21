# ========================================================================
# Script Name : clippy.ps1
# Description : Windows 用の Clippy 実行
# Usage       : ./clippy.ps1
# Requires    : PowerShell 7.0+, Rust / Cargo
# ========================================================================

#requires -Version 7.0

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

. (Join-Path $PSScriptRoot '..\common\wait-if-double-clicked.ps1')

Invoke-ScriptMain {
    . (Join-Path $PSScriptRoot '..\common\cd-project-root.ps1')
    . (Join-Path $PSScriptRoot '..\common\cargo.ps1')

    Write-Host 'Clippy による静的解析を実行しています...'
    Write-Host ''
    Invoke-CargoClippy
    Write-Host ''
    Write-Host 'Clippy が完了しました'
}
