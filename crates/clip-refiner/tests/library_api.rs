//! 公開 API の統合テスト (`clip-refiner` re-export と core 連携)

use clip_refiner::{
    AppConfig, CONFIG_VERSION, ClipboardProcessError, ClipboardProcessOutcome, RefineContext,
    RefineMode, Refiner, apply_pipeline_to_text, apply_text_pipeline, is_within_clipboard_limit,
    split_pipeline,
};
use clip_refiner_core::consts::MAX_CLIPBOARD_TEXT_BYTES;

/// クレートルートから加工パイプライン API を呼び出せること
#[test]
fn crate_root_pipeline_api() {
    let ctx = RefineContext::default();
    let pipeline = vec![RefineMode::UrlDecode, RefineMode::Trim];

    let (text_modes, image_tail) = split_pipeline(&pipeline);
    assert_eq!(text_modes, pipeline);
    assert_eq!(image_tail, None);

    assert_eq!(
        apply_text_pipeline("  %E3%81%82  ", &pipeline, &ctx),
        Some("あ".to_string())
    );

    assert_eq!(
        apply_pipeline_to_text("  hello  ", &[RefineMode::Trim], &ctx),
        Ok(ClipboardProcessOutcome::Processed("hello".to_string()))
    );
}

/// 設定型と `RefineContext::from_config` が連携すること
#[test]
fn config_drives_refine_context() {
    let mut config = AppConfig::default();
    config.regex.pattern = r"\s+".to_string();
    config.regex.replacement = "-".to_string();

    let ctx = RefineContext::from_config(&config);
    assert_eq!(RefineMode::RegexReplace.refine("a   b", &ctx), "a-b");
    assert!(!config.effective_pipeline().is_empty());
    assert_eq!(CONFIG_VERSION, config.version);
}

/// re-export された型が core 経由と同じ結果を返すこと
#[test]
fn reexports_match_core_types() {
    let input = "a%2Fb";
    let facade = RefineMode::UrlDecode.refine(input, &RefineContext::default());
    let core = clip_refiner_core::RefineMode::UrlDecode
        .refine(input, &clip_refiner_core::RefineContext::default());
    assert_eq!(facade, core);
}

/// 空入力は `NoText` エラーになること
#[test]
fn apply_pipeline_rejects_empty_via_reexport() {
    assert_eq!(
        apply_pipeline_to_text("", &[RefineMode::Trim], &RefineContext::default()),
        Err(ClipboardProcessError::NoText)
    );
}

/// 上限超過は `TextTooLarge` エラーになること
#[test]
fn apply_pipeline_rejects_oversized_via_reexport() {
    let oversized = "a".repeat(MAX_CLIPBOARD_TEXT_BYTES + 1);
    assert!(!is_within_clipboard_limit(&oversized));
    assert_eq!(
        apply_pipeline_to_text(&oversized, &[RefineMode::Trim], &RefineContext::default()),
        Err(ClipboardProcessError::TextTooLarge)
    );
}

/// `ClipboardProcessError` のユーザー向けメッセージが re-export 経由で参照できること
#[test]
fn clipboard_error_user_messages() {
    assert_eq!(
        ClipboardProcessError::NoText.user_message(),
        "クリップボードにテキストがありません"
    );
    assert_eq!(
        ClipboardProcessError::TextTooLarge.user_message(),
        "クリップボードのテキストが大きすぎます"
    );
}
