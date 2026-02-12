pub mod datetime;
pub mod escape;
pub mod json;
pub mod line_actions;
pub mod markdown;
pub mod number;
pub mod path;
pub mod trim;
pub mod url;
pub mod utils;
pub mod yaml;

use arboard::Clipboard;
use clap::ValueEnum;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

/// メニューの階層化に使用するカテゴリ
#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub enum RefineCategory {
    /// 通常の単独メニュー
    Normal,
    /// URL操作サブメニュー内
    UrlActions,
    /// パス操作サブメニュー内
    Path,
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
            RefineCategory::Path => "パス操作",
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

/// クリップボードのテキストを加工するトレイト
pub trait Refiner {
    /// テキストを加工する
    fn refine(&self, text: &str) -> String;
}

macro_rules! define_refine_modes {
    (
        $(
            $(#[doc = $doc:expr])*
            $variant:ident => {
                label: $label:expr,
                category: $category:expr,
                refine: |$text:ident| $body:expr
            }
        ),* $(,)?
    ) => {
        /// クリップボードのテキストを加工する各モードの定義
        #[derive(Copy, Clone, Debug, ValueEnum, PartialEq, Eq, Serialize, Deserialize)]
        pub enum RefineMode {
            $(
                $(#[doc = $doc])*
                #[value(help = $label)]
                $variant,
            )*
        }

        impl RefineMode {
            /// UIに表示する名前を取得する
            pub fn label(&self) -> &'static str {
                match self {
                    $(Self::$variant => $label,)*
                }
            }

            /// 所属するカテゴリを取得する
            pub fn category(&self) -> RefineCategory {
                match self {
                    $(Self::$variant => $category,)*
                }
            }

            /// 定義されているすべてのモードを順番に取得する
            pub fn variants() -> &'static [RefineMode] {
                &[
                    $(Self::$variant,)*
                ]
            }

            /// UI（Webview）に渡すためのモード情報のJSONリストを生成する
            pub fn to_json_list() -> String {
                let list: Vec<serde_json::Value> = Self::variants()
                    .iter()
                    .map(|m| {
                        serde_json::json!({
                            "id": m,
                            "label": m.label(),
                            "category": m.category().label(),
                        })
                    })
                    .collect();
                serde_json::to_string(&list).unwrap_or_else(|_| "[]".to_string())
            }
        }

        impl Refiner for RefineMode {
            fn refine(&self, text: &str) -> String {
                match self {
                    $(Self::$variant => {
                        let $text = text;
                        $body
                    },)*
                }
            }
        }
    };
}

