# ========================================================================
# Script Name : wix.ps1
# Description : WiX / cargo-wix ツールチェーン検証 (ドットソース用)
# Usage       : . (Join-Path $PSScriptRoot 'wix.ps1')
# Requires    : PowerShell 7.0+, cargo-wix, WiX Toolset v3
# ========================================================================

#requires -Version 7.0

Set-StrictMode -Version Latest

# cargo-wix がインストールされているか確認する
function Assert-CargoWixInstalled {
    [CmdletBinding()]
    param()

    $null = & cargo wix --version 2>&1
    if ($LASTEXITCODE -ne 0) {
        throw @(
            'エラー: cargo-wix が見つかりません',
            '',
            '以下でインストールしてください:',
            '  cargo install cargo-wix --locked'
        ) -join [Environment]::NewLine
    }
}

# WiX Toolset v3 (candle.exe / light.exe) が利用可能か確認する
function Assert-WixToolsetInstalled {
    [CmdletBinding()]
    param()

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

    throw @(
        'エラー: WiX Toolset v3 (candle.exe / light.exe) が見つかりません',
        '',
        'cargo-wix は WiX v3 を使用します。以下のいずれかでインストールしてください:',
        '',
        '  winget install WiXToolset.WiXToolset',
        '',
        'インストール後、新しいターミナルで candle -? を実行して確認してください。',
        '詳細: https://github.com/volks73/cargo-wix'
    ) -join [Environment]::NewLine
}
