# ========================================================================
# Script Name : windows-target.ps1
# Description : Windows 配布ターゲット (x64 / ARM64) の解決と PE 検証
# Usage       : . (Join-Path $PSScriptRoot 'windows-target.ps1')
# Requires    : PowerShell 7.0+, Rust / Cargo
# ========================================================================

#requires -Version 7.0

Set-StrictMode -Version Latest

$script:WindowsX64Triple = 'x86_64-pc-windows-msvc'
$script:WindowsArm64Triple = 'aarch64-pc-windows-msvc'
$script:WindowsMsvcOriginalPath = $null

# rustc の host triple を取得する
function Get-RustHostTriple {
    [CmdletBinding()]
    param()

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

# サポートする Windows 配布ターゲット triple の一覧を返す
function Get-WindowsSupportedTargetTriples {
    [CmdletBinding()]
    param()

    return @($script:WindowsX64Triple, $script:WindowsArm64Triple)
}

# ターゲット triple が x64 / ARM64 の配布対象か検証する
function Assert-WindowsSupportedTargetTriple {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)]
        [string]$TargetTriple
    )

    $supported = Get-WindowsSupportedTargetTriples
    if ($supported -notcontains $TargetTriple) {
        $list = $supported -join ', '
        throw "エラー: 未対応のターゲットです: $TargetTriple (対応: $list)"
    }
}

# エイリアスまたは triple を正規化する (All / Host はここでは扱わない)
function Resolve-WindowsTargetTripleAlias {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)]
        [string]$Value
    )

    switch -Regex ($Value.Trim()) {
        '^(?i)(x64|x86_64|amd64|x86_64-pc-windows-msvc)$' { return $script:WindowsX64Triple }
        '^(?i)(arm64|aarch64|aarch64-pc-windows-msvc)$' { return $script:WindowsArm64Triple }
        default {
            throw @(
                "エラー: 未知のターゲットです: $Value",
                '指定できる値: Host, All, x64, arm64, x86_64-pc-windows-msvc, aarch64-pc-windows-msvc'
            ) -join [Environment]::NewLine
        }
    }
}

# -Target 指定を配布用 triple の配列へ展開する
#
# 省略 / Host: 実行中ホスト (x64 または ARM64)
# All: x64 と ARM64 の両方
function Resolve-WindowsPackageTargets {
    [CmdletBinding()]
    param(
        [string]$Target
    )

    if ([string]::IsNullOrWhiteSpace($Target) -or $Target -match '^(?i)host$') {
        $hostTriple = Get-RustHostTriple
        Assert-WindowsSupportedTargetTriple -TargetTriple $hostTriple
        return @($hostTriple)
    }

    if ($Target -match '^(?i)(all|both)$') {
        return @(Get-WindowsSupportedTargetTriples)
    }

    $triple = Resolve-WindowsTargetTripleAlias -Value $Target
    return @($triple)
}

# rustup にターゲットが無ければ追加する
function Assert-RustTargetInstalled {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)]
        [string]$TargetTriple
    )

    Assert-WindowsSupportedTargetTriple -TargetTriple $TargetTriple

    $installed = & rustup target list --installed 2>&1
    if ($LASTEXITCODE -ne 0) {
        throw "エラー: rustup target list --installed が終了コード $LASTEXITCODE で失敗しました"
    }

    $found = $installed | Where-Object { $_ -eq $TargetTriple }
    if ($found) {
        return
    }

    Write-Host "Rust ターゲットを追加しています: $TargetTriple"
    rustup target add $TargetTriple
    if ($LASTEXITCODE -ne 0) {
        throw "エラー: rustup target add $TargetTriple が終了コード $LASTEXITCODE で失敗しました"
    }
}

