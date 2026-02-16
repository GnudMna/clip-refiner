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
use strum::{EnumIter, EnumMessage, EnumProperty, IntoEnumIterator, IntoStaticStr};

/// クリップボードのテキストを加工する各モードの定義
#[derive(
    Copy,
    Clone,
    Debug,
    ValueEnum,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    EnumIter,
    EnumMessage,
    EnumProperty,
    IntoStaticStr,
)]
pub enum RefineMode {
    /// URLエンコードを行う
    #[value(help = "URLエンコード")]
    #[strum(message = "URLエンコード", props(Category = "URL操作"))]
    UrlEncode,
    /// URLデコードを行う。失敗した場合は元のテキストを維持する
    #[value(help = "URLデコード")]
    #[strum(message = "URLデコード", props(Category = "URL操作"))]
    UrlDecode,
    /// URLから utm_ で始まる計測用パラメータを削除する
    #[value(help = "UTMパラメータを削除")]
    #[strum(message = "UTM除去", props(Category = "URL操作"))]
    RemoveUtm,
    /// パスからベースネームを抽出する
    #[value(help = "パスからベースネームを抽出")]
    #[strum(message = "ベースネーム抽出", props(Category = "パス操作"))]
    ExtractBasename,
    /// パスからベースネームを抽出しダブルクォーテーションで囲む
    #[value(help = "パスからベースネームを抽出(引用符付き)")]
    #[strum(message = "ベースネーム抽出(引用符付)", props(Category = "パス操作"))]
    ExtractBasenameQuoted,
    /// パスの前後にダブルクォーテーションを付与する
    #[value(help = "パスに引用符を付与")]
    #[strum(message = "引用符を付与", props(Category = "パス操作"))]
    AddPathQuotes,
    /// パスの前後にあるダブルクォーテーションを削除する
    #[value(help = "パスの引用符を削除")]
    #[strum(message = "引用符を削除", props(Category = "パス操作"))]
    RemovePathQuotes,
    /// パスのバックスラッシュをスラッシュに変換する
    #[value(help = "パスをスラッシュ区切りに変換")]
    #[strum(message = "スラッシュ区切りに変換", props(Category = "パス操作"))]
    PathToSlash,
    /// パスのスラッシュをバックスラッシュに変換する
    #[value(help = "パスをバックスラッシュ区切りに変換")]
    #[strum(message = "バックスラッシュ区切りに変換", props(Category = "パス操作"))]
    PathToBackslash,
    /// 行単位で昇順に並び替える。CSVの場合は各行をレコードとして認識してソートする
    #[value(help = "昇順で並び替え")]
    #[strum(message = "昇順で並び替え", props(Category = "行操作"))]
    SortLinesAsc,
    /// 行単位で降順に並び替える。CSVの場合は各行をレコードとして認識してソートする
    #[value(help = "降順で並び替え")]
    #[strum(message = "降順で並び替え", props(Category = "行操作"))]
    SortLinesDesc,
    /// 空行を削除する
    #[value(help = "空行を削除")]
    #[strum(message = "空行削除", props(Category = "行操作"))]
    RemoveEmptyLines,
    /// 重複行を削除する
    #[value(help = "重複行を削除")]
    #[strum(message = "重複行削除", props(Category = "行操作"))]
    RemoveDuplicateLines,
    /// テキスト全体の前後にある空白および改行を削除する
    #[value(help = "改行や空白を整形")]
    #[strum(message = "全体をトリム", props(Category = "トリム"))]
    Trim,
    /// 行ごとに前後の空白を削除する
    #[value(help = "行単位で改行や空白を整形")]
    #[strum(message = "行単位でトリム", props(Category = "トリム"))]
    TrimLines,
    /// 文字列をバックスラッシュでエスケープする
    #[value(help = "文字列をエスケープ")]
    #[strum(message = "エスケープ", props(Category = "エスケープ"))]
    Escape,
    /// 文字列のエスケープを解除する
    #[value(help = "文字列のアンエスケープ")]
    #[strum(message = "アンエスケープ", props(Category = "エスケープ"))]
    Unescape,
    /// 正規表現のメタ文字をエスケープする
    #[value(help = "正規表現のエスケープ")]
    #[strum(message = "正規表現エスケープ", props(Category = "エスケープ"))]
    RegexEscape,
    /// 正規表現のエスケープを解除する
    #[value(help = "正規表現のアンエスケープ")]
    #[strum(message = "正規表現アンエスケープ", props(Category = "エスケープ"))]
    RegexUnescape,
    /// JSON形式をインデント整形する（キーの順序はパース時に不定となる）
    #[value(help = "JSON形式を整形(キー順序不同)")]
    #[strum(message = "JSON整形(キー順序不同)", props(Category = "JSON整形"))]
    JsonFormat,
    /// JSON形式をインデント整形する（元のキー順序を維持する）
    #[value(help = "JSON形式を整形(キー順序保持)")]
    #[strum(message = "JSON整形(キー順序保持)", props(Category = "JSON整形"))]
    JsonFormatPreserveOrder,
    /// YAML形式をJSON形式へ変換する
    #[value(help = "YAML形式をJSON形式へ変換(キー順序不同)")]
    #[strum(message = "YAML→JSON(キー順序不同)", props(Category = "JSONへ変換"))]
    YamlToJson,
    /// YAML形式をJSON形式へ変換する（元のキー順序を維持する）
    #[value(help = "YAML形式をJSON形式へ変換(キー順序保持)")]
    #[strum(message = "YAML→JSON(キー順序保持)", props(Category = "JSONへ変換"))]
    YamlToJsonPreserveOrder,
    /// JSON形式をYAML形式へ変換する
    #[value(help = "JSON形式をYAML形式へ変換(キー順序不同)")]
    #[strum(message = "JSON→YAML(キー順序不同)", props(Category = "YAMLへ変換"))]
    JsonToYaml,
    /// JSON形式をYAML形式へ変換する（元のキー順序を維持する）
    #[value(help = "JSON形式をYAML形式へ変換(キー順序保持)")]
    #[strum(message = "JSON→YAML(キー順序保持)", props(Category = "YAMLへ変換"))]
    JsonToYamlPreserveOrder,
    /// Markdown形式のテキストをHTML形式へ変換する
    #[value(help = "MarkdownをHTML形式へ変換")]
    #[strum(message = "Markdown→HTML", props(Category = ""))]
    MarkdownToHtml,
    /// ExcelでコピーしたTSV形式のテキストをMarkdown形式へ変換する
    #[value(help = "Excel(TSV)をMarkdown形式へ変換")]
    #[strum(message = "Excel→Markdown", props(Category = ""))]
    ExcelToMarkdown,
    /// Unixタイムスタンプを日時文字列へ変換する
    #[value(help = "Unixタイムスタンプを日時文字列へ変換")]
    #[strum(
        message = "Unixタイムスタンプ→日時文字列",
        props(Category = "日時変換")
    )]
    TimestampToDatetime,
    /// 日時文字列をUnixタイムスタンプへ変換する
    #[value(help = "日時文字列をUnixタイムスタンプへ変換")]
    #[strum(
        message = "日時文字列→Unixタイムスタンプ",
        props(Category = "日時変換")
    )]
    DatetimeToTimestamp,
    /// 数値に対して3桁ごとのカンマを付与する（例: 1000 -> 1,000）
    #[value(help = "カンマ無し数値をカンマ区切りの数値に")]
    #[strum(message = "カンマ追加", props(Category = "数値変換"))]
    AddComma,
    /// 数値からカンマを削除する（例: 1,000 -> 1000）
    #[value(help = "カンマ区切りの数値をカンマ無し数値に")]
    #[strum(message = "カンマ除去", props(Category = "数値変換"))]
    RemoveComma,
}

