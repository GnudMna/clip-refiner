//! `ClipRefiner` の設定型と永続化 API
//!
//! [`AppConfig`] は `config.toml` の内容に対応する。正規表現設定 ([`RegexSettings`]) は
//! [`RefineContext::from_config`](crate::RefineContext::from_config) 経由で加工 API に渡せる

mod clip_store_key;
mod migrate;
mod paths;
pub mod permissions;
mod persistence;
mod registered_clips;
mod registered_images;
mod serialize;
mod types;

pub use crate::consts::CONFIG_VERSION;
#[cfg(any(test, debug_assertions, feature = "test-helpers"))]
pub use paths::{clear_test_config_dir, set_test_config_dir};
pub use paths::{get_config_dir, open_config_file};
pub use persistence::{ConfigReloadError, disk_config_modified_time};
pub use types::{
    AddRegisteredClipError, AppConfig, FavoriteMoveDirection, FavoriteToggleResult, HotkeySettings,
    MonitorMode, NotificationSettings, RegexSettings, ResolvedClip,
};

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;

    use crate::consts;
    use crate::refiner::RefineMode;

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

    /// v0 TOML は移行後に v1 へ更新され、設定値と `favorite_modes` が保持されること
    #[test]
    fn test_prepare_loaded_migrates_v0_toml() {
        let v0_toml = r#"
version = 0
mode = "Trim"
interval_ms = 500
"#;
        let config: AppConfig = toml::from_str(v0_toml).expect("デシリアライズに失敗");
        assert_eq!(config.version, 0);

        let (prepared, migration) = {
            let migration = config.prepare_loaded();
            (migration.config.clone(), migration)
        };
        assert!(migration.migrated);
        assert_eq!(prepared.version, consts::CONFIG_VERSION);
        assert_eq!(prepared.mode, RefineMode::Trim);
        assert_eq!(prepared.interval_ms, 500);
        assert!(prepared.favorite_modes.is_empty());
    }

    /// v0 TOML を保存すると v1 スキーマで書き出されること
    #[test]
    fn test_save_after_v0_migration_writes_current_version() {
        use super::serialize::config_to_toml;

        let v0_toml = r#"
version = 0
mode = "JsonFormat"
interval_ms = 2000
"#;
        let config: AppConfig = toml::from_str(v0_toml).expect("デシリアライズに失敗");
        let migration = config.prepare_loaded();
        let mut prepared = migration.config;
        assert!(migration.migrated);

        prepared.normalize();
        let content = config_to_toml(&prepared, Some(v0_toml)).expect("移行後 TOML の生成に失敗");
        let decoded: AppConfig = toml::from_str(&content).expect("保存後 TOML の解析に失敗");
        assert_eq!(decoded.version, consts::CONFIG_VERSION);
        assert_eq!(decoded.mode, RefineMode::JsonFormat);
        assert_eq!(decoded.interval_ms, 2000);
        assert!(content.contains("favorite_modes"));
    }

    /// version 未指定の TOML は現行スキーマとして読み込まれ、移行不要であること
    #[test]
    fn test_prepare_loaded_without_version_is_current_schema() {
        let toml_str = r#"
mode = "Trim"
interval_ms = 500
"#;
        let config: AppConfig = toml::from_str(toml_str).expect("デシリアライズに失敗");
        assert_eq!(config.version, consts::CONFIG_VERSION);

        let migration = config.prepare_loaded();
        assert!(!migration.migrated);
        let prepared = migration.config;
        assert_eq!(prepared.version, consts::CONFIG_VERSION);
        assert_eq!(prepared.mode, RefineMode::Trim);
        assert_eq!(prepared.interval_ms, 500);
    }

    /// v1 TOML は現行スキーマへ移行し、`pipeline` など既存項目を保持すること
    #[test]
    fn test_prepare_loaded_migrates_v1_toml_with_pipeline() {
        let v1_toml = r#"
version = 1
mode = "Trim"
pipeline = ["UrlDecode", "Trim"]
interval_ms = 750
history_enabled = true
"#;
        let config: AppConfig = toml::from_str(v1_toml).expect("デシリアライズに失敗");
        assert_eq!(config.version, 1);
        assert_eq!(
            config.pipeline,
            vec![RefineMode::UrlDecode, RefineMode::Trim]
        );

        let migration = config.prepare_loaded();
        assert!(migration.migrated);
        let prepared = migration.config;
        assert_eq!(prepared.version, consts::CONFIG_VERSION);
        assert_eq!(prepared.mode, RefineMode::Trim);
        assert_eq!(
            prepared.pipeline,
            vec![RefineMode::UrlDecode, RefineMode::Trim]
        );
        assert_eq!(prepared.interval_ms, 750);
        assert!(prepared.history_enabled);
    }

    /// `fix_invalid` が不正なホットキーをデフォルトへ置き換えること
    #[test]
    fn test_hotkey_settings_fix_invalid() {
        let mut hotkeys = HotkeySettings {
            quick_selector: "Bad+Key".to_string(),
            ..HotkeySettings::default()
        };
        hotkeys.fix_invalid();
        assert_eq!(
            hotkeys.quick_selector,
            consts::DEFAULT_HOTKEY_QUICK_SELECTOR
        );
    }

    /// `add_registered_clip` が登録・検証・上限チェックを行うこと
    #[test]
    fn test_add_registered_clip() {
        use super::AddRegisteredClipError;
        use super::types::RegisteredClip;

        let mut config = AppConfig::default();
        assert_eq!(config.add_registered_clip("  hello  "), Ok(()));
        assert_eq!(config.clips.len(), 1);
        assert_eq!(config.clips[0].text, "  hello  ");
        assert!(!config.clips[0].label.is_empty());

        assert_eq!(
            config.add_registered_clip("   ".to_string()),
            Err(AddRegisteredClipError::Empty)
        );

        config.clips = vec![RegisteredClip {
            label: "x".into(),
            text: "y".into(),
            image_file: None,
        }];
        for i in 0..consts::MAX_REGISTERED_CLIPS {
            config.clips.push(RegisteredClip {
                label: format!("l{i}"),
                text: format!("t{i}"),
                image_file: None,
            });
        }
        assert_eq!(
            config.add_registered_clip("overflow"),
            Err(AddRegisteredClipError::LimitReached)
        );
    }

    /// `remove_registered_clip` が指定項目を削除すること
    #[test]
    fn test_remove_registered_clip() {
        use super::types::RegisteredClip;

        let mut config = AppConfig {
            clips: vec![
                RegisteredClip {
                    label: "first".into(),
                    text: "a".into(),
                    image_file: None,
                },
                RegisteredClip {
                    label: "second".into(),
                    text: "b".into(),
                    image_file: None,
                },
            ],
            ..Default::default()
        };

        assert!(config.remove_registered_clip(0));
        assert_eq!(config.clips.len(), 1);
        assert_eq!(config.clips[0].text, "b");
        assert!(!config.remove_registered_clip(5));
    }

    /// お気に入り変換モードの登録・解除と正規化が機能すること
    #[test]
    fn test_favorite_modes() {
        use super::types::{FavoriteMoveDirection, FavoriteToggleResult};

        let mut config = AppConfig::default();
        assert_eq!(
            config.toggle_favorite_mode(RefineMode::Trim),
            FavoriteToggleResult::Added
        );
        assert!(config.is_favorite_mode(RefineMode::Trim));
        assert_eq!(
            config.toggle_favorite_mode(RefineMode::Trim),
            FavoriteToggleResult::Removed
        );
        assert!(!config.is_favorite_mode(RefineMode::Trim));

        config.favorite_modes = vec![RefineMode::Trim, RefineMode::Trim, RefineMode::UrlDecode];
        config.normalize_favorite_modes();
        assert_eq!(
            config.favorite_modes,
            vec![RefineMode::Trim, RefineMode::UrlDecode]
        );

        config.favorite_modes = vec![
            RefineMode::Trim,
            RefineMode::UrlDecode,
            RefineMode::JsonFormat,
        ];
        assert!(config.move_favorite_mode(RefineMode::JsonFormat, FavoriteMoveDirection::Up));
        assert_eq!(
            config.favorite_modes,
            vec![
                RefineMode::Trim,
                RefineMode::JsonFormat,
                RefineMode::UrlDecode
            ]
        );
        assert!(!config.move_favorite_mode(RefineMode::Trim, FavoriteMoveDirection::Up));
        assert!(config.move_favorite_mode(RefineMode::Trim, FavoriteMoveDirection::Down));
        assert_eq!(
            config.favorite_modes,
            vec![
                RefineMode::JsonFormat,
                RefineMode::Trim,
                RefineMode::UrlDecode
            ]
        );
    }

    /// お気に入りスロットのデフォルトホットキーが解決されること
    #[test]
    fn test_favorite_slot_default_bindings() {
        let hotkeys = HotkeySettings::default();
        assert_eq!(
            hotkeys.favorite_slot_binding(0).as_deref(),
            Some("Alt+Shift+1")
        );
        assert_eq!(
            hotkeys.favorite_slot_binding(8).as_deref(),
            Some("Alt+Shift+9")
        );
        assert_eq!(
            hotkeys.favorite_slot_binding(9).as_deref(),
            Some("Alt+Shift+F1")
        );
    }

    /// 空文字のスロット設定はホットキーを無効化すること
    #[test]
    fn test_favorite_slot_empty_binding_disables_hotkey() {
        let hotkeys = HotkeySettings {
            favorite_mode_slots: vec![String::new()],
            ..HotkeySettings::default()
        };
        assert!(hotkeys.favorite_slot_binding(0).is_none());
        assert_eq!(
            hotkeys.favorite_slot_binding(1).as_deref(),
            Some("Alt+Shift+2")
        );
    }

    /// `pipeline` 未設定時は `mode` のみが有効パイプラインになること
    #[test]
    fn test_effective_pipeline_falls_back_to_mode() {
        let config = AppConfig::default();
        assert_eq!(config.effective_pipeline(), vec![RefineMode::UrlDecode]);
        assert!(!config.is_pipeline_active());
    }

    /// `pipeline` 設定時はその順序で有効パイプラインになること
    #[test]
    fn test_effective_pipeline_uses_configured_chain() {
        let config = AppConfig {
            pipeline: vec![RefineMode::UrlDecode, RefineMode::Trim],
            ..Default::default()
        };
        assert_eq!(
            config.effective_pipeline(),
            vec![RefineMode::UrlDecode, RefineMode::Trim]
        );
        assert!(config.is_pipeline_active());
    }

    /// `pipeline` が TOML に保存・復元されること
    #[test]
    fn test_pipeline_serde_roundtrip() {
        let config = AppConfig {
            pipeline: vec![RefineMode::Trim, RefineMode::JsonFormat],
            ..Default::default()
        };

        let toml_str = toml::to_string(&config).expect("AppConfig のシリアライズに失敗");
        let decoded: AppConfig =
            toml::from_str(&toml_str).expect("AppConfig のデシリアライズに失敗");
        assert_eq!(decoded.pipeline, config.pipeline);
    }

    /// `normalize_pipeline` が画像モードを末尾へ移動し件数を制限すること
    #[test]
    fn test_normalize_pipeline_moves_image_mode_to_end() {
        let mut config = AppConfig {
            pipeline: vec![
                RefineMode::ExcelToImage,
                RefineMode::Trim,
                RefineMode::UrlDecode,
            ],
            ..Default::default()
        };
        config.normalize_pipeline();
        assert_eq!(
            config.pipeline,
            vec![
                RefineMode::Trim,
                RefineMode::UrlDecode,
                RefineMode::ExcelToImage,
            ]
        );
    }
}
