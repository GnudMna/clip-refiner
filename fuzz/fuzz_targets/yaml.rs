//! YAML→JSON 変換に任意の文字列を与え、パニックしないことと出力が JSON であることを検証する

#![no_main]

use std::borrow::Cow;

use clip_refiner_core::{RefineContext, RefineMode, Refiner};
use clip_refiner_fuzz::fuzz_text;
use libfuzzer_sys::fuzz_target;
use serde_json::Value;

fuzz_target!(|data: &[u8]| {
    fuzz_text(data, |text| {
        let ctx = RefineContext::default();
        for mode in [RefineMode::YamlToJson, RefineMode::YamlToJsonPreserveOrder] {
            if let Cow::Owned(json) = mode.refine(text, &ctx) {
                assert!(
                    serde_json::from_str::<Value>(&json).is_ok(),
                    "変換結果が JSON でない: mode={mode:?}\ninput={text:?}\noutput={json:?}"
                );
            }
        }
    });
});
