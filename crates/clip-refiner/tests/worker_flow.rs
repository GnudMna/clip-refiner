//! ワーカーコマンド経路の統合テスト

use clip_refiner::RefineMode;
use clip_refiner::test_helpers::{ClipboardHarness, MonitorMode};

/// `ProcessMode` → `Undo` でクリップボードと取り消し状態が往復すること
#[test]
fn worker_process_mode_then_undo_round_trip() {
    let mut harness = ClipboardHarness::with_text("  round-trip  ");

    harness.process_mode(RefineMode::Trim);
    assert_eq!(harness.clipboard_text(), "round-trip");
    assert!(harness.matches_last_written("round-trip"));

    harness.undo();
    assert_eq!(harness.clipboard_text(), "  round-trip  ");
    assert!(harness.take_undo_source().is_none());
}

/// 履歴復元 (`SetText`) 直後のポーリングでは再加工しないこと
#[test]
fn history_restore_via_set_text_avoids_immediate_reprocess() {
    let mut harness = ClipboardHarness::with_text("  first  ")
        .with_mode(RefineMode::Trim)
        .with_history(true);

    assert!(harness.run_monitor_update(false));
    assert_eq!(harness.clipboard_text(), "first");
    assert_eq!(harness.history_entries(), vec!["first"]);

    harness.set_text("restored-from-history");
    assert_eq!(harness.clipboard_text(), "restored-from-history");
    assert!(!harness.run_monitor_update(false));
    assert_eq!(harness.clipboard_text(), "restored-from-history");
}

/// Event 監視方式でも履歴復元直後の再加工を抑制すること
#[test]
fn event_mode_history_restore_avoids_immediate_reprocess() {
    let mut harness = ClipboardHarness::with_text("  first  ")
        .with_mode(RefineMode::Trim)
        .with_history(true)
        .with_monitor_mode(MonitorMode::Event);

    assert!(harness.run_configured_monitor_update());
    assert_eq!(harness.clipboard_text(), "first");

    harness.set_text("restored-from-history");
    assert_eq!(harness.clipboard_text(), "restored-from-history");
    assert!(!harness.run_configured_monitor_update());
    assert_eq!(harness.clipboard_text(), "restored-from-history");
}
