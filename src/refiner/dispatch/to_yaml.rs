use std::borrow::Cow;

use super::super::context::RefineContext;
use super::super::mode::RefineMode;
use super::super::transform::json;

/// YAML へ変換カテゴリの加工を実行する
pub(crate) fn refine<'a>(mode: RefineMode, text: &'a str, _ctx: &RefineContext) -> Cow<'a, str> {
    match mode {
        RefineMode::JsonToYaml => json::json_to_yaml(text),
        RefineMode::JsonToYamlPreserveOrder => json::json_to_yaml_preserve_order(text),
        _ => unreachable!("{mode:?} は ToYaml カテゴリではない"),
    }
}
