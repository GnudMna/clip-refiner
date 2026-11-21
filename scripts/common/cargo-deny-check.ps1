# ========================================================================
# Script Name : cargo-deny-check.ps1
# Description : cargo-deny によるサプライチェーン監査 (ドットソース用)
# Usage       : . (Join-Path $PSScriptRoot 'cargo-deny-check.ps1')
# Requires    : PowerShell 7.0+, cargo-deny
# ========================================================================

#requires -Version 7.0

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

. (Join-Path $PSScriptRoot 'assert.ps1')

if (-not (Get-Command cargo-deny -ErrorAction SilentlyContinue)) {
    throw @(
        'エラー: cargo-deny が見つかりません',
        'インストール: cargo install cargo-deny --locked'
    ) -join [Environment]::NewLine
}

cargo deny check
Assert-LastExitCode -CommandLabel 'cargo deny check'
