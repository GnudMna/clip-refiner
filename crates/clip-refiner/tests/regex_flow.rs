//! 正規表現加工の統合テスト

use clip_refiner::RefineMode;
use clip_refiner::test_helpers::{ClipboardHarness, MonitorMode};

// ======================================================================
// 監視経路
// ======================================================================
/// 監視加工が `config` の正規表現設定を参照すること
#[test]
fn monitor_applies_regex_replace_from_config() {
    let mut harness = ClipboardHarness::with_text("2024-01-15").with_mode(RefineMode::RegexReplace);
    harness.with_config_mut(|c| {
        c.regex.pattern = r"(\d{4})-(\d{2})-(\d{2})".to_string();
        c.regex.replacement = "$1/$2/$3".to_string();
    });

    assert!(harness.run_configured_monitor_update());
    assert_eq!(harness.clipboard_text(), "2024/01/15");
}

/// 同一設定で連続加工しても正規表現キャッシュが機能すること
#[test]
fn monitor_reuses_regex_cache_across_updates() {
    let mut harness = ClipboardHarness::with_text("2024-01-15").with_mode(RefineMode::RegexReplace);
    harness.with_config_mut(|c| {
        c.regex.pattern = r"(\d{4})-(\d{2})-(\d{2})".to_string();
        c.regex.replacement = "$1/$2/$3".to_string();
    });

    assert!(harness.run_configured_monitor_update());
    assert_eq!(harness.clipboard_text(), "2024/01/15");

    harness.replace_clipboard("1999-12-31");
    assert!(harness.run_configured_monitor_update());
    assert_eq!(harness.clipboard_text(), "1999/12/31");
}

/// Event 監視方式でも正規表現加工が動作すること
#[test]
fn event_mode_applies_regex_replace() {
    let mut harness = ClipboardHarness::with_text("a1b2")
        .with_mode(RefineMode::RegexDelete)
        .with_monitor_mode(MonitorMode::Event);
    harness.with_config_mut(|c| c.regex.pattern = r"\d".to_string());

    assert!(harness.run_configured_monitor_update());
    assert_eq!(harness.clipboard_text(), "ab");
}
