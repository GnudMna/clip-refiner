//! 監視ループ経路の統合テスト

use clip_refiner::test_helpers::{
    ClipboardHarness, MAX_CLIPBOARD_TEXT_BYTES, MonitorMode, with_temp_config_dir,
};
use clip_refiner_core::{RefineContext, RefineMode};

// ======================================================================
// 監視加工
// ======================================================================
// ======================================================================
// テスト
// ======================================================================

/// 監視加工後に Undo で加工前テキストへ戻せるようにすること
#[test]
fn monitor_process_then_undo_restores_original() {
    let mut harness = ClipboardHarness::with_text("  hello  ").with_mode(RefineMode::Trim);

    assert!(harness.run_monitor_update(false));
    assert_eq!(harness.clipboard_text(), "hello");

    harness.undo();
    assert_eq!(harness.clipboard_text(), "  hello  ");
}

/// 自動変換が無効なときは監視でクリップボードを加工しないこと
#[test]
fn monitor_skips_processing_when_auto_refine_disabled() {
    let mut harness = ClipboardHarness::with_text("  hello  ").with_mode(RefineMode::Trim);
    harness.with_config_mut(|c| c.auto_refine_enabled = false);

    assert!(!harness.run_monitor_update(false));
    assert!(!harness.run_monitor_update(true));
    assert_eq!(harness.clipboard_text(), "  hello  ");
}

/// ポーリング時に同一テキストの再観測では再加工しないようにすること
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

/// 設定の変換モード変更が次の監視加工に反映されるようにすること
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

/// 連続記録が有効なときは自動変換が有効でも自動加工を実行しないようにすること
#[test]
fn continuous_record_disables_auto_processing_even_when_auto_refine_enabled() {
    let mut harness = ClipboardHarness::with_text("  hello  ")
        .with_mode(RefineMode::Trim)
        .with_continuous_record(true)
        .with_monitor_mode(MonitorMode::Event);

    assert!(!harness.run_configured_monitor_update());
    assert_eq!(harness.clipboard_text(), "  hello  ");
    assert_eq!(harness.continuous_record_entries(), vec!["  hello  "]);
}

/// 履歴件数が `history_limit` を超えないようにすること
#[test]
fn monitor_respects_history_limit() {
    let mut harness = ClipboardHarness::with_text("  a  ")
        .with_mode(RefineMode::Trim)
        .with_history(true);
    harness.with_config_mut(|c| c.history_limit = 2);

    for text in ["  a  ", "  b  ", "  c  "] {
        harness.replace_clipboard(text);
        assert!(harness.run_monitor_update(false));
    }

    assert_eq!(harness.history_len(), 2);
    assert_eq!(harness.history_entries(), vec!["c", "b"]);
}

/// 上限超過テキストは監視加工をスキップしクリップボードを変更しないようにすること
#[test]
fn monitor_skips_oversized_clipboard_without_modifying() {
    let oversized = "a".repeat(MAX_CLIPBOARD_TEXT_BYTES + 1);
    let mut harness = ClipboardHarness::with_text(oversized.clone()).with_mode(RefineMode::Trim);

    assert!(!harness.run_monitor_update(false));
    assert_eq!(harness.clipboard_text(), oversized);
    assert_eq!(harness.history_len(), 0);
}

// ======================================================================
// Event 監視方式
// ======================================================================
/// Event 監視方式でも加工が実行されるようにすること
#[test]
fn event_mode_processes_clipboard() {
    let mut harness = ClipboardHarness::with_text("  hello  ")
        .with_mode(RefineMode::Trim)
        .with_monitor_mode(MonitorMode::Event);

    assert!(harness.run_configured_monitor_update());
    assert_eq!(harness.clipboard_text(), "hello");
}

/// Event 監視方式でも Undo で加工前テキストへ戻せるようにすること
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

/// Event 監視時は元テキストの再コピーでも再加工されるようにすること
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

