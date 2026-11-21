//! 通知設定の移行・IPC・トレイメニュー連携の統合テスト

use clip_refiner::AppConfig;
use clip_refiner::NotificationSettings;
use clip_refiner::test_helpers::SettingsTestContext;

// ======================================================================
// テスト
// ======================================================================

/// 旧 `notify_mode` / `notify_actions` を含む TOML が新項目へ移行されるようにすること
#[test]
fn legacy_notification_toml_migrates_to_per_category_toggles() {
    let toml_str = r"
version = 2
enabled = true
notify_mode = false
notify_result = false
notify_actions = false
";

    let mut settings: NotificationSettings =
        toml::from_str(toml_str).expect("NotificationSettings のデシリアライズに失敗");
    settings.migrate_legacy_fields();

    assert!(!settings.notify_process);
    assert!(!settings.notify_history_restore);
    assert!(!settings.notify_ocr);
    assert!(!settings.notify_registered_clip);
    assert!(!settings.notify_favorites);
    assert!(!settings.notify_continuous_record);
}

/// 旧設定を含む `NotificationSettings` が `AppConfig` へ取り込めるようにすること
#[test]
fn legacy_notification_fields_migrate_into_app_config() {
    let legacy_section = r"
enabled = true
notify_mode = false
notify_result = false
notify_actions = false
";
    let mut notification_settings: NotificationSettings =
        toml::from_str(legacy_section).expect("NotificationSettings のデシリアライズに失敗");
    notification_settings.migrate_legacy_fields();

    let config = AppConfig {
        notification_settings,
        ..Default::default()
    };

    assert!(!config.notification_settings.notify_process);
    assert!(!config.notification_settings.notify_ocr);
}

/// 通知 IPC パッチが全カテゴリとトレイメニューへ反映されるようにすること
#[test]
fn apply_ipc_updates_all_notification_menu_items() {
    let ctx = SettingsTestContext::new();

    assert!(ctx.apply_ipc(
        r#"{"type":"apply","patch":{"notificationSettings":{"enabled":true,"notifyProcess":false,"notifyPause":true,"notifyHistoryRestore":false,"notifyOcr":true,"notifyRegisteredClip":false,"notifyFavorites":true,"notifyContinuousRecord":false,"notifyResult":true}}}"#,
    ));

    let ns = ctx.state().with_config(|c| c.notification_settings.clone());
    assert!(ns.enabled);
    assert!(!ns.notify_process);
    assert!(ns.notify_pause);
    assert!(!ns.notify_history_restore);
    assert!(ns.notify_ocr);
    assert!(!ns.notify_registered_clip);
    assert!(ns.notify_favorites);
    assert!(!ns.notify_continuous_record);
    assert!(ns.notify_result);

    let menu = &ctx.menu().notification;
    assert!(menu.enabled_item.is_checked());
    assert!(!menu.notify_process_item.is_checked());
    assert!(menu.notify_pause_item.is_checked());
    assert!(!menu.notify_history_restore_item.is_checked());
    assert!(menu.notify_ocr_item.is_checked());
    assert!(!menu.notify_registered_clip_item.is_checked());
    assert!(menu.notify_favorites_item.is_checked());
    assert!(!menu.notify_continuous_record_item.is_checked());
    assert!(menu.notify_result_item.is_checked());
}

/// マスター OFF 時でも個別通知トグルを設定へ保存できるようにすること
#[test]
fn master_off_allows_persisting_sub_notification_toggles() {
    let ctx = SettingsTestContext::new();

    assert!(ctx.apply_ipc(
        r#"{"type":"apply","patch":{"notificationSettings":{"enabled":false,"notifyProcess":true,"notifyOcr":false,"notifyResult":true}}}"#,
    ));

    let ns = ctx.state().with_config(|c| c.notification_settings.clone());
    assert!(!ns.enabled);
    assert!(ns.notify_process);
    assert!(!ns.notify_ocr);
    assert!(ns.notify_result);
    assert!(!ctx.menu().notification.enabled_item.is_checked());
    assert!(ctx.menu().notification.notify_process_item.is_checked());
}

/// マスター OFF 時の部分パッチが他カテゴリを保持できるようにすること
#[test]
fn partial_notification_patch_preserves_sibling_categories_when_master_off() {
    let ctx = SettingsTestContext::new();

    assert!(ctx.apply_ipc(
        r#"{"type":"apply","patch":{"notificationSettings":{"enabled":false,"notifyProcess":true,"notifyPause":true,"notifyOcr":false}}}"#,
    ));
    assert!(
        ctx.apply_ipc(r#"{"type":"apply","patch":{"notificationSettings":{"notifyOcr":true}}}"#,)
    );

    let ns = ctx.state().with_config(|c| c.notification_settings.clone());
    assert!(!ns.enabled);
    assert!(ns.notify_process);
    assert!(ns.notify_pause);
    assert!(ns.notify_ocr);
}

/// 旧キー (`notify_process`) を含む IPC パッチが解釈されるようにすること
#[test]
fn apply_ipc_accepts_snake_case_notification_fields() {
    let ctx = SettingsTestContext::new();

    assert!(ctx.apply_ipc(
        r#"{"type":"apply","patch":{"notificationSettings":{"enabled":true,"notify_process":false,"notify_ocr":true}}}"#,
    ));

    let ns = ctx.state().with_config(|c| c.notification_settings.clone());
    assert!(ns.enabled);
    assert!(!ns.notify_process);
    assert!(ns.notify_ocr);
}
