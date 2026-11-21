mod docs;
mod document;
mod format;
mod scalar;
mod sections;
mod template;

use crate::config::types::AppConfig;

use anyhow::Result;
use toml_edit::DocumentMut;

// ======================================================================
// コメント付き TOML 出力
// ======================================================================
/// 既存の TOML を維持しながら `AppConfig` を書き出す
///
/// 既存ファイルがない、または解析できない場合は説明コメント付きの新規フォーマットを生成する
pub fn config_to_toml(config: &AppConfig, existing: Option<&str>) -> Result<String> {
    if let Some(content) = existing
        && let Ok(mut doc) = content.parse::<DocumentMut>()
    {
        apply_config_to_document(&mut doc, config)?;
        return Ok(doc.to_string());
    }
    template::to_commented_toml(config)
}

/// 既存ドキュメントの値のみを `AppConfig` の内容で更新する (コメント・レイアウトは維持)
fn apply_config_to_document(doc: &mut DocumentMut, config: &AppConfig) -> Result<()> {
    sections::root::apply(doc, config)?;
    sections::notification::apply(doc, config)?;
    sections::hotkeys::apply(doc, config)?;
    sections::regex::apply(doc, config)?;
    sections::clips::apply(doc);
    Ok(())
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;

    use crate::config::NotificationSettings;
    use crate::consts;

    /// 新規ファイル向けに説明コメント付き TOML が生成されること
    #[test]
    fn test_config_to_toml_new_file_uses_template() {
        let config = AppConfig::default();
        let content = config_to_toml(&config, None).expect("TOML の生成に失敗");

        assert!(content.contains("# 基本"));
        assert!(content.contains("# 設定スキーマのバージョン"));
    }

    /// 既存ファイルのユーザー追記コメントが保存後も維持されること
    #[test]
    fn test_config_to_toml_preserves_user_comments() {
        let existing = r#"
# 私のメモ: モードは慎重に変更すること
mode = "UrlEncode"

# 通知はオフのまま
[notification_settings]
enabled = false
"#;
        let config = AppConfig {
            mode: crate::refiner::RefineMode::JsonFormat,
            notification_settings: NotificationSettings {
                enabled: true,
                ..NotificationSettings::default()
            },
            ..AppConfig::default()
        };

        let content =
            config_to_toml(&config, Some(existing)).expect("コメント維持付き TOML の生成に失敗");

        assert!(
            content.contains("# 私のメモ: モードは慎重に変更すること"),
            "ユーザーコメントが消えている: {content}"
        );
        assert!(
            content.contains("# 通知はオフのまま"),
            "テーブル内ユーザーコメントが消えている: {content}"
        );

        let decoded: AppConfig = toml::from_str(&content).expect("保存後 TOML の解析に失敗");
        assert_eq!(decoded.mode, crate::refiner::RefineMode::JsonFormat);
        assert!(decoded.notification_settings.enabled);
        assert_eq!(decoded.version, config.version);
    }

    /// 不足キーを挿入する際に説明コメントが付与されること
    #[test]
    fn test_config_to_toml_inserts_missing_keys_with_comments() {
        let existing = r#"
mode = "UrlDecode"
interval_ms = 1000
"#;
        let config = AppConfig::default();
        let content =
            config_to_toml(&config, Some(existing)).expect("不足キー挿入付き TOML の生成に失敗");

        assert!(
            content.contains("# 設定スキーマのバージョン"),
            "version の説明コメントがない: {content}"
        );
        assert!(
            content.contains("# 正規表現パターン"),
            "regex.pattern の説明コメントがない: {content}"
        );
        assert!(
            content.contains("# 正規表現加工モード用のパターンと置換文字列"),
            "regex セクション見出しがない: {content}"
        );
        assert!(content.contains("[regex]"));

        let decoded: AppConfig = toml::from_str(&content).expect("保存後 TOML の解析に失敗");
        assert_eq!(decoded.mode, config.mode);
        assert_eq!(decoded.regex, config.regex);
    }

    /// コメント付き TOML に各項目の説明が含まれること
    #[test]
    fn test_commented_toml_contains_field_descriptions() {
        let config = AppConfig::default();
        let content = config_to_toml(&config, None).expect("コメント付き TOML の生成に失敗");

        assert!(content.contains("# 基本"));
        assert!(content.contains("# 設定スキーマのバージョン"));
        assert!(content.contains("[notification_settings]"));
        assert!(content.contains("  enabled = "));
        assert!(content.contains("[regex]"));
        assert!(content.contains("  pattern = "));
    }

    /// コメント付き TOML を読み戻しても設定値が一致すること
    #[test]
    fn test_commented_toml_roundtrip() {
        let mut config = AppConfig::default();
        config.notification_settings.enabled = true;
        config.regex.pattern = r"\d+".to_string();
        config.regex.case_insensitive = true;

        let content = config_to_toml(&config, None).expect("コメント付き TOML の生成に失敗");
        let decoded: AppConfig = toml::from_str(&content).expect("コメント付き TOML の解析に失敗");

        assert_eq!(config.version, decoded.version);
        assert_eq!(config.mode, decoded.mode);
        assert_eq!(config.interval_ms, decoded.interval_ms);
        assert_eq!(
            config.notification_settings.enabled,
            decoded.notification_settings.enabled
        );
        assert_eq!(config.hotkeys, decoded.hotkeys);
        assert_eq!(config.regex, decoded.regex);
        assert_eq!(decoded.version, consts::CONFIG_VERSION);
    }
}