/// 内容が変わらないクリップボードでも、イベント監視は加工を試み、ポーリングはスキップすること
///
/// 自身の書き戻しエコーを消化したあとで、同じ内容を変化させるモードへ切り替えて比べる
#[test]
fn event_mode_attempts_reprocess_on_unchanged_clipboard() {
    for (monitor_mode, expected_written, expected_text) in [
        (MonitorMode::Polling, false, "hello"),
        (MonitorMode::Event, true, "ｈｅｌｌｏ"),
    ] {
        let mut harness = ClipboardHarness::with_text("  hello  ")
            .with_mode(RefineMode::Trim)
            .with_monitor_mode(monitor_mode);
        assert!(harness.run_configured_monitor_update());
        // 書き戻しのエコーはどちらの方式でもスキップする
        assert!(!harness.run_configured_monitor_update());

        // クリップボードは `hello` のまま、加工すれば変わるモードへ切り替える
        harness.set_mode(RefineMode::ToFullwidth);
        assert_eq!(
            harness.run_configured_monitor_update(),
            expected_written,
            "{monitor_mode:?}"
        );
        assert_eq!(harness.clipboard_text(), expected_text, "{monitor_mode:?}");
    }
}

// ======================================================================
// 加工パイプライン
// ======================================================================
/// 監視時に `pipeline` 設定が順に適用されるようにすること
#[test]
fn monitor_applies_configured_pipeline_chain() {
    let mut harness = ClipboardHarness::with_text("  %E3%81%82  ")
        .with_mode(RefineMode::Trim)
        .with_pipeline(vec![RefineMode::UrlDecode, RefineMode::Trim]);

    assert!(harness.run_monitor_update(false));
    assert_eq!(harness.clipboard_text(), "あ");
}

/// `pipeline` 設定時は `mode` 単体ではなくパイプラインが優先されるようにすること
#[test]
fn monitor_pipeline_overrides_mode_when_active() {
    let mut harness = ClipboardHarness::with_text("hello%20world")
        .with_mode(RefineMode::Trim)
        .with_pipeline(vec![RefineMode::UrlDecode]);

    assert!(harness.run_monitor_update(false));
    assert_eq!(harness.clipboard_text(), "hello world");
}

/// パイプライン監視加工後に Undo で加工前テキストへ戻せるようにすること
#[test]
fn monitor_pipeline_then_undo_restores_original() {
    let mut harness = ClipboardHarness::with_text("  %E3%81%82  ")
        .with_pipeline(vec![RefineMode::UrlDecode, RefineMode::Trim]);

    assert!(harness.run_monitor_update(false));
    assert_eq!(harness.clipboard_text(), "あ");

    harness.undo();
    assert_eq!(harness.clipboard_text(), "  %E3%81%82  ");
}

/// パイプライン設定変更が次の監視加工に反映されるようにすること
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

/// 監視でリッチテキストを書き込み、直後のエコーでは再加工しないこと
#[test]
fn monitor_writes_rich_text_and_skips_echo() {
    let mut harness =
        ClipboardHarness::with_text("# Title").with_mode(RefineMode::MarkdownToRichText);

    assert!(harness.run_monitor_update(true));
    assert_eq!(harness.clipboard_text(), "# Title");
    assert_eq!(harness.clipboard_html(), Some("<h1>Title</h1>"));

    // 書き込みによる変更通知ではテキストが同じため加工しない
    assert!(!harness.run_monitor_update(true));
    assert_eq!(harness.clipboard_html(), Some("<h1>Title</h1>"));
}

