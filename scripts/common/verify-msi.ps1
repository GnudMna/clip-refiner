# ========================================================================
# Script Name : verify-msi.ps1
# Description : ビルド済み MSI の内容・メタデータを検証する (ドットソース用)
# Usage       : . (Join-Path $PSScriptRoot 'verify-msi.ps1')
# Requires    : PowerShell 7.0+, Windows (Windows Installer COM)
# ========================================================================

#requires -Version 7.0

Set-StrictMode -Version Latest

# ======================================================================
# 定数
# ======================================================================

$script:MsiExpectedProductName = 'ClipRefiner'
$script:MsiExpectedManufacturer = 'Yuto Hirata'
$script:MsiExpectedLanguageId = '1041'
$script:MsiRequiredFileNames = @(
    'ClipRefiner.exe',
    'LICENSE',
    'THIRD_PARTY_NOTICES',
    'index.html'
)
# 利用者向けドキュメント (docs\) の各ページ。docs/user/SUMMARY.md の章に対応する
$script:MsiRequiredDocPages = @(
    'index.html',
    'introduction.html',
    'usage.html',
    'configuration.html',
    'modes.html',
    'security.html',
    'troubleshooting.html',
    'changelog.html'
)
$script:MsiRequiredFeatureIds = @(
    'Binaries',
    'Environment',
    'Autostart'
)
$script:MsiThirdPartyNoticesHeader = '第三者ソフトウェアに関する表示'
$script:MsiMinExecutableBytes = 512 * 1024
$script:MsiMinThirdPartyNoticesBytes = 1024

# ======================================================================
# メタデータ読み取り
# ======================================================================

# clip-refiner の Cargo.toml から upgrade-guid を読み取る
function Get-ClipRefinerUpgradeGuid {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)]
        [string]$ProjectRoot
    )

    $manifestPath = Join-Path $ProjectRoot 'crates\clip-refiner\Cargo.toml'
    $content = Get-Content -LiteralPath $manifestPath -Raw -Encoding utf8

    if ($content -match '(?m)^upgrade-guid\s*=\s*"([^"]+)"') {
        return $Matches[1].ToUpperInvariant()
    }

    throw "エラー: crates/clip-refiner/Cargo.toml から upgrade-guid を読み取れませんでした"
}

# wix/main.wxs から UpgradeCode を読み取る
function Get-WixUpgradeCode {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)]
        [string]$ProjectRoot
    )

    $wxsPath = Join-Path $ProjectRoot 'wix\main.wxs'
    $content = Get-Content -LiteralPath $wxsPath -Raw -Encoding utf8

    if ($content -match "UpgradeCode='([^']+)'") {
        return $Matches[1].ToUpperInvariant()
    }

    throw 'エラー: wix/main.wxs から UpgradeCode を読み取れませんでした'
}

. (Join-Path $PSScriptRoot 'windows-target.ps1')

# 期待する MSI パスを解決する
function Resolve-BuiltMsiPath {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)]
        [string]$ProjectRoot,

        [Parameter(Mandatory = $true)]
        [string]$Version,

        [string]$MsiPath,

        [string]$TargetTriple
    )

    if ($MsiPath) {
        if (-not (Test-Path -LiteralPath $MsiPath)) {
            throw "エラー: 指定した MSI が見つかりません: $MsiPath"
        }

        return (Resolve-Path -LiteralPath $MsiPath).Path
    }

    $wixDir = Join-Path $ProjectRoot 'target\wix'
    if (-not (Test-Path -LiteralPath $wixDir)) {
        throw "エラー: target\wix が見つかりません。先に build-msi.ps1 を実行してください"
    }

    $escapedVersion = [regex]::Escape($Version)
    $candidates = @(Get-ChildItem -LiteralPath $wixDir -Filter 'clip-refiner-*.msi' |
        Where-Object { $_.Name -match "^clip-refiner-$escapedVersion-" })

    if ($candidates.Count -eq 0) {
        throw "エラー: バージョン $Version の MSI が見つかりません ($wixDir)"
    }

    $preferredTriple = if ($TargetTriple) {
        $TargetTriple
    } else {
        Get-RustHostTriple
    }

    # cargo-wix の既定名は `{name}-{version}-{arch}.msi` (例: x86_64 / aarch64)
    # フル triple (`x86_64-pc-windows-msvc`) では -Target All 時に 0 件になり、複数 MSI で失敗する
    $preferred = @($candidates | Where-Object {
        (Resolve-WindowsTargetTripleFromMsiName -MsiFileName $_.Name) -eq $preferredTriple
    })
    if ($preferred.Count -eq 1) {
        return $preferred[0].FullName
    }
    if ($preferred.Count -gt 1) {
        $names = ($preferred | ForEach-Object { $_.Name }) -join ', '
        throw "エラー: バージョン $Version ($preferredTriple) に一致する MSI が複数あります: $names"
    }

    if ($TargetTriple) {
        $names = ($candidates | ForEach-Object { $_.Name }) -join ', '
        throw "エラー: バージョン $Version の $TargetTriple 向け MSI が見つかりません: $names"
    }

    if ($candidates.Count -eq 1) {
        return $candidates[0].FullName
    }

    $names = ($candidates | ForEach-Object { $_.Name }) -join ', '
    throw "エラー: バージョン $Version に一致する MSI が複数あります: $names"
}

