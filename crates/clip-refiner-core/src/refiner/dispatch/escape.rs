use std::borrow::Cow;

use super::super::context::RefineContext;
use super::super::mode::RefineMode;
use super::super::transform::escape as escape_transform;

/// エスケープカテゴリの加工を実行する
pub(crate) fn refine<'a>(mode: RefineMode, text: &'a str, _ctx: &RefineContext) -> Cow<'a, str> {
    match mode {
        RefineMode::Escape => escape_transform::escape_string(text),
        RefineMode::Unescape => escape_transform::unescape_string(text),
        RefineMode::RegexEscape => escape_transform::regex_escape(text),
        RefineMode::RegexUnescape => escape_transform::regex_unescape(text),
        _ => unreachable!("{mode:?} は Escape カテゴリではない"),
    }
}
