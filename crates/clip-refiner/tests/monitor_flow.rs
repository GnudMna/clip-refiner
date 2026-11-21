//! 監視ループ経路の統合テスト

use clip_refiner::RefineMode;
use clip_refiner::test_helpers::{ClipboardHarness, MonitorMode};

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

// ======================================================================
// Event 監視方式
// ======================================================================
/// Event 監視方式でも加工が実行されること
#[test]
fn event_mode_processes_clipboard() {
    let mut harness = ClipboardHarness::with_text("  hello  ")
        .with_mode(RefineMode::Trim)
        .with_monitor_mode(MonitorMode::Event);

    assert!(harness.run_configured_monitor_update());
    assert_eq!(harness.clipboard_text(), "hello");
}

/// Event 監視方式でも Undo で加工前テキストへ戻せること
#[test]
fn event_mode_process_then_undo_restores_original() {
    let mut harness = ClipboardHarness::with_text("  undo-me  ")
        .with_mode(RefineMode::Trim)
        .with_monitor_mode(MonitorMode::Event);

    assert!(harness.run_configured_monitor_update());
    assert_eq!(harness.clipboard_text(), "undo-me");

    harness.undo();
    assert_eq!(harness.clipboard_text(), "  undo-me  ");
}

/// Event 監視時は元テキストの再コピーでも再加工されること
#[test]
fn event_mode_reprocesses_recopied_source_text() {
    let mut harness = ClipboardHarness::with_text("  hello  ")
        .with_mode(RefineMode::Trim)
        .with_monitor_mode(MonitorMode::Event);

    assert!(harness.run_configured_monitor_update());
    assert_eq!(harness.clipboard_text(), "hello");

    harness.replace_clipboard("  hello  ");
    assert!(harness.run_configured_monitor_update());
    assert_eq!(harness.clipboard_text(), "hello");
}

/// Event 監視時も自身の書き戻し直後のエコーを1回スキップすること
#[test]
fn event_mode_skips_own_write_back_echo_once() {
    let mut harness = ClipboardHarness::with_text("  hello  ")
        .with_mode(RefineMode::Trim)
        .with_monitor_mode(MonitorMode::Event);

    assert!(harness.run_configured_monitor_update());
    assert_eq!(harness.clipboard_text(), "hello");

    assert!(!harness.run_configured_monitor_update());
    assert_eq!(harness.clipboard_text(), "hello");

    harness.replace_clipboard("  hello  ");
    assert!(harness.run_configured_monitor_update());
    assert_eq!(harness.clipboard_text(), "hello");
}

/// Event 監視時は加工済みテキストの再イベントでもポーリングと異なり観測を試みること
#[test]
fn event_mode_attempts_reprocess_on_unchanged_clipboard() {
    let mut polling = ClipboardHarness::with_text("  hello  ")
        .with_mode(RefineMode::Trim)
        .with_monitor_mode(MonitorMode::Polling);
    assert!(polling.run_configured_monitor_update());
    assert!(!polling.run_configured_monitor_update());

    let mut event = ClipboardHarness::with_text("  hello  ")
        .with_mode(RefineMode::Trim)
        .with_monitor_mode(MonitorMode::Event);
    assert!(event.run_configured_monitor_update());
    // 加工結果と同じ `hello` に対する再イベントでは `Unchanged` だが、ポーリングとは異なり観測する
    assert!(!event.run_configured_monitor_update());
    assert_eq!(event.clipboard_text(), "hello");
}

// ======================================================================
// 加工パイプライン
// ======================================================================
/// 監視時に `pipeline` 設定が順に適用されること
#[test]
fn monitor_applies_configured_pipeline_chain() {
    let mut harness = ClipboardHarness::with_text("  %E3%81%82  ")
        .with_mode(RefineMode::Trim)
        .with_pipeline(vec![RefineMode::UrlDecode, RefineMode::Trim]);

    assert!(harness.run_monitor_update(false));
    assert_eq!(harness.clipboard_text(), "あ");
}

/// `pipeline` 設定時は `mode` 単体ではなくパイプラインが優先されること
#[test]
fn monitor_pipeline_overrides_mode_when_active() {
    let mut harness = ClipboardHarness::with_text("hello%20world")
        .with_mode(RefineMode::Trim)
        .with_pipeline(vec![RefineMode::UrlDecode]);

    assert!(harness.run_monitor_update(false));
    assert_eq!(harness.clipboard_text(), "hello world");
}

/// パイプライン監視加工後に Undo で加工前テキストへ戻せること
#[test]
fn monitor_pipeline_then_undo_restores_original() {
    let mut harness = ClipboardHarness::with_text("  %E3%81%82  ")
        .with_pipeline(vec![RefineMode::UrlDecode, RefineMode::Trim]);

    assert!(harness.run_monitor_update(false));
    assert_eq!(harness.clipboard_text(), "あ");

    harness.undo();
    assert_eq!(harness.clipboard_text(), "  %E3%81%82  ");
}

/// パイプライン設定変更が次の監視加工に反映されること
#[test]
fn monitor_applies_updated_pipeline_from_snapshot() {
    let mut harness =
        ClipboardHarness::with_text("a%2Fb").with_pipeline(vec![RefineMode::UrlDecode]);

    assert!(harness.run_monitor_update(false));
    assert_eq!(harness.clipboard_text(), "a/b");

    harness.with_config_mut(|c| {
        c.pipeline = vec![RefineMode::UrlEncode];
    });
    harness.replace_clipboard("c d");

    assert!(harness.run_monitor_update(false));
    assert_eq!(harness.clipboard_text(), "c%20d");
}
