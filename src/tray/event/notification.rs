use std::sync::Arc;

use super::super::menu::TrayMenu;
use super::super::state::AppState;

/// 通知設定に関連するメニューイベントを処理する
///
/// # Arguments
/// * `id` - クリックされたメニュー項目の ID
/// * `menu` - トレイメニュー構造体
/// * `state` - アプリケーションの共有状態
///
/// # Returns
/// * `bool` - イベントがこの関数内で処理された場合は `true`、そうでない場合は `false` を返す
pub(super) fn handle_notification_event(
    id: &tray_icon::menu::MenuId,
    menu: &TrayMenu,
    state: &Arc<AppState>,
) -> bool {
    if id == menu.notification.enabled_item.id() {
        let enabled = menu.notification.enabled_item.is_checked();
        state.with_config_mut(|c| c.notification_settings.enabled = enabled);
        state.save_config();
        return true;
    }
    if id == menu.notification.notify_mode_item.id() {
        let enabled = menu.notification.notify_mode_item.is_checked();
        state.with_config_mut(|c| c.notification_settings.notify_mode = enabled);
        state.save_config();
        return true;
    }
    if id == menu.notification.notify_result_item.id() {
        let enabled = menu.notification.notify_result_item.is_checked();
        state.with_config_mut(|c| c.notification_settings.notify_result = enabled);
        state.save_config();
        return true;
    }
    if id == menu.notification.notify_pause_item.id() {
        let enabled = menu.notification.notify_pause_item.is_checked();
        state.with_config_mut(|c| c.notification_settings.notify_pause = enabled);
        state.save_config();
        return true;
    }
    false
}