# vswhere のパスを返す
function Get-VsWherePath {
    [CmdletBinding()]
    param()

    $candidates = @(
        (Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'),
        (Join-Path $env:ProgramFiles 'Microsoft Visual Studio\Installer\vswhere.exe')
    )
    foreach ($candidate in $candidates) {
        if ($candidate -and (Test-Path -LiteralPath $candidate)) {
            return $candidate
        }
    }

    return $null
}

# OS のネイティブ CPU を返す (x64 プロセスが ARM64 上でエミュレーションされていても ARM64)
function Get-WindowsNativeArchitecture {
    [CmdletBinding()]
    param()

    switch ([System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture) {
        'X64' { return 'x64' }
        'Arm64' { return 'arm64' }
        'X86' { return 'x86' }
        default {
            throw "エラー: 未対応の OS アーキテクチャです: $([System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture)"
        }
    }
}

# ターゲット triple に対応する VS コンポーネント ID を返す
function Get-MsvcVcToolsComponentId {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)]
        [string]$TargetTriple
    )

    if ($TargetTriple.StartsWith('aarch64-')) {
        return 'Microsoft.VisualStudio.Component.VC.Tools.ARM64'
    }

    return 'Microsoft.VisualStudio.Component.VC.Tools.x86.x64'
}

# ホスト OS / ターゲットに対応する vcvarsall.bat のアーキテクチャ引数を返す
#
# rustc の host triple は使わない
# ARM64 Windows 上の x64 rustc (エミュレーション) だと `amd64` (x64 ホストツール) になり、導入済みの ARM64 ホストツールと食い違う
function Get-VcVarsAllArchitecture {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)]
        [string]$TargetTriple,

        [string]$HostArch
    )

    if ([string]::IsNullOrWhiteSpace($HostArch)) {
        $HostArch = Get-WindowsNativeArchitecture
    }

    $targetArch = if ($TargetTriple.StartsWith('aarch64-')) { 'arm64' } else { 'x64' }
    if ($HostArch -eq $targetArch) {
        return $HostArch
    }

    return "${HostArch}_${targetArch}"
}

# vswhere から C++ ツール付きの VS インストール先を返す
function Get-VisualStudioInstallPath {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)]
        [string]$TargetTriple
    )

    $vswhere = Get-VsWherePath
    if (-not $vswhere) {
        $hint = Get-WindowsCrossCompileHint -TargetTriple $TargetTriple
        throw "エラー: vswhere.exe が見つかりません (Visual Studio Installer)`n$hint"
    }

    $componentId = Get-MsvcVcToolsComponentId -TargetTriple $TargetTriple
    $raw = & $vswhere -latest -products * -requires $componentId -property installationPath
    $installPath = @(
        $raw | ForEach-Object { $_.ToString().Trim() } | Where-Object { $_ }
    ) | Select-Object -First 1

    if ($LASTEXITCODE -ne 0 -or [string]::IsNullOrWhiteSpace($installPath)) {
        $hint = Get-WindowsCrossCompileHint -TargetTriple $TargetTriple
        throw "エラー: Visual Studio のインストール先を取得できませんでした (コンポーネント: $componentId)`n$hint"
    }

    return $installPath
}

# PATH を結合し、空要素と重複を除く (Windows は大文字小文字を無視)
function Merge-WindowsPathEnvironment {
    [CmdletBinding()]
    param(
        [string]$First,
        [string]$Second
    )

    $seen = [System.Collections.Generic.HashSet[string]]::new(
        [System.StringComparer]::OrdinalIgnoreCase
    )
    $parts = [System.Collections.Generic.List[string]]::new()

    foreach ($source in @($First, $Second)) {
        if ([string]::IsNullOrWhiteSpace($source)) {
            continue
        }
        foreach ($entry in $source.Split(';')) {
            $trimmed = $entry.Trim()
            if (-not $trimmed) {
                continue
            }
            if ($seen.Add($trimmed)) {
                $parts.Add($trimmed)
            }
        }
    }

    return ($parts -join ';')
}