# ======================================================================
# Windows Installer データベース
# ======================================================================

# MSI データベースを読み取り専用で開く
function Open-MsiDatabase {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)]
        [string]$Path
    )

    $installer = New-Object -ComObject WindowsInstaller.Installer
    return $installer.OpenDatabase($Path, 0)
}

# MSI クエリを実行し、レコード一覧を返す
function Invoke-MsiQuery {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)]
        $Database,

        [Parameter(Mandatory = $true)]
        [string]$Sql
    )

    $records = [System.Collections.ArrayList]::new()
    $view = $Database.OpenView($Sql)

    try {
        [void]$view.Execute()

        while ($true) {
            try {
                $record = $view.Fetch()
            } catch [System.Runtime.InteropServices.COMException] {
                break
            }

            if ($null -eq $record) {
                break
            }

            $null = $records.Add($record)
        }
    } finally {
        [void]$view.Close()
    }

    return [object[]]$records.ToArray()
}

# GUID 文字列を比較用に正規化する
function Get-NormalizedGuid {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)]
        [string]$Value
    )

    return $Value.Trim().Trim('{', '}').ToUpperInvariant()
}

# MSI 文字列プロパティを正規化する
function Get-NormalizedMsiString {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)]
        [AllowNull()]
        [AllowEmptyString()]
        [string]$Value
    )

    if ([string]::IsNullOrEmpty($Value)) {
        return [string]::Empty
    }

    return $Value.Replace([string][char]0, [string]::Empty).Trim()
}

function Get-MsiPropertyValue {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)]
        $Database,

        [Parameter(Mandatory = $true)]
        [string]$PropertyName
    )

    $escapedName = $PropertyName.Replace("'", "''")
    $sql = "SELECT Value FROM Property WHERE Property = '$escapedName'"
    $view = $Database.OpenView($sql)

    try {
        [void]$view.Execute()

        try {
            $record = $view.Fetch()
        } catch [System.Runtime.InteropServices.COMException] {
            return [string]::Empty
        }

        if ($null -eq $record) {
            return [string]::Empty
        }

        $rawValue = [string]$record.StringData(1)
        return ,$(Get-NormalizedMsiString -Value $rawValue)
    } finally {
        [void]$view.Close()
    }
}

# MSI File テーブルの FileName を表示名に正規化する
function Get-MsiFileDisplayName {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)]
        [string]$FileName
    )

    $normalized = Get-NormalizedMsiString -Value $FileName
    if ($normalized.Contains('|')) {
        return $normalized.Split('|')[-1]
    }

    return $normalized
}

# File テーブルのファイル名一覧を取得する
function Get-MsiFileNames {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)]
        $Database
    )

    $records = Invoke-MsiQuery -Database $Database -Sql 'SELECT FileName FROM File'
    $names = [System.Collections.Generic.HashSet[string]]::new([System.StringComparer]::OrdinalIgnoreCase)

    foreach ($record in $records) {
        if ($null -ne $record) {
            $displayName = Get-MsiFileDisplayName -FileName ([string]$record.StringData(1))
            $null = $names.Add($displayName)
        }
    }

    return $names
}

