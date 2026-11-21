//! 行の並べ替え (CSV レコード単位の並べ替えを含む) を検証する
//!
//! パニックしないことに加え、行数と行の集合が変わらないことを確かめる

#![no_main]

use std::collections::BTreeMap;

use clip_refiner_core::{RefineContext, RefineMode, Refiner};
use clip_refiner_fuzz::fuzz_text;
use libfuzzer_sys::fuzz_target;

/// 行の出現回数 (行の多重集合)
fn line_counts(text: &str) -> BTreeMap<&str, usize> {
    let mut counts = BTreeMap::new();
    for line in text.lines() {
        *counts.entry(line).or_insert(0) += 1;
    }
    counts
}

fuzz_target!(|data: &[u8]| {
    fuzz_text(data, |text| {
        let ctx = RefineContext::default();
        // 改行コードが混在する入力と、末尾が単独の CR の入力は出力の改行コードがそろうため行の比較から除く
        let mixed_line_endings = text.contains("\r\n") && text.replace("\r\n", "").contains('\n');
        let comparable = !mixed_line_endings && !text.ends_with('\r');
        for mode in [RefineMode::SortLinesAsc, RefineMode::SortLinesDesc] {
            let output = mode.refine(text, &ctx);
            if comparable {
                assert_eq!(
                    line_counts(&output),
                    line_counts(text),
                    "mode={mode:?}\ninput={text:?}\noutput={output:?}"
                );
            }
        }
    });
});
