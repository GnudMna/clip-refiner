use std::borrow::Cow;

use super::super::context::{RefineContext, RefineSkipReason};
use super::super::mode::RefineMode;
use super::super::transform::markdown;
use crate::security::is_within_parser_limit;

/// Markdown カテゴリの加工を実行する
pub(crate) fn refine<'a>(mode: RefineMode, text: &'a str, ctx: &RefineContext) -> Cow<'a, str> {
    if !is_within_parser_limit(text) {
        ctx.record_skip(RefineSkipReason::ParserInputTooLarge);
        return Cow::Borrowed(text);
    }

    match mode {
        RefineMode::MarkdownToHtml => markdown::markdown_to_html(text),
        _ => unreachable!("{mode:?} は Markdown カテゴリではない"),
    }
}
