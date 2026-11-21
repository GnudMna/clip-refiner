//! 登録クリップのワーカー経路統合テスト

use clip_refiner::RefineMode;
use clip_refiner::test_helpers::ClipboardHarness;

// ======================================================================
// テスト
// ======================================================================

/// クリップボード内容を登録クリップとして保存できるようにすること
#[test]
fn register_clip_from_clipboard_saves_text() {
    let mut harness = ClipboardHarness::with_text("  saved-clip  ");

    harness.register_clip_from_clipboard();
    assert_eq!(harness.registered_clip_count(), 1);
    assert_eq!(
        harness.registered_clip_text(0).as_deref(),
        Some("  saved-clip  ")
    );
}

/// 登録クリップをコピーするとクリップボードへ反映されるようにすること
#[test]
fn copy_registered_clip_writes_to_clipboard() {
    let mut harness = ClipboardHarness::with_text("registered-value");

    harness.register_clip_from_clipboard();
    harness.reset_clipboard("other");
    harness.copy_registered_clip(0);

    assert_eq!(harness.clipboard_text(), "registered-value");
}

/// 登録クリップコピー直後の監視では再加工しないようにすること
#[test]
fn copy_registered_clip_avoids_immediate_reprocess() {
    let mut harness = ClipboardHarness::with_text("  clip  ").with_mode(RefineMode::Trim);

    harness.register_clip_from_clipboard();
    harness.reset_clipboard("placeholder");
    harness.copy_registered_clip(0);

    assert_eq!(harness.clipboard_text(), "  clip  ");
    assert!(!harness.run_monitor_update(false));
    assert_eq!(harness.clipboard_text(), "  clip  ");
}

/// 空クリップボードは登録されないようにすること
#[test]
fn register_clip_skips_empty_clipboard() {
    let mut harness = ClipboardHarness::with_text("   ");

    harness.register_clip_from_clipboard();
    assert_eq!(harness.registered_clip_count(), 0);
}
