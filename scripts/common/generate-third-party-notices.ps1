# ========================================================================
# Script Name : generate-third-party-notices.ps1
# Description : THIRD_PARTY_NOTICES を cargo-about で生成する
# Usage       : . (Join-Path $PSScriptRoot 'generate-third-party-notices.ps1')
# Requires    : PowerShell 7.0+
# ========================================================================

#requires -Version 7.0

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

if (-not (Get-Command cargo-about -ErrorAction SilentlyContinue)) {
    throw @(
        'エラー: cargo-about が見つかりません',
        'インストール: cargo install cargo-about --locked --features cli'
    ) -join [Environment]::NewLine
}

. (Join-Path $PSScriptRoot 'cd-project-root.ps1')

$OutputFile = Join-Path $ProjectRoot 'THIRD_PARTY_NOTICES'
$Template = Join-Path $ProjectRoot 'scripts\templates\third-party-notices.hbs'
$Manifest = Join-Path $ProjectRoot 'crates\clip-refiner\Cargo.toml'
$Config = Join-Path $ProjectRoot 'about.toml'

if (-not (Test-Path -LiteralPath $Template)) {
    throw "エラー: テンプレートが見つかりません: $Template"
}

Write-Host 'THIRD_PARTY_NOTICES を生成しています...'

cargo about generate $Template `
    --config $Config `
    --manifest-path $Manifest `
    --output-file $OutputFile `
    --all-features `
    --locked

if ($LASTEXITCODE -ne 0) {
    throw "エラー: cargo about generate が終了コード $LASTEXITCODE で失敗しました"
}

Write-Host "生成しました: $OutputFile"
