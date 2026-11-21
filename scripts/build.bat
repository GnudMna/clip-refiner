@echo off
REM ========================================================================
REM Script Name : build.bat
REM Description : Windows用のビルドスクリプト
REM Usage       : build.bat
REM ========================================================================
setlocal

REM カレントディレクトリをプロジェクトのルートに変更
set SCRIPT_DIR=%~dp0
call "%SCRIPT_DIR%cd-project-root.bat"

echo Cargoビルドを実行しています...
echo.
call cargo build -r

if %errorlevel% neq 0 (
    echo.
    echo [ERROR] ビルドに失敗しました!
    pause
    exit /b %errorlevel%
)

echo.
echo ビルドが完了しました!
echo 出力先: target\release\
pause
