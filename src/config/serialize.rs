use std::fmt::Write;

use super::types::AppConfig;

use anyhow::Result;
use serde::Serialize;
use toml_edit::{DocumentMut, Item, Table, Value};

const SECTION_RULE: &str =
    "# -----------------------------------------------------------------------------";
const TABLE_INDENT: &str = "  ";

// 各項目の説明コメント (テンプレート生成と新規キー挿入で共有)
const DOC_VERSION: &str = "設定スキーマのバージョン";
const DOC_MODE: &str = "使用する加工モード";
const DOC_INTERVAL_MS: &str = "クリップボードのポーリング間隔 (ミリ秒、100〜60000)";
const DOC_MONITOR_MODE: &str = "監視方式 (\"Polling\" または \"Event\")";
const DOC_IS_PAUSED: &str = "監視を一時停止するかどうか";
const DOC_HISTORY_ENABLED: &str = "加工履歴の有効・無効";
const DOC_HISTORY_LIMIT: &str = "履歴の最大保持件数 (1〜100)";
const DOC_NS_ENABLED: &str = "デスクトップ通知の有効・無効";
const DOC_NS_NOTIFY_MODE: &str = "モード変更時の通知";
const DOC_NS_NOTIFY_RESULT: &str = "通知にクリップボードの内容を表示するかどうか";
const DOC_NS_NOTIFY_PAUSE: &str = "一時停止切替時の通知";
const DOC_HOTKEY_SELECTOR: &str = "クイックセレクタ表示";
const DOC_HOTKEY_NOTIFICATION: &str = "成功通知の ON/OFF";
const DOC_HOTKEY_PAUSE: &str = "監視の一時停止・再開";
const DOC_HOTKEY_UNDO: &str = "直近の加工を取り消し";
const DOC_HOTKEY_TEXT_SELECTOR: &str = "登録文字列セレクター表示";
const DOC_HOTKEY_QUIT: &str = "アプリケーション終了";
const DOC_REGEX_PATTERN: &str = "正規表現パターン";
const DOC_REGEX_REPLACEMENT: &str =
    "置換文字列 (RegexReplace で使用。キャプチャグループは $1 形式)";
const DOC_REGEX_CASE_INSENSITIVE: &str = "大文字小文字を無視する ((?i) 相当)";
const DOC_REGEX_MULTILINE: &str = "複数行モード ((?m) 相当)";

const SECTION_BASIC: &str = "基本";
const SECTION_MONITOR: &str = "監視";
const SECTION_HISTORY: &str = "履歴";
const SECTION_NOTIFICATION: &str = "通知";
const SECTION_HOTKEYS: &str =
    "グローバルホットキー (\"Alt+Shift+S\" 形式。変更反映には再起動が必要)";
const SECTION_REGEX: &str = "正規表現加工モード用のパターンと置換文字列";
const SECTION_TEXTS: &str = "クリップボードへコピーする登録文字列 (`[[texts]]` 形式)";
const DOC_TEXT_LABEL: &str = "一覧表示用のラベル";
const DOC_TEXT_BODY: &str = "クリップボードへコピーする本文";

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
        apply_config_to_document(&mut doc, config);
        return Ok(doc.to_string());
    }
    to_commented_toml(config)
}

/// 既存ドキュメントの値のみを `AppConfig` の内容で更新する (コメント・レイアウトは維持)
fn apply_config_to_document(doc: &mut DocumentMut, config: &AppConfig) {
    apply_root_fields(doc, config);
    apply_notification_table(doc, config);
    apply_hotkeys_table(doc, config);
    apply_regex_table(doc, config);
    apply_texts_tables(doc, config);
}

/// ルートレベルの設定値を更新する
fn apply_root_fields(doc: &mut DocumentMut, config: &AppConfig) {
    let root = doc.as_table_mut();
    set_table_value(root, "version", DOC_VERSION, "", &config.version);
    set_table_value(root, "mode", DOC_MODE, "", &config.mode);
    set_table_value(
        root,
        "interval_ms",
        DOC_INTERVAL_MS,
        "",
        &config.interval_ms,
    );
    set_table_value(
        root,
        "monitor_mode",
        DOC_MONITOR_MODE,
        "",
        &config.monitor_mode,
    );
    set_table_value(
        root,
        "history_enabled",
        DOC_HISTORY_ENABLED,
        "",
        &config.history_enabled,
    );
    set_table_value(
        root,
        "history_limit",
        DOC_HISTORY_LIMIT,
        "",
        &config.history_limit,
    );
    set_table_value(root, "is_paused", DOC_IS_PAUSED, "", &config.is_paused);
}

