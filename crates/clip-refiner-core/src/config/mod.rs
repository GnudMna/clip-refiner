//! 正規表現設定型と (feature `config`) 設定永続化 API
//!
//! [`RegexSettings`] は加工 API が常に利用する。`AppConfig` と `config.toml` I/O は
//! feature `config` が必要。`RefineContext::from_config` も同じ feature に属する

#[cfg(feature = "config")]
mod atomic_io;
#[cfg(feature = "config")]
mod clip_store_key;
#[cfg(feature = "config")]
mod migrate;
#[cfg(feature = "config")]
mod paths;
#[cfg(feature = "config")]
pub mod permissions;
#[cfg(feature = "config")]
mod persistence;
#[cfg(feature = "config")]
mod registered_clips;
#[cfg(feature = "config")]
mod registered_images;
#[cfg(feature = "config")]
mod serialize;
#[cfg(all(test, feature = "config"))]
mod spec;
mod types;
#[cfg(feature = "config")]
mod unknown_keys;

pub use types::{RegexPreset, RegexSettings, RegexStep};

#[cfg(feature = "config")]
pub use crate::consts::CONFIG_VERSION;
#[cfg(feature = "config")]
pub use crate::error::ConfigError;
#[cfg(feature = "config")]
pub use migrate::format_unsupported_config_version_message;
#[cfg(all(
    feature = "config",
    any(test, debug_assertions, feature = "test-helpers")
))]
#[doc(hidden)]
pub use paths::{begin_capture_open_path_for_test, take_captured_open_path_for_test};
#[cfg(all(
    feature = "config",
    any(test, debug_assertions, feature = "test-helpers")
))]
#[doc(hidden)]
pub use paths::{clear_test_config_dir, set_test_config_dir};
#[cfg(feature = "config")]
pub use paths::{get_config_dir, is_portable_mode, open_config_file};
#[cfg(feature = "config")]
pub use persistence::{
    ConfigDiskReload, ConfigFileFingerprint, ConfigLoadFallback, ConfigLoadOutcome,
    ConfigReloadError, disk_config_content_fingerprint, disk_config_modified_time,
};
#[cfg(feature = "config")]
pub use registered_clips::RegisteredClipsSaveOutcome;
#[cfg(feature = "config")]
pub use types::{
    AddRegisteredClipError, AppConfig, FavoriteMoveDirection, FavoriteToggleResult, HotkeyFix,
    HotkeyFixKind, HotkeySettings, MonitorMode, NotificationSettings, RegisteredClip, ResolvedClip,
    SelectorWebviewRetention, UpdateRegisteredClipError, format_hotkey_fixes_message,
};
#[cfg(feature = "config")]
pub use unknown_keys::format_unknown_config_keys_message;

// ======================================================================
// テスト
// ======================================================================
#[cfg(all(test, feature = "config"))]
mod tests {
    use super::*;
    use crate::consts;
    use crate::refiner::RefineMode;

    /// `AppConfig` のデフォルト値が正しいとなるようにすること
    #[test]
    fn test_app_config_default() {
        let config = AppConfig::default();
        assert_eq!(config.version, consts::CONFIG_VERSION);
        assert_eq!(config.interval_ms, 1000);
        assert_eq!(config.mode, RefineMode::UrlDecode);
        assert_eq!(config.history_limit, consts::DEFAULT_HISTORY_LIMIT);
        assert_eq!(config.hotkeys, HotkeySettings::default());
        assert_eq!(
            config.selector_webview_retention,
            SelectorWebviewRetention::Keep
        );
        assert_eq!(
            config.selector_webview_idle_minutes,
            consts::DEFAULT_SELECTOR_WEBVIEW_IDLE_MINUTES
        );
    }

    /// v4 以前の `hotkeys.pause` / `notify_pause` を新しいキーとして読み込めること
    #[test]
    fn legacy_pause_keys_load_as_auto_refine() {
        let toml = r#"
version = 4
mode = "Trim"
interval_ms = 1000

[notification_settings]
notify_pause = false

[hotkeys]
pause = "Ctrl+Alt+P"
"#;

        let config: AppConfig = toml::from_str(toml).expect("v4 TOML のデシリアライズ");

        assert!(!config.notification_settings.notify_auto_refine);
        assert_eq!(config.hotkeys.auto_refine, "Ctrl+Alt+P");
    }

