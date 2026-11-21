//! 統合テスト共通: 全 `RefineMode` の回帰ケース

#![allow(dead_code)]

use std::collections::HashSet;

use clip_refiner_core::config::RegexSettings;
use clip_refiner_core::{RefineContext, RefineMode, Refiner};

use strum::IntoEnumIterator;

/// 固定入出力で検証するテキスト加工モードのケース
pub const TEXT_MODE_CASES: &[(RefineMode, &str, &str)] = &[
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

/// 正規表現設定付きコンテキストを生成する
pub fn regex_ctx(pattern: &str, replacement: &str) -> RefineContext {
    RefineContext::with_regex(RegexSettings {
        pattern: pattern.to_string(),
        replacement: replacement.to_string(),
        ..RegexSettings::default()
    })
}

/// 全 `RefineMode` バリアントに回帰ケースが存在すること
pub fn assert_all_modes_covered() {
    let fixed: HashSet<_> = TEXT_MODE_CASES.iter().map(|(mode, _, _)| *mode).collect();
    // 固定ケース 35 + 日時 2 + 正規表現 4 + 画像 1 = 42
    assert_eq!(
        fixed.len() + 7,
        RefineMode::iter().count(),
        "固定ケースと特殊モードの合計が RefineMode バリアント数と一致しません"
    );

    for mode in RefineMode::iter() {
        match mode {
            RefineMode::TimestampToDatetime
            | RefineMode::DatetimeToTimestamp
            | RefineMode::RegexReplace
            | RefineMode::RegexExtract
            | RefineMode::RegexDelete
            | RefineMode::RegexSplit
            | RefineMode::ExcelToImage => {}
            other => {
                assert!(fixed.contains(&other), "回帰ケース未定義: {other:?}");
            }
        }
    }
}

/// 全モードの回帰テストを実行する
pub fn run_all_refine_mode_regression() {
    assert_all_modes_covered();

    let empty_ctx = RefineContext::default();

    for mode in RefineMode::iter() {
        match mode {
            RefineMode::TimestampToDatetime => {
                let input = "1672531200";
                let actual = mode.refine(input, &empty_ctx);
                assert_ne!(actual.as_ref(), input);
                let roundtrip = RefineMode::DatetimeToTimestamp.refine(&actual, &empty_ctx);
                assert_eq!(roundtrip, input);
            }
            RefineMode::DatetimeToTimestamp => {
                let datetime = RefineMode::TimestampToDatetime.refine("1672531200", &empty_ctx);
                let actual = mode.refine(&datetime, &empty_ctx);
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
                assert_eq!(mode.refine("A\tB\n1\t2", &empty_ctx), "A\tB\n1\t2");
            }
            other => {
                let (input, expected) = TEXT_MODE_CASES
                    .iter()
                    .find(|(m, _, _)| *m == other)
                    .map_or_else(
                        || panic!("回帰ケース未定義: {other:?}"),
                        |(_, input, expected)| (*input, *expected),
                    );
                let actual = other.refine(input, &empty_ctx);
                assert_eq!(
                    actual, expected,
                    "mode={other:?}\ninput={input}\nexpected={expected}\nactual={actual}"
                );
            }
        }
    }
}
