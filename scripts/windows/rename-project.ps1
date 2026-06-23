# ========================================================================
# Script Name : rename-project.ps1
# Description : テンプレートのプロジェクト名を一括変更する
# Usage       : ./rename-project.ps1 <new-name> [copyright-holder]
# Requires    : PowerShell 7.0+
# ========================================================================

#requires -Version 7.0

[CmdletBinding()]
param(
    [Parameter(Mandatory = $true, Position = 0)]
    [string]$NewName,

    [Parameter(Position = 1)]
    [string]$CopyrightHolder = ''
)

# シェルオプションを設定(エラー時に即終了)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

# ダブルクリック実行時の Enter 待ちヘルパーを読み込み
. (Join-Path $PSScriptRoot '..\common\wait-if-double-clicked.ps1')

Invoke-ScriptMain {
    # プロジェクトルートへ移動
    . (Join-Path $PSScriptRoot '..\common\cd-project-root.ps1')

    # UTF-8(BOM なし)読み書き用のヘルパーを定義
    $Utf8NoBom = [System.Text.UTF8Encoding]::new($false)

    function Read-Utf8NoBom {
        param(
            [Parameter(Mandatory = $true)]
            [string]$Path
        )

        return [System.IO.File]::ReadAllText($Path, $Utf8NoBom)
    }

    function Write-Utf8NoBom {
        param(
            [Parameter(Mandatory = $true)]
            [string]$Path,

            [Parameter(Mandatory = $true)]
            [string]$Value
        )

        [System.IO.File]::WriteAllText($Path, $Value, $Utf8NoBom)
    }

    # プロジェクト名の形式を検証(kebab-case)
    if ($NewName -notmatch '^[a-z][a-z0-9]*(-[a-z0-9]+)*$') {
        throw 'エラー: プロジェクト名は kebab-case で指定してください (例: my-project)'
    }

    # 現在のプロジェクト名と Rust 識別子を取得
    $CargoTomlPath = Join-Path $ProjectRoot 'Cargo.toml'
    $CargoToml = Read-Utf8NoBom -Path $CargoTomlPath
    if ($CargoToml -notmatch 'name = "([^"]+)"') {
        throw 'エラー: Cargo.toml から name を読み取れません'
    }

    $CurrentName = $Matches[1]
    $NewIdent = $NewName -replace '-', '_'
    $OldIdent = $CurrentName -replace '-', '_'

    # 変更が不要な場合は終了
    if ($CurrentName -eq $NewName) {
        Write-Host "プロジェクト名は既に `"$NewName`" です"
        return
    }

    Write-Host "プロジェクト名を `"$CurrentName`" から `"$NewName`" に変更しています..."
    Write-Host ''

    $OldUpgradeGuid = $null
    $OldPathGuid = $null
    if ($CargoToml -match '(?m)^upgrade-guid = "([^"]+)"') {
        $OldUpgradeGuid = $Matches[1]
    }
    if ($CargoToml -match '(?m)^path-guid = "([^"]+)"') {
        $OldPathGuid = $Matches[1]
    }

    # Cargo.toml の name を更新
    $CargoToml = $CargoToml -replace "name = `"$CurrentName`"", "name = `"$NewName`""

    # MSI 用 GUID を再生成(別アプリとの衝突を防ぐ)
    $NewUpgradeGuid = $null
    $NewPathGuid = $null
    if ($OldUpgradeGuid -and $OldPathGuid) {
        $NewUpgradeGuid = [guid]::NewGuid().ToString().ToUpperInvariant()
        $NewPathGuid = [guid]::NewGuid().ToString().ToUpperInvariant()
        $CargoToml = [regex]::Replace(
            $CargoToml,
            '(?m)^upgrade-guid = ".*"',
            "upgrade-guid = `"$NewUpgradeGuid`""
        )
        $CargoToml = [regex]::Replace(
            $CargoToml,
            '(?m)^path-guid = ".*"',
            "path-guid = `"$NewPathGuid`""
        )
    }

    Write-Utf8NoBom -Path $CargoTomlPath -Value $CargoToml

    # use 文と doc コメントのクレート名を更新
    foreach ($File in @('src/main.rs', 'tests/integration_test.rs', 'src/lib.rs')) {
        $FilePath = Join-Path $ProjectRoot $File
        $Content = Read-Utf8NoBom -Path $FilePath
        $Content = $Content -replace "use ${OldIdent}::", "use ${NewIdent}::"
        $OldCrateRef = '`' + $CurrentName + '`'
        $NewCrateRef = '`' + $NewName + '`'
        $Content = $Content.Replace($OldCrateRef, $NewCrateRef)
        Write-Utf8NoBom -Path $FilePath -Value $Content
    }

    # wix/main.wxs のプロジェクト名と GUID を更新
    $MainWxsPath = Join-Path $ProjectRoot 'wix\main.wxs'
    if (Test-Path -LiteralPath $MainWxsPath) {
        $MainWxs = Read-Utf8NoBom -Path $MainWxsPath
        $MainWxs = $MainWxs.Replace($CurrentName, $NewName)
        if ($OldUpgradeGuid -and $NewUpgradeGuid) {
            $MainWxs = $MainWxs.Replace($OldUpgradeGuid, $NewUpgradeGuid)
        }
        if ($OldPathGuid -and $NewPathGuid) {
            $MainWxs = $MainWxs.Replace($OldPathGuid, $NewPathGuid)
        }
        Write-Utf8NoBom -Path $MainWxsPath -Value $MainWxs
    }

    # 著作権表記を更新(指定時のみ)
    if ($CopyrightHolder) {
        $CopyrightLine = 'Copyright (c) 2026 ' + $CopyrightHolder

        $LicensePath = Join-Path $ProjectRoot 'LICENSE'
        $License = Read-Utf8NoBom -Path $LicensePath
        $License = [regex]::Replace($License, '(?m)^Copyright \(c\) .*', $CopyrightLine)
        Write-Utf8NoBom -Path $LicensePath -Value $License

        $BuildRsPath = Join-Path $ProjectRoot 'build.rs'
        $BuildRs = Read-Utf8NoBom -Path $BuildRsPath
        $LegalCopyright = 'const LEGAL_COPYRIGHT: &str = "' + $CopyrightLine + '";'
        $BuildRs = [regex]::Replace($BuildRs, 'const LEGAL_COPYRIGHT: &str = ".*";', $LegalCopyright)
        Write-Utf8NoBom -Path $BuildRsPath -Value $BuildRs
    }

    # Cargo.lock を更新
    Write-Host 'Cargo.lock を更新しています...'
    cargo check --quiet
    if ($LASTEXITCODE -ne 0) {
        throw "エラー: cargo check が終了コード $LASTEXITCODE で失敗しました"
    }
    Write-Host ''

    # 手動更新が必要な項目を案内
    Write-Host '変更が完了しました。必要に応じて以下を手動で更新してください:'
    Write-Host '  - Cargo.toml の description, authors, repository'
    Write-Host '  - README.md のタイトル'
    Write-Host '  - assets/icon.ico'
    if (-not $CopyrightHolder) {
        Write-Host '  - LICENSE / build.rs の著作権表記(または -CopyrightHolder を指定して再実行)'
    }
}
