//! ワンショット加工経路の統合テスト

use clip_refiner::test_helpers::{ClipboardHarness, InMemoryTextClipboard};
use clip_refiner_core::refiner::TextClipboard;
use clip_refiner_core::{
    AppConfig, ClipboardProcessOutcome, RefineContext, RefineMode, apply_pipeline_to_text,
};

fn run_oneshot_on_clipboard(
    clipboard: &mut InMemoryTextClipboard,
    pipeline: &[RefineMode],
    ctx: &RefineContext,
) -> ClipboardProcessOutcome {
    let input = clipboard.text().to_string();
    let outcome = apply_pipeline_to_text(&input, pipeline, ctx).expect("apply_pipeline_to_text");
    if let ClipboardProcessOutcome::Processed(result) = &outcome {
        clipboard.set_text(result.clone()).expect("clipboard write");
    }
    outcome
}

// ======================================================================
// テスト
// ======================================================================

/// 単一モードのワンショット相当処理がクリップボードを更新すること
#[test]
fn oneshot_pipeline_trims_clipboard_text() {
    let mut clipboard = InMemoryTextClipboard::with_text("  hello  ");
    let ctx = RefineContext::default();
    let pipeline = vec![RefineMode::Trim];

    let outcome = run_oneshot_on_clipboard(&mut clipboard, &pipeline, &ctx);

    assert!(matches!(outcome, ClipboardProcessOutcome::Processed(_)));
    assert_eq!(clipboard.text(), "hello");
}

/// 複数モードのパイプラインを順に適用できるようにすること
#[test]
fn oneshot_pipeline_applies_multiple_modes_in_order() {
    let mut clipboard = InMemoryTextClipboard::with_text("hello%20world");
    let ctx = RefineContext::default();
    let pipeline = vec![RefineMode::UrlDecode, RefineMode::Trim];

    let outcome = run_oneshot_on_clipboard(&mut clipboard, &pipeline, &ctx);

    assert!(matches!(outcome, ClipboardProcessOutcome::Processed(_)));
    assert_eq!(clipboard.text(), "hello world");
}

/// 変更がない場合は `Unchanged` を返すようにすること
#[test]
fn oneshot_pipeline_returns_unchanged_when_no_diff() {
    let mut clipboard = InMemoryTextClipboard::with_text("already-trimmed");
    let ctx = RefineContext::default();
    let pipeline = vec![RefineMode::Trim];

    let outcome = run_oneshot_on_clipboard(&mut clipboard, &pipeline, &ctx);

    assert!(matches!(outcome, ClipboardProcessOutcome::Unchanged));
    assert_eq!(clipboard.text(), "already-trimmed");
}

/// 設定の正規表現をコンテキストへ反映できるようにすること
#[test]
fn oneshot_context_uses_config_regex_settings() {
    let mut config = AppConfig::default();
    config.regex.pattern = "(world)".to_string();
    config.regex.replacement = "ClipRefiner".to_string();
    let ctx = RefineContext::from_config(&config);
    let mut clipboard = InMemoryTextClipboard::with_text("hello world");
    let pipeline = vec![RefineMode::RegexReplace];

    let outcome = run_oneshot_on_clipboard(&mut clipboard, &pipeline, &ctx);

    assert!(matches!(outcome, ClipboardProcessOutcome::Processed(_)));
    assert_eq!(clipboard.text(), "hello ClipRefiner");
}

/// 常駐委譲相当のワンショット加工後、監視がエコー再加工しないようにすること
#[test]
fn oneshot_via_worker_skips_monitor_write_back_echo() {
    let mut harness = ClipboardHarness::with_text("  hello  ").with_mode(RefineMode::UrlEncode);
    harness.process_oneshot_pipeline(vec![RefineMode::Trim], RefineContext::default());

    assert_eq!(harness.clipboard_text(), "hello");

    // ワンショット書き戻しのエコーを1回スキップし、常駐モードでは再加工しない
    assert!(!harness.run_monitor_update(true));
    assert_eq!(harness.clipboard_text(), "hello");
}
