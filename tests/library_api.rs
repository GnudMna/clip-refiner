//! 公開 API のスモークテスト

use clip_refiner::{
    AppConfig, CONFIG_VERSION, ClipboardProcessOutcome, RefineContext, RefineMode, Refiner,
    apply_pipeline_to_text, apply_text_pipeline, split_pipeline,
};

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
