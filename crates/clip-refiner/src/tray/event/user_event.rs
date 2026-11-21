use super::super::dispatch;
use super::super::hotkey::{HotkeyEventContext, notify_registration_failures};
use super::super::settings::{
    refresh_settings_clips, refresh_settings_favorites_and_shortcuts, refresh_settings_window,
    refresh_settings_window_after_external_reload,
};
use super::super::state::AppEvent;
use super::super::worker::ClipboardCommand;
use super::context::AppEventContext;
use super::{
    copy_registered_clip, delete_registered_clip, move_favorite_mode, refresh_clips_views,
    reload_config_with_notification, toggle_favorite_mode, update_refine,
};

use crate::config::FavoriteMoveDirection;
use crate::refiner::RefineMode;

// ======================================================================
// ユーザーイベント処理
// ======================================================================
/// アプリケーション独自のユーザーイベント (`AppEvent`) を処理する
pub(crate) fn handle_user_event(ctx: &mut AppEventContext<'_>, event: AppEvent) {
    match event {
        AppEvent::RequestModeChange(mode) => {
            ctx.selectors.quick.hide();
            update_refine(ctx.state, ctx.menu, ctx.clipboard_worker, mode, None);
        }
        AppEvent::HideQuickSelector => {
            ctx.selectors.quick.hide();
        }
        AppEvent::HideClipSelector => {
            ctx.selectors.clip.hide();
        }
        AppEvent::RequestClipCopy(index) => handle_clip_copy(ctx, index),
        AppEvent::RequestClipRegister => {
            dispatch::send_clipboard_command(
                ctx.clipboard_worker,
                ClipboardCommand::RegisterClipFromClipboard,
            );
        }
        AppEvent::RequestClipDelete(index) => handle_clip_delete(ctx, index),
        AppEvent::RequestFavoriteToggle(mode) => handle_favorite_toggle(ctx, mode),
        AppEvent::RequestFavoriteMove(mode, direction) => {
            handle_favorite_move(ctx, mode, direction);
        }
        AppEvent::RefreshHistory => refresh_history_menu(ctx),
        AppEvent::RefreshContinuousRecord => refresh_continuous_record_menu(ctx),
        AppEvent::RefreshClips => {
            refresh_clips_views(ctx.state, ctx.menu, &ctx.selectors.clip);
            refresh_settings_clips(ctx.state, ctx.settings);
        }
        AppEvent::ReloadConfig => {
            reload_config_with_notification(
                ctx.state,
                ctx.menu,
                ctx.hotkey_handler,
                &ctx.selectors.clip,
            );
            refresh_settings_window_after_external_reload(ctx.state, ctx.settings);
        }
        AppEvent::RestartClipboardWorker
        | AppEvent::ClipboardWorkerStopped
        | AppEvent::ClipboardWorkerReady => handle_clipboard_worker_lifecycle(ctx, event),
        AppEvent::ReloadFavoriteHotkeys => reload_favorite_hotkeys(ctx),
        AppEvent::RefreshSettingsWindow => refresh_settings_window(ctx.state, ctx.settings),
        AppEvent::ShowSettingsWindow
        | AppEvent::HideSettingsWindow
        | AppEvent::SettingsApplyPatch
        | AppEvent::SettingsClearHistory
        | AppEvent::SettingsClearContinuousRecord
        | AppEvent::SettingsOpenConfigFile
        | AppEvent::SettingsRegexPreview => handle_settings_window_event(ctx, event),
        AppEvent::Hotkey(hotkey_event) => handle_hotkey_event(ctx, hotkey_event),
    }
}

/// 登録クリップのコピー要求を処理する
fn handle_clip_copy(ctx: &mut AppEventContext<'_>, index: usize) {
    ctx.selectors.clip.hide();
    copy_registered_clip(ctx.state, ctx.clipboard_worker, index);
}

/// 登録クリップの削除要求を処理する
fn handle_clip_delete(ctx: &mut AppEventContext<'_>, index: usize) {
    delete_registered_clip(ctx.state, ctx.menu, &ctx.selectors.clip, index);
    refresh_settings_clips(ctx.state, ctx.settings);
}

