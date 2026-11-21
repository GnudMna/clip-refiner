# ========================================================================
# Script Name : build-msi.ps1
# Description : Windows 向け MSI インストーラー作成 (cargo-wix)
# Usage       : ./build-msi.ps1 [-SkipBuild] [-SkipVerify]
# Requires    : PowerShell 7.0+, cargo-wix, WiX Toolset v3.14.1+
#
# 前提:
#   - cargo install cargo-wix --locked
#   - cargo install cargo-about --locked --features cli
#   - winget install WiXToolset.WiXToolset  (WiX v3。コード署名は不要)
#   - wix/main.wxs, wix/Banner.bmp, wix/Dialog.bmp, wix/Eula.rtf が存在すること
#     (画像・Eula.rtf は scripts/windows/generate-wix-assets.ps1 で再生成可能)
#   - Cargo.toml の culture = "ja-JP" でインストーラー UI を日本語化
#
# 出力:
#   target/wix/{パッケージ名}-{バージョン}-{アーキテクチャ}.msi
# ========================================================================

#requires -Version 7.0

[CmdletBinding()]
param(
    # リリースビルドをスキップし、既存の target/release/*.exe から MSI のみ作成する
    [switch]$SkipBuild,

    # MSI 作成後の内容検証 (verify-msi.ps1) をスキップする
    [switch]$SkipVerify
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

. (Join-Path $PSScriptRoot '..\common\wait-if-double-clicked.ps1')

Invoke-ScriptMain {
    . (Join-Path $PSScriptRoot '..\common\cd-project-root.ps1')
    . (Join-Path $PSScriptRoot '..\common\assert.ps1')
    . (Join-Path $PSScriptRoot '..\common\wix.ps1')

    $MainWxsPath = Join-Path $ProjectRoot 'wix\main.wxs'
    if (-not (Test-Path -LiteralPath $MainWxsPath)) {
        throw "エラー: wix\main.wxs が見つかりません。先に cargo wix init を実行してください。"
    }

    Assert-CargoWixInstalled
    Assert-WixToolsetInstalled

    $GenerateAssetsScript = Join-Path $PSScriptRoot 'generate-wix-assets.ps1'
    & $GenerateAssetsScript
    Assert-LastExitCode -CommandLabel 'generate-wix-assets.ps1'

    . (Join-Path $PSScriptRoot '..\common\generate-third-party-notices.ps1')

    Write-Host 'MSI インストーラーの作成を開始します...'
    Write-Host ''

    $wixArgs = @('-p', 'clip-refiner', '-I', 'wix/main.wxs', '--nocapture')
    if ($SkipBuild) {
        $wixArgs += '--no-build'
    }
    cargo wix @wixArgs

    if ($LASTEXITCODE -ne 0) {
        throw @"
エラー: cargo wix が終了コード $LASTEXITCODE で失敗しました

cargo の終了コード 101 はリリースビルド (cargo build --release) の失敗を示すことが多い。
上記の rustc / linker のメッセージを確認するか、次を手動で実行してください:

  rustc --version
  cargo build --release -p clip-refiner

Rust 1.96 以上 (rust-toolchain.toml) と Visual Studio Build Tools (C++ ワークロード) が必要です。
"@
    }

    Write-Host ''
    Write-Host 'MSI の作成が完了しました:'
    Write-Host "  $(Join-Path $ProjectRoot 'target\wix')"
    Write-Host ''

    if (-not $SkipVerify) {
        . (Join-Path $PSScriptRoot '..\common\release-version.ps1')
        . (Join-Path $PSScriptRoot '..\common\verify-msi.ps1')

        $workspaceVersion = Get-WorkspaceVersion -ProjectRoot $ProjectRoot
        Invoke-MsiPackageVerification -ProjectRoot $ProjectRoot -Version $workspaceVersion
    } else {
        Write-Host 'MSI 検証をスキップしました (-SkipVerify)'
    }
}
