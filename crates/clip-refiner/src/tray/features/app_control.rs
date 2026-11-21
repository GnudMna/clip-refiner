use std::sync::Arc;

use tao::event_loop::ControlFlow;

use crate::tray::menu::TrayMenu;
use crate::tray::state::{AppState, SelectorEvent, SettingsEvent, WorkerEvent};

// ======================================================================
// メニューイベント処理
// ======================================================================
/// アプリケーションの基本操作(終了、設定ウィンドウの表示など)を処理する
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
    } else if id == menu.quick_selector_item.id() {
        let _ = state.proxy.send_event(SelectorEvent::ShowQuick.into());
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
    use tao::event_loop::ControlFlow;

    use super::*;
    use crate::test_helpers::TrayTestContext;

    /// 監視再開メニューを処理済みとし、イベントループを終了しないようにすること
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
    /// テスト用 `AppState` は `EventLoop` を保持しないため、`SettingsEvent::Show` の
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

    /// クイック変換メニューで処理が完了しイベントループを終了しないようにすること
    #[test]
    fn handle_app_control_quick_selector_keeps_event_loop() {
        let ctx = TrayTestContext::new();
        let mut control_flow = ControlFlow::Wait;

        let id = ctx.menu().quick_selector_item.id();
        let handled = handle_app_control(id, ctx.menu(), ctx.state(), &mut control_flow);

        assert!(handled);
        assert!(matches!(control_flow, ControlFlow::Wait));
    }
}
