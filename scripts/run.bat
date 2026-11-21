@echo off
REM ========================================================================
REM Script Name : run.bat
REM Description : Windows用の実行スクリプト
REM Usage       : run.bat
REM ========================================================================

REM カレントディレクトリをプロジェクトのルートに変更
set SCRIPT_DIR=%~dp0
call "%SCRIPT_DIR%cd-project-root.bat"

echo 実行しています...
echo.
call cargo run

if %errorlevel% neq 0 (
    echo.
    echo [ERROR] 実行中にエラーが発生しました。
    pause
    exit /b %errorlevel%
)
