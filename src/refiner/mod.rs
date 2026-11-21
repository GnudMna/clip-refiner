pub mod datetime;
pub mod escape;
pub mod json;
pub mod line_actions;
pub mod markdown;
pub mod number;
pub mod trim;
pub mod url;
pub mod yaml;

use arboard::Clipboard;
use clap::ValueEnum;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

/// クリップボードのテキストを加工する各モードの定義
#[derive(Copy, Clone, Debug, ValueEnum, PartialEq, Eq, Serialize, Deserialize)]
pub enum RefineMode {
    /// URLエンコードを行う
    #[value(help = "URLエンコード")]
    UrlEncode,
    /// URLデコードを行う。失敗した場合は元のテキストを維持する
    #[value(help = "URLデコード")]
    UrlDecode,
    /// URLから utm_ で始まる計測用パラメータを削除する
    #[value(help = "UTMパラメータを削除")]
    RemoveUtm,
    /// 行単位でアルファベット順（ケース不問）に並び替える。CSVの場合は各行をレコードとして認識してソートする
    #[value(help = "並び替え")]
    SortLines,
    /// 空行を削除する
    #[value(help = "空行を削除")]
    RemoveEmptyLines,
    /// 重複行を削除する
    #[value(help = "重複行を削除")]
    RemoveDuplicateLines,
    /// テキスト全体の前後にある空白および改行を削除する
    #[value(help = "改行や空白を整形")]
    Trim,
    /// 行ごとに前後の空白を削除する
    #[value(help = "行単位で改行や空白を整形")]
    TrimLines,
    /// 文字列をバックスラッシュでエスケープする
    #[value(help = "文字列をエスケープ")]
    Escape,
    /// 文字列のエスケープを解除する
    #[value(help = "文字列のアンエスケープ")]
    Unescape,
    /// 正規表現のメタ文字をエスケープする
    #[value(help = "正規表現のエスケープ")]
    RegexEscape,
    /// 正規表現のエスケープを解除する
    #[value(help = "正規表現のアンエスケープ")]
    RegexUnescape,
    /// JSON形式をインデント整形する（キーの順序はパース時に不定となる）
    #[value(help = "JSON形式を整形(キー順序不同)")]
    JsonFormat,
    /// JSON形式をインデント整形する（元のキー順序を維持する）
    #[value(help = "JSON形式を整形(キー順序保持)")]
    JsonFormatPreserveOrder,
    /// YAML形式をJSON形式へ変換する
    #[value(help = "YAML形式をJSON形式へ変換(キー順序不同)")]
    YamlToJson,
    /// YAML形式をJSON形式へ変換する（元のキー順序を維持する）
    #[value(help = "YAML形式をJSON形式へ変換(キー順序保持)")]
    YamlToJsonPreserveOrder,
    /// JSON形式をYAML形式へ変換する
    #[value(help = "JSON形式をYAML形式へ変換(キー順序不同)")]
    JsonToYaml,
    /// JSON形式をYAML形式へ変換する（元のキー順序を維持する）
    #[value(help = "JSON形式をYAML形式へ変換(キー順序保持)")]
    JsonToYamlPreserveOrder,
    /// Markdown形式のテキストをHTML形式へ変換する
    #[value(help = "MarkdownをHTML形式へ変換")]
    MarkdownToHtml,
    /// ExcelでコピーしたTSV形式のテキストをMarkdown形式へ変換する
    #[value(help = "Excel(TSV)をMarkdown形式へ変換")]
    ExcelToMarkdown,
    /// Unixタイムスタンプを日時文字列へ変換する
    #[value(help = "Unixタイムスタンプを日時文字列へ変換")]
    TimestampToDatetime,
    /// 日時文字列をUnixタイムスタンプへ変換する
    #[value(help = "日時文字列をUnixタイムスタンプへ変換")]
    DatetimeToTimestamp,
    /// 数値に対して3桁ごとのカンマを付与する（例: 1000 -> 1,000）
    #[value(help = "カンマ無し数値をカンマ区切りの数値に")]
    AddComma,
    /// 数値からカンマを削除する（例: 1,000 -> 1000）
    #[value(help = "カンマ区切りの数値をカンマ無し数値に")]
    RemoveComma,
}

