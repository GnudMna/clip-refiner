use std::sync::Arc;

use crate::tray::menu::TrayMenu;
use crate::tray::settings::ipc::SettingsPatch;
use crate::tray::state::AppState;

// ======================================================================
// 通知パッチ
// ======================================================================
/// 通知設定のパッチを適用する
pub(super) fn apply_notification_patch(
    patch: &SettingsPatch,
    state: &Arc<AppState>,
    menu: &TrayMenu,
    config_dirty: &mut bool,
) {
    let Some(notification_settings) = patch.notification_settings.as_ref() else {
        return;
    };

    state.with_config_mut(|c| c.notification_settings = notification_settings.clone());
    menu.notification
        .enabled_item
        .set_checked(state.with_config(|c| c.notification_settings.enabled));
    menu.notification
        .notify_mode_item
        .set_checked(state.with_config(|c| c.notification_settings.notify_mode));
    menu.notification
        .notify_result_item
        .set_checked(state.with_config(|c| c.notification_settings.notify_result));
    menu.notification
        .notify_pause_item
        .set_checked(state.with_config(|c| c.notification_settings.notify_pause));
    *config_dirty = true;
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;

    use crate::config::NotificationSettings;
    use crate::tray::menu::TrayMenu;
    use crate::tray::state::test_app_state;

    /// 通知設定パッチが `AppConfig` とトレイメニューへ反映されるようにすること
    #[test]
    fn notification_patch_updates_config_and_menu() {
        let state = Arc::new(test_app_state());
        let menu = TrayMenu::build(&state).expect("テスト用トレイメニューの構築に失敗");
        let patch = SettingsPatch {
            notification_settings: Some(NotificationSettings {
                enabled: true,
                notify_mode: false,
                notify_result: true,
                notify_pause: false,
            }),
            ..SettingsPatch::default()
        };

        let mut config_dirty = false;
        apply_notification_patch(&patch, &state, &menu, &mut config_dirty);

        let ns = state.with_config(|c| c.notification_settings.clone());
        assert!(ns.enabled);
        assert!(!ns.notify_mode);
        assert!(ns.notify_result);
        assert!(!ns.notify_pause);
        assert!(menu.notification.enabled_item.is_checked());
        assert!(!menu.notification.notify_mode_item.is_checked());
        assert!(menu.notification.notify_result_item.is_checked());
        assert!(!menu.notification.notify_pause_item.is_checked());
        assert!(config_dirty);
    }
}