    /// 新しい `hotkeys.auto_refine` / `notify_auto_refine` を読み込めること
    #[test]
    fn auto_refine_keys_load() {
        let toml = r#"
version = 5
mode = "Trim"
interval_ms = 1000

[notification_settings]
notify_auto_refine = false

[hotkeys]
auto_refine = "Ctrl+Alt+R"
"#;

        let config: AppConfig = toml::from_str(toml).expect("TOML のデシリアライズ");

        assert!(!config.notification_settings.notify_auto_refine);
        assert_eq!(config.hotkeys.auto_refine, "Ctrl+Alt+R");
    }

    /// 読み込み時に自動変換と連続記録が同時に有効なら、連続記録を優先すること
    #[test]
    fn prepare_loaded_disables_auto_refine_with_continuous_record() {
        let config = AppConfig {
            auto_refine_enabled: true,
            continuous_record_enabled: true,
            ..AppConfig::default()
        };

        let prepared = config.prepare_loaded().config;

        assert!(!prepared.auto_refine_enabled);
        assert!(prepared.continuous_record_enabled);
    }

    /// 保存前の正規化でも自動変換と連続記録の同時有効を解消すること
    #[test]
    fn normalize_disables_auto_refine_with_continuous_record() {
        let mut config = AppConfig {
            auto_refine_enabled: true,
            continuous_record_enabled: true,
            ..AppConfig::default()
        };

        config.normalize();

        assert!(!config.auto_refine_enabled);
        assert!(config.continuous_record_enabled);
    }

    /// 保存した設定に廃止した `is_paused` / `pause` / `notify_pause` を出力しないこと
    #[test]
    fn serialized_config_omits_removed_pause_keys() {
        let toml =
            serialize::config_to_toml(&AppConfig::default(), None).expect("設定の TOML 生成");

        assert!(!toml.contains("is_paused"));
        assert!(!toml.contains("notify_pause"));
        assert!(!toml.contains("pause"));
        assert!(toml.contains("auto_refine"));
        assert!(toml.contains("notify_auto_refine"));
        let reloaded: AppConfig = toml::from_str(&toml).expect("生成結果のデシリアライズ");
        assert_eq!(
            reloaded.hotkeys.auto_refine,
            consts::DEFAULT_HOTKEY_AUTO_REFINE
        );
    }

    /// 旧キーを含む既存ファイルを保存し直すと、旧キーを除去して新キーへ置き換えること
    #[test]
    fn saving_over_legacy_file_replaces_pause_keys() {
        let existing = r#"
version = 4
auto_refine_enabled = true
mode = "Trim"
interval_ms = 1000
is_paused = true

[notification_settings]
notify_pause = false

[hotkeys]
pause = "Ctrl+Alt+P"
"#;
        let loaded: AppConfig = toml::from_str(existing).expect("v4 TOML のデシリアライズ");
        let config = loaded.prepare_loaded().config;

        let toml = serialize::config_to_toml(&config, Some(existing)).expect("設定の TOML 生成");

        assert!(!toml.contains("is_paused"));
        assert!(!toml.contains("notify_pause"));
        assert!(!toml.contains("\npause "));
        // 保存結果を読み直しても、旧設定の内容が新キーに引き継がれていること
        let reloaded: AppConfig = toml::from_str(&toml).expect("保存結果のデシリアライズ");
        assert_eq!(reloaded.version, consts::CONFIG_VERSION);
        assert!(!reloaded.auto_refine_enabled);
        assert!(!reloaded.legacy_is_paused);
        assert!(!reloaded.notification_settings.notify_auto_refine);
        assert_eq!(reloaded.hotkeys.auto_refine, "Ctrl+Alt+P");
    }

    /// `AppConfig` のシリアライズ/デシリアライズ往復できるようにすること
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

    /// `NotificationSettings` のデフォルト値が正しいとなるようにすること
    #[test]
    fn test_notification_settings_default() {
        let ns = NotificationSettings::default();
        assert!(!ns.enabled, "enabled のデフォルトは false");
        assert!(ns.notify_process);
        assert!(!ns.notify_result, "notify_result のデフォルトは false");
        assert!(ns.notify_auto_refine);
        assert!(ns.notify_history_restore);
        assert!(ns.notify_ocr);
        assert!(ns.notify_registered_clip);
        assert!(ns.notify_favorites);
        assert!(ns.notify_continuous_record);
    }

