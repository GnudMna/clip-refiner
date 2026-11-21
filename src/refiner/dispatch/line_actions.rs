use std::borrow::Cow;

use super::super::context::RefineContext;
use super::super::mode::RefineMode;
use super::super::transform::line_actions as line_transform;

/// 行操作カテゴリの加工を実行する
pub(crate) fn refine<'a>(mode: RefineMode, text: &'a str, _ctx: &RefineContext) -> Cow<'a, str> {
    match mode {
        RefineMode::SortLinesAsc => line_transform::sort_lines(text, false),
        RefineMode::SortLinesDesc => line_transform::sort_lines(text, true),
        RefineMode::RemoveEmptyLines => line_transform::remove_empty_lines(text),
        RefineMode::RemoveDuplicateLines => line_transform::remove_duplicate_lines(text),
        _ => unreachable!("{mode:?} は LineActions カテゴリではない"),
    }
}
