use std::borrow::Cow;

use super::super::context::{RefineContext, RefineSkipReason};
use super::super::mode::RefineMode;
use super::super::transform::yaml;
use crate::security::is_within_parser_limit;

/// JSON へ変換カテゴリの加工を実行する
pub(crate) fn refine<'a>(mode: RefineMode, text: &'a str, ctx: &RefineContext) -> Cow<'a, str> {
    if !is_within_parser_limit(text) {
        ctx.record_skip(RefineSkipReason::ParserInputTooLarge);
        return Cow::Borrowed(text);
    }

    match mode {
        RefineMode::YamlToJson => yaml::yaml_to_json(text),
        RefineMode::YamlToJsonPreserveOrder => yaml::yaml_to_json_preserve_order(text),
        _ => unreachable!("{mode:?} は ToJson カテゴリではない"),
    }
}