    /// `notification_settings.enabled` が TOML に保存・復元されるようにすること
    #[test]
    fn test_notification_settings_serde_roundtrip() {
        let mut config = AppConfig::default();
        config.notification_settings.enabled = true;
        config.notification_settings.notify_result = false;
        config.notification_settings.notify_ocr = false;

        let toml_str = toml::to_string(&config).expect("AppConfig のシリアライズに失敗");
        let decoded: AppConfig =
            toml::from_str(&toml_str).expect("AppConfig のデシリアライズに失敗");
        assert!(decoded.notification_settings.enabled);
        assert!(!decoded.notification_settings.notify_result);
        assert!(!decoded.notification_settings.notify_ocr);
        assert!(!toml_str.contains("notify_mode"));
        assert!(!toml_str.contains("notify_actions"));
    }

    /// 旧 `notify_actions` OFF は個別カテゴリへ移行されるようにすること
    #[test]
    fn test_notification_settings_migrates_legacy_actions() {
        let toml_str = r"
enabled = true
notify_actions = false
";
        let mut settings: NotificationSettings =
            toml::from_str(toml_str).expect("NotificationSettings のデシリアライズに失敗");
        settings.migrate_legacy_fields();
        assert!(!settings.notify_history_restore);
        assert!(!settings.notify_ocr);
        assert!(!settings.notify_registered_clip);
        assert!(!settings.notify_favorites);
        assert!(!settings.notify_continuous_record);
    }

    /// 旧 `notify_mode` と `notify_result` が両方 OFF の場合は加工完了を無効化すること
    #[test]
    fn test_notification_settings_migrates_legacy_process() {
        let toml_str = r"
enabled = true
notify_mode = false
notify_result = false
";
        let mut settings: NotificationSettings =
            toml::from_str(toml_str).expect("NotificationSettings のデシリアライズに失敗");
        settings.migrate_legacy_fields();
        assert!(!settings.notify_process);
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

    /// v0 TOML は移行後に現行バージョンへ更新され、設定値と `favorite_modes` が保持されるようにすること
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
        assert_eq!(prepared.favorite_modes.len(), 0);
    }

    /// v0 TOML を保存すると現行スキーマで書き出されるようにすること
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

    /// version 未指定の TOML は現行スキーマとして読み込まれ、移行不要となるようにすること
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

    /// `fix_invalid` が不正なホットキーをデフォルトへ置き換えるようにすること
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

    /// `fix_duplicates` が固定割り当ての重複をデフォルトへ戻すようにすること
    #[test]
    fn test_hotkey_settings_fix_duplicate_fixed_fields() {
        let mut hotkeys = HotkeySettings {
            notification: "Alt+Shift+P".to_string(),
            auto_refine: "Alt+Shift+P".to_string(),
            ..HotkeySettings::default()
        };
        hotkeys.fix_duplicates();
        assert_eq!(hotkeys.notification, "Alt+Shift+P");
        assert_eq!(hotkeys.auto_refine, consts::DEFAULT_HOTKEY_AUTO_REFINE);
    }

    /// `fix_duplicates` が固定割り当てと重複するお気に入りスロットを無効化すること
    #[test]
    fn test_hotkey_settings_fix_duplicate_favorite_slot() {
        let mut hotkeys = HotkeySettings {
            quick_selector: "Alt+Shift+1".to_string(),
            ..HotkeySettings::default()
        };
        hotkeys.fix_duplicates();
        assert_eq!(hotkeys.quick_selector, "Alt+Shift+1");
        assert_eq!(hotkeys.favorite_mode_slots, vec![String::new()]);
        assert!(hotkeys.favorite_slot_binding(0).is_none());
    }

    /// `fix_duplicates` がお気に入りスロット同士の重複を後勝ちで無効化すること
    #[test]
    fn test_hotkey_settings_fix_duplicate_among_favorite_slots() {
        let mut hotkeys = HotkeySettings {
            favorite_mode_slots: vec!["Alt+Shift+F10".to_string(), "Alt+Shift+F10".to_string()],
            ..HotkeySettings::default()
        };
        hotkeys.fix_duplicates();
        assert_eq!(hotkeys.favorite_mode_slots[0], "Alt+Shift+F10");
        assert_eq!(hotkeys.favorite_mode_slots[1].len(), 0);
    }

