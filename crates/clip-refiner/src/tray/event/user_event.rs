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
        AppEvent::ReloadHotkeys => reload_hotkeys(ctx),
        AppEvent::RefreshSettingsWindow => refresh_settings_window(ctx.state, ctx.settings),
        AppEvent::DisposeOcrOverlay => ctx.selectors.ocr.hide(),
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
        ctx.event_loop,
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
        event_loop,
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
        event_loop: Some(event_loop),
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

/// グローバルホットキー全体を再登録する
fn reload_hotkeys(ctx: &mut AppEventContext<'_>) {
    let (hotkeys, favorite_modes) = ctx
        .state
        .with_config(|c| (c.hotkeys.clone(), c.favorite_modes.clone()));
    match ctx.hotkey_handler.reload(&hotkeys, &favorite_modes) {
        Ok(failures) => notify_registration_failures(&failures),
        Err(err) => {
            crate::log_warn!("ホットキーの再登録に失敗: {}", err);
            crate::platform::show_notification(
                "ホットキー",
                &format!("ホットキーの再登録に失敗しました: {err}"),
            );
        }
    }
    refresh_settings_favorites_and_shortcuts(ctx.state, ctx.settings);
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::mpsc;
    use std::time::Instant;

    use super::super::context::AppEventContext;
    use super::handle_user_event;

    use crate::config::HotkeySettings;
    use crate::tray::hotkey::HotkeyHandler;
    use crate::tray::menu::TrayMenu;
    use crate::tray::selector::SelectorUi;
    use crate::tray::settings::SettingsWindow;
    use crate::tray::state::{AppEvent, test_app_state};
    use crate::tray::worker::ClipboardWorkerHandle;

    use tao::event_loop::{ControlFlow, EventLoopBuilder};
    use tao::platform::windows::EventLoopBuilderExtWindows;

    /// セレクター / OCR の非表示系 `AppEvent` を検証すること
    ///
    /// ホットキー登録は OS グローバルのため、衝突回避で 1 テストにまとめる
    #[test]
    fn hide_and_dispose_overlay_events() {
        let event_loop = EventLoopBuilder::<AppEvent>::with_user_event()
            .with_any_thread(true)
            .build();
        let state = Arc::new(test_app_state());
        let menu = TrayMenu::build(&state).expect("テスト用トレイメニューの構築に失敗");
        let (tx, _) = mpsc::channel();
        let clipboard_worker = Arc::new(ClipboardWorkerHandle::for_test(Arc::clone(&state), tx));
        let selectors = SelectorUi::new(&state.proxy, Arc::clone(&clipboard_worker));
        let settings = SettingsWindow::new_shell(state.proxy.clone(), Arc::clone(&state));

        // 他テスト (Alt+Shift / Alt+Ctrl / Alt+Ctrl+Shift+F*) と衝突しないキー
        let hotkeys = HotkeySettings {
            quick_selector: "Ctrl+Alt+Shift+1".to_string(),
            notification: "Ctrl+Alt+Shift+2".to_string(),
            pause: "Ctrl+Alt+Shift+3".to_string(),
            quit: "Ctrl+Alt+Shift+4".to_string(),
            undo: "Ctrl+Alt+Shift+5".to_string(),
            clip_selector: "Ctrl+Alt+Shift+6".to_string(),
            ocr: "Ctrl+Alt+Shift+7".to_string(),
            continuous_record: "Ctrl+Alt+Shift+8".to_string(),
            favorite_mode_slots: Vec::new(),
        };
        let mut hotkey_handler = HotkeyHandler::new(&hotkeys, &[])
            .expect("テスト用ホットキーの登録に失敗")
            .0;
        let mut last_quick_selector_show = Instant::now();
        let mut last_clip_selector_show = Instant::now();
        let mut control_flow = ControlFlow::Wait;

        let mut ctx = AppEventContext {
            state: &state,
            menu: &menu,
            selectors: &selectors,
            settings: &settings,
            last_quick_selector_show: &mut last_quick_selector_show,
            last_clip_selector_show: &mut last_clip_selector_show,
            hotkey_handler: &mut hotkey_handler,
            clipboard_worker: &clipboard_worker,
            event_loop: &event_loop,
            control_flow: &mut control_flow,
        };

        selectors.ocr.force_visible_for_test(true);
        assert!(selectors.ocr.is_visible());
        handle_user_event(&mut ctx, AppEvent::DisposeOcrOverlay);
        assert!(!selectors.ocr.is_visible());

        selectors.quick.force_visible_for_test(true);
        selectors.clip.force_visible_for_test(true);
        handle_user_event(&mut ctx, AppEvent::HideQuickSelector);
        handle_user_event(&mut ctx, AppEvent::HideClipSelector);
        assert!(!selectors.quick.is_visible());
        assert!(!selectors.clip.is_visible());
    }
}