/// `[notification_settings]` の設定値を更新する
fn apply_notification_table(doc: &mut DocumentMut, config: &AppConfig) {
    ensure_table(doc, "notification_settings", SECTION_NOTIFICATION);
    let notification = doc["notification_settings"]
        .as_table_mut()
        .expect("notification_settings テーブル");
    set_table_value(
        notification,
        "enabled",
        DOC_NS_ENABLED,
        TABLE_INDENT,
        &config.notification_settings.enabled,
    );
    set_table_value(
        notification,
        "notify_mode",
        DOC_NS_NOTIFY_MODE,
        TABLE_INDENT,
        &config.notification_settings.notify_mode,
    );
    set_table_value(
        notification,
        "notify_result",
        DOC_NS_NOTIFY_RESULT,
        TABLE_INDENT,
        &config.notification_settings.notify_result,
    );
    set_table_value(
        notification,
        "notify_pause",
        DOC_NS_NOTIFY_PAUSE,
        TABLE_INDENT,
        &config.notification_settings.notify_pause,
    );
}

/// `[hotkeys]` の設定値を更新する
fn apply_hotkeys_table(doc: &mut DocumentMut, config: &AppConfig) {
    ensure_table(doc, "hotkeys", SECTION_HOTKEYS);
    let hotkeys = doc["hotkeys"].as_table_mut().expect("hotkeys テーブル");
    set_table_value(
        hotkeys,
        "quick_selector",
        DOC_HOTKEY_SELECTOR,
        TABLE_INDENT,
        &config.hotkeys.quick_selector,
    );
    hotkeys.remove("selector");
    set_table_value(
        hotkeys,
        "notification",
        DOC_HOTKEY_NOTIFICATION,
        TABLE_INDENT,
        &config.hotkeys.notification,
    );
    set_table_value(
        hotkeys,
        "pause",
        DOC_HOTKEY_PAUSE,
        TABLE_INDENT,
        &config.hotkeys.pause,
    );
    set_table_value(
        hotkeys,
        "undo",
        DOC_HOTKEY_UNDO,
        TABLE_INDENT,
        &config.hotkeys.undo,
    );
    set_table_value(
        hotkeys,
        "text_selector",
        DOC_HOTKEY_TEXT_SELECTOR,
        TABLE_INDENT,
        &config.hotkeys.text_selector,
    );
    set_table_value(
        hotkeys,
        "quit",
        DOC_HOTKEY_QUIT,
        TABLE_INDENT,
        &config.hotkeys.quit,
    );
}

/// `[regex]` の設定値を更新する
fn apply_regex_table(doc: &mut DocumentMut, config: &AppConfig) {
    ensure_table(doc, "regex", SECTION_REGEX);
    let regex = doc["regex"].as_table_mut().expect("regex テーブル");
    set_table_value(
        regex,
        "pattern",
        DOC_REGEX_PATTERN,
        TABLE_INDENT,
        &config.regex.pattern,
    );
    set_table_value(
        regex,
        "replacement",
        DOC_REGEX_REPLACEMENT,
        TABLE_INDENT,
        &config.regex.replacement,
    );
    set_table_value(
        regex,
        "case_insensitive",
        DOC_REGEX_CASE_INSENSITIVE,
        TABLE_INDENT,
        &config.regex.case_insensitive,
    );
    set_table_value(
        regex,
        "multiline",
        DOC_REGEX_MULTILINE,
        TABLE_INDENT,
        &config.regex.multiline,
    );
}

