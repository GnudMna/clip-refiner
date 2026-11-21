use super::super::dispatch;
use super::super::hotkey::{HotkeyEventContext, notify_registration_failures};
use super::super::state::AppEvent;
use super::super::worker::ClipboardCommand;
use super::context::AppEventContext;
use super::{
    copy_registered_clip, delete_registered_clip, move_favorite_mode, refresh_clips_views,
    reload_config_with_notification, toggle_favorite_mode, update_refine,
};

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
        AppEvent::RequestClipCopy(index) => {
            ctx.selectors.clip.hide();
            copy_registered_clip(ctx.state, ctx.clipboard_worker, index);
        }
        AppEvent::RequestClipRegister => {
            dispatch::send_clipboard_command(
                ctx.clipboard_worker,
                ClipboardCommand::RegisterClipFromClipboard,
            );
        }
        AppEvent::RequestClipDelete(index) => {
            delete_registered_clip(ctx.state, ctx.menu, &ctx.selectors.clip, index);
        }
        AppEvent::RequestFavoriteToggle(mode) => {
            toggle_favorite_mode(ctx.state, ctx.menu, Some(&ctx.selectors.quick), mode);
        }
        AppEvent::RequestFavoriteMove(mode, direction) => {
            move_favorite_mode(
                ctx.state,
                ctx.menu,
                Some(&ctx.selectors.quick),
                mode,
                direction,
            );
        }
        AppEvent::RefreshHistory => {
            if let Err(err) = ctx.menu.refresh_history(ctx.state) {
                dispatch::log_menu_operation_error("履歴メニューの更新", err);
            }
        }
        AppEvent::RefreshContinuousRecord => {
            if let Err(err) = ctx.menu.refresh_continuous_record(ctx.state) {
                dispatch::log_menu_operation_error("連続記録メニューの更新", err);
            }
        }
        AppEvent::RefreshClips => {
            refresh_clips_views(ctx.state, ctx.menu, &ctx.selectors.clip);
        }
        AppEvent::ReloadConfig => {
            reload_config_with_notification(
                ctx.state,
                ctx.menu,
                ctx.hotkey_handler,
                &ctx.selectors.clip,
            );
        }
        AppEvent::RestartClipboardWorker
        | AppEvent::ClipboardWorkerStopped
        | AppEvent::ClipboardWorkerReady => handle_clipboard_worker_lifecycle(ctx, event),
        AppEvent::ReloadFavoriteHotkeys => reload_favorite_hotkeys(ctx),
        AppEvent::Hotkey(hotkey_event) => {
            let AppEventContext {
                state,
                menu,
                selectors,
                hotkey_handler,
                clipboard_worker,
                control_flow,
                last_quick_selector_show,
                last_clip_selector_show,
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
    }
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
}
