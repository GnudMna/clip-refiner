//! 常駐アプリから更新プログラムを起動し、終了の依頼を受け取る
//!
//! 更新プログラムは自身も差し替え対象になるため、`%TEMP%` へコピーして起動する
//! 終了の依頼と起動完了の通知は、名前付きイベントで受け渡す

use std::ffi::OsString;
use std::fs;
use std::os::windows::io::AsHandle;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::time::{SystemTime, UNIX_EPOCH};

use clip_refiner_core::consts::APP_ID;

use super::error::UpdateError;
use super::named_event::{self, EventWait, NamedEvent};
use super::replace::update_work_root;

// ======================================================================
// 定数
// ======================================================================
/// 更新プログラムの実行ファイル名
pub(crate) const UPDATER_EXE_NAME: &str = "ClipRefinerUpdater.exe";
/// 更新プログラムをコピーするフォルダ名の接頭辞 (更新作業フォルダの直下)
const SESSION_DIR_PREFIX: &str = "updater-";
/// `ClipRefiner.exe` のあるフォルダを渡す引数
pub(crate) const ARG_INSTALL_DIR: &str = "--install-dir";
/// 終了を待つ常駐アプリの PID を渡す引数
pub(crate) const ARG_WAIT_PID: &str = "--wait-pid";
/// ログの出力先フォルダを渡す引数
pub(crate) const ARG_LOG_DIR: &str = "--log-dir";

// ======================================================================
// 更新プログラムの起動
// ======================================================================
/// 起動中の更新プログラムとの接続
pub struct UpdaterSession {
    exit_request: NamedEvent,
    updater: Child,
}

/// 更新プログラムとの接続の終わり方
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdaterSessionEnd {
    /// アプリの終了を依頼された
    ExitRequested,
    /// 終了を依頼せずに更新プログラムが閉じた
    UpdaterClosed,
}

impl UpdaterSession {
    /// アプリの終了を依頼されるか、更新プログラムが閉じるまで待つ
    ///
    /// # Returns
    /// * `UpdaterSessionEnd` - 先に起きたもの
    pub fn wait(self) -> UpdaterSessionEnd {
        match self
            .exit_request
            .wait_with_process(self.updater.as_handle(), None)
        {
            Ok(EventWait::Signaled) => UpdaterSessionEnd::ExitRequested,
            Ok(EventWait::ProcessExited | EventWait::TimedOut) => UpdaterSessionEnd::UpdaterClosed,
            Err(err) => {
                tracing::warn!("更新プログラムの待機に失敗: {err}");
                UpdaterSessionEnd::UpdaterClosed
            }
        }
    }
}

/// 更新プログラムを起動する
///
/// 実行フォルダの `ClipRefinerUpdater.exe` を `%TEMP%` へコピーして起動する
///
/// # Arguments
/// * `install_dir` - `ClipRefiner.exe` のあるフォルダ
/// * `log_dir` - ログの出力先フォルダ
///
/// # Returns
/// * `Result<UpdaterSession, UpdateError>` - 起動した更新プログラムとの接続
///
/// # Errors
/// 更新プログラムが無い、コピーできない、起動できない場合
pub fn launch_updater(install_dir: &Path, log_dir: &Path) -> Result<UpdaterSession, UpdateError> {
    let source = install_dir.join(UPDATER_EXE_NAME);
    if !source.is_file() {
        tracing::warn!("更新プログラムが見つからない: {}", source.display());
        return Err(UpdateError::UpdaterMissing);
    }

    let pid = std::process::id();
    // 更新プログラムより先に作る。自動リセットにし、応じられなかった依頼を次回へ残さない
    let exit_request = NamedEvent::create(&exit_request_event_name(pid), false).map_err(|err| {
        tracing::warn!("終了依頼イベントの作成に失敗: {err}");
        UpdateError::UpdaterLaunch
    })?;

    let session_dir = create_session_dir(pid)?;
    let exe = session_dir.join(UPDATER_EXE_NAME);
    fs::copy(&source, &exe).map_err(|err| {
        tracing::warn!("更新プログラムのコピーに失敗: {err}");
        UpdateError::UpdaterLaunch
    })?;

    let updater = Command::new(&exe)
        .args(updater_args(install_dir, pid, log_dir))
        .current_dir(&session_dir)
        .spawn()
        .map_err(|err| {
            tracing::warn!("更新プログラムの起動に失敗: {err}");
            UpdateError::UpdaterLaunch
        })?;

    Ok(UpdaterSession {
        exit_request,
        updater,
    })
}

/// 更新プログラムのコピー先フォルダを作る
fn create_session_dir(pid: u32) -> Result<PathBuf, UpdateError> {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_nanos());
    let dir = update_work_root().join(format!("{SESSION_DIR_PREFIX}{pid}-{nanos}"));
    fs::create_dir_all(&dir).map_err(|err| {
        tracing::warn!("更新プログラムのコピー先の作成に失敗: {err}");
        UpdateError::WorkDirCreate
    })?;
    Ok(dir)
}

/// 更新プログラムへ渡す引数を組み立てる
pub(crate) fn updater_args(install_dir: &Path, pid: u32, log_dir: &Path) -> Vec<OsString> {
    vec![
        ARG_INSTALL_DIR.into(),
        install_dir.into(),
        ARG_WAIT_PID.into(),
        pid.to_string().into(),
        ARG_LOG_DIR.into(),
        log_dir.into(),
    ]
}

// ======================================================================
// イベント
// ======================================================================
/// 常駐の起動完了を更新プログラムへ知らせる
///
/// 更新プログラムが待っていなければ何もしない
pub fn notify_startup_complete() {
    if named_event::signal(&startup_complete_event_name()).is_ok() {
        tracing::info!("更新プログラムへ起動完了を通知しました");
    }
}

/// 常駐アプリへ終了を依頼するイベントの名前を返す
///
/// # Arguments
/// * `pid` - 常駐アプリの PID
///
/// # Returns
/// * `String` - イベント名
pub(crate) fn exit_request_event_name(pid: u32) -> String {
    format!(r"Local\{APP_ID}.update-exit.{pid}")
}

/// 新しい版の起動完了を受け取るイベントの名前を返す
///
/// # Returns
/// * `String` - イベント名
pub(crate) fn startup_complete_event_name() -> String {
    format!(r"Local\{APP_ID}.update-started")
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;

    /// 更新プログラムが無いフォルダでは起動せずに失敗すること
    #[test]
    fn launch_updater_fails_without_updater_exe() {
        let dir = std::env::temp_dir().join(format!(
            "clip-refiner-update-test-no-updater-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("dir");

        let result = launch_updater(&dir, &dir);
        assert_eq!(result.err(), Some(UpdateError::UpdaterMissing));

        let _ = fs::remove_dir_all(&dir);
    }

    /// 待っている更新プログラムが無くても、起動完了の通知で異常にならないこと
    #[test]
    fn notify_startup_complete_ignores_missing_listener() {
        notify_startup_complete();
    }
}
