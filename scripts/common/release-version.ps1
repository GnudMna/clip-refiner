# ========================================================================
# Script Name : release-version.ps1
# Description : リリース用バージョン更新ヘルパー (ドットソース用)
# Usage       : . (Join-Path $PSScriptRoot 'release-version.ps1')
# Requires    : PowerShell 7.0+
# ========================================================================

#requires -Version 7.0

# ======================================================================
# バージョン取得・検証
# ======================================================================

# ワークスペース Cargo.toml から現在のバージョンを読み取る
function Get-WorkspaceVersion {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)]
        [string]$ProjectRoot
    )

    $cargoTomlPath = Join-Path $ProjectRoot 'Cargo.toml'
    $content = Get-Content -LiteralPath $cargoTomlPath -Raw -Encoding utf8

    if ($content -match '(?m)^version\s*=\s*"([^"]+)"') {
        return $Matches[1]
    }

    throw "エラー: Cargo.toml から version を読み取れませんでした"
}

# SemVer 文字列を検証する
function Test-SemVerVersion {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)]
        [string]$Version
    )

    return $Version -match '^\d+\.\d+\.\d+$'
}

# 指定バンプ種別で SemVer をインクリメントする
function Get-BumpedSemVer {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)]
        [string]$Version,

        [Parameter(Mandatory = $true)]
        [ValidateSet('Patch', 'Minor', 'Major')]
        [string]$Bump
    )

    if (-not (Test-SemVerVersion -Version $Version)) {
        throw "エラー: 現在のバージョンが SemVer 形式ではありません: $Version"
    }

    $parts = $Version.Split('.')
    [int]$major = $parts[0]
    [int]$minor = $parts[1]
    [int]$patch = $parts[2]

    switch ($Bump) {
        'Major' {
            return '{0}.0.0' -f ($major + 1)
        }
        'Minor' {
            return '{0}.{1}.0' -f $major, ($minor + 1)
        }
        'Patch' {
            return '{0}.{1}.{2}' -f $major, $minor, ($patch + 1)
        }
    }

    throw "エラー: バージョンのインクリメントに失敗しました"
}

# 新バージョンが現在より大きいことを検証する
function Assert-NewVersionIsGreater {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)]
        [string]$CurrentVersion,

        [Parameter(Mandatory = $true)]
        [string]$NewVersion
    )

    if (-not (Test-SemVerVersion -Version $NewVersion)) {
        throw "エラー: 新バージョンが SemVer 形式ではありません: $NewVersion"
    }

    $currentParts = $CurrentVersion.Split('.') | ForEach-Object { [int]$_ }
    $newParts = $NewVersion.Split('.') | ForEach-Object { [int]$_ }

    for ($index = 0; $index -lt 3; $index++) {
        if ($newParts[$index] -gt $currentParts[$index]) {
            return
        }

        if ($newParts[$index] -lt $currentParts[$index]) {
            throw "エラー: 新バージョン ($NewVersion) は現在 ($CurrentVersion) より大きい必要があります"
        }
    }

    throw "エラー: 新バージョン ($NewVersion) は現在 ($CurrentVersion) と同じです"
}

# ======================================================================
# ファイル更新
# ======================================================================

