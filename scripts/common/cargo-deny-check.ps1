# ========================================================================
# Script Name : cargo-deny-check.ps1
# Description : cargo-deny によるサプライチェーン監査
# Usage       : . (Join-Path $PSScriptRoot 'cargo-deny-check.ps1')
# Requires    : PowerShell 7.0+
# ========================================================================

#requires -Version 7.0

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

if (-not (Get-Command cargo-deny -ErrorAction SilentlyContinue)) {
    throw @(
        'エラー: cargo-deny が見つかりません',
        'インストール: cargo install cargo-deny --locked'
    ) -join [Environment]::NewLine
}

cargo deny check
if ($LASTEXITCODE -ne 0) {
    throw "エラー: cargo deny check が終了コード $LASTEXITCODE で失敗しました"
}
