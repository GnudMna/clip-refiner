use std::borrow::Cow;

use super::super::context::RefineContext;
use super::super::mode::RefineMode;
use super::super::transform::json;

/// JSON 整形カテゴリの加工を実行する
pub(crate) fn refine<'a>(mode: RefineMode, text: &'a str, _ctx: &RefineContext) -> Cow<'a, str> {
    match mode {
        RefineMode::JsonFormat => json::format_json(text),
        RefineMode::JsonFormatPreserveOrder => json::format_json_preserve_order(text),
        _ => unreachable!("{mode:?} は JsonFormat カテゴリではない"),
    }
}
