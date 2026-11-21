@echo off
REM ========================================================================
REM Script Name : format.bat
REM Description : フォーマット整形スクリプト
REM Usage       : format.bat
REM ========================================================================
setlocal

REM カレントディレクトリをプロジェクトのルートに変更
set SCRIPT_DIR=%~dp0
call "%SCRIPT_DIR%cd-project-root.bat"

echo フォーマットを整形しています...
echo.
call cargo fmt

if %errorlevel% neq 0 (
    echo.
    echo [ERROR] コードのフォーマット整形に失敗しました!
    pause
    exit /b %errorlevel%
)

echo.
echo すべてのフォーマット整形が成功しました!
pause
