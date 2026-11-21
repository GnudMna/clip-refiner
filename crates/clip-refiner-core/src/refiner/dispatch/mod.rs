use std::borrow::Cow;

use super::context::{RefineContext, RefineSkipReason};
use super::mode::RefineMode;
use super::transform::html::{self, HtmlToMarkdownResult};
use super::transform::{
    base64, case_convert, datetime, escape, json, line_actions, markdown, number, path, regex,
    trim, url, width, yaml,
};
use crate::security::is_within_parser_limit;

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
        refine_by_mode(*self, text, ctx)
    }
}

// ======================================================================
// プライベート関数
// ======================================================================
/// `RefineMode` に対応する変換処理へディスパッチする
///
/// `RefineMode` を網羅的に match するため、モード追加時の振り分け漏れはコンパイルエラーになる
/// 振り分けは `category()` に依存しない (カテゴリの割り当て誤りで実行時にパニックしない)
fn refine_by_mode<'a>(mode: RefineMode, text: &'a str, ctx: &RefineContext) -> Cow<'a, str> {
    match mode {
        RefineMode::UrlEncode => url::url_encode(text),
        RefineMode::UrlDecode => {
            url::url_decode(text).map_or_else(|_| Cow::Borrowed(text), Cow::Owned)
        }
        RefineMode::RemoveUtm => url::remove_utm_params(text),
        RefineMode::Base64Encode => Cow::Owned(base64::base64_encode(text)),
        RefineMode::Base64Decode => {
            base64::base64_decode(text).map_or(Cow::Borrowed(text), Cow::Owned)
        }
        RefineMode::ExtractBasename => path::extract_basename(text),
        RefineMode::ExtractBasenameQuoted => path::extract_basename_quoted(text),
        RefineMode::AddPathQuotes => path::add_path_quotes(text),
        RefineMode::RemovePathQuotes => path::remove_path_quotes(text),
        RefineMode::PathToSlash => path::convert_to_forward_slash(text),
        RefineMode::PathToBackslash => path::convert_to_backslash(text),
        RefineMode::SortLinesAsc => line_actions::sort_lines(text, false),
        RefineMode::SortLinesDesc => line_actions::sort_lines(text, true),
        RefineMode::SortLinesNaturalAsc => line_actions::sort_lines_natural(text, false),
        RefineMode::SortLinesNaturalDesc => line_actions::sort_lines_natural(text, true),
        RefineMode::RemoveEmptyLines => line_actions::remove_empty_lines(text),
        RefineMode::RemoveDuplicateLines => line_actions::remove_duplicate_lines(text),
        RefineMode::Trim => trim::trim_text(text),
        RefineMode::TrimLines => trim::trim_lines(text),
        RefineMode::Escape => escape::escape_string(text),
        RefineMode::Unescape => escape::unescape_string(text),
        RefineMode::RegexEscape => escape::regex_escape(text),
        RefineMode::RegexUnescape => escape::regex_unescape(text),
        RefineMode::RegexReplace => regex::regex_replace(text, ctx),
        RefineMode::RegexExtract => regex::regex_extract(text, ctx),
        RefineMode::RegexDelete => regex::regex_delete(text, ctx),
        RefineMode::RegexSplit => regex::regex_split(text, ctx),
        RefineMode::JsonFormat => with_parser_limit(text, ctx, json::format_json),
        RefineMode::JsonFormatPreserveOrder => {
            with_parser_limit(text, ctx, json::format_json_preserve_order)
        }
        RefineMode::YamlToJson => with_parser_limit(text, ctx, yaml::yaml_to_json),
        RefineMode::YamlToJsonPreserveOrder => {
            with_parser_limit(text, ctx, yaml::yaml_to_json_preserve_order)
        }
        RefineMode::JsonToYaml => with_parser_limit(text, ctx, json::json_to_yaml),
        RefineMode::JsonToYamlPreserveOrder => {
            with_parser_limit(text, ctx, json::json_to_yaml_preserve_order)
        }
        // リッチテキストの `CF_HTML` 書き込みは `process_clipboard_pipeline_io` で行う
        RefineMode::MarkdownToHtml | RefineMode::MarkdownToRichText => {
            with_parser_limit(text, ctx, markdown::markdown_to_html)
        }
        RefineMode::HtmlToMarkdown => {
            with_parser_limit(text, ctx, |text| html_to_markdown(text, ctx))
        }
        RefineMode::ExcelToMarkdown => markdown::excel_to_markdown_table(text),
        RefineMode::MarkdownToExcel => markdown::markdown_table_to_excel(text),
        // 画像出力は `process_image_clipboard` で処理する
        RefineMode::ExcelToImage => Cow::Borrowed(text),
        RefineMode::ToDatetimeString => datetime::to_datetime_string(text),
        RefineMode::ToDatetimeStringUtc => datetime::to_datetime_string_utc(text),
        RefineMode::ToIso8601 => datetime::to_iso8601(text),
        RefineMode::ToIso8601Utc => datetime::to_iso8601_utc(text),
        RefineMode::ToRfc2822 => datetime::to_rfc2822(text),
        RefineMode::ToJapaneseDatetime => datetime::to_japanese_datetime(text),
        RefineMode::ToUnixTimestamp => datetime::to_unix_timestamp(text),
        RefineMode::AddComma => number::add_commas(text),
        RefineMode::RemoveComma => number::remove_commas(text),
        RefineMode::ToCamelCase => case_convert::to_camel_case(text),
        RefineMode::ToSnakeCase => case_convert::to_snake_case(text),
        RefineMode::ToPascalCase => case_convert::to_pascal_case(text),
        RefineMode::ToKebabCase => case_convert::to_kebab_case(text),
        RefineMode::ToScreamingSnakeCase => case_convert::to_screaming_snake_case(text),
        RefineMode::ToHalfwidth => width::to_halfwidth(text),
        RefineMode::ToFullwidth => width::to_fullwidth(text),
    }
}

/// パーサ入力の上限内であれば `convert` を適用し、超える場合はスキップを記録して素通しする
fn with_parser_limit<'a>(
    text: &'a str,
    ctx: &RefineContext,
    convert: impl FnOnce(&'a str) -> Cow<'a, str>,
) -> Cow<'a, str> {
    if !is_within_parser_limit(text) {
        ctx.record_skip(RefineSkipReason::ParserInputTooLarge);
        return Cow::Borrowed(text);
    }
    convert(text)
}

/// HTML を Markdown に変換する
///
/// 変換できない・変換結果が空か入力と同じ場合は元のテキストを返す
fn html_to_markdown<'a>(text: &'a str, ctx: &RefineContext) -> Cow<'a, str> {
    match html::html_to_markdown(text) {
        // 非表示要素だけの HTML で空文字列を書き戻さない
        HtmlToMarkdownResult::Converted(markdown) if !markdown.is_empty() && markdown != text => {
            Cow::Owned(markdown)
        }
        HtmlToMarkdownResult::Converted(_) | HtmlToMarkdownResult::NotHtml => Cow::Borrowed(text),
        HtmlToMarkdownResult::TooDeep => {
            ctx.record_skip(RefineSkipReason::HtmlNestingTooDeep);
            Cow::Borrowed(text)
        }
    }
}

#[cfg(test)]
mod test_cases;
#[cfg(test)]
mod tests;
