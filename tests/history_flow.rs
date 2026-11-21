//! 暗号化履歴の統合テスト

use clip_refiner::RefineMode;
use clip_refiner::test_helpers::ClipboardHarness;

/// 加工成功時に履歴へ結果が記録され、重複は先頭へ移動すること
#[test]
fn monitor_records_history_and_moves_duplicates_to_front() {
    let mut harness = ClipboardHarness::with_text("  alpha  ")
        .with_mode(RefineMode::Trim)
        .with_history(true);

    assert!(harness.run_monitor_update(false));
    assert_eq!(harness.history_entries(), vec!["alpha"]);

    harness.replace_clipboard("  beta  ");
    assert!(harness.run_monitor_update(false));
    assert_eq!(harness.history_entries(), vec!["beta", "alpha"]);

    harness.replace_clipboard("  alpha  ");
    assert!(harness.run_monitor_update(false));
    assert_eq!(harness.history_entries(), vec!["alpha", "beta"]);
}

/// 履歴無効時は監視加工でも履歴へ追加しないこと
#[test]
fn monitor_skips_history_when_disabled() {
    let mut harness = ClipboardHarness::with_text("  x  ")
        .with_mode(RefineMode::Trim)
        .with_history(false);

    assert!(harness.run_monitor_update(false));
    assert_eq!(harness.history_len(), 0);
}

/// 履歴の暗号化保存と復号取得が加工フロー経由で機能すること
#[test]
fn encrypted_history_round_trip_through_monitor_flow() {
    let mut harness = ClipboardHarness::with_text("  secret-value  ")
        .with_mode(RefineMode::Trim)
        .with_history(true);

    assert!(harness.run_monitor_update(false));
    assert_eq!(harness.history_entries(), vec!["secret-value"]);

    let restored = harness
        .history_entry_text(0)
        .expect("履歴エントリの取得に失敗");
    assert_eq!(restored, "secret-value");

    harness.set_text(&restored);
    assert_eq!(harness.clipboard_text(), "secret-value");
}
