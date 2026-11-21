//! 設定ウィンドウ / アプリ更新のユーザーイベント処理

use crate::tray::event::AppEventContext;
use crate::tray::features::{handle_update_apply_finished, handle_update_check_finished};
use crate::tray::state::{SettingsEvent, UpdateEvent};

use tao::event_loop::ControlFlow;

// ======================================================================
// 設定ウィンドウ
// ======================================================================
/// 設定ウィンドウ関連イベントを処理する
pub(super) fn handle_settings(ctx: &mut AppEventContext<'_>, event: SettingsEvent) {
    ctx.settings.handle_event(
        event,
        ctx.event_loop,
        ctx.state,
        ctx.menu,
        Some(&ctx.selectors.quick),
        ctx.clipboard_worker,
    );
}

// ======================================================================
// アプリ更新
// ======================================================================
/// アプリ更新関連イベントを処理する
pub(super) fn handle_update(ctx: &mut AppEventContext<'_>, event: UpdateEvent) {
    match event {
        UpdateEvent::CheckFinished => {
            handle_update_check_finished(ctx.state, ctx.settings);
        }
        UpdateEvent::ApplyFinished => {
            if handle_update_apply_finished(ctx.state, ctx.settings) {
                *ctx.control_flow = ControlFlow::Exit;
            }
        }
    }
}
