use tao::event::Event;
use tao::event::WindowEvent;
use tao::event_loop::ControlFlow;
use tray_icon::menu::MenuEvent;

use super::super::state::AppEvent;
use super::context::AppEventContext;
use super::handle_menu_event;
use super::handle_window_event;
use super::user_event::handle_user_event;
use crate::tray::selector::{apply_idle_control_flow, sync_selector_retention};

// ======================================================================
// イベントループ処理
// ======================================================================
/// メインループからの `tao` イベントを受信し、各ハンドラへ振り分ける
///
/// トレイメニューイベントは `WindowEvent` / `UserEvent` の処理後にも drain する
/// 設定ウィンドウ表示中など、メニュー専用のイベントが来ない間も滞留させない
pub(crate) fn handle_loop_event(event: Event<AppEvent>, ctx: &mut AppEventContext<'_>) {
    *ctx.control_flow = ControlFlow::Wait;

    match event {
        Event::UserEvent(app_event) => handle_user_event(ctx, app_event),
        Event::WindowEvent {
            window_id, event, ..
        } if ctx.selectors.quick.id() == Some(window_id) => {
            handle_window_event(
                &event,
                &ctx.selectors.quick,
                ctx.last_quick_selector_show,
                ctx.state,
            );
        }
        Event::WindowEvent {
            window_id, event, ..
        } if ctx.selectors.clip.id() == Some(window_id) => {
            handle_window_event(
                &event,
                &ctx.selectors.clip,
                ctx.last_clip_selector_show,
                ctx.state,
            );
        }
        Event::WindowEvent {
            window_id,
            event: WindowEvent::CloseRequested,
            ..
        } if ctx.settings.id() == Some(window_id) => {
            ctx.settings.request_close_from_os();
        }
        _ => {}
    }

    drain_pending_menu_events(ctx);
    sync_and_schedule_selector_retention(ctx);
}

/// セレクター `WebView` の保持方針を同期し、アイドル破棄の起床を予約する
fn sync_and_schedule_selector_retention(ctx: &mut AppEventContext<'_>) {
    let (retention, idle_minutes) = ctx.state.with_config(|c| {
        (
            c.selector_webview_retention,
            c.selector_webview_idle_minutes,
        )
    });
    sync_selector_retention(
        &ctx.selectors.quick,
        &ctx.selectors.clip,
        retention,
        idle_minutes,
    );
    apply_idle_control_flow(
        ctx.control_flow,
        &ctx.selectors.quick,
        &ctx.selectors.clip,
        retention,
        idle_minutes,
    );
}

/// 滞留しているトレイメニューイベントをすべて処理する
///
/// ホットキーリスナーと同様に `try_recv` でキューを空にする
/// 終了が選ばれた場合は以降のメニュー操作を捨てる
fn drain_pending_menu_events(ctx: &mut AppEventContext<'_>) {
    while let Ok(menu_event) = MenuEvent::receiver().try_recv() {
        handle_menu_event(
            &menu_event,
            ctx.menu,
            ctx.state,
            ctx.clipboard_worker,
            Some(&ctx.selectors.quick),
            ctx.control_flow,
        );
        if matches!(*ctx.control_flow, ControlFlow::Exit) {
            break;
        }
    }
}
