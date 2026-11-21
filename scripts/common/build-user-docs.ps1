# ========================================================================
# Script Name : build-user-docs.ps1
# Description : 利用者向けドキュメント (docs/user) を mdBook で HTML 化し、MSI 用 WiX 断片を生成する (ドットソース用)
# Usage       : . (Join-Path $PSScriptRoot 'build-user-docs.ps1')
# Requires    : PowerShell 7.0+, mdBook (版は $UserDocsMdBookVersion に固定)
# ========================================================================

#requires -Version 7.0

Set-StrictMode -Version Latest

. (Join-Path $PSScriptRoot 'assert.ps1')

# mdBook 0.5 以降は静的ファイル名にハッシュが付き、版で出力が変わるため固定する
$script:UserDocsMdBookVersion = '0.5.4'

# WiX の Component GUID を相対パスから決めるための名前空間 (UUIDv5)
$script:UserDocsGuidNamespace = [Guid]'6F1C2B8E-3D4A-4E5F-9A7B-0C1D2E3F4A5B'

# mdBook が所定の版でインストールされているか確認する
function Assert-MdBookInstalled {
    [CmdletBinding()]
    param()

    $installHint = "  cargo install mdbook --locked --version $($script:UserDocsMdBookVersion)"

    if (-not (Get-Command mdbook -ErrorAction SilentlyContinue)) {
        throw @(
            'エラー: mdbook が見つかりません',
            '',
            '以下でインストールしてください:',
            $installHint
        ) -join [Environment]::NewLine
    }

    $versionText = (& mdbook --version 2>&1 | Out-String).Trim()
    Assert-LastExitCode -CommandLabel 'mdbook --version'
    if ($versionText -notmatch '(\d+\.\d+\.\d+)' -or $Matches[1] -ne $script:UserDocsMdBookVersion) {
        throw @(
            "エラー: mdbook の版が異なります (実際: $versionText, 期待: $($script:UserDocsMdBookVersion))",
            '',
            '以下で入れ直してください:',
            "$installHint --force"
        ) -join [Environment]::NewLine
    }
}

# CHANGELOG.md を利用者向けの変更履歴ページに整える
# - 先頭の記載規約 (Keep a Changelog・節の分類表など。開発者向け) を除き、最初のバージョン見出しから始める
# - バージョン見出しの参照リンクを外す (見出し内のリンクで左側の目次が崩れるため)
# - リンクを本の中から辿れる形に書き換える
#   (docs/user/*.md → 同じ本のページ、本に含めない相対リンク → リンクを外して文字だけ残す)
# - 箇条書きの項目は、「。」の直後に文が続く箇所で行を折り返す
function ConvertTo-UserDocsChangelog {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)]
        [string]$Markdown
    )

    $title = [regex]::Match($Markdown, '(?m)^# [^\r\n]*').Value
    $firstRelease = [regex]::Match($Markdown, '(?m)^## \[')
    if (-not $title -or -not $firstRelease.Success) {
        throw 'エラー: CHANGELOG.md から見出しとバージョン節を読み取れませんでした'
    }
    $body = $title + [Environment]::NewLine + [Environment]::NewLine + $Markdown.Substring($firstRelease.Index)

    # バージョン見出しの参照リンク (`## [1.2.1]` → 比較 URL) は、見出しの中にリンクが入れ子になり、
    # 左側の目次にバージョンが出なくなる。ブラケットと、末尾の参照リンク定義を除く
    $body = [regex]::Replace($body, '(?m)^## \[([^\]]+)\]', '## $1')
    $body = [regex]::Replace($body, '(?m)^\[[^\]]+\]: \S+[ \t]*(\r?\n|$)', '')

    $pattern = '\[(?<text>[^\]]+)\]\((?<target>[^)\s]+)\)'
    $linked = [regex]::Replace($body, $pattern, {
            param($match)
            $text = $match.Groups['text'].Value
            $target = $match.Groups['target'].Value

            if ($target -match '^[a-z][a-z0-9+.-]*:' -or $target.StartsWith('#')) {
                return $match.Value
            }
            if ($target -match '^docs/user/(?<page>[^/]+\.md(#.*)?)$') {
                return "[$text]($($Matches['page']))"
            }
            if ($target -match '^CHANGELOG\.md(?<anchor>#.*)?$') {
                return "[$text](changelog.md$($Matches['anchor']))"
            }
            return $text
        })

    # 箇条書きの行だけ、「。」の直後に文が続く箇所 (閉じ括弧・引用符・行末を除く) で折り返す
    # (行末の `\` は Markdown の強制改行)
    $lines = $linked -split '\r?\n'
    for ($i = 0; $i -lt $lines.Count; $i++) {
        if ($lines[$i] -match '^(?<indent>\s*)[-*] ') {
            $continuation = $Matches['indent'] + '  '
            $lines[$i] = [regex]::Replace($lines[$i], '。(?=[^\s)）」』])', "。\`n$continuation")
        }
    }
    return $lines -join [Environment]::NewLine
}

