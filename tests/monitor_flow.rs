//! 監視ループ経路の統合テスト

use clip_refiner::RefineMode;
use clip_refiner::test_helpers::ClipboardHarness;

// ======================================================================
// 監視加工
// ======================================================================
/// 監視加工後に Undo で加工前テキストへ戻せること
#[test]
fn monitor_process_then_undo_restores_original() {
    let mut harness = ClipboardHarness::with_text("  hello  ").with_mode(RefineMode::Trim);

    assert!(harness.run_monitor_update(false));
    assert_eq!(harness.clipboard_text(), "hello");

    harness.undo();
    assert_eq!(harness.clipboard_text(), "  hello  ");
}

/// ポーリング時に同一テキストの再観測では再加工しないこと
#[test]
fn polling_does_not_reprocess_unchanged_clipboard() {
    let mut harness = ClipboardHarness::with_text("  hello  ").with_mode(RefineMode::Trim);

    assert!(harness.run_monitor_update(false));
    assert_eq!(harness.clipboard_text(), "hello");

    harness.reset_clipboard("hello");
    assert!(!harness.run_monitor_update(false));
    assert_eq!(harness.clipboard_text(), "hello");
}

/// 自身の書き戻し直後のエコーイベントを1回スキップすること
#[test]
fn polling_skips_own_write_back_echo_once() {
    let mut harness = ClipboardHarness::with_text("  hello  ").with_mode(RefineMode::Trim);

    assert!(harness.run_monitor_update(false));
    assert_eq!(harness.clipboard_text(), "hello");

    assert!(!harness.run_monitor_update(true));
    assert_eq!(harness.clipboard_text(), "hello");

    harness.replace_clipboard("  hello  ");
    assert!(harness.run_monitor_update(true));
    assert_eq!(harness.clipboard_text(), "hello");
}

/// 設定の加工モード変更が次の監視加工に反映されること
#[test]
fn monitor_applies_updated_mode_from_snapshot() {
    let mut harness = ClipboardHarness::with_text("a%2Fb").with_mode(RefineMode::UrlDecode);

    assert!(harness.run_monitor_update(false));
    assert_eq!(harness.clipboard_text(), "a/b");

    harness.set_mode(RefineMode::UrlEncode);
    harness.replace_clipboard("c d");

    assert!(harness.run_monitor_update(false));
    assert_eq!(harness.clipboard_text(), "c%20d");
}

/// 上限超過テキストは監視加工をスキップしクリップボードを変更しないこと
#[test]
fn monitor_skips_oversized_clipboard_without_modifying() {
    use clip_refiner::test_helpers::MAX_CLIPBOARD_TEXT_BYTES;

    let oversized = "a".repeat(MAX_CLIPBOARD_TEXT_BYTES + 1);
    let mut harness = ClipboardHarness::with_text(oversized.clone()).with_mode(RefineMode::Trim);

    assert!(!harness.run_monitor_update(false));
    assert_eq!(harness.clipboard_text(), oversized);
    assert_eq!(harness.history_len(), 0);
}
