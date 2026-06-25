use std::borrow::Cow;

use super::context::RefineContext;
use super::mode::RefineMode;
use super::transform::{
    datetime, escape, json, line_actions, markdown, number, path, regex, trim, url, yaml,
};

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
        match self {
            RefineMode::UrlEncode => url::url_encode(text),
            RefineMode::UrlDecode => {
                url::url_decode(text).map_or_else(|_| Cow::Borrowed(text), Cow::Owned)
            }
            RefineMode::RemoveUtm => url::remove_utm_params(text),
            RefineMode::ExtractBasename => path::extract_basename(text),
            RefineMode::ExtractBasenameQuoted => path::extract_basename_quoted(text),
            RefineMode::AddPathQuotes => path::add_path_quotes(text),
            RefineMode::RemovePathQuotes => path::remove_path_quotes(text),
            RefineMode::PathToSlash => path::convert_to_forward_slash(text),
            RefineMode::PathToBackslash => path::convert_to_backslash(text),
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
            RefineMode::RegexReplace => regex::regex_replace(text, &ctx.regex),
            RefineMode::RegexExtract => regex::regex_extract(text, &ctx.regex),
            RefineMode::RegexDelete => regex::regex_delete(text, &ctx.regex),
            RefineMode::RegexSplit => regex::regex_split(text, &ctx.regex),
            RefineMode::JsonFormat => json::format_json(text),
            RefineMode::JsonFormatPreserveOrder => json::format_json_preserve_order(text),
            RefineMode::YamlToJson => yaml::yaml_to_json(text),
            RefineMode::YamlToJsonPreserveOrder => yaml::yaml_to_json_preserve_order(text),
            RefineMode::JsonToYaml => json::json_to_yaml(text),
            RefineMode::JsonToYamlPreserveOrder => json::json_to_yaml_preserve_order(text),
            RefineMode::MarkdownToHtml => markdown::markdown_to_html(text),
            RefineMode::ExcelToMarkdown => markdown::excel_to_markdown_table(text),
            RefineMode::MarkdownToExcel => markdown::markdown_table_to_excel(text),
            RefineMode::TimestampToDatetime => datetime::timestamp_to_datetime_string(text),
            RefineMode::DatetimeToTimestamp => datetime::datetime_string_to_timestamp(text),
            RefineMode::AddComma => number::add_commas(text),
            RefineMode::RemoveComma => number::remove_commas(text),
        }
    }
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;

    use crate::config::RegexSettings;

    use strum::IntoEnumIterator;

    fn regex_ctx(pattern: &str, replacement: &str) -> RefineContext {
        RefineContext {
            regex: RegexSettings {
                pattern: pattern.to_string(),
                replacement: replacement.to_string(),
                ..RegexSettings::default()
            },
        }
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
        ];

        assert_eq!(
            CASES.len() + 6,
            RefineMode::iter().count(),
            "固定ケースと日時・正規表現モードの合計が RefineMode バリアント数と一致しません"
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
