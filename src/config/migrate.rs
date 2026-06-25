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
    /// スキーマ移行または互換性フォールバックが実行された
    pub migrated: bool,
}

// ======================================================================
// 移行エントリ
// ======================================================================
/// 保存済み設定を現行スキーマ (`CONFIG_VERSION`) へ順次移行する
///
/// `version` が現行より新しい場合はデフォルト設定へフォールバックする
/// 未対応の旧バージョンも同様にフォールバックする
pub fn migrate_config(config: AppConfig) -> ConfigMigration {
    if config.version > consts::CONFIG_VERSION {
        crate::log_warn!(
            "設定 version={} はアプリが対応する version={} より新しい。デフォルト設定を使用する",
            config.version,
            consts::CONFIG_VERSION
        );
        return ConfigMigration {
            config: AppConfig::default(),
            migrated: true,
        };
    }

    let mut config = config;
    let mut migrated = false;

    while config.version < consts::CONFIG_VERSION {
        let from = config.version;
        config = match from {
            0 => migrate_v0_to_v1(config),
            v => {
                crate::log_warn!("未対応の設定 version={v}。デフォルト設定を使用する");
                return ConfigMigration {
                    config: AppConfig::default(),
                    migrated: true,
                };
            }
        };
        migrated = true;
        crate::log_info!("設定を v{from} から v{} へ移行した", config.version);
    }

    ConfigMigration { config, migrated }
}

// ======================================================================
// バージョン別移行
// ======================================================================
/// v0 (`version` 未指定または 0) から v1 へ移行
///
/// v1 で `[regex]` および `hotkeys.undo` 等が追加された。
/// 各項目はデシリアライズ時の `serde(default)` で補完済みのため、version の更新と検証のみ行う
fn migrate_v0_to_v1(mut config: AppConfig) -> AppConfig {
    config.hotkeys.fix_invalid();
    config.version = 1;
    config
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use crate::consts;
    use crate::refiner::RefineMode;

    use super::*;

    /// 現行バージョンの設定は移行不要
    #[test]
    fn migrate_current_version_is_noop() {
        let config = AppConfig::default();
        let result = migrate_config(config);
        assert!(!result.migrated);
        assert_eq!(result.config.version, consts::CONFIG_VERSION);
    }

    /// v0 から v1 へ移行し、ユーザー設定を保持する
    #[test]
    fn migrate_v0_to_v1_preserves_user_values() {
        let config = AppConfig {
            version: 0,
            mode: RefineMode::Trim,
            interval_ms: 500,
            ..Default::default()
        };
        let result = migrate_config(config);
        assert!(result.migrated);
        assert_eq!(result.config.version, 1);
        assert_eq!(result.config.mode, RefineMode::Trim);
        assert_eq!(result.config.interval_ms, 500);
    }

    /// 現行より新しい version はデフォルトへフォールバック
    #[test]
    fn migrate_newer_version_falls_back_to_default() {
        let config = AppConfig {
            version: consts::CONFIG_VERSION + 1,
            mode: RefineMode::Trim,
            ..Default::default()
        };
        let result = migrate_config(config);
        assert!(result.migrated);
        let default = AppConfig::default();
        assert_eq!(result.config.version, default.version);
        assert_eq!(result.config.mode, default.mode);
    }

    /// 連鎖移行: 将来 v1→v2 を追加した際はステップごとのテストを足す
    #[test]
    fn migrate_at_current_version_does_not_mark_migrated() {
        let config = AppConfig {
            version: consts::CONFIG_VERSION,
            ..Default::default()
        };
        let result = migrate_config(config);
        assert!(!result.migrated);
    }
}
