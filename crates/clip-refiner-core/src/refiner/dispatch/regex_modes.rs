use std::borrow::Cow;

use super::super::context::RefineContext;
use super::super::mode::RefineMode;
use super::super::transform::regex;

/// 正規表現カテゴリの加工を実行する
pub(crate) fn refine<'a>(mode: RefineMode, text: &'a str, ctx: &RefineContext) -> Cow<'a, str> {
    match mode {
        RefineMode::RegexReplace => regex::regex_replace(text, ctx),
        RefineMode::RegexExtract => regex::regex_extract(text, ctx),
        RefineMode::RegexDelete => regex::regex_delete(text, ctx),
        RefineMode::RegexSplit => regex::regex_split(text, ctx),
        _ => unreachable!("{mode:?} は Regex カテゴリではない"),
    }
}