# 出力 HTML の相対リンク (href / src) のリンク先ファイルとアンカーが存在するか検証する
function Assert-UserDocsLinks {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)]
        [string]$OutputDir
    )

    $idCache = @{}
    $getIds = {
        param([string]$Path)
        if (-not $idCache.ContainsKey($Path)) {
            $html = Get-Content -LiteralPath $Path -Raw -Encoding utf8
            $ids = [System.Collections.Generic.HashSet[string]]::new([System.StringComparer]::Ordinal)
            foreach ($m in [regex]::Matches($html, '\sid="([^"]+)"')) {
                $null = $ids.Add([System.Net.WebUtility]::HtmlDecode($m.Groups[1].Value))
            }
            $idCache[$Path] = $ids
        }
        return , $idCache[$Path]
    }

    $errors = [System.Collections.Generic.List[string]]::new()
    foreach ($page in Get-ChildItem -LiteralPath $OutputDir -Recurse -File -Filter '*.html') {
        $html = Get-Content -LiteralPath $page.FullName -Raw -Encoding utf8
        $relativePage = [System.IO.Path]::GetRelativePath($OutputDir, $page.FullName)

        foreach ($m in [regex]::Matches($html, '\s(?:href|src)="([^"]+)"')) {
            $link = [System.Net.WebUtility]::HtmlDecode($m.Groups[1].Value)
            if ($link -match '^[a-z][a-z0-9+.-]*:' -or $link.StartsWith('//')) {
                continue
            }

            $parts = $link.Split('#', 2)
            $pathPart = [Uri]::UnescapeDataString($parts[0].Split('?', 2)[0])
            $fragment = if ($parts.Count -gt 1) { [Uri]::UnescapeDataString($parts[1]) } else { '' }

            $target = if ($pathPart) {
                [System.IO.Path]::GetFullPath((Join-Path $page.DirectoryName $pathPart))
            } else {
                $page.FullName
            }

            if (-not (Test-Path -LiteralPath $target -PathType Leaf)) {
                $errors.Add("$relativePage -> $link (ファイルが無い)")
                continue
            }
            if ($fragment -and $target.EndsWith('.html') -and -not (& $getIds $target).Contains($fragment)) {
                $errors.Add("$relativePage -> $link (アンカーが無い)")
            }
        }
    }

    if ($errors.Count -gt 0) {
        $lines = @("エラー: 利用者向けドキュメントにリンク切れが $($errors.Count) 件あります") +
            @($errors | ForEach-Object { "  $_" })
        throw ($lines -join [Environment]::NewLine)
    }
}