# 指定ターゲット向けに MSVC の INCLUDE / LIB / PATH をプロセスへ取り込む
function Import-WindowsMsvcBuildEnvironment {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)]
        [string]$TargetTriple
    )

    Assert-WindowsSupportedTargetTriple -TargetTriple $TargetTriple

    $installPath = Get-VisualStudioInstallPath -TargetTriple $TargetTriple
    $vcvars = Join-Path $installPath 'VC\Auxiliary\Build\vcvarsall.bat'
    if (-not (Test-Path -LiteralPath $vcvars)) {
        $hint = Get-WindowsCrossCompileHint -TargetTriple $TargetTriple
        throw "エラー: vcvarsall.bat が見つかりません: $vcvars`n$hint"
    }

    $hostArch = Get-WindowsNativeArchitecture
    $archArg = Get-VcVarsAllArchitecture -TargetTriple $TargetTriple -HostArch $hostArch
    $targetArch = if ($TargetTriple.StartsWith('aarch64-')) { 'arm64' } else { 'x64' }
    if (
        $env:VSCMD_ARG_HOST_ARCH -eq $hostArch -and
        $env:VSCMD_ARG_TGT_ARCH -eq $targetArch -and
        -not [string]::IsNullOrWhiteSpace($env:INCLUDE)
    ) {
        Write-Host "MSVC ビルド環境は既に読み込み済みです (host=$hostArch, $archArg)"
        return
    }

    Write-Host "MSVC ビルド環境を読み込んでいます (host=$hostArch, $archArg)..."

    if ($null -eq $script:WindowsMsvcOriginalPath) {
        $script:WindowsMsvcOriginalPath = [Environment]::GetEnvironmentVariable('PATH', 'Process')
    }

    $tmpBat = Join-Path ([System.IO.Path]::GetTempPath()) ("clip-refiner-vcvars-{0}.bat" -f [guid]::NewGuid().ToString('n'))
    # 親の長い PATH を継承すると vsdevcmd が「入力行が長すぎます」(終了コード 255) になる
    $batText = @(
        '@echo off'
        'set VSCMD_SKIP_SENDTELEMETRY=1'
        'set "PATH=%SystemRoot%\System32;%SystemRoot%;%SystemRoot%\System32\Wbem"'
        'set "INCLUDE="'
        'set "LIB="'
        'set "LIBPATH="'
        'set "EXTERNAL_INCLUDE="'
        'set "VSCMD_ARG_TGT_ARCH="'
        'set "VSCMD_ARG_HOST_ARCH="'
        "call `"$vcvars`" $archArg"
        'if errorlevel 1 exit /b %ERRORLEVEL%'
        'echo CLIP_REFINER_MSVC_ENV_BEGIN'
        'set'
    ) -join "`r`n"

    [System.IO.File]::WriteAllText($tmpBat, $batText + "`r`n", [System.Text.UTF8Encoding]::new($false))

    $prevEap = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    $hasNativeEap = Test-Path variable:PSNativeCommandUseErrorActionPreference
    $prevNativeEap = $null
    if ($hasNativeEap) {
        $prevNativeEap = $PSNativeCommandUseErrorActionPreference
        $PSNativeCommandUseErrorActionPreference = $false
    }

    $output = $null
    $vcvarsExit = 1
    try {
        # AutoRun 無効。作業ディレクトリの括弧・日本語パスで vsdevcmd が壊れないよう SystemRoot から起動する
        Push-Location -LiteralPath $env:SystemRoot
        try {
            $output = & $env:ComSpec @('/d', '/c', $tmpBat)
            $vcvarsExit = $LASTEXITCODE
        } finally {
            Pop-Location
        }
    } finally {
        $ErrorActionPreference = $prevEap
        if ($hasNativeEap) {
            $PSNativeCommandUseErrorActionPreference = $prevNativeEap
        }
        Remove-Item -LiteralPath $tmpBat -ErrorAction SilentlyContinue
    }

    $lines = [System.Collections.Generic.List[string]]::new()
    foreach ($item in @($output)) {
        if ($null -ne $item) {
            $lines.Add([string]$item)
        }
    }

    if ($vcvarsExit -ne 0) {
        $detail = ($lines | Where-Object { $_ } | Select-Object -Last 40) -join [Environment]::NewLine
        $hint = Get-WindowsCrossCompileHint -TargetTriple $TargetTriple
        $detailBlock = if ($detail) { "`n`n$detail" } else { '' }
        throw "エラー: vcvarsall.bat $archArg の実行に失敗しました (終了コード $vcvarsExit)`n$vcvars`n$hint$detailBlock"
    }

    $begin = $lines.IndexOf('CLIP_REFINER_MSVC_ENV_BEGIN')
    if ($begin -lt 0) {
        throw "エラー: vcvarsall.bat $archArg は成功しましたが、環境変数一覧を取得できませんでした"
    }

    for ($i = $begin + 1; $i -lt $lines.Count; $i++) {
        $line = $lines[$i]
        if ($line -match '^([^=]+)=(.*)$') {
            [Environment]::SetEnvironmentVariable($Matches[1], $Matches[2], 'Process')
        }
    }

    $mergedPath = Merge-WindowsPathEnvironment `
        -First ([Environment]::GetEnvironmentVariable('PATH', 'Process')) `
        -Second $script:WindowsMsvcOriginalPath
    [Environment]::SetEnvironmentVariable('PATH', $mergedPath, 'Process')
}

# Cargo の target ディレクトリを解決する
#
# `cargo metadata` の JSON はコンソールコードページ次第で日本語パスが化けるため使わない
function Get-CargoTargetDirectory {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)]
        [string]$ProjectRoot
    )

    if (-not [string]::IsNullOrWhiteSpace($env:CARGO_TARGET_DIR)) {
        return [System.IO.Path]::GetFullPath($env:CARGO_TARGET_DIR)
    }

    return Join-Path $ProjectRoot 'target'
}

# 指定 triple のリリース実行ファイル置き場を返す
function Get-CargoReleaseBinDirectory {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)]
        [string]$ProjectRoot,

        [Parameter(Mandatory = $true)]
        [string]$TargetTriple,

        [switch]$ImplicitHost
    )

    $targetDir = Get-CargoTargetDirectory -ProjectRoot $ProjectRoot
    if ($ImplicitHost) {
        return Join-Path $targetDir 'release'
    }

    return Join-Path $targetDir "$TargetTriple\release"
}

# ホスト向けに `--target` を省略するか (出力は target/release)
function Test-WindowsImplicitHostTarget {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)]
        [string]$TargetTriple,

        [string]$RequestedTarget
    )

    if ([string]::IsNullOrWhiteSpace($RequestedTarget) -or $RequestedTarget -match '^(?i)host$') {
        return $true
    }

    return $false
}

# cargo / cargo-wix に渡す `--target` 引数を返す (ホスト省略時は空)
function Get-CargoTargetArguments {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)]
        [string]$TargetTriple,

        [switch]$ImplicitHost
    )

    if ($ImplicitHost) {
        return @()
    }

    return @('--target', $TargetTriple)
}

# リリース済み ClipRefiner.exe のパスを解決する
function Resolve-ReleaseExecutable {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)]
        [string]$ProjectRoot,

        [Parameter(Mandatory = $true)]
        [string]$TargetTriple,

        [switch]$ImplicitHost,

        [string]$FileName = 'ClipRefiner.exe'
    )

    $targetDir = Get-CargoTargetDirectory -ProjectRoot $ProjectRoot
    $binDir = Get-CargoReleaseBinDirectory `
        -ProjectRoot $ProjectRoot `
        -TargetTriple $TargetTriple `
        -ImplicitHost:$ImplicitHost

    $candidates = [System.Collections.Generic.List[string]]::new()
    $candidates.Add((Join-Path $binDir $FileName))
    $candidates.Add((Join-Path $binDir "deps\$FileName"))
    $candidates.Add((Join-Path $targetDir "$TargetTriple\release\$FileName"))
    $candidates.Add((Join-Path $targetDir "release\$FileName"))

    foreach ($candidate in $candidates) {
        if (Test-Path -LiteralPath $candidate) {
            return (Get-Item -LiteralPath $candidate).FullName
        }
    }

    $checked = ($candidates | ForEach-Object { "  - $_" }) -join [Environment]::NewLine
    throw @"
エラー: 実行ファイル ($FileName) が見つかりません。
探索した target ディレクトリ: $targetDir
確認した候補:
$checked

先にリリースビルドを実行するか、-SkipBuild を外してください。
CARGO_TARGET_DIR を使っている場合は、ビルド時と同じ値がこのシェルにも設定されているか確認してください。
"@
}

