mod migrate;
mod paths;
pub(crate) mod permissions;
mod persistence;
mod serialize;
mod types;

pub use paths::{get_config_dir, open_config_file};
pub use types::{AppConfig, HotkeySettings, MonitorMode, NotificationSettings, RegexSettings};

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
        let toml_str = toml::to_string(&config).expect("AppConfig のシリアライズに失敗");
        let decoded: AppConfig =
            toml::from_str(&toml_str).expect("AppConfig のデシリアライズに失敗");
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

    /// `notification_settings.enabled` が TOML に保存・復元されること
    #[test]
    fn test_notification_settings_serde_roundtrip() {
        let mut config = AppConfig::default();
        config.notification_settings.enabled = true;
        config.notification_settings.notify_result = false;

        let toml_str = toml::to_string(&config).expect("AppConfig のシリアライズに失敗");
        let decoded: AppConfig =
            toml::from_str(&toml_str).expect("AppConfig のデシリアライズに失敗");
        assert!(decoded.notification_settings.enabled);
        assert!(!decoded.notification_settings.notify_result);
    }

    /// normalize が範囲外の値をクランプし version を現行へ更新すること
    #[test]
    fn test_app_config_normalize_clamps() {
        let mut config = AppConfig {
            version: 0,
            history_limit: 999,
            interval_ms: 10,
            ..Default::default()
        };

        config.normalize();

        assert_eq!(config.version, consts::CONFIG_VERSION);
        assert_eq!(config.history_limit, consts::MAX_HISTORY_LIMIT);
        assert_eq!(config.interval_ms, consts::MIN_INTERVAL_MS);
    }

    /// version 未指定の TOML が v0 として読み込まれ v1 へ移行されること
    #[test]
    fn test_prepare_loaded_migrates_missing_version() {
        let toml_str = r#"
mode = "Trim"
interval_ms = 500
"#;
        let config: AppConfig = toml::from_str(toml_str).expect("デシリアライズに失敗");
        assert_eq!(config.version, 0);

        let (prepared, migrated) = config.prepare_loaded();
        assert!(migrated);
        assert_eq!(prepared.version, consts::CONFIG_VERSION);
        assert_eq!(prepared.mode, RefineMode::Trim);
        assert_eq!(prepared.interval_ms, 500);
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
