use strum::IntoEnumIterator;

use super::super::transform::datetime;
use super::test_cases::{FIXED_CASES, SPECIAL_MODE_COUNT};
use super::*;
use crate::config::RegexSettings;

fn regex_ctx(pattern: &str, replacement: &str) -> RefineContext {
    let mut ctx = RefineContext::default();
    ctx.regex = RegexSettings::single(pattern, replacement);
    ctx
}

fn verify_datetime_modes(empty_ctx: &RefineContext) {
    let timestamp_mode = RefineMode::ToDatetimeString;
    let input = "1672531200";
    let actual = timestamp_mode.refine(input, empty_ctx);
    let expected = datetime::to_datetime_string(input);
    assert_eq!(actual, expected);
    assert_ne!(actual.as_ref(), input);

    let datetime_input = datetime::to_datetime_string("1672531200");
    let actual = RefineMode::ToUnixTimestamp.refine(&datetime_input, empty_ctx);
    assert_eq!(actual, "1672531200");

    // ローカルタイムゾーンの出力形式は、日時文字列→タイムスタンプで元の値に戻る
    for mode in [
        RefineMode::ToIso8601,
        RefineMode::ToRfc2822,
        RefineMode::ToJapaneseDatetime,
    ] {
        let formatted = mode.refine(input, empty_ctx);
        assert_ne!(formatted.as_ref(), input, "mode={mode:?}");
        let roundtrip = RefineMode::ToUnixTimestamp.refine(&formatted, empty_ctx);
        assert_eq!(roundtrip, input, "mode={mode:?}");
    }
}

fn verify_regex_modes() {
    let replace_ctx = regex_ctx(r"\d", "X");
    assert_eq!(
        RefineMode::RegexReplace.refine("a1b2", &replace_ctx),
        "aXbX"
    );

    let extract_ctx = regex_ctx(r"\d+", "");
    assert_eq!(
        RefineMode::RegexExtract.refine("a1b22", &extract_ctx),
        "1\n22"
    );

    let delete_ctx = regex_ctx(r"\d", "");
    assert_eq!(RefineMode::RegexDelete.refine("a1b2", &delete_ctx), "ab");

    let split_ctx = regex_ctx(",", "");
    assert_eq!(
        RefineMode::RegexSplit.refine("a,b,c", &split_ctx),
        "a\nb\nc"
    );
}

fn verify_fixed_cases(empty_ctx: &RefineContext) {
    for (mode, input, expected) in FIXED_CASES {
        let actual = mode.refine(input, empty_ctx);
        assert_eq!(
            actual, *expected,
            "Failed at mode: {mode:?}\nInput: {input}\nExpected: {expected}\nActual: {actual}"
        );
    }
}

// ======================================================================
// テスト
// ======================================================================

/// 全 `RefineMode` バリアントを網羅してテーブル駆動で検証すること
///
/// 各モードが正しく配線され、期待通りの加工を行う
#[test]
fn test_all_refine_modes() {
    assert_eq!(
        FIXED_CASES.len() + SPECIAL_MODE_COUNT,
        RefineMode::iter().count(),
        "固定ケースと日時・正規表現・画像モードの合計が RefineMode バリアント数と一致しません"
    );

    let empty_ctx = RefineContext::default();
    verify_datetime_modes(&empty_ctx);
    verify_regex_modes();
    verify_fixed_cases(&empty_ctx);
}
