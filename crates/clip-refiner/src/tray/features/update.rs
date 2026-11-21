use std::sync::Arc;
use std::thread;

use crate::platform::update::{
    UpdateApplyOutcome, UpdateApplyUiResponse, UpdateCheckUiResponse, apply_update,
    check_for_update,
};
use crate::tray::dispatch;
use crate::tray::settings::SettingsWindow;
use crate::tray::state::{AppState, UpdateEvent};
use crate::user_error::{UserNotification, titles};

// ======================================================================
// 更新操作の起動
// ======================================================================
/// バックグラウンドで更新確認を開始する
///
/// # Arguments
/// * `state` - アプリケーション共有状態
/// * `settings` - 設定ウィンドウ (ビジー応答の即時通知用)
///
/// # Returns
/// * `bool` - スレッドを起動できた場合は `true`
pub(crate) fn start_update_check(state: &Arc<AppState>, settings: &SettingsWindow) -> bool {
    if !state.settings_pending().try_begin_update_op() {
        settings.notify_update_check(&UpdateCheckUiResponse::Busy);
        return false;
    }

    let state = Arc::clone(state);
    thread::spawn(move || {
        let outcome = check_for_update();
        state.settings_pending().set_update_check_result(outcome);
        state.settings_pending().end_update_op();
        dispatch::send_app_event(&state.proxy, UpdateEvent::CheckFinished.into());
    });
    true
}

/// バックグラウンドで更新適用を開始する
///
/// # Arguments
/// * `state` - アプリケーション共有状態
/// * `settings` - 設定ウィンドウ
///
/// # Returns
/// * `bool` - スレッドを起動できた場合は `true`
pub(crate) fn start_update_apply(state: &Arc<AppState>, settings: &SettingsWindow) -> bool {
    if !state.settings_pending().try_begin_update_op() {
        settings.notify_update_apply(&UpdateApplyUiResponse::Busy {
            message: "更新処理はすでに実行中です".to_string(),
        });
        return false;
    }

    let Some(update) = state.settings_pending().take_available_update() else {
        state.settings_pending().end_update_op();
        settings.notify_update_apply(&UpdateApplyUiResponse::Error {
            message: "適用できる更新がありません。先に更新を確認してください".to_string(),
        });
        return false;
    };

    let state = Arc::clone(state);
    thread::spawn(move || {
        let outcome = apply_update(&update);
        state.settings_pending().set_update_apply_result(outcome);
        state.settings_pending().end_update_op();
        dispatch::send_app_event(&state.proxy, UpdateEvent::ApplyFinished.into());
    });
    true
}

// ======================================================================
// 完了処理
// ======================================================================
/// 更新確認完了イベントを処理する
///
/// # Arguments
/// * `state` - アプリケーション共有状態
/// * `settings` - 設定ウィンドウ
pub(crate) fn handle_update_check_finished(state: &Arc<AppState>, settings: &SettingsWindow) {
    let Some(response) = state.settings_pending().take_update_check_ui_response() else {
        return;
    };

    if settings.is_visible() {
        settings.notify_update_check(&response);
    }
}

/// 更新適用完了イベントを処理する
///
/// # Arguments
/// * `state` - アプリケーション共有状態
/// * `settings` - 設定ウィンドウ
///
/// # Returns
/// * `bool` - MSI / ポータブル差し替えヘルパー起動成功でアプリ終了が必要なとき `true`
pub(crate) fn handle_update_apply_finished(
    state: &Arc<AppState>,
    settings: &SettingsWindow,
) -> bool {
    let Some(outcome) = state.settings_pending().take_update_apply_result() else {
        return false;
    };
    let response = UpdateApplyUiResponse::from(&outcome);
    if settings.is_visible() {
        settings.notify_update_apply(&response);
    }

    match &outcome {
        UpdateApplyOutcome::MsiStarted => {
            UserNotification::static_message(
                titles::UPDATE,
                "インストーラーを起動しました。アプリを終了します",
            )
            .notify();
            true
        }
        UpdateApplyOutcome::PortableExtracted {
            helper_launched: true,
            ..
        } => {
            UserNotification::static_message(
                titles::UPDATE,
                "差し替えを開始しました。アプリを終了し、新しい版を自動で起動します",
            )
            .notify();
            true
        }
        UpdateApplyOutcome::PortableExtracted {
            path,
            helper_launched: false,
        } => {
            UserNotification::new(
                titles::UPDATE,
                format!(
                    "展開しました。アプリを終了し、次のフォルダの内容で差し替えて再起動してください: {path}"
                ),
            )
            .notify();
            false
        }
        UpdateApplyOutcome::Error { message } => {
            UserNotification::new(titles::UPDATE, message.clone()).notify();
            false
        }
    }
}
