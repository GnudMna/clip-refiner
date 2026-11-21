//! 全 `RefineMode` に任意の文字列を与え、パニックしないことを検証する

#![no_main]

use clip_refiner_core::{RefineContext, RefineMode, Refiner, RegexSettings};
use clip_refiner_fuzz::fuzz_text;
use libfuzzer_sys::fuzz_target;
use strum::IntoEnumIterator;

fuzz_target!(|data: &[u8]| {
    fuzz_text(data, |text| {
        let contexts = [
            RefineContext::default(),
            RefineContext::with_regex(RegexSettings::single(r"(\w+)\s*", "[$1]")),
        ];
        for ctx in &contexts {
            for mode in RefineMode::iter() {
                let _ = mode.refine(text, ctx);
            }
        }
    });
});