# docs/user を mdBook でビルドし、出力ディレクトリ (target/user-docs) を返す
# CHANGELOG.md のリンクを書き換えるため、原稿を target/user-docs-src へ複製してからビルドする
function Invoke-UserDocsBuild {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)]
        [string]$ProjectRoot
    )

    Assert-MdBookInstalled

    $sourceDir = Join-Path $ProjectRoot 'docs\user'
    $stagingDir = Join-Path $ProjectRoot 'target\user-docs-src'
    $outputDir = Join-Path $ProjectRoot 'target\user-docs'

    Write-Host '利用者向けドキュメントを生成しています (mdBook)...'

    foreach ($dir in @($stagingDir, $outputDir)) {
        if (Test-Path -LiteralPath $dir) {
            Remove-Item -LiteralPath $dir -Recurse -Force
        }
    }
    $null = New-Item -ItemType Directory -Path $stagingDir -Force

    Copy-Item -Path (Join-Path $sourceDir '*') -Destination $stagingDir -Recurse -Force

    $changelog = Get-Content -LiteralPath (Join-Path $ProjectRoot 'CHANGELOG.md') -Raw -Encoding utf8
    $changelogPage = ConvertTo-UserDocsChangelog -Markdown $changelog
    Set-Content -LiteralPath (Join-Path $stagingDir 'changelog.md') -Value $changelogPage -Encoding utf8 -NoNewline

    & mdbook build $stagingDir --dest-dir $outputDir | Out-Host
    Assert-LastExitCode -CommandLabel 'mdbook build'

    # book.toml は src = "." のため出力へ複製される。404.html (<base href="/">) と .nojekyll は
    # Web 公開用で、ローカルのファイルとして開く配布物では使わない
    foreach ($name in @('book.toml', '404.html', '.nojekyll')) {
        $unused = Join-Path $outputDir $name
        if (Test-Path -LiteralPath $unused) {
            Remove-Item -LiteralPath $unused -Force
        }
    }

    Assert-UserDocsLinks -OutputDir $outputDir

    $fileCount = @(Get-ChildItem -LiteralPath $outputDir -Recurse -File).Count
    Write-Host "  出力: $outputDir ($fileCount ファイル)"

    return $outputDir
}

# 文字列から決まる UUIDv5 を返す
function New-UserDocsGuid {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)]
        [string]$Name
    )

    $namespaceBytes = $script:UserDocsGuidNamespace.ToByteArray()
    # Guid.ToByteArray は先頭 3 フィールドがリトルエンディアンのため、RFC 4122 のバイト順へ直す
    [Array]::Reverse($namespaceBytes, 0, 4)
    [Array]::Reverse($namespaceBytes, 4, 2)
    [Array]::Reverse($namespaceBytes, 6, 2)

    $nameBytes = [System.Text.Encoding]::UTF8.GetBytes($Name)
    $hash = [System.Security.Cryptography.SHA1]::HashData([byte[]]($namespaceBytes + $nameBytes))
    $bytes = $hash[0..15]
    $bytes[6] = ($bytes[6] -band 0x0F) -bor 0x50
    $bytes[8] = ($bytes[8] -band 0x3F) -bor 0x80

    [Array]::Reverse($bytes, 0, 4)
    [Array]::Reverse($bytes, 4, 2)
    [Array]::Reverse($bytes, 6, 2)
    return ([Guid]::new([byte[]]$bytes)).ToString().ToUpperInvariant()
}

# 相対パスから WiX 識別子 (72 文字以内、英数字と _ .) を作る
function Get-UserDocsWixId {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)]
        [string]$Prefix,

        [Parameter(Mandatory = $true)]
        [AllowEmptyString()]
        [string]$RelativePath
    )

    $hash = [System.Security.Cryptography.SHA1]::HashData([System.Text.Encoding]::UTF8.GetBytes($RelativePath))
    return $Prefix + [Convert]::ToHexString($hash).Substring(0, 20)
}

