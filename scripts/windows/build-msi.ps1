# ========================================================================
# Script Name : build-msi.ps1
# Description : Windows 向け MSI インストーラー作成 (cargo-wix)
# Usage       : ./build-msi.ps1 [-SkipBuild] [-SkipVerify] [-Target x64]
# Requires    : PowerShell 7.0+, cargo-wix, WiX Toolset v3.14.1+, mdBook
#
# 前提:
#   - cargo install cargo-wix --locked
#   - cargo install cargo-about --locked --features cli
#   - cargo install mdbook --locked --version <scripts/common/build-user-docs.ps1 の固定版>
#   - winget install WiXToolset.WiXToolset  (WiX v3。コード署名は不要)
#   - wix/main.wxs, wix/Banner.bmp, wix/Dialog.bmp が存在すること
#     (画像は scripts/windows/generate-wix-assets.ps1 で再生成可能)
#   - Cargo.toml の culture = "ja-JP" でインストーラー UI を日本語化
#
# 出力:
#   target/wix/{パッケージ名}-{バージョン}-{target-triple}.msi
# ========================================================================

#requires -Version 7.0

[CmdletBinding()]
param(
    # リリースビルドをスキップし、既存の target/release/*.exe から MSI のみ作成する
    [switch]$SkipBuild,

    # MSI 作成後の内容検証 (verify-msi.ps1) をスキップする
    [switch]$SkipVerify,

    # 配布ターゲット。省略時はホスト。Host / x64 / triple
    [string]$Target
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

. (Join-Path $PSScriptRoot '..\common\wait-if-double-clicked.ps1')

Invoke-ScriptMain {
    . (Join-Path $PSScriptRoot '..\common\cd-project-root.ps1')
    . (Join-Path $PSScriptRoot '..\common\assert.ps1')
    . (Join-Path $PSScriptRoot '..\common\wix.ps1')
    . (Join-Path $PSScriptRoot '..\common\windows-target.ps1')

    $MainWxsPath = Join-Path $ProjectRoot 'wix\main.wxs'
    if (-not (Test-Path -LiteralPath $MainWxsPath)) {
        throw "エラー: wix/main.wxs が見つかりません。先に cargo wix init を実行してください。"
    }

    Assert-CargoWixInstalled
    Assert-WixToolsetInstalled

    $GenerateAssetsScript = Join-Path $PSScriptRoot 'generate-wix-assets.ps1'
    & $GenerateAssetsScript
    Assert-LastExitCode -CommandLabel 'generate-wix-assets.ps1'

    . (Join-Path $PSScriptRoot '..\common\generate-third-party-notices.ps1')
    . (Join-Path $PSScriptRoot '..\common\release-version.ps1')
    . (Join-Path $PSScriptRoot '..\common\verify-msi.ps1')
    . (Join-Path $PSScriptRoot '..\common\build-user-docs.ps1')

    $userDocsDir = Invoke-UserDocsBuild -ProjectRoot $ProjectRoot
    $userDocsWxsPath = Join-Path $ProjectRoot 'target\wix\user-docs.wxs'
    New-UserDocsWixFragment -DocsDir $userDocsDir -OutputPath $userDocsWxsPath
    Write-Host ''

    $targets = Resolve-WindowsPackageTargets -Target $Target
    $wixDir = Join-Path $ProjectRoot 'target\wix'
    $workspaceVersion = Get-WorkspaceVersion -ProjectRoot $ProjectRoot

    foreach ($triple in $targets) {
        Assert-RustTargetInstalled -TargetTriple $triple
        $implicitHost = Test-WindowsImplicitHostTarget -TargetTriple $triple -RequestedTarget $Target
        if (-not $implicitHost) {
            Import-WindowsMsvcBuildEnvironment -TargetTriple $triple
        }

        Write-Host "MSI インストーラーの作成を開始します ($triple)..."
        Write-Host ''

        # cargo wix は --locked を渡せないため、ビルドは自前で行い cargo wix には --no-build を渡す
        if (-not $SkipBuild) {
            $targetArgs = Get-CargoTargetArguments -TargetTriple $triple -ImplicitHost:$implicitHost
            cargo build --locked -p clip-refiner --bins --release @targetArgs
            if ($LASTEXITCODE -ne 0) {
                $hint = Get-WindowsCrossCompileHint -TargetTriple $triple
                $hintBlock = if ($hint) { "`n$hint" } else { '' }
                throw "エラー: cargo build --locked -p clip-refiner --bins --release が終了コード $LASTEXITCODE で失敗しました ($triple)$hintBlock"
            }
            Write-Host ''
        }

        $wixArgs = @('-p', 'clip-refiner', '-I', 'wix/main.wxs', '-I', $userDocsWxsPath, '--nocapture', '--no-build')
        if (-not $implicitHost) {
            $binDir = Get-CargoReleaseBinDirectory `
                -ProjectRoot $ProjectRoot `
                -TargetTriple $triple
            $wixArgs += @('-t', $triple, '--target-bin-dir', $binDir)
        }

        cargo wix @wixArgs

        if ($LASTEXITCODE -ne 0) {
            $hint = Get-WindowsCrossCompileHint -TargetTriple $triple
            $hintBlock = if ($hint) { "`n`n$hint" } else { '' }
            throw @"
エラー: cargo wix が終了コード $LASTEXITCODE で失敗しました ($triple)

上記のメッセージを確認するか、リリースビルドを手動で実行して原因を調べてください:

  rustc --version
  cargo build --locked --release -p clip-refiner --bins$(if (-not $implicitHost) { " --target $triple" })

Rust 1.96 以上 (rust-toolchain.toml) と Visual Studio Build Tools (C++ ワークロード) が必要です。$hintBlock
"@
        }

        Write-Host ''
        Write-Host 'MSI の作成が完了しました:'
        Write-Host "  $wixDir"
        Write-Host ''

        if (-not $SkipVerify) {
            Invoke-MsiPackageVerification `
                -ProjectRoot $ProjectRoot `
                -Version $workspaceVersion `
                -TargetTriple $triple
        } else {
            Write-Host 'MSI 検証をスキップしました (-SkipVerify)'
        }

        Write-Host ''
    }
}
