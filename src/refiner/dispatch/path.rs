use std::borrow::Cow;

use super::super::context::RefineContext;
use super::super::mode::RefineMode;
use super::super::transform::path as path_transform;

/// パス操作カテゴリの加工を実行する
pub(crate) fn refine<'a>(mode: RefineMode, text: &'a str, _ctx: &RefineContext) -> Cow<'a, str> {
    match mode {
        RefineMode::ExtractBasename => path_transform::extract_basename(text),
        RefineMode::ExtractBasenameQuoted => path_transform::extract_basename_quoted(text),
        RefineMode::AddPathQuotes => path_transform::add_path_quotes(text),
        RefineMode::RemovePathQuotes => path_transform::remove_path_quotes(text),
        RefineMode::PathToSlash => path_transform::convert_to_forward_slash(text),
        RefineMode::PathToBackslash => path_transform::convert_to_backslash(text),
        _ => unreachable!("{mode:?} は Path カテゴリではない"),
    }
}
