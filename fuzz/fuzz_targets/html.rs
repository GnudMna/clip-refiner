//! HTML→Markdown・Markdown→HTML 変換に任意の文字列を与え、パニックしないことを検証する
//!
//! 深い入れ子によるスタックあふれも検出対象とする

#![no_main]

use clip_refiner_core::{RefineContext, RefineMode, Refiner};
use clip_refiner_fuzz::fuzz_text;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    fuzz_text(data, |text| {
        let ctx = RefineContext::default();
        // 変換結果を逆方向の変換へ渡し、互いの出力を入力にしたときの経路も通す
        let markdown = RefineMode::HtmlToMarkdown.refine(text, &ctx);
        let _ = RefineMode::MarkdownToHtml.refine(&markdown, &ctx);
        let html = RefineMode::MarkdownToHtml.refine(text, &ctx);
        let _ = RefineMode::HtmlToMarkdown.refine(&html, &ctx);
    });
});