# ワークスペース Cargo.toml の version を更新する
function Set-WorkspaceCargoVersion {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)]
        [string]$ProjectRoot,

        [Parameter(Mandatory = $true)]
        [string]$NewVersion,

        [switch]$DryRun
    )

    $cargoTomlPath = Join-Path $ProjectRoot 'Cargo.toml'
    $content = Get-Content -LiteralPath $cargoTomlPath -Raw -Encoding utf8
    $updated = $content -replace '(?m)^(version\s*=\s*")[^"]+(")', "`${1}$NewVersion`${2}"

    if ($updated -eq $content) {
        throw "エラー: Cargo.toml の version を更新できませんでした"
    }

    if ($DryRun) {
        Write-Host "[DryRun] Cargo.toml -> version = `"$NewVersion`""
        return
    }

    Set-Content -LiteralPath $cargoTomlPath -Value $updated -Encoding utf8NoBOM -NoNewline
}

# lib.rs ドキュメント内の依存バージョン例を更新する
function Set-LibRsDocVersions {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)]
        [string]$ProjectRoot,

        [Parameter(Mandatory = $true)]
        [string]$NewVersion,

        [switch]$DryRun
    )

    $libRsPath = Join-Path $ProjectRoot 'crates\clip-refiner\src\lib.rs'
    $content = Get-Content -LiteralPath $libRsPath -Raw -Encoding utf8
    $updated = $content -replace '(?m)^(//!\s+\S+\s*=\s*\{\s*version\s*=\s*")[^"]+(")', "`${1}$NewVersion`${2}"

    if ($updated -eq $content) {
        Write-Host '注意: crates/clip-refiner/src/lib.rs のバージョン例は更新対象がありませんでした'
        return
    }

    if ($DryRun) {
        Write-Host "[DryRun] crates/clip-refiner/src/lib.rs -> ドキュメント内 version 例を `"$NewVersion`" に更新"
        return
    }

    Set-Content -LiteralPath $libRsPath -Value $updated -Encoding utf8NoBOM -NoNewline
}

# Cargo.lock の workspace パッケージ version を Cargo.toml と同期する
function Sync-WorkspaceCargoLock {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)]
        [string]$ProjectRoot,

        [switch]$DryRun
    )

    $lockPath = Join-Path $ProjectRoot 'Cargo.lock'

    if ($DryRun) {
        Write-Host '[DryRun] Cargo.lock -> workspace パッケージ version を同期'
        return
    }

    if (-not (Test-Path -LiteralPath $lockPath)) {
        throw 'エラー: Cargo.lock が見つかりません'
    }

    Push-Location -LiteralPath $ProjectRoot
    try {
        cargo check --workspace -q
        if ($LASTEXITCODE -ne 0) {
            throw "エラー: cargo check --workspace が終了コード $LASTEXITCODE で失敗しました"
        }
    } finally {
        Pop-Location
    }
}

# CHANGELOG の [Unreleased] 節をリリース見出しへ移し、フッターリンクを更新する
function Update-ChangelogForRelease {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)]
        [string]$ProjectRoot,

        [Parameter(Mandatory = $true)]
        [string]$CurrentVersion,

        [Parameter(Mandatory = $true)]
        [string]$NewVersion,

        [Parameter(Mandatory = $true)]
        [string]$ReleaseDate,

        [switch]$DryRun
    )

    $changelogPath = Join-Path $ProjectRoot 'CHANGELOG.md'
    $content = Get-Content -LiteralPath $changelogPath -Raw -Encoding utf8

    if ($content -notmatch '(?ms)^## \[Unreleased\]\r?\n(?<body>.*?)(?=^## \[)') {
        throw 'エラー: CHANGELOG.md に ## [Unreleased] 節が見つかりません'
    }

    $unreleasedBody = $Matches['body'].Trim()
    if ($unreleasedBody -notmatch '(?m)^-\s') {
        throw 'エラー: CHANGELOG.md の [Unreleased] 節に変更項目がありません (- で始まる行が必要)'
    }

    $releasedSection = "## [$NewVersion] - $ReleaseDate`r`n`r`n$unreleasedBody`r`n`r`n"
    $updated = [regex]::Replace(
        $content,
        '(?ms)^## \[Unreleased\]\r?\n.*?(?=^## \[)',
        "## [Unreleased]`r`n`r`n$releasedSection",
        1
    )

    if ($updated -notmatch '(?m)^\[Unreleased\]:') {
        throw 'エラー: CHANGELOG.md のフッター [Unreleased] リンクが見つかりません'
    }

    $compareBase = $null
    if ($updated -match '(?m)^\[Unreleased\]:\s*(?<url>.+?)/compare/') {
        $compareBase = $Matches['url']
    } else {
        throw 'エラー: CHANGELOG.md の [Unreleased] リンクから compare URL を抽出できませんでした'
    }

    $updated = [regex]::Replace(
        $updated,
        '(?m)^\[Unreleased\]:\s*.+$',
        "[Unreleased]: $compareBase/compare/v$NewVersion...develop",
        1
    )

    $newVersionLink = "[$NewVersion]: $compareBase/compare/v$CurrentVersion...v$NewVersion"
    $updated = [regex]::Replace(
        $updated,
        '(?m)^(\[Unreleased\]:.+)$',
        "`$1`r`n$newVersionLink",
        1
    )

    if ($DryRun) {
        Write-Host "[DryRun] CHANGELOG.md -> [Unreleased] を [$NewVersion] - $ReleaseDate へ移動"
        return
    }

    Set-Content -LiteralPath $changelogPath -Value $updated -Encoding utf8NoBOM -NoNewline
}