# PE の Machine 値 (IMAGE_FILE_HEADER) を返す
function Get-PeMachineType {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)]
        [string]$Path
    )

    $stream = [System.IO.File]::OpenRead($Path)
    try {
        $reader = New-Object System.IO.BinaryReader $stream
        if ($reader.ReadUInt16() -ne 0x5A4D) {
            throw "エラー: PE ではありません (MZ 署名がありません): $Path"
        }

        $null = $stream.Seek(0x3C, [System.IO.SeekOrigin]::Begin)
        $peOffset = $reader.ReadInt32()
        $null = $stream.Seek($peOffset, [System.IO.SeekOrigin]::Begin)
        if ($reader.ReadUInt32() -ne 0x00004550) {
            throw "エラー: PE 署名がありません: $Path"
        }

        return $reader.ReadUInt16()
    } finally {
        $stream.Dispose()
    }
}

# ターゲット triple に対応する PE Machine 値を返す
function Get-ExpectedPeMachineType {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)]
        [string]$TargetTriple
    )

    switch ($TargetTriple) {
        $script:WindowsX64Triple { return [uint16]0x8664 }
        $script:WindowsArm64Triple { return [uint16]0xAA64 }
        default {
            throw "エラー: PE Machine を解決できません: $TargetTriple"
        }
    }
}

