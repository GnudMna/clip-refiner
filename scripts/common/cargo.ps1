# ========================================================================
# Script Name : cargo.ps1
# Description : Cargo コマンドの共通ラッパー (fmt / clippy / test / llvm-cov)
# Usage       : . (Join-Path $PSScriptRoot 'cargo.ps1')
# Requires    : PowerShell 7.0+, Rust / Cargo
#               Invoke-CargoLlvmCov は cargo-llvm-cov と llvm-tools-preview も必要
# ========================================================================

#requires -Version 7.0

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

. (Join-Path $PSScriptRoot 'assert.ps1')

$script:CargoTestFeatures = 'test-helpers'

# rustfmt でコードを整形する
function Invoke-CargoFmt {
    [CmdletBinding()]
    param()

    cargo fmt
    Assert-LastExitCode -CommandLabel 'cargo fmt'
}

# rustfmt による整形差分がないことを検証する
function Invoke-CargoFmtCheck {
    [CmdletBinding()]
    param()

    cargo fmt --check
    Assert-LastExitCode -CommandLabel 'cargo fmt --check'
}

# Clippy を実行する
function Invoke-CargoClippy {
    [CmdletBinding()]
    param()

    cargo clippy --all-targets --features $script:CargoTestFeatures -- -D warnings
    Assert-LastExitCode -CommandLabel 'cargo clippy'

    cargo clippy -p clip-refiner-core --no-default-features --all-targets -- -D warnings
    Assert-LastExitCode -CommandLabel 'cargo clippy -p clip-refiner-core --no-default-features'
}

# テストを実行する
function Invoke-CargoTest {
    [CmdletBinding()]
    param(
        [switch]$IncludeDocTests
    )

    cargo test --all-targets --features $script:CargoTestFeatures
    Assert-LastExitCode -CommandLabel 'cargo test'

    cargo test -p clip-refiner-core --no-default-features --all-targets
    Assert-LastExitCode -CommandLabel 'cargo test -p clip-refiner-core --no-default-features'

    if ($IncludeDocTests) {
        cargo test --doc --features $script:CargoTestFeatures
        Assert-LastExitCode -CommandLabel 'cargo test --doc'
    }
}

# ベンチマーク (criterion) を実行し、基準との比較で悪化したベンチ名を返す
#
# -SaveBaseline は結果を指定名の基準として保存し、-Baseline は保存済みの基準と比較する。
# 計測値はマシン負荷でぶれるため、悪化の判定は目安として扱う (終了コードには反映しない)
function Invoke-CargoBench {
    [CmdletBinding()]
    param(
        [string]$SaveBaseline,
        [string]$Baseline,
        [string]$Filter
    )

    $benchArgs = @('bench', '-p', 'clip-refiner-core', '--bench', 'transforms')
    $criterionArgs = @()
    if ($Filter) {
        $criterionArgs += $Filter
    }
    if ($SaveBaseline) {
        $criterionArgs += @('--save-baseline', $SaveBaseline)
    }
    if ($Baseline) {
        $criterionArgs += @('--baseline', $Baseline)
    }
    if ($criterionArgs.Count -gt 0) {
        $benchArgs += '--'
        $benchArgs += $criterionArgs
    }

    $regressed = [System.Collections.Generic.List[string]]::new()
    $current = ''
    cargo @benchArgs | ForEach-Object {
        Write-Host $_
        # ベンチ名は単独行か `time:` と同じ行の先頭に出る
        if ($_ -match '^(\S+/\d+)(\s|$)') {
            $current = $Matches[1]
        }
        if ($_ -match 'Performance has regressed') {
            $regressed.Add($current)
        }
    }
    Assert-LastExitCode -CommandLabel 'cargo bench'

    return $regressed.ToArray()
}

# デバッグバイナリをビルドする (E2E 用など)
function Invoke-CargoBuild {
    [CmdletBinding()]
    param()

    cargo build -p clip-refiner
    Assert-LastExitCode -CommandLabel 'cargo build -p clip-refiner'
}

# cargo-llvm-cov と llvm-tools-preview が使えることを検証する
function Assert-CargoLlvmCovReady {
    [CmdletBinding()]
    param()

    if (-not (Get-Command cargo-llvm-cov -ErrorAction SilentlyContinue)) {
        throw @(
            'エラー: cargo-llvm-cov が見つかりません',
            'インストール: cargo install cargo-llvm-cov --locked',
            'あわせて: rustup component add llvm-tools-preview'
        ) -join [Environment]::NewLine
    }

    $installed = rustup component list --installed 2>$null
    Assert-LastExitCode -CommandLabel 'rustup component list --installed'
    $hasLlvmTools = $installed | Where-Object {
        $_ -match '^llvm-tools(-preview)?(-|$)'
    }
    if (-not $hasLlvmTools) {
        throw @(
            'エラー: rustup の llvm-tools-preview が未インストールです',
            'インストール: rustup component add llvm-tools-preview'
        ) -join [Environment]::NewLine
    }
}

# テストをカバレッジ計測付きで実行し、レポートを出力する
#
# 既定は端末向けテキスト要約。統合テスト・test_helpers・*_tests.rs は集計から除外する。
# doctests は cargo-llvm-cov では不安定のため含めない (呼び出し側で別途実行する)
function Invoke-CargoLlvmCov {
    [CmdletBinding()]
    param(
        [switch]$Html,
        [switch]$Open
    )

    Assert-CargoLlvmCovReady

    # 本番コードの厚みを見るため、テスト専用パスを除外する
    $ignoreFilenameRegex = '(^|[/\\])(tests|test_helpers)([/\\]|$)|[/\\][^/\\]*_tests\.rs$|[/\\]tests\.rs$'

    $llvmCovArgs = @(
        'llvm-cov',
        '--all-targets',
        '--features', $script:CargoTestFeatures,
        '--ignore-filename-regex', $ignoreFilenameRegex
    )

    if ($Open) {
        $llvmCovArgs += '--open'
    }
    elseif ($Html) {
        $llvmCovArgs += '--html'
    }

    cargo @llvmCovArgs
    Assert-LastExitCode -CommandLabel 'cargo llvm-cov'

    if ($Open -or $Html) {
        $targetRoot = if ($env:CARGO_TARGET_DIR) {
            $env:CARGO_TARGET_DIR
        }
        else {
            Join-Path (Get-Location) 'target'
        }
        $reportPath = Join-Path $targetRoot 'llvm-cov\html\index.html'
        if (Test-Path -LiteralPath $reportPath) {
            Write-Host "HTML レポート: $reportPath"
        }
    }
}
