//! ワーカーコマンド経路の統合テスト

use clip_refiner::test_helpers::{ClipboardHarness, MonitorMode};
use clip_refiner_core::RefineMode;

// ======================================================================
// テスト
// ======================================================================

/// 除外形式付きのコピーを手動加工しても、除外形式を維持すること
#[test]
fn worker_process_mode_keeps_exclusion() {
    let mut harness = ClipboardHarness::with_text("  secret  ").with_excluded_clipboard();

    harness.process_mode(RefineMode::Trim);
    assert_eq!(harness.clipboard_text(), "secret");
    assert!(harness.clipboard_is_excluded());

    // 取り消しでも維持する
    harness.undo();
    assert!(harness.clipboard_is_excluded());
}

/// 除外形式の維持を無効にすると、手動加工で除外形式が消えること
#[test]
fn worker_process_mode_drops_exclusion_when_disabled() {
    let mut harness = ClipboardHarness::with_text("  secret  ").with_excluded_clipboard();
    harness.with_config_mut(|c| c.keep_clipboard_exclusion = false);

    harness.process_mode(RefineMode::Trim);
    assert!(!harness.clipboard_is_excluded());
}

/// 除外形式の無いコピーの手動加工では、除外形式を付けないこと
#[test]
fn worker_process_mode_does_not_add_exclusion() {
    let mut harness = ClipboardHarness::with_text("  plain  ");

    harness.process_mode(RefineMode::Trim);
    assert!(!harness.clipboard_is_excluded());
}

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

/// 加工後に別の内容がコピーされた場合、`Undo` はその内容を上書きしないこと
///
/// 取り消し用の記録は破棄され、以降の `Undo` も何も書き戻さない
#[test]
fn worker_undo_does_not_overwrite_later_copy() {
    let mut harness = ClipboardHarness::with_text("  before  ");

    harness.process_mode(RefineMode::Trim);
    assert_eq!(harness.clipboard_text(), "before");

    // 加工後に別アプリで新しい内容をコピーした状況
    harness.set_text("copied-later");
    harness.undo();
    assert_eq!(harness.clipboard_text(), "copied-later");

    harness.undo();
    assert_eq!(harness.clipboard_text(), "copied-later");
    assert!(harness.take_undo_source().is_none());
}

/// 履歴復元 (`SetText`) 直後のポーリングでは再加工しないようにすること
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

/// 変更のない手動加工ではクリップボードを変更しないようにすること
#[test]
fn worker_process_mode_leaves_unchanged_text() {
    let mut harness = ClipboardHarness::with_text("already-trimmed");

    harness.process_mode(RefineMode::Trim);
    assert_eq!(harness.clipboard_text(), "already-trimmed");
}

/// 手動加工の読み取り直後に別のコピーが入った場合、上書きも取り消し元の記録もしないこと
#[test]
fn worker_process_mode_skips_write_when_clipboard_changes_during_read() {
    let mut harness =
        ClipboardHarness::with_text("  original  ").with_external_copy_after_read("  external  ");

    harness.process_mode(RefineMode::Trim);
    assert_eq!(harness.clipboard_text(), "  external  ");
    assert!(harness.take_undo_source().is_none());
}

/// 取り消し対象がないとき `Undo` はクリップボードを変更しないようにすること
#[test]
fn worker_undo_without_source_is_no_op() {
    let mut harness = ClipboardHarness::with_text("  keep  ");

    harness.undo();
    assert_eq!(harness.clipboard_text(), "  keep  ");
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
