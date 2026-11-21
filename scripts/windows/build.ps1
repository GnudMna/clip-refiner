# ========================================================================
# Script Name : build.ps1
# Description : Windows 用のビルド実行
# Usage       : ./build.ps1 [--release|-r]
# Requires    : PowerShell 7.0+, Rust / Cargo
# ========================================================================

#requires -Version 7.0

param(
    [switch]$Release
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

. (Join-Path $PSScriptRoot '..\common\wait-if-double-clicked.ps1')

Invoke-ScriptMain {
    . (Join-Path $PSScriptRoot '..\common\cd-project-root.ps1')
    . (Join-Path $PSScriptRoot '..\common\assert.ps1')

    if ($args -contains '--release' -or $args -contains '-r') {
        $Release = $true
    }

    Write-Host 'Rust プロジェクトのビルドを実行しています...'
    Write-Host ''

    if ($Release) {
        Write-Host 'リリースビルドを実行中...'
        cargo build --release
    } else {
        Write-Host 'デバッグビルドを実行中...'
        cargo build
    }
    Assert-LastExitCode -CommandLabel 'cargo build'

    Write-Host ''
    Write-Host 'ビルドが完了しました'
}
