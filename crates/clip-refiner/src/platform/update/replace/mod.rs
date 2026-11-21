//! ポータブル更新の差し替えヘルパー
//!
//! 展開済みの新しい `ClipRefiner.exe` を `--apply-portable-update` で起動し、
//! 旧プロセス終了後に実行フォルダへファイルをコピーして再起動する
//! CLI の `--install-dir` / `--cleanup-dir` / `--wait-pid` は検証してから使う

mod files;
mod process;
#[cfg(test)]
mod test_support;
mod validate;

use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use super::error::UpdateError;
use files::{
    copy_portable_payload, current_exe_dir, find_portable_exe, path_to_string, portable_exe_name,
};
use process::{schedule_dir_delete, show_replace_error, wait_for_pid_exit};
use validate::validate_replace_request;

// ======================================================================
// 定数
// ======================================================================
/// 旧プロセス終了待ちの上限
const WAIT_PID_TIMEOUT: Duration = Duration::from_mins(2);
/// コンソールウィンドウを出さないプロセス作成フラグ
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

// ======================================================================
// パブリック関数
// ======================================================================
/// ポータブル差し替えヘルパーを実行する (CLI エントリ)
///
/// 旧 PID の終了を待ち、自身のディレクトリからインストール先へファイルをコピーし、
/// 新しい常駐プロセスを起動する。`data` ディレクトリは上書きしない
///
/// # Arguments
/// * `wait_pid` - 終了を待つ旧プロセスの PID
/// * `install_dir` - 差し替え先 (現在の実行フォルダ)
/// * `cleanup_dir` - 終了後に削除する作業ディレクトリ (任意)
///
/// # Returns
/// * `Result<(), UpdateError>` - 成功時 `Ok(())`
pub(crate) fn run_portable_replace(
    wait_pid: u32,
    install_dir: &Path,
    cleanup_dir: Option<&Path>,
) -> Result<(), UpdateError> {
    match run_portable_replace_inner(wait_pid, install_dir, cleanup_dir) {
        Ok(()) => Ok(()),
        Err(err) => {
            show_replace_error(&err);
            Err(err)
        }
    }
}

/// 展開済みポータブルの差し替えヘルパーをバックグラウンド起動する
///
/// # Arguments
/// * `source_dir` - 展開ルート (新しい `ClipRefiner.exe` があるディレクトリ)
/// * `install_dir` - 差し替え先
/// * `cleanup_dir` - ヘルパー終了後に削除する作業ディレクトリ
///
/// # Returns
/// * `Result<(), UpdateError>` - 起動成功時 `Ok(())`
pub(crate) fn launch_portable_replace_helper(
    source_dir: &Path,
    install_dir: &Path,
    cleanup_dir: &Path,
) -> Result<(), UpdateError> {
    validate_replace_request(source_dir, install_dir, Some(cleanup_dir))?;
    let helper_exe = find_portable_exe(source_dir).ok_or(UpdateError::HelperExeMissing)?;

    let wait_pid = std::process::id().to_string();
    let install_dir_str = path_to_string(install_dir)?;
    let cleanup_dir_str = path_to_string(cleanup_dir)?;

    let mut command = Command::new(&helper_exe);
    command
        .args([
            "--apply-portable-update",
            "--wait-pid",
            &wait_pid,
            "--install-dir",
            &install_dir_str,
            "--cleanup-dir",
            &cleanup_dir_str,
        ])
        .current_dir(source_dir)
        .creation_flags(CREATE_NO_WINDOW);

    match command.spawn() {
        Ok(_) => Ok(()),
        Err(err) => {
            crate::log_warn!("ポータブル差し替えヘルパーの起動に失敗: {err}");
            Err(UpdateError::HelperLaunch)
        }
    }
}

/// 現在の実行ファイルがあるディレクトリを返す
///
/// # Returns
/// * `Result<PathBuf, UpdateError>` - 実行ディレクトリ
pub(crate) fn current_install_dir() -> Result<PathBuf, UpdateError> {
    current_exe_dir()
}

// ======================================================================
// 差し替え実行
// ======================================================================
/// 検証・待機・コピー・再起動・掃除予約を順に行う
fn run_portable_replace_inner(
    wait_pid: u32,
    install_dir: &Path,
    cleanup_dir: Option<&Path>,
) -> Result<(), UpdateError> {
    let source_dir = current_exe_dir()?;
    validate_replace_request(&source_dir, install_dir, cleanup_dir)?;
    wait_for_pid_exit(wait_pid, WAIT_PID_TIMEOUT)?;
    copy_portable_payload(&source_dir, install_dir)?;

    let new_exe = install_dir.join(portable_exe_name());
    if let Err(err) = Command::new(&new_exe).current_dir(install_dir).spawn() {
        let update_err = UpdateError::RestartFailed {
            detail: err.to_string(),
        };
        crate::log_warn!("{update_err}");
        return Err(update_err);
    }

    if let Some(dir) = cleanup_dir {
        schedule_dir_delete(dir);
    }

    Ok(())
}
