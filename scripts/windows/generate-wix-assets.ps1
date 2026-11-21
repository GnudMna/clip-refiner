# ========================================================================
# Script Name : generate-wix-assets.ps1
# Description : WiX インストーラー用バナー・ダイアログ画像 (BMP) を生成する
# Usage       : ./generate-wix-assets.ps1
# Requires    : PowerShell 7.0+, Windows (System.Drawing)
#
# 出力 (WiX 必須サイズ):
#   wix/Banner.bmp  - 493 x 58  (上部バナー。左白 + 右装飾)
#   wix/Dialog.bmp  - 493 x 312 (ようこそ画面。左装飾 + 右白背景)
#   wix/Eula.rtf    - ライセンス同意画面 (LICENSE から生成)
# ========================================================================

#requires -Version 7.0

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

. (Join-Path $PSScriptRoot '..\common\wait-if-double-clicked.ps1')

# アプリアイコン (icon-base.svg) と揃えたブランドカラー
$ColorPrimary = [System.Drawing.Color]::FromArgb(96, 125, 139)
$ColorPrimaryDark = [System.Drawing.Color]::FromArgb(69, 90, 100)
$ColorAccent = [System.Drawing.Color]::FromArgb(255, 193, 7)
$ColorAccentLight = [System.Drawing.Color]::FromArgb(255, 224, 130)
$ColorClipHandle = [System.Drawing.Color]::FromArgb(144, 164, 174)
$ColorWhite = [System.Drawing.Color]::White

function New-LinearGradientBrush {
    param(
        [System.Drawing.Rectangle]$Rect,
        [System.Drawing.Color]$ColorStart,
        [System.Drawing.Color]$ColorEnd,
        [float]$Angle = 0
    )

    return New-Object System.Drawing.Drawing2D.LinearGradientBrush @(
        $Rect,
        $ColorStart,
        $ColorEnd,
        $Angle
    )
}

function Set-GraphicsQuality {
    param([System.Drawing.Graphics]$Graphics)

    $Graphics.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
    $Graphics.TextRenderingHint = [System.Drawing.Text.TextRenderingHint]::ClearTypeGridFit
}

function Draw-ClipboardIcon {
    param(
        [System.Drawing.Graphics]$Graphics,
        [int]$X,
        [int]$Y,
        [int]$Size
    )

    $bodyRect = [System.Drawing.Rectangle]::new(
        $X,
        $Y + [int]($Size * 0.12),
        $Size,
        [int]($Size * 0.88)
    )
    $handleRect = [System.Drawing.Rectangle]::new(
        $X + [int]($Size * 0.28),
        $Y,
        [int]($Size * 0.44),
        [int]($Size * 0.16)
    )

    $bodyBrush = New-Object System.Drawing.SolidBrush $ColorPrimary
    $handleBrush = New-Object System.Drawing.SolidBrush $ColorClipHandle
    $lineBrush = New-Object System.Drawing.SolidBrush $ColorWhite
    $accentBrush = New-Object System.Drawing.SolidBrush $ColorAccent

    $Graphics.FillRectangle($handleBrush, $handleRect)
    $Graphics.FillRectangle($bodyBrush, $bodyRect)

    $lineHeight = [Math]::Max(1, [int]($Size * 0.04))
    $lineY1 = $bodyRect.Top + [int]($bodyRect.Height * 0.22)
    $lineY2 = $bodyRect.Top + [int]($bodyRect.Height * 0.42)
    $lineY3 = $bodyRect.Top + [int]($bodyRect.Height * 0.62)
    $lineX = $bodyRect.Left + [int]($bodyRect.Width * 0.2)
    $lineWidth = [int]($bodyRect.Width * 0.6)

    $Graphics.FillRectangle($lineBrush, $lineX, $lineY1, $lineWidth, $lineHeight)
    $Graphics.FillRectangle($lineBrush, $lineX, $lineY2, $lineWidth, $lineHeight)
    $Graphics.FillRectangle($lineBrush, $lineX, $lineY3, $lineWidth, $lineHeight)

    $badgeSize = [int]($Size * 0.28)
    $badgeX = $bodyRect.Right - [int]($badgeSize * 0.55)
    $badgeY = $bodyRect.Bottom - [int]($badgeSize * 0.55)
    $Graphics.FillEllipse($accentBrush, $badgeX, $badgeY, $badgeSize, $badgeSize)

    $bodyBrush.Dispose()
    $handleBrush.Dispose()
    $lineBrush.Dispose()
    $accentBrush.Dispose()
}

function Save-WixBitmap {
    param(
        [int]$Width,
        [int]$Height,
        [string]$OutputPath,
        [scriptblock]$Draw
    )

    $bitmap = New-Object System.Drawing.Bitmap $Width, $Height
    $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
    Set-GraphicsQuality -Graphics $graphics

    & $Draw $graphics $Width $Height

    $graphics.Dispose()
    $bitmap.Save($OutputPath, [System.Drawing.Imaging.ImageFormat]::Bmp)
    $bitmap.Dispose()
}

