use std::borrow::Cow;

use super::super::context::{RefineContext, RefineSkipReason};
use super::super::mode::RefineMode;
use super::super::transform::json;
use crate::security::is_within_parser_limit;

/// JSON 整形カテゴリの加工を実行する
pub(crate) fn refine<'a>(mode: RefineMode, text: &'a str, ctx: &RefineContext) -> Cow<'a, str> {
    if !is_within_parser_limit(text) {
        ctx.record_skip(RefineSkipReason::ParserInputTooLarge);
        return Cow::Borrowed(text);
    }

    match mode {
        RefineMode::JsonFormat => json::format_json(text),
        RefineMode::JsonFormatPreserveOrder => json::format_json_preserve_order(text),
        _ => unreachable!("{mode:?} は JsonFormat カテゴリではない"),
    }
}