/// 監視でコピーされた HTML 形式を Markdown テキストへ変換すること
#[test]
fn monitor_converts_copied_html_to_markdown() {
    let mut harness = ClipboardHarness::with_text("Docs")
        .with_html(r#"<a href="https://example.com/">Docs</a>"#)
        .with_mode(RefineMode::HtmlToMarkdown);

    assert!(harness.run_monitor_update(true));
    assert_eq!(harness.clipboard_text(), "[Docs](https://example.com/)");
    assert_eq!(harness.clipboard_html(), None);

    // Undo では加工前のテキストへ戻す
    harness.undo();
    assert_eq!(harness.clipboard_text(), "Docs");
}

/// 自動変換が画像モードでも、手動で書き込んだ画像は再加工せず Undo で元へ戻せること
#[test]
fn manual_image_is_not_reprocessed_and_can_be_undone() {
    let mut harness = ClipboardHarness::with_text("").with_mode(RefineMode::ExcelToImage);
    harness.copy_excel("a	b");
    harness.process_mode(RefineMode::ExcelToImage);

    // 監視方式によらず加工しない
    assert!(!harness.run_monitor_update(true));
    assert!(!harness.run_monitor_update(false));

    harness.undo();
    assert_eq!(harness.clipboard_text(), "a	b");
}

/// 自動変換が画像モードでも、CLI から書き込んだ画像は再加工せず Undo で元へ戻せること
#[test]
fn oneshot_image_is_not_reprocessed_and_can_be_undone() {
    let mut harness = ClipboardHarness::with_text("").with_mode(RefineMode::ExcelToImage);
    harness.copy_excel("a	b");
    let _ =
        harness.process_oneshot_pipeline(vec![RefineMode::ExcelToImage], RefineContext::default());

    assert!(!harness.run_monitor_update(true));

    harness.undo();
    assert_eq!(harness.clipboard_text(), "a	b");
}

/// 手動で画像を書き込んだ後でも、同じ内容のコピーを監視方式によらず自動変換すること
#[test]
fn same_copy_after_manual_image_is_processed() {
    for event_driven in [true, false] {
        // 自動変換が画像モードの場合
        let mut harness = ClipboardHarness::with_text("").with_mode(RefineMode::ExcelToImage);
        harness.copy_excel("a	b");
        harness.process_mode(RefineMode::ExcelToImage);
        assert!(!harness.run_monitor_update(event_driven));
        harness.copy_excel("a	b");
        assert!(harness.run_monitor_update(event_driven));

        // 自動変換がテキストのモードの場合
        let mut harness = ClipboardHarness::with_text("").with_mode(RefineMode::Trim);
        harness.copy_excel(" a	b ");
        harness.process_mode(RefineMode::ExcelToImage);
        assert!(!harness.run_monitor_update(event_driven));
        harness.copy_excel(" a	b ");
        assert!(harness.run_monitor_update(event_driven));
        assert_eq!(harness.clipboard_text(), "a	b");
    }
}

/// 自動変換で画像にした後でも、同じ内容のコピーを監視方式によらずもう一度変換すること
#[test]
fn same_copy_after_auto_image_is_processed() {
    for event_driven in [true, false] {
        let mut harness = ClipboardHarness::with_text("").with_mode(RefineMode::ExcelToImage);
        for _ in 0..3 {
            harness.copy_excel("a	b");
            assert!(harness.run_monitor_update(event_driven));
        }

        harness.undo();
        assert_eq!(harness.clipboard_text(), "a	b");
    }
}

/// 登録クリップの画像をコピーした後は、以前に書き戻した本文と同じコピーも加工すること
#[test]
fn same_copy_after_registered_image_is_processed() {
    with_temp_config_dir(|| {
        let mut harness = ClipboardHarness::with_text("").with_mode(RefineMode::Trim);
        harness.copy_excel("");
        harness.register_clip_from_clipboard();

        // 履歴復元などの書き戻しを監視が観測する前に、登録クリップの画像で置き換える
        harness.set_text(" text ");
        harness.copy_registered_clip(0);
        harness.replace_clipboard(" text ");
        assert!(harness.run_monitor_update(true));
        assert_eq!(harness.clipboard_text(), "text");
    });
}
