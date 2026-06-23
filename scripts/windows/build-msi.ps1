# ========================================================================
# Script Name : build-msi.ps1
# Description : Windows 向け MSI インストーラー作成スクリプト (cargo-wix)
# Usage       : ./build-msi.ps1 [-SkipBuild]
# Requires    : PowerShell 7.0+, cargo-wix, WiX Toolset v3.14.1+
#
# 前提:
#   - cargo install cargo-wix --locked
#   - winget install WiXToolset.WiXToolset  (WiX v3。コード署名は不要)
#   - wix/main.wxs が存在すること (cargo wix init で生成)
#   - Cargo.toml の culture = "ja-JP" でインストーラー UI を日本語化
#
# 出力:
#   target/wix/{パッケージ名}-{バージョン}-{アーキテクチャ}.msi
# ========================================================================

#requires -Version 7.0

[CmdletBinding()]
param(
    # リリースビルドをスキップし、既存の target/release/*.exe から MSI のみ作成する
    [switch]$SkipBuild
)

# シェルオプションを設定(エラー時に即終了)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

# ダブルクリック実行時の Enter 待ちヘルパーを読み込み
. (Join-Path $PSScriptRoot '..\common\wait-if-double-clicked.ps1')

# cargo-wix がインストールされているか確認する
function Assert-CargoWixInstalled {
    $null = & cargo wix --version 2>&1
    if ($LASTEXITCODE -ne 0) {
        throw @"
エラー: cargo-wix が見つかりません。

以下でインストールしてください:
  cargo install cargo-wix --locked
"@
    }
}

# WiX Toolset v3 (candle.exe / light.exe) が利用可能か確認する
function Assert-WixToolsetInstalled {
    $candle = Get-Command candle.exe -ErrorAction SilentlyContinue
    if ($candle) {
        return
    }

    # WiX インストーラーが設定する WIX 環境変数を参照する (値は bin の親ディレクトリ)
    if ($env:WIX) {
        foreach ($relativePath in @('bin\candle.exe', 'candle.exe')) {
            $candlePath = Join-Path $env:WIX $relativePath
            if (Test-Path -LiteralPath $candlePath) {
                return
            }
        }
    }

    throw @"
エラー: WiX Toolset v3 (candle.exe / light.exe) が見つかりません。

cargo-wix は WiX v3 を使用します。以下のいずれかでインストールしてください:

  winget install WiXToolset.WiXToolset

インストール後、新しいターミナルで candle -? を実行して確認してください。
詳細: https://github.com/volks73/cargo-wix
"@
}

Invoke-ScriptMain {
    # プロジェクトルートへ移動
    . (Join-Path $PSScriptRoot '..\common\cd-project-root.ps1')

    # WiX ソースの存在を確認 (Cargo.toml の [package.metadata.wix] と対になる)
    $MainWxsPath = Join-Path $ProjectRoot 'wix\main.wxs'
    if (-not (Test-Path -LiteralPath $MainWxsPath)) {
        throw "エラー: wix\main.wxs が見つかりません。先に cargo wix init を実行してください。"
    }

    # 依存ツールを検証
    $null = Assert-CargoWixInstalled
    $null = Assert-WixToolsetInstalled

    Write-Host 'MSI インストーラーの作成を開始します...'
    Write-Host ''

    # cargo wix: リリースビルド + WiX コンパイル/リンク (MSI 生成)
    # --nocapture: cargo build の rustc エラーを表示する (省略時は exit 101 だけになる)
    $wixArgs = @('--nocapture')
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
  cargo build --release

Rust 1.96 以上 (rust-toolchain.toml) と Visual Studio Build Tools (C++ ワークロード) が必要です。
"@
    }

    Write-Host ''
    Write-Host 'MSI の作成が完了しました:'
    Write-Host "  $(Join-Path $ProjectRoot 'target\wix')"
}
