# ========================================================================
# Script Name : cargo.ps1
# Description : Cargo コマンドの共通ラッパー (ドットソース用)
# Usage       : . (Join-Path $PSScriptRoot 'cargo.ps1')
# Requires    : PowerShell 7.0+, Rust / Cargo
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
}

# テストを実行する
function Invoke-CargoTest {
    [CmdletBinding()]
    param(
        [switch]$IncludeDocTests
    )

    cargo test --all-targets --features $script:CargoTestFeatures
    Assert-LastExitCode -CommandLabel 'cargo test'

    if ($IncludeDocTests) {
        cargo test --doc --features $script:CargoTestFeatures
        Assert-LastExitCode -CommandLabel 'cargo test --doc'
    }
}