/// `[[texts]]` の配列を更新する
fn apply_texts_tables(doc: &mut DocumentMut, config: &AppConfig) {
    use toml_edit::ArrayOfTables;

    if config.texts.is_empty() {
        doc.as_table_mut().remove("texts");
        return;
    }

    let mut array = ArrayOfTables::new();
    for entry in &config.texts {
        let mut table = Table::new();
        table.insert("label", Item::Value(serde_to_toml_value(&entry.label)));
        table.insert("text", Item::Value(serde_to_toml_value(&entry.text)));
        array.push(table);
    }

    doc["texts"] = Item::ArrayOfTables(array);
}

/// テーブルがなければセクション見出し付きで挿入する
fn ensure_table(doc: &mut DocumentMut, name: &str, section_title: &str) {
    if doc.get(name).and_then(Item::as_table).is_some() {
        return;
    }
    let mut table = Table::new();
    table
        .decor_mut()
        .set_prefix(section_header_prefix(section_title));
    doc.insert(name, Item::Table(table));
}

/// テーブル内の値を更新する (既存キーはコメント付きのまま値だけ差し替え)
fn set_table_value<T: Serialize>(
    table: &mut Table,
    key: &str,
    comment: &str,
    indent: &str,
    value: &T,
) {
    let new_value = serde_to_toml_value(value);
    if let Some(item) = table.get_mut(key)
        && let Some(existing) = item.as_value_mut()
    {
        *existing = new_value;
        return;
    }
    table.insert(key, Item::Value(new_value));
    if let Some(mut key_mut) = table.key_mut(key) {
        key_mut
            .leaf_decor_mut()
            .set_prefix(field_comment_prefix(indent, comment));
    }
}

/// セクション見出しの decor 用プレフィックスを返す
fn section_header_prefix(title: &str) -> String {
    format!("\n{SECTION_RULE}\n# {title}\n{SECTION_RULE}\n\n")
}

/// 項目説明コメントの decor 用プレフィックスを返す
fn field_comment_prefix(indent: &str, comment: &str) -> String {
    format!("{indent}# {comment}\n")
}

/// Serde 値を `toml_edit::Value` へ変換する
fn serde_to_toml_value<T: Serialize>(value: &T) -> Value {
    let line = format!("v={}", toml_scalar(value));
    let doc: DocumentMut = line.parse().expect("TOML スカラー行のパースに失敗");
    doc["v"].as_value().expect("スカラー値").clone()
}

