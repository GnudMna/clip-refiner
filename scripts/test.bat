@echo off
REM ========================================================================
REM Script Name : test.bat
REM Description : Windows用のテスト実行スクリプト
REM Usage       : test.bat
REM ========================================================================
setlocal

REM カレントディレクトリをプロジェクトのルートに変更
set SCRIPT_DIR=%~dp0
call "%SCRIPT_DIR%cd-project-root.bat"

echo テストを実行しています...
echo.
call cargo test

if %errorlevel% neq 0 (
    echo.
    echo [ERROR] テストが失敗しました。
    pause
    exit /b %errorlevel%
)

echo.
echo すべてのテストが成功しました!
pause
