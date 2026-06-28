use std::borrow::Cow;

use super::context::RefineContext;
use super::mode::{RefineCategory, RefineMode};

mod case_convert;
mod datetime;
mod escape;
mod excel;
mod json_format;
mod line_actions;
mod markdown;
mod number;
mod path;
mod regex_modes;
mod to_json;
mod to_yaml;
mod trim;
mod url;

// ======================================================================
// 加工インターフェース
// ======================================================================
/// クリップボードのテキストを加工するための共通インターフェース
pub trait Refiner {
    /// テキストを加工する
    ///
    /// # Arguments
    /// * `text` - 加工前のテキスト
    /// * `ctx` - 設定依存の加工パラメータ
    ///
    /// # Returns
    /// * `Cow<'a, str>` - 加工後のテキスト(変更がない場合は元のテキストを借用)
    fn refine<'a>(&self, text: &'a str, ctx: &RefineContext) -> Cow<'a, str>;
}

impl Refiner for RefineMode {
    fn refine<'a>(&self, text: &'a str, ctx: &RefineContext) -> Cow<'a, str> {
        refine_by_category(*self, text, ctx)
    }
}

/// `RefineMode` のカテゴリに応じて対応サブモジュールへディスパッチする
///
/// 新カテゴリ追加時はこことカテゴリ用 `refine` 実装の両方がコンパイルを要求する
fn refine_by_category<'a>(mode: RefineMode, text: &'a str, ctx: &RefineContext) -> Cow<'a, str> {
    match mode.category() {
        RefineCategory::UrlActions => url::refine(mode, text, ctx),
        RefineCategory::Path => path::refine(mode, text, ctx),
        RefineCategory::LineActions => line_actions::refine(mode, text, ctx),
        RefineCategory::Trim => trim::refine(mode, text, ctx),
        RefineCategory::Escape => escape::refine(mode, text, ctx),
        RefineCategory::Regex => regex_modes::refine(mode, text, ctx),
        RefineCategory::JsonFormat => json_format::refine(mode, text, ctx),
        RefineCategory::ToJson => to_json::refine(mode, text, ctx),
        RefineCategory::ToYaml => to_yaml::refine(mode, text, ctx),
        RefineCategory::Markdown => markdown::refine(mode, text, ctx),
        RefineCategory::Excel => excel::refine(mode, text, ctx),
        RefineCategory::Datetime => datetime::refine(mode, text, ctx),
        RefineCategory::Number => number::refine(mode, text, ctx),
        RefineCategory::Case => case_convert::refine(mode, text, ctx),
        RefineCategory::Normal => unreachable!("{mode:?} は Normal カテゴリに属さない"),
    }
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::super::transform::datetime;
    use super::*;

    use crate::config::RegexSettings;

    use strum::IntoEnumIterator;

    fn regex_ctx(pattern: &str, replacement: &str) -> RefineContext {
        let mut ctx = RefineContext::default();
        ctx.regex = RegexSettings {
            pattern: pattern.to_string(),
            replacement: replacement.to_string(),
            ..RegexSettings::default()
        };
        ctx
    }

    /// `全てのRefineModeバリアントを網羅するテーブル駆動テスト`
    /// 各モードが正しく配線され、期待通りの加工を行うかを確認する
    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_all_refine_modes() {
        const CASES: &[(RefineMode, &str, &str)] = &[
            (
                RefineMode::UrlEncode,
                "あいう",
                "%E3%81%82%E3%81%84%E3%81%86",
            ),
            (
                RefineMode::UrlDecode,
                "%E3%81%82%E3%81%84%E3%81%86",
                "あいう",
            ),
            (
                RefineMode::RemoveUtm,
                "http://example.com/?utm_source=test",
                "http://example.com/",
            ),
            (
                RefineMode::ExtractBasename,
                "C:\\path\\to\\file.txt",
                "file.txt",
            ),
            (
                RefineMode::ExtractBasenameQuoted,
                "C:\\path\\to\\file.txt",
                "\"file.txt\"",
            ),
            (
                RefineMode::AddPathQuotes,
                "C:\\path\\to\\file.txt",
                "\"C:\\path\\to\\file.txt\"",
            ),
            (
                RefineMode::RemovePathQuotes,
                "\"C:\\path\\to\\file.txt\"",
                "C:\\path\\to\\file.txt",
            ),
            (
                RefineMode::PathToSlash,
                "C:\\path\\to\\file.txt",
                "C:/path/to/file.txt",
            ),
            (
                RefineMode::PathToBackslash,
                "C:/path/to/file.txt",
                "C:\\path\\to\\file.txt",
            ),
            (RefineMode::SortLinesAsc, "c\na\nb", "a\nb\nc"),
            (RefineMode::SortLinesDesc, "a\nc\nb", "c\nb\na"),
            (RefineMode::RemoveEmptyLines, "a\n\nb", "a\nb"),
            (RefineMode::RemoveDuplicateLines, "a\na\nb", "a\nb"),
            (RefineMode::Trim, "  abc  ", "abc"),
            (RefineMode::TrimLines, " a \n b ", "a\nb"),
            (RefineMode::Escape, "\"", "\\\""),
            (RefineMode::Unescape, "\\\"", "\""),
            (RefineMode::RegexEscape, "(.*)", "\\(\\.\\*\\)"),
            (RefineMode::RegexUnescape, "\\(\\.\\*\\)", "(.*)"),
            (
                RefineMode::JsonFormat,
                "{\"b\":1,\"a\":2}",
                "{\n  \"a\": 2,\n  \"b\": 1\n}",
            ),
            (
                RefineMode::JsonFormatPreserveOrder,
                "{\"b\":1,\"a\":2}",
                "{\n  \"b\": 1,\n  \"a\": 2\n}",
            ),
            (
                RefineMode::YamlToJson,
                "a: 1\nb: 2",
                "{\n  \"a\": 1,\n  \"b\": 2\n}",
            ),
            (
                RefineMode::YamlToJsonPreserveOrder,
                "a: 1\nb: 2",
                "{\n  \"a\": 1,\n  \"b\": 2\n}",
            ),
            (RefineMode::JsonToYaml, "{\"a\":1}", "a: 1\n"),
            (RefineMode::JsonToYamlPreserveOrder, "{\"a\":1}", "a: 1\n"),
            (
                RefineMode::MarkdownToHtml,
                "**bold**",
                "<p><strong>bold</strong></p>",
            ),
            (
                RefineMode::ExcelToMarkdown,
                "A\tB\n1\t2",
                "| A | B |\n|---|---|\n| 1 | 2 |",
            ),
            (
                RefineMode::MarkdownToExcel,
                "| A | B |\n|---|---|\n| 1 | 2 |",
                "A\tB\n1\t2",
            ),
            (RefineMode::AddComma, "1000", "1,000"),
            (RefineMode::RemoveComma, "1,000", "1000"),
            (RefineMode::ToCamelCase, "foo_bar", "fooBar"),
            (RefineMode::ToSnakeCase, "fooBar", "foo_bar"),
            (RefineMode::ToPascalCase, "foo_bar", "FooBar"),
            (RefineMode::ToKebabCase, "fooBar", "foo-bar"),
            (RefineMode::ToScreamingSnakeCase, "fooBar", "FOO_BAR"),
        ];

        assert_eq!(
            CASES.len() + 7,
            RefineMode::iter().count(),
            "固定ケースと日時・正規表現・画像モードの合計が RefineMode バリアント数と一致しません"
        );

        let empty_ctx = RefineContext::default();

        for mode in RefineMode::iter() {
            match mode {
                RefineMode::TimestampToDatetime => {
                    let input = "1672531200";
                    let actual = mode.refine(input, &empty_ctx);
                    let expected = datetime::timestamp_to_datetime_string(input);
                    assert_eq!(actual, expected);
                    assert_ne!(actual.as_ref(), input);
                }
                RefineMode::DatetimeToTimestamp => {
                    let datetime_input = datetime::timestamp_to_datetime_string("1672531200");
                    let actual = mode.refine(&datetime_input, &empty_ctx);
                    assert_eq!(actual, "1672531200");
                }
                RefineMode::RegexReplace => {
                    let ctx = regex_ctx(r"\d", "X");
                    assert_eq!(mode.refine("a1b2", &ctx), "aXbX");
                }
                RefineMode::RegexExtract => {
                    let ctx = regex_ctx(r"\d+", "");
                    assert_eq!(mode.refine("a1b22", &ctx), "1\n22");
                }
                RefineMode::RegexDelete => {
                    let ctx = regex_ctx(r"\d", "");
                    assert_eq!(mode.refine("a1b2", &ctx), "ab");
                }
                RefineMode::RegexSplit => {
                    let ctx = regex_ctx(",", "");
                    assert_eq!(mode.refine("a,b,c", &ctx), "a\nb\nc");
                }
                RefineMode::ExcelToImage => {
                    // 画像出力モードは `process_image_clipboard` で処理する
                }
                other => {
                    let (input, expected) = CASES.iter().find(|(m, _, _)| *m == other).map_or_else(
                        || panic!("TestCase missing for {other:?}"),
                        |(_, input, expected)| (*input, *expected),
                    );
                    let actual = other.refine(input, &empty_ctx);
                    assert_eq!(
                        actual, expected,
                        "Failed at mode: {other:?}\nInput: {input}\nExpected: {expected}\nActual: {actual}"
                    );
                }
            }
        }
    }
}
