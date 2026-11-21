use super::types::AppConfig;
use crate::consts;

// ======================================================================
// 移行結果
// ======================================================================
/// 設定読み込み後のスキーマ移行結果
#[derive(Debug, Clone)]
pub struct ConfigMigration {
    /// 移行後の設定
    pub config: AppConfig,
    /// スキーマ移行を実行し、ディスクへ書き戻してよい
    pub migrated: bool,
    /// 現行アプリより新しい `version` を検出し、番号のみ現行へ揃えた
    pub newer_version_clamped: bool,
    /// 移行手順が無い `version` を検出し、既存項目を維持した
    pub unsupported_version_kept: bool,
}

// ======================================================================
// 公開関数
// ======================================================================
/// 未対応スキーマ `version` を検出したときのユーザー向けメッセージ
///
/// # Returns
/// * `&'static str` - デスクトップ通知・CLI 向けの固定文言
#[must_use]
pub fn format_unsupported_config_version_message() -> &'static str {
    "未対応の設定スキーマ version を検出した。既存の項目は維持する。ディスク上の version は自動では書き換えない"
}

// ======================================================================
// 移行エントリ
// ======================================================================
/// 保存済み設定を現行スキーマ (`CONFIG_VERSION`) へ順次移行する
///
/// `version` が現行より新しい場合は既存項目を維持し、`version` 番号のみ現行へ揃える
/// 移行手順が無い番号はデフォルトへ置換せず、既存項目を維持する (ディスクへは自動保存しない)
///
/// # Arguments
/// * `config` - 移行対象の設定
///
/// # Returns
/// * `ConfigMigration` - 移行後の設定と移行メタデータ
pub fn migrate_config(config: AppConfig) -> ConfigMigration {
    migrate_config_to(config, consts::CONFIG_VERSION)
}

/// 指定スキーマ番号へ順次移行する
///
/// テストでは現行より先の番号を渡し、移行ステップ欠落時の維持方針を検証する
fn migrate_config_to(config: AppConfig, target_version: u32) -> ConfigMigration {
    if config.version > target_version {
        let saved_version = config.version;
        tracing::warn!(
            "設定 version={saved_version} はアプリが対応する version={target_version} より新しい。既存項目を維持し version を現行へ揃える"
        );
        let mut config = config;
        config.version = target_version;
        return ConfigMigration {
            config,
            migrated: true,
            newer_version_clamped: true,
            unsupported_version_kept: false,
        };
    }

    let mut config = config;
    let mut migrated = false;

    while config.version != target_version {
        let from = config.version;
        config = match from {
            0 => migrate_v0_to_v1(config),
            1 => migrate_v1_to_v2(config),
            2 => migrate_v2_to_v3(config),
            3 => migrate_v3_to_v4(config),
            v => {
                tracing::warn!(
                    "未対応の設定 version={v} (現行={target_version})。既存項目を維持する。ディスクへは自動保存しない"
                );
                config.version = target_version;
                return ConfigMigration {
                    config,
                    migrated: false,
                    newer_version_clamped: false,
                    unsupported_version_kept: true,
                };
            }
        };
        migrated = true;
        tracing::info!("設定を v{from} から v{} へ移行した", config.version);
    }

    ConfigMigration {
        config,
        migrated,
        newer_version_clamped: false,
        unsupported_version_kept: false,
    }
}

// ======================================================================
// バージョン別移行
// ======================================================================
/// v0 から v1 へ移行
///
/// v1 で追加: `favorite_modes` (お気に入り変換モード)
/// v0 設定に当フィールドが無い場合は空配列として扱う
fn migrate_v0_to_v1(mut config: AppConfig) -> AppConfig {
    config.version = 1;
    config
}

/// v1 から v2 へ移行
///
/// v2 で変更: `[[texts]]` → 登録クリップ (`registered-clips.dat`)、`hotkeys.text_selector` → `hotkeys.clip_selector`
fn migrate_v1_to_v2(mut config: AppConfig) -> AppConfig {
    config.version = 2;
    config
}

/// v2 から v3 へ移行
///
/// v3 で変更: フラット `[regex]` を名前付きプリセット + ステップ列へ包む
/// デシリアライズ時点で既にプリセット形式へ変換済みの場合は正規化のみ行う
fn migrate_v2_to_v3(mut config: AppConfig) -> AppConfig {
    config.regex.normalize();
    config.version = 3;
    config
}

/// v3 から v4 へ移行
///
/// v4 で追加: `auto_refine_enabled` (新規設定の既定は無効)
/// 既存利用者はコピー時の自動変換を使っていたため、移行時は有効にして挙動を維持する
fn migrate_v3_to_v4(mut config: AppConfig) -> AppConfig {
    config.auto_refine_enabled = true;
    config.version = 4;
    config
}

