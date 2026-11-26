# ========================================================================
# Script Name : verify-msi.ps1
# Description : ビルド済み MSI の内容・メタデータを検証する
# Usage       : ./verify-msi.ps1
#               ./verify-msi.ps1 -MsiPath target\wix\clip-refiner-0.12.1-x86_64-pc-windows-msvc.msi
#               ./verify-msi.ps1 -Target x64
#               ./verify-msi.ps1 -SkipExtract
# Requires    : PowerShell 7.0+, Windows (Windows Installer COM)
#
# 検証内容:
#   - Cargo.toml upgrade-guid と wix/main.wxs UpgradeCode の一致
#   - ProductVersion / ProductName / UpgradeCode / Manufacturer
#   - 必須ファイル (ClipRefiner.exe, ClipRefinerUpdater.exe, LICENSE, THIRD_PARTY_NOTICES)
#   - 必須 Feature (Binaries, Environment, Autostart)
#   - per-user インストール (ALLUSERS)
#   - MSI Platform テンプレートと実行ファイルの PE アーキテクチャ (x64)
#   - 管理インストール展開後の実ファイル内容 (既定。SkipExtract で省略)
# ========================================================================

#requires -Version 7.0

[CmdletBinding()]
param(
    # 検証対象 MSI (省略時は target/wix からワークスペース version に一致する MSI を解決)
    [string]$MsiPath,

    # 期待する ProductVersion (省略時は Cargo.toml の version)
    [string]$Version,

    # 管理インストールによる展開検証をスキップする
    [switch]$SkipExtract,

    # 配布ターゲット。省略時は MSI ファイル名またはホストから推定
    [string]$Target
)

# シェルオプションを設定(エラー時に即終了)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

# ダブルクリック実行時の Enter 待ちヘルパーを読み込み
. (Join-Path $PSScriptRoot '..\common\wait-if-double-clicked.ps1')

Invoke-ScriptMain {
    # プロジェクトルートへ移動
    . (Join-Path $PSScriptRoot '..\common\cd-project-root.ps1')
    . (Join-Path $PSScriptRoot '..\common\windows-target.ps1')
    . (Join-Path $PSScriptRoot '..\common\verify-msi.ps1')

    $targetTriple = $null
    if (-not [string]::IsNullOrWhiteSpace($Target)) {
        # 1 件だとスカラー (文字列) で返り、[0] が期待どおりに動かないため配列に包む
        $targetTriple = @(Resolve-WindowsPackageTargets -Target $Target)[0]
    }

    Invoke-MsiPackageVerification `
        -ProjectRoot $ProjectRoot `
        -MsiPath $MsiPath `
        -Version $Version `
        -TargetTriple $targetTriple `
        -SkipExtract:$SkipExtract
}
