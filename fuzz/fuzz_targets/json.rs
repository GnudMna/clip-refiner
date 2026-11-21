//! JSON 整形・JSON→YAML 変換を検証する
//!
//! パニックしないことに加え、次を確かめる
//! - JSON 整形は値を変えない (数値の表記も保つ)
//! - JSON→YAML で変換できた結果は YAML→JSON で読み戻せる

#![no_main]

use std::borrow::Cow;

use clip_refiner_core::{RefineContext, RefineMode, Refiner};
use clip_refiner_fuzz::fuzz_text;
use libfuzzer_sys::fuzz_target;
use serde_json::Value;

fuzz_target!(|data: &[u8]| {
    fuzz_text(data, |text| {
        let ctx = RefineContext::default();
        let parsed = serde_json::from_str::<Value>(text).ok();

        for mode in [RefineMode::JsonFormat, RefineMode::JsonFormatPreserveOrder] {
            let formatted = mode.refine(text, &ctx);
            if let Some(value) = &parsed {
                let reparsed: Value = serde_json::from_str(&formatted).expect("整形結果は JSON");
                assert_eq!(&reparsed, value, "mode={mode:?}\ninput={text:?}");
            }
        }

        for (to_yaml, to_json) in [
            (RefineMode::JsonToYaml, RefineMode::YamlToJson),
            (
                RefineMode::JsonToYamlPreserveOrder,
                RefineMode::YamlToJsonPreserveOrder,
            ),
        ] {
            let yaml = to_yaml.refine(text, &ctx);
            if matches!(yaml, Cow::Owned(_)) {
                let back = to_json.refine(&yaml, &ctx);
                assert!(
                    matches!(back, Cow::Owned(_)),
                    "YAML を JSON へ戻せない: mode={to_yaml:?}\ninput={text:?}\nyaml={yaml:?}"
                );
            }
        }
    });
});