/// `AppConfig` を各項目の説明コメント付き TOML 文字列へ変換する
#[allow(clippy::too_many_lines)]
fn to_commented_toml(config: &AppConfig) -> Result<String> {
    let mut out = String::new();

    writeln!(out, "# ClipRefiner 設定ファイル")?;

    write_section(&mut out, SECTION_BASIC)?;
    write_field(&mut out, "", DOC_VERSION, "version", &config.version)?;
    write_field(&mut out, "", DOC_MODE, "mode", &config.mode)?;

    write_section(&mut out, SECTION_MONITOR)?;
    write_field(
        &mut out,
        "",
        DOC_INTERVAL_MS,
        "interval_ms",
        &config.interval_ms,
    )?;
    write_field(
        &mut out,
        "",
        DOC_MONITOR_MODE,
        "monitor_mode",
        &config.monitor_mode,
    )?;
    write_field(&mut out, "", DOC_IS_PAUSED, "is_paused", &config.is_paused)?;

    write_section(&mut out, SECTION_HISTORY)?;
    write_field(
        &mut out,
        "",
        DOC_HISTORY_ENABLED,
        "history_enabled",
        &config.history_enabled,
    )?;
    write_field(
        &mut out,
        "",
        DOC_HISTORY_LIMIT,
        "history_limit",
        &config.history_limit,
    )?;

    write_table_section(&mut out, SECTION_NOTIFICATION, "notification_settings")?;
    write_field(
        &mut out,
        TABLE_INDENT,
        DOC_NS_ENABLED,
        "enabled",
        &config.notification_settings.enabled,
    )?;
    write_field(
        &mut out,
        TABLE_INDENT,
        DOC_NS_NOTIFY_MODE,
        "notify_mode",
        &config.notification_settings.notify_mode,
    )?;
    write_field(
        &mut out,
        TABLE_INDENT,
        DOC_NS_NOTIFY_RESULT,
        "notify_result",
        &config.notification_settings.notify_result,
    )?;
    write_field(
        &mut out,
        TABLE_INDENT,
        DOC_NS_NOTIFY_PAUSE,
        "notify_pause",
        &config.notification_settings.notify_pause,
    )?;

    write_table_section(&mut out, SECTION_HOTKEYS, "hotkeys")?;
    write_field(
        &mut out,
        TABLE_INDENT,
        DOC_HOTKEY_SELECTOR,
        "quick_selector",
        &config.hotkeys.quick_selector,
    )?;
    write_field(
        &mut out,
        TABLE_INDENT,
        DOC_HOTKEY_NOTIFICATION,
        "notification",
        &config.hotkeys.notification,
    )?;
    write_field(
        &mut out,
        TABLE_INDENT,
        DOC_HOTKEY_PAUSE,
        "pause",
        &config.hotkeys.pause,
    )?;
    write_field(
        &mut out,
        TABLE_INDENT,
        DOC_HOTKEY_UNDO,
        "undo",
        &config.hotkeys.undo,
    )?;
    write_field(
        &mut out,
        TABLE_INDENT,
        DOC_HOTKEY_TEXT_SELECTOR,
        "text_selector",
        &config.hotkeys.text_selector,
    )?;
    write_field(
        &mut out,
        TABLE_INDENT,
        DOC_HOTKEY_QUIT,
        "quit",
        &config.hotkeys.quit,
    )?;

    write_table_section(&mut out, SECTION_REGEX, "regex")?;
    write_field(
        &mut out,
        TABLE_INDENT,
        DOC_REGEX_PATTERN,
        "pattern",
        &config.regex.pattern,
    )?;
    write_field(
        &mut out,
        TABLE_INDENT,
        DOC_REGEX_REPLACEMENT,
        "replacement",
        &config.regex.replacement,
    )?;
    write_field(
        &mut out,
        TABLE_INDENT,
        DOC_REGEX_CASE_INSENSITIVE,
        "case_insensitive",
        &config.regex.case_insensitive,
    )?;
    write_field(
        &mut out,
        TABLE_INDENT,
        DOC_REGEX_MULTILINE,
        "multiline",
        &config.regex.multiline,
    )?;

    if !config.texts.is_empty() {
        write_section(&mut out, SECTION_TEXTS)?;
        for entry in &config.texts {
            writeln!(out, "[[texts]]")?;
            writeln!(out)?;
            write_field(&mut out, "", DOC_TEXT_LABEL, "label", &entry.label)?;
            write_field(&mut out, "", DOC_TEXT_BODY, "text", &entry.text)?;
        }
    }

    Ok(out)
}

/// ルートレベルのセクション見出しを書き出す
fn write_section<W: Write>(out: &mut W, title: &str) -> Result<()> {
    writeln!(out)?;
    writeln!(out, "{SECTION_RULE}")?;
    writeln!(out, "# {title}")?;
    writeln!(out, "{SECTION_RULE}")?;
    writeln!(out)?;
    Ok(())
}

/// テーブルセクションの見出しと `[name]` 行を書き出す
fn write_table_section<W: Write>(out: &mut W, title: &str, table_name: &str) -> Result<()> {
    write_section(out, title)?;
    writeln!(out, "[{table_name}]")?;
    writeln!(out)?;
    Ok(())
}

/// キーと値をコメント付きで書き出す
fn write_field<W, T>(out: &mut W, indent: &str, comment: &str, key: &str, value: &T) -> Result<()>
where
    W: Write,
    T: Serialize,
{
    writeln!(out, "{indent}# {comment}")?;
    writeln!(out, "{indent}{key} = {}", toml_scalar(value))?;
    writeln!(out)?;
    Ok(())
}

/// TOML のスカラー値をエスケープ済み文字列として返す
fn toml_scalar<T: Serialize>(value: &T) -> String {
    #[derive(Serialize)]
    struct Row<'a, T: Serialize> {
        v: &'a T,
    }

    let line = toml::to_string(&Row { v: value }).expect("TOML スカラーのエンコードに失敗");
    line.split_once('=')
        .expect("TOML スカラー行の解析に失敗")
        .1
        .trim()
        .to_string()
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use crate::config::NotificationSettings;
    use crate::consts;

    use super::*;

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
