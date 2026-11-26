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

# 最後にインストールした時点の package-lock.json (なければ package.json) のハッシュを記録するファイル
$script:NpmInstallStampPath = Join-Path 'node_modules' '.clip-refiner-install-hash'

# 依存定義ファイル (package-lock.json、なければ package.json) の SHA-256 を返す
function Get-NpmDependencyHash {
    [CmdletBinding()]
    param()

    $source = if (Test-Path -LiteralPath 'package-lock.json') { 'package-lock.json' } else { 'package.json' }
    return (Get-FileHash -LiteralPath $source -Algorithm SHA256).Hash
}

# node_modules がない、または前回のインストール後に package-lock.json が変わっていれば
# npm ci で依存関係をインストールする
function Install-NpmDependenciesIfNeeded {
    [CmdletBinding()]
    param()

    Assert-NpmInstalled

    $currentHash = Get-NpmDependencyHash
    if (Test-Path -LiteralPath $script:NpmInstallStampPath) {
        $installedHash = (Get-Content -LiteralPath $script:NpmInstallStampPath -Raw).Trim()
        if ($installedHash -eq $currentHash) {
            return
        }
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

    Set-Content -LiteralPath $script:NpmInstallStampPath -Value $currentHash -NoNewline
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

# 設定 / セレクター UI バンドルがソースからの再生成結果と一致することを検証する
#
# `*.bundle.js` は `include_str!` で埋め込むためリポジトリにコミットする。
# ソースだけ更新してバンドルを忘れると配布物の UI が古いままになるため、
# 作業ツリーを書き換えずに再生成結果と比較し、差分があれば失敗させる。
# コミット済みのバンドルとソースの整合は `cargo test` (`tests/flow/ui_bundle.rs`) が Node なしで検証する
function Assert-UiBundlesSynced {
    [CmdletBinding()]
    param()

    Invoke-NpmScript -Name 'build:ui:check'
}

# ESLint・Prettier 整形チェック・TypeScript 型チェック・未使用 export の検出 (knip)・
# UI ロジックテストのカバレッジの下限の検証を実行する
function Invoke-TypeScriptLintCheck {
    [CmdletBinding()]
    param()

    Assert-UiBundlesSynced
    Invoke-NpmScript -Name 'lint'
    Invoke-NpmScript -Name 'format:check'
    Invoke-NpmScript -Name 'typecheck'
    Invoke-NpmScript -Name 'knip'
    Invoke-NpmScript -Name 'test:ui:coverage'
}

# npm 依存の既知の脆弱性を監査する (high 以上で失敗する)
#
# low / moderate は失敗にしない。現状の low は markdownlint-cli2 経由の katex (開発専用)。
# `npm audit fix --force` は markdownlint-cli2 を旧版へ戻すため使わない
function Invoke-NpmAudit {
    [CmdletBinding()]
    param()

    Invoke-NpmScript -Name 'audit:deps'
}

# markdownlint で Markdown の書式 (表の書き方など) を検証する
function Invoke-MarkdownLintCheck {
    [CmdletBinding()]
    param()

    Invoke-NpmScript -Name 'lint:md'
}

# markdownlint で Markdown の自動修正できる違反を直す
function Invoke-MarkdownLintFix {
    [CmdletBinding()]
    param()

    Invoke-NpmScript -Name 'lint:md:fix'
}

# Prettier で TypeScript などを整形する
function Invoke-TypeScriptFormat {
    [CmdletBinding()]
    param()

    Invoke-NpmScript -Name 'format'
}
