# ========================================================================
# Script Name : cd-project-root.ps1
# Description : プロジェクトのルートディレクトリに移動する(ドットソース用)
# Usage       : . (Join-Path $PSScriptRoot '..\common\cd-project-root.ps1')
# Requires    : PowerShell 7.0+
# ========================================================================

#requires -Version 7.0

# プロジェクトルートを計算
$ProjectRoot = (Get-Item -LiteralPath (Join-Path $PSScriptRoot '..\..')).FullName

# プロジェクトルートに移動
Set-Location -LiteralPath $ProjectRoot

# Cargo.toml の存在を確認
$CargoTomlPath = Join-Path $ProjectRoot 'Cargo.toml'
if (-not (Test-Path -LiteralPath $CargoTomlPath)) {
    throw "エラー: プロジェクトルートが見つかりません (Cargo.toml がありません): $ProjectRoot"
}
