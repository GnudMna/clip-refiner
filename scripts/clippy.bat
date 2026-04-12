@echo off
REM ========================================================================
REM Script Name : clippy.bat
REM Description : Clippy (Rustの静的解析ツール)を実行するスクリプト
REM Usage       : clippy.bat
REM ========================================================================
setlocal

REM カレントディレクトリをプロジェクトのルートに変更
set SCRIPT_DIR=%~dp0
call "%SCRIPT_DIR%cd-project-root.bat"

echo Clippyを実行しています...
echo.
call cargo clippy --fix --bin "ClipRefiner" -p clip-refiner -- -D warnings

if %errorlevel% neq 0 (
    echo.
    echo [ERROR] Clippyの実行に失敗しました!
    pause
    exit /b %errorlevel%
)

echo.
echo すべてのClippyチェックが成功しました!
pause
