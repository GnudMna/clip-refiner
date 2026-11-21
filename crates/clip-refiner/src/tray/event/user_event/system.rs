//! メニュー再構築 / 設定再読込 / ワーカー寿命 / ホットキーのユーザーイベント処理

use crate::tray::dispatch;
use crate::tray::event::{AppEventContext, reload_config_with_notification};
use crate::tray::hotkey::{HotkeyEventContext, notify_registration_failures};
use crate::tray::settings::{
    refresh_settings_favorites_and_shortcuts, refresh_settings_window_after_external_reload,
};
use crate::tray::state::{ConfigEvent, HotkeyEvent, MenuRefreshEvent, WorkerEvent};
use crate::tray::worker::worker_stopped_notification_body;

// ======================================================================
// トレイメニュー再構築
// ======================================================================
/// トレイメニュー再構築イベントを処理する
pub(super) fn handle_menu_refresh(ctx: &mut AppEventContext<'_>, event: MenuRefreshEvent) {
    match event {
        MenuRefreshEvent::History => {
            if let Err(err) = ctx.menu.refresh_history(ctx.state) {
                dispatch::log_menu_operation_error("履歴メニューの更新", err);
            }
        }
        MenuRefreshEvent::ContinuousRecord => {
            if let Err(err) = ctx.menu.refresh_continuous_record(ctx.state) {
                dispatch::log_menu_operation_error("連続記録メニューの更新", err);
            }
        }
    }
}

// ======================================================================
// 設定ファイル
// ======================================================================
/// 設定ファイル関連イベントを処理する
pub(super) fn handle_config(ctx: &mut AppEventContext<'_>, event: ConfigEvent) {
    match event {
        ConfigEvent::Reload => {
            reload_config_with_notification(
                ctx.state,
                ctx.menu,
                ctx.hotkey_handler,
                &ctx.selectors.clip,
            );
            refresh_settings_window_after_external_reload(ctx.state, ctx.settings);
        }
    }
}

// ======================================================================
// クリップボードワーカー
// ======================================================================
/// クリップボードワーカーの再起動・停止・復旧イベントを処理する
pub(super) fn handle_worker(ctx: &mut AppEventContext<'_>, event: WorkerEvent) {
    match event {
        WorkerEvent::Restart => ctx.clipboard_worker.restart(),
        WorkerEvent::Stopped => {
            ctx.clipboard_worker.sync_menu_state(ctx.menu);
            crate::platform::show_notification(
                "クリップボードエラー",
                worker_stopped_notification_body(),
            );
        }
        WorkerEvent::Ready => {
            ctx.clipboard_worker.sync_menu_state(ctx.menu);
            crate::platform::show_notification(
                "クリップボード監視",
                "クリップボード監視を再開しました",
            );
        }
    }
}

// ======================================================================
// ホットキー
// ======================================================================
/// ホットキー関連イベントを処理する
pub(super) fn handle_hotkey(ctx: &mut AppEventContext<'_>, event: HotkeyEvent) {
    match event {
        HotkeyEvent::Received(hotkey_event) => handle_hotkey_received(ctx, hotkey_event),
        HotkeyEvent::ReloadFavorites => reload_favorite_hotkeys(ctx),
        HotkeyEvent::ReloadAll => reload_hotkeys(ctx),
    }
}

/// グローバルホットキーイベントを処理する
fn handle_hotkey_received(
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

/// お気に入りホットキーを再登録する
fn reload_favorite_hotkeys(ctx: &mut AppEventContext<'_>) {
    let (hotkeys, favorite_count) = ctx
        .state
        .with_config(|c| (c.hotkeys.clone(), c.favorite_modes.len()));
    let failures = ctx
        .hotkey_handler
        .reload_favorite_slots(&hotkeys, favorite_count);
    notify_registration_failures(&failures);
    refresh_settings_favorites_and_shortcuts(ctx.state, ctx.settings);
}

/// グローバルホットキー全体を再登録する
fn reload_hotkeys(ctx: &mut AppEventContext<'_>) {
    let (hotkeys, favorite_modes) = ctx
        .state
        .with_config(|c| (c.hotkeys.clone(), c.favorite_modes.clone()));
    let failures = ctx.hotkey_handler.reload(&hotkeys, &favorite_modes);
    notify_registration_failures(&failures);
    refresh_settings_favorites_and_shortcuts(ctx.state, ctx.settings);
}
