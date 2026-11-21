# ========================================================================
# Script Name : release.ps1
# Description : リリース手順を一括実行する (バージョン更新 / CHANGELOG / 品質チェック / Git / MSI / ポータブル ZIP)
# Usage       : git checkout -b release/v0.12.2
#               ./release.ps1 -Version 0.12.2
#               ./release.ps1 -Bump Patch -DryRun
# Requires    : PowerShell 7.0+, Git
#
# 前提:
#   - 環境変数 CLIP_REFINER_KEY を設定すること (gh-release-bridge ダウンロードキー。ビルド時埋め込み)
#   - 環境変数 CLIP_REFINER_SIGNING_KEY に更新署名用 minisign 秘密鍵のパスを設定すること
#     (keys/update-signing/*.pub に対応する公開鍵があり、minisign コマンドが使えること)
#   - release/v{version} ブランチ上で実行すること (ブランチ名と更新バージョンが一致必須)
#   - Git タグは Gitea 上で付与する (本スクリプトでは作成しない)
#
# オプション:
#   -Bump Patch|Minor|Major   現在のバージョンから SemVer インクリメント
#   -Version x.y.z             明示的な新バージョン指定 (-Bump と排他)
#   -Date yyyy-MM-dd           リリース日 (既定: 本日)
#   -DryRun                    変更内容を表示するのみ
#   -SkipCheck                 check.ps1 (fmt / clippy / deny / eslint / test / e2e) をスキップ
#   -SkipMsi                   build-msi.ps1 をスキップ
#   -SkipPortable              build-portable.ps1 をスキップ
#   -NoCommit                  Git コミットを作成しない
#   -AllowDirty                未コミット変更があっても続行する
#   -Target Host|All|x64|arm64 配布アーキテクチャ (省略時はホスト。All で x64 と ARM64)
# ========================================================================

#requires -Version 7.0

[CmdletBinding()]
param(
    [string]$Version,

    [ValidateSet('Patch', 'Minor', 'Major')]
    [string]$Bump,

    [string]$Date = (Get-Date -Format 'yyyy-MM-dd'),

    [switch]$DryRun,
    [switch]$SkipCheck,
    [switch]$SkipMsi,
    [switch]$SkipPortable,
    [switch]$NoCommit,
    [switch]$AllowDirty,

    # 配布ターゲット。省略時はホスト。Host / All / x64 / arm64 / triple
    [string]$Target
)

# シェルオプションを設定(エラー時に即終了)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

# ダブルクリック実行時の Enter 待ちヘルパーを読み込み
. (Join-Path $PSScriptRoot '..\common\wait-if-double-clicked.ps1')

