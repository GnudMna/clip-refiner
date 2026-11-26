//! ポータブル更新の差し替え
//!
//! 更新プログラムは `install_portable` で、展開済みの配布物を実行フォルダへ反映する
//! `run_portable_replace` は、更新プログラムを持たない旧版からの更新を受ける入口
//! 旧版は展開済みの新しい `ClipRefiner.exe` を `--apply-portable-update` で起動する
//! 渡されたフォルダと PID は検証してから使う

mod files;
mod process;
#[cfg(test)]
mod test_support;
mod validate;

use std::path::Path;
use std::process::Command;
use std::time::Duration;

use super::error::UpdateError;
use files::{copy_portable_payload, current_exe_dir};
use process::wait_for_pid_exit;
use validate::validate_replace_request;

pub(crate) use files::portable_exe_name;
pub(crate) use process::{process_image_path, schedule_dir_delete, show_error_box};
pub(crate) use validate::{canonicalize_dir, is_strict_descendant, update_work_root};

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
///
/// # Errors
/// 引数の検証、旧プロセスの終了待ち、ファイルの差し替え、再起動のいずれかが失敗した場合
pub fn run_portable_replace(
    wait_pid: u32,
    install_dir: &Path,
    cleanup_dir: Option<&Path>,
) -> Result<(), UpdateError> {
    match run_portable_replace_inner(wait_pid, install_dir, cleanup_dir) {
        Ok(()) => Ok(()),
        Err(err) => {
            show_error_box(&err.to_string());
            Err(err)
        }
    }
}

/// 差し替え元・先がポータブル更新の前提を満たすか検証する
///
/// # Arguments
/// * `source_dir` - 展開ルート (新しい `ClipRefiner.exe` があるフォルダ)
/// * `install_dir` - 差し替え先
///
/// # Returns
/// * `Result<(), UpdateError>` - 差し替えてよい場合は `Ok(())`
pub(crate) fn validate_portable(source_dir: &Path, install_dir: &Path) -> Result<(), UpdateError> {
    validate_replace_request(source_dir, install_dir, None)
}

/// 展開済みの配布物を実行フォルダへ反映する
///
/// アプリの終了後に呼ぶ。失敗した場合は差し替え前の状態へ戻す
///
/// # Arguments
/// * `source_dir` - 展開ルート
/// * `install_dir` - 差し替え先
///
/// # Returns
/// * `Result<(), UpdateError>` - 成功時 `Ok(())`
pub(crate) fn install_portable(source_dir: &Path, install_dir: &Path) -> Result<(), UpdateError> {
    validate_portable(source_dir, install_dir)?;
    copy_portable_payload(source_dir, install_dir)
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
        tracing::warn!("{update_err}");
        return Err(update_err);
    }

    if let Some(dir) = cleanup_dir {
        schedule_dir_delete(dir);
    }

    Ok(())
}
