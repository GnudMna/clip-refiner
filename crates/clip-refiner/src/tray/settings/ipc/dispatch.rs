use std::sync::Arc;

use super::message::SettingsIpcAction;
use super::queue::enqueue_settings_patch;

use crate::tray::dispatch;
use crate::tray::state::{AppEvent, AppState, RegexPreviewRequest};
use crate::user_error;

use tao::event_loop::EventLoopProxy;

// ======================================================================
// IPC ディスパッチ
// ======================================================================
/// IPC アクションを `AppEvent` へ変換して送信する
///
/// 設定ウィンドウ内で完結する操作 (`Apply` / データ削除 / 設定ファイル起動) は
/// 専用の `AppEvent` としてイベントループへ送り、`user_event` 側の共通ハンドラへ委譲する
pub(crate) fn dispatch_settings_ipc(
    proxy: &EventLoopProxy<AppEvent>,
    state: &Arc<AppState>,
    action: SettingsIpcAction,
) {
    match action {
        SettingsIpcAction::Close => {
            dispatch::send_app_event(proxy, AppEvent::HideSettingsWindow);
        }
        SettingsIpcAction::OpenConfigFile => {
            dispatch::send_app_event(proxy, AppEvent::SettingsOpenConfigFile);
        }
        SettingsIpcAction::Apply(patch) => {
            if enqueue_settings_patch(state, &patch) {
                dispatch::send_app_event(proxy, AppEvent::SettingsApplyPatch);
            } else {
                user_error::settings_patch_enqueue_failure().notify();
            }
        }
        SettingsIpcAction::ClearHistory => {
            dispatch::send_app_event(proxy, AppEvent::SettingsClearHistory);
        }
        SettingsIpcAction::ClearContinuousRecord => {
            dispatch::send_app_event(proxy, AppEvent::SettingsClearContinuousRecord);
        }
        SettingsIpcAction::ClipCopy(index) => {
            dispatch::send_app_event(proxy, AppEvent::RequestClipCopy(index));
        }
        SettingsIpcAction::ClipDelete(index) => {
            dispatch::send_app_event(proxy, AppEvent::RequestClipDelete(index));
        }
        SettingsIpcAction::ClipRegister => {
            dispatch::send_app_event(proxy, AppEvent::RequestClipRegister);
        }
        SettingsIpcAction::FavoriteToggle(mode) => {
            dispatch::send_app_event(proxy, AppEvent::RequestFavoriteToggle(mode));
        }
        SettingsIpcAction::FavoriteMove(mode, direction) => {
            dispatch::send_app_event(proxy, AppEvent::RequestFavoriteMove(mode, direction));
        }
        SettingsIpcAction::RegexPreview(request_id, settings, sample) => {
            state
                .settings_pending()
                .set_regex_preview_request(RegexPreviewRequest {
                    request_id,
                    settings,
                    sample,
                });
            dispatch::send_app_event(proxy, AppEvent::SettingsRegexPreview);
        }
        SettingsIpcAction::CheckUpdate => {
            dispatch::send_app_event(proxy, AppEvent::SettingsCheckUpdate);
        }
        SettingsIpcAction::ApplyUpdate => {
            dispatch::send_app_event(proxy, AppEvent::SettingsApplyUpdate);
        }
    }
}
