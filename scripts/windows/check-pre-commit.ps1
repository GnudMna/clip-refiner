# ========================================================================
# Script Name : check-pre-commit.ps1
# Description : pre-commit フック用の軽量品質チェック (cargo fmt --check)
# Usage       : Git フック経由、または手動で ./check-pre-commit.ps1
# Requires    : PowerShell 7.0+
# ========================================================================

#requires -Version 7.0

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

if ($env:CLIP_REFINER_SKIP_HOOK -eq '1') {
    Write-Host 'note: pre-commit をスキップ (CLIP_REFINER_SKIP_HOOK=1)'
    exit 0
}

. (Join-Path $PSScriptRoot '..\common\cd-project-root.ps1')
. (Join-Path $PSScriptRoot '..\common\cargo.ps1')

Write-Host 'pre-commit: コード整形を検証中...'
try {
    Invoke-CargoFmtCheck
} catch {
    Write-Host ''
    Write-Host $_.Exception.Message
    Write-Host '  修正: cargo fmt または ./scripts/windows/format.ps1'
    Write-Host '  スキップ: git commit --no-verify'
    exit 1
}
