# ========================================================================
# Script Name : fuzz.ps1
# Description : Windows 用のファジング (cargo-fuzz)
# Usage       : ./fuzz.ps1 [-Target <名前>] [-Seconds <秒>] [-Reproduce <入力ファイル>]
# Requires    : PowerShell 7.0+, Rust nightly, cargo-fuzz, Visual Studio (C++ ツール)
# ========================================================================

#requires -Version 7.0

param(
    # 省略時は全ターゲットを順に実行する
    [ValidateSet('all_modes', 'line_sort', 'markdown_table', 'json', 'yaml', 'html')]
    [string]$Target,

    # ターゲットごとの実行時間
    [ValidateRange(1, [int]::MaxValue)]
    [int]$Seconds = 60,

    # 失敗した入力 (fuzz/artifacts/<ターゲット>/crash-*) を再実行する。-Target と併用する
    [string]$Reproduce
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

. (Join-Path $PSScriptRoot '..\common\wait-if-double-clicked.ps1')

# プロジェクトルートへ移動する前に、呼び出し元から見た相対パスを解決する
$reproducePath = if ($Reproduce) { (Resolve-Path -LiteralPath $Reproduce).Path } else { $null }

Invoke-ScriptMain {
    . (Join-Path $PSScriptRoot '..\common\cd-project-root.ps1')
    . (Join-Path $PSScriptRoot '..\common\fuzz.ps1')

    Assert-CargoFuzzReady
    Add-AsanRuntimeToPath

    if ($reproducePath) {
        if (-not $Target) {
            throw 'エラー: -Reproduce には -Target も指定してください'
        }
        Invoke-CargoFuzzReproduce -Target $Target -InputPath $reproducePath
        return
    }

    $targets = if ($Target) { @($Target) } else { $script:FuzzTargets }
    foreach ($name in $targets) {
        Write-Host "ファジングを実行しています: $name ($Seconds 秒)..."
        Invoke-CargoFuzz -Target $name -Seconds $Seconds
        Write-Host ''
    }

    Write-Host 'ファジングが完了しました'
}