define_refine_modes! {
    /// URLエンコードを行う
    UrlEncode => {
        label: "URLエンコード",
        category: RefineCategory::UrlActions,
        refine: |text| url::url_encode(text)
    },
    /// URLデコードを行う。失敗した場合は元のテキストを維持する
    UrlDecode => {
        label: "URLデコード",
        category: RefineCategory::UrlActions,
        refine: |text| url::url_decode(text).unwrap_or_else(|_| text.to_string())
    },
    /// URLから utm_ で始まる計測用パラメータを削除する
    RemoveUtm => {
        label: "UTM除去",
        category: RefineCategory::UrlActions,
        refine: |text| url::remove_utm_params(text)
    },
    /// パスからベースネームを抽出する
    ExtractBasename => {
        label: "ベースネーム抽出",
        category: RefineCategory::Path,
        refine: |text| path::extract_basename(text).unwrap_or_else(|| text.to_string())
    },
    /// パスからベースネームを抽出しダブルクォーテーションで囲む
    ExtractBasenameQuoted => {
        label: "ベースネーム抽出(引用符付)",
        category: RefineCategory::Path,
        refine: |text| path::extract_basename_quoted(text).unwrap_or_else(|| text.to_string())
    },
    /// パスの前後にダブルクォーテーションを付与する
    AddPathQuotes => {
        label: "引用符を付与",
        category: RefineCategory::Path,
        refine: |text| path::add_path_quotes(text).unwrap_or_else(|| text.to_string())
    },
    /// パスの前後にあるダブルクォーテーションを削除する
    RemovePathQuotes => {
        label: "引用符を削除",
        category: RefineCategory::Path,
        refine: |text| path::remove_path_quotes(text).unwrap_or_else(|| text.to_string())
    },
    /// パスのバックスラッシュをスラッシュに変換する
    PathToSlash => {
        label: "スラッシュ区切りに変換",
        category: RefineCategory::Path,
        refine: |text| path::convert_to_forward_slash(text).unwrap_or_else(|| text.to_string())
    },
    /// パスのスラッシュをバックスラッシュに変換する
    PathToBackslash => {
        label: "バックスラッシュ区切りに変換",
        category: RefineCategory::Path,
        refine: |text| path::convert_to_backslash(text).unwrap_or_else(|| text.to_string())
    },
    /// 行単位で昇順に並び替える。CSVの場合は各行をレコードとして認識してソートする
    SortLinesAsc => {
        label: "昇順で並び替え",
        category: RefineCategory::LineActions,
        refine: |text| line_actions::sort_lines(text, false)
    },
    /// 行単位で降順に並び替える。CSVの場合は各行をレコードとして認識してソートする
    SortLinesDesc => {
        label: "降順で並び替え",
        category: RefineCategory::LineActions,
        refine: |text| line_actions::sort_lines(text, true)
    },
    /// 空行を削除する
    RemoveEmptyLines => {
        label: "空行削除",
        category: RefineCategory::LineActions,
        refine: |text| line_actions::remove_empty_lines(text)
    },
    /// 重複行を削除する
    RemoveDuplicateLines => {
        label: "重複行削除",
        category: RefineCategory::LineActions,
        refine: |text| line_actions::remove_duplicate_lines(text)
    },
    /// テキスト全体の前後にある空白および改行を削除する
    Trim => {
        label: "全体をトリム",
        category: RefineCategory::Trim,
        refine: |text| trim::trim_text(text)
    },
    /// 行ごとに前後の空白を削除する
    TrimLines => {
        label: "行単位でトリム",
        category: RefineCategory::Trim,
        refine: |text| trim::trim_lines(text)
    },
    /// 文字列をバックスラッシュでエスケープする
    Escape => {
        label: "エスケープ",
        category: RefineCategory::Escape,
        refine: |text| escape::escape_string(text)
    },
    /// 文字列のエスケープを解除する
    Unescape => {
        label: "アンエスケープ",
        category: RefineCategory::Escape,
        refine: |text| escape::unescape_string(text)
    },
    /// 正規表現のメタ文字をエスケープする
    RegexEscape => {
        label: "正規表現エスケープ",
        category: RefineCategory::Escape,
        refine: |text| escape::regex_escape(text)
    },
    /// 正規表現のエスケープを解除する
    RegexUnescape => {
        label: "正規表現アンエスケープ",
        category: RefineCategory::Escape,
        refine: |text| escape::regex_unescape(text)
    },
    /// JSON形式をインデント整形する（キーの順序はパース時に不定となる）
    JsonFormat => {
        label: "JSON整形(キー順序不同)",
        category: RefineCategory::JsonFormat,
        refine: |text| json::format_json(text)
    },
    /// JSON形式をインデント整形する（元のキー順序を維持する）
    JsonFormatPreserveOrder => {
        label: "JSON整形(キー順序保持)",
        category: RefineCategory::JsonFormat,
        refine: |text| json::format_json_preserve_order(text)
    },
    /// YAML形式をJSON形式へ変換する
    YamlToJson => {
        label: "YAML→JSON(キー順序不同)",
        category: RefineCategory::ToJson,
        refine: |text| yaml::yaml_to_json(text)
    },
    /// YAML形式をJSON形式へ変換する（元のキー順序を維持する）
    YamlToJsonPreserveOrder => {
        label: "YAML→JSON(キー順序保持)",
        category: RefineCategory::ToJson,
        refine: |text| yaml::yaml_to_json_preserve_order(text)
    },
    /// JSON形式をYAML形式へ変換する
    JsonToYaml => {
        label: "JSON→YAML(キー順序不同)",
        category: RefineCategory::ToYaml,
        refine: |text| json::json_to_yaml(text)
    },
    /// JSON形式をYAML形式へ変換する（元のキー順序を維持する）
    JsonToYamlPreserveOrder => {
        label: "JSON→YAML(キー順序保持)",
        category: RefineCategory::ToYaml,
        refine: |text| json::json_to_yaml_preserve_order(text)
    },
    /// Markdown形式のテキストをHTML形式へ変換する
    MarkdownToHtml => {
        label: "Markdown→HTML",
        category: RefineCategory::Normal,
        refine: |text| markdown::markdown_to_html(text)
    },
    /// ExcelでコピーしたTSV形式のテキストをMarkdown形式へ変換する
    ExcelToMarkdown => {
        label: "Excel→Markdown",
        category: RefineCategory::Normal,
        refine: |text| markdown::excel_to_markdown_table(text)
    },
    /// Unixタイムスタンプを日時文字列へ変換する
    TimestampToDatetime => {
        label: "Unixタイムスタンプ→日時文字列",
        category: RefineCategory::Datetime,
        refine: |text| datetime::timestamp_to_datetime_string(text)
    },
    /// 日時文字列をUnixタイムスタンプへ変換する
    DatetimeToTimestamp => {
        label: "日時文字列→Unixタイムスタンプ",
        category: RefineCategory::Datetime,
        refine: |text| datetime::datetime_string_to_timestamp(text)
    },
    /// 数値に対して3桁ごとのカンマを付与する（例: 1000 -> 1,000）
    AddComma => {
        label: "カンマ追加",
        category: RefineCategory::Number,
        refine: |text| number::add_commas(text)
    },
    /// 数値からカンマを削除する（例: 1,000 -> 1000）
    RemoveComma => {
        label: "カンマ除去",
        category: RefineCategory::Number,
        refine: |text| number::remove_commas(text)
    },
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
/// * `Option<String>` - テキストが加工された場合は `Some(加工後テキスト)` を返す。加工されなかった場合は `None` を返す。
pub fn process_clipboard(clipboard: &mut Clipboard, mode: RefineMode) -> Option<String> {
    let text = clipboard.get_text().ok()?;
    if text.is_empty() {
        return None;
    }

    let processed = mode.refine(&text);

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

        assert_eq!(RefineMode::JsonFormat.label(), "JSON整形(キー順序不同)");
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
        assert!(variants.contains(&RefineMode::SortLinesAsc));
        assert!(variants.contains(&RefineMode::SortLinesDesc));
        assert!(variants.contains(&RefineMode::TimestampToDatetime));
        assert_eq!(variants.len(), 31);
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
