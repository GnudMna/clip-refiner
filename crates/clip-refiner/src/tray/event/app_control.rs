use std::sync::Arc;

use super::super::clipboard_monitor::bump_monitor_generation;
use super::super::menu::TrayMenu;
use super::super::notify;
use super::super::state::{AppEvent, AppState};

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
pub(super) fn handle_app_control(
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
        let _ = state.proxy.send_event(AppEvent::RefreshSettingsWindow);
        true
    } else if id == menu.open_settings_window_item.id() {
        let _ = state
            .proxy
            .send_event(crate::tray::state::AppEvent::ShowSettingsWindow);
        true
    } else if id == menu.retry_clipboard_worker_item.id() {
        let _ = state
            .proxy
            .send_event(crate::tray::state::AppEvent::RestartClipboardWorker);
        true
    } else {
        false
    }
}