# Feature テーブルの Feature ID 一覧を取得する
function Get-MsiFeatureIds {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)]
        $Database
    )

    $records = Invoke-MsiQuery -Database $Database -Sql 'SELECT Feature FROM Feature'
    $ids = [System.Collections.Generic.HashSet[string]]::new([System.StringComparer]::OrdinalIgnoreCase)

    foreach ($record in $records) {
        if ($null -ne $record) {
            $null = $ids.Add((Get-NormalizedMsiString -Value ([string]$record.StringData(1))))
        }
    }

    return $ids
}

# ======================================================================
# 検証
# ======================================================================

# Cargo.toml と wix/main.wxs の UpgradeCode 一致を検証する
function Assert-WixMetadataConsistency {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)]
        [string]$ProjectRoot
    )

    $cargoGuid = Get-ClipRefinerUpgradeGuid -ProjectRoot $ProjectRoot
    $wixGuid = Get-WixUpgradeCode -ProjectRoot $ProjectRoot

    if ($cargoGuid -ne $wixGuid) {
        throw @(
            'エラー: UpgradeCode が一致しません',
            "  Cargo.toml upgrade-guid: $cargoGuid",
            "  wix/main.wxs UpgradeCode: $wixGuid"
        ) -join [Environment]::NewLine
    }
}

# MSI の Property / File / Feature を検証する
function Assert-MsiDatabaseContents {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)]
        [string]$MsiPath,

        [Parameter(Mandatory = $true)]
        [string]$ExpectedVersion,

        [Parameter(Mandatory = $true)]
        [string]$ExpectedUpgradeCode,

        [string]$TargetTriple
    )

    $database = Open-MsiDatabase -Path $MsiPath

    try {
        $productName = Get-MsiPropertyValue -Database $database -PropertyName 'ProductName'
        if ([string]::IsNullOrWhiteSpace($productName)) {
            throw 'エラー: MSI の ProductName が空です'
        }

        if ($productName -cne $script:MsiExpectedProductName) {
            throw "エラー: ProductName が期待値と異なります (実際: $productName, 期待: $($script:MsiExpectedProductName))"
        }

        $productVersion = Get-MsiPropertyValue -Database $database -PropertyName 'ProductVersion'
        if ($productVersion -ne $ExpectedVersion) {
            throw "エラー: ProductVersion が期待値と異なります (実際: $productVersion, 期待: $ExpectedVersion)"
        }

        $upgradeCode = Get-MsiPropertyValue -Database $database -PropertyName 'UpgradeCode'
        if (-not $upgradeCode) {
            throw 'エラー: MSI の Property テーブルに UpgradeCode がありません'
        }

        $normalizedUpgradeCode = Get-NormalizedGuid -Value $upgradeCode
        $normalizedExpectedUpgradeCode = Get-NormalizedGuid -Value $ExpectedUpgradeCode
        if ($normalizedUpgradeCode -ne $normalizedExpectedUpgradeCode) {
            throw "エラー: UpgradeCode が期待値と異なります (実際: $upgradeCode, 期待: $ExpectedUpgradeCode)"
        }

        $manufacturer = Get-MsiPropertyValue -Database $database -PropertyName 'Manufacturer'
        if ($manufacturer -ne $script:MsiExpectedManufacturer) {
            throw "エラー: Manufacturer が期待値と異なります (実際: $manufacturer, 期待: $($script:MsiExpectedManufacturer))"
        }

        $allUsers = Get-MsiPropertyValue -Database $database -PropertyName 'ALLUSERS'
        if ($allUsers -and $allUsers -ne '2') {
            throw "エラー: per-user インストールを想定していますが ALLUSERS=$allUsers です (期待: 未設定または 2)"
        }

        $fileNames = Get-MsiFileNames -Database $database
        foreach ($requiredName in $script:MsiRequiredFileNames) {
            if (-not $fileNames.Contains($requiredName)) {
                throw "エラー: MSI の File テーブルに $requiredName が含まれていません"
            }
        }

        $featureIds = Get-MsiFeatureIds -Database $database
        foreach ($requiredFeature in $script:MsiRequiredFeatureIds) {
            if (-not $featureIds.Contains($requiredFeature)) {
                throw "エラー: MSI の Feature テーブルに $requiredFeature が含まれていません"
            }
        }
    } finally {
        [System.Runtime.InteropServices.Marshal]::ReleaseComObject($database) | Out-Null
    }

    if ($TargetTriple) {
        Assert-MsiPlatformTemplate -MsiPath $MsiPath -TargetTriple $TargetTriple
    }
}

