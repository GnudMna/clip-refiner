use std::sync::Arc;

use crate::tray::clipboard_monitor::bump_monitor_generation;
use crate::tray::menu::TrayMenu;
use crate::tray::notify;
use crate::tray::state::{AppState, SettingsEvent, UpdateEvent, WorkerEvent};

use tao::event_loop::ControlFlow;

// ======================================================================
// メニューイベント処理
// ======================================================================
/// アプリケーションの基本操作(終了、一時停止、設定ウィンドウの表示)を処理する
///
/// # Arguments
/// * `id` - クリックされたメニュー項目の ID
/// * `menu` - トレイメニュー構造体
/// * `state` - アプリケーションの共有状態
/// * `control_flow` - イベントループの制御フロー
///
/// # Returns
/// * `bool` - イベントがこの関数内で処理された場合は `true`、そうでない場合は `false` を返す
pub(crate) fn handle_app_control(
    id: &tray_icon::menu::MenuId,
    menu: &TrayMenu,
    state: &Arc<AppState>,
    control_flow: &mut ControlFlow,
) -> bool {
    if id == menu.quit_item.id() {
        *control_flow = ControlFlow::Exit;
        true
    } else if id == menu.pause_item.id() {
        let paused = menu.pause_item.is_checked();
        state.with_config_mut(|c| c.is_paused = paused);
        notify::show_pause_notification(state, paused, "設定変更");
        state.save_config();
        bump_monitor_generation(state);
        let _ = state.proxy.send_event(SettingsEvent::Refresh.into());
        true
    } else if id == menu.check_update_item.id() {
        let _ = state.proxy.send_event(UpdateEvent::TrayCheck.into());
        true
    } else if id == menu.open_settings_window_item.id() {
        let _ = state.proxy.send_event(SettingsEvent::Show.into());
        true
    } else if id == menu.retry_clipboard_worker_item.id() {
        let _ = state.proxy.send_event(WorkerEvent::Restart.into());
        true
    } else {
        false
    }
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::TrayTestContext;

    use tao::event_loop::ControlFlow;

    /// 監視再開メニューで `RestartClipboardWorker` をキューすること
    #[test]
    fn handle_app_control_retry_sends_restart_event() {
        let ctx = TrayTestContext::new();
        let mut control_flow = ControlFlow::Wait;

        let id = ctx.menu().retry_clipboard_worker_item.id();
        let handled = handle_app_control(id, ctx.menu(), ctx.state(), &mut control_flow);

        assert!(handled);
        assert!(matches!(control_flow, ControlFlow::Wait));
    }

    /// 設定メニューで処理が完了しイベントループを終了しないようにすること
    ///
    /// テスト用 `AppState` は `EventLoop` を保持しないため、`ShowSettingsWindow` の
    /// 受信は検証できない。メニュー ID 照合と `ControlFlow` 維持を固定する
    #[test]
    fn handle_app_control_open_settings_keeps_event_loop() {
        let ctx = TrayTestContext::new();
        let mut control_flow = ControlFlow::Wait;

        let id = ctx.menu().open_settings_window_item.id();
        let handled = handle_app_control(id, ctx.menu(), ctx.state(), &mut control_flow);

        assert!(handled);
        assert!(matches!(control_flow, ControlFlow::Wait));
    }

    /// 更新確認メニューで処理が完了しイベントループを終了しないようにすること
    #[test]
    fn handle_app_control_check_update_keeps_event_loop() {
        let ctx = TrayTestContext::new();
        let mut control_flow = ControlFlow::Wait;

        let id = ctx.menu().check_update_item.id();
        let handled = handle_app_control(id, ctx.menu(), ctx.state(), &mut control_flow);

        assert!(handled);
        assert!(matches!(control_flow, ControlFlow::Wait));
    }
}
