use std::sync::Arc;

use super::super::clipboard_monitor::bump_monitor_generation;
use super::super::menu::TrayMenu;
use super::super::notify;
use super::super::state::AppState;

use crate::platform;

use tao::event_loop::ControlFlow;

// ======================================================================
// メニューイベント処理
// ======================================================================
/// アプリケーションの基本操作(終了、一時停止、設定ファイルの起動、ショートカット一覧表示)を処理する
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
        true
    } else if id == menu.open_config_item.id() {
        state.save_config();
        if let Err(e) = crate::config::open_config_file() {
            crate::log_error!("設定ファイルの起動に失敗: {:?}", e);
            platform::show_notification("エラー", "設定ファイルを開けませんでした");
        }
        true
    } else if id == menu.reload_config_item.id() {
        let _ = state
            .proxy
            .send_event(crate::tray::state::AppEvent::ReloadConfig);
        true
    } else if id == menu.retry_clipboard_worker_item.id() {
        let _ = state
            .proxy
            .send_event(crate::tray::state::AppEvent::RestartClipboardWorker);
        true
    } else if id == menu.shortcut_list_item.id() {
        let body = state.with_config(|c| c.hotkeys.shortcut_list_text(&c.favorite_modes));
        platform::show_notification("ショートカット一覧", &body);
        true
    } else if id == menu.launch_at_login_item.id() {
        let enabled = menu.launch_at_login_item.is_checked();
        if let Err(e) = crate::autostart::set_enabled(enabled) {
            crate::log_error!("ログイン時自動起動の設定に失敗: {:?}", e);
            menu.launch_at_login_item.set_checked(!enabled);
            platform::show_notification("エラー", "ログイン時自動起動の設定に失敗しました");
        }
        true
    } else {
        false
    }
}