# MSI を管理インストールで展開する
function Expand-MsiAdministrative {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)]
        [string]$MsiPath,

        [Parameter(Mandatory = $true)]
        [string]$TargetDir
    )

    if (Test-Path -LiteralPath $TargetDir) {
        Remove-Item -LiteralPath $TargetDir -Recurse -Force
    }

    $null = New-Item -ItemType Directory -Path $TargetDir -Force

    $arguments = @(
        '/a',
        $MsiPath,
        '/qn',
        "TARGETDIR=$TargetDir"
    )

    $process = Start-Process -FilePath 'msiexec.exe' -ArgumentList $arguments -Wait -PassThru -NoNewWindow
    if ($process.ExitCode -ne 0) {
        throw "エラー: msiexec /a が終了コード $($process.ExitCode) で失敗しました"
    }
}

# 展開した MSI 内容を検証する
function Assert-ExtractedMsiContents {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)]
        [string]$ExtractDir,

        [Parameter(Mandatory = $true)]
        [string]$ProjectRoot,

        [string]$TargetTriple
    )

    foreach ($requiredName in $script:MsiRequiredFileNames) {
        $matches = @(Get-ChildItem -LiteralPath $ExtractDir -Recurse -File -Filter $requiredName -ErrorAction SilentlyContinue)
        if ($matches.Count -eq 0) {
            throw "エラー: 展開結果に $requiredName が見つかりません ($ExtractDir)"
        }
    }

    $executable = @(Get-ChildItem -LiteralPath $ExtractDir -Recurse -File -Filter 'ClipRefiner.exe')[0]
    if ($executable.Length -lt $script:MsiMinExecutableBytes) {
        throw "エラー: ClipRefiner.exe のサイズが小さすぎます ($($executable.Length) bytes, 最低 $($script:MsiMinExecutableBytes) bytes)"
    }

    if ($TargetTriple) {
        Assert-PeMachineType -Path $executable.FullName -TargetTriple $TargetTriple
    }

    $noticesFile = @(Get-ChildItem -LiteralPath $ExtractDir -Recurse -File -Filter 'THIRD_PARTY_NOTICES')[0]
    if ($noticesFile.Length -lt $script:MsiMinThirdPartyNoticesBytes) {
        throw "エラー: THIRD_PARTY_NOTICES のサイズが小さすぎます ($($noticesFile.Length) bytes)"
    }

    $noticesContent = Get-Content -LiteralPath $noticesFile.FullName -Raw -Encoding utf8
    if ($noticesContent -notmatch [regex]::Escape($script:MsiThirdPartyNoticesHeader)) {
        throw 'エラー: THIRD_PARTY_NOTICES に期待するヘッダーが含まれていません'
    }

    $docsDirs = @(Get-ChildItem -LiteralPath $ExtractDir -Recurse -Directory -Filter 'docs' |
        Where-Object { Test-Path -LiteralPath (Join-Path $_.FullName 'index.html') })
    if ($docsDirs.Count -ne 1) {
        throw "エラー: 展開結果に利用者向けドキュメント (docs\index.html) が見つかりません ($ExtractDir)"
    }
    foreach ($page in $script:MsiRequiredDocPages) {
        if (-not (Test-Path -LiteralPath (Join-Path $docsDirs[0].FullName $page) -PathType Leaf)) {
            throw "エラー: 利用者向けドキュメントに $page がありません ($($docsDirs[0].FullName))"
        }
    }

    $sourceNoticesPath = Join-Path $ProjectRoot 'THIRD_PARTY_NOTICES'
    if (Test-Path -LiteralPath $sourceNoticesPath) {
        $sourceInfo = Get-Item -LiteralPath $sourceNoticesPath
        if ($sourceInfo.Length -ne $noticesFile.Length) {
            throw @(
                'エラー: MSI 内の THIRD_PARTY_NOTICES サイズがビルド時生成物と一致しません',
                "  生成物: $($sourceInfo.Length) bytes",
                "  MSI 内: $($noticesFile.Length) bytes"
            ) -join [Environment]::NewLine
        }
    }
}