# 出力ディレクトリの全ファイルを DocsDir 配下へ配置する WiX 断片 (ComponentGroup UserDocs) を生成する
#
# perUser (%LOCALAPPDATA%) 構成のため、ディレクトリごとに 1 Component とし、HKCU の値を KeyPath に、
# RemoveFolder でアンインストール時にディレクトリを消す (ICE38 / ICE64)。
# Component の GUID は相対ディレクトリから決まる。mdBook の版を上げてファイル名が変わると中身が変わるが、
# main.wxs の MajorUpgrade は旧版を先にアンインストールする (afterInstallInitialize) ため問題にならない
function New-UserDocsWixFragment {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)]
        [string]$DocsDir,

        [Parameter(Mandatory = $true)]
        [string]$OutputPath
    )

    $DocsDir = (Resolve-Path -LiteralPath $DocsDir).Path
    $componentIds = [System.Collections.Generic.List[string]]::new()
    $builder = [System.Text.StringBuilder]::new()

    $escape = { param([string]$Value) [System.Security.SecurityElement]::Escape($Value) }

    $writeDirectory = $null
    $writeDirectory = {
        param([string]$Path, [string]$DirectoryId, [string]$Indent)

        $relativeDir = [System.IO.Path]::GetRelativePath($DocsDir, $Path).Replace('\', '/')
        $componentId = Get-UserDocsWixId -Prefix 'UserDocsCmp_' -RelativePath $relativeDir
        $guid = New-UserDocsGuid -Name "user-docs/$relativeDir"
        $componentIds.Add($componentId)

        $null = $builder.AppendLine("$Indent<Component Id='$componentId' Guid='$guid'>")
        foreach ($file in Get-ChildItem -LiteralPath $Path -File | Sort-Object Name) {
            $relativeFile = [System.IO.Path]::GetRelativePath($DocsDir, $file.FullName).Replace('\', '/')
            $fileId = Get-UserDocsWixId -Prefix 'UserDocsFile_' -RelativePath $relativeFile
            $null = $builder.AppendLine("$Indent    <File Id='$fileId' Name='$(& $escape $file.Name)' DiskId='1' Source='$(& $escape $file.FullName)'/>")
        }
        $removeId = Get-UserDocsWixId -Prefix 'UserDocsRm_' -RelativePath $relativeDir
        $null = $builder.AppendLine("$Indent    <RemoveFolder Id='$removeId' Directory='$DirectoryId' On='uninstall'/>")
        $null = $builder.AppendLine("$Indent    <RegistryValue Root='HKCU' Key='Software\Yuto Hirata\ClipRefiner\docs' Name='$(& $escape $relativeDir)' Type='integer' Value='1' KeyPath='yes'/>")
        $null = $builder.AppendLine("$Indent</Component>")

        foreach ($child in Get-ChildItem -LiteralPath $Path -Directory | Sort-Object Name) {
            $relativeChild = [System.IO.Path]::GetRelativePath($DocsDir, $child.FullName).Replace('\', '/')
            $childId = Get-UserDocsWixId -Prefix 'UserDocsDir_' -RelativePath $relativeChild
            $null = $builder.AppendLine("$Indent<Directory Id='$childId' Name='$(& $escape $child.Name)'>")
            & $writeDirectory $child.FullName $childId "$Indent    "
            $null = $builder.AppendLine("$Indent</Directory>")
        }
    }

    $null = $builder.AppendLine("<?xml version='1.0' encoding='utf-8'?>")
    $null = $builder.AppendLine('<!-- scripts/common/build-user-docs.ps1 が生成する。直接編集しない -->')
    $null = $builder.AppendLine("<Wix xmlns='http://schemas.microsoft.com/wix/2006/wi'>")
    $null = $builder.AppendLine('    <Fragment>')
    $null = $builder.AppendLine("        <DirectoryRef Id='DocsDir'>")
    & $writeDirectory $DocsDir 'DocsDir' '            '
    $null = $builder.AppendLine('        </DirectoryRef>')
    $null = $builder.AppendLine("        <ComponentGroup Id='UserDocs'>")
    foreach ($id in $componentIds) {
        $null = $builder.AppendLine("            <ComponentRef Id='$id'/>")
    }
    $null = $builder.AppendLine('        </ComponentGroup>')
    $null = $builder.AppendLine('    </Fragment>')
    $null = $builder.AppendLine('</Wix>')

    $null = New-Item -ItemType Directory -Path (Split-Path -Parent $OutputPath) -Force
    Set-Content -LiteralPath $OutputPath -Value $builder.ToString() -Encoding utf8 -NoNewline
}