/// メニューの階層化に使用するカテゴリ
#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub enum RefineCategory {
    /// 通常の単独メニュー
    Normal,
    /// URL操作サブメニュー内
    UrlActions,
    /// 行操作サブメニュー内
    LineActions,
    /// トリムサブメニュー内
    Trim,
    /// エスケープサブメニュー内
    Escape,
    /// JSON整形サブメニュー内
    JsonFormat,
    /// JSON to YAMLサブメニュー内
    ToYaml,
    /// YAML to JSONサブメニュー内
    ToJson,
    /// 日時変換サブメニュー内
    Datetime,
    /// 数値変換サブメニュー内
    Number,
}

impl RefineCategory {
    /// カテゴリの表示名を取得する
    pub fn label(&self) -> &'static str {
        match self {
            RefineCategory::Normal => "",
            RefineCategory::UrlActions => "URL操作",
            RefineCategory::LineActions => "行操作",
            RefineCategory::Trim => "トリム",
            RefineCategory::Escape => "エスケープ",
            RefineCategory::JsonFormat => "JSON整形",
            RefineCategory::ToJson => "JSONへ変換",
            RefineCategory::ToYaml => "YAMLへ変換",
            RefineCategory::Datetime => "日時変換",
            RefineCategory::Number => "数値変換",
        }
    }
}

impl RefineMode {
    /// UIに表示する名前を取得する
    ///
    /// # Returns
    /// * `&'static str` - モードに対応する静的な文字列ラベル。
    pub fn label(&self) -> &'static str {
        match self {
            RefineMode::UrlEncode => "URLエンコード",
            RefineMode::UrlDecode => "URLデコード",
            RefineMode::RemoveUtm => "UTM除去",
            RefineMode::SortLines => "並び替え",
            RefineMode::RemoveEmptyLines => "空行削除",
            RefineMode::RemoveDuplicateLines => "重複行削除",
            RefineMode::Trim => "全体",
            RefineMode::TrimLines => "行単位",
            RefineMode::Escape => "エスケープ",
            RefineMode::Unescape => "アンエスケープ",
            RefineMode::RegexEscape => "正規表現エスケープ",
            RefineMode::RegexUnescape => "正規表現アンエスケープ",
            RefineMode::JsonFormat => "JSON整形(キー順序不同)",
            RefineMode::JsonFormatPreserveOrder => "JSON整形(キー順序保持)",
            RefineMode::YamlToJson => "YAML→JSON(キー順序不同)",
            RefineMode::YamlToJsonPreserveOrder => "YAML→JSON(キー順序保持)",
            RefineMode::JsonToYaml => "JSON→YAML(キー順序不同)",
            RefineMode::JsonToYamlPreserveOrder => "JSON→YAML(キー順序保持)",
            RefineMode::MarkdownToHtml => "Markdown→HTML",
            RefineMode::ExcelToMarkdown => "Excel→Markdown",
            RefineMode::TimestampToDatetime => "Unixタイムスタンプ→日時文字列",
            RefineMode::DatetimeToTimestamp => "日時文字列→Unixタイムスタンプ",
            RefineMode::AddComma => "カンマ追加",
            RefineMode::RemoveComma => "カンマ除去",
        }
    }

    /// 所属するカテゴリを取得する。トレイメニューの階層構築に利用される
    ///
    /// # Returns
    /// * `RefineCategory` - モードが属するカテゴリ。
    pub fn category(&self) -> RefineCategory {
        match self {
            RefineMode::UrlEncode | RefineMode::UrlDecode | RefineMode::RemoveUtm => {
                RefineCategory::UrlActions
            }
            RefineMode::SortLines
            | RefineMode::RemoveEmptyLines
            | RefineMode::RemoveDuplicateLines => RefineCategory::LineActions,
            RefineMode::Trim | RefineMode::TrimLines => RefineCategory::Trim,
            RefineMode::Escape
            | RefineMode::Unescape
            | RefineMode::RegexEscape
            | RefineMode::RegexUnescape => RefineCategory::Escape,
            RefineMode::JsonFormat | RefineMode::JsonFormatPreserveOrder => {
                RefineCategory::JsonFormat
            }
            RefineMode::YamlToJson | RefineMode::YamlToJsonPreserveOrder => RefineCategory::ToJson,
            RefineMode::JsonToYaml | RefineMode::JsonToYamlPreserveOrder => RefineCategory::ToYaml,
            RefineMode::TimestampToDatetime | RefineMode::DatetimeToTimestamp => {
                RefineCategory::Datetime
            }
            RefineMode::AddComma | RefineMode::RemoveComma => RefineCategory::Number,
            _ => RefineCategory::Normal,
        }
    }

    /// 定義されているすべてのモードを順番に取得する
    ///
    /// # Returns
    /// * `&'static [RefineMode]` - 全ての `RefineMode` バリアントを含む静的スライス。
    pub fn variants() -> &'static [RefineMode] {
        &[
            RefineMode::UrlEncode,
            RefineMode::UrlDecode,
            RefineMode::RemoveUtm,
            RefineMode::SortLines,
            RefineMode::RemoveEmptyLines,
            RefineMode::RemoveDuplicateLines,
            RefineMode::Trim,
            RefineMode::TrimLines,
            RefineMode::Escape,
            RefineMode::Unescape,
            RefineMode::RegexEscape,
            RefineMode::RegexUnescape,
            RefineMode::JsonFormat,
            RefineMode::JsonFormatPreserveOrder,
            RefineMode::YamlToJson,
            RefineMode::YamlToJsonPreserveOrder,
            RefineMode::JsonToYaml,
            RefineMode::JsonToYamlPreserveOrder,
            RefineMode::MarkdownToHtml,
            RefineMode::ExcelToMarkdown,
            RefineMode::TimestampToDatetime,
            RefineMode::DatetimeToTimestamp,
            RefineMode::AddComma,
            RefineMode::RemoveComma,
        ]
    }
}

