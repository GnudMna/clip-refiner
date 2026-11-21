use std::borrow::Cow;

use super::super::context::RefineContext;
use super::super::mode::RefineMode;
use super::super::transform::case_convert as case_transform;

/// ケース変換カテゴリの加工を実行する
pub(crate) fn refine<'a>(mode: RefineMode, text: &'a str, _ctx: &RefineContext) -> Cow<'a, str> {
    match mode {
        RefineMode::ToCamelCase => case_transform::to_camel_case(text),
        RefineMode::ToSnakeCase => case_transform::to_snake_case(text),
        RefineMode::ToPascalCase => case_transform::to_pascal_case(text),
        RefineMode::ToKebabCase => case_transform::to_kebab_case(text),
        RefineMode::ToScreamingSnakeCase => case_transform::to_screaming_snake_case(text),
        _ => unreachable!("{mode:?} は Case カテゴリではない"),
    }
}
