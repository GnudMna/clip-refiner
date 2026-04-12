@echo off
REM ========================================================================
REM Script Name : cd-project-root.bat
REM Description : プロジェクトのルートディレクトリに移動するバッチファイル
REM Usage       : cd-project-root.bat
REM ========================================================================

REM スクリプトのディレクトリを取得
set SCRIPT_DIR=%~dp0

REM プロジェクトのルートディレクトリを計算
for %%I in ("%SCRIPT_DIR%..") do set PROJECT_ROOT=%%~fI

REM プロジェクトのルートディレクトリに移動
cd /d "%PROJECT_ROOT%"
