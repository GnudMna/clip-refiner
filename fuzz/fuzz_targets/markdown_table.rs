//! Markdown 表 ↔ Excel (TSV) 変換に任意の文字列を与え、パニックしないことを検証する

#![no_main]

use clip_refiner_core::{RefineContext, RefineMode, Refiner};
use clip_refiner_fuzz::fuzz_text;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    fuzz_text(data, |text| {
        let ctx = RefineContext::default();
        // 変換結果をもう一方の変換へ渡し、互いの出力を入力にしたときの経路も通す
        let markdown = RefineMode::ExcelToMarkdown.refine(text, &ctx);
        let _ = RefineMode::MarkdownToExcel.refine(&markdown, &ctx);
        let tsv = RefineMode::MarkdownToExcel.refine(text, &ctx);
        let _ = RefineMode::ExcelToMarkdown.refine(&tsv, &ctx);
    });
});
