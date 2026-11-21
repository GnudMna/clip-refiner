# ========================================================================
# Script Name : npm.ps1
# Description : Node.js / npm コマンドの共通ラッパー (ドットソース用)
# Usage       : . (Join-Path $PSScriptRoot 'npm.ps1')
# Requires    : PowerShell 7.0+, Node.js (npm)
# ========================================================================

#requires -Version 7.0

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

. (Join-Path $PSScriptRoot 'assert.ps1')

# Node.js / npm のディレクトリを PATH に追加する (インストール済みだが PATH 未設定の場合)
function Initialize-NodeJsPath {
    [CmdletBinding()]
    param()

    if (Get-Command node -ErrorAction SilentlyContinue) {
        return
    }

    $candidateDirs = @()

    if ($env:ProgramFiles) {
        $candidateDirs += Join-Path $env:ProgramFiles 'nodejs'
    }

    if (${env:ProgramFiles(x86)}) {
        $candidateDirs += Join-Path ${env:ProgramFiles(x86)} 'nodejs'
    }

    if ($env:LOCALAPPDATA) {
        $candidateDirs += Join-Path $env:LOCALAPPDATA 'Programs\nodejs'
    }

    if ($env:NVM_SYMLINK) {
        $candidateDirs += $env:NVM_SYMLINK
    }

    if ($env:VOLTA_HOME) {
        $candidateDirs += Join-Path $env:VOLTA_HOME 'bin'
    }

    foreach ($dir in $candidateDirs) {
        if (-not $dir) {
            continue
        }

        $nodeExe = Join-Path $dir 'node.exe'
        if (Test-Path -LiteralPath $nodeExe) {
            $env:PATH = "$dir;$env:PATH"
            return
        }
    }
}

# npm が利用可能であることを検証する
function Assert-NpmInstalled {
    [CmdletBinding()]
    param()

    Initialize-NodeJsPath

    if (-not (Get-Command npm -ErrorAction SilentlyContinue)) {
        throw @(
            'エラー: npm が見つかりません',
            'Node.js をインストールしてください: https://nodejs.org/'
        ) -join [Environment]::NewLine
    }
}

# Node.js が利用可能であることを検証する
function Assert-NodeInstalled {
    [CmdletBinding()]
    param()

    Initialize-NodeJsPath

    if (-not (Get-Command node -ErrorAction SilentlyContinue)) {
        throw @(
            'エラー: node が見つかりません',
            'Node.js をインストールしてください: https://nodejs.org/'
        ) -join [Environment]::NewLine
    }

    node --version
    Assert-LastExitCode -CommandLabel 'node --version'
}

# node_modules がなければ npm ci で依存関係をインストールする
function Install-NpmDependenciesIfNeeded {
    [CmdletBinding()]
    param()

    Assert-NpmInstalled

    $playwrightPkg = Join-Path 'node_modules' '@playwright\test'
    if ((Test-Path -LiteralPath 'node_modules') -and (Test-Path -LiteralPath $playwrightPkg)) {
        return
    }

    Write-Host 'npm 依存関係をインストールしています...'
    if (Test-Path -LiteralPath 'package-lock.json') {
        npm ci
        Assert-LastExitCode -CommandLabel 'npm ci'
    }
    else {
        npm install
        Assert-LastExitCode -CommandLabel 'npm install'
    }
    Write-Host ''
}

# package.json の npm script を実行する
function Invoke-NpmScript {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)]
        [string]$Name
    )

    Install-NpmDependenciesIfNeeded
    npm run $Name
    Assert-LastExitCode -CommandLabel "npm run $Name"
}

# 設定 / セレクター UI バンドルが再生成後も作業ツリーと index で一致することを検証する
#
# `*.bundle.js` は `include_str!` で埋め込むためリポジトリにコミットする。
# ソースだけ更新してバンドルを忘れると、品質チェックは再生成で緑になっても
# 配布物の UI が古いままになる。再生成直後に未ステージ差分が残っていれば失敗させる。
# ステージ済みのみの変更 (`M `) はコミット前の正当な状態として許容する
function Assert-UiBundlesSynced {
    [CmdletBinding()]
    param()

    $relativePaths = @(
        'crates/clip-refiner/src/ui/settings/settings.bundle.js',
        'crates/clip-refiner/src/ui/quick_selector.bundle.js',
        'crates/clip-refiner/src/ui/clip_selector.bundle.js'
    )

    if (-not (Get-Command git -ErrorAction SilentlyContinue)) {
        throw @(
            'エラー: git が見つかりません',
            'UI バンドルのコミットずれ検出には git が必要です'
        ) -join [Environment]::NewLine
    }

    foreach ($relativePath in $relativePaths) {
        $status = git status --porcelain --untracked-files=all -- $relativePath 2>&1
        if ($LASTEXITCODE -ne 0) {
            throw "エラー: git status の実行に失敗しました: $status"
        }

        $statusText = if ($null -eq $status) { '' } else { "$status".TrimEnd() }
        if (-not $statusText) {
            continue
        }

        # 許容: ステージ済みのみ (porcelain の XY が "M ")。未ステージ・未追跡・削除は拒否
        if ($statusText.StartsWith('M  ')) {
            continue
        }

        throw @(
            "エラー: $relativePath がソースと同期していません",
            'UI のソース変更後はバンドルを再生成し、生成物をステージ / コミットしてください:',
            '  npm run build:ui',
            "  git add $relativePath",
            '',
            'git status:',
            $statusText
        ) -join [Environment]::NewLine
    }
}

# ESLint と Prettier 整形チェックを実行する
function Invoke-JavaScriptLintCheck {
    [CmdletBinding()]
    param()

    Invoke-NpmScript -Name 'build:ui'
    Assert-UiBundlesSynced
    Invoke-NpmScript -Name 'lint'
    Invoke-NpmScript -Name 'format:check'
}

# Prettier で JavaScript を整形する
function Invoke-JavaScriptFormat {
    [CmdletBinding()]
    param()

    Invoke-NpmScript -Name 'format'
}
