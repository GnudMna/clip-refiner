# ========================================================================
# Script Name : fuzz.ps1
# Description : cargo-fuzz の共通処理 (前提確認・ASan ランタイムの PATH 追加・実行)
# Usage       : . (Join-Path $PSScriptRoot 'fuzz.ps1')
# Requires    : PowerShell 7.0+, Rust nightly, cargo-fuzz, Visual Studio (C++ ツール)
# ========================================================================

#requires -Version 7.0

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

. (Join-Path $PSScriptRoot 'assert.ps1')

# fuzz/Cargo.toml の [[bin]] と同じ並び
$script:FuzzTargets = @('all_modes', 'line_sort', 'markdown_table', 'json', 'yaml', 'html')

# nightly ツールチェーンと cargo-fuzz が使えることを検証する
function Assert-CargoFuzzReady {
    [CmdletBinding()]
    param()

    $toolchains = rustup toolchain list 2>$null
    Assert-LastExitCode -CommandLabel 'rustup toolchain list'
    if (-not ($toolchains | Where-Object { $_ -match '^nightly-' })) {
        throw @(
            'エラー: Rust nightly ツールチェーンが未インストールです',
            'インストール: rustup toolchain install nightly --profile minimal'
        ) -join [Environment]::NewLine
    }

    if (-not (Get-Command cargo-fuzz -ErrorAction SilentlyContinue)) {
        throw @(
            'エラー: cargo-fuzz が見つかりません',
            'インストール: cargo +nightly install cargo-fuzz --locked'
        ) -join [Environment]::NewLine
    }
}

# AddressSanitizer のランタイム DLL があるフォルダを PATH の先頭へ追加する
#
# Windows の cargo-fuzz は既定で ASan を有効にしてビルドし、実行時に
# clang_rt.asan_dynamic-x86_64.dll を必要とする (見つからないと STATUS_DLL_NOT_FOUND で終了する)
function Add-AsanRuntimeToPath {
    [CmdletBinding()]
    param()

    $dllName = 'clang_rt.asan_dynamic-x86_64.dll'
    if (Get-Command $dllName -ErrorAction SilentlyContinue) {
        return
    }

    $vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
    if (-not (Test-Path -LiteralPath $vswhere)) {
        throw 'エラー: vswhere.exe が見つかりません。Visual Studio (C++ によるデスクトップ開発) をインストールしてください'
    }

    $installPath = & $vswhere -latest -products * `
        -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
    Assert-LastExitCode -CommandLabel 'vswhere'
    if (-not $installPath) {
        throw 'エラー: MSVC の C++ ツールを含む Visual Studio が見つかりません'
    }

    $versionFile = Join-Path $installPath 'VC\Auxiliary\Build\Microsoft.VCToolsVersion.default.txt'
    $toolsVersion = (Get-Content -LiteralPath $versionFile -Raw).Trim()
    $binDir = Join-Path $installPath "VC\Tools\MSVC\$toolsVersion\bin\Hostx64\x64"
    if (-not (Test-Path -LiteralPath (Join-Path $binDir $dllName))) {
        throw "エラー: $dllName が見つかりません ($binDir)。Visual Studio Installer で「C++ AddressSanitizer」を追加してください"
    }

    $env:PATH = "$binDir;$env:PATH"
}

# ファズターゲットを実行する
#
# 生成した入力は fuzz/corpus/<ターゲット>/ (Git 管理外) に蓄積し、次回の実行で再利用する。
# 失敗した入力は fuzz/artifacts/<ターゲット>/ に保存する
function Invoke-CargoFuzz {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)]
        [string]$Target,

        [Parameter(Mandatory = $true)]
        [int]$Seconds
    )

    # libFuzzer の既定 (4096 バイト) より長い表や JSON も生成させる
    cargo +nightly fuzz run $Target -- "-max_total_time=$Seconds" '-max_len=65536'
    Assert-LastExitCode -CommandLabel "cargo fuzz run $Target"
}

# 失敗した入力を再実行する
function Invoke-CargoFuzzReproduce {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)]
        [string]$Target,

        [Parameter(Mandatory = $true)]
        [string]$InputPath
    )

    cargo +nightly fuzz run $Target $InputPath
    Assert-LastExitCode -CommandLabel "cargo fuzz run $Target $InputPath"
}
