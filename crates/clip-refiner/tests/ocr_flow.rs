//! OCR 結果のワーカー経路統合テスト

use clip_refiner::test_helpers::ClipboardHarness;

/// `SetOcrText` でクリップボードへ OCR 結果が書き込まれること
#[test]
fn ocr_set_text_writes_clipboard() {
    let mut harness = ClipboardHarness::with_text("before-ocr");

    harness.set_ocr_text("recognized text");
    assert_eq!(harness.clipboard_text(), "recognized text");
}

/// OCR 結果書き込み直後のポーリングでは再加工しないこと
#[test]
fn ocr_set_text_avoids_immediate_reprocess() {
    let mut harness = ClipboardHarness::with_text("  hello  ");

    harness.set_ocr_text("  hello  ");
    assert!(!harness.run_monitor_update(false));
    assert_eq!(harness.clipboard_text(), "  hello  ");
}

/// OCR 結果は加工書き戻しではなく外部設定として記録されること
#[test]
fn ocr_set_text_does_not_mark_write_back() {
    let mut harness = ClipboardHarness::with_text("old");

    harness.set_ocr_text("ocr-output");
    assert!(!harness.matches_last_written("ocr-output"));
}
