# ========================================================================
# Script Name : bench.ps1
# Description : Windows 用のベンチマーク (criterion。性能の回帰検知)
# Usage       : ./bench.ps1 [-Save <基準名>] [-Compare <基準名>] [-Filter <ベンチ名>]
# Requires    : PowerShell 7.0+, Rust / Cargo
# ========================================================================

#requires -Version 7.0

param(
    # 計測結果を指定名の基準として保存する
    [string]$Save,

    # 保存済みの基準と比較し、悪化したベンチを一覧する
    [string]$Compare,

    # 名前の一部に一致するベンチだけ実行する (例: json_format)
    [string]$Filter
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

. (Join-Path $PSScriptRoot '..\common\wait-if-double-clicked.ps1')

Invoke-ScriptMain {
    . (Join-Path $PSScriptRoot '..\common\cd-project-root.ps1')
    . (Join-Path $PSScriptRoot '..\common\cargo.ps1')

    Write-Host 'ベンチマークを実行しています...'
    Write-Host ''

    $regressed = @(Invoke-CargoBench -SaveBaseline $Save -Baseline $Compare -Filter $Filter)
    Write-Host ''

    if ($regressed.Count -gt 0) {
        Write-Warning "基準 '$Compare' より悪化したベンチがあります (ぶれの可能性もあるため、再計測で確認してください)"
        $regressed | ForEach-Object { Write-Warning "  $_" }
    }
    elseif ($Compare) {
        Write-Host "基準 '$Compare' からの有意な悪化はありません"
    }

    Write-Host 'ベンチマークが完了しました'
    if (-not ($Save -or $Compare)) {
        Write-Host '基準として保存する場合: ./scripts/windows/bench.ps1 -Save main'
        Write-Host '基準と比較する場合: ./scripts/windows/bench.ps1 -Compare main'
    }
}
