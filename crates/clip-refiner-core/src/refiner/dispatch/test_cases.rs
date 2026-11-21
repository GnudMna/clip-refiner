use super::RefineMode;

// ======================================================================
// 固定入出力ケース
// ======================================================================
/// 設定非依存の `RefineMode` ごとの入出力ペア
pub(super) const FIXED_CASES: &[(RefineMode, &str, &str)] = &[
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
    (RefineMode::Base64Encode, "あいう", "44GC44GE44GG"),
    (RefineMode::Base64Decode, "44GC44GE44GG", "あいう"),
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
    (RefineMode::SortLinesNaturalAsc, "10\n9\n2", "2\n9\n10"),
    (RefineMode::SortLinesNaturalDesc, "2\n10\n9", "10\n9\n2"),
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
        RefineMode::MarkdownToRichText,
        "**bold**",
        "<p><strong>bold</strong></p>",
    ),
    (
        RefineMode::HtmlToMarkdown,
        "<p><strong>bold</strong></p>",
        "**bold**",
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
    (
        RefineMode::ToDatetimeStringUtc,
        "1700000000",
        "2023-11-14 22:13:20",
    ),
    (
        RefineMode::ToIso8601Utc,
        "1700000000",
        "2023-11-14T22:13:20Z",
    ),
    (RefineMode::AddComma, "1000", "1,000"),
    (RefineMode::RemoveComma, "1,000", "1000"),
    (RefineMode::ToCamelCase, "foo_bar", "fooBar"),
    (RefineMode::ToSnakeCase, "fooBar", "foo_bar"),
    (RefineMode::ToPascalCase, "foo_bar", "FooBar"),
    (RefineMode::ToKebabCase, "fooBar", "foo-bar"),
    (RefineMode::ToScreamingSnakeCase, "fooBar", "FOO_BAR"),
    (RefineMode::ToHalfwidth, "ＡＢＣ　ガ", "ABC ｶﾞ"),
    (RefineMode::ToFullwidth, "ABC ｶﾞ", "ＡＢＣ　ガ"),
];

/// 固定ケース以外で個別検証が必要なモード数 (ローカル日時 5 + 正規表現 4 + 画像 1)
pub(super) const SPECIAL_MODE_COUNT: usize = 10;
