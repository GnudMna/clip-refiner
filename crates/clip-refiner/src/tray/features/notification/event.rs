use std::sync::Arc;

use crate::tray::menu::TrayMenu;
use crate::tray::state::AppState;

/// 通知設定に関連するメニューイベントを処理する
///
/// # Arguments
/// * `id` - クリックされたメニュー項目の ID
/// * `menu` - トレイメニュー構造体
/// * `state` - アプリケーションの共有状態
///
/// # Returns
/// * `bool` - イベントがこの関数内で処理された場合は `true`、そうでない場合は `false` を返す
pub(crate) fn handle_notification_event(
    id: &tray_icon::menu::MenuId,
    menu: &TrayMenu,
    state: &Arc<AppState>,
) -> bool {
    macro_rules! handle_toggle {
        ($item:expr, $field:ident) => {
            if id == $item.id() {
                let enabled = $item.is_checked();
                state.with_config_mut(|c| c.notification_settings.$field = enabled);
                state.save_config();
                return true;
            }
        };
    }

    if id == menu.notification.enabled_item.id() {
        let enabled = menu.notification.enabled_item.is_checked();
        state.with_config_mut(|c| c.notification_settings.enabled = enabled);
        state.save_config();
        return true;
    }

    handle_toggle!(menu.notification.notify_process_item, notify_process);
    handle_toggle!(menu.notification.notify_pause_item, notify_pause);
    handle_toggle!(
        menu.notification.notify_history_restore_item,
        notify_history_restore
    );
    handle_toggle!(menu.notification.notify_ocr_item, notify_ocr);
    handle_toggle!(
        menu.notification.notify_registered_clip_item,
        notify_registered_clip
    );
    handle_toggle!(menu.notification.notify_favorites_item, notify_favorites);
    handle_toggle!(
        menu.notification.notify_continuous_record_item,
        notify_continuous_record
    );
    handle_toggle!(menu.notification.notify_result_item, notify_result);

    false
}
