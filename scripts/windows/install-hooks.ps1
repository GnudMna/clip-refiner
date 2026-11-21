# ========================================================================
# Script Name : install-hooks.ps1
# Description : Git フックを有効化する (core.hooksPath を設定)
# Usage       : ./install-hooks.ps1
# Requires    : PowerShell 7.0+, Git
# ========================================================================

#requires -Version 7.0

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

. (Join-Path $PSScriptRoot '..\common\wait-if-double-clicked.ps1')

Invoke-ScriptMain {
    . (Join-Path $PSScriptRoot '..\common\cd-project-root.ps1')

    if (-not (Get-Command git -ErrorAction SilentlyContinue)) {
        throw 'エラー: git が見つかりません'
    }

    if (-not (Test-Path -LiteralPath (Join-Path $ProjectRoot '.git'))) {
        throw 'エラー: Git リポジトリではありません (.git がありません)'
    }

    $hooksRelativePath = 'scripts/git/hooks'
    $hooksAbsolutePath = Join-Path $ProjectRoot $hooksRelativePath
    if (-not (Test-Path -LiteralPath $hooksAbsolutePath)) {
        throw "エラー: フックディレクトリが見つかりません: $hooksRelativePath"
    }

    $requiredHooks = @('pre-commit', 'pre-push')
    foreach ($hookName in $requiredHooks) {
        $hookPath = Join-Path $hooksAbsolutePath $hookName
        if (-not (Test-Path -LiteralPath $hookPath)) {
            throw "エラー: フックが見つかりません: $hooksRelativePath/$hookName"
        }
    }

    $currentHooksPath = git config --get core.hooksPath 2>$null
    if ($currentHooksPath -and $currentHooksPath -ne $hooksRelativePath) {
        throw @"
エラー: core.hooksPath が別の値に設定されています
  現在: $currentHooksPath
  期待: $hooksRelativePath
解除する場合: ./uninstall-hooks.ps1
"@
    }

    git config core.hooksPath $hooksRelativePath
    if ($LASTEXITCODE -ne 0) {
        throw 'エラー: git config core.hooksPath の設定に失敗しました'
    }

    foreach ($hookName in $requiredHooks) {
        $relativePath = "$hooksRelativePath/$hookName"
        git ls-files --error-unmatch $relativePath 2>$null | Out-Null
        if ($LASTEXITCODE -ne 0) {
            Write-Host "note: $relativePath は未追跡のため実行権限を Git 索引に設定しません"
            continue
        }

        git update-index --chmod=+x $relativePath
        if ($LASTEXITCODE -ne 0) {
            throw "エラー: フックの実行権限設定に失敗しました: $relativePath"
        }
    }

    Write-Host 'Git フックを有効化しました'
    Write-Host ''
    Write-Host "  core.hooksPath = $hooksRelativePath"
    Write-Host '  pre-commit: cargo fmt --check (check-pre-commit.ps1)'
    Write-Host '  pre-push:   check.ps1 (fmt / clippy / deny / eslint / test)'
    Write-Host ''
    Write-Host 'スキップ:'
    Write-Host '  CLIP_REFINER_SKIP_HOOK=1'
    Write-Host '  git commit --no-verify / git push --no-verify'
}
