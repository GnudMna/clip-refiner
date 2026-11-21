# ========================================================================
# Script Name : update-signing.ps1
# Description : 更新アセット (MSI / ポータブル ZIP) の minisign 署名 (ドットソース用)
# Usage       : . (Join-Path $PSScriptRoot 'update-signing.ps1')
# Requires    : PowerShell 7.0+, minisign
#
# 前提:
#   - 環境変数 CLIP_REFINER_SIGNING_KEY に minisign 秘密鍵ファイルのパスを設定すること
#   - keys/update-signing/*.pub に対応する公開鍵を置き、アプリへ埋め込んでおくこと
#
# 信頼コメントはアプリ側 (platform/update/signature.rs) の期待値と同じ形式にする
#   clip-refiner-update version={version} file={アセット名}
# ========================================================================

#requires -Version 7.0

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

. (Join-Path $PSScriptRoot 'assert.ps1')

# 署名ファイルの拡張子 (アプリ側 SIGNATURE_SUFFIX と同じ)
$script:UpdateSignatureSuffix = '.minisig'

# 埋め込み公開鍵の一覧を返す
function Get-UpdateSigningPublicKeyPaths {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)]
        [string]$ProjectRoot
    )

    $keysDir = Join-Path $ProjectRoot 'keys\update-signing'
    if (-not (Test-Path -LiteralPath $keysDir)) {
        return @()
    }
    return @(Get-ChildItem -LiteralPath $keysDir -Filter '*.pub' -File | Sort-Object Name | ForEach-Object FullName)
}

# 署名に必要なツール・鍵が揃っていることを検証し、秘密鍵のパスを返す
function Assert-UpdateSigningReady {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)]
        [string]$ProjectRoot
    )

    if (-not (Get-Command minisign -ErrorAction SilentlyContinue)) {
        throw @(
            'エラー: minisign が見つかりません (更新アセットの署名に必要)',
            'インストール: winget install jedisct1.minisign または scoop install minisign'
        ) -join [Environment]::NewLine
    }

    # 関数の戻り値はパイプラインで展開され 1 件だとスカラーになるため、配列に包み直す
    $publicKeys = @(Get-UpdateSigningPublicKeyPaths -ProjectRoot $ProjectRoot)
    if ($publicKeys.Count -eq 0) {
        throw 'エラー: keys/update-signing/*.pub がありません。公開鍵を埋め込まないとアプリが更新を適用できません (DEVELOPMENT.md の更新署名鍵を参照)'
    }

    $secretKey = [Environment]::GetEnvironmentVariable('CLIP_REFINER_SIGNING_KEY')
    if ([string]::IsNullOrWhiteSpace($secretKey)) {
        throw 'エラー: 環境変数 CLIP_REFINER_SIGNING_KEY が未設定です (更新署名用 minisign 秘密鍵ファイルのパス)'
    }
    if (-not (Test-Path -LiteralPath $secretKey -PathType Leaf)) {
        throw "エラー: CLIP_REFINER_SIGNING_KEY の秘密鍵ファイルが見つかりません: $secretKey"
    }

    return $secretKey
}

# 指定バージョンの更新アセットへ署名し、埋め込み公開鍵で検証する
function Invoke-UpdateAssetSigning {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)]
        [string]$ProjectRoot,

        [Parameter(Mandatory = $true)]
        [string]$Version,

        [Parameter(Mandatory = $true)]
        [string]$SecretKeyPath
    )

    $assets = @(
        Get-ChildItem -Path (Join-Path $ProjectRoot 'target\wix') -Filter "clip-refiner-$Version-*.msi" -File -ErrorAction SilentlyContinue
        Get-ChildItem -Path (Join-Path $ProjectRoot 'target\portable') -Filter "clip-refiner-$Version-*-portable.zip" -File -ErrorAction SilentlyContinue
    )
    if ($assets.Count -eq 0) {
        throw "エラー: 署名対象の更新アセットが見つかりません (version=$Version)"
    }

    $publicKeys = @(Get-UpdateSigningPublicKeyPaths -ProjectRoot $ProjectRoot)

    foreach ($asset in $assets) {
        $signaturePath = "$($asset.FullName)$script:UpdateSignatureSuffix"
        $trustedComment = "clip-refiner-update version=$Version file=$($asset.Name)"

        Write-Host "  署名: $($asset.Name)"
        # 秘密鍵がパスワード保護されている場合はここで入力を求められる
        minisign -S -s $SecretKeyPath -m $asset.FullName -x $signaturePath -t $trustedComment
        Assert-LastExitCode -CommandLabel "minisign -S ($($asset.Name))"

        # 埋め込み公開鍵のいずれかで検証できなければ、アプリ側でも検証に失敗する
        $verified = $false
        foreach ($publicKey in $publicKeys) {
            minisign -V -q -p $publicKey -m $asset.FullName -x $signaturePath *> $null
            if ($LASTEXITCODE -eq 0) {
                $verified = $true
                break
            }
        }
        if (-not $verified) {
            throw "エラー: $($asset.Name) の署名を keys/update-signing/*.pub で検証できません (秘密鍵と公開鍵の組を確認してください)"
        }
    }
}
