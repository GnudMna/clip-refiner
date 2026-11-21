use std::sync::Arc;

use crate::tray::settings::ipc::SettingsPatch;
use crate::tray::state::AppState;

// ======================================================================
// 通知パッチ
// ======================================================================
/// 通知設定のパッチを適用する
pub(super) fn apply_notification_patch(
    patch: &SettingsPatch,
    state: &Arc<AppState>,
    config_dirty: &mut bool,
) {
    let Some(notification_settings) = patch.notification_settings.as_ref() else {
        return;
    };

    state.with_config_mut(|c| {
        c.notification_settings = notification_settings.clone();
        c.notification_settings.migrate_legacy_fields();
    });

    *config_dirty = true;
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::NotificationSettings;
    use crate::test_helpers::TrayTestContext;

    /// 通知設定パッチが `AppConfig` へ反映されるようにすること
    #[test]
    fn notification_patch_updates_config() {
        let ctx = TrayTestContext::new();
        let mut notification_settings = NotificationSettings::default();
        notification_settings.enabled = true;
        notification_settings.notify_process = false;
        notification_settings.notify_auto_refine = true;
        notification_settings.notify_history_restore = false;
        notification_settings.notify_ocr = true;
        notification_settings.notify_registered_clip = false;
        notification_settings.notify_favorites = true;
        notification_settings.notify_continuous_record = false;
        notification_settings.notify_result = true;
        let patch = SettingsPatch {
            notification_settings: Some(notification_settings),
            ..SettingsPatch::default()
        };

        let mut config_dirty = false;
        apply_notification_patch(&patch, ctx.state(), &mut config_dirty);

        let ns = ctx.state().with_config(|c| c.notification_settings.clone());
        assert!(ns.enabled);
        assert!(!ns.notify_process);
        assert!(ns.notify_auto_refine);
        assert!(!ns.notify_history_restore);
        assert!(ns.notify_ocr);
        assert!(!ns.notify_registered_clip);
        assert!(ns.notify_favorites);
        assert!(!ns.notify_continuous_record);
        assert!(ns.notify_result);
        assert!(config_dirty);
    }
}