// ======================================================================
// テスト
// ======================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::consts;
    use crate::refiner::RefineMode;

    /// 現行バージョンの設定は移行不要となるようにすること
    #[test]
    fn migrate_current_version_is_noop() {
        let config = AppConfig::default();
        let result = migrate_config(config);
        assert!(!result.migrated);
        assert_eq!(result.config.version, consts::CONFIG_VERSION);
    }

    /// v0 設定は v1 へ移行し、既存項目を保持すること
    #[test]
    fn migrate_v0_to_v1_preserves_settings() {
        let config = AppConfig {
            version: 0,
            mode: RefineMode::Trim,
            interval_ms: 500,
            history_enabled: true,
            ..Default::default()
        };
        let result = migrate_config(config);
        assert!(result.migrated);
        assert_eq!(result.config.version, consts::CONFIG_VERSION);
        assert_eq!(result.config.mode, RefineMode::Trim);
        assert_eq!(result.config.interval_ms, 500);
        assert!(result.config.history_enabled);
    }

    /// v0 設定の移行後は `favorite_modes` が空配列であようにすること
    #[test]
    fn migrate_v0_to_v1_initializes_favorite_modes() {
        let config = AppConfig {
            version: 0,
            ..Default::default()
        };
        let result = migrate_config(config);
        assert!(result.migrated);
        assert!(result.config.favorite_modes.is_empty());
    }

    /// 現行より新しい version は既存項目を維持し version 番号のみ現行へ揃えようにすること
    #[test]
    fn migrate_newer_version_clamps_version_and_preserves_settings() {
        let config = AppConfig {
            version: consts::CONFIG_VERSION + 1,
            mode: RefineMode::Trim,
            interval_ms: 500,
            history_enabled: true,
            ..Default::default()
        };
        let result = migrate_config(config);
        assert!(result.migrated);
        assert!(result.newer_version_clamped);
        assert_eq!(result.config.version, consts::CONFIG_VERSION);
        assert_eq!(result.config.mode, RefineMode::Trim);
        assert_eq!(result.config.interval_ms, 500);
        assert!(result.config.history_enabled);
    }

    /// 現行 version 指定時は `migrated` にならなようにすること
    #[test]
    fn migrate_at_current_version_does_not_mark_migrated() {
        let config = AppConfig {
            version: consts::CONFIG_VERSION,
            ..Default::default()
        };
        let result = migrate_config(config);
        assert!(!result.migrated);
    }

    /// v1 設定は v2 へ移行し、加工パイプラインを保持すること
    #[test]
    fn migrate_v1_to_v2_preserves_pipeline() {
        let config = AppConfig {
            version: 1,
            mode: RefineMode::JsonFormat,
            pipeline: vec![RefineMode::UrlDecode, RefineMode::Trim],
            interval_ms: 333,
            ..Default::default()
        };
        let result = migrate_config(config);
        assert!(result.migrated);
        assert_eq!(result.config.version, consts::CONFIG_VERSION);
        assert_eq!(result.config.mode, RefineMode::JsonFormat);
        assert_eq!(
            result.config.pipeline,
            vec![RefineMode::UrlDecode, RefineMode::Trim]
        );
        assert_eq!(result.config.interval_ms, 333);
    }

    /// v0 設定は現行バージョンまで連鎖移行できるようにすること
    #[test]
    fn migrate_v0_to_current_preserves_settings() {
        let config = AppConfig {
            version: 0,
            mode: RefineMode::UrlEncode,
            interval_ms: 777,
            history_enabled: true,
            ..Default::default()
        };
        let result = migrate_config(config);
        assert!(result.migrated);
        assert_eq!(result.config.version, consts::CONFIG_VERSION);
        assert_eq!(result.config.mode, RefineMode::UrlEncode);
        assert_eq!(result.config.interval_ms, 777);
        assert!(result.config.history_enabled);
        assert!(result.config.favorite_modes.is_empty());
    }

    /// v2 設定は v3 へ移行し、正規表現プリセットを保持すること
    #[test]
    fn migrate_v2_to_v3_preserves_regex_presets() {
        use crate::config::{RegexPreset, RegexSettings, RegexStep};

        let config = AppConfig {
            version: 2,
            regex: RegexSettings {
                active_preset_id: "p1".to_string(),
                presets: vec![RegexPreset {
                    id: "p1".to_string(),
                    name: "日付".to_string(),
                    steps: vec![RegexStep {
                        pattern: r"\d+".to_string(),
                        replacement: "N".to_string(),
                        ..RegexStep::default()
                    }],
                }],
            },
            ..Default::default()
        };
        let result = migrate_config(config);
        assert!(result.migrated);
        assert_eq!(result.config.version, consts::CONFIG_VERSION);
        assert_eq!(result.config.regex.active_preset_id, "p1");
        assert_eq!(result.config.regex.active_steps()[0].pattern, r"\d+");
    }

    /// v3 設定は v4 へ移行し、コピー時の自動変換を有効にすること
    #[test]
    fn migrate_v3_to_v4_enables_auto_refine() {
        let config = AppConfig {
            version: 3,
            mode: RefineMode::Trim,
            ..Default::default()
        };
        let result = migrate_config(config);
        assert!(result.migrated);
        assert_eq!(result.config.version, consts::CONFIG_VERSION);
        assert!(result.config.auto_refine_enabled);
        assert_eq!(result.config.mode, RefineMode::Trim);
    }

    /// `auto_refine_enabled` を持たない v3 TOML 相当は自動変換有効として読み込むこと
    #[test]
    fn migrate_v3_toml_without_auto_refine_key_enables_auto_refine() {
        let toml = r#"
version = 3
mode = "UrlDecode"
interval_ms = 1000
"#;
        let config: AppConfig = toml::from_str(toml).expect("v3 TOML のデシリアライズ");
        assert!(!config.auto_refine_enabled);
        let result = migrate_config(config);
        assert!(result.config.auto_refine_enabled);
    }

    /// 新規の既定設定ではコピー時の自動変換を無効とすること
    #[test]
    fn default_config_disables_auto_refine() {
        assert!(!AppConfig::default().auto_refine_enabled);
    }

    /// 旧フラット regex を含む v2 TOML 相当はプリセットへ包まれること
    #[test]
    fn migrate_v2_legacy_flat_regex_via_deserialize() {
        let toml = r#"
version = 2
mode = "RegexReplace"
interval_ms = 1000
[regex]
pattern = "foo"
replacement = "bar"
case_insensitive = true
"#;
        let config: AppConfig = toml::from_str(toml).expect("parse");
        let result = migrate_config(config);
        assert!(result.migrated);
        assert_eq!(result.config.version, consts::CONFIG_VERSION);
        assert_eq!(result.config.regex.presets.len(), 1);
        assert_eq!(result.config.regex.active_steps()[0].pattern, "foo");
        assert_eq!(result.config.regex.active_steps()[0].replacement, "bar");
        assert!(result.config.regex.active_steps()[0].case_insensitive);
        assert!(!result.unsupported_version_kept);
    }

    /// 移行ステップが無い version はデフォルトへ置換せず既存項目を維持すること
    #[test]
    fn migrate_missing_step_keeps_settings_without_marking_migrated() {
        let config = AppConfig {
            version: consts::CONFIG_VERSION,
            mode: RefineMode::Trim,
            interval_ms: 500,
            history_enabled: true,
            ..Default::default()
        };
        let result = migrate_config_to(config, consts::CONFIG_VERSION + 2);
        assert!(!result.migrated);
        assert!(!result.newer_version_clamped);
        assert!(result.unsupported_version_kept);
        assert_eq!(result.config.version, consts::CONFIG_VERSION + 2);
        assert_eq!(result.config.mode, RefineMode::Trim);
        assert_eq!(result.config.interval_ms, 500);
        assert!(result.config.history_enabled);
    }

    /// 連鎖移行の途中でステップが欠けても、そこまでの項目を維持すること
    #[test]
    fn migrate_chain_keeps_settings_when_later_step_is_missing() {
        let config = AppConfig {
            version: 0,
            mode: RefineMode::UrlEncode,
            interval_ms: 777,
            history_enabled: true,
            ..Default::default()
        };
        let result = migrate_config_to(config, consts::CONFIG_VERSION + 1);
        assert!(!result.migrated);
        assert!(result.unsupported_version_kept);
        assert_eq!(result.config.version, consts::CONFIG_VERSION + 1);
        assert_eq!(result.config.mode, RefineMode::UrlEncode);
        assert_eq!(result.config.interval_ms, 777);
        assert!(result.config.history_enabled);
        assert!(result.config.favorite_modes.is_empty());
    }

    /// 未対応 version の案内文言が既存維持を伝えること
    #[test]
    fn format_unsupported_config_version_message_mentions_keep() {
        let message = format_unsupported_config_version_message();
        assert!(message.contains("未対応"));
        assert!(message.contains("既存"));
        assert!(message.contains("自動では書き換えない"));
    }
}
