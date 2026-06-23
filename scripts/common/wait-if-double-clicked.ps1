# ========================================================================
# Script Name : wait-if-double-clicked.ps1
# Description : エクスプローラーからのダブルクリック実行時にウィンドウを開いたままにする
# Usage       : . (Join-Path $PSScriptRoot '..\common\wait-if-double-clicked.ps1')
#               Invoke-ScriptMain { ... }
# Requires    : PowerShell 7.0+
# ========================================================================

#requires -Version 7.0

# エクスプローラー経由の起動かどうかを親プロセスチェーンで判定する
function Test-LaunchedFromExplorer {
    try {
        $process = Get-CimInstance Win32_Process -Filter "ProcessId=$PID" -ErrorAction Stop
        $depth = 0

        while ($process -and $depth -lt 12) {
            $name = $process.Name.ToLowerInvariant()
            if ($name -eq 'explorer.exe' -or $name -eq 'openwith.exe') {
                return $true
            }

            if ($process.ParentProcessId -eq 0) {
                break
            }

            $process = Get-CimInstance Win32_Process -Filter "ProcessId=$($process.ParentProcessId)" -ErrorAction SilentlyContinue
            $depth++
        }

        return $false
    } catch {
        return $false
    }
}

# ダブルクリック実行時のみ Enter 待ちでウィンドウを開いたままにする
function Wait-IfDoubleClicked {
    if (-not (Test-LaunchedFromExplorer)) {
        return
    }

    Write-Host ''
    Read-Host '終了するには Enter キーを押してください'
}

# スクリプト本体を実行し、エラー時も Enter 待ち後に静かに終了する
function Invoke-ScriptMain {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)]
        [scriptblock]$Body
    )

    $exitCode = 0

    try {
        & $Body

        if ($LASTEXITCODE -ne 0) {
            $exitCode = $LASTEXITCODE
        }
    } catch {
        $exitCode = 1
        Write-Host ''

        if ($_.Exception.Message) {
            Write-Host $_.Exception.Message
        } else {
            Write-Host $_
        }
    } finally {
        # catch でエラーを処理済みのため、finally 後の再スローを避けてここで待機する
        Wait-IfDoubleClicked
    }

    if ($exitCode -ne 0) {
        exit $exitCode
    }
}