    /// `add_registered_clip` が登録・検証・上限チェックを行い、平文を常駐させないようにすること
    #[test]
    fn test_add_registered_clip() {
        use super::AddRegisteredClipError;

        crate::test_helpers::with_temp_config_dir(|| {
            let mut config = AppConfig::default();
            assert_eq!(config.add_registered_clip("  hello  "), Ok(()));
            assert_eq!(config.clips.len(), 1);
            assert_ne!(config.clips[0].text_ciphertext.len(), 0);
            assert_ne!(config.clips[0].label.len(), 0);
            assert_ne!(config.clips[0].preview.len(), 0);
            let resolved = config.resolve_registered_clip(0).expect("resolve");
            match resolved {
                super::ResolvedClip::Text(text) => assert_eq!(text.as_str(), "  hello  "),
                super::ResolvedClip::Image { .. } => panic!("text expected"),
            }
            // 非機密の短い本文は preview に残るが、ストアには text_ciphertext のみ
            assert_eq!(config.clips[0].preview, "  hello  ");

            // 機密らしい本文は preview がマスクされる
            config.clips.clear();
            let jwt = "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.dozjgNryP4J3jVmNHl0w5N_XgL0n3I9PlFUP0THsR8U";
            assert_eq!(config.add_registered_clip(jwt), Ok(()));
            assert!(!config.clips[0].preview.contains(jwt));
            assert_eq!(
                config.clips[0].preview,
                crate::consts::SENSITIVE_SNIPPET_LABEL
            );

            assert_eq!(
                config.add_registered_clip("   ".to_string()),
                Err(AddRegisteredClipError::Empty)
            );

            config.clips.clear();
            for i in 0..consts::MAX_REGISTERED_CLIPS {
                assert_eq!(config.add_registered_clip(format!("t{i}")), Ok(()));
            }
            assert_eq!(
                config.add_registered_clip("overflow"),
                Err(AddRegisteredClipError::LimitReached)
            );
        });
    }

    /// `remove_registered_clip` が指定項目を削除すること
    #[test]
    fn test_remove_registered_clip() {
        crate::test_helpers::with_temp_config_dir(|| {
            let mut config = AppConfig::default();
            config.add_registered_clip("a").expect("add a");
            config.add_registered_clip("b").expect("add b");

            assert!(config.remove_registered_clip(0));
            assert_eq!(config.clips.len(), 1);
            let resolved = config.resolve_registered_clip(0).expect("resolve");
            match resolved {
                super::ResolvedClip::Text(text) => assert_eq!(text.as_str(), "b"),
                super::ResolvedClip::Image { .. } => panic!("text expected"),
            }
            assert!(!config.remove_registered_clip(5));
        });
    }

    /// `move_registered_clip` が隣の項目と入れ替わり、端と範囲外では移動しないこと
    #[test]
    fn test_move_registered_clip() {
        use super::types::FavoriteMoveDirection;

        crate::test_helpers::with_temp_config_dir(|| {
            let mut config = AppConfig::default();
            for text in ["a", "b", "c"] {
                config.add_registered_clip(text).expect("add");
            }
            let texts = |config: &AppConfig| -> Vec<String> {
                (0..config.clips.len())
                    .map(
                        |i| match config.resolve_registered_clip(i).expect("resolve") {
                            super::ResolvedClip::Text(text) => text.as_str().to_string(),
                            super::ResolvedClip::Image { .. } => panic!("text expected"),
                        },
                    )
                    .collect()
            };

            assert!(config.move_registered_clip(2, FavoriteMoveDirection::Up));
            assert_eq!(texts(&config), ["a", "c", "b"]);
            assert!(config.move_registered_clip(0, FavoriteMoveDirection::Down));
            assert_eq!(texts(&config), ["c", "a", "b"]);

            assert!(!config.move_registered_clip(0, FavoriteMoveDirection::Up));
            assert!(!config.move_registered_clip(2, FavoriteMoveDirection::Down));
            assert!(!config.move_registered_clip(3, FavoriteMoveDirection::Up));
            assert_eq!(texts(&config), ["c", "a", "b"]);
        });
    }