# リリースコミット対象のファイルパス一覧を返す
function Get-ReleaseVersionFilePaths {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)]
        [string]$ProjectRoot
    )

    return @(
        (Join-Path $ProjectRoot 'Cargo.toml'),
        (Join-Path $ProjectRoot 'Cargo.lock'),
        (Join-Path $ProjectRoot 'CHANGELOG.md'),
        (Join-Path $ProjectRoot 'crates\clip-refiner\src\lib.rs')
    )
}

# ======================================================================
# Git
# ======================================================================

# 作業ツリーがクリーンであることを検証する
function Assert-GitWorkingTreeClean {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)]
        [string]$ProjectRoot
    )

    Push-Location -LiteralPath $ProjectRoot
    try {
        $status = git status --porcelain
        if ($LASTEXITCODE -ne 0) {
            throw 'エラー: git status の実行に失敗しました'
        }

        if ($status) {
            throw @"
エラー: 作業ツリーに未コミットの変更があります。
リリース前にコミットするか、意図的に続行する場合は -AllowDirty を指定してください。

$status
"@
        }
    } finally {
        Pop-Location
    }
}

# 現在のブランチが release/v{version} であることを検証する
function Assert-GitReleaseBranch {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)]
        [string]$ProjectRoot,

        [Parameter(Mandatory = $true)]
        [string]$NewVersion
    )

    $expectedBranch = "release/v$NewVersion"

    Push-Location -LiteralPath $ProjectRoot
    try {
        $currentBranch = git branch --show-current
        if ($LASTEXITCODE -ne 0) {
            throw 'エラー: git branch --show-current の実行に失敗しました'
        }

        if ($currentBranch -ne $expectedBranch) {
            throw @"
エラー: リリースブランチが一致しません。
  現在のブランチ: $currentBranch
  必要なブランチ: $expectedBranch

release/v{version} ブランチを checkout してから実行してください。
"@
        }
    } finally {
        Pop-Location
    }
}

# リリース用バージョン更新をコミットする
function New-ReleaseGitCommit {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)]
        [string]$ProjectRoot,

        [Parameter(Mandatory = $true)]
        [string]$NewVersion,

        [Parameter(Mandatory = $true)]
        [string[]]$FilePaths
    )

    Push-Location -LiteralPath $ProjectRoot
    try {
        foreach ($filePath in $FilePaths) {
            if (Test-Path -LiteralPath $filePath) {
                git add -- $filePath
                if ($LASTEXITCODE -ne 0) {
                    throw "エラー: git add に失敗しました: $filePath"
                }
            }
        }

        $commitMessage = "Release: v$NewVersion"
        git commit -m $commitMessage
        if ($LASTEXITCODE -ne 0) {
            throw 'エラー: git commit に失敗しました'
        }

        Write-Host "Git コミットを作成しました: $commitMessage"
    } finally {
        Pop-Location
    }
}