function New-BannerBitmap {
    param([string]$OutputPath)

    # WiX 各ダイアログ: タイトル/説明はバナー左側 (X=15 付近) に重なる
    $textOverlayEnd = 300

    Save-WixBitmap -Width 493 -Height 58 -OutputPath $OutputPath -Draw {
        param($Graphics, $Width, $Height)

        $leftRect = [System.Drawing.Rectangle]::new(0, 0, $textOverlayEnd, $Height)
        $leftBrush = New-Object System.Drawing.SolidBrush ([System.Drawing.Color]::FromArgb(255, 255, 255))
        $Graphics.FillRectangle($leftBrush, $leftRect)
        $leftBrush.Dispose()

        $rightRect = [System.Drawing.Rectangle]::new($textOverlayEnd, 0, $Width - $textOverlayEnd, $Height)
        $background = New-LinearGradientBrush -Rect $rightRect `
            -ColorStart $ColorPrimary `
            -ColorEnd $ColorPrimaryDark `
            -Angle 0
        $Graphics.FillRectangle($background, $rightRect)
        $background.Dispose()

        $accentBrush = New-Object System.Drawing.SolidBrush $ColorAccent
        $Graphics.FillRectangle($accentBrush, $textOverlayEnd, 0, 3, $Height)
        $accentBrush.Dispose()

        $iconSize = 40
        $iconX = $Width - $iconSize - 24
        $iconY = [int](($Height - $iconSize) / 2)
        Draw-ClipboardIcon -Graphics $Graphics -X $iconX -Y $iconY -Size $iconSize
    }
}

function New-DialogBitmap {
    param([string]$OutputPath)

    # WiX WelcomeDlg: ビットマップ X=11、テキスト X=135 のため左 124px が装飾領域
    $textOverlayStart = 124

    Save-WixBitmap -Width 493 -Height 312 -OutputPath $OutputPath -Draw {
        param($Graphics, $Width, $Height)

        $rightRect = [System.Drawing.Rectangle]::new(
            $textOverlayStart,
            0,
            $Width - $textOverlayStart,
            $Height
        )
        $rightBrush = New-Object System.Drawing.SolidBrush ([System.Drawing.Color]::FromArgb(255, 255, 255))
        $Graphics.FillRectangle($rightBrush, $rightRect)
        $rightBrush.Dispose()

        $leftRect = [System.Drawing.Rectangle]::new(0, 0, $textOverlayStart, $Height)
        $background = New-LinearGradientBrush -Rect $leftRect `
            -ColorStart $ColorPrimaryDark `
            -ColorEnd $ColorPrimary `
            -Angle 90
        $Graphics.FillRectangle($background, $leftRect)
        $background.Dispose()

        $accentBrush = New-Object System.Drawing.SolidBrush $ColorAccent
        $Graphics.FillRectangle($accentBrush, ($textOverlayStart - 4), 0, 4, $Height)
        $accentBrush.Dispose()

        $iconSize = 88
        $iconX = [int](($textOverlayStart - $iconSize) / 2)
        $iconY = [int](($Height - $iconSize) / 2)
        Draw-ClipboardIcon -Graphics $Graphics -X $iconX -Y $iconY -Size $iconSize
    }
}

function ConvertTo-RtfUnicodeText {
    param([string]$Text)

    $builder = [System.Text.StringBuilder]::new()
    foreach ($char in $Text.ToCharArray()) {
        $code = [int][char]$char
        switch ($char) {
            '\' { [void]$builder.Append('\\') }
            '{' { [void]$builder.Append('\{') }
            '}' { [void]$builder.Append('\}') }
            default {
                if ($code -gt 127) {
                    [void]$builder.Append("\u${code}?")
                } else {
                    [void]$builder.Append($char)
                }
            }
        }
    }

    return $builder.ToString()
}

function ConvertTo-RtfParagraphs {
    param([string[]]$Lines)

    $result = [System.Text.StringBuilder]::new()
    foreach ($line in $Lines) {
        if ($result.Length -gt 0) {
            # 直後が英字だと \parAll 等の制御語と誤認識されるため区切りに空白を入れる
            [void]$result.Append('\par ')
        }
        [void]$result.Append((ConvertTo-RtfUnicodeText $line))
    }

    return $result.ToString()
}

function New-EulaRtf {
    param(
        [string]$LicensePath,
        [string]$OutputPath
    )

    $lines = Get-Content -LiteralPath $LicensePath -Encoding utf8
    $body = ConvertTo-RtfParagraphs -Lines $lines

    $rtf = @"
{\rtf1\ansi\ansicpg932\deff0\deflang1041
{\fonttbl{\f0\fnil\fcharset128 MS UI Gothic;}{\f1\fswiss\fcharset0 Segoe UI;}}
\viewkind4\uc1\pard\sa200\sl276\slmult1
{\pard\qc\b\f1\fs28 ClipRefiner \u20351?\u29992?\u35377?\u35531?\u22865?\u32004?\b0\par}
{\pard\qc\f1\fs20 License Agreement\par}
\pard\par
\f0\fs22
$body
\par
}
"@

    [System.IO.File]::WriteAllText($OutputPath, $rtf, [System.Text.Encoding]::ASCII)
}

Invoke-ScriptMain {
    . (Join-Path $PSScriptRoot '..\common\cd-project-root.ps1')

    Add-Type -AssemblyName System.Drawing

    $wixDir = Join-Path $ProjectRoot 'wix'
    if (-not (Test-Path -LiteralPath $wixDir)) {
        New-Item -ItemType Directory -Path $wixDir | Out-Null
    }

    $bannerPath = Join-Path $wixDir 'Banner.bmp'
    $dialogPath = Join-Path $wixDir 'Dialog.bmp'
    $eulaPath = Join-Path $wixDir 'Eula.rtf'
    $licensePath = Join-Path $ProjectRoot 'LICENSE'

    if (-not (Test-Path -LiteralPath $licensePath)) {
        throw "LICENSE が見つかりません: $licensePath"
    }

    Write-Host 'WiX インストーラー用アセットを生成しています...'
    New-BannerBitmap -OutputPath $bannerPath
    New-DialogBitmap -OutputPath $dialogPath
    New-EulaRtf -LicensePath $licensePath -OutputPath $eulaPath

    Write-Host "  $bannerPath"
    Write-Host "  $dialogPath"
    Write-Host "  $eulaPath"
    Write-Host '完了'
}
