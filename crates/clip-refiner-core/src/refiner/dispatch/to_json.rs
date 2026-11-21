use std::borrow::Cow;

use super::super::context::RefineContext;
use super::super::mode::RefineMode;
use super::super::transform::yaml;

/// JSON へ変換カテゴリの加工を実行する
pub(crate) fn refine<'a>(mode: RefineMode, text: &'a str, _ctx: &RefineContext) -> Cow<'a, str> {
    match mode {
        RefineMode::YamlToJson => yaml::yaml_to_json(text),
        RefineMode::YamlToJsonPreserveOrder => yaml::yaml_to_json_preserve_order(text),
        _ => unreachable!("{mode:?} は ToJson カテゴリではない"),
    }
}