    /// 識別トークンが登録ごとに異なり、並べ替え・テンプレート・連番では変わらず、本文の更新で変わること
    #[test]
    fn test_registered_clip_identity_token() {
        use super::types::FavoriteMoveDirection;

        crate::test_helpers::with_temp_config_dir(|| {
            let mut config = AppConfig::default();
            config.add_registered_clip("same").expect("add a");
            config.add_registered_clip("same").expect("add b");
            let tokens: Vec<u64> = config
                .clips
                .iter()
                .map(super::types::RegisteredClip::identity_token)
                .collect();
            // 同じ本文でもノンスが異なるため区別できる
            assert_ne!(tokens[0], tokens[1]);

            // 位置・テンプレート指定・連番が変わっても同じ登録を指す
            assert!(config.move_registered_clip(0, FavoriteMoveDirection::Down));
            assert_eq!(config.clips[1].identity_token(), tokens[0]);
            assert!(config.set_registered_clip_template(1, true));
            assert!(config.set_registered_clip_counter(1, 3));
            assert_eq!(config.clips[1].identity_token(), tokens[0]);

            // 本文を更新すると別物として扱う
            config
                .update_registered_clip(1, "", "changed")
                .expect("update");
            assert_ne!(config.clips[1].identity_token(), tokens[0]);
        });
    }

    /// `set_registered_clip_template` がテキスト登録だけを切り替え、変化の有無を返すこと
    #[test]
    fn test_set_registered_clip_template() {
        crate::test_helpers::with_temp_config_dir(|| {
            let mut config = AppConfig::default();
            config.add_registered_clip("{date}").expect("add");
            config
                .add_registered_image(1, 1, &[0, 0, 0, 255])
                .expect("add image");

            assert!(config.set_registered_clip_template(0, true));
            assert!(config.clips[0].template);
            // 同じ値・画像登録・範囲外は変更しない
            assert!(!config.set_registered_clip_template(0, true));
            assert!(!config.set_registered_clip_template(1, true));
            assert!(!config.clips[1].template);
            assert!(!config.set_registered_clip_template(2, true));

            assert!(config.set_registered_clip_template(0, false));
            assert!(!config.clips[0].template);
        });
    }

    /// `set_registered_clip_counter` がテキスト登録だけを更新し、並び替えと本文編集で連番を保つこと
    #[test]
    fn test_set_registered_clip_counter() {
        crate::test_helpers::with_temp_config_dir(|| {
            let mut config = AppConfig::default();
            config.add_registered_clip("No.{counter}").expect("add");
            config.add_registered_clip("other").expect("add");
            config
                .add_registered_image(1, 1, &[0, 0, 0, 255])
                .expect("add image");

            assert!(config.set_registered_clip_counter(0, 5));
            assert_eq!(config.clips[0].counter, 5);
            // 同じ値・画像登録・範囲外は変更しない
            assert!(!config.set_registered_clip_counter(0, 5));
            assert!(!config.set_registered_clip_counter(2, 1));
            assert!(!config.set_registered_clip_counter(3, 1));

            // 並び替えと本文編集では連番を引き継ぐ
            assert!(config.move_registered_clip(0, FavoriteMoveDirection::Down));
            assert_eq!(config.clips[1].counter, 5);
            config
                .update_registered_clip(1, "", "No.{counter} v2")
                .expect("update");
            assert_eq!(config.clips[1].counter, 5);

            assert!(config.set_registered_clip_counter(1, 0));
            assert_eq!(config.clips[1].counter, 0);
        });
    }

