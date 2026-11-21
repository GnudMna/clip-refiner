//! 登録クリップのワーカー経路統合テスト

use clip_refiner::test_helpers::ClipboardHarness;
use clip_refiner_core::RefineMode;

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

/// テンプレートの登録クリップはプレースホルダを展開してコピーすること
#[test]
fn copy_template_clip_expands_placeholders() {
    let mut harness = ClipboardHarness::with_text("[{clipboard}] {date} {{x}}");

    harness.register_clip_from_clipboard();
    assert!(harness.with_config_mut(|c| c.set_registered_clip_template(0, true)));
    harness.reset_clipboard("copied");
    harness.copy_registered_clip(0);

    // `{clipboard}` は展開直前の内容、`{date}` は YYYY-MM-DD、`{{` `}}` は波括弧 1 文字
    let text = harness.clipboard_text().to_string();
    let (head, tail) = text.split_at("[copied] ".len());
    assert_eq!(head, "[copied] ");
    let (date, rest) = tail.split_at("YYYY-MM-DD".len());
    assert!(
        date.chars().enumerate().all(|(i, c)| if i == 4 || i == 7 {
            c == '-'
        } else {
            c.is_ascii_digit()
        }),
        "日付形式ではない: {date}"
    );
    assert_eq!(rest, " {x}");

    // 登録済みの本文はテンプレートのまま残る
    assert_eq!(
        harness.registered_clip_text(0).as_deref(),
        Some("[{clipboard}] {date} {{x}}")
    );
    // 展開結果のコピー直後も監視で再加工しない
    assert!(!harness.run_monitor_update(false));
}

/// テンプレートでないクリップはプレースホルダを展開しないこと
#[test]
fn copy_plain_clip_keeps_placeholders() {
    let mut harness = ClipboardHarness::with_text("{date}");

    harness.register_clip_from_clipboard();
    harness.reset_clipboard("other");
    harness.copy_registered_clip(0);

    assert_eq!(harness.clipboard_text(), "{date}");
}

/// 展開に失敗したテンプレートはクリップボードを変更しないこと
#[test]
fn copy_template_clip_with_invalid_format_keeps_clipboard() {
    let mut harness = ClipboardHarness::with_text("{date:%Q}");

    harness.register_clip_from_clipboard();
    harness.with_config_mut(|c| c.set_registered_clip_template(0, true));
    harness.reset_clipboard("unchanged");
    harness.copy_registered_clip(0);

    assert_eq!(harness.clipboard_text(), "unchanged");
}
