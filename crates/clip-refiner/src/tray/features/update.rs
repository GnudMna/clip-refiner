use std::sync::Arc;
use std::thread;

use clip_refiner_updater::{UpdateError, UpdaterSession, UpdaterSessionEnd, launch_updater};

use crate::config;
use crate::tray::dispatch;
use crate::tray::state::{AppState, UpdateEvent};
use crate::user_error::UserNotification;

// ======================================================================
// 更新プログラムの起動
// ======================================================================
/// 更新プログラムを起動し、終了の依頼を別スレッドで待つ
///
/// 更新の確認から結果の表示までは、更新プログラムの画面で行う
/// 起動できなかった場合だけ、アプリの通知で知らせる
///
/// # Arguments
/// * `state` - アプリケーション共有状態
pub(crate) fn start_updater(state: &Arc<AppState>) {
    let session = match launch() {
        Ok(session) => session,
        Err(err) => {
            crate::log_warn!("更新プログラムの起動に失敗: {err}");
            UserNotification::from(err).notify();
            return;
        }
    };

    let proxy = state.proxy.clone();
    thread::spawn(move || {
        if session.wait() == UpdaterSessionEnd::ExitRequested {
            crate::log_info!("更新のためアプリを終了します");
            dispatch::send_app_event(&proxy, UpdateEvent::ExitRequested.into());
        }
    });
}

/// 実行フォルダの更新プログラムを起動する
fn launch() -> Result<UpdaterSession, UpdateError> {
    let exe = std::env::current_exe().map_err(|_| UpdateError::ExePath)?;
    let install_dir = exe.parent().ok_or(UpdateError::ExeDir)?;
    let config_dir = config::get_config_dir().map_err(|err| {
        crate::log_warn!("ログの出力先を特定できません: {err}");
        UpdateError::UpdaterLaunch
    })?;
    launch_updater(install_dir, &config_dir.join("logs"))
}