/// メニューの階層化に使用するカテゴリ
#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash, EnumIter, EnumMessage, IntoStaticStr)]
pub enum RefineCategory {
    /// 通常の単独メニュー
    #[strum(message = "")]
    Normal,
    /// URL操作サブメニュー内
    #[strum(message = "URL操作")]
    UrlActions,
    /// パス操作サブメニュー内
    #[strum(message = "パス操作")]
    Path,
    /// 行操作サブメニュー内
    #[strum(message = "行操作")]
    LineActions,
    /// トリムサブメニュー内
    #[strum(message = "トリム")]
    Trim,
    /// エスケープサブメニュー内
    #[strum(message = "エスケープ")]
    Escape,
    /// JSON整形サブメニュー内
    #[strum(message = "JSON整形")]
    JsonFormat,
    /// JSON to YAMLサブメニュー内
    #[strum(message = "YAMLへ変換")]
    ToYaml,
    /// YAML to JSONサブメニュー内
    #[strum(message = "JSONへ変換")]
    ToJson,
    /// 日時変換サブメニュー内
    #[strum(message = "日時変換")]
    Datetime,
    /// 数値変換サブメニュー内
    #[strum(message = "数値変換")]
    Number,
}

impl RefineCategory {
    /// カテゴリの表示名を取得する
    pub fn label(&self) -> &'static str {
        self.get_message().unwrap_or("")
    }
}

