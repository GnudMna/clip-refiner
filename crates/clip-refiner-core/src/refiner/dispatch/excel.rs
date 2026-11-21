use std::borrow::Cow;

use super::super::context::RefineContext;
use super::super::mode::RefineMode;
use super::super::transform::markdown;

/// Excel カテゴリの加工を実行する
pub(crate) fn refine<'a>(mode: RefineMode, text: &'a str, _ctx: &RefineContext) -> Cow<'a, str> {
    match mode {
        RefineMode::ExcelToMarkdown => markdown::excel_to_markdown_table(text),
        RefineMode::MarkdownToExcel => markdown::markdown_table_to_excel(text),
        // 画像出力は `process_image_clipboard` で処理する
        RefineMode::ExcelToImage => Cow::Borrowed(text),
        _ => unreachable!("{mode:?} は Excel カテゴリではない"),
    }
}
