use std::borrow::Cow;

use super::super::context::RefineContext;
use super::super::mode::RefineMode;
use super::super::transform::markdown;

/// Markdown カテゴリの加工を実行する
pub(crate) fn refine<'a>(mode: RefineMode, text: &'a str, _ctx: &RefineContext) -> Cow<'a, str> {
    match mode {
        RefineMode::MarkdownToHtml => markdown::markdown_to_html(text),
        _ => unreachable!("{mode:?} は Markdown カテゴリではない"),
    }
}
