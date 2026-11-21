# ========================================================================
# Script Name : uninstall-hooks.ps1
# Description : Git フックを無効化する (core.hooksPath を解除)
# Usage       : ./uninstall-hooks.ps1
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

    $expectedHooksPath = 'scripts/git/hooks'
    $currentHooksPath = git config --get core.hooksPath 2>$null
    if (-not $currentHooksPath) {
        # 未設定時の git config --get の終了コード 1 を Invoke-ScriptMain に拾わせない
        $global:LASTEXITCODE = 0
        Write-Host 'note: core.hooksPath は未設定です'
        return
    }

    if ($currentHooksPath -ne $expectedHooksPath) {
        throw @"
エラー: core.hooksPath がこのリポジトリのフックではありません
  現在: $currentHooksPath
  期待: $expectedHooksPath
手動で解除する場合: git config --unset core.hooksPath
"@
    }

    git config --unset core.hooksPath
    if ($LASTEXITCODE -ne 0) {
        throw 'エラー: git config --unset core.hooksPath に失敗しました'
    }

    Write-Host 'Git フックを無効化しました'
}
