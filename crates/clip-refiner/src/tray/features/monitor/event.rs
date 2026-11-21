use std::sync::Arc;

use crate::tray::clipboard_monitor::{self, bump_monitor_generation};
use crate::tray::menu::TrayMenu;
use crate::tray::state::AppState;

use crate::config::MonitorMode;

// ======================================================================
// 監視設定更新
// ======================================================================
/// 監視モードを更新し、必要に応じて監視用スレッドのリセットを行う
///
/// # Arguments
/// * `state` - アプリケーションの共有状態
/// * `menu` - トレイメニュー構造体
/// * `monitor_mode` - 設定する新しい監視モード
pub fn update_monitor_mode(state: &Arc<AppState>, menu: &TrayMenu, monitor_mode: MonitorMode) {
    if state.with_config(|c| c.monitor_mode) == monitor_mode {
        return;
    }

    state.with_config_mut(|c| c.monitor_mode = monitor_mode);

    for (item, m) in &menu.monitor.items {
        item.set_checked(*m == monitor_mode);
    }

    clipboard_monitor::update_monitor_mode_impl(menu, monitor_mode);

    state.save_config();
    bump_monitor_generation(state);
}

// ======================================================================
// メニューイベント処理
// ======================================================================
/// 監視設定(監視モード、ポーリング間隔)に関連するメニューイベントを処理する
///
/// # Arguments
/// * `id` - クリックされたメニュー項目の ID
/// * `menu` - トレイメニュー構造体
/// * `state` - アプリケーションの共有状態
///
/// # Returns
/// * `bool` - イベントがこの関数内で処理された場合は `true`、そうでない場合は `false` を返す
pub(crate) fn handle_monitor_event(
    id: &tray_icon::menu::MenuId,
    menu: &TrayMenu,
    state: &Arc<AppState>,
) -> bool {
    if let Some((_, monitor_mode)) = menu.monitor.items.iter().find(|(item, _)| item.id() == id) {
        update_monitor_mode(state, menu, *monitor_mode);
        return true;
    }

    if id == menu.interval.custom_item.id() {
        // カスタム間隔は設定ウィンドウで数値入力する
        let _ = state
            .proxy
            .send_event(crate::tray::state::SettingsEvent::Show.into());
        // クリックでチェックが反転しないよう、現在値へ再同期する
        menu.sync_interval_menu(state.with_config(|c| c.interval_ms));
        return true;
    }

    for (item, ms) in &menu.interval.items {
        if item.id() == id {
            state.with_config_mut(|c| c.interval_ms = *ms);
            menu.sync_interval_menu(*ms);
            state.save_config();
            return true;
        }
    }
    false
}
