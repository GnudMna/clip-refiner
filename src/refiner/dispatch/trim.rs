use std::borrow::Cow;

use super::super::context::RefineContext;
use super::super::mode::RefineMode;
use super::super::transform::trim as trim_transform;

/// トリムカテゴリの加工を実行する
pub(crate) fn refine<'a>(mode: RefineMode, text: &'a str, _ctx: &RefineContext) -> Cow<'a, str> {
    match mode {
        RefineMode::Trim => trim_transform::trim_text(text),
        RefineMode::TrimLines => trim_transform::trim_lines(text),
        _ => unreachable!("{mode:?} は Trim カテゴリではない"),
    }
}
