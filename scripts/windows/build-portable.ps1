# ========================================================================
# Script Name : build-portable.ps1
# Description : Windows 向けポータブル配布物の作成
# Usage       : ./build-portable.ps1 [-SkipBuild] [-SkipZip] [-SkipNotices]
# Requires    : PowerShell 7.0+, Rust / Cargo
#               (既定では cargo-about も必要。-SkipNotices で省略可)
#
# 前提:
#   - 実行ファイル隣の `portable` マーカーでポータブルモードになること
#     (詳細は CONFIG.md の「ポータブルモード」)
#
# 出力:
#   target/portable/clip-refiner-{version}-{host-triple}-portable/
#   target/portable/clip-refiner-{version}-{host-triple}-portable.zip
# ========================================================================

#requires -Version 7.0

[CmdletBinding()]
param(
    # リリースビルドをスキップし、既存の target/release/ClipRefiner.exe を使う
    [switch]$SkipBuild,

    # ZIP 作成をスキップし、配布フォルダのみ作成する
    [switch]$SkipZip,

    # THIRD_PARTY_NOTICES の再生成をスキップする (既存ファイルがあれば同梱)
    [switch]$SkipNotices
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

. (Join-Path $PSScriptRoot '..\common\wait-if-double-clicked.ps1')

Invoke-ScriptMain {
    . (Join-Path $PSScriptRoot '..\common\cd-project-root.ps1')
    . (Join-Path $PSScriptRoot '..\common\assert.ps1')
    . (Join-Path $PSScriptRoot '..\common\release-version.ps1')

    $ExeName = 'ClipRefiner.exe'
    $MarkerName = 'portable'
    $LicenseName = 'LICENSE'
    $NoticesName = 'THIRD_PARTY_NOTICES'
    $ReadmeName = 'PORTABLE.txt'

    function Get-RustHostTriple {
        $output = & rustc -vV 2>&1
        if ($LASTEXITCODE -ne 0) {
            throw "エラー: rustc -vV が終了コード $LASTEXITCODE で失敗しました"
        }

        foreach ($line in $output) {
            if ($line -match '^host:\s+(.+)$') {
                return $Matches[1].Trim()
            }
        }

        throw 'エラー: rustc -vV から host triple を読み取れませんでした'
    }

    # Cargo の target ディレクトリを解決する
    #
    # `cargo metadata` の JSON はコンソールコードページ次第で日本語パスが化けるため使わない。
    # `$ProjectRoot` (ファイルシステム由来) と環境変数だけを使う。
    function Get-CargoTargetDirectory {
        if (-not [string]::IsNullOrWhiteSpace($env:CARGO_TARGET_DIR)) {
            return [System.IO.Path]::GetFullPath($env:CARGO_TARGET_DIR)
        }
        return Join-Path $ProjectRoot 'target'
    }

    # リリース済み ClipRefiner.exe のパスを解決する
    function Resolve-ReleaseExecutable {
        [CmdletBinding()]
        param(
            [Parameter(Mandatory = $true)]
            [string]$TargetDirectory,

            [Parameter(Mandatory = $true)]
            [string]$FileName
        )

        $candidates = [System.Collections.Generic.List[string]]::new()
        $candidates.Add((Join-Path $TargetDirectory "release\$FileName"))
        $candidates.Add((Join-Path $TargetDirectory "release\deps\$FileName"))

        try {
            $hostTriple = Get-RustHostTriple
            $candidates.Add((Join-Path $TargetDirectory "$hostTriple\release\$FileName"))
            $candidates.Add((Join-Path $TargetDirectory "$hostTriple\release\deps\$FileName"))
        } catch {
            # host triple が取れなくても候補探索は継続する
        }

        foreach ($candidate in $candidates) {
            if (Test-Path -LiteralPath $candidate) {
                return (Get-Item -LiteralPath $candidate).FullName
            }
        }

        $releaseDir = Join-Path $TargetDirectory 'release'
        if (Test-Path -LiteralPath $releaseDir) {
            $found = Get-ChildItem -LiteralPath $releaseDir -Filter $FileName -File -Recurse -ErrorAction SilentlyContinue |
                Sort-Object FullName |
                Select-Object -First 1
            if ($found) {
                return $found.FullName
            }
        }

        $checked = ($candidates | ForEach-Object { "  - $_" }) -join [Environment]::NewLine
        throw @"
エラー: 実行ファイル ($FileName) が見つかりません。
探索した target ディレクトリ: $TargetDirectory
確認した候補:
$checked

先にリリースビルドを実行するか、-SkipBuild を外してください。
CARGO_TARGET_DIR を使っている場合は、ビルド時と同じ値がこのシェルにも設定されているか確認してください。
"@
    }

    # パスを削除する。失敗時は利用中プロセスの可能性を案内する
    function Remove-PathIfExists {
        [CmdletBinding()]
        param(
            [Parameter(Mandatory = $true)]
            [string]$Path,

            [Parameter(Mandatory = $true)]
            [string]$Label
        )

        if (-not (Test-Path -LiteralPath $Path)) {
            return
        }

        try {
            Remove-Item -LiteralPath $Path -Recurse -Force -ErrorAction Stop
        } catch {
            throw @"
エラー: $Label を削除できませんでした: $Path
$($_.Exception.Message)

ClipRefiner.exe が起動中、またはエクスプローラー / 別プロセスがファイルを開いている可能性があります。
終了してから再実行してください。
"@
        }
    }

    # 配布フォルダを ZIP 化する
    function New-PortableZipArchive {
        [CmdletBinding()]
        param(
            [Parameter(Mandatory = $true)]
            [string]$SourceDirectory,

            [Parameter(Mandatory = $true)]
            [string]$DestinationZip
        )

        Add-Type -AssemblyName System.IO.Compression.FileSystem

        $tempZip = "$DestinationZip.tmp"
        Remove-PathIfExists -Path $tempZip -Label '一時 ZIP'
        Remove-PathIfExists -Path $DestinationZip -Label '既存 ZIP'

        try {
            [System.IO.Compression.ZipFile]::CreateFromDirectory(
                $SourceDirectory,
                $tempZip,
                [System.IO.Compression.CompressionLevel]::Optimal,
                $true
            )
            Move-Item -LiteralPath $tempZip -Destination $DestinationZip -Force
        } catch {
            Remove-PathIfExists -Path $tempZip -Label '一時 ZIP'
            throw @"
エラー: ZIP の作成に失敗しました: $DestinationZip
$($_.Exception.Message)

エクスプローラーで ZIP を開いていないか確認し、再実行してください。
"@
        }
    }

    Write-Host 'ポータブル配布物の作成を開始します...'
    Write-Host ''

    $cargoTargetDir = Get-CargoTargetDirectory
    Write-Host "Cargo target ディレクトリ: $cargoTargetDir"
    Write-Host ''

    if (-not $SkipBuild) {
        Write-Host 'リリースビルドを実行中...'
        cargo build -p clip-refiner --bins --release
        Assert-LastExitCode -CommandLabel 'cargo build -p clip-refiner --bins --release'
        Write-Host ''
    } else {
        Write-Host 'リリースビルドをスキップしました (-SkipBuild)'
        Write-Host ''
    }

    $exeSource = Resolve-ReleaseExecutable -TargetDirectory $cargoTargetDir -FileName $ExeName
    Write-Host "使用する実行ファイル: $exeSource"
    Write-Host ''

    $licenseSource = Join-Path $ProjectRoot $LicenseName
    if (-not (Test-Path -LiteralPath $licenseSource)) {
        throw "エラー: $LicenseName が見つかりません: $licenseSource"
    }

    $noticesSource = Join-Path $ProjectRoot $NoticesName
    if (-not $SkipNotices) {
        . (Join-Path $PSScriptRoot '..\common\generate-third-party-notices.ps1')
        Write-Host ''
        if (-not (Test-Path -LiteralPath $noticesSource)) {
            throw "エラー: $NoticesName の生成後もファイルが見つかりません: $noticesSource"
        }
    } elseif (-not (Test-Path -LiteralPath $noticesSource)) {
        Write-Host "警告: $NoticesName が無いため同梱をスキップします (-SkipNotices)"
        Write-Host ''
        $noticesSource = $null
    }

    $version = Get-WorkspaceVersion -ProjectRoot $ProjectRoot
    $hostTriple = Get-RustHostTriple
    $packageName = "clip-refiner-$version-$hostTriple-portable"
    $portableRoot = Join-Path $ProjectRoot 'target\portable'
    $stagingDir = Join-Path $portableRoot $packageName
    $stagingWorkDir = Join-Path $portableRoot "$packageName.building"
    $zipPath = Join-Path $portableRoot "$packageName.zip"

    New-Item -ItemType Directory -Path $portableRoot -Force | Out-Null
    Remove-PathIfExists -Path $stagingWorkDir -Label '作業用配布フォルダ'

    New-Item -ItemType Directory -Path $stagingWorkDir -Force | Out-Null

    Copy-Item -LiteralPath $exeSource -Destination (Join-Path $stagingWorkDir $ExeName) -Force
    Copy-Item -LiteralPath $licenseSource -Destination (Join-Path $stagingWorkDir $LicenseName) -Force
    if ($noticesSource) {
        Copy-Item -LiteralPath $noticesSource -Destination (Join-Path $stagingWorkDir $NoticesName) -Force
    }

    # ポータブルモード有効化マーカー (中身は空でよい)
    $markerPath = Join-Path $stagingWorkDir $MarkerName
    New-Item -ItemType File -Path $markerPath -Force | Out-Null

    $portableReadme = @"
ClipRefiner ポータブル版
========================

このフォルダには ``portable`` マーカーが含まれます。
起動すると設定・ログ・登録クリップは同じフォルダ内の ``data\`` に保存されます。

必要なもの:
- Windows
- Microsoft Edge WebView2 Runtime (設定ウィンドウ / セレクター用)

使い方:
1. このフォルダを任意の場所 (USB メモリなど) へ置く
2. ClipRefiner.exe を起動する (初回起動で data\ が作成される)
3. 通知領域のアイコンから操作する

注意:
- ログイン時の自動起動はレジストリに絶対パスを書くため、USB 用途には向きません
- 登録クリップの鍵は Windows DPAPI で保護されるため、別ユーザー / 別 PC では復号できない場合があります
- 詳細はプロジェクトの CONFIG.md (ポータブルモード) を参照してください
"@
    Set-Content -LiteralPath (Join-Path $stagingWorkDir $ReadmeName) -Value $portableReadme.TrimEnd() -Encoding utf8

    $requiredNames = [System.Collections.Generic.List[string]]::new()
    $requiredNames.AddRange([string[]]@($ExeName, $MarkerName, $LicenseName, $ReadmeName))
    if ($noticesSource) {
        $requiredNames.Add($NoticesName)
    }
    foreach ($name in $requiredNames) {
        $path = Join-Path $stagingWorkDir $name
        if (-not (Test-Path -LiteralPath $path)) {
            throw "エラー: 配布物に必要なファイルがありません: $path"
        }
    }

    $markerItem = Get-Item -LiteralPath (Join-Path $stagingWorkDir $MarkerName)
    if ($markerItem.PSIsContainer) {
        throw "エラー: $MarkerName がディレクトリになっています (ファイルである必要があります)"
    }

    # 完成した作業フォルダを最終配置へ入れ替える (起動中 exe があるとここで失敗する)
    try {
        Remove-PathIfExists -Path $stagingDir -Label '既存の配布フォルダ'
        Move-Item -LiteralPath $stagingWorkDir -Destination $stagingDir -Force -ErrorAction Stop
    } catch {
        throw @"
$($_.Exception.Message)

新しい配布物は一時フォルダにあります: $stagingWorkDir
既存フォルダ内の ClipRefiner.exe を終了してから再実行するか、一時フォルダを利用してください。
"@
    }

    Write-Host '配布フォルダを作成しました:'
    Write-Host "  $stagingDir"
    Write-Host ''

    if (-not $SkipZip) {
        Write-Host 'ZIP を作成しています...'
        New-PortableZipArchive -SourceDirectory $stagingDir -DestinationZip $zipPath
        Write-Host "作成しました: $zipPath"
        Write-Host ''
    } else {
        Write-Host 'ZIP 作成をスキップしました (-SkipZip)'
        Write-Host ''
    }

    Write-Host 'ポータブル配布物の作成が完了しました'
    Write-Host "  フォルダ: $stagingDir"
    if (-not $SkipZip) {
        Write-Host "  ZIP:     $zipPath"
    }
}
