# ========================================================================
# Script Name : check.ps1
# Description : Windows 用の品質チェック (fmt / clippy / deny / eslint / markdownlint / test / e2e)
# Usage       : ./check.ps1 [-Coverage] [-SkipE2E]
# Requires    : PowerShell 7.0+, Node.js (npm), WebView2 Runtime (E2E)
#               -Coverage 時は cargo-llvm-cov と llvm-tools-preview も必要
#               -SkipE2E は WebView E2E だけを省く (pre-push フックが指定する)
# ========================================================================

#requires -Version 7.0

param(
    [switch]$Coverage,
    [switch]$SkipE2E
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

. (Join-Path $PSScriptRoot '..\common\wait-if-double-clicked.ps1')

Invoke-ScriptMain {
    . (Join-Path $PSScriptRoot '..\common\cd-project-root.ps1')
    . (Join-Path $PSScriptRoot '..\common\cargo.ps1')
    . (Join-Path $PSScriptRoot '..\common\npm.ps1')

    $runCoverage = $Coverage -or ($env:CLIP_REFINER_COVERAGE -eq '1')

    Write-Host '品質チェックを実行しています...'
    Write-Host ''

    Write-Host 'コード整形を検証中...'
    Invoke-CargoFmtCheck
    Write-Host ''

    Write-Host 'Clippy を実行中...'
    Invoke-CargoClippy
    Write-Host ''

    Write-Host 'サプライチェーン監査を実行中...'
    . (Join-Path $PSScriptRoot '..\common\cargo-deny-check.ps1')
    Write-Host ''

    Write-Host 'Node.js を検証中...'
    Assert-NodeInstalled
    $env:CLIP_REFINER_REQUIRE_NODE = '1'
    Write-Host ''

    Write-Host 'JavaScript リンターを実行中...'
    Invoke-JavaScriptLintCheck
    Write-Host ''

    Write-Host 'Markdown リンターを実行中...'
    Invoke-MarkdownLintCheck
    Write-Host ''

    if ($runCoverage) {
        Write-Host 'カバレッジ付きテストを実行中...'
        Invoke-CargoLlvmCov
        Write-Host ''

        Write-Host 'doctests を実行中 (カバレッジ対象外)...'
        Invoke-CargoDocTest
        Write-Host ''
    }
    else {
        Write-Host 'テストを実行中...'
        Invoke-CargoTest -IncludeDocTests
        Write-Host ''
    }

    if ($SkipE2E) {
        Write-Host 'note: WebView E2E をスキップしました (-SkipE2E)'
        Write-Host ''
    }
    elseif ($env:CLIP_REFINER_SKIP_E2E -eq '1') {
        Write-Host 'note: WebView E2E をスキップしました (CLIP_REFINER_SKIP_E2E=1)'
        Write-Host ''
    }
    else {
        Write-Host 'E2E 用バイナリをビルド中...'
        Invoke-CargoBuild
        Write-Host ''

        Write-Host 'WebView E2E (Playwright) を実行中...'
        Install-NpmDependenciesIfNeeded
        Assert-NpmInstalled
        if ($env:CARGO_TARGET_DIR) {
            $env:CLIP_REFINER_E2E_EXE = Join-Path $env:CARGO_TARGET_DIR 'debug\ClipRefiner.exe'
        }
        else {
            $env:CLIP_REFINER_E2E_EXE = Join-Path (Get-Location) 'target\debug\ClipRefiner.exe'
        }
        npm run test:e2e
        Assert-LastExitCode -CommandLabel 'npm run test:e2e'
        Write-Host ''
    }

    Write-Host 'すべてのチェックが完了しました'
}
