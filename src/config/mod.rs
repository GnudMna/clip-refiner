mod paths;
pub(crate) mod permissions;
mod persistence;
mod types;

pub use paths::{get_config_dir, open_config_file};
pub use types::{AppConfig, HotkeySettings, MonitorMode, NotificationSettings};

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use crate::consts;
    use crate::refiner::RefineMode;

    use super::*;

    /// `AppConfig` のデフォルト値が正しいこと
    #[test]
    fn test_app_config_default() {
        let config = AppConfig::default();
        assert_eq!(config.version, consts::CONFIG_VERSION);
        assert_eq!(config.interval_ms, 1000);
        assert_eq!(config.mode, RefineMode::UrlDecode);
        assert_eq!(config.history_limit, consts::DEFAULT_HISTORY_LIMIT);
        assert_eq!(config.hotkeys, HotkeySettings::default());
    }

    /// `AppConfig` のシリアライズ/デシリアライズ往復
    #[test]
    fn test_app_config_serde() {
        let config = AppConfig::default();
        let json = serde_json::to_string(&config).expect("AppConfig のシリアライズに失敗");
        let decoded: AppConfig =
            serde_json::from_str(&json).expect("AppConfig のデシリアライズに失敗");
        assert_eq!(config.interval_ms, decoded.interval_ms);
        assert_eq!(config.mode, decoded.mode);
        assert_eq!(config.history_limit, decoded.history_limit);
        assert_eq!(config.hotkeys, decoded.hotkeys);
    }

    /// `NotificationSettings` のデフォルト値が正しいこと
    #[test]
    fn test_notification_settings_default() {
        let ns = NotificationSettings::default();
        assert!(!ns.enabled, "enabled のデフォルトは false");
        assert!(ns.notify_mode);
        assert!(!ns.notify_result, "notify_result のデフォルトは false");
        assert!(ns.notify_pause);
    }

    /// 古い設定 JSON (`show_success_notification` フィールドあり) を読んでも
    /// デフォルト値でデシリアライズできること
    #[test]
    fn test_app_config_backward_compat_old_field() {
        let old_json = r#"{
            "mode": "UrlDecode",
            "interval_ms": 1000,
            "show_success_notification": true
        }"#;
        let config: AppConfig =
            serde_json::from_str(old_json).expect("後方互換 JSON のデシリアライズに失敗");
        assert_eq!(config.interval_ms, 1000);
        assert!(!config.notification_settings.enabled);
        assert_eq!(config.history_limit, consts::DEFAULT_HISTORY_LIMIT);
        assert_eq!(config.hotkeys.selector, consts::DEFAULT_HOTKEY_SELECTOR);
    }

    /// `notification_settings.enabled` が JSON に保存・復元されること
    #[test]
    fn test_notification_settings_serde_roundtrip() {
        let mut config = AppConfig::default();
        config.notification_settings.enabled = true;
        config.notification_settings.notify_result = false;

        let json = serde_json::to_string(&config).expect("AppConfig のシリアライズに失敗");
        let decoded: AppConfig =
            serde_json::from_str(&json).expect("AppConfig のデシリアライズに失敗");
        assert!(decoded.notification_settings.enabled);
        assert!(!decoded.notification_settings.notify_result);
    }

    /// normalize が範囲外の値をクランプすること
    #[test]
    fn test_app_config_normalize_clamps() {
        let mut config = AppConfig {
            history_limit: 999,
            interval_ms: 10,
            ..Default::default()
        };

        config.normalize();

        assert_eq!(config.history_limit, consts::MAX_HISTORY_LIMIT);
        assert_eq!(config.interval_ms, consts::MIN_INTERVAL_MS);
    }

    /// `fix_invalid` が不正なホットキーをデフォルトへ置き換えること
    #[test]
    fn test_hotkey_settings_fix_invalid() {
        let mut hotkeys = HotkeySettings {
            selector: "Bad+Key".to_string(),
            ..HotkeySettings::default()
        };
        hotkeys.fix_invalid();
        assert_eq!(hotkeys.selector, consts::DEFAULT_HOTKEY_SELECTOR);
    }
}
