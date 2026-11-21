# ========================================================================
# Script Name : build.ps1
# Description : Windows 用のビルド実行
# Usage       : ./build.ps1 [--release|-r]
#               ./build.ps1 -Release -Target x64
# Requires    : PowerShell 7.0+, Rust / Cargo
# ========================================================================

#requires -Version 7.0

param(
    [switch]$Release,

    # 配布ターゲット。省略時はホスト。Host / x64 / triple
    [string]$Target
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

. (Join-Path $PSScriptRoot '..\common\wait-if-double-clicked.ps1')

Invoke-ScriptMain {
    . (Join-Path $PSScriptRoot '..\common\cd-project-root.ps1')
    . (Join-Path $PSScriptRoot '..\common\assert.ps1')
    . (Join-Path $PSScriptRoot '..\common\windows-target.ps1')

    if ($args -contains '--release' -or $args -contains '-r') {
        $Release = $true
    }

    $targets = Resolve-WindowsPackageTargets -Target $Target

    Write-Host 'Rust プロジェクトのビルドを実行しています...'
    Write-Host ''

    foreach ($triple in $targets) {
        Assert-RustTargetInstalled -TargetTriple $triple
        $implicitHost = Test-WindowsImplicitHostTarget -TargetTriple $triple -RequestedTarget $Target
        if (-not $implicitHost) {
            Import-WindowsMsvcBuildEnvironment -TargetTriple $triple
        }
        $targetArgs = Get-CargoTargetArguments -TargetTriple $triple -ImplicitHost:$implicitHost

        if ($Release) {
            Write-Host "リリースビルドを実行中 ($triple)..."
            cargo build --locked --release @targetArgs
        } else {
            Write-Host "デバッグビルドを実行中 ($triple)..."
            cargo build --locked @targetArgs
        }

        if ((Test-Path variable:LASTEXITCODE) -and $LASTEXITCODE -ne 0) {
            $hint = Get-WindowsCrossCompileHint -TargetTriple $triple
            if ($hint) {
                throw "エラー: cargo build が終了コード $LASTEXITCODE で失敗しました`n$hint"
            }
            throw "エラー: cargo build が終了コード $LASTEXITCODE で失敗しました"
        }

        Write-Host ''
    }

    Write-Host 'ビルドが完了しました'
}
