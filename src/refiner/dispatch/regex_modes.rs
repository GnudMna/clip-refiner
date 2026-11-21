use std::borrow::Cow;

use super::super::context::RefineContext;
use super::super::mode::RefineMode;
use super::super::transform::regex;

/// 正規表現カテゴリの加工を実行する
pub(crate) fn refine<'a>(mode: RefineMode, text: &'a str, ctx: &RefineContext) -> Cow<'a, str> {
    match mode {
        RefineMode::RegexReplace => {
            regex::regex_replace(text, &ctx.regex, &mut ctx.regex_cache_mut())
        }
        RefineMode::RegexExtract => {
            regex::regex_extract(text, &ctx.regex, &mut ctx.regex_cache_mut())
        }
        RefineMode::RegexDelete => {
            regex::regex_delete(text, &ctx.regex, &mut ctx.regex_cache_mut())
        }
        RefineMode::RegexSplit => regex::regex_split(text, &ctx.regex, &mut ctx.regex_cache_mut()),
        _ => unreachable!("{mode:?} は Regex カテゴリではない"),
    }
}
