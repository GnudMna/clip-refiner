//! 加工変換のプロパティベーステスト

use clip_refiner_core::{AppConfig, RefineContext, RefineMode, Refiner};

use proptest::prelude::*;

proptest! {
    /// URL エンコード後にデコードすると元の文字列に戻ること
    #[test]
    fn url_encode_decode_roundtrip(input in "\\PC*") {
        let ctx = RefineContext::default();
        let encoded = RefineMode::UrlEncode.refine(&input, &ctx);
        let decoded = RefineMode::UrlDecode.refine(&encoded, &ctx);
        prop_assert_eq!(&decoded, &input);
    }

    /// JSON エスケープ後にアンエスケープすると元の文字列に戻ること
    #[test]
    fn escape_unescape_roundtrip(input in "[\\x20-\\x7E]*") {
        let ctx = RefineContext::default();
        let escaped = RefineMode::Escape.refine(&input, &ctx);
        let unescaped = RefineMode::Unescape.refine(&escaped, &ctx);
        prop_assert_eq!(&unescaped, &input);
    }

    /// 正規表現エスケープ後にアンエスケープすると元の文字列に戻ること
    #[test]
    fn regex_escape_unescape_roundtrip(input in "[A-Za-z0-9_]*") {
        let ctx = RefineContext::default();
        let escaped = RefineMode::RegexEscape.refine(&input, &ctx);
        let unescaped = RefineMode::RegexUnescape.refine(&escaped, &ctx);
        prop_assert_eq!(&unescaped, &input);
    }

    /// 数字のみの文字列は正規表現置換で一貫して変換されること
    #[test]
    fn regex_replace_digits_are_stable(input in "[0-9]{0,32}") {
        let mut config = AppConfig::default();
        config.regex.pattern = r"\d".to_string();
        config.regex.replacement = "X".to_string();
        let ctx = RefineContext::from_config(&config);
        let once = RefineMode::RegexReplace.refine(&input, &ctx);
        let twice = RefineMode::RegexReplace.refine(&once, &ctx);
        prop_assert_eq!(&once, &twice);
    }
}