impl RefineMode {
    /// UIに表示する名前を取得する
    ///
    /// # Returns
    /// * `&'static str` - モードに対応する静的な文字列ラベル。
    pub fn label(&self) -> &'static str {
        self.get_message().unwrap_or("")
    }

    /// 所属するカテゴリを取得する。トレイメニューの階層構築に利用される
    ///
    /// # Returns
    /// * `RefineCategory` - モードが属するカテゴリ。
    pub fn category(&self) -> RefineCategory {
        let cat_str = self.get_str("Category").unwrap_or("");
        RefineCategory::iter()
            .find(|c| c.label() == cat_str)
            .unwrap_or(RefineCategory::Normal)
    }

    /// UI（Webview）に渡すためのモード情報のJSONリストを生成する
    pub fn to_json_list() -> String {
        let list: Vec<serde_json::Value> = RefineMode::iter()
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

/// クリップボードのテキストを加工するトレイト
pub trait Refiner {
    /// テキストを加工する
    ///
    /// # Arguments
    /// * `text` - 加工前のテキスト
    ///
    /// # Returns
    /// * `String` - 加工後のテキスト
    fn refine(&self, text: &str) -> String;
}

impl Refiner for RefineMode {
    fn refine(&self, text: &str) -> String {
        match self {
            RefineMode::UrlEncode => url::url_encode(text),
            RefineMode::UrlDecode => url::url_decode(text).unwrap_or_else(|_| text.to_string()),
            RefineMode::RemoveUtm => url::remove_utm_params(text),
            RefineMode::ExtractBasename => {
                path::extract_basename(text).unwrap_or_else(|| text.to_string())
            }
            RefineMode::ExtractBasenameQuoted => {
                path::extract_basename_quoted(text).unwrap_or_else(|| text.to_string())
            }
            RefineMode::AddPathQuotes => {
                path::add_path_quotes(text).unwrap_or_else(|| text.to_string())
            }
            RefineMode::RemovePathQuotes => {
                path::remove_path_quotes(text).unwrap_or_else(|| text.to_string())
            }
            RefineMode::PathToSlash => {
                path::convert_to_forward_slash(text).unwrap_or_else(|| text.to_string())
            }
            RefineMode::PathToBackslash => {
                path::convert_to_backslash(text).unwrap_or_else(|| text.to_string())
            }
            RefineMode::SortLinesAsc => line_actions::sort_lines(text, false),
            RefineMode::SortLinesDesc => line_actions::sort_lines(text, true),
            RefineMode::RemoveEmptyLines => line_actions::remove_empty_lines(text),
            RefineMode::RemoveDuplicateLines => line_actions::remove_duplicate_lines(text),
            RefineMode::Trim => trim::trim_text(text),
            RefineMode::TrimLines => trim::trim_lines(text),
            RefineMode::Escape => escape::escape_string(text),
            RefineMode::Unescape => escape::unescape_string(text),
            RefineMode::RegexEscape => escape::regex_escape(text),
            RefineMode::RegexUnescape => escape::regex_unescape(text),
            RefineMode::JsonFormat => json::format_json(text),
            RefineMode::JsonFormatPreserveOrder => json::format_json_preserve_order(text),
            RefineMode::YamlToJson => yaml::yaml_to_json(text),
            RefineMode::YamlToJsonPreserveOrder => yaml::yaml_to_json_preserve_order(text),
            RefineMode::JsonToYaml => json::json_to_yaml(text),
            RefineMode::JsonToYamlPreserveOrder => json::json_to_yaml_preserve_order(text),
            RefineMode::MarkdownToHtml => markdown::markdown_to_html(text),
            RefineMode::ExcelToMarkdown => markdown::excel_to_markdown_table(text),
            RefineMode::TimestampToDatetime => datetime::timestamp_to_datetime_string(text),
            RefineMode::DatetimeToTimestamp => datetime::datetime_string_to_timestamp(text),
            RefineMode::AddComma => number::add_commas(text),
            RefineMode::RemoveComma => number::remove_commas(text),
        }
    }
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
        let variants: Vec<_> = RefineMode::iter().collect();
        assert!(variants.contains(&RefineMode::UrlEncode));
        assert!(variants.contains(&RefineMode::SortLinesAsc));
        assert!(variants.contains(&RefineMode::SortLinesDesc));
        assert!(variants.contains(&RefineMode::TimestampToDatetime));
        assert_eq!(variants.len(), 31);
    }

    /// クリップボード処理の統合テスト
    /// 実際のクリップボード操作を伴うため、実行環境によってはスキップされる可能性がある
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

    /// 全てのRefineModeバリアントを網羅するテーブル駆動テスト
    /// 各モードが正しく配線され、期待通りの加工を行うかを確認する
    #[test]
    fn test_all_refine_modes() {
        struct TestCase {
            mode: RefineMode,
            input: &'static str,
            expected: &'static str,
        }

        let cases = vec![
            TestCase {
                mode: RefineMode::UrlEncode,
                input: "あいう",
                expected: "%E3%81%82%E3%81%84%E3%81%86",
            },
            TestCase {
                mode: RefineMode::UrlDecode,
                input: "%E3%81%82%E3%81%84%E3%81%86",
                expected: "あいう",
            },
            TestCase {
                mode: RefineMode::RemoveUtm,
                input: "http://example.com/?utm_source=test",
                expected: "http://example.com/",
            },
            TestCase {
                mode: RefineMode::ExtractBasename,
                input: "C:\\path\\to\\file.txt",
                expected: "file.txt",
            },
            TestCase {
                mode: RefineMode::ExtractBasenameQuoted,
                input: "C:\\path\\to\\file.txt",
                expected: "\"file.txt\"",
            },
            TestCase {
                mode: RefineMode::AddPathQuotes,
                input: "C:\\path\\to\\file.txt",
                expected: "\"C:\\path\\to\\file.txt\"",
            },
            TestCase {
                mode: RefineMode::RemovePathQuotes,
                input: "\"C:\\path\\to\\file.txt\"",
                expected: "C:\\path\\to\\file.txt",
            },
            TestCase {
                mode: RefineMode::PathToSlash,
                input: "C:\\path\\to\\file.txt",
                expected: "C:/path/to/file.txt",
            },
            TestCase {
                mode: RefineMode::PathToBackslash,
                input: "C:/path/to/file.txt",
                expected: "C:\\path\\to\\file.txt",
            },
            TestCase {
                mode: RefineMode::SortLinesAsc,
                input: "c\na\nb",
                expected: "a\nb\nc",
            },
            TestCase {
                mode: RefineMode::SortLinesDesc,
                input: "a\nc\nb",
                expected: "c\nb\na",
            },
            TestCase {
                mode: RefineMode::RemoveEmptyLines,
                input: "a\n\nb",
                expected: "a\nb",
            },
            TestCase {
                mode: RefineMode::RemoveDuplicateLines,
                input: "a\na\nb",
                expected: "a\nb",
            },
            TestCase {
                mode: RefineMode::Trim,
                input: "  abc  ",
                expected: "abc",
            },
            TestCase {
                mode: RefineMode::TrimLines,
                input: " a \n b ",
                expected: "a\nb",
            },
            TestCase {
                mode: RefineMode::Escape,
                input: "\"",
                expected: "\\\"",
            },
            TestCase {
                mode: RefineMode::Unescape,
                input: "\\\"",
                expected: "\"",
            },
            TestCase {
                mode: RefineMode::RegexEscape,
                input: "(.*)",
                expected: "\\(\\.\\*\\)",
            },
            TestCase {
                mode: RefineMode::RegexUnescape,
                input: "\\(\\.\\*\\)",
                expected: "(.*)",
            },
            TestCase {
                mode: RefineMode::JsonFormat,
                input: "{\"b\":1,\"a\":2}",
                expected: "{\n  \"a\": 2,\n  \"b\": 1\n}",
            },
            TestCase {
                mode: RefineMode::JsonFormatPreserveOrder,
                input: "{\"b\":1,\"a\":2}",
                expected: "{\n  \"b\": 1,\n  \"a\": 2\n}",
            },
            TestCase {
                mode: RefineMode::YamlToJson,
                input: "a: 1\nb: 2",
                expected: "{\n  \"a\": 1,\n  \"b\": 2\n}",
            },
            TestCase {
                mode: RefineMode::YamlToJsonPreserveOrder,
                input: "a: 1\nb: 2",
                expected: "{\n  \"a\": 1,\n  \"b\": 2\n}",
            },
            TestCase {
                mode: RefineMode::JsonToYaml,
                input: "{\"a\":1}",
                expected: "a: 1\n",
            },
            TestCase {
                mode: RefineMode::JsonToYamlPreserveOrder,
                input: "{\"a\":1}",
                expected: "a: 1\n",
            },
            TestCase {
                mode: RefineMode::MarkdownToHtml,
                input: "**bold**",
                expected: "<p><strong>bold</strong></p>",
            },
            TestCase {
                mode: RefineMode::ExcelToMarkdown,
                input: "A\tB\n1\t2",
                expected: "| A | B |\n|---|---|\n| 1 | 2 |",
            },
            TestCase {
                mode: RefineMode::TimestampToDatetime,
                input: "0",
                expected: "1970-01-01 09:00:00", // JST想定(環境依存だが固定値チェックのため)
            },
            TestCase {
                mode: RefineMode::DatetimeToTimestamp,
                input: "1970-01-01 09:00:00",
                expected: "0",
            },
            TestCase {
                mode: RefineMode::AddComma,
                input: "1000",
                expected: "1,000",
            },
            TestCase {
                mode: RefineMode::RemoveComma,
                input: "1,000",
                expected: "1000",
            },
        ];

        // 全モードが網羅されているかチェック
        let all_variants: Vec<_> = RefineMode::iter().collect();
        assert_eq!(
            cases.len(),
            all_variants.len(),
            "TestCase count does not match RefineMode variants count. Please add missing test cases."
        );

        for case in cases {
            // TimestampToDatetime はローカルタイムゾーンに依存するため、環境によって結果が変わる可能性がある
            // ここでは簡易的に、変換が成功して入力と異なる結果になることだけを確認する（変換失敗時は入力が返るため）
            if matches!(case.mode, RefineMode::TimestampToDatetime) {
                let result = case.mode.refine(case.input);
                assert_ne!(result, case.input, "TimestampToDatetime failed to convert");
                // JST環境なら一致するはずだが、CI環境などでUTCの場合は一致しない。
                // 厳密なチェックは datetime.rs のテストに任せる
                continue;
            }

            // DatetimeToTimestamp も同様
            if matches!(case.mode, RefineMode::DatetimeToTimestamp) {
                let result = case.mode.refine(case.input);
                assert_ne!(result, case.input, "DatetimeToTimestamp failed to convert");
                continue;
            }

            let actual = case.mode.refine(case.input);
            assert_eq!(
                actual, case.expected,
                "Failed at mode: {:?}\nInput: {}\nExpected: {}\nActual: {}",
                case.mode, case.input, case.expected, actual
            );
        }
    }
}
