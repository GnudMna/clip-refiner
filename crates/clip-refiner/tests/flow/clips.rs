//! 登録クリップのワーカー経路統合テスト

use clip_refiner::test_helpers::{ClipboardHarness, with_temp_config_dir};
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

/// 登録クリップのコピーに除外形式を付けること
#[test]
fn copy_registered_clip_marks_excluded() {
    let mut harness = ClipboardHarness::with_text("registered-value");

    harness.register_clip_from_clipboard();
    harness.reset_clipboard("other");
    harness.copy_registered_clip(0);

    assert!(harness.clipboard_is_excluded());
}

/// 設定が無効なら登録クリップのコピーに除外形式を付けないこと
#[test]
fn copy_registered_clip_without_setting_is_not_excluded() {
    let mut harness = ClipboardHarness::with_text("registered-value");
    harness.with_config_mut(|c| c.keep_clipboard_exclusion = false);

    harness.register_clip_from_clipboard();
    harness.reset_clipboard("other");
    harness.copy_registered_clip(0);

    assert!(!harness.clipboard_is_excluded());
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

/// 登録クリップの画像は、自動変換が画像モードでもコピー直後に再加工しないこと
#[test]
fn copy_registered_image_avoids_immediate_reprocess() {
    with_temp_config_dir(|| {
        let mut harness = ClipboardHarness::with_text("")
            .with_source_image(1, 1, vec![1, 2, 3, 255])
            .with_mode(RefineMode::ExcelToImage);
        harness.register_clip_from_clipboard();
        harness.copy_registered_clip(0);
        assert_eq!(harness.written_image_size(), Some((1, 1)));

        // 監視方式によらず加工せず、ファイルとしては載せない
        assert!(!harness.run_monitor_update(true));
        assert!(!harness.run_monitor_update(false));
        assert!(!harness.image_written_as_file());
    });
}

/// 登録クリップの画像をコピーした後でも、別のコピーの画像は加工すること
#[test]
fn copy_registered_image_does_not_block_next_copy() {
    with_temp_config_dir(|| {
        let mut harness = ClipboardHarness::with_text("")
            .with_source_image(1, 1, vec![1, 2, 3, 255])
            .with_mode(RefineMode::ExcelToImage);
        harness.register_clip_from_clipboard();
        harness.copy_registered_clip(0);

        // 別のコピーで変更トークンが進む
        harness.copy_external_image();
        assert!(harness.run_monitor_update(true));
        assert!(harness.image_written_as_file());
    });
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

/// 編集した本文が以降のコピーに使われ、ラベルとテンプレート指定が保たれること
#[test]
fn edited_clip_is_copied_with_new_text() {
    let mut harness = ClipboardHarness::with_text("before {date}");

    harness.register_clip_from_clipboard();
    assert!(harness.with_config_mut(|c| c.set_registered_clip_template(0, true)));
    harness
        .with_config_mut(|c| c.update_registered_clip(0, "見出し", "after {{x}}"))
        .expect("edit");
    harness.reset_clipboard("other");
    harness.copy_registered_clip(0);

    // テンプレート指定が残るため、編集後の本文も展開して書き込む
    assert_eq!(harness.clipboard_text(), "after {x}");
    assert_eq!(
        harness.registered_clip_text(0).as_deref(),
        Some("after {{x}}")
    );
    assert_eq!(
        harness.with_config_mut(|c| c.clips[0].label.clone()),
        "見出し"
    );
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
    let mut harness = ClipboardHarness::with_text("{date:%+}");

    harness.register_clip_from_clipboard();
    harness.with_config_mut(|c| c.set_registered_clip_template(0, true));
    harness.reset_clipboard("unchanged");
    harness.copy_registered_clip(0);

    assert_eq!(harness.clipboard_text(), "unchanged");
}

/// `{counter}` を含むテンプレートはコピーのたびに連番を 1 ずつ進め、リセットで 1 に戻ること
#[test]
fn copy_template_clip_advances_counter() {
    let mut harness = ClipboardHarness::with_text("No.{counter} / {counter}");

    harness.register_clip_from_clipboard();
    assert!(harness.with_config_mut(|c| c.set_registered_clip_template(0, true)));

    harness.copy_registered_clip(0);
    assert_eq!(harness.clipboard_text(), "No.1 / 1");
    harness.copy_registered_clip(0);
    assert_eq!(harness.clipboard_text(), "No.2 / 2");
    assert_eq!(harness.with_config_mut(|c| c.clips[0].counter), 2);

    assert!(harness.with_config_mut(|c| c.set_registered_clip_counter(0, 0)));
    harness.copy_registered_clip(0);
    assert_eq!(harness.clipboard_text(), "No.1 / 1");
}

/// `{counter}` を含まないテンプレートと、展開に失敗したコピーでは連番を進めないこと
#[test]
fn copy_does_not_advance_counter_without_issuing_it() {
    let mut harness = ClipboardHarness::with_text("{date:%Y}");
    harness.register_clip_from_clipboard();
    assert!(harness.with_config_mut(|c| c.set_registered_clip_template(0, true)));
    harness.copy_registered_clip(0);
    assert_eq!(harness.with_config_mut(|c| c.clips[0].counter), 0);

    // 不正な書式で展開に失敗した場合も連番は変わらない
    harness
        .with_config_mut(|c| c.update_registered_clip(0, "", "{counter}{date:%+}"))
        .expect("edit");
    harness.reset_clipboard("unchanged");
    harness.copy_registered_clip(0);
    assert_eq!(harness.clipboard_text(), "unchanged");
    assert_eq!(harness.with_config_mut(|c| c.clips[0].counter), 0);

    // テンプレートでないクリップの `{counter}` は展開せず、連番も進めない
    harness
        .with_config_mut(|c| c.update_registered_clip(0, "", "{counter}"))
        .expect("edit");
    assert!(harness.with_config_mut(|c| c.set_registered_clip_template(0, false)));
    harness.copy_registered_clip(0);
    assert_eq!(harness.clipboard_text(), "{counter}");
    assert_eq!(harness.with_config_mut(|c| c.clips[0].counter), 0);
}
