//! 連続記録キューの統合テスト

use clip_refiner::RefineMode;
use clip_refiner::test_helpers::ClipboardHarness;

// ======================================================================
// テスト
// ======================================================================

/// 連続記録モードでは加工せず元テキストをキューへ積むようにすること
#[test]
fn continuous_record_stores_raw_text_without_processing() {
    let mut harness = ClipboardHarness::with_text("  raw-text  ")
        .with_mode(RefineMode::Trim)
        .with_continuous_record(true);

    assert!(!harness.run_monitor_update(false));
    assert_eq!(harness.clipboard_text(), "  raw-text  ");
    assert_eq!(harness.continuous_record_entries(), vec!["  raw-text  "]);
    assert_eq!(harness.history_len(), 0);
}

/// 連続記録モードでは同一テキストの再コピーごとにキュー末尾へ積ようにすること
#[test]
fn continuous_record_keeps_duplicate_entries_in_fifo_order() {
    let mut harness = ClipboardHarness::with_text("same")
        .with_mode(RefineMode::Trim)
        .with_continuous_record(true);

    assert!(!harness.run_monitor_update(true));
    assert!(!harness.run_monitor_update(true));
    assert_eq!(harness.continuous_record_entries(), vec!["same", "same"]);
}

/// 連続記録 ON でも手動加工は動作すること
#[test]
fn continuous_record_allows_manual_processing() {
    let mut harness = ClipboardHarness::with_text("  manual  ")
        .with_mode(RefineMode::Trim)
        .with_continuous_record(true);

    harness.process_mode(RefineMode::Trim);
    assert_eq!(harness.clipboard_text(), "manual");
}

/// キュー先頭取り出し直後は連続記録モードでも誤記録しないようにすること
#[test]
fn continuous_record_skips_immediate_restore_echo() {
    let mut harness = ClipboardHarness::with_text("first")
        .with_mode(RefineMode::Trim)
        .with_continuous_record(true);

    assert!(!harness.run_monitor_update(false));
    assert_eq!(harness.continuous_record_len(), 1);

    harness.discharge_continuous_record_on_paste();
    assert_eq!(harness.clipboard_text(), "first");
    assert!(!harness.run_monitor_update(false));
    assert_eq!(harness.continuous_record_len(), 0);
}

/// `Alt+Shift+X` 相当でキュー先頭をクリップボードへ載せ、FIFO 順に取り出すようにすること
#[test]
fn continuous_record_paste_discharges_front_on_shortcut() {
    let mut harness = ClipboardHarness::with_text("first")
        .with_mode(RefineMode::Trim)
        .with_continuous_record(true);

    assert!(!harness.run_monitor_update(false));
    harness.replace_clipboard("second");
    assert!(!harness.run_monitor_update(false));
    assert_eq!(harness.continuous_record_entries(), vec!["first", "second"]);

    harness.discharge_continuous_record_on_paste();
    assert_eq!(harness.clipboard_text(), "first");
    assert_eq!(harness.continuous_record_entries(), vec!["second"]);
}

/// `Alt+Shift+X` 相当はキュー先頭を取り出し、一致検索ではないようにすること
#[test]
fn continuous_record_paste_discharges_front_not_matching_clipboard() {
    let mut harness = ClipboardHarness::with_text("alpha")
        .with_mode(RefineMode::Trim)
        .with_continuous_record(true);

    assert!(!harness.run_monitor_update(false));
    harness.replace_clipboard("beta");
    assert!(!harness.run_monitor_update(false));
    harness.replace_clipboard("gamma");
    assert!(!harness.run_monitor_update(false));
    assert_eq!(
        harness.continuous_record_entries(),
        vec!["alpha", "beta", "gamma"]
    );

    harness.discharge_continuous_record_on_paste();
    assert_eq!(harness.clipboard_text(), "alpha");
    assert_eq!(harness.continuous_record_entries(), vec!["beta", "gamma"]);
}

/// キューが空のときは `Alt+Shift+X` 相当でクリップボードを変更しないようにすること
#[test]
fn continuous_record_paste_skips_when_queue_empty() {
    let mut harness = ClipboardHarness::with_text("keep-me")
        .with_mode(RefineMode::Trim)
        .with_continuous_record(true);

    harness.discharge_continuous_record_on_paste();
    assert_eq!(harness.clipboard_text(), "keep-me");
    assert_eq!(harness.continuous_record_len(), 0);
}

/// メニュー選択相当で指定位置の項目を取り出ようにすること
#[test]
fn continuous_record_menu_discharge_removes_entry() {
    let mut harness = ClipboardHarness::with_text("first")
        .with_mode(RefineMode::Trim)
        .with_continuous_record(true);

    assert!(!harness.run_monitor_update(false));
    harness.replace_clipboard("second");
    assert!(!harness.run_monitor_update(false));

    harness.discharge_continuous_record_at(1);
    assert_eq!(harness.clipboard_text(), "second");
    assert_eq!(harness.continuous_record_entries(), vec!["first"]);
}

/// 連続記録の直近 1 件取り消しが末尾から除去すること
#[test]
fn continuous_record_undo_removes_last_entry() {
    let mut harness = ClipboardHarness::with_text("first")
        .with_mode(RefineMode::Trim)
        .with_continuous_record(true);

    assert!(!harness.run_monitor_update(false));
    harness.replace_clipboard("second");
    assert!(!harness.run_monitor_update(false));
    harness.replace_clipboard("third");
    assert!(!harness.run_monitor_update(false));

    assert!(harness.undo_last_continuous_record());
    assert_eq!(harness.continuous_record_entries(), vec!["first", "second"]);
    assert_eq!(harness.clipboard_text(), "third");
}

/// 連続記録無効時は監視でもキューへ積まないようにすること
#[test]
fn monitor_skips_continuous_record_when_disabled() {
    let mut harness = ClipboardHarness::with_text("  x  ")
        .with_mode(RefineMode::Trim)
        .with_continuous_record(false);

    assert!(harness.run_monitor_update(false));
    assert_eq!(harness.continuous_record_len(), 0);
}

/// 連続記録 ON 時は既にクリップボードにあるテキストを記録しないこと
#[test]
fn continuous_record_enable_skips_existing_clipboard() {
    let mut harness = ClipboardHarness::with_text("already-on-clipboard")
        .with_mode(RefineMode::Trim)
        .with_continuous_record(false);

    assert!(!harness.run_monitor_update(false));
    assert_eq!(harness.continuous_record_len(), 0);

    harness.with_config_mut(|c| c.continuous_record_enabled = true);
    assert_eq!(harness.continuous_record_len(), 0);

    harness.replace_clipboard("new-copy");
    assert!(!harness.run_monitor_update(true));
    assert_eq!(harness.continuous_record_entries(), vec!["new-copy"]);
}

/// 履歴と連続記録は独立して動作すること
#[test]
fn history_and_continuous_record_are_independent() {
    let mut harness = ClipboardHarness::with_text("  raw  ")
        .with_mode(RefineMode::Trim)
        .with_continuous_record(true);

    assert!(!harness.run_monitor_update(false));
    assert_eq!(harness.continuous_record_entries(), vec!["  raw  "]);
    assert_eq!(harness.history_len(), 0);

    harness.with_config_mut(|c| {
        c.continuous_record_enabled = false;
        c.history_enabled = true;
    });
    harness.replace_clipboard("  other  ");
    assert!(harness.run_monitor_update(false));
    assert_eq!(harness.history_entries(), vec!["other"]);
    assert_eq!(harness.continuous_record_len(), 1);
}
