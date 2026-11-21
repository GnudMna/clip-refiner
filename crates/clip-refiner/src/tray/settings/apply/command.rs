use std::sync::Arc;

use super::feedback::SettingsApplyFeedback;
use super::patch::apply_settings_patch;
use super::refresh::refresh_settings_window;
use crate::platform::bundled_docs::{BundledDocument, open_bundled_document};
use crate::tray::dispatch;
use crate::tray::menu::TrayMenu;
use crate::tray::selector::QuickSelectorWindow;
use crate::tray::settings::SettingsWindow;
use crate::tray::settings::ipc::SettingsPatch;
use crate::tray::state::AppState;
use crate::tray::worker::ClipboardWorkerHandle;
use crate::user_error::{UserNotification, titles};

// ======================================================================
// ウィンドウ内コマンド
// ======================================================================
/// 設定ウィンドウ内で完結する操作
///
/// クリップ操作やお気に入り変更などは `ipc::dispatch_settings_ipc` 経由で
/// 共通の `SettingsEvent` ハンドラへ委譲する
pub(crate) enum SettingsWindowCommand {
    /// 部分更新パッチを適用する
    Apply(Box<SettingsPatch>),
    /// 履歴を全削除する
    ClearHistory,
    /// 連続記録を全削除する
    ClearContinuousRecord,
    /// 設定ファイルを既定アプリで開く
    OpenConfigFile,
    /// 同梱文書を既定アプリで開く
    OpenDocument(BundledDocument),
}

/// 設定ウィンドウ内コマンドを処理する
pub(crate) fn handle_settings_window_command(
    command: SettingsWindowCommand,
    state: &Arc<AppState>,
    menu: &TrayMenu,
    settings: &SettingsWindow,
    quick_selector: Option<&QuickSelectorWindow>,
    clipboard_worker: &ClipboardWorkerHandle,
) {
    match command {
        SettingsWindowCommand::Apply(patch) => {
            apply_settings_patch(
                &patch,
                state,
                menu,
                settings,
                quick_selector,
                clipboard_worker,
            );
        }
        SettingsWindowCommand::ClearHistory => {
            state.clipboard_session().clear_history();
            state.save_config();
            if let Err(err) = menu.refresh_history(state) {
                dispatch::log_menu_operation_error("履歴メニューの更新", err);
            }
            refresh_settings_window(state, settings);
            settings.notify_apply_feedback(&SettingsApplyFeedback::ok("履歴をクリアしました"));
        }
        SettingsWindowCommand::ClearContinuousRecord => {
            state.clipboard_session().clear_continuous_record();
            state.save_config();
            if let Err(err) = menu.refresh_continuous_record(state) {
                dispatch::log_menu_operation_error("連続記録メニューの更新", err);
            }
            refresh_settings_window(state, settings);
            settings.notify_apply_feedback(&SettingsApplyFeedback::ok("連続記録をクリアしました"));
        }
        SettingsWindowCommand::OpenConfigFile => {
            state.save_config();
            if let Err(e) = crate::config::open_config_file() {
                UserNotification::static_message(
                    titles::SETTINGS,
                    "設定ファイルを開けませんでした",
                )
                .notify_with_debug(&e);
                settings.notify_apply_feedback(
                    &SettingsApplyFeedback::error(None, "設定ファイルを開けませんでした")
                        .with_desktop_already_notified(),
                );
            }
        }
        SettingsWindowCommand::OpenDocument(document) => {
            if let Err(e) = open_bundled_document(document) {
                UserNotification::static_message(titles::SETTINGS, "文書を開けませんでした")
                    .notify_with_debug(&e);
                settings.notify_apply_feedback(
                    &SettingsApplyFeedback::error(None, "文書を開けませんでした")
                        .with_desktop_already_notified(),
                );
            }
        }
    }
}
