use std::sync::Arc;

use tao::event_loop::EventLoopProxy;

use super::message::SettingsIpcAction;
use super::queue::enqueue_settings_patch;
use crate::tray::dispatch;
use crate::tray::state::{
    AppEvent, AppState, ClipEditRequest, ClipsEvent, FavoritesEvent, RegexPreviewRequest,
    SettingsEvent,
};
use crate::user_error;

// ======================================================================
// IPC ディスパッチ
// ======================================================================
/// IPC アクションを `AppEvent` へ変換して送信する
///
/// 設定ウィンドウ内で完結する操作 (`Apply` / データ削除 / 設定ファイル起動) は
/// 専用の `SettingsEvent` としてイベントループへ送り、`user_event` 側の共通ハンドラへ委譲する
pub(crate) fn dispatch_settings_ipc(
    proxy: &EventLoopProxy<AppEvent>,
    state: &Arc<AppState>,
    action: SettingsIpcAction,
) {
    match action {
        SettingsIpcAction::Close => {
            dispatch::send_app_event(proxy, SettingsEvent::Hide.into());
        }
        SettingsIpcAction::OpenConfigFile => {
            dispatch::send_app_event(proxy, SettingsEvent::OpenConfigFile.into());
        }
        SettingsIpcAction::OpenDocument(document) => {
            dispatch::send_app_event(proxy, SettingsEvent::OpenDocument(document).into());
        }
        SettingsIpcAction::Apply(patch) => {
            if enqueue_settings_patch(state, &patch) {
                dispatch::send_app_event(proxy, SettingsEvent::ApplyPatch.into());
            } else {
                user_error::settings_patch_enqueue_failure().notify();
            }
        }
        SettingsIpcAction::ClearHistory => {
            dispatch::send_app_event(proxy, SettingsEvent::ClearHistory.into());
        }
        SettingsIpcAction::ClearContinuousRecord => {
            dispatch::send_app_event(proxy, SettingsEvent::ClearContinuousRecord.into());
        }
        SettingsIpcAction::ClipCopy(target) => {
            dispatch::send_app_event(proxy, ClipsEvent::RequestCopy(target).into());
        }
        SettingsIpcAction::ClipDelete(target) => {
            dispatch::send_app_event(proxy, ClipsEvent::RequestDelete(target).into());
        }
        SettingsIpcAction::ClipMove(target, direction) => {
            dispatch::send_app_event(proxy, ClipsEvent::RequestMove(target, direction).into());
        }
        SettingsIpcAction::ClipTemplate(target, enabled) => {
            dispatch::send_app_event(
                proxy,
                ClipsEvent::RequestSetTemplate(target, enabled).into(),
            );
        }
        SettingsIpcAction::ClipCounterReset(target) => {
            dispatch::send_app_event(proxy, ClipsEvent::RequestResetCounter(target).into());
        }
        SettingsIpcAction::ClipEditOpen(target) => {
            dispatch::send_app_event(proxy, ClipsEvent::RequestEditOpen(target).into());
        }
        SettingsIpcAction::ClipEdit(target, label, text) => {
            state.settings_pending().push_clip_edit(ClipEditRequest {
                target,
                label,
                text: text.0,
            });
            dispatch::send_app_event(proxy, ClipsEvent::RequestEditApply.into());
        }
        SettingsIpcAction::ClipRegister => {
            dispatch::send_app_event(proxy, ClipsEvent::RequestRegister.into());
        }
        SettingsIpcAction::FavoriteToggle(mode) => {
            dispatch::send_app_event(proxy, FavoritesEvent::RequestToggle(mode).into());
        }
        SettingsIpcAction::FavoriteMove(mode, direction) => {
            dispatch::send_app_event(proxy, FavoritesEvent::RequestMove(mode, direction).into());
        }
        SettingsIpcAction::RegexPreview(request_id, settings, sample) => {
            state
                .settings_pending()
                .set_regex_preview_request(RegexPreviewRequest {
                    request_id,
                    settings,
                    sample,
                });
            dispatch::send_app_event(proxy, SettingsEvent::RegexPreview.into());
        }
        SettingsIpcAction::CheckUpdate => {
            dispatch::send_app_event(proxy, SettingsEvent::CheckUpdate.into());
        }
        SettingsIpcAction::ApplyUpdate => {
            dispatch::send_app_event(proxy, SettingsEvent::ApplyUpdate.into());
        }
    }
}
