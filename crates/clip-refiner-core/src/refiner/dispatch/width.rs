use std::borrow::Cow;

use super::super::context::RefineContext;
use super::super::mode::RefineMode;
use super::super::transform::width as width_transform;

/// 全角・半角カテゴリの加工を実行する
pub(crate) fn refine<'a>(mode: RefineMode, text: &'a str, _ctx: &RefineContext) -> Cow<'a, str> {
    match mode {
        RefineMode::ToHalfwidth => width_transform::to_halfwidth(text),
        RefineMode::ToFullwidth => width_transform::to_fullwidth(text),
        _ => unreachable!("{mode:?} は Width カテゴリではない"),
    }
}
