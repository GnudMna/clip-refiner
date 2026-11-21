use std::borrow::Cow;

use super::context::RefineContext;
use super::mode::{RefineCategory, RefineMode};

mod case_convert;
mod datetime;
mod escape;
mod excel;
mod json_format;
mod line_actions;
mod markdown;
mod number;
mod path;
mod regex_modes;
mod to_json;
mod to_yaml;
mod trim;
mod url;
mod width;

// ======================================================================
// 加工インターフェース
// ======================================================================
/// クリップボードのテキストを加工するための共通インターフェース
pub trait Refiner {
    /// テキストを加工する
    ///
    /// # Arguments
    /// * `text` - 加工前のテキスト
    /// * `ctx` - 設定依存の加工パラメータ
    ///
    /// # Returns
    /// * `Cow<'a, str>` - 加工後のテキスト(変更がない場合は元のテキストを借用)
    fn refine<'a>(&self, text: &'a str, ctx: &RefineContext) -> Cow<'a, str>;
}

impl Refiner for RefineMode {
    fn refine<'a>(&self, text: &'a str, ctx: &RefineContext) -> Cow<'a, str> {
        refine_by_category(*self, text, ctx)
    }
}

/// `RefineMode` のカテゴリに応じて対応サブモジュールへディスパッチする
///
/// 新カテゴリ追加時はこことカテゴリ用 `refine` 実装の両方がコンパイルを要求する
fn refine_by_category<'a>(mode: RefineMode, text: &'a str, ctx: &RefineContext) -> Cow<'a, str> {
    match mode.category() {
        RefineCategory::UrlActions => url::refine(mode, text, ctx),
        RefineCategory::Path => path::refine(mode, text, ctx),
        RefineCategory::LineActions => line_actions::refine(mode, text, ctx),
        RefineCategory::Trim => trim::refine(mode, text, ctx),
        RefineCategory::Escape => escape::refine(mode, text, ctx),
        RefineCategory::Regex => regex_modes::refine(mode, text, ctx),
        RefineCategory::JsonFormat => json_format::refine(mode, text, ctx),
        RefineCategory::ToJson => to_json::refine(mode, text, ctx),
        RefineCategory::ToYaml => to_yaml::refine(mode, text, ctx),
        RefineCategory::Markdown => markdown::refine(mode, text, ctx),
        RefineCategory::Excel => excel::refine(mode, text, ctx),
        RefineCategory::Datetime => datetime::refine(mode, text, ctx),
        RefineCategory::Number => number::refine(mode, text, ctx),
        RefineCategory::Case => case_convert::refine(mode, text, ctx),
        RefineCategory::Width => width::refine(mode, text, ctx),
        RefineCategory::Normal => unreachable!("{mode:?} は Normal カテゴリに属さない"),
    }
}

#[cfg(test)]
mod test_cases;
#[cfg(test)]
mod tests;
