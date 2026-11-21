use super::super::state::AppEvent;
use super::context::AppEventContext;
use super::handle_menu_event;
use super::handle_window_event;
use super::user_event::handle_user_event;

use tao::event::Event;
use tao::event::WindowEvent;
use tao::event_loop::ControlFlow;
use tray_icon::menu::MenuEvent;

// ======================================================================
// イベントループ処理
// ======================================================================
/// メインループからの `tao` イベントを受信し、各ハンドラへ振り分ける
pub(crate) fn handle_loop_event(event: Event<AppEvent>, ctx: &mut AppEventContext<'_>) {
    *ctx.control_flow = ControlFlow::Wait;

    match event {
        Event::UserEvent(app_event) => handle_user_event(ctx, app_event),
        Event::WindowEvent {
            window_id, event, ..
        } if ctx.selectors.quick.id() == Some(window_id) => {
            handle_window_event(&event, &ctx.selectors.quick, ctx.last_quick_selector_show);
        }
        Event::WindowEvent {
            window_id, event, ..
        } if ctx.selectors.clip.id() == Some(window_id) => {
            handle_window_event(&event, &ctx.selectors.clip, ctx.last_clip_selector_show);
        }
        Event::WindowEvent {
            window_id, event, ..
        } if ctx.settings.id() == Some(window_id) => {
            if let WindowEvent::CloseRequested = event {
                ctx.settings.request_close_from_os();
            }
        }
        _ => {
            if let Ok(menu_event) = MenuEvent::receiver().try_recv() {
                handle_menu_event(
                    &menu_event,
                    ctx.menu,
                    ctx.state,
                    ctx.clipboard_worker,
                    Some(&ctx.selectors.quick),
                    ctx.control_flow,
                );
            }
        }
    }
}