# ビルド済み MSI を一括検証する
function Invoke-MsiPackageVerification {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)]
        [string]$ProjectRoot,

        [string]$MsiPath,

        [string]$Version,

        [string]$TargetTriple,

        [switch]$SkipExtract
    )

    if (-not $Version) {
        . (Join-Path $PSScriptRoot 'release-version.ps1')
        $Version = Get-WorkspaceVersion -ProjectRoot $ProjectRoot
    }

    Write-Host 'MSI パッケージを検証しています...'
    Write-Host "  対象バージョン: $Version"
    Write-Host ''

    Write-Host 'WiX メタデータの整合性を確認中...'
    Assert-WixMetadataConsistency -ProjectRoot $ProjectRoot
    $expectedUpgradeCode = Get-ClipRefinerUpgradeGuid -ProjectRoot $ProjectRoot
    Write-Host '  Cargo.toml と wix/main.wxs の UpgradeCode が一致'
    Write-Host ''

    $resolvedMsiPath = Resolve-BuiltMsiPath `
        -ProjectRoot $ProjectRoot `
        -Version $Version `
        -MsiPath $MsiPath `
        -TargetTriple $TargetTriple
    if (-not $TargetTriple) {
        $TargetTriple = Resolve-WindowsTargetTripleFromMsiName -MsiFileName $resolvedMsiPath
    }
    Write-Host "検証対象 MSI: $resolvedMsiPath"
    if ($TargetTriple) {
        Write-Host "  ターゲット: $TargetTriple"
    }
    Write-Host ''

    Write-Host 'MSI データベースを確認中...'
    Assert-MsiDatabaseContents `
        -MsiPath $resolvedMsiPath `
        -ExpectedVersion $Version `
        -ExpectedUpgradeCode $expectedUpgradeCode `
        -TargetTriple $TargetTriple
    Write-Host '  ProductName / ProductVersion / UpgradeCode / Manufacturer'
    Write-Host '  必須ファイル (ClipRefiner.exe, LICENSE, THIRD_PARTY_NOTICES, docs\index.html)'
    Write-Host '  必須 Feature (Binaries, Environment, Autostart)'
    Write-Host '  per-user インストール (ALLUSERS)'
    if ($TargetTriple) {
        Write-Host "  Platform テンプレート ($TargetTriple)"
    }
    Write-Host ''

    if ($SkipExtract) {
        Write-Host '展開検証をスキップしました (-SkipExtract)'
        Write-Host ''
        Write-Host 'MSI 検証が完了しました'
        return
    }

    $extractDir = Join-Path ([System.IO.Path]::GetTempPath()) ("clip-refiner-msi-verify-{0}" -f [Guid]::NewGuid().ToString())
    try {
        Write-Host 'MSI を管理インストールで展開中...'
        Expand-MsiAdministrative -MsiPath $resolvedMsiPath -TargetDir $extractDir
        Write-Host "  展開先: $extractDir"
        Write-Host ''

        Write-Host '展開内容を確認中...'
        Assert-ExtractedMsiContents `
            -ExtractDir $extractDir `
            -ProjectRoot $ProjectRoot `
            -TargetTriple $TargetTriple
        Write-Host '  実行ファイル・ライセンス・THIRD_PARTY_NOTICES の存在とサイズ'
        Write-Host '  THIRD_PARTY_NOTICES ヘッダーと生成物サイズの一致'
        Write-Host '  利用者向けドキュメント (docs\) の各ページ'
        if ($TargetTriple) {
            Write-Host "  ClipRefiner.exe の PE アーキテクチャ ($TargetTriple)"
        }
        Write-Host ''
    } finally {
        if (Test-Path -LiteralPath $extractDir) {
            Remove-Item -LiteralPath $extractDir -Recurse -Force -ErrorAction SilentlyContinue
        }
    }

    Write-Host 'MSI 検証が完了しました'
}
