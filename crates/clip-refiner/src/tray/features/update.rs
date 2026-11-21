use std::sync::Arc;
use std::thread;

use crate::platform;
use crate::platform::update::{
    UpdateApplyOutcome, UpdateApplyUiResponse, UpdateCheckSource, UpdateCheckUiResponse,
    apply_update, check_for_update,
};
use crate::tray::dispatch;
use crate::tray::settings::SettingsWindow;
use crate::tray::state::{AppEvent, AppState};
use crate::user_error::titles;

// ======================================================================
// 更新操作の起動
// ======================================================================
/// バックグラウンドで更新確認を開始する
///
/// # Arguments
/// * `state` - アプリケーション共有状態
/// * `source` - 起動元 (設定 / トレイ)
/// * `settings` - 設定ウィンドウ (ビジー応答の即時通知用。トレイからは `None`)
///
/// # Returns
/// * `bool` - スレッドを起動できた場合は `true`
pub(crate) fn start_update_check(
    state: &Arc<AppState>,
    source: UpdateCheckSource,
    settings: Option<&SettingsWindow>,
) -> bool {
    if !state.settings_pending().try_begin_update_op() {
        if matches!(source, UpdateCheckSource::Settings)
            && let Some(settings) = settings
        {
            settings.notify_update_check(&UpdateCheckUiResponse::Busy);
        }
        if matches!(source, UpdateCheckSource::Tray) {
            platform::show_notification(titles::UPDATE, "更新確認はすでに実行中です");
        }
        return false;
    }

    state.settings_pending().set_update_check_source(source);
    let state = Arc::clone(state);
    thread::spawn(move || {
        let outcome = check_for_update();
        state.settings_pending().set_update_check_result(outcome);
        state.settings_pending().end_update_op();
        dispatch::send_app_event(&state.proxy, AppEvent::UpdateCheckFinished);
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
        dispatch::send_app_event(&state.proxy, AppEvent::UpdateApplyFinished);
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
    let source = state
        .settings_pending()
        .take_update_check_source()
        .unwrap_or(UpdateCheckSource::Settings);
    let Some(response) = state.settings_pending().take_update_check_ui_response() else {
        return;
    };

    if settings.is_visible() {
        settings.notify_update_check(&response);
    }

    if matches!(source, UpdateCheckSource::Tray) {
        notify_tray_check_result(&response);
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
            platform::show_notification(
                titles::UPDATE,
                "インストーラーを起動しました。アプリを終了します",
            );
            true
        }
        UpdateApplyOutcome::PortableExtracted {
            helper_launched: true,
            ..
        } => {
            platform::show_notification(
                titles::UPDATE,
                "差し替えを開始しました。アプリを終了し、新しい版を自動で起動します",
            );
            true
        }
        UpdateApplyOutcome::PortableExtracted {
            path,
            helper_launched: false,
        } => {
            platform::show_notification(
                titles::UPDATE,
                &format!(
                    "展開しました。アプリを終了し、次のフォルダの内容で差し替えて再起動してください: {path}"
                ),
            );
            false
        }
        UpdateApplyOutcome::Error { message } => {
            platform::show_notification(titles::UPDATE, message);
            false
        }
    }
}

/// トレイ起点の更新確認結果をデスクトップ通知する
fn notify_tray_check_result(response: &UpdateCheckUiResponse) {
    match response {
        UpdateCheckUiResponse::UpToDate { latest_version, .. } => {
            platform::show_notification(titles::UPDATE, &format!("最新版です (v{latest_version})"));
        }
        UpdateCheckUiResponse::Available { latest_version, .. } => {
            platform::show_notification(
                titles::UPDATE,
                &format!("v{latest_version} が利用可能です。設定の一般タブから適用できます"),
            );
        }
        UpdateCheckUiResponse::Error { message } => {
            platform::show_notification(titles::UPDATE, message);
        }
        UpdateCheckUiResponse::Busy => {
            platform::show_notification(titles::UPDATE, "更新確認はすでに実行中です");
        }
    }
}