Invoke-ScriptMain {
    # プロジェクトルートへ移動
    . (Join-Path $PSScriptRoot '..\common\cd-project-root.ps1')
    . (Join-Path $PSScriptRoot '..\common\release-version.ps1')
    . (Join-Path $PSScriptRoot '..\common\update-signing.ps1')

    if ($Date -notmatch '^\d{4}-\d{2}-\d{2}$') {
        throw "エラー: -Date は yyyy-MM-dd 形式で指定してください: $Date"
    }

    if ($Version -and $Bump) {
        throw 'エラー: -Version と -Bump は同時に指定できません'
    }

    # gh-release-bridge の保護範囲 DL 用キー (ビルド時にバイナリへ埋め込まれる)
    $bridgeDownloadKey = [Environment]::GetEnvironmentVariable('CLIP_REFINER_KEY')
    if ([string]::IsNullOrWhiteSpace($bridgeDownloadKey)) {
        throw 'エラー: 環境変数 CLIP_REFINER_KEY が未設定または空です (gh-release-bridge のダウンロードキー。ビルド時に埋め込まれます)'
    }

    # 署名なしのアセットはアプリが適用しないため、ビルド前に署名の準備を確認する
    $needsSigning = -not ($SkipMsi -and $SkipPortable)
    $signingSecretKey = if ($needsSigning) {
        Assert-UpdateSigningReady -ProjectRoot $ProjectRoot
    }

    $currentVersion = Get-WorkspaceVersion -ProjectRoot $ProjectRoot
    $newVersion = if ($Version) {
        $Version
    } elseif ($Bump) {
        Get-BumpedSemVer -Version $currentVersion -Bump $Bump
    } else {
        throw 'エラー: -Version または -Bump を指定してください (例: -Bump Patch)'
    }

    Assert-NewVersionIsGreater -CurrentVersion $currentVersion -NewVersion $newVersion

    $releaseBranch = "release/v$newVersion"
    $releaseFiles = Get-ReleaseVersionFilePaths -ProjectRoot $ProjectRoot

    Write-Host 'ClipRefiner リリース準備'
    Write-Host "  現在のバージョン: $currentVersion"
    Write-Host "  新しいバージョン: $newVersion"
    Write-Host "  リリース日:       $Date"
    Write-Host "  リリースブランチ: $releaseBranch"
    if ($DryRun) {
        Write-Host '  モード:           DryRun (ファイルは変更しません)'
    }
    Write-Host ''

    Assert-GitReleaseBranch -ProjectRoot $ProjectRoot -NewVersion $newVersion

    if (-not $DryRun -and -not $AllowDirty) {
        Assert-GitWorkingTreeClean -ProjectRoot $ProjectRoot
    }

    Write-Host 'バージョン関連ファイルを更新します...'
    Set-WorkspaceCargoVersion -ProjectRoot $ProjectRoot -NewVersion $newVersion -DryRun:$DryRun
    Update-ChangelogForRelease `
        -ProjectRoot $ProjectRoot `
        -CurrentVersion $currentVersion `
        -NewVersion $newVersion `
        -ReleaseDate $Date `
        -DryRun:$DryRun
    Set-LibRsDocVersions -ProjectRoot $ProjectRoot -NewVersion $newVersion -DryRun:$DryRun
    Sync-WorkspaceCargoLock -ProjectRoot $ProjectRoot -DryRun:$DryRun
    Write-Host ''

    if ($DryRun) {
        Write-Host 'DryRun のため、品質チェック・Git・MSI・ポータブル ZIP は実行しません'
        return
    }

    if (-not $SkipCheck) {
        Write-Host '品質チェックを実行します (check.ps1)...'
        Write-Host ''
        $checkScript = Join-Path $PSScriptRoot 'check.ps1'
        & $checkScript
        if ($LASTEXITCODE -ne 0) {
            throw "エラー: check.ps1 が終了コード $LASTEXITCODE で失敗しました"
        }
        Write-Host ''
    } else {
        Write-Host '品質チェックをスキップしました (-SkipCheck)'
        Write-Host ''
    }

    if (-not $NoCommit) {
        $filesToCommit = $releaseFiles | Where-Object { Test-Path -LiteralPath $_ }
        New-ReleaseGitCommit -ProjectRoot $ProjectRoot -NewVersion $newVersion -FilePaths $filesToCommit
        Write-Host ''
    } else {
        Write-Host 'Git コミットをスキップしました (-NoCommit)'
        Write-Host '  手動でコミットする場合: git add Cargo.toml crates/clip-refiner/Cargo.toml Cargo.lock CHANGELOG.md crates/clip-refiner/src/lib.rs'
        Write-Host "  推奨メッセージ: Release: v$newVersion"
        Write-Host ''
    }

    $msiBuilt = $false
    if (-not $SkipMsi) {
        Write-Host 'MSI インストーラーを作成します (build-msi.ps1)...'
        Write-Host ''
        $msiScript = Join-Path $PSScriptRoot 'build-msi.ps1'
        if ($Target) {
            & $msiScript -Target $Target
        } else {
            & $msiScript
        }
        if ($LASTEXITCODE -ne 0) {
            throw "エラー: build-msi.ps1 が終了コード $LASTEXITCODE で失敗しました"
        }
        $msiBuilt = $true
        Write-Host ''
    } else {
        Write-Host 'MSI 作成をスキップしました (-SkipMsi)'
        Write-Host ''
    }

    if (-not $SkipPortable) {
        Write-Host 'ポータブル ZIP を作成します (build-portable.ps1)...'
        Write-Host ''
        $portableScript = Join-Path $PSScriptRoot 'build-portable.ps1'
        # MSI 作成済みならリリースビルドと THIRD_PARTY_NOTICES を再利用する
        # (配列スプラットだと '-SkipBuild' が位置引数になりバインドに失敗する)
        if ($msiBuilt) {
            if ($Target) {
                & $portableScript -SkipBuild -SkipNotices -Target $Target
            } else {
                & $portableScript -SkipBuild -SkipNotices
            }
        } else {
            if ($Target) {
                & $portableScript -Target $Target
            } else {
                & $portableScript
            }
        }
        if ($LASTEXITCODE -ne 0) {
            throw "エラー: build-portable.ps1 が終了コード $LASTEXITCODE で失敗しました"
        }
        Write-Host ''
    } else {
        Write-Host 'ポータブル ZIP 作成をスキップしました (-SkipPortable)'
        Write-Host ''
    }

    if ($needsSigning) {
        Write-Host '更新アセットへ署名します (minisign)...'
        Invoke-UpdateAssetSigning -ProjectRoot $ProjectRoot -Version $newVersion -SecretKeyPath $signingSecretKey
        Write-Host ''
    }

    Write-Host 'リリース手順が完了しました'
    Write-Host ''
    Write-Host '次の作業 (手動):'
    Write-Host '  - README.md / docs/user/ にユーザ向けの大きな変更があれば更新'
    Write-Host '  - consts::CONFIG_VERSION を変更した場合は CHANGELOG の設定節を追記'
    Write-Host "  - リモートへ push: git push -u origin $releaseBranch"
    Write-Host "  - Gitea 上で v$newVersion タグを付与する"
    Write-Host '  - GitHub Release へ MSI / ポータブル ZIP と署名ファイル (.minisig) をアップロードする'
    Write-Host '      MSI:      target\wix\clip-refiner-{version}-{target-triple}.msi (+ .msi.minisig)'
    Write-Host '      Portable: target\portable\clip-refiner-{version}-{target-triple}-portable.zip (+ .zip.minisig)'
    Write-Host '      (署名ファイルが無いとアプリは更新を適用しない)'
    Write-Host '      (x64 と ARM64 の両方を出す場合は -Target All)'
}