/// お気に入り変換モードの登録切替要求を処理する
fn handle_favorite_toggle(ctx: &mut AppEventContext<'_>, mode: RefineMode) {
    toggle_favorite_mode(ctx.state, ctx.menu, Some(&ctx.selectors.quick), mode);
    refresh_settings_favorites_and_shortcuts(ctx.state, ctx.settings);
}

/// お気に入り変換モードの表示順変更要求を処理する
fn handle_favorite_move(
    ctx: &mut AppEventContext<'_>,
    mode: RefineMode,
    direction: FavoriteMoveDirection,
) {
    move_favorite_mode(
        ctx.state,
        ctx.menu,
        Some(&ctx.selectors.quick),
        mode,
        direction,
    );
    refresh_settings_favorites_and_shortcuts(ctx.state, ctx.settings);
}

/// 履歴メニューの再構築要求を処理する
fn refresh_history_menu(ctx: &mut AppEventContext<'_>) {
    if let Err(err) = ctx.menu.refresh_history(ctx.state) {
        dispatch::log_menu_operation_error("履歴メニューの更新", err);
    }
}

/// 連続記録メニューの再構築要求を処理する
fn refresh_continuous_record_menu(ctx: &mut AppEventContext<'_>) {
    if let Err(err) = ctx.menu.refresh_continuous_record(ctx.state) {
        dispatch::log_menu_operation_error("連続記録メニューの更新", err);
    }
}

/// 設定ウィンドウ関連の `AppEvent` を処理する
fn handle_settings_window_event(ctx: &mut AppEventContext<'_>, event: AppEvent) {
    ctx.settings.handle_event(
        event,
        ctx.state,
        ctx.menu,
        Some(&ctx.selectors.quick),
        ctx.clipboard_worker,
    );
}

/// グローバルホットキーイベントを処理する
fn handle_hotkey_event(
    ctx: &mut AppEventContext<'_>,
    hotkey_event: global_hotkey::GlobalHotKeyEvent,
) {
    let AppEventContext {
        state,
        menu,
        selectors,
        hotkey_handler,
        clipboard_worker,
        control_flow,
        last_quick_selector_show,
        last_clip_selector_show,
        ..
    } = ctx;
    let mut hotkey_ctx = HotkeyEventContext {
        state,
        menu,
        quick_selector: Some(&selectors.quick),
        clip_selector: Some(&selectors.clip),
        ocr_capture: Some(&selectors.ocr),
        control_flow,
        last_quick_selector_show,
        last_clip_selector_show,
        clipboard_worker,
    };
    hotkey_handler.handle_event(hotkey_event, &mut hotkey_ctx);
}

/// クリップボードワーカーの再起動・停止・復旧イベントを処理する
fn handle_clipboard_worker_lifecycle(ctx: &mut AppEventContext<'_>, event: AppEvent) {
    match event {
        AppEvent::RestartClipboardWorker => ctx.clipboard_worker.restart(),
        AppEvent::ClipboardWorkerStopped => {
            ctx.clipboard_worker.sync_menu_state(ctx.menu);
            crate::platform::show_notification(
                "クリップボードエラー",
                super::super::worker::worker_stopped_notification_body(),
            );
        }
        AppEvent::ClipboardWorkerReady => {
            ctx.clipboard_worker.sync_menu_state(ctx.menu);
            crate::platform::show_notification(
                "クリップボード監視",
                "クリップボード監視を再開しました",
            );
        }
        _ => {}
    }
}

/// お気に入りホットキーを再登録する
fn reload_favorite_hotkeys(ctx: &mut AppEventContext<'_>) {
    let (hotkeys, favorite_count) = ctx
        .state
        .with_config(|c| (c.hotkeys.clone(), c.favorite_modes.len()));
    match ctx
        .hotkey_handler
        .reload_favorite_slots(&hotkeys, favorite_count)
    {
        Ok(failures) => notify_registration_failures(&failures),
        Err(err) => {
            crate::log_warn!("お気に入りホットキーの再登録に失敗: {}", err);
        }
    }
    refresh_settings_favorites_and_shortcuts(ctx.state, ctx.settings);
}