/// JSON, YAMLキー順序保持用
#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrderedValue {
    Null,
    Bool(bool),
    Number(serde_json::Number),
    String(String),
    Array(Vec<OrderedValue>),
    Object(IndexMap<String, OrderedValue>),
}

/// クリップボードの内容を変換
///
/// # Arguments
/// * `clipboard` - `arboard::Clipboard` のミュータブルなインスタンス。
/// * `mode` - 適用する `RefineMode`。
///
/// # Returns
/// * `Option<String>` - テキストが加工された場合は `Some(加工後テキスト)` を、加工されなかった場合（変更なし、またはエラー）は `None` を返す。
pub fn process_clipboard(clipboard: &mut Clipboard, mode: RefineMode) -> Option<String> {
    let text = clipboard.get_text().ok()?;
    if text.is_empty() {
        return None;
    }

    let processed = match mode {
        RefineMode::UrlEncode => url::url_encode(&text),
        RefineMode::UrlDecode => url::url_decode(&text).unwrap_or_else(|_| text.clone()),
        RefineMode::RemoveUtm => url::remove_utm_params(&text),
        RefineMode::SortLines => line_actions::sort_lines(&text),
        RefineMode::RemoveEmptyLines => line_actions::remove_empty_lines(&text),
        RefineMode::RemoveDuplicateLines => line_actions::remove_duplicate_lines(&text),
        RefineMode::Trim => trim::trim_text(&text),
        RefineMode::TrimLines => trim::trim_lines(&text),
        RefineMode::Escape => escape::escape_string(&text),
        RefineMode::Unescape => escape::unescape_string(&text),
        RefineMode::RegexEscape => escape::regex_escape(&text),
        RefineMode::RegexUnescape => escape::regex_unescape(&text),
        RefineMode::JsonFormat => json::format_json(&text),
        RefineMode::JsonFormatPreserveOrder => json::format_json_preserve_order(&text),
        RefineMode::YamlToJson => yaml::yaml_to_json(&text),
        RefineMode::YamlToJsonPreserveOrder => yaml::yaml_to_json_preserve_order(&text),
        RefineMode::JsonToYaml => json::json_to_yaml(&text),
        RefineMode::JsonToYamlPreserveOrder => json::json_to_yaml_preserve_order(&text),
        RefineMode::MarkdownToHtml => markdown::markdown_to_html(&text),
        RefineMode::ExcelToMarkdown => markdown::excel_to_markdown_table(&text),
        RefineMode::TimestampToDatetime => datetime::timestamp_to_datetime_string(&text),
        RefineMode::DatetimeToTimestamp => datetime::datetime_string_to_timestamp(&text),
        RefineMode::AddComma => number::add_commas(&text),
        RefineMode::RemoveComma => number::remove_commas(&text),
    };

    if processed != text {
        let _ = clipboard.set_text(processed.clone());
        Some(processed)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arboard::Clipboard;

    #[test]
    fn test_refine_mode_metadata() {
        assert_eq!(RefineMode::UrlEncode.label(), "URLエンコード");
        assert_eq!(RefineMode::UrlEncode.category(), RefineCategory::UrlActions);

        assert_eq!(RefineMode::JsonFormat.label(), "キー順序不同");
        assert_eq!(
            RefineMode::JsonFormat.category(),
            RefineCategory::JsonFormat
        );

        assert_eq!(
            RefineMode::TimestampToDatetime.label(),
            "Unixタイムスタンプ→日時文字列"
        );

        assert_eq!(
            RefineMode::TimestampToDatetime.category(),
            RefineCategory::Datetime
        );
    }

    #[test]
    fn test_refine_mode_variants() {
        let variants = RefineMode::variants();
        assert!(variants.contains(&RefineMode::UrlEncode));
        assert!(variants.contains(&RefineMode::SortLines));
        assert!(variants.contains(&RefineMode::TimestampToDatetime));
        assert_eq!(variants.len(), 24);
    }

    #[test]
    fn test_process_clipboard_integration() {
        // 並列実行による干渉を避けるため、1つのテストケースにまとめる
        if let Ok(mut cb) = Clipboard::new() {
            // Case 1: 変化あり
            let unique_str_1 = "  clip_refiner_test_1  ";
            let _ = cb.set_text(unique_str_1.to_string());
            // システムのクリップボードへの反映を待つ必要がある場合があるが、まずはそのまま
            if let Ok(current) = cb.get_text() {
                if current == unique_str_1 {
                    let result = process_clipboard(&mut cb, RefineMode::Trim);
                    assert_eq!(result, Some("clip_refiner_test_1".to_string()));
                } else {
                    eprintln!(
                        "Clipboard content mismatch for unique_str_1. Expected: '{}', Got: '{}'",
                        unique_str_1, current
                    );
                }
            } else {
                eprintln!("Failed to get clipboard text for unique_str_1.");
            }

            // Case 2: 変化なし
            let unique_str_2 = "clip_refiner_test_2";
            let _ = cb.set_text(unique_str_2.to_string());
            if let Ok(current) = cb.get_text() {
                if current == unique_str_2 {
                    let result = process_clipboard(&mut cb, RefineMode::Trim);
                    assert!(result.is_none());
                } else {
                    eprintln!(
                        "Clipboard content mismatch for unique_str_2. Expected: '{}', Got: '{}'",
                        unique_str_2, current
                    );
                }
            } else {
                eprintln!("Failed to get clipboard text for unique_str_2.");
            }
        } else {
            eprintln!("Failed to initialize clipboard. Skipping clipboard integration tests.");
        }
    }
}