    /// `update_registered_clip` が本文・ラベル・プレビューを更新し、テンプレート指定と位置を保つこと
    #[test]
    fn test_update_registered_clip() {
        crate::test_helpers::with_temp_config_dir(|| {
            let mut config = AppConfig::default();
            config.add_registered_clip("first").expect("add");
            config.add_registered_clip("second").expect("add");
            assert!(config.set_registered_clip_template(1, true));

            assert_eq!(
                config.update_registered_clip(1, "  新ラベル  ", "  updated body  "),
                Ok(())
            );
            assert_eq!(config.clips.len(), 2);
            assert_eq!(config.clips[1].label, "新ラベル");
            assert_eq!(config.clips[1].preview, "  updated body  ");
            assert!(config.clips[1].template);
            match config.resolve_registered_clip(1).expect("resolve") {
                super::ResolvedClip::Text(text) => assert_eq!(text.as_str(), "  updated body  "),
                super::ResolvedClip::Image { .. } => panic!("text expected"),
            }
            // 他の項目は変わらない
            match config.resolve_registered_clip(0).expect("resolve") {
                super::ResolvedClip::Text(text) => assert_eq!(text.as_str(), "first"),
                super::ResolvedClip::Image { .. } => panic!("text expected"),
            }

            // 空白のみのラベルは新しいプレビューから補完し、長いラベルは上限で切り詰める
            assert_eq!(
                config.update_registered_clip(0, "   ", "from preview"),
                Ok(())
            );
            assert_eq!(config.clips[0].label, "from preview");
            let long_label = "あ".repeat(consts::MAX_REGISTERED_CLIP_LABEL_CHARS + 10);
            assert_eq!(config.update_registered_clip(0, &long_label, "x"), Ok(()));
            assert_eq!(
                config.clips[0].label.chars().count(),
                consts::MAX_REGISTERED_CLIP_LABEL_CHARS
            );
        });
    }

    /// `update_registered_clip` が不正な更新を拒否し、元の内容を変えないこと
    #[test]
    fn test_update_registered_clip_rejects_invalid() {
        use super::UpdateRegisteredClipError;

        crate::test_helpers::with_temp_config_dir(|| {
            let mut config = AppConfig::default();
            config.add_registered_clip("keep").expect("add");
            config
                .add_registered_image(1, 1, &[0, 0, 0, 255])
                .expect("add image");
            let before = config.clips[0].clone();

            assert_eq!(
                config.update_registered_clip(0, "l", "   "),
                Err(UpdateRegisteredClipError::Empty)
            );
            assert_eq!(
                config.update_registered_clip(
                    0,
                    "l",
                    &"a".repeat(consts::MAX_CLIPBOARD_TEXT_BYTES + 1)
                ),
                Err(UpdateRegisteredClipError::TooLarge)
            );
            assert_eq!(
                config.update_registered_clip(1, "l", "text"),
                Err(UpdateRegisteredClipError::ImageClip)
            );
            assert_eq!(
                config.update_registered_clip(2, "l", "text"),
                Err(UpdateRegisteredClipError::NotFound)
            );
            assert_eq!(config.clips[0], before);
        });
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

    /// お気に入りスロットのデフォルトホットキーが解決されるようにすること
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

    /// `pipeline` 未設定時は `mode` のみが有効パイプラインになるようにすること
    #[test]
    fn test_effective_pipeline_falls_back_to_mode() {
        let config = AppConfig::default();
        assert_eq!(config.effective_pipeline(), vec![RefineMode::UrlDecode]);
        assert!(!config.is_pipeline_active());
    }

    /// `pipeline` 設定時はその順序で有効パイプラインになるようにすること
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

    /// `pipeline` が TOML に保存・復元されるようにすること
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

    /// `normalize_pipeline` が画像モードを末尾へ移動すること
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

    /// 段数が上限を超えても末尾の画像モードを残し、テキスト加工の超過分を切り捨てること
    #[test]
    fn test_normalize_pipeline_keeps_image_mode_when_truncating() {
        let mut pipeline = vec![RefineMode::ExcelToImage];
        pipeline.extend(std::iter::repeat_n(
            RefineMode::Trim,
            consts::MAX_PIPELINE_LENGTH,
        ));
        let mut config = AppConfig {
            pipeline,
            ..Default::default()
        };
        config.normalize_pipeline();

        assert_eq!(config.pipeline.len(), consts::MAX_PIPELINE_LENGTH);
        assert_eq!(config.pipeline.last(), Some(&RefineMode::ExcelToImage));
        // 画像モード以外はテキスト加工のみ
        assert!(
            config.pipeline[..consts::MAX_PIPELINE_LENGTH - 1]
                .iter()
                .all(|mode| *mode == RefineMode::Trim)
        );
    }
}
