use std::borrow::Cow;

use super::super::context::RefineContext;
use super::super::mode::RefineMode;
use super::super::transform::number as number_transform;

/// 数値変換カテゴリの加工を実行する
pub(crate) fn refine<'a>(mode: RefineMode, text: &'a str, _ctx: &RefineContext) -> Cow<'a, str> {
    match mode {
        RefineMode::AddComma => number_transform::add_commas(text),
        RefineMode::RemoveComma => number_transform::remove_commas(text),
        _ => unreachable!("{mode:?} は Number カテゴリではない"),
    }
}
