use std::borrow::Cow;

use super::super::context::{RefineContext, RefineSkipReason};
use super::super::mode::RefineMode;
use super::super::transform::html::{self, HtmlToMarkdownResult};
use super::super::transform::markdown;
use crate::security::is_within_parser_limit;

/// Markdown カテゴリの加工を実行する
pub(crate) fn refine<'a>(mode: RefineMode, text: &'a str, ctx: &RefineContext) -> Cow<'a, str> {
    if !is_within_parser_limit(text) {
        ctx.record_skip(RefineSkipReason::ParserInputTooLarge);
        return Cow::Borrowed(text);
    }

    match mode {
        // リッチテキストの `CF_HTML` 書き込みは `process_clipboard_pipeline_io` で行う
        RefineMode::MarkdownToHtml | RefineMode::MarkdownToRichText => {
            markdown::markdown_to_html(text)
        }
        RefineMode::HtmlToMarkdown => match html::html_to_markdown(text) {
            // 非表示要素だけの HTML で空文字列を書き戻さない
            HtmlToMarkdownResult::Converted(markdown)
                if !markdown.is_empty() && markdown != text =>
            {
                Cow::Owned(markdown)
            }
            HtmlToMarkdownResult::Converted(_) | HtmlToMarkdownResult::NotHtml => {
                Cow::Borrowed(text)
            }
            HtmlToMarkdownResult::TooDeep => {
                ctx.record_skip(RefineSkipReason::HtmlNestingTooDeep);
                Cow::Borrowed(text)
            }
        },
        _ => unreachable!("{mode:?} は Markdown カテゴリではない"),
    }
}