# 実行ファイルの CPU アーキテクチャがターゲットと一致することを検証する
function Assert-PeMachineType {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)]
        [string]$Path,

        [Parameter(Mandatory = $true)]
        [string]$TargetTriple
    )

    $actual = Get-PeMachineType -Path $Path
    $expected = Get-ExpectedPeMachineType -TargetTriple $TargetTriple
    if ($actual -ne $expected) {
        throw @(
            "エラー: 実行ファイルのアーキテクチャがターゲットと一致しません: $Path",
            ('  実際の PE Machine: 0x{0:X4}' -f $actual),
            ('  期待値 ({0}): 0x{1:X4}' -f $TargetTriple, $expected)
        ) -join [Environment]::NewLine
    }
}

# MSI SummaryInformation の Template (PID_TEMPLATE = 7) を返す
function Get-MsiSummaryTemplate {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)]
        [string]$MsiPath
    )

    $installer = New-Object -ComObject WindowsInstaller.Installer
    $summary = $installer.SummaryInformation($MsiPath, 0)
    try {
        return [string]$summary.Property(7)
    } finally {
        [System.Runtime.InteropServices.Marshal]::ReleaseComObject($summary) | Out-Null
        [System.Runtime.InteropServices.Marshal]::ReleaseComObject($installer) | Out-Null
    }
}

# MSI の Platform テンプレートがターゲットと一致することを検証する
function Assert-MsiPlatformTemplate {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)]
        [string]$MsiPath,

        [Parameter(Mandatory = $true)]
        [string]$TargetTriple
    )

    $template = Get-MsiSummaryTemplate -MsiPath $MsiPath
    $normalized = $template.Trim()

    $expectedToken = switch ($TargetTriple) {
        $script:WindowsX64Triple { 'x64' }
        $script:WindowsArm64Triple { 'Arm64' }
        default { throw "エラー: MSI Platform を解決できません: $TargetTriple" }
    }

    if ($normalized -notmatch "(?i)$([regex]::Escape($expectedToken))") {
        throw @(
            "エラー: MSI の Platform テンプレートがターゲットと一致しません: $MsiPath",
            "  実際: $template",
            "  期待: $expectedToken を含む ($TargetTriple)"
        ) -join [Environment]::NewLine
    }
}

# MSI ファイル名から配布ターゲット triple を推定する
function Resolve-WindowsTargetTripleFromMsiName {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)]
        [string]$MsiFileName
    )

    $name = [System.IO.Path]::GetFileName($MsiFileName)
    if ($name -match 'aarch64') {
        return $script:WindowsArm64Triple
    }
    if ($name -match 'x86_64') {
        return $script:WindowsX64Triple
    }

    return $null
}

# クロスコンパイル失敗時の案内を付けて再送出する
function Get-WindowsCrossCompileHint {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)]
        [string]$TargetTriple
    )

    if ($TargetTriple -eq $script:WindowsArm64Triple) {
        return @(
            'ARM64 向けには Visual Studio の C++ ARM64/ARM64EC ビルドツールが必要です。',
            'Visual Studio Installer の Individual components から MSVC の ARM64 ビルドツールを追加してください。'
        ) -join [Environment]::NewLine
    }

    if ($TargetTriple -eq $script:WindowsX64Triple) {
        return @(
            'x64 向けには Visual Studio の C++ x64/x86 ビルドツールが必要です。',
            'Visual Studio Installer の Individual components から MSVC の x64/x86 ビルドツールを追加してください。'
        ) -join [Environment]::NewLine
    }

    return [string]::Empty
}
