# ========================================================================
# Script Name : build.ps1
# Description : Windows用のビルド実行スクリプト
# Usage       : ./build.ps1 [--release|-r]
# Requires    : PowerShell 7.0+
# ========================================================================

#requires -Version 7.0

param(
    [switch]$Release
)

# シェルオプションを設定(エラー時に即終了)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

# ダブルクリック実行時の Enter 待ちヘルパーを読み込み
. (Join-Path $PSScriptRoot '..\common\wait-if-double-clicked.ps1')

Invoke-ScriptMain {
    # プロジェクトルートへ移動
    . (Join-Path $PSScriptRoot '..\common\cd-project-root.ps1')

    # 引数を解析(--release / -r)
    if ($args -contains '--release' -or $args -contains '-r') {
        $Release = $true
    }

    # ビルドを実行
    Write-Host 'Rust プロジェクトのビルドを実行しています...'
    Write-Host ''

    if ($Release) {
        Write-Host 'リリースビルドを実行中...'
        cargo build --release
    } else {
        Write-Host 'デバッグビルドを実行中...'
        cargo build
    }

    if ($LASTEXITCODE -ne 0) {
        throw "エラー: cargo build が終了コード $LASTEXITCODE で失敗しました"
    }

    Write-Host ''
    Write-Host 'ビルドが完了しました'
}
