# ========================================================================
# Script Name : wait-if-double-clicked.ps1
# Description : エクスプローラーからのダブルクリック実行時にウィンドウを開いたままにする
#               終了時はカレントディレクトリと環境変数を呼び出し前の状態に戻す
# Usage       : . (Join-Path $PSScriptRoot '..\common\wait-if-double-clicked.ps1')
#               Invoke-ScriptMain { ... }
# Requires    : PowerShell 7.0+
# ========================================================================

#requires -Version 7.0

# pwsh がスクリプトファイル実行用に起動されたか (対話セッションで .\script.ps1 した場合は false)
function Test-StartedAsFileInvocation {
    $arguments = @([Environment]::GetCommandLineArgs())
    if ($arguments.Count -lt 2) {
        return $false
    }

    for ($i = 1; $i -lt $arguments.Count; $i++) {
        $argument = $arguments[$i]
        if ($argument -match '^-(File|f|Command|c|EncodedCommand)(:.*)?$') {
            return $true
        }
        if ($argument -match '\.ps1$') {
            return $true
        }
    }

    return $false
}

# 既存のターミナル / エディタホストか (自分自身の pwsh.exe は呼び出し側で除外する)
function Test-IsExistingTerminalProcess {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Name
    )

    $terminalNames = @(
        'cmd.exe',
        'pwsh.exe',
        'powershell.exe',
        'windowsterminal.exe',
        'openconsole.exe',
        'conhost.exe',
        'wt.exe',
        'code.exe',
        'cursor.exe',
        'devenv.exe'
    )

    return $terminalNames -contains $Name
}

# エクスプローラー経由の起動かどうかを親プロセスチェーンで判定する
function Test-LaunchedFromExplorer {
    try {
        # 対話ホスト内での実行は、explorer が祖先でも待たない
        if (-not (Test-StartedAsFileInvocation)) {
            return $false
        }

        $process = Get-CimInstance Win32_Process -Filter "ProcessId=$PID" -ErrorAction Stop
        $depth = 0

        while ($process -and $depth -lt 12) {
            $name = $process.Name.ToLowerInvariant()
            if ($name -eq 'explorer.exe' -or $name -eq 'openwith.exe') {
                return $true
            }

            # cmd / Windows Terminal など既存シェルの子なら、explorer が祖先でも待たない
            if ($depth -gt 0 -and (Test-IsExistingTerminalProcess -Name $name)) {
                return $false
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

# プロセスの環境変数を退避時の内容に戻す (退避後に追加された変数は削除する)
function Restore-ProcessEnvironment {
    param(
        [Parameter(Mandatory = $true)]
        [System.Collections.IDictionary]$Snapshot
    )

    # 名前の大文字小文字だけが変わった変数も、削除してから退避時の名前で設定し直す
    foreach ($name in @([Environment]::GetEnvironmentVariables('Process').Keys)) {
        if (-not $Snapshot.Contains($name)) {
            # $null は .NET の string 引数で空文字に変換され、削除されずに空の変数が残る
            [Environment]::SetEnvironmentVariable($name, [NullString]::Value, 'Process')
        }
    }

    foreach ($entry in $Snapshot.GetEnumerator()) {
        [Environment]::SetEnvironmentVariable($entry.Key, $entry.Value, 'Process')
    }
}

# スクリプト本体を実行し、エラー時も Enter 待ち後に静かに終了する
#
# 対話セッションから ./script.ps1 で実行しても呼び出し元を汚さないよう、
# 本体が変えたカレントディレクトリと環境変数 (PATH や MSVC のビルド環境を含む) は終了時に戻す
function Invoke-ScriptMain {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)]
        [scriptblock]$Body
    )

    $exitCode = 0
    # 外部コマンドを実行しない本体でも判定できるよう、未定義 (StrictMode で参照エラー) や前回の値を消しておく
    $global:LASTEXITCODE = 0

    $originalLocation = Get-Location
    $originalEnvironment = [Environment]::GetEnvironmentVariables('Process')

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
        Set-Location -LiteralPath $originalLocation.Path
        Restore-ProcessEnvironment -Snapshot $originalEnvironment
        Wait-IfDoubleClicked
    }

    if ($exitCode -ne 0) {
        exit $exitCode
    }
}
